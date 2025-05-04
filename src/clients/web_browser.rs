use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use base64::Engine;
use bevy::ui::Node;
use bevy_egui::egui::debug_text::print;
use crossbeam_channel::{select_biased, Receiver, Sender};
use petgraph::algo::dijkstra;
use petgraph::Direction;
use petgraph::graphmap::DiGraphMap;
use petgraph::visit::EdgeRef;
use wg_2024::packet;
use wg_2024::controller;
use serde::{Serialize, Deserialize};
use wg_2024::config::Client;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, FloodResponse, Fragment, NodeType, Packet, PacketType};
use crate::clients::assembler::Fragmentation;
use crate::common_things::common::{ChatRequest, ClientType, ContentCommands, FileMetaData, MediaId, MediaServer, ServerType, TextServer, WebBrowserCommands, WebBrowserEvents};
use crate::common_things::common::WebBrowserEvents::TypeClient;
use base64::engine::general_purpose::STANDARD as BASE64;

pub struct WebBrowser {
    pub config: Client,
    pub client_type: ClientType,
    pub receiver_msg: Receiver<Packet>,
    pub receiver_commands: Receiver<ContentCommands>, //command received by the simulation control
    pub send_packets: HashMap<NodeId, Sender<Packet>>,
    pub servers: Vec<NodeId>,//to store id server once the flood is done
    pub visited_nodes: HashSet<(u64, NodeId)>,
    pub flood: Vec<FloodResponse> ,//to store all the flood responses found
    pub unique_flood_id: u64,
    pub session_id_packet: u64,
    pub incoming_fragments: HashMap<(u64, NodeId ), HashMap<u64, Fragment>>,
    pub fragments_sent: HashMap<u64, Fragment>, //used for sending the correct fragment if was lost in the process
    pub problematic_nodes: Vec<NodeId>,
    pub topology: DiGraphMap<NodeId, u32>,
    pub send_event: Sender<WebBrowserEvents>,
    pub media_servers: Vec<NodeId>,
    pub text_servers: Vec<NodeId>,
    pub waiting_response: usize

}

impl WebBrowser {
    pub fn new(
        id: NodeId, receiver_msg: Receiver<Packet>,
        receiver_commands: Receiver<ContentCommands>,
        send_packets: HashMap<NodeId, Sender<Packet>>,
        send_event: Sender<WebBrowserEvents>
    ) -> Self {
        Self{
            config: Client{id, connected_drone_ids:Vec::new()},
            client_type: ClientType::WebBrowser,
            receiver_msg,
            receiver_commands,
            send_packets,
            servers: Vec::new(),
            visited_nodes: HashSet::new(),
            flood: Vec::new(),
            unique_flood_id: 0,
            session_id_packet: 0,
            incoming_fragments: HashMap::new(),
            fragments_sent: HashMap::new(),
            problematic_nodes: Vec::new(),
            topology: DiGraphMap::new(),
            send_event,
            media_servers: Vec::new(),
            text_servers: Vec::new(),
            waiting_response: 0
        }
    }
    pub fn run(& mut self) {
        self.flooding();
        self.send_type_sim();

        loop{
            select_biased! {
                recv(self.receiver_commands) -> command =>{
                    if let Ok(command) = command {
                        self.build_topology();
                        self.handle_commands(command);
                    }
                }
                recv(self.receiver_msg) -> message =>{
                    if let Ok(message) = message {
                        self.build_topology();
                        self.handle_messages(message)
                    }
                }
            }
        }
    }

    fn handle_commands(&mut self, command: ContentCommands) {
        match command {
            ContentCommands::GetServerType(id_server) => {
                self.ask_type(id_server)
            },
            ContentCommands::GetTextList(id_server) => {
                self.get_list(id_server)
            },
            ContentCommands::GetMediaPosition(id_server, id_media) => {
                self.get_position(id_server, id_media)
            },
            ContentCommands::GetMedia(id_media_server, id_media) => {
                println!("received get media");
                self.get_media(id_media_server, id_media)
            },
            ContentCommands::GetText(id_server, text_id) => {
                self.get_text(id_server, text_id);
            }
            ContentCommands::SearchTypeServers => {
                self.search_type_servers();
            }

            _ => {}
        }
    }

    fn handle_messages(& mut self, message: Packet){
        match message.pack_type{
            PacketType::MsgFragment(_) => {self.handle_fragments(message)},
            PacketType::Ack(_) => {self.handle_acks(message)},
            PacketType::Nack(_) => {self.handle_nacks(message)},
            PacketType::FloodResponse(_) => {self.handle_flood_response(message)},
            PacketType::FloodRequest(_) => {self.handle_flood_request(message)},
        }
    }

    pub fn send_type_sim(& mut self){
        if let Err(_) = self.send_event.send(TypeClient(self.client_type.clone(), self.config.id.clone())){
            println!("Error sending client type to simulation control")
        }
    }

    pub fn search_type_servers(& mut self) {
        self.waiting_response = self.servers.len();
        for server in self.servers.clone() {
            self.ask_type(server);
        }
    }

    pub fn ask_type(& mut self, id_server: NodeId) {
        if !self.servers.contains(&id_server) {
            println!("server was not found");
            return;
        }

        let request = WebBrowserCommands::GetServerType;
        self.fragments_sent = WebBrowserCommands::fragment_message(&request);

        match self.find_path(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&self.fragments_sent, route.clone(), &mut self.session_id_packet);
                for packet in packets_to_send {
                    if let Some(next_hop) = route.get(1) {
                        self.send_messages(next_hop, packet);
                    } else { println!("No next hop found") }
                }
                println!("Sent request to get the server type to server: {}", id_server);
            }
            Err(_) => { println!("No route found for the destination server") }
        }
    }

    pub fn get_list(& mut self, id_server: NodeId) {
        if !self.servers.contains(&id_server) {
            println!("server was not found");
            return;
        }

        let request = WebBrowserCommands::GetList;
        self.fragments_sent = WebBrowserCommands::fragment_message(&request);

        match self.find_path(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&self.fragments_sent, route.clone(), &mut self.session_id_packet);
                for packet in packets_to_send {
                    if let Some(next_hop) = route.get(1) {
                        self.send_messages(next_hop, packet);
                    } else { println!("No next hop found") }
                }
                println!("Sent request to get the server type to server: {}", id_server);
            }
            Err(_) => { println!("No route found for the destination server") }
        }
    }

    pub fn get_position (& mut self, id_server: NodeId, media_id: MediaId){
        if !self.servers.contains(&id_server) {
            println!("server was not found");
            return;
        }

        let request = WebBrowserCommands::GetPosition(media_id);
        self.fragments_sent = WebBrowserCommands::fragment_message(&request);

        match self.find_path(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&self.fragments_sent, route.clone(), &mut self.session_id_packet);
                for packet in packets_to_send {
                    if let Some(next_hop) = route.get(1) {
                        self.send_messages(next_hop, packet);
                    } else { println!("No next hop found") }
                }
                println!("Sent request to get the server type to server: {}", id_server);
            }
            Err(_) => { println!("No route found for the destination server") }
        }
    }

    pub fn get_media(& mut self, id_media_server: NodeId, media_id: MediaId) {
        if !self.servers.contains(&id_media_server) {
            println!("server was not found");
            return;
        }
        let request = WebBrowserCommands::GetMedia(media_id);
        self.fragments_sent = WebBrowserCommands::fragment_message(&request);

        match self.find_path(&id_media_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&self.fragments_sent, route.clone(), &mut self.session_id_packet);
                for packet in packets_to_send {
                    if let Some(next_hop) = route.get(1) {
                        self.send_messages(next_hop, packet);
                    } else { println!("No next hop found") }
                }
                println!("Sent request to get the server type to server: {}", id_media_server);
            }
            Err(_) => { println!("No route found for the destination server") }
        }
    }

    pub fn get_text (& mut self, id_server: NodeId, text_id: String) {
        if !self.servers.contains(&id_server) {
            println!("server was not found");
            return;
        }
        let request = WebBrowserCommands::GetText(text_id);
        self.fragments_sent = WebBrowserCommands::fragment_message(&request);

        match self.find_path(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&self.fragments_sent, route.clone(), &mut self.session_id_packet);
                for packet in packets_to_send {
                    if let Some(next_hop) = route.get(1) {
                        self.send_messages(next_hop, packet);
                    } else { println!("No next hop found") }
                }
                println!("Sent request to get the server type to server: {}", id_server);
            }
            Err(_) => { println!("No route found for the destination server") }
        }

    }

    // incoming messages
    pub fn handle_fragments(& mut self, packet: Packet){
        let src_id = packet.routing_header.hops.first().unwrap();
        let check = (packet.session_id, *src_id);

        let ack = self.create_ack(&packet);
        let prev = packet.routing_header.hops[packet.routing_header.hop_index-1];
        self.send_messages(&prev, ack);

        if let PacketType::MsgFragment(fragment) = packet.pack_type{
            if !self.incoming_fragments.contains_key(&check){
                self.incoming_fragments.insert(check, HashMap::new());
            }
            self.incoming_fragments
                .get_mut(&check)
                .unwrap()
                .insert(fragment.fragment_index, fragment.clone());

            if let Some(fragments) = self.incoming_fragments.get(&check){
                if fragments.len() as u64 == fragment.total_n_fragments{
                    if let Ok(message) = TextServer::reassemble_msg(fragments) {
                        match message {
                            TextServer::ServerTypeText(server_type) => {
                                println!("server found is of type: {:?}", server_type);

                                if server_type == ServerType::TextServer && !self.text_servers.contains(&src_id) {
                                    self.text_servers.push(src_id.clone());
                                }

                                if let Err(_) = self.send_event.send(WebBrowserEvents::TextServers(self.config.id.clone(), self.text_servers.clone())) {
                                    println!("failed to send list of text servers to simulation control")
                                } else {
                                    println!("client {} sent text_servers {:?}", self.config.id, self.text_servers);
                                }
                            }

                            TextServer::SendFileList(list) => {
                                println!("list of files available: {:?}", list);

                                if let Err(_) = self.send_event.send(WebBrowserEvents::ListFiles(self.config.id.clone(), list.clone())) {
                                    println!("failed to send list of files to simulation control")
                                }
                            }

                            TextServer::PositionMedia(media_server_id) => {
                                println!("the wanted media is located at: {}", media_server_id);
                                if let Err(_) = self.send_event.send(WebBrowserEvents::MediaPosition(self.config.id.clone(), media_server_id.clone())) {
                                    println!("failed to send media position to simulation control")
                                }
                            }

                            TextServer::Text(text) => {
                                println!("the text file was received by the web browser");
                                let path_folder = "src/multimedia/SC".to_string();
                                match self.save_file(&path_folder, text) {
                                    Ok(path) => {
                                        if let Err(_) = self.send_event.send(WebBrowserEvents::SavedTextFile(self.config.id.clone(), path.clone())) {
                                            println!("failed to send path to text file to simulation control")
                                        }
                                    }
                                    Err(str) => { println!("{}", str) }
                                }
                            }

                            _ => {}
                        }
                    }

                    if let Ok(message) = MediaServer::reassemble_msg(fragments){
                        match message{
                            MediaServer::ServerTypeMedia(server_type) => {
                                println!("server found is of type: {:?}", server_type);

                                if server_type == ServerType::MediaServer && !self.media_servers.contains(&src_id){
                                    self.media_servers.push(src_id.clone());
                                }

                                if let Err(_) = self.send_event.send(WebBrowserEvents::MediaServers(self.config.id.clone(), self.media_servers.clone())){
                                    println!("failed to send list of text servers to simulation control")
                                }

                            }

                            MediaServer::SendMedia(media) => {
                                println!("the media was received by the web browser");
                                let path_folder = "src/multimedia/SC".to_string();
                                match self.save_file(&path_folder, media){
                                    Ok(path) => {
                                        if let Err(_) = self.send_event.send(WebBrowserEvents::SavedMedia(self.config.id.clone(), path.clone())){
                                            println!("failed to send path to media to simulation control")
                                        }
                                    }
                                    Err(str) => {println!("{}", str)}
                                }

                            }

                            _ => {}
                        }
                    }

                }
            }
        }
    }

    pub fn handle_nacks(& mut self, packet: Packet){}

    pub fn handle_acks(& mut self, packet: Packet){
        if let PacketType::Ack(ack) = packet.pack_type{
            self.fragments_sent.retain(|index, _| *index != ack.fragment_index); //this filters the hashmap, removing the ones with that index
        }
    }


    pub fn flooding(& mut self){
        let mut flood_id = self.unique_flood_id;
        self.unique_flood_id += 1;

        let mut flood_request = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            0,
            FloodRequest {
                flood_id,
                initiator_id: self.config.id.clone(),
                path_trace: vec![(self.config.id, NodeType::Client)],
            }
        );

        for (_, sender) in &self.send_packets { //directly using the sender in the loop
            if let Err(_) = sender.send(flood_request.clone()) {
                println!("Error sending the flood request")
            }
        }
    }

    pub fn handle_flood_response(& mut self, packet: Packet){
        if let PacketType::FloodResponse(flood_response) = packet.clone().pack_type {
            if !flood_response.path_trace.is_empty(){
                for (node_id, node_type) in &flood_response.path_trace{
                    if *node_type == NodeType::Server && !self.servers.contains(&node_id){
                        self.servers.push(*node_id); //no duplicates
                    }
                }
                self.flood.push(flood_response); //storing all the flood responses to then access the path traces and find the quickest one
            }

        }
    }

    pub fn handle_flood_request(& mut self, packet: Packet){
        if let PacketType::FloodRequest(mut flood_request) = packet.clone().pack_type {

            //check if the pair (flood_id, initiator id) has already been received -> self.visited_nodes
            if self.visited_nodes.contains(&(flood_request.flood_id, flood_request.initiator_id)){
                flood_request.path_trace.push((self.config.id.clone(), NodeType::Client));
                // if let Some(next_hop) = flood_request.path_trace.iter().rev().nth(1){
                //     println!("next hop: {}", next_hop.0);
                //     self.send_messages(&next_hop.0, flood_request.generate_response(packet.session_id) );
                // }else { println!("No next hop found") }
                self.send_flooding_packet( flood_request.generate_response(packet.session_id) );
            }else {
                flood_request.path_trace.push((self.config.id.clone(), NodeType::Client));
                self.visited_nodes.insert((flood_request.flood_id, flood_request.initiator_id));

                if self.send_packets.len() == 1{
                    // if let Some(next_hop) = flood_request.path_trace.iter().rev().nth(1){
                    //     println!("next hop: {}", next_hop.0);
                    //     self.send_messages(&next_hop.0, flood_request.generate_response(packet.session_id) );
                    // }else { println!("No next hop found") }
                    self.send_flooding_packet( flood_request.generate_response(packet.session_id) );
                }else {
                    let new_packet = Packet::new_flood_request(packet.routing_header, packet.session_id, flood_request.clone()); //create the packet of the flood request that needs to be forwarded
                    for (neighbour, sender) in &self.send_packets {
                        if let Some(sender_flood_req) = flood_request.path_trace.iter().rev().nth(1) {
                            if *neighbour != sender_flood_req.0 {
                                sender.send(new_packet.clone()).unwrap()
                            }

                        }
                    }
                }
            }
        }
    }
    pub fn send_flooding_packet(& mut self, mut packet: Packet){
        if packet.routing_header.hop_index < packet.routing_header.hops.len() -1 {
            packet.routing_header.hop_index += 1;
            let next_hop = packet.routing_header.hops[packet.routing_header.hop_index];
            if let Some(sender) = self.send_packets.get(&next_hop) {
                sender.send(packet.clone()).unwrap_or_default();
            }
        } else {
            println!("destination reached!!");
            return;
        }
    }

    pub fn build_topology(& mut self){
        self.topology.clear();

        for resp in &self.flood{
            let path = &resp.path_trace;
            for pair in path.windows(2) {
                let (src, _) = pair[0];
                let (dst, _) = pair[1];
                self.topology.add_edge(src.clone(), dst.clone(), 1); // use 1 as weight (hop), could me modified for the nack
            }
        }
    }

    pub fn find_path(& mut self, destination_id : &NodeId)-> Result<Vec<NodeId>, String>{
        let source = self.config.id.clone();
        let result = dijkstra(&self.topology, source.clone(), Some(destination_id.clone()), |_| 1);

        if let Some(_) = result.get(destination_id) {
            let mut path = vec![destination_id.clone()];
            let mut current = destination_id.clone();

            while current != source {
                let mut found = false;
                for edge in self.topology.edges_directed(current.clone(), Direction::Incoming) {
                    let prev = edge.source();
                    let weight = edge.weight();

                    if let (Some(&prev_cost), Some(&curr_cost)) = (result.get(&prev), result.get(&current)) {
                        if prev_cost + weight == curr_cost {
                            path.push(prev.clone());
                            current = prev.clone();
                            found = true;
                            break;
                        }
                    }
                }

                if !found {
                    return Err("Failed to reconstruct path".into());
                }
            }

            path.reverse();
            Ok(path)
        } else {
            Err("No route found".into())
        }
    }

    pub fn send_messages(& mut self, destination_id: &NodeId, mut packet: Packet){
        packet.routing_header.hop_index+=1;
        if let Some(sender) = self.send_packets.get(&destination_id){
            if let Err(err) = sender.send(packet.clone()){
                println!("Error sending command: {}", err); //have to send back nack
            }
        }else { println!("no sender") } //have to send back nack
    }

    pub fn create_ack(& mut self, packet: &Packet)-> Packet{
        let mut fragment_index = 0;
        if let PacketType::MsgFragment(fragment) = packet.clone().pack_type {
            fragment_index = fragment.fragment_index;
        }
        let mut hops = Vec::new();
        for h in packet.routing_header.hops.iter().rev(){
            hops.push(*h);
        }
        let source_routing_header = SourceRoutingHeader::new(hops, 0);

        let ack_packet = Packet::new_ack(source_routing_header, packet.session_id, fragment_index );
        ack_packet
    }

    fn save_file(& self, path_folder: &str, fmd: FileMetaData)-> Result<String, String>{
        let full_path = format!("{}/{}.{}", path_folder, fmd.title, fmd.extension);

        let decode = match BASE64.decode(fmd.content){
            Ok(decode) => decode,
            Err(_) => return Err("Failed to decode the file".to_string()),
        };

        fs::write(&full_path, decode)
            .map_err(|e| format!("Failed to save the file: {}", e))?;

        println!("saved the file to {}", full_path);
        Ok(full_path)
    }


}


