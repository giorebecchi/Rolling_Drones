use std::collections::{HashMap, HashSet};
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
use crate::common_things::common::{ChatRequest, CommandText, ContentCommands, MediaId, WebBrowserCommands};

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
    pub problematic_nodes: Vec<NodeId>,
    pub topology: DiGraphMap<NodeId, u32>
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
            topology: DiGraphMap::new()
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
            ContentCommands::GetMediaPosition(id_server, id_media) => {
                self.get_position(id_server, id_media)
            },
            ContentCommands::GetMedia(id_media_server, id_media) => {
                self.get_media(id_media_server, id_media)
            },
            ContentCommands::GetText(id_server, text_id) => {
                self.get_text(id_server, text_id);
            }
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
        let source = self.config.id.clone();
        let result = dijkstra(&self.topology, source.clone(), Some(destination_id.clone()), |_| 1);

        if let Some(_) = result.get(destination_id) {
            let mut path = vec![destination_id.clone()];
            let mut current = destination_id.clone();

            while current != source {
                // Look for a predecessor node 'prev' with:
                // result[prev] + weight == result[current]
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



}


