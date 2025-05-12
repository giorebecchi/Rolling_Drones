
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::{HashMap, HashSet};

use std::thread;
use crate::servers::TextServerFillo::Server as TextServerBaia;
use crate::servers::MediaServerFillo::Server as MediaServerBaia;
use std::sync::{Arc,Mutex};
use std::time::Duration;
use wg_2024::controller::{DroneCommand,DroneEvent};
use wg_2024::drone::{Drone};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Fragment, NackType, Packet, PacketType};
use bevy::prelude::{Res, ResMut};
use bagel_bomber::BagelBomber;
use fungi_drone::FungiDrone;
use skylink::SkyLinkDrone;
use LeDron_James::Drone as Le_Drone;
use lockheedrustin_drone::LockheedRustin;
use wg_2024_rust::drone::RustDrone;
use rustbusters_drone::RustBustersDrone;
use rusteze_drone::RustezeDrone;
use rustafarian_drone::RustafarianDrone;
use wg_2024::config::Config;
use wg_2024::packet::PacketType::{FloodRequest, MsgFragment, Nack};
use crate::clients::assembler::Fragmentation;
use crate::GUI::login_window::{SharedSimState, SimulationController, SHARED_LOG};
use crate::network_initializer::network_initializer::parse_config;
use crate::GUI::shared_info_plugin::SHARED_STATE;
use crate::servers::ChatServer::Server;
use crate::clients::chat_client::ChatClient;
use crate::clients::web_browser::WebBrowser;
use crate::common_things::common::{ChatClientEvent, ChatRequest, ChatResponse, ClientType, CommandChat, ContentCommands, ContentType, ServerType, WebBrowserEvents};
use crate::common_things::common::ServerType::CommunicationServer;
use crate::servers::Text_max::Server as TextMax;
use crate::servers::Chat_max::Server as ChatMax;




impl Default for SimulationController{
    fn default() -> Self {
        let (sender, receiver) = unbounded();
        let (_, chat_recv)=unbounded();
        let (_, web_recv)=unbounded();
        Self {
            node_event_send: sender,
            node_event_recv: receiver,
            drones: HashMap::new(),
            packet_channel: HashMap::new(),
            neighbours: HashMap::new(),
            client:  HashMap::new(),
            web_client: HashMap::new(),
            seen_floods: HashSet::new(),
            client_list: HashMap::new(),
            chat_event: chat_recv,
            web_event: web_recv,
            messages: HashMap::new(),
            incoming_message: HashMap::new(),
            register_success : HashMap::new()
        }
    }
}



impl SimulationController {
    fn run(&mut self) {
        let mut flood_req_hash=HashSet::new();
        let mut msg_hash=HashMap::new();
        let mut already_seen_nacks= HashMap::new();
        loop{
            select_biased! {
                recv(self.chat_event) -> event =>{
                    if let Ok(chat_event) = event {
                        match chat_event {
                            ChatClientEvent::IncomingMessage((id_client,id_server,id_from),message)=>{

                                if let Ok(mut state)=SHARED_STATE.write(){
                                    if let Some(mut messages) = state.responses.get_mut(&(id_server,(id_from,id_client))){
                                        messages.push(message.clone());
                                    }else{
                                        let mut messages=Vec::new();
                                        messages.push(message);
                                        state.responses.insert((id_server,(id_from,id_client)),messages);
                                    }
                                    state.is_updated=true;
                                }

                            },
                            ChatClientEvent::ClientList((id_client,id_server), mut registered_clients)=>{

                                if let Ok(mut state)=SHARED_STATE.write(){
                                    if let Some( current_clients) = state.client_list.get_mut(&(id_client,id_server)) {
                                        let _=std::mem::replace(current_clients, registered_clients);
                                    }else{
                                        state.client_list.insert((id_client,id_server),registered_clients);
                                    }
                                    state.is_updated=true;
                                }
                            },
                            ChatClientEvent::RegisteredSuccess((id_client, id_server), result)=>{

                                if let Ok(mut state)=SHARED_STATE.write(){
                                    match result{
                                        Ok(_)=>{
                                            state.registered_clients.insert((id_client, id_server), true);
                                        },
                                        Err(_)=>{
                                            state.registered_clients.insert((id_client, id_server), false);

                                        }

                                    }
                                    state.is_updated=true;
                                }

                            },
                            ChatClientEvent::ChatServers(client_id, chat_servers)=>{
                                if let Ok(mut state)=SHARED_STATE.write(){
                                    if let Some( current_chat_servers) = state.chat_servers.get_mut(&client_id) {
                                        let _=std::mem::replace(current_chat_servers, chat_servers);
                                    }else{
                                        state.chat_servers.insert(client_id,chat_servers);
                                    }
                                    state.is_updated=true;
                                }

                            },
                            ChatClientEvent::ClientType(client_type,node_id)=>{
                                if let Ok(mut state)=SHARED_STATE.write(){
                                    match client_type{
                                        ClientType::ChatClient=>state.chat_clients.push(node_id),
                                        ClientType::WebBrowser=>state.web_clients.push(node_id),
                                    }
                                    state.is_updated=true;
                                }
                            }
                            _=>{}
                        }
                    }
                }
                recv(self.web_event) -> event =>{
                    if let Ok(web_event) = event{
                        match web_event {
                            WebBrowserEvents::MediaServers(client, media_servers)=>{
                                if let Ok(mut state)=SHARED_STATE.write(){
                                    println!("media_servers sim: {:?} from client: {}",media_servers,client);
                                    if let Some(current_media_servers)=state.media_servers.get_mut(&client){
                                        let _=std::mem::replace(current_media_servers, media_servers);
                                    }else{
                                        state.media_servers.insert(client, media_servers);
                                    }
                                    state.is_updated=true;
                                }

                            }
                            WebBrowserEvents::TextServers(client, text_servers)=>{
                                if let Ok(mut state)=SHARED_STATE.write(){
                                    if let Some(current_media_servers)=state.text_servers.get_mut(&client){
                                        let _=std::mem::replace(current_media_servers, text_servers);
                                    }else{
                                        state.text_servers.insert(client, text_servers);
                                    }
                                    state.is_updated=true;
                                }

                            }
                            WebBrowserEvents::ListFiles(client, media_paths)=>{
                                if let Ok(mut state)=SHARED_STATE.write(){
                                    if let Some(current_paths)=state.client_medias.get_mut(&client){
                                        let _=std::mem::replace(current_paths, media_paths);
                                    }else{
                                        state.client_medias.insert(client,media_paths);
                                    }
                                    state.is_updated=true;
                                }
                            }
                            WebBrowserEvents::MediaPosition(client, target_media_server)=>{
                                if let Ok(mut state)=SHARED_STATE.write(){
                                    if let Some(current_media_server)=state.target_media_server.get_mut(&client){
                                        let _=std::mem::replace(current_media_server, target_media_server);
                                    }else{
                                        state.target_media_server.insert(client, target_media_server);
                                    }
                                    state.is_updated=true;
                                }
                            }
                            WebBrowserEvents::SavedMedia(client, actual_media)=>{
                                println!("saved_media: {}",actual_media);
                                if let Ok(mut state)=SHARED_STATE.write(){
                                    if let Some(current_path)=state.actual_media_path.get_mut(&client){
                                        let _=std::mem::replace(current_path, actual_media);
                                    }else{
                                        state.actual_media_path.insert(client,actual_media);
                                    }
                                    state.is_updated=true;
                                }
                            }
                            WebBrowserEvents::SavedTextFile(client, actual_file)=>{
                                if let Ok(mut state)=SHARED_STATE.write(){
                                    if let Some(current_path)=state.actual_file_path.get_mut(&client){
                                        let _=std::mem::replace(current_path, actual_file);
                                    }else{
                                        state.actual_file_path.insert(client,actual_file);
                                    }
                                    state.is_updated=true;
                                }
                            }
                            WebBrowserEvents::PacketInfo(client, packet_info, session_id)=>{
                                msg_hash.insert((client, session_id), packet_info.clone());
                                match packet_info{
                                    ContentType::TextServerList(size)=>{
                                        if let Ok(mut state)= SHARED_LOG.write(){
                                            state.msg_log.push_str(&format!("Web browser: {} asked for list of Text Servers\n the message was made of {} fragments\n", client, size));
                                            state.is_updated=true;
                                        }
                                    }
                                    ContentType::MediaServerList(size)=>{
                                         if let Ok(mut state)= SHARED_LOG.write(){
                                            state.msg_log.push_str(&format!("Web browser: {} asked for list of Media Servers\n the message was made of {} fragments\n", client, size));
                                            state.is_updated=true;
                                        }
                                    }
                                    ContentType::FileList(size)=>{
                                        if let Ok(mut state)= SHARED_LOG.write(){
                                            state.msg_log.push_str(&format!("Web browser: {} asked for File List\n the message was made of {} fragments\n", client, size));
                                            state.is_updated=true;
                                        }
                                    }
                                    ContentType::MediaPosition(size)=>{
                                        if let Ok(mut state)= SHARED_LOG.write(){
                                            state.msg_log.push_str(&format!("Web browser: {} asked for Media Position\n the message was made of {} fragments\n", client, size));
                                            state.is_updated=true;
                                        }
                                    }
                                    ContentType::SavedText(size)=>{
                                        if let Ok(mut state)= SHARED_LOG.write(){
                                            state.msg_log.push_str(&format!("Web browser: {} asked for a Text File\n the message was made of {} fragments\n", client, size));
                                            state.is_updated=true;
                                        }
                                    }
                                    ContentType::SavedMedia(size)=>{
                                        if let Ok(mut state)= SHARED_LOG.write(){
                                            state.msg_log.push_str(&format!("Web browser: {} asked for a Media\n the message was made of {} fragments\n", client, size));
                                            state.is_updated=true;
                                        }
                                    }
                                }
                            }

                            _=>{}

                        }
                    }
                },
            recv(self.node_event_recv) -> command =>{
                if let Ok(drone_event) = command {
                     match drone_event{
                        DroneEvent::PacketSent(ref packet) => {
                                match packet.pack_type.clone(){
                                    FloodRequest(flood_req)=>{
                                        if flood_req_hash.insert((flood_req.initiator_id,flood_req.flood_id)){
                                            if let Ok(mut state)=SHARED_LOG.write(){
                                                state.flooding_log.push_str(&format!("{:?} with id {} has initiated a flood with id {}\n",flood_req.path_trace[0].1,flood_req.initiator_id,flood_req.flood_id));
                                                state.is_updated=true;
                                            }


                                        }

                                    },
                                    Nack(nack)=>{
                                        if None == already_seen_nacks.insert(packet.routing_header.hops.clone(), packet.session_id){
                                            let route_len=packet.routing_header.len();
                                            let naccked_drone=packet.routing_header.hops[0];
                                            let initiator=packet.routing_header.hops[route_len-1];
                                            let mut pack_info=None;
                                            if let Some(packet_info)=msg_hash.get(&(initiator, packet.session_id)){
                                                pack_info=Some(packet_info);
                                            }
                                            match nack.nack_type{
                                                NackType::ErrorInRouting(neighbor)=>{

                                                    if let Ok(mut state)=SHARED_LOG.write(){
                                                        match pack_info{
                                                            Some(pack)=>{
                                                                state.nack_log.push_str(&format!("1. An Error In Routing was received by Drone {}\n 2. Nack is about Packet {} to {}\n",naccked_drone,pack, neighbor ));
                                                            },
                                                            None=>{
                                                                state.nack_log.push_str(&format!("1. An Error In Routing was received by Drone {}\n 2. Nack is about Packet from Server to {}\n", naccked_drone, neighbor));
                                                            }
                                                        }
                                                        state.is_updated=true;

                                                    }

                                                }
                                                NackType::DestinationIsDrone=>{
                                                    if let Ok(mut state)=SHARED_LOG.write(){
                                                        match pack_info{
                                                            Some(pack)=>{
                                                                state.nack_log.push_str(&format!("1. A Destination Is Drone was received by Drone {}\n 2. Nack is about Packet {}\n",naccked_drone,pack));
                                                            }
                                                            None=>{
                                                                state.nack_log.push_str(&format!("1. A Destination Is Drone was received by Drone {}\n 2. Nack is about a Packet from Server\n",naccked_drone));
                                                            }
                                                        }
                                                        state.is_updated=true;
                                                    }
                                                }
                                                NackType::Dropped=>{
                                                    if let Ok(mut state)=SHARED_LOG.write(){
                                                        match pack_info{
                                                            Some(pack)=>{
                                                                state.nack_log.push_str(&format!("1. A Dropped was received by Drone {}\n 2. Nack is about Packet {}\n",naccked_drone,pack));
                                                            }
                                                            None=>{
                                                                state.nack_log.push_str(&format!("1. A Dropped was received by Drone {}\n 2. Nack is about a Packet from Server\n",naccked_drone));
                                                            }
                                                        }
                                                        state.is_updated=true;
                                                    }
                                                }
                                                NackType::UnexpectedRecipient(drone)=>{
                                                    if let Ok(mut state)=SHARED_LOG.write(){
                                                        match pack_info{
                                                            Some(pack)=>{
                                                                state.nack_log.push_str(&format!("1. An Unexpected Recipient was received by Drone {}\n 2. Nack is about Packet {}\n",drone,pack));
                                                            }
                                                            None=>{
                                                                state.nack_log.push_str(&format!("1. An Unexpected Recipient was received by Drone {}\n 2. Nack is about a Packet from Server\n",drone));
                                                            }
                                                        }
                                                        state.is_updated=true;
                                                    }
                                                }
                                            }
                                        }

                                    },


                                    _=>{}
                                }
                        }
                        DroneEvent::PacketDropped(ref packet) => {
                            println!("Simulation control: drone dropped packet");
                        }
                        DroneEvent::ControllerShortcut(ref controller_shortcut) => {

                            println!("Simulation control: packet {:?} sent to destination",controller_shortcut.pack_type);
                        }
                    }

                    self.handle_event(drone_event.clone());
                }

            }
        }
        }

    }

    fn handle_event(&mut self, command: DroneEvent) {


    }
    fn print_packet(&mut self, packet: Packet) {
        // print!("  source id: {:#?}  |", packet.routing_header.hops[0]);
        // print!("  destination id: {:#?}  |", packet.routing_header.hops[packet.routing_header.hops.len() - 1]);
        // print!("  path: [");
        // for i in 0..packet.routing_header.hops.len()-1 {
        //     print!("{}, ", packet.routing_header.hops[i]);
        // }
        // println!("{}]", packet.routing_header.hops[packet.routing_header.hops.len() - 1]);
    }
    fn send_to_destination(&mut self, mut packet: Packet) {
        let addr = packet.routing_header.hops[packet.routing_header.hops.len() - 1];
        self.print_packet(packet.clone());
        packet.routing_header.hop_index = packet.routing_header.hops.len()-1;

        if let Some(sender) = self.packet_channel.get(&addr) {
            sender.send(packet).unwrap();
        }

    }




    pub fn crash_all(&mut self) {
        for (_, sender) in self.drones.iter() {
            sender.send(DroneCommand::Crash).unwrap();
            println!("Sent Crash");
        }
    }
    pub fn crash(&mut self, id: NodeId) {
        let nghb = self.neighbours.get(&id).unwrap();
        for neighbour in nghb.iter(){
            if let Some(sender) = self.drones.get(&neighbour) {
                sender.send(DroneCommand::RemoveSender(id)).unwrap();
            }
        }

        // Send the Crash command to the target drone
        if let Some(drone_sender) = self.drones.get(&id) {
            if let Err(err) = drone_sender.send(DroneCommand::Crash) {
                println!("Failed to send Crash command to drone {}: {:?}", id, err);
                return;
            }
           // println!("Sent Crash command to drone {}", id);
        } else {
            println!("No drone with ID {:?}", id);
            return;
        }

    }

    pub fn pdr(&mut self, id : NodeId, pdr: f32) {
        for (idd, sender) in self.drones.iter() {
            if idd == &id {
                println!("pdr of drone {idd} changed to {pdr}");
                sender.send(DroneCommand::SetPacketDropRate(pdr)).unwrap()
            }
        }
    }
    pub fn add_sender(&mut self, dst_id: NodeId, nghb_id: NodeId) {
       let sender=self.packet_channel.get(&dst_id).unwrap().clone();
        if let Some(drone_sender) = self.drones.get(&nghb_id) {
            if let Err(err) = drone_sender.send(DroneCommand::AddSender(dst_id, sender)) {
                println!(
                    "Failed to send AddSender command to drone {}: {:?}",
                    nghb_id, err
                );
            }
        } else {
            println!("No drone found with ID {}", nghb_id);
        }
    }

    pub fn remove_sender(&mut self, drone_id: NodeId, nghb_id: NodeId) {
        if let Some(drone_sender) = self.drones.get(&drone_id) {
            if let Err(err) = drone_sender.send(DroneCommand::RemoveSender(nghb_id)) {
                println!(
                    "Failed to send RemoveSender command to drone {} for neighbor {}: {:?}",
                    drone_id, nghb_id, err
                );
            } else {
                println!("Sent RemoveSender command to drone {} for neighbor {}", drone_id, nghb_id);
            }
        } else {
            println!("No drone found with ID {}", drone_id);
        }
    }
    fn ack(&mut self, mut packet: Packet) {
        let next_hop=packet.routing_header.hops[packet.routing_header.hop_index +1];
        if let Some(sender) = self.packet_channel.get(&next_hop) {
            packet.routing_header.hop_index+=1;
            sender.send(packet).unwrap();
        }else{
            println!("No sender found for hop {}", next_hop);
        }
    }
    fn msg_fragment(&mut self, mut packet: Packet){
        println!("You're tying to send a message fragment");
        let next_hop=packet.routing_header.hops[packet.routing_header.hop_index+1];
        if let Some(sender) = self.packet_channel.get(&next_hop) {
            packet.routing_header.hop_index+=1;
            println!("Starting from hop_index: {} \n ",packet.routing_header.hop_index);
            sender.send(packet).unwrap();
        }
    }
    pub fn initiate_flood(&mut self, packet: Packet){
        println!("Initiating flood");
        if let FloodRequest(_)=packet.clone().pack_type {
            for node_neighbours in packet.clone().routing_header.hops{
                if let Some(sender) = self.packet_channel.get(&node_neighbours) {
                    sender.send(packet.clone()).unwrap();
                }else{
                    println!("No sender found for neighbours {:?}", node_neighbours);
                }
            }
        }else{
            println!("Unexpected error occurred, message wasn't a flood request");
        }
    }
    pub fn spawn_new_drone(&mut self, links: Vec<NodeId>, id: NodeId){
        let node_event_send_clone=self.node_event_send.clone();
        let (sender_command, recv_command)=unbounded();
        self.drones.insert(id, sender_command);
        let mut packet_channels = HashMap::new();
        packet_channels.insert(id, unbounded());
        for id in links.clone(){
            packet_channels.insert(id, unbounded());
        }
        let packet_recv = packet_channels[&id].1.clone();
        let packet_send = links.iter().map(|nid|(*nid, packet_channels[nid].0.clone())).collect::<HashMap<_,_>>();
        thread::spawn(move || {
            let mut drone = create_drone(
                id,
                node_event_send_clone,
                recv_command,
                packet_recv,
                packet_send,
                0.0
            );
            if let Some(mut drone) = drone {
                drone.run();
            }
        });
    }
    pub fn send_message(&mut self, message: String, client_id: NodeId, destination_client: NodeId, chat_server: NodeId){
        self.client.get(&client_id).unwrap().send(CommandChat::SendMessage(destination_client, chat_server, message)).unwrap()
    }
    pub fn register_client(&mut self, client_id: NodeId, server_id: NodeId){
        self.client.get(&client_id).unwrap().send(CommandChat::RegisterClient(server_id)).unwrap();
    }
    pub fn get_client_list(&mut self, client_id: NodeId, server_id: NodeId){
        self.client.get(&client_id).unwrap().send(CommandChat::GetListClients(server_id)).unwrap();
    }
    pub fn get_chat_servers(&self){
        for (_,sender) in self.client.iter(){
            sender.send(CommandChat::SearchChatServers).unwrap();
        }
    }
    pub fn get_web_servers(&self){
        for (_,sender) in self.web_client.iter(){
            println!("sent command searchserver to client");
            sender.send(ContentCommands::SearchTypeServers).unwrap()
        }
    }
    pub fn get_media_list(&self, web_browser: NodeId, text_server: NodeId){
        if let Some(sender)=self.web_client.get(&web_browser){
            sender.send(ContentCommands::GetTextList(text_server)).unwrap();
        }
    }
    pub fn get_text_file(&self, web_browser: NodeId, text_server: NodeId, text_file: String){
        if let Some(sender)=self.web_client.get(&web_browser){
            sender.send(ContentCommands::GetText(text_server, text_file)).unwrap();
        }
    }
    pub fn get_media_position(&self, web_browser: NodeId, text_server:NodeId, media_path: String){
        if let Some(sender)=self.web_client.get(&web_browser){
            sender.send(ContentCommands::GetMediaPosition(text_server, media_path)).unwrap();
        }
    }
    pub fn get_media_from(&self, web_browser: NodeId, media_server: NodeId, media_path: String){
        println!("function get_media_from was called");
        if let Some(sender)= self.web_client.get(&web_browser){
            sender.send(ContentCommands::GetMedia(media_server, media_path)).unwrap();
        }
    }

}

pub fn start_simulation(
    mut simulation_controller: ResMut<SimulationController>
) {
    let file_path = "assets/configurations/double_chain.toml";
    let config = parse_config(file_path);

    let (packet_channels, command_chat_channel, command_web_channel) =
        setup_communication_channels(&config);

    let (chat_event_send, chat_event_recv) = unbounded();
    let (web_event_send, web_event_recv) = unbounded();

    let neighbours = create_neighbours_map(&config);

    let mut controller_drones = HashMap::new();
    let mut packet_drones = HashMap::new();
    let node_event_send = simulation_controller.node_event_send.clone();
    let node_event_recv = simulation_controller.node_event_recv.clone();
    let mut client = simulation_controller.client.clone();
    let mut web_client = simulation_controller.web_client.clone();

    spawn_drones(
        &config,
        &mut controller_drones,
        &mut packet_drones,
        &packet_channels,
        node_event_send.clone()
    );
    #[cfg(feature = "max")]
    {
        spawn_servers_max(&config, &packet_channels);
    }
    #[cfg(not(feature = "max"))]
    {
        spawn_servers_baia(&config, &packet_channels);
    }

    spawn_clients(
        &config,
        &packet_channels,
        &command_chat_channel,
        &command_web_channel,
        &mut client,
        &mut web_client,
        chat_event_send.clone(),
        web_event_send.clone()
    );

    update_simulation_controller(
        &mut simulation_controller,
        node_event_send.clone(),
        controller_drones,
        node_event_recv,
        neighbours,
        packet_drones,
        client,
        web_client
    );

    let mut controller = create_simulation_controller(
        node_event_send,
        simulation_controller,
        chat_event_recv,
        web_event_recv
    );

    thread::spawn(move || {
        controller.run();
    });
}

fn setup_communication_channels(config: &Config) -> (
    HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>,
    HashMap<NodeId, (Sender<CommandChat>, Receiver<CommandChat>)>,
    HashMap<NodeId, (Sender<ContentCommands>, Receiver<ContentCommands>)>
) {
    let mut packet_channels = HashMap::new();
    let mut command_chat_channel = HashMap::new();
    let mut command_web_channel = HashMap::new();

    for node_id in config.drone.iter().map(|d| d.id)
        .chain(config.client.iter().map(|c| c.id))
        .chain(config.server.iter().map(|s| s.id)) {
        packet_channels.insert(node_id, unbounded());
    }

    for client in &config.client {
        command_chat_channel.insert(client.id, unbounded());
        command_web_channel.insert(client.id, unbounded());
    }

    (packet_channels, command_chat_channel, command_web_channel)
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
        0 => Some(Box::new(BagelBomber::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        1 => Some(Box::new(FungiDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        2 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        3 => Some(Box::new(SkyLinkDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        4 => Some(Box::new(Le_Drone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        5 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        6 => Some(Box::new(RustDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        7 => Some(Box::new(RustBustersDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        8 => Some(Box::new(RustezeDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        9 => Some(Box::new(RustafarianDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        _ => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
    }
}


fn spawn_servers_baia(
    config: &Config,
    packet_channels: &HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>
) {
    for (i, cfg_server) in config.server.iter().cloned().enumerate() {
        let rcv = packet_channels[&cfg_server.id].1.clone();
        let packet_send = cfg_server.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_,_>>();

        match i {
            0 => {
                let mut server_baia = Server::new(cfg_server.id, rcv, packet_send);
                thread::spawn(move || {
                    server_baia.run();
                });
            },
            1 => {
                let mut text_server_baia = TextServerBaia::new(
                    cfg_server.id,
                    rcv,
                    packet_send,
                    "assets/multimedia/paths/text_server1.txt"
                );
                thread::spawn(move || {
                    text_server_baia.run();
                });
            },
            2 => {
                let mut media_server_baia = MediaServerBaia::new(
                    cfg_server.id,
                    rcv,
                    packet_send,
                    "assets/multimedia/paths/media_server1.txt"
                );
                thread::spawn(move || {
                    media_server_baia.run();
                });
            },
            _ => {
                let mut media_server_baia = MediaServerBaia::new(
                    cfg_server.id,
                    rcv,
                    packet_send,
                    "assets/multimedia/paths/media_serverr2.txt"
                );
                thread::spawn(move || {
                    media_server_baia.run();
                });
            }
        }
    }
}
fn spawn_clients(
    config: &Config,
    packet_channels: &HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>,
    command_chat_channel: &HashMap<NodeId, (Sender<CommandChat>, Receiver<CommandChat>)>,
    command_web_channel: &HashMap<NodeId, (Sender<ContentCommands>, Receiver<ContentCommands>)>,
    client: &mut HashMap<NodeId, Sender<CommandChat>>,
    web_client: &mut HashMap<NodeId, Sender<ContentCommands>>,
    chat_event_send: Sender<ChatClientEvent>,
    web_event_send: Sender<WebBrowserEvents>
) {
    for (i, cfg_client) in config.client.iter().cloned().enumerate() {
        let packet_send: HashMap<NodeId, Sender<Packet>> = cfg_client.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect();
        let rcv_packet = packet_channels[&cfg_client.id].1.clone();

        if i < 2 {
            let rcv_command = command_chat_channel[&cfg_client.id].1.clone();
            client.insert(cfg_client.id, command_chat_channel[&cfg_client.id].0.clone());

            let mut client_instance = ChatClient::new(
                cfg_client.id,
                rcv_packet,
                packet_send.clone(),
                rcv_command,
                chat_event_send.clone()
            );

            thread::spawn(move || {
                client_instance.run();
            });

            if let Ok(mut state) = SHARED_STATE.write() {
                state.n_clients = config.client.len();
                state.client_types.push((ClientType::ChatClient, cfg_client.id));
                state.is_updated = true;
            }
        } else {
            let rcv_command = command_web_channel[&cfg_client.id].1.clone();
            web_client.insert(cfg_client.id, command_web_channel[&cfg_client.id].0.clone());

            let mut web_browser = WebBrowser::new(
                cfg_client.id,
                rcv_packet,
                rcv_command,
                packet_send.clone(),
                web_event_send.clone()
            );
            thread::spawn(move || {
                web_browser.run();
            });

            if let Ok(mut state) = SHARED_STATE.write() {
                state.n_clients = config.client.len();
                state.client_types.push((ClientType::WebBrowser, cfg_client.id));
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
    packet_channel: HashMap<NodeId, Sender<Packet>>,
    client: HashMap<NodeId, Sender<CommandChat>>,
    web_client: HashMap<NodeId, Sender<ContentCommands>>
) {
    simulation_controller.node_event_send = node_event_send.clone();
    simulation_controller.drones = controller_drones;
    simulation_controller.node_event_recv = node_event_recv;
    simulation_controller.neighbours = neighbours;
    simulation_controller.packet_channel = packet_channel;
    simulation_controller.client = client;
    simulation_controller.web_client = web_client;
}

fn create_simulation_controller(
    node_event_send: Sender<DroneEvent>,
    simulation_controller: ResMut<SimulationController>,
    chat_event_recv: Receiver<ChatClientEvent>,
    web_event_recv: Receiver<WebBrowserEvents>
) -> SimulationController {
    SimulationController {
        node_event_send,
        drones: simulation_controller.drones.clone(),
        node_event_recv: simulation_controller.node_event_recv.clone(),
        neighbours: simulation_controller.neighbours.clone(),
        packet_channel: simulation_controller.packet_channel.clone(),
        client: simulation_controller.client.clone(),
        web_client: simulation_controller.web_client.clone(),
        seen_floods: HashSet::new(),
        client_list: HashMap::new(),
        chat_event: chat_event_recv,
        web_event: web_event_recv,
        incoming_message: HashMap::new(),
        messages: HashMap::new(),
        register_success: HashMap::new()
    }
}

fn spawn_servers_max(
    config: &Config,
    packet_channels: &HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)>
) {
    for (i, cfg_server) in config.server.iter().cloned().enumerate() {
        let rcv = packet_channels[&cfg_server.id].1.clone();
        let packet_send = cfg_server.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_,_>>();

        match i {
            0 => {
                let mut server_max = ChatMax::new(cfg_server.id, rcv, packet_send);
                thread::spawn(move || {
                    server_max.run();
                });
            },
            _ => {
                let mut text_server_max = TextMax::new(
                    cfg_server.id,
                    rcv,
                    packet_send,
                    "assets/multimedia/paths/text_server1.txt"
                );
                thread::spawn(move || {
                    text_server_max.run();
                });
            }

        }
    }
}