use std::collections::{HashMap, HashSet};
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
    pub receiver_commands: Receiver<CommandChat>,
    pub send_req: HashMap<NodeId, Sender<ChatRequest>>,
    pub send_packets: HashMap<NodeId, Sender<Packet>>,
    pub servers: Vec<NodeId>,//to store id server once the flood is done
    pub visited_nodes: HashSet<(u64, NodeId)>,
    pub flood: Vec<FloodResponse> ,//to store all the flood responses found
    pub unique_flood_id: u64
    // pub simulation_control: HashMap<NodeId, Sender<Packet>>
}
impl ChatClient {
    pub fn new(id: NodeId, receiver_msg: Receiver<Packet>, send_packets: HashMap<NodeId, Sender<Packet>>, receiver_commands: Receiver<CommandChat>, send_req: HashMap<NodeId, Sender<ChatRequest>>) -> Self {
        Self {
            config: Client { id, connected_drone_ids: Vec::new() },
            receiver_msg,
            receiver_commands,
            send_req,
            send_packets,
            servers: Vec::new(),
            visited_nodes: HashSet::new(),
            flood: Vec::new(),
            unique_flood_id: 0
        }
    }
    pub fn run(&mut self) {
        loop {
            select_biased! {
                recv(self.receiver_commands) -> command =>{
                    if let Ok(command) = command {
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
            CommandChat::SendMessage(destination_id, content) => {
                let message_to_send = MessageChat::new(content, self.config.id.clone(), destination_id);
                self.send_message(message_to_send);
            }
            CommandChat::EndChat(server_id) => {
                self.end_chat(server_id);
            }
        }
    }
    pub fn handle_incoming(&mut self, message: Packet) {
        match message.pack_type {
            PacketType::MsgFragment(fragment) => {},
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
            if !self.servers.contains(&id_server){
                println!("server not found ");
                return;
            }//to start flooding if it wasn't done before
        }
        match self.find_route(&id_server) {
            Ok(route) => {
                if let Some(next_hop) = route.get(1){
                    self.send_request(&next_hop, ChatRequest::ServerType)
                }
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
            if !self.servers.contains(&id_server){
                println!("The server was not found during flooding");
                return;
            }
        }

        match self.find_route(&id_server) {
            Ok(route) => {
                if let Some(next_hop) = route.get(1){
                    println!("Sent request to register this client to the server {}", id_server);
                    self.send_request(&next_hop, ChatRequest::RegisterClient(self.config.id.clone()))
                }
            }
            Err(_) => {println!("No route found for the destination client")}
        }

    }
    pub fn get_list_clients(&mut self, id_server: NodeId) {
        if self.servers.is_empty(){
            self.initiate_flooding();
            if !self.servers.contains(&id_server){
                println!("server not found after the flooding");
                return;
            }
        }

        match self.find_route(&id_server) {
            Ok(route) => {
                if let Some(next_hop) = route.get(1){
                    println!("sent request to get list clients of registered servers to server: {}", id_server);
                    self.send_request(&next_hop, ChatRequest::GetListClients)
                }
            }
            Err(_) => {println!("No route found for the destination client")}
        }
    }
    pub fn send_message(&mut self, message: MessageChat) {
        if self.flood.is_empty(){
            self.initiate_flooding();
            if !self.servers.is_empty(){
                println!("servers not found after the flooding");
                return;
            }
        }
        //i should check the length of each path to find the shortest one to a communication server of the ones saved
        //if the server is not a communication server?
        //how can i check the type of each server during flooding




    }
    pub fn end_chat(&mut self, id_server: NodeId) {
        if self.servers.is_empty(){
            self.initiate_flooding();
            if !self.servers.contains(&id_server){
                println!("The server was not found during flooding");
                return;
            }
        }

        match self.find_route(&id_server) {
            Ok(route) => {
                if let Some(next_hop) = route.get(1){
                    self.send_request(&next_hop, ChatRequest::EndChat(self.config.id.clone()))
                }
            }
            Err(_) => {println!("No route found for the destination client")}
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
                path_trace: Vec::new(),
            }
        );

        for (_, sender) in &self.send_packets { //directly using the sender in the loop
            if let Err(_) = sender.send(flood_request.clone()) {
                println!("Error sending the flood request")
            }
        }

    }

    pub fn handle_flood_req(& mut self, packet: Packet){
        if let PacketType::FloodRequest(mut flood_request) = packet.clone().pack_type {
            if self.visited_nodes.contains(&(flood_request.flood_id, flood_request.initiator_id)) { //case if client has already received the request
                flood_request.path_trace.push((self.config.id, NodeType::Client));

                if let Some(first_hop) = Some(packet.routing_header.hops[0] ) {
                    if let Some(sender) = self.send_packets.get(&first_hop) {
                       if let Err(e) = sender.send(flood_request.generate_response(packet.session_id)){
                           println!("Error sending the flood request: {}", e)
                       }
                    }else { println!("No sender found") }
                }else { println!("No next hop found") }
                return;
            }

            self.visited_nodes.insert((flood_request.flood_id, flood_request.initiator_id)); //mark as visited
            flood_request.path_trace.push((self.config.id, NodeType::Client));

            if let Some(sender) = packet.routing_header.hops.get(0){
                for (neighbor, sender) in &self.send_packets{
                    if *neighbor != packet.routing_header.hops[0]{ //forward to everyone that's not the one sending the
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

    pub fn send_request(& mut self, destination_id: &NodeId, request: ChatRequest){ //THE DESTINATION ID NEEDS TO BE THE ONE OF FIRST  DRONE IN THE FASTEST PATH TO REACH DESTINATION
        if let Some(sender) = self.send_req.get(&destination_id){
            if let Err(err) = sender.send(request){
                println!("Error sending command: {}", err); //have to send back nack
            }
        }else { println!("no sender") } //have to send back nack
    }
    pub fn send_packet(& mut self, destination_id: &NodeId, packet: Packet){
        if let Some(sender) = self.send_packets.get(&destination_id){
            if let Err(err) = sender.send(packet){
                println!("Error sending command: {}", err); //have to send back nack
            }
        }else { println!("no sender") } //have to send back nack
    }
}

pub fn main(){
    let (packet_tx, packet_rx) = unbounded();
    let (command_tx, command_rx) = unbounded();

    let mut send_packets = HashMap::new();
    send_packets.insert(1, packet_tx);

    let send_req = HashMap::new();

    let mut chat_client = ChatClient::new(0, packet_rx, send_packets, command_rx, send_req);

    // Mock server discovery and request
    command_tx.send(CommandChat::ServerType(1)).unwrap();

    chat_client.run(); // This will process commands and send requests
}
