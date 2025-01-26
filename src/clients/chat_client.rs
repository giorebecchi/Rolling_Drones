use std::collections::{HashMap, HashSet};
use std::os::unix::raw::mode_t;
use crossbeam_channel::{select_biased, unbounded, Receiver, RecvError, Sender};
use wg_2024::packet;
use wg_2024::controller;
use wg_2024::config;
use serde::{Serialize, Deserialize};
use wg_2024::config::{Client};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, FloodResponse, Fragment, NodeType, Packet, PacketType, FRAGMENT_DSIZE};
use crate::clients::assembler::{Fragmentation, MessageChat, Serialization};
use crate::common_things::common::{ChatRequest, CommandChat};
use crate::servers::ChatServer::Server;

//missing flooding, handling incoming packets and handling errors
pub struct ChatClient {
    pub config: Client,
    pub receiver_msg: Receiver<Packet>,
    pub receiver_commands: Receiver<CommandChat>,
    pub send_req: HashMap<NodeId, Sender<ChatRequest>>,
    pub send_packets: HashMap<NodeId, Sender<Packet>>,
    pub communication_servers: Vec<NodeId>,//to store id server once the flood is done
    pub visited_nodes: HashSet<(u64, NodeId)>,
    pub flood: Vec<FloodResponse> ,//to store all the flood responses found
    pub unique_flood_id: u64
}
impl ChatClient {
    fn new(id: NodeId, receiver_msg: Receiver<Packet>, send_packets: HashMap<NodeId, Sender<Packet>>, receiver_commands: Receiver<CommandChat>, send_req: HashMap<NodeId, Sender<ChatRequest>>) -> Self {
        Self {
            config: Client { id, connected_drone_ids: Vec::new() },
            receiver_msg,
            receiver_commands,
            send_req,
            send_packets,
            communication_servers: Vec::new(),
            visited_nodes: HashSet::new(),
            flood: Vec::new(),
            unique_flood_id: 0
        }
    }
    fn run(&mut self) {
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

        //without the request of a specific server (no parameter), if the client wants to send the request to every server found after performing the flood

    }
    pub fn register_client(&mut self, id_server: NodeId) {
        //to register client to the server specified in the command by simulation control.

    }
    pub fn get_list_clients(&mut self, id_server: NodeId) {
        //get list from the server to see which are available
    }
    pub fn send_message(&mut self, message: MessageChat) {}
    pub fn end_chat(&mut self, id_server: NodeId) {}

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

        for (node_id, sender) in &self.send_packets {
            if let Some(sender) = self.send_packets.get(node_id) {
                sender.send(flood_request.clone()).unwrap();
            }else { println!("No sender found") }
        }

    }

    pub fn handle_flood_req(& mut self, packet: Packet){
        if let PacketType::FloodRequest(mut flood_request) = packet.clone().pack_type {
            if self.visited_nodes.contains(&(flood_request.flood_id, flood_request.initiator_id)) { //case if client have already received the request
                flood_request.path_trace.push((self.config.id, NodeType::Client));
                if let Some(sender) = self.send_packets.get(&packet.routing_header.hops[0]){
                    sender.send(flood_request.generate_response(packet.session_id)).unwrap()
                }
                return;
            }

            self.visited_nodes.insert((flood_request.flood_id, flood_request.initiator_id)); //mark as visited
            flood_request.path_trace.push((self.config.id, NodeType::Client));

            for (neighbor, sender) in &self.send_packets{
                if *neighbor != packet.routing_header.hops[0]{ //forward to everyone that's not the one sending the
                    let mut forward_packet = packet.clone();
                    forward_packet.pack_type = PacketType::FloodRequest(flood_request.clone());
                    sender.send(forward_packet).unwrap();
                }
            }

        }

    }

    pub fn handle_flood_response(& mut self, packet: Packet){
        if let PacketType::FloodResponse(flood_response) = packet.clone().pack_type {
            if !flood_response.path_trace.is_empty(){
                for (node_id, node_type) in &flood_response.path_trace{
                    if *node_type == NodeType::Server && !self.communication_servers.contains(&node_id){
                        self.communication_servers.push(*node_id); //no duplicates
                    }
                }
                self.flood.push(flood_response); //storing all the flood responses to then access the path traces and find the quickes one
            }
        }
    }

    pub fn find_route(& mut self, destination_id : NodeId)-> Vec<(NodeId, NodeType)>{
        let mut route = Vec::new();

        route

    }

    pub fn send_request(& mut self, destination_id: NodeId, request: ChatRequest){ //THE DESTINATION ID NEEDS TO BE THE ONE OF FIRST  DRONE IN THE FASTEST PATH TO REACH DESTINATION
        if let Some(sender) = self.send_req.get(&destination_id){
            if let Err(err) = sender.send(request){
                println!("Error sending command: {}", err); //have to send back nack
            }
        }else { println!("no sender") } //have to send back nack
    }
    pub fn send_packet(& mut self, destination_id: NodeId, packet: Packet){
        if let Some(sender) = self.send_packets.get(&destination_id){
            if let Err(err) = sender.send(packet){
                println!("Error sending command: {}", err); //have to send back nack
            }
        }else { println!("no sender") } //have to send back nack
    }
}

pub fn main(){

}
