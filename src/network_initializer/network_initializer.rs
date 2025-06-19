use std::collections::HashMap;
use std::{fs, thread};
use bagel_bomber::BagelBomber;
use bevy::prelude::{ResMut, Vec2};
use crossbeam_channel::{unbounded, Receiver, Sender};
use fungi_drone::FungiDrone;
use Krusty_Club::Krusty_C;
use LeDron_James::Drone as LeDron;
use lockheedrustin_drone::LockheedRustin;
use rustafarian_drone::RustafarianDrone;
use rustbusters_drone::RustBustersDrone;
use rusteze_drone::RustezeDrone;
use skylink::SkyLinkDrone;
use wg_2024::config::{Config};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;
use wg_2024_rust::drone::RustDrone;
use crate::clients::chat_client::ChatClient;
use crate::clients::web_browser::WebBrowser;
use crate::common_data::common::{BackGroundFlood, ChatClientEvent, CommandChat, ContentCommands, ServerCommands, ServerEvent, WebBrowserEvents};
use crate::gui::login_window::{NodeConfig, NodeType};
use crate::simulation_control::simulation_control::SimulationController;
use crate::gui::shared_info_plugin::{NodeCategory, ERROR_VERIFY, SHARED_STATE};
use crate::network_initializer::connection_validity::{validate_drone_pdr, validate_duplex_connections, validate_generic_configuration, would_break_connectivity};
use crate::servers::chat_server_fillo::Server;
use crate::servers::text_server_fillo::Server as TextServerBaia;
use crate::servers::media_server_fillo::Server as MediaServerBaia;
use crate::servers::chat_max::Server as ChatMax;
use crate::servers::text_max::Server as TextMax;

/// Parses the configuration file based on the active feature flags
/// Returns a Config struct containing the network topology
pub fn parse_config() -> Config {
    let file_str = if cfg!(feature = "web") {
        // Web topology for web-based simulations
        fs::read_to_string("assets/configurations/web_topology.toml").unwrap()
    } else if cfg!(feature = "full") {
        // Full topology for complete simulations
        fs::read_to_string("assets/configurations/full_topology.toml").unwrap()
    } else {
        // Default chat topology
        fs::read_to_string("assets/configurations/chat_topology.toml").unwrap()
    };
    toml::from_str(&file_str).unwrap()
}

/// Sets up all communication channels, spawns drones, servers, and clients
/// and initializes the simulation controller
pub fn start_simulation(
    mut simulation_controller: ResMut<SimulationController>
) {
    // Parse configuration from TOML file
    let config = parse_config();

    // Set up all communication channels between nodes
    let (packet_channels, command_chat_channel,
        command_web_channel, background_flooding, server_commands) =
        setup_communication_channels(&config);

    // Create event channels for different node types
    let (chat_event_send, chat_event_recv) = unbounded();
    let (web_event_send, web_event_recv) = unbounded();
    let (server_event_send, server_event_recv) = unbounded();

    // Create a map of node neighbors for routing
    let neighbours = create_neighbours_map(&config);

    // Initialize storage for different node types (some of it probably useless)
    let mut controller_drones = HashMap::new();
    let mut packet_drones = HashMap::new();
    let node_event_send = simulation_controller.node_event_send.clone();
    let node_event_recv = simulation_controller.node_event_recv.clone();
    let mut client = simulation_controller.client.clone();
    let mut web_client = simulation_controller.web_client.clone();
    let mut text_servers = simulation_controller.text_server.clone();
    let mut media_servers = simulation_controller.media_server.clone();
    let mut chat_servers = simulation_controller.chat_server.clone();
    let mut background_flood = simulation_controller.background_flooding.clone();

    // Spawn all drone instances and get IDs of Rustafarian drones (since they decided to use non protocol based hop_indexes)
    let rustafarian_ids=spawn_drones(
        &config,
        &mut controller_drones,
        &mut packet_drones,
        &packet_channels,
        node_event_send.clone()
    );

    // Spawn servers based on feature flags
    #[cfg(feature = "max")]
    {
        spawn_servers_max(
            &config,
            &packet_channels,
            &background_flooding,
            &mut background_flood,
            &server_commands,
            &mut text_servers,
            &mut chat_servers,
            server_event_send.clone(),
        );
    }
    #[cfg(not(feature = "max"))]
    {
        spawn_servers_baia(
            &config,
            &packet_channels,
            &background_flooding,
            &mut background_flood,
            &server_commands,
            &mut text_servers,
            &mut media_servers,
            &mut chat_servers,
            server_event_send.clone(),
        );
    }

    // Calculate total number of servers
    let n_servers=text_servers.len()+chat_servers.len()+media_servers.len();

    // Spawn client instances (chat clients and web browsers)
    spawn_clients(
        &config,
        &packet_channels,
        &command_chat_channel,
        &command_web_channel,
        &background_flooding,
        &mut client,
        &mut web_client,
        &mut background_flood,
        chat_event_send.clone(),
        web_event_send.clone(),
        n_servers
    );

    // Update the simulation controller with all initialized components
    update_simulation_controller(
        &mut simulation_controller,
        node_event_send.clone(),
        controller_drones,
        node_event_recv,
        neighbours,
        packet_channels,
        client.clone(),
        web_client.clone(),
        text_servers.clone(),
        media_servers.clone(),
        chat_servers.clone(),
        background_flood
    );

    // Check which client types are active (since it's needed to close one of the communication channel)
    let web_active=!web_client.is_empty();
    let chat_active=!client.is_empty();

    // Create the main simulation controller
    let mut controller = create_simulation_controller(
        node_event_send,
        simulation_controller,
        chat_event_recv,
        web_event_recv,
        server_event_recv,
        web_active,
        chat_active,
        rustafarian_ids
    );

    // Run the controller in a separate thread
    thread::spawn(move || {
        controller.run();
    });

    // Build a map of all nodes with their categories (used to differentiate between types in GUI)
    let mut nodes=HashMap::new();
    for client in client.keys(){
        nodes.insert(*client, NodeCategory::Client(NodeType::ChatClient));
    }
    for web in web_client.keys(){
        nodes.insert(*web, NodeCategory::Client(NodeType::WebBrowser));
    }
    for text in text_servers.keys(){
        nodes.insert(*text, NodeCategory::Server(NodeType::TextServer));
    }
    for media in media_servers.keys(){
        nodes.insert(*media, NodeCategory::Server(NodeType::MediaServer));
    }
    for chat in chat_servers.keys(){
        nodes.insert(*chat, NodeCategory::Server(NodeType::ChatServer));
    }

    // Validate the network configuration - checks for
    // 1. Client/Server without neighbors
    // 2. Drone PDR higher than 1.00 or lower than 0.00
    // 3. All connections in full duplex
    // 4. Duplicated IDs, nodes with themselves as neighbors, Client/Servers connected between each other without drones in between
    let isolated_node=would_break_connectivity(&convert_to_config(config.clone(), nodes.clone()));
    let wrong_pdr=validate_drone_pdr(&convert_to_config(config.clone(), nodes.clone()));
    let connection_error=validate_duplex_connections(&convert_to_config(config.clone(), nodes.clone()));
    let generic_misconfiguration= validate_generic_configuration(&convert_to_config(config, nodes.clone()));

    // Tells GUI output of the checks, GUI will decide
    // whether to change the AppState to InGame (if no errors)
    // or stay in AppState::SetUp if errors occurred, in this last case a message will also be displayed
    if let Ok(mut state) = ERROR_VERIFY.write(){
        state.connection_error=(false,connection_error.clone());
        state.wrong_pdr=(false, wrong_pdr.clone());
        state.isolated_node=(false, isolated_node.clone());
        state.generic_misconfiguration=(false, generic_misconfiguration);
        state.is_updated=true;
    }

    // Update shared state with node information (probably useless, was useful when checks weren't implemented)
    if let Ok(mut state)=SHARED_STATE.write(){
        state.nodes=nodes;
        state.is_updated=true;
    }
}

/// Sets up all communication channels needed for the simulation
/// Returns channels for packets, chat commands, web commands, background flooding, and server commands
fn setup_communication_channels(config: &Config) -> (
    HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>,
    HashMap<NodeId, (Sender<CommandChat>, Receiver<CommandChat>)>,
    HashMap<NodeId, (Sender<ContentCommands>, Receiver<ContentCommands>)>,
    HashMap<NodeId, (Sender<BackGroundFlood>, Receiver<BackGroundFlood>)>,
    HashMap<NodeId, (Sender<ServerCommands>, Receiver<ServerCommands>)>
) {
    let mut packet_channels = HashMap::new();
    let mut command_chat_channel = HashMap::new();
    let mut command_web_channel = HashMap::new();
    let mut background_flood=HashMap::new();
    let mut server_commands = HashMap::new();

    // Create *packet_channels* for all nodes (drones, clients, servers)
    // *packet_channels* is the main communication channel, where Packets are exchanged between nodes
    for node_id in config.drone.iter().map(|d| d.id)
        .chain(config.client.iter().map(|c| c.id))
        .chain(config.server.iter().map(|s| s.id)) {
        packet_channels.insert(node_id, unbounded());
    }

    // Create command and background flood channels for clients
    // command_chat_channel => SC sends CommandChat to chat_clients
    // command_web_channel => SC sends ContentCommands to web_browser
    // Note how this channels appear for all types of clients not only chat_clients and web_browser **
    for client in &config.client {
        command_chat_channel.insert(client.id, unbounded());
        command_web_channel.insert(client.id, unbounded());
        background_flood.insert(client.id, unbounded());
    }

    // Create command and background flood channels for servers
    // server_commands should be used only to request
    // -Connection Graph to servers
    // -RemoveSender to servers
    // -AddSender to servers
    for server in &config.server{
        server_commands.insert(server.id, unbounded());
        background_flood.insert(server.id, unbounded());
    }

    (packet_channels, command_chat_channel, command_web_channel, background_flood, server_commands)
}

/// Creates a map of node neighbors from the configuration
/// Used for routing decisions in the drone network
fn create_neighbours_map(config: &Config) -> HashMap<NodeId, Vec<NodeId>> {
    let mut neighbours = HashMap::new();
    for drone in &config.drone {
        neighbours.insert(drone.id, drone.connected_node_ids.clone());
    }
    neighbours
}

/// Spawns all drone instances based on configuration
/// Returns a list of Rustafarian drone IDs (useful since Rustafarian doesn't follow protocol when sending hop_index of nacks)
fn spawn_drones(
    config: &Config,
    controller_drones: &mut HashMap<NodeId, Sender<DroneCommand>>,
    packet_drones: &mut HashMap<NodeId, Sender<Packet>>,
    packet_channels: &HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>,
    node_event_send: Sender<DroneEvent>
)->Vec<NodeId> {
    let mut rustafarian_drone_ids = Vec::new();

    // Iterate through all drones in config and spawn them
    for (i,cfg_drone) in config.drone.iter().cloned().enumerate() {
        // Create control channel for this drone (used by SC to send commands to drone, e.g. RemoveSender)
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(cfg_drone.id, controller_drone_send);
        packet_drones.insert(cfg_drone.id, packet_channels[&cfg_drone.id].0.clone());

        // Every 10th drone starting at index 3 is a Rustafarian drone (problematic drone)
        if i % 10 == 3{
            rustafarian_drone_ids.push(cfg_drone.id);
        }

        // Retrieving Sender and Receiver for DroneEvent, how SC can listen to actions made by Drones
        let node_event_send_clone = node_event_send.clone();
        let packet_recv = packet_channels[&cfg_drone.id].1.clone();

        // Create packet send channels for all connected nodes
        let packet_send = cfg_drone.connected_node_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_, _>>();

        // Spawn drone in a new thread
        thread::spawn(move || {
            let drone = create_drone(
                cfg_drone.id,
                node_event_send_clone,
                controller_drone_recv,
                packet_recv,
                packet_send,
                cfg_drone.pdr,
                i
            );

            if let Some(mut drone) = drone {
                drone.run();
            }
        });
    }
    rustafarian_drone_ids
}

/// Creates a drone instance based on the index
fn create_drone(
    id: NodeId,
    node_event_send: Sender<DroneEvent>,
    controller_drone_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<u8, Sender<Packet>>,
    pdr: f32,
    i: usize
) -> Option<Box<dyn Drone>> {
    // Select drone type based on index modulo 10
    match i % 10 {
        0 => Some(Box::new(BagelBomber::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        1 => Some(Box::new(SkyLinkDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        2 => Some(Box::new(FungiDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        3 => Some(Box::new(RustafarianDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        4 => Some(Box::new(RustezeDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        5 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        6 => Some(Box::new(RustDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        7 => Some(Box::new(RustBustersDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        8 => Some(Box::new(LeDron::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        9 => Some(Box::new(Krusty_C::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        _ => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
    }
}

/// Spawns servers for the "baia" configuration
/// Server types are determined based on the number of clients and servers
#[allow(dead_code)]
fn spawn_servers_baia(
    config: &Config,
    packet_channels: &HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>,
    background_flood: &HashMap<NodeId, (Sender<BackGroundFlood>, Receiver<BackGroundFlood>)>,
    flooding: &mut HashMap<NodeId, Sender<BackGroundFlood>>,
    server_commands: &HashMap<NodeId, (Sender<ServerCommands>, Receiver<ServerCommands>)>,
    text_servers: &mut HashMap<NodeId, Sender<ServerCommands>>,
    media_servers: &mut HashMap<NodeId, Sender<ServerCommands>>,
    chat_servers: &mut HashMap<NodeId, Sender<ServerCommands>>,
    server_event_send: Sender<ServerEvent>
) {
    let n_clients = config.client.len();
    let n_servers = config.server.len();

    for (i, cfg_server) in config.server.iter().cloned().enumerate() {
        // Set up channels for this server
        let rcv = packet_channels[&cfg_server.id].1.clone();
        let packet_send = cfg_server.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_, _>>();
        let rcv_flood = background_flood[&cfg_server.id].1.clone();
        flooding.insert(cfg_server.id, background_flood[&cfg_server.id].0.clone());
        let rcv_command = server_commands[&cfg_server.id].1.clone();

        // Spawn different server types based on client/server configuration
        match (n_clients, n_servers) {
            (1, 1) => {
                // Single client, single server: text server
                spawn_text_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), text_servers, server_commands,
                                  "assets/multimedia/paths/text_server1.txt", n_servers);
            },
            (2, 1) => {
                // Two clients, one server: chat server
                spawn_chat_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (2, 2) => {
                // Two clients, two servers: both chat servers
                spawn_chat_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (3, 1) => {
                // Three clients, one server: chat server
                spawn_chat_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (3, 2) => {
                // Three clients, two servers: chat and text
                match i {
                    0 => spawn_chat_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                           server_event_send.clone(), chat_servers, server_commands, n_servers),
                    1 => spawn_text_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                           server_event_send.clone(), text_servers, server_commands,
                                           "assets/multimedia/paths/text_server1.txt", n_servers),
                    _ => unreachable!()
                }
            },
            (1, 3) => {
                // One client, three servers: mixed media, text, and media
                match i {
                    0 => spawn_media_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                            server_event_send.clone(), media_servers, server_commands,"assets/multimedia/paths/media_server2.txt", n_servers),
                    1 => spawn_text_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                           server_event_send.clone(), text_servers, server_commands,
                                           "assets/multimedia/paths/text_server1.txt", n_servers),
                    2 => spawn_media_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                            server_event_send.clone(), media_servers, server_commands,
                                            "assets/multimedia/paths/media_server1.txt", n_servers),
                    _ => unreachable!()
                }
            },
            _ => {
                // Default configuration for 3+ servers
                if n_servers >= 3 {
                    match i  {
                        0 => spawn_chat_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                               server_event_send.clone(), chat_servers, server_commands, n_servers),
                        1 => spawn_text_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                               server_event_send.clone(), text_servers, server_commands,
                                               "assets/multimedia/paths/text_server1.txt", n_servers),
                        2 => spawn_media_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                                server_event_send.clone(), media_servers, server_commands,
                                                "assets/multimedia/paths/media_server1.txt", n_servers),
                        _=> spawn_media_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                               server_event_send.clone(), media_servers, server_commands,
                                               "assets/multimedia/paths/media_server2.txt", n_servers),

                    }
                } else {
                    // Fallback to chat server
                    spawn_chat_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                      server_event_send.clone(), chat_servers, server_commands, n_servers);
                }
            }
        }
    }
}

/// Spawns a chat server instance
#[allow(dead_code)]
fn spawn_chat_server(
    id: NodeId,
    rcv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    rcv_flood: Receiver<BackGroundFlood>,
    rcv_command: Receiver<ServerCommands>,
    server_event_send: Sender<ServerEvent>,
    chat_servers: &mut HashMap<NodeId, Sender<ServerCommands>>,
    server_commands: &HashMap<NodeId, (Sender<ServerCommands>, Receiver<ServerCommands>)>,
    n_servers: usize
) {
    let mut server = Server::new(id, rcv, packet_send, rcv_flood, rcv_command, server_event_send);
    thread::spawn(move || {
        server.run();
    });
    chat_servers.insert(id, server_commands[&id].0.clone());
    set_node_types(NodeType::ChatServer, n_servers, id);
}

/// Spawns a text server instance
#[allow(dead_code)]
fn spawn_text_server(
    id: NodeId,
    rcv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    rcv_flood: Receiver<BackGroundFlood>,
    rcv_command: Receiver<ServerCommands>,
    server_event_send: Sender<ServerEvent>,
    text_servers: &mut HashMap<NodeId, Sender<ServerCommands>>,
    server_commands: &HashMap<NodeId, (Sender<ServerCommands>, Receiver<ServerCommands>)>,
    path: &str,
    n_servers: usize
) {
    let mut server = TextServerBaia::new(id, rcv, packet_send, rcv_flood, rcv_command, server_event_send, path);
    thread::spawn(move || {
        server.run();
    });
    text_servers.insert(id, server_commands[&id].0.clone());
    set_node_types(NodeType::TextServer, n_servers, id);
}

/// Spawns a media server instance
#[allow(dead_code)]
fn spawn_media_server(
    id: NodeId,
    rcv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    rcv_flood: Receiver<BackGroundFlood>,
    rcv_command: Receiver<ServerCommands>,
    server_event_send: Sender<ServerEvent>,
    media_servers: &mut HashMap<NodeId, Sender<ServerCommands>>,
    server_commands: &HashMap<NodeId, (Sender<ServerCommands>, Receiver<ServerCommands>)>,
    path: &str,
    n_servers: usize
) {
    let mut server = MediaServerBaia::new(id, rcv, packet_send, rcv_flood, rcv_command, server_event_send, path);
    thread::spawn(move || {
        server.run();
    });
    media_servers.insert(id, server_commands[&id].0.clone());
    set_node_types(NodeType::MediaServer, n_servers, id);
}

/// Spawns client instances (chat clients and web browsers)
/// Client types are determined based on the number of clients and servers
fn spawn_clients(
    config: &Config,
    packet_channels: &HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>,
    command_chat_channel: &HashMap<NodeId, (Sender<CommandChat>, Receiver<CommandChat>)>,
    command_web_channel: &HashMap<NodeId, (Sender<ContentCommands>, Receiver<ContentCommands>)>,
    background_flood: &HashMap<NodeId, (Sender<BackGroundFlood>, Receiver<BackGroundFlood>)>,
    client: &mut HashMap<NodeId, Sender<CommandChat>>,
    web_client: &mut HashMap<NodeId, Sender<ContentCommands>>,
    flooding: &mut HashMap<NodeId, Sender<BackGroundFlood>>,
    chat_event_send: Sender<ChatClientEvent>,
    web_event_send: Sender<WebBrowserEvents>,
    n_servers: usize
) {
    let n_clients = config.client.len();

    for (i, cfg_client) in config.client.iter().cloned().enumerate() {
        // Set up channels for this client
        let packet_send: HashMap<NodeId, Sender<Packet>> = cfg_client.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect();
        let rcv_packet = packet_channels[&cfg_client.id].1.clone();
        let rcv_flood = background_flood[&cfg_client.id].1.clone();
        flooding.insert(cfg_client.id, background_flood[&cfg_client.id].0.clone());

        // Spawn different client types based on configuration
        match (n_clients, n_servers) {
            (1, _) => {
                // Single client: web browser
                spawn_web_browser(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                  command_web_channel, web_client, web_event_send.clone(), n_clients);
            },
            (2, _) => {
                // Two clients: both chat clients
                spawn_chat_client(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                  command_chat_channel, client, chat_event_send.clone(), n_clients);
            },
            (3, 1) => {
                // Three clients, one server: all chat clients
                spawn_chat_client(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                  command_chat_channel, client, chat_event_send.clone(), n_clients);
            },
            (3, 2) | (3,3) => {
                // Three clients, 2-3 servers: 2 chat, 1 web
                match i {
                    0 | 1 => spawn_chat_client(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                               command_chat_channel, client, chat_event_send.clone(), n_clients),
                    2 => spawn_web_browser(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                           command_web_channel, web_client, web_event_send.clone(), n_clients),
                    _ => unreachable!()
                }
            },
            _ => {
                // Default: first 2 are chat clients, rest are web browsers
                if i <  2 {
                    spawn_chat_client(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                      command_chat_channel, client, chat_event_send.clone(), n_clients);
                } else {
                    spawn_web_browser(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                      command_web_channel, web_client, web_event_send.clone(), n_clients);
                }
            }
        }
    }
}

/// Spawns a chat client instance
#[allow(dead_code)]
fn spawn_chat_client(
    id: NodeId,
    rcv_packet: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    rcv_flood: Receiver<BackGroundFlood>,
    command_chat_channel: &HashMap<NodeId, (Sender<CommandChat>, Receiver<CommandChat>)>,
    client: &mut HashMap<NodeId, Sender<CommandChat>>,
    chat_event_send: Sender<ChatClientEvent>,
    n_clients: usize
) {
    let rcv_command = command_chat_channel[&id].1.clone();
    client.insert(id, command_chat_channel[&id].0.clone());

    let mut client_instance = ChatClient::new(
        id,
        rcv_packet,
        packet_send,
        rcv_command,
        chat_event_send,
        rcv_flood,
    );

    thread::spawn(move || {
        client_instance.run();
    });
    set_node_types(NodeType::ChatClient, n_clients, id);
}

/// Spawns a web browser instance
#[allow(dead_code)]
fn spawn_web_browser(
    id: NodeId,
    rcv_packet: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    rcv_flood: Receiver<BackGroundFlood>,
    command_web_channel: &HashMap<NodeId, (Sender<ContentCommands>, Receiver<ContentCommands>)>,
    web_client: &mut HashMap<NodeId, Sender<ContentCommands>>,
    web_event_send: Sender<WebBrowserEvents>,
    n_clients: usize
) {
    let rcv_command = command_web_channel[&id].1.clone();
    web_client.insert(id, command_web_channel[&id].0.clone());

    let mut web_browser = WebBrowser::new(
        id,
        rcv_packet,
        rcv_command,
        packet_send,
        rcv_flood,
        web_event_send
    );

    thread::spawn(move || {
        web_browser.run();
    });
    set_node_types(NodeType::WebBrowser, n_clients, id);
}

/// Let the GUI know about the difference between types of Clients and Server
fn set_node_types(node_type: NodeType, n: usize, id: NodeId){
    if let Ok(mut state) = SHARED_STATE.write() {
        match node_type{
            NodeType::WebBrowser=>{
                // Update client count and add web browser to list
                state.n_clients=n;
                state.client_types.push((NodeType::WebBrowser, id));
                state.is_updated=true;
            },
            NodeType::ChatClient=>{
                // Update client count and add chat client to list
                state.n_clients=n;
                state.client_types.push((NodeType::ChatClient, id));
                state.is_updated=true;
            },
            NodeType::TextServer=>{
                // Update server count and add text server to list
                state.n_servers=n;
                state.server_types.push((NodeType::TextServer, id));
                state.is_updated=true;
            },
            NodeType::MediaServer=> {
                // Update server count and add media server to list
                state.n_servers = n;
                state.server_types.push((NodeType::MediaServer, id));
                state.is_updated = true;
            },
            NodeType::ChatServer=> {
                // Update server count and add chat server to list
                state.n_servers = n;
                state.server_types.push((NodeType::ChatServer, id));
                state.is_updated = true;
            },
            NodeType::Drone=> {
                // Drones are handled differently, no action needed
            }
        }
    }
}

/// Updates the simulation controller with all the initialized components
/// Transfers ownership of channels and maps to the controller
fn update_simulation_controller(
    simulation_controller: &mut SimulationController,
    node_event_send: Sender<DroneEvent>,
    controller_drones: HashMap<NodeId, Sender<DroneCommand>>,
    node_event_recv: Receiver<DroneEvent>,
    neighbours: HashMap<NodeId, Vec<NodeId>>,
    packet_channel: HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>,
    client: HashMap<NodeId, Sender<CommandChat>>,
    web_client: HashMap<NodeId, Sender<ContentCommands>>,
    text_servers: HashMap<NodeId, Sender<ServerCommands>>,
    media_servers: HashMap<NodeId, Sender<ServerCommands>>,
    chat_servers: HashMap<NodeId, Sender<ServerCommands>>,
    background_flooding : HashMap<NodeId, Sender<BackGroundFlood>>
) {
    // Extract only the sender channels from packet_channel
    let sender_channels: HashMap<NodeId, Sender<Packet>> = packet_channel
        .into_iter()
        .map(|(node_id, (sender, _receiver))| (node_id, sender))
        .collect();

    // Update all fields in the simulation controller
    simulation_controller.node_event_send = node_event_send.clone();
    simulation_controller.drones = controller_drones;
    simulation_controller.node_event_recv = node_event_recv;
    simulation_controller.neighbours = neighbours;
    simulation_controller.packet_channel = sender_channels;
    simulation_controller.client = client;
    simulation_controller.web_client = web_client;
    simulation_controller.text_server = text_servers;
    simulation_controller.media_server = media_servers;
    simulation_controller.chat_server = chat_servers;
    simulation_controller.background_flooding= background_flooding;
}

/// Creates a new simulation controller instance with all necessary components
/// This controller will manage the entire simulation lifecycle
fn create_simulation_controller(
    node_event_send: Sender<DroneEvent>,
    simulation_controller: ResMut<SimulationController>,
    chat_event_recv: Receiver<ChatClientEvent>,
    web_event_recv: Receiver<WebBrowserEvents>,
    server_event_recv: Receiver<ServerEvent>,
    web_active: bool,
    chat_active: bool,
    rustafarian_ids: Vec<NodeId>
) -> SimulationController {
    SimulationController {
        node_event_send,
        drones: simulation_controller.drones.clone(),
        node_event_recv: simulation_controller.node_event_recv.clone(),
        neighbours: simulation_controller.neighbours.clone(),
        packet_channel: simulation_controller.packet_channel.clone(),
        client: simulation_controller.client.clone(),
        web_client: simulation_controller.web_client.clone(),
        text_server: simulation_controller.text_server.clone(),
        media_server: simulation_controller.media_server.clone(),
        chat_server: simulation_controller.chat_server.clone(),
        chat_event: chat_event_recv,
        web_event: web_event_recv,
        server_event: server_event_recv,
        background_flooding: simulation_controller.background_flooding.clone(),
        chat_active,
        web_active,
        rustafarian_ids //SC needs to know Rustafarian_ids since they follow a different paradigm to send nacks
    }
}

/// Spawns servers for the "max" configuration
#[allow(dead_code)]
fn spawn_servers_max(
    config: &Config,
    packet_channels: &HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>,
    background_flood: &HashMap<NodeId, (Sender<BackGroundFlood>, Receiver<BackGroundFlood>)>,
    flooding: &mut HashMap<NodeId, Sender<BackGroundFlood>>,
    server_commands: &HashMap<NodeId, (Sender<ServerCommands>, Receiver<ServerCommands>)>,
    text_servers: &mut HashMap<NodeId, Sender<ServerCommands>>,
    chat_servers: &mut HashMap<NodeId, Sender<ServerCommands>>,
    server_event_send: Sender<ServerEvent>
) {
    let n_clients = config.client.len();
    let n_servers = config.server.len();

    for (i, cfg_server) in config.server.iter().cloned().enumerate() {
        // Set up channels for this server
        let rcv = packet_channels[&cfg_server.id].1.clone();
        let packet_send = cfg_server.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_, _>>();
        let rcv_flood = background_flood[&cfg_server.id].1.clone();
        flooding.insert(cfg_server.id, background_flood[&cfg_server.id].0.clone());

        let rcv_command = server_commands[&cfg_server.id].1.clone();

        // Spawn different server types based on client/server configuration
        match (n_clients, n_servers) {
            (1, 1) => {
                // Single client, single server: text server
                spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                      server_event_send.clone(), text_servers, server_commands,
                                      "assets/multimedia/path_max/max_server.txt", n_servers);
            },
            (2, 1) => {
                // Two clients, one server: chat server
                spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                      server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (2, 2) => {
                // Two clients, two servers: both chat servers
                spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                      server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (3, 1) => {
                // Three clients, one server: chat server
                spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                      server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (3, 2) => {
                // Three clients, two servers: chat and text
                match i {
                    0 => spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                               server_event_send.clone(), chat_servers, server_commands, n_servers),
                    1 => spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                               server_event_send.clone(), text_servers, server_commands,
                                               "assets/multimedia/path_max/max_server.txt", n_servers),
                    _ => unreachable!()
                }
            },
            (1, 3) => {
                // One client, three servers: all text servers with different content
                match i {
                    1 => spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                               server_event_send.clone(), text_servers, server_commands,
                                               "assets/multimedia/path_max/max_server.txt", n_servers),
                    _ => spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                               server_event_send.clone(), text_servers, server_commands,
                                               "assets/multimedia/path_max/max_server2.txt", n_servers),
                }
            },
            _ => {
                // Default configuration for 3+ servers
                if n_servers >= 3 {
                    match i {
                        0 => spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                                   server_event_send.clone(), chat_servers, server_commands, n_servers),
                        1 => spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                                   server_event_send.clone(), text_servers, server_commands,
                                                   "assets/multimedia/path_max/max_server.txt", n_servers),
                        _ => spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                                   server_event_send.clone(), text_servers, server_commands,
                                                   "assets/multimedia/path_max/max_server2.txt", n_servers),
                    }
                } else {
                    // Fallback to chat server
                    spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                          server_event_send.clone(), chat_servers, server_commands, n_servers);
                }
            }
        }
    }
}
/// Spawns a "max" ChatServer instance
#[allow(dead_code)]
fn spawn_chat_server_max(
    id: NodeId,
    rcv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    rcv_flood: Receiver<BackGroundFlood>,
    rcv_command: Receiver<ServerCommands>,
    server_event_send: Sender<ServerEvent>,
    chat_servers: &mut HashMap<NodeId, Sender<ServerCommands>>,
    server_commands: &HashMap<NodeId, (Sender<ServerCommands>, Receiver<ServerCommands>)>,
    n_servers: usize
) {
    let mut server = ChatMax::new(id, rcv, packet_send, rcv_flood, rcv_command, server_event_send);
    thread::spawn(move || {
        server.run();
    });
    chat_servers.insert(id, server_commands[&id].0.clone());
    set_node_types(NodeType::ChatServer, n_servers, id);
}
/// Spawns a "max" TextServer instance
#[allow(dead_code)]
fn spawn_text_server_max(
    id: NodeId,
    rcv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    rcv_flood: Receiver<BackGroundFlood>,
    rcv_command: Receiver<ServerCommands>,
    server_event_send: Sender<ServerEvent>,
    text_servers: &mut HashMap<NodeId, Sender<ServerCommands>>,
    server_commands: &HashMap<NodeId, (Sender<ServerCommands>, Receiver<ServerCommands>)>,
    path: &str,
    n_servers: usize
) {
    let mut server = TextMax::new(id, rcv, packet_send, rcv_command, server_event_send, path,rcv_flood);
    thread::spawn(move || {
        server.run();
    });
    text_servers.insert(id, server_commands[&id].0.clone());
    set_node_types(NodeType::TextServer, n_servers, id);
}


///Useful because it allows to use some of the checks done when updating nodes' connection at simulation startup
pub fn convert_to_config(
    config: Config,
    nodes_cat: HashMap<NodeId,NodeCategory>
)->Vec<NodeConfig>{
    let mut nodes_config=Vec::new();
    for drone in config.drone{
        nodes_config.push(NodeConfig{
            node_type: NodeType::Drone,
            id: drone.id,
            position: Vec2::default(),
            connected_node_ids: drone.connected_node_ids,
            pdr: drone.pdr,
        })
    }
    for client in config.client{
        let node_cat=nodes_cat.get(&client.id);
        if let Some(cat)=node_cat{
            let category=match cat{
                NodeCategory::Client(client)=>client,
                NodeCategory::Server(server)=>server,
            };
            nodes_config.push(NodeConfig::new(category.clone(), client.id, Vec2::default(), client.connected_drone_ids, -1.00));

        }
    }
    for server in config.server{
        let node_cat=nodes_cat.get(&server.id);
        if let Some(cat)=node_cat{
            let category=match cat{
                NodeCategory::Client(client)=>client,
                NodeCategory::Server(server)=>server,
            };
            nodes_config.push(NodeConfig::new(category.clone(), server.id, Vec2::default(), server.connected_drone_ids, -1.00));

        }

    }
    nodes_config
}
