use std::collections::{HashMap, HashSet};
use std::{fs, thread};
use bevy::prelude::{ResMut};
use crossbeam_channel::{unbounded, Receiver, Sender};
use lockheedrustin_drone::LockheedRustin;
use wg_2024::config::{Config};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;
use crate::clients::chat_client::ChatClient;
use crate::clients::web_browser::WebBrowser;
use crate::common_things::common::{BackGroundFlood, ChatClientEvent, CommandChat, ContentCommands, ServerCommands, ServerEvent, WebBrowserEvents};
use crate::gui::login_window::SimulationController;
use crate::gui::shared_info_plugin::SHARED_STATE;
use crate::servers::ChatServer::Server;
use crate::simulation_control::simulation_control::MyNodeType;
use crate::servers::TextServerFillo::Server as TextServerBaia;
use crate::servers::MediaServerFillo::Server as MediaServerBaia;
use crate::servers::Chat_max::Server as ChatMax;
use crate::servers::Text_max::Server as TextMax;

pub fn parse_config(file: &str) -> Config {
    let file_str = fs::read_to_string(file).unwrap();
    toml::from_str(&file_str).unwrap()
}
pub fn start_simulation(
    mut simulation_controller: ResMut<SimulationController>
) {
    let file_path = "assets/configurations/double_chain.toml";
    let config = parse_config(file_path);
    check_full_duplex_connections(&config);

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
        web_event_send.clone()
    );

    update_simulation_controller(
        &mut simulation_controller,
        node_event_send.clone(),
        controller_drones,
        node_event_recv,
        neighbours,
        packet_channels,
        client,
        web_client,
        text_servers,
        media_servers,
        chat_servers,
        background_flood
    );

    let mut controller = create_simulation_controller(
        node_event_send,
        simulation_controller,
        chat_event_recv,
        web_event_recv,
        server_event_recv
    );

    thread::spawn(move || {
        controller.run();
    });
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
    for cfg_drone in config.drone.iter().cloned() {
        if cfg_drone.pdr>1.0{
            if let Ok(mut state)= SHARED_STATE.write(){
                state.wrong_pdr.insert(cfg_drone.id, true);
                state.is_updated=true;
            }
        }
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(cfg_drone.id, controller_drone_send);
        packet_drones.insert(cfg_drone.id, packet_channels[&cfg_drone.id].0.clone());

        let node_event_send_clone = node_event_send.clone();
        let packet_recv = packet_channels[&cfg_drone.id].1.clone();
        let packet_send = cfg_drone.connected_node_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_, _>>();

        thread::spawn(move || {
            let mut drone = create_drone(
                cfg_drone.id,
                node_event_send_clone,
                controller_drone_recv,
                packet_recv,
                packet_send,
                cfg_drone.pdr,
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
) -> Option<Box<dyn Drone>> {
    match id % 10 {
        0 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        1 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        2 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        3 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        4 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        5 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        6 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        7 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        8 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        9 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
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
    for (i, cfg_server) in config.server.iter().cloned().enumerate() {
        if cfg_server.connected_drone_ids.is_empty(){
            topology_error(cfg_server.id, cfg_server.connected_drone_ids.clone());
        }
        let rcv = packet_channels[&cfg_server.id].1.clone();
        let packet_send = cfg_server.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_,_>>();
        let rcv_flood= background_flood[&cfg_server.id].1.clone();
        flooding.insert(cfg_server.id, background_flood[&cfg_server.id].0.clone());

        let rcv_command = server_commands[&cfg_server.id].1.clone();

        match i {
            0 => {
                let mut server_baia = Server::new(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command.clone(), server_event_send.clone());
                thread::spawn(move || {
                    server_baia.run();
                });
                chat_servers.insert(cfg_server.id, server_commands[&cfg_server.id].0.clone());
                set_node_types(MyNodeType::ChatServer, config.server.len(), cfg_server.id);
            },
            1 => {
                let mut text_server_baia = TextServerBaia::new(
                    cfg_server.id,
                    rcv,
                    packet_send,
                    rcv_flood,
                    rcv_command.clone(),
                    server_event_send.clone(),
                    "assets/multimedia/paths/text_server1.txt"
                );
                thread::spawn(move || {
                    text_server_baia.run();
                });
                text_servers.insert(cfg_server.id, server_commands[&cfg_server.id].0.clone());
                set_node_types(MyNodeType::TextServer, config.server.len(), cfg_server.id);

            },
            2 => {
                let mut media_server_baia = MediaServerBaia::new(
                    cfg_server.id,
                    rcv,
                    packet_send,
                    rcv_flood,
                    rcv_command.clone(),
                    server_event_send.clone(),
                    "assets/multimedia/paths/media_server1.txt"
                );
                thread::spawn(move || {
                    media_server_baia.run();
                });
                media_servers.insert(cfg_server.id, server_commands[&cfg_server.id].0.clone());
                set_node_types(MyNodeType::MediaServer, config.server.len(), cfg_server.id);
            },
            3 => {
                let mut media_server_baia = MediaServerBaia::new(
                    cfg_server.id,
                    rcv,
                    packet_send,
                    rcv_flood,
                    rcv_command.clone(),
                    server_event_send.clone(),
                    "assets/multimedia/paths/media_serverr2.txt"
                );
                thread::spawn(move || {
                    media_server_baia.run();
                });
                media_servers.insert(cfg_server.id, server_commands[&cfg_server.id].0.clone());
                set_node_types(MyNodeType::MediaServer, config.server.len(), cfg_server.id);
            },
            _=>{}
        }
    }
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
    web_event_send: Sender<WebBrowserEvents>
) {
    for (i, cfg_client) in config.client.iter().cloned().enumerate() {
        if cfg_client.connected_drone_ids.is_empty(){
            topology_error(cfg_client.id, cfg_client.connected_drone_ids.clone());
        }
        let packet_send: HashMap<NodeId, Sender<Packet>> = cfg_client.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect();
        let rcv_packet = packet_channels[&cfg_client.id].1.clone();
        let rcv_flood = background_flood[&cfg_client.id].1.clone();
        flooding.insert(cfg_client.id, background_flood[&cfg_client.id].0.clone());


        if i < 2 {
            let rcv_command = command_chat_channel[&cfg_client.id].1.clone();
            client.insert(cfg_client.id, command_chat_channel[&cfg_client.id].0.clone());

            let mut client_instance = ChatClient::new(
                cfg_client.id,
                rcv_packet,
                packet_send.clone(),
                rcv_command,
                chat_event_send.clone(),
                rcv_flood,
            );

            thread::spawn(move || {
                client_instance.run();
            });
            set_node_types(MyNodeType::ChatClient, config.client.len(), cfg_client.id);
        } else {
            let rcv_command = command_web_channel[&cfg_client.id].1.clone();
            web_client.insert(cfg_client.id, command_web_channel[&cfg_client.id].0.clone());

            let mut web_browser = WebBrowser::new(
                cfg_client.id,
                rcv_packet,
                rcv_command,
                packet_send.clone(),
                rcv_flood,
                web_event_send.clone()
            );
            thread::spawn(move || {
                web_browser.run();
            });
            set_node_types(MyNodeType::WebBrowser, config.client.len(), cfg_client.id);
        }
    }
}
fn set_node_types(node_type: MyNodeType, n: usize, id: NodeId){
    if let Ok(mut state) = SHARED_STATE.write() {
        match node_type{
            MyNodeType::WebBrowser=>{
                state.n_clients=n;
                state.client_types.push((MyNodeType::WebBrowser, id));
                state.is_updated=true;
            },
            MyNodeType::ChatClient=>{
                state.n_clients=n;
                state.client_types.push((MyNodeType::ChatClient, id));
                state.is_updated=true;
            },
            MyNodeType::TextServer=>{
                state.n_servers=n;
                state.server_types.push((MyNodeType::TextServer, id));
                state.is_updated=true;


            },
            MyNodeType::MediaServer=> {
                state.n_servers = n;
                state.server_types.push((MyNodeType::MediaServer, id));
                state.is_updated = true;
            },
            MyNodeType::ChatServer=> {
                state.n_servers = n;
                state.server_types.push((MyNodeType::ChatServer, id));
                state.is_updated = true;
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
    server_event_recv: Receiver<ServerEvent>
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
        seen_floods: HashSet::new(),
        client_list: HashMap::new(),
        chat_event: chat_event_recv,
        web_event: web_event_recv,
        server_event: server_event_recv,
        incoming_message: HashMap::new(),
        messages: HashMap::new(),
        register_success: HashMap::new(),
        background_flooding: simulation_controller.background_flooding.clone()
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
    for (i, cfg_server) in config.server.iter().cloned().enumerate() {
        if cfg_server.connected_drone_ids.is_empty(){
            topology_error(cfg_server.id, cfg_server.connected_drone_ids.clone());
        }
        if cfg_server.connected_drone_ids.is_empty(){
            topology_error(cfg_server.id, cfg_server.connected_drone_ids.clone());
        }
        let rcv = packet_channels[&cfg_server.id].1.clone();
        let packet_send = cfg_server.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_,_>>();
        let rcv_flood= background_flood[&cfg_server.id].1.clone();
        flooding.insert(cfg_server.id, background_flood[&cfg_server.id].0.clone());

        let rcv_command = server_commands[&cfg_server.id].1.clone();

        match i {
            0 => {
                let mut server_max = ChatMax::new(cfg_server.id, rcv, packet_send, rcv_flood, rcv_command.clone(), server_event_send.clone());
                thread::spawn(move || {
                    server_max.run();
                });
                chat_servers.insert(cfg_server.id, server_commands[&cfg_server.id].0.clone());
                set_node_types(MyNodeType::ChatServer, config.server.len(), cfg_server.id);
            },
            _ => {
                let mut text_server_max = TextMax::new(
                    cfg_server.id,
                    rcv,
                    packet_send,
                    rcv_command.clone(),
                    server_event_send.clone(),
                    "assets/multimedia/path_max/max_server.txt",
                    rcv_flood
                );
                thread::spawn(move || {
                    text_server_max.run();
                });
                text_servers.insert(cfg_server.id, server_commands[&cfg_server.id].0.clone());
                set_node_types(MyNodeType::TextServer, config.server.len(), cfg_server.id);
            }

        }
    }
}
fn topology_error(id: NodeId, connected_ids: Vec<NodeId>){
    if let Ok(mut state) = SHARED_STATE.write(){
        state.wrong_connections.insert(id, connected_ids);
        state.is_updated=true;
    }
}
fn check_full_duplex_connections(config: &Config){
    let mut incomplete_connections = Vec::new();

    let mut connection_map: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

    for client in &config.client {
        let connected_set: HashSet<NodeId> = client.connected_drone_ids.iter().cloned().collect();
        connection_map.insert(client.id, connected_set);
    }
    for server in &config.server{
        let connected_set: HashSet<NodeId> = server.connected_drone_ids.iter().cloned().collect();
        connection_map.insert(server.id, connected_set);
    }
    for drone in &config.drone{
        let connected_set: HashSet<NodeId> = drone.connected_node_ids.iter().cloned().collect();
        connection_map.insert(drone.id, connected_set);
    }

    for client in &config.client {
        for &neighbor_id in &client.connected_drone_ids {
            if let Some(neighbor_connections) = connection_map.get(&neighbor_id) {
                if !neighbor_connections.contains(&client.id) {
                    incomplete_connections.push((client.id, neighbor_id));
                }
            } else {
                incomplete_connections.push((client.id, neighbor_id));
            }
        }
    }
    for server in &config.server {
        for &neighbor_id in &server.connected_drone_ids {
            if let Some(neighbor_connections) = connection_map.get(&neighbor_id) {
                if !neighbor_connections.contains(&server.id) {
                    incomplete_connections.push((server.id, neighbor_id));
                }
            } else {
                incomplete_connections.push((server.id, neighbor_id));
            }
        }
    }
    for drone in &config.drone {
        for &neighbor_id in &drone.connected_node_ids {
            if let Some(neighbor_connections) = connection_map.get(&neighbor_id) {
                if !neighbor_connections.contains(&drone.id) {
                    incomplete_connections.push((drone.id, neighbor_id));
                }
            } else {
                incomplete_connections.push((drone.id, neighbor_id));
            }
        }
    }
    if let Ok(mut state) = SHARED_STATE.write(){
        state.incomplete_connections=incomplete_connections;
        state.is_updated=true;
    }

}
