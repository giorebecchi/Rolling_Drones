use std::collections::{HashMap, HashSet};
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
use crate::common_things::common::{BackGroundFlood, ChatClientEvent, CommandChat, ContentCommands, ServerCommands, ServerEvent, WebBrowserEvents};
use crate::gui::login_window::{NodeConfig, NodeType};
use crate::simulation_control::simulation_control::SimulationController;
use crate::gui::shared_info_plugin::{NodeCategory, ERROR_VERIFY, SHARED_STATE};
use crate::network_initializer::connection_validity::{validate_drone_pdr, validate_duplex_connections, would_break_connectivity};
use crate::servers::ChatServer::Server;
use crate::servers::TextServerFillo::Server as TextServerBaia;
use crate::servers::MediaServerFillo::Server as MediaServerBaia;
use crate::servers::Chat_max::Server as ChatMax;
use crate::servers::Text_max::Server as TextMax;

pub fn parse_config() -> Config {
    let file_str = if cfg!(feature = "web") {
        fs::read_to_string("assets/configurations/web_topology.toml").unwrap()
    } else if cfg!(feature = "full") {
        fs::read_to_string("assets/configurations/full_topology.toml").unwrap()
    } else {
        fs::read_to_string("assets/configurations/chat_topology.toml").unwrap()
    };
    toml::from_str(&file_str).unwrap()
}
pub fn start_simulation(
    mut simulation_controller: ResMut<SimulationController>
) {
    let config = parse_config();

    let (packet_channels, command_chat_channel,
        command_web_channel, background_flooding, server_commands) =
        setup_communication_channels(&config);
    let (chat_event_send, chat_event_recv) = unbounded();
    let (web_event_send, web_event_recv) = unbounded();
    let (server_event_send, server_event_recv) = unbounded();

    let neighbours = create_neighbours_map(&config);

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

    spawn_drones(
        &config,
        &mut controller_drones,
        &mut packet_drones,
        &packet_channels,
        node_event_send.clone()
    );
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
    let n_servers=text_servers.len()+chat_servers.len()+media_servers.len();

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
    let web_active=!web_client.is_empty();
    let chat_active=!client.is_empty();

    let mut controller = create_simulation_controller(
        node_event_send,
        simulation_controller,
        chat_event_recv,
        web_event_recv,
        server_event_recv,
        web_active,
        chat_active
    );

    thread::spawn(move || {
        controller.run();
    });
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
    let isolated_node=would_break_connectivity(&convert_to_config(config.clone(), nodes.clone()));
    let wrong_pdr=validate_drone_pdr(&convert_to_config(config.clone(), nodes.clone()));
    let connection_error=validate_duplex_connections(&convert_to_config(config, nodes.clone()));
    println!("wrong_pdr: {:?}", wrong_pdr);
    if let Ok(mut state) = ERROR_VERIFY.write(){
        state.connection_error=(false,connection_error.clone());
        state.wrong_pdr=(false, wrong_pdr.clone());
        state.isolated_node=(false, isolated_node.clone());
        state.is_updated=true;
    }

    if let Ok(mut state)=SHARED_STATE.write(){
        state.nodes=nodes;
        state.is_updated=true;
    }
}

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

    for node_id in config.drone.iter().map(|d| d.id)
        .chain(config.client.iter().map(|c| c.id))
        .chain(config.server.iter().map(|s| s.id)) {
        packet_channels.insert(node_id, unbounded());
    }

    for client in &config.client {
        command_chat_channel.insert(client.id, unbounded());
        command_web_channel.insert(client.id, unbounded());
        background_flood.insert(client.id, unbounded());
    }
    for server in &config.server{
        server_commands.insert(server.id, unbounded());
        background_flood.insert(server.id, unbounded());
    }

    (packet_channels, command_chat_channel, command_web_channel, background_flood, server_commands)
}

fn create_neighbours_map(config: &Config) -> HashMap<NodeId, Vec<NodeId>> {
    let mut neighbours = HashMap::new();
    for drone in &config.drone {
        neighbours.insert(drone.id, drone.connected_node_ids.clone());
    }
    neighbours
}

fn spawn_drones(
    config: &Config,
    controller_drones: &mut HashMap<NodeId, Sender<DroneCommand>>,
    packet_drones: &mut HashMap<NodeId, Sender<Packet>>,
    packet_channels: &HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>,
    node_event_send: Sender<DroneEvent>
) {
    for (i,cfg_drone) in config.drone.iter().cloned().enumerate() {
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(cfg_drone.id, controller_drone_send);
        packet_drones.insert(cfg_drone.id, packet_channels[&cfg_drone.id].0.clone());

        let node_event_send_clone = node_event_send.clone();
        let packet_recv = packet_channels[&cfg_drone.id].1.clone();
        let packet_send = cfg_drone.connected_node_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_, _>>();

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
}
fn create_drone(
    id: NodeId,
    node_event_send: Sender<DroneEvent>,
    controller_drone_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<u8, Sender<Packet>>,
    pdr: f32,
    i: usize
) -> Option<Box<dyn Drone>> {
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

        let rcv = packet_channels[&cfg_server.id].1.clone();
        let packet_send = cfg_server.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_, _>>();
        let rcv_flood = background_flood[&cfg_server.id].1.clone();
        flooding.insert(cfg_server.id, background_flood[&cfg_server.id].0.clone());
        let rcv_command = server_commands[&cfg_server.id].1.clone();

        match (n_clients, n_servers) {
            (1, 1) => {
                spawn_text_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), text_servers, server_commands,
                                  "assets/multimedia/paths/text_server1.txt", n_servers);
            },
            (2, 1) => {
                spawn_chat_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (2, 2) => {
                spawn_chat_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (3, 1) => {
                spawn_chat_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (3, 2) => {
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
                    spawn_chat_server(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                      server_event_send.clone(), chat_servers, server_commands, n_servers);
                }
            }
        }
    }
}
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

        let packet_send: HashMap<NodeId, Sender<Packet>> = cfg_client.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect();
        let rcv_packet = packet_channels[&cfg_client.id].1.clone();
        let rcv_flood = background_flood[&cfg_client.id].1.clone();
        flooding.insert(cfg_client.id, background_flood[&cfg_client.id].0.clone());

        match (n_clients, n_servers) {
            (1, _) => {
                spawn_web_browser(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                  command_web_channel, web_client, web_event_send.clone(), n_clients);
            },
            (2, _) => {
                spawn_chat_client(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                  command_chat_channel, client, chat_event_send.clone(), n_clients);
            },
            (3, 1) => {
                spawn_chat_client(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                  command_chat_channel, client, chat_event_send.clone(), n_clients);
            },
            (3, 2) | (3,3) => {
                match i {
                    0 | 1 => spawn_chat_client(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                               command_chat_channel, client, chat_event_send.clone(), n_clients),
                    2 => spawn_web_browser(cfg_client.id, rcv_packet, packet_send, rcv_flood,
                                           command_web_channel, web_client, web_event_send.clone(), n_clients),
                    _ => unreachable!()
                }
            },
            _ => {
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
fn set_node_types(node_type: NodeType, n: usize, id: NodeId){
    if let Ok(mut state) = SHARED_STATE.write() {
        match node_type{
            NodeType::WebBrowser=>{
                state.n_clients=n;
                state.client_types.push((NodeType::WebBrowser, id));
                state.is_updated=true;
            },
            NodeType::ChatClient=>{
                state.n_clients=n;
                state.client_types.push((NodeType::ChatClient, id));
                state.is_updated=true;
            },
            NodeType::TextServer=>{
                state.n_servers=n;
                state.server_types.push((NodeType::TextServer, id));
                state.is_updated=true;


            },
            NodeType::MediaServer=> {
                state.n_servers = n;
                state.server_types.push((NodeType::MediaServer, id));
                state.is_updated = true;
            },
            NodeType::ChatServer=> {
                state.n_servers = n;
                state.server_types.push((NodeType::ChatServer, id));
                state.is_updated = true;
            },
            NodeType::Drone=> {
            }
        }
    }
}

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
    let sender_channels: HashMap<NodeId, Sender<Packet>> = packet_channel
        .into_iter()
        .map(|(node_id, (sender, _receiver))| (node_id, sender))
        .collect();
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

fn create_simulation_controller(
    node_event_send: Sender<DroneEvent>,
    simulation_controller: ResMut<SimulationController>,
    chat_event_recv: Receiver<ChatClientEvent>,
    web_event_recv: Receiver<WebBrowserEvents>,
    server_event_recv: Receiver<ServerEvent>,
    web_active: bool,
    chat_active: bool,
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
        web_active
    }
}

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

        let rcv = packet_channels[&cfg_server.id].1.clone();
        let packet_send = cfg_server.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_,_>>();
        let rcv_flood= background_flood[&cfg_server.id].1.clone();
        flooding.insert(cfg_server.id, background_flood[&cfg_server.id].0.clone());

        let rcv_command = server_commands[&cfg_server.id].1.clone();

        match (n_clients, n_servers) {
            (1, 1) => {
                spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), text_servers, server_commands,
                                  "assets/multimedia/path_max/max_server.txt", n_servers);
            },
            (2, 1) => {
                spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (2, 2) => {
                spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (3, 1) => {
                spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                  server_event_send.clone(), chat_servers, server_commands, n_servers);
            },
            (3, 2) => {
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
                match i {
                    1=> spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                              server_event_send.clone(), text_servers, server_commands,
                                              "assets/multimedia/path_max/max_server.txt", n_servers),
                    _ => spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                           server_event_send.clone(), text_servers, server_commands,
                                               "assets/multimedia/path_max/max_server2.txt", n_servers),

                }
            },
            _ => {
                if n_servers >= 3 {
                    match i  {
                        0 => spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                               server_event_send.clone(), chat_servers, server_commands, n_servers),
                        1=> spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                               server_event_send.clone(), text_servers, server_commands,
                                                  "assets/multimedia/path_max/max_server.txt", n_servers),
                        _=> spawn_text_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                                  server_event_send.clone(), text_servers, server_commands,
                                                  "assets/multimedia/path_max/max_server2.txt", n_servers),

                    }
                } else {
                    spawn_chat_server_max(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command,
                                      server_event_send.clone(), chat_servers, server_commands, n_servers);
                }
            }
        }
    }
}
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
