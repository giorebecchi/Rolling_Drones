use std::collections::{HashMap, HashSet};
use std::process::id;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use bevy::pbr::add_clusters;
use crossbeam_channel::{select_biased, unbounded, Receiver, RecvError, Sender};
use wg_2024::packet;
use wg_2024::controller;
use wg_2024::config;
use serde::{Serialize, Deserialize};
use wg_2024::config::{Client};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, FloodResponse, Fragment, NodeType, Packet, PacketType, FRAGMENT_DSIZE};
use crate::clients::assembler::{Fragmentation, Serialization};
use crate::common_things::common::{ChatRequest, MessageChat, CommandChat};
use crate::servers::ChatServer::Server;

//missing flooding, handling incoming packets and handling errors
pub struct ChatClient {
    pub config: Client,
    pub receiver_msg: Receiver<Packet>,
    pub receiver_commands: Receiver<CommandChat>, //command received by the simulation control
    pub send_packets: HashMap<NodeId, Sender<Packet>>,
    pub simulation_control: HashMap<NodeId, Sender<Packet>>,
    pub servers: Vec<NodeId>,//to store id server once the flood is done
    pub visited_nodes: HashSet<(u64, NodeId)>,
    pub flood: Vec<FloodResponse> ,//to store all the flood responses found
    pub unique_flood_id: u64,
    pub session_id_packet: u64,
    pub flooding_end: Arc<Mutex<bool>>,

}
impl ChatClient {
    pub fn new(id: NodeId, receiver_msg: Receiver<Packet>, send_packets: HashMap<NodeId, Sender<Packet>>, receiver_commands: Receiver<CommandChat>, simulation_control: HashMap<NodeId, Sender<Packet>>) -> Self {
       let mut client = Self {
            config: Client { id, connected_drone_ids: Vec::new() },
            receiver_msg,
            receiver_commands,
            simulation_control,
            send_packets,
            servers: Vec::new(),
            visited_nodes: HashSet::new(),
            flood: Vec::new(),
            unique_flood_id: 0,
            session_id_packet: 0,
            flooding_end: Arc::new(Mutex::new(false)),
        };
        client.initiate_flooding();
        client
    }
    pub fn run(&mut self) {
        let mut crash = false;
        self.initiate_flooding();
        while !crash{
                select_biased! {
                recv(self.receiver_commands) -> command =>{
                    if let Ok(command) = command {
                            match command.clone(){
                                CommandChat::Crash => {
                                    crash = true;
                                },
                                _ => {}
                            }
                        self.handle_sim_command(command);
                    }
                }
                recv(self.receiver_msg) -> message =>{
                        if let Ok(message) = message {
                        self.handle_incoming(message)
                    }
                }
            }

        }

    }

    pub fn handle_sim_command(&mut self, command: CommandChat) {
        match command {
            CommandChat::ServerType(id_server) => {
                self.ask_server_type(id_server);
            }
            CommandChat::RegisterClient(id_server) => {
                self.register_client(id_server);
            }
            CommandChat::GetListClients(id_server) => {
                self.get_list_clients(id_server);
            }
            CommandChat::SendMessage(destination_id, id_server,  content) => {
                let message_to_send = MessageChat::new(content, self.config.id.clone(), destination_id);
                self.send_message(message_to_send, id_server);
            }
            CommandChat::EndChat(server_id) => {
                self.end_chat(server_id);
            }
            _ => {}
        }
    }
    pub fn handle_incoming(&mut self, message: Packet) {
        match message.pack_type {
            PacketType::MsgFragment(_) => {
                self.handle_fragments(message);
            },
            PacketType::Ack(ack) => {},
            PacketType::Nack(nack) => {},
            PacketType::FloodRequest(_) => { self.handle_flood_req(message); },
            PacketType::FloodResponse(_) => { self.handle_flood_response(message); },
        }
    }

    pub fn ask_server_type(&mut self, id_server: NodeId) {
        //if the client has to send it to a specific server (?)
        if self.servers.is_empty(){
            self.initiate_flooding();
        }
        if !self.servers.contains(&id_server){
            println!("server not found ");
            return;
        }
        let request_to_send = ChatRequest::ServerType;
        let fragmented_request = ChatRequest::fragment_message(&request_to_send);



        match self.find_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragmented_request, route.clone(), &mut self.session_id_packet);
                for packet in packets_to_send {
                    if let Some(next_hop) = route.get(1){
                        self.send_packet(next_hop, packet);
                    }else { println!("No next hop found") }
                }
                println!("Sent request to get the server type to server: {}", id_server);

            }
            Err(_) => {println!("No route found for the destination server")}
        }

        //without the request of a specific server (no parameter), if the client wants to send the request to every server found after performing the flood
        // if self.servers.is_empty(){
        //     self.initiate_flooding(); //to start flooding if it wasn't done before
        // }

        // for server in &self.servers.clone() {
        //     match self.find_route(&server){
        //         Ok(route) => {
        //             if let Some((next_hop, _)) = route.get(1){
        //                 self.send_request(&next_hop, ChatRequest::ServerType)
        //             }
        //         }
        //         Err(_) => {println!("no route was found for the server: {}", server)}
        //     }
        // }
    }
    pub fn register_client(&mut self, id_server: NodeId) {
        //to register client to the server specified in the command by simulation control.
        if self.flood.is_empty(){
            self.initiate_flooding();
        }
        if !self.servers.contains(&id_server){
            println!("The server was not found during flooding");
            return;
        }
        let request = ChatRequest::RegisterClient(self.config.id.clone());
        let fragments = ChatRequest::fragment_message(&request);

        match self.find_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(),  & mut self.session_id_packet);
                for packet in packets_to_send {
                    if let Some(next_hop) = route.get(1){
                        self.send_packet(next_hop, packet);
                    }
                }
                println!("Sent request to register this client to the server {}", id_server);

            }
            Err(_) => {println!("No route found for the destination client")}
        }

    }
    pub fn get_list_clients(&mut self, id_server: NodeId) {
        if self.servers.is_empty(){
            self.initiate_flooding();
        }
        if !self.servers.contains(&id_server){
            println!("server not found after the flooding");
            return;
        }
        let request = ChatRequest::GetListClients;
        let fragments = ChatRequest::fragment_message(&request);

        match self.find_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(), & mut self.session_id_packet);
                for packet in packets_to_send {
                    if let Some(next_hop) = route.get(1){
                        self.send_packet(next_hop, packet);
                    }
                }
                println!("sent request to get list clients of registered servers to server: {}", id_server);

            }
            Err(_) => {println!("No route found for the destination client")}
        }
    }
    pub fn send_message(&mut self, message: MessageChat, id_server: NodeId) {
        // if self.flood.is_empty() {
        //     self.initiate_flooding();
        // }
        println!("servers: {:?}", self.servers);
        println!("I've done the flooding");


        let request = ChatRequest::SendMessage(message, id_server);
        let fragments = ChatRequest::fragment_message(&request);

        match self.find_route(&id_server){
            Ok(route) => {
                println!("route: {:?}",route);
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(), & mut self.session_id_packet);
                for packet in packets_to_send {

                    println!("route: {}",packet.routing_header);
                    if let Some(next_hop) = packet.clone().routing_header.hops.get(1){
                        println!("next hop: {}",next_hop);

                        self.send_packet(next_hop, packet);
                    }
                }
            }
            Err(_) => {println!("No route found for the destination client")}
        }

    }

    pub fn end_chat(&mut self, id_server: NodeId) {
        if self.servers.is_empty(){
            println!("here");
            self.initiate_flooding();
        }

        if !self.servers.contains(&id_server){
            println!("server not found after the flooding");
            return;
        }

        let request = ChatRequest::EndChat(self.config.id.clone());
        let fragments = ChatRequest::fragment_message(&request);

        match self.find_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(),  & mut self.session_id_packet);
                for packet in packets_to_send {
                    if let Some(next_hop) = route.get(1){
                        self.send_packet(next_hop, packet);
                    }
                }
                println!("Sent request to end chat to server: {}", id_server);
            }
            Err(_) => {println!("No route found for the destination client")}
        }
    }

    //incoming messagges
    pub fn handle_fragments(& mut self, packet: Packet){
        if let PacketType::MsgFragment(fragment) = packet.pack_type{

        }
    }

    pub fn initiate_flooding(&mut self) { //this sends a flood request to its immediate neighbours
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

    pub fn handle_flood_req(& mut self, packet: Packet){
        println!("sus packet: {}",packet);
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
                    println!("first_hop: {}",first_hop);
                    if let Some(sender) = self.send_packets.get(&first_hop){
                        if let Err(e) = sender.send(flood_request.generate_response(packet.session_id)){
                            println!("Error sending the flood request: {}", e)
                        }
                    }else { println!("No sender found in the case of the client having only 1 connection") }
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

    pub fn find_route(& mut self, destination_id : &NodeId)-> Result<Vec<NodeId>, String>{
        let mut shortest_route: Option<Vec<NodeId>> = None;
        for flood_resp in &self.flood{
            if flood_resp.path_trace.contains(&(*destination_id, NodeType::Server)){
                let length = flood_resp.path_trace.len();
                if shortest_route.is_none() || length < shortest_route.as_ref().unwrap().len(){
                    shortest_route = Some(flood_resp.path_trace.iter().map(|(id,_ )| *id).collect());
                }
            }
        }
        if let Some(route) = shortest_route{
            Ok(route)
        }else { Err(String::from("route not found")) }
    }

    pub fn send_packet(& mut self, destination_id: &NodeId, mut packet: Packet){
        packet.routing_header.hop_index+=1;
        println!("send_packets: {:?}",self.send_packets);
        if let Some(sender) = self.send_packets.get(&destination_id){
            if let Err(err) = sender.send(packet.clone()){
                println!("Error sending command: {}", err); //have to send back nack
            }else{
                println!("success: {}",packet.routing_header);
            }
        }else { println!("no sender") } //have to send back nack
    }

    pub fn ack(& mut self, packet: &Packet)-> Packet{
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
}

pub fn main(){

}
