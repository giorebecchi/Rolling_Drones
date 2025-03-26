use std::collections::{HashMap, HashSet};
use crossbeam_channel::{select_biased, Receiver, Sender};
use wg_2024::packet;
use wg_2024::controller;
use serde::{Serialize, Deserialize};
use wg_2024::config::Client;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, FloodResponse, Fragment, NodeType, Packet, PacketType};
use crate::common_things::common::{CommandText, ContentCommands, MediaId};

pub struct WebBrowser {
    pub config: Client,
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
    pub problematic_nodes: Vec<NodeId>
}

impl WebBrowser {
    pub fn new(id: NodeId, receiver_msg: Receiver<Packet>, receiver_commands: Receiver<ContentCommands>, send_packets: HashMap<NodeId, Sender<Packet>>) -> WebBrowser {
        Self{
            config: Client{id, connected_drone_ids:Vec::new()},
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
        }
    }
    pub fn run(& mut self) {
        self.flooding();
        loop{
            select_biased! {
                recv(self.receiver_commands) -> command =>{
                    if let Ok(command) = command {
                        self.handle_commands(command);
                    }
                }
                recv(self.receiver_msg) -> message =>{
                    if let Ok(message) = message {
                        self.handle_messages(message)
                    }
                }
            }
        }
    }

    fn handle_commands(&mut self, command: ContentCommands) {
        match command {
            ContentCommands::GetServerType(id_server) => {self.ask_type(id_server)},
            ContentCommands::GetTextList(id_server) => {self.get_list(id_server)},
            ContentCommands::GetMediaPosition(id_server, id_media) => {self.get_position(id_server, id_media)},
            ContentCommands::GetMedia(id_media_server, id_media) => {self.get_media(id_media_server, id_media)},
            _ => {}
        }
    }

    fn handle_messages(& mut self, message: Packet){
        match message.pack_type{
            PacketType::MsgFragment(_) => {},
            PacketType::Ack(_) => {},
            PacketType::Nack(_) => {},
            PacketType::FloodResponse(_) => {self.handle_flood_response(message)},
            PacketType::FloodRequest(_) => {self.handle_flood_request(message)},
        }
    }

    pub fn ask_type(& mut self, id_server: NodeId) {
        if !self.servers.contains(&id_server) {
            println!("server was not found");
            return;
        }

    }

    pub fn get_list(& mut self, id_server: NodeId) {}

    pub fn get_position (& mut self, id_server: NodeId, media_id: MediaId){}

    pub fn get_media(& mut self, id_media_server: NodeId, media_id: MediaId) {}



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
            if self.visited_nodes.contains(&(flood_request.flood_id, flood_request.initiator_id)) { //case if client has already received the request
                if let Some(first_hop) = Some(flood_request.path_trace[1].0) {
                    if let Some(sender) = self.send_packets.get(&first_hop) {
                        if let Err(e) = sender.send(flood_request.generate_response(packet.session_id)){
                            println!("Error sending the flood flood request: {}", e)
                        }
                    }else { println!("No sender found for first hop {}", first_hop) }
                }else { println!("No next hop found") }
                return;
            }

            self.visited_nodes.insert((flood_request.flood_id, flood_request.initiator_id)); //mark as visited


            if self.send_packets.len() == 1{ //check if the client has only one node connected to it
                if let Some(first_hop) = Some(flood_request.path_trace[1].0 ) {
                    if let Some(sender) = self.send_packets.get(&first_hop){
                        if let Err(e) = sender.send(flood_request.generate_response(packet.session_id)){
                            println!("Error sending the flood request: {}", e)
                        }
                    }else { println!("This is not an error message: \n'No sender found in the case of the client having only 1 connection'") }
                }else { println!("No next hop found") }
                return;
            }

            if let Some(_) = flood_request.path_trace.get(1){ //normal case when the client forwards it to the every neighbor
                println!("forwarding to all direct neighbours");
                for (neighbor, sender) in &self.send_packets{
                    if *neighbor != flood_request.path_trace[1].0 { //forward to everyone that's not the one sending the request
                        let mut forward_packet = packet.clone();
                        forward_packet.pack_type = PacketType::FloodRequest(flood_request.clone());
                        if let Err(e) = sender.send(forward_packet){
                            println!("error forwarding the flood request to every neighbor: {}", e)
                        }
                    }
                }
            }

        }else { println!("Flood request not found") }
    }

    pub fn find_path(& mut self, destination_id : &NodeId)-> Result<Vec<NodeId>, String>{
        let mut shortest_route: Option<Vec<NodeId>> = None;
        for flood_resp in &self.flood{
            if flood_resp.path_trace.contains(&(*destination_id, NodeType::Server))&& !flood_resp.path_trace.iter().any(|(id, _)| self.problematic_nodes.contains(id)){
                let length = flood_resp.path_trace.len();
                if shortest_route.is_none() || length < shortest_route.as_ref().unwrap().len(){
                    shortest_route = Some(flood_resp.path_trace.iter().map(|(id,_ )| *id).collect());
                }
            }
        }
        // for flood_resp in &self.flood {
        //     if flood_resp.path_trace.contains(&(*destination_id, NodeType::Server)) {
        //         let route: Vec<NodeId> = flood_resp.path_trace.iter().map(|(id, _)| *id).collect();
        //
        //         if route.iter().any(|id| self.problematic_nodes.contains(id)) {
        //             continue; // Skip this route and look for another one
        //         }
        //
        //         let length = route.len();
        //         if shortest_route.as_ref().map_or(true, |r| length < r.len()) {
        //             shortest_route = Some(route);
        //         }
        //     }
        // }
        if let Some(best_route) = shortest_route{
            Ok(best_route)
        }else { Err(String::from("route not found")) }
    }

    pub fn send_messages(& mut self, destination_id: &NodeId, mut packet: Packet){
        packet.routing_header.hop_index+=1;
        if let Some(sender) = self.send_packets.get(&destination_id){
            if let Err(err) = sender.send(packet.clone()){
                println!("Error sending command: {}", err); //have to send back nack
            }
        }else { println!("no sender") } //have to send back nack
    }



}


