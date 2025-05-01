use std::collections::{HashMap, HashSet};
use crossbeam_channel::{select_biased, Receiver, Sender};
use petgraph::algo::dijkstra;
use petgraph::data::Build;
use petgraph::Direction;
use petgraph::graphmap::DiGraphMap;
use petgraph::prelude::EdgeRef;
use petgraph::visit::IntoEdgesDirected;
use wg_2024::config::{Client};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, FloodResponse, Fragment, NackType, NodeType, Packet, PacketType};
use crate::clients::assembler::{Fragmentation};
use crate::common_things::common::{ChatRequest, MessageChat, CommandChat, ChatResponse, ServerType, ChatClientEvent, ClientType};
use crate::common_things::common::ChatClientEvent::{ClientList, ClientType as OtherClientType, IncomingMessage, RegisteredSuccess};

pub struct ChatClient {
    pub config: Client,
    pub client_type: ClientType,
    pub receiver_msg: Receiver<Packet>,
    pub receiver_commands: Receiver<CommandChat>, //command received by the simulation control
    pub send_packets: HashMap<NodeId, Sender<Packet>>,
    pub servers: Vec<NodeId>,//to store id server once the flood is done
    pub visited_nodes: HashSet<(u64, NodeId)>,
    pub flood: Vec<FloodResponse> ,//to store all the flood responses found
    pub unique_flood_id: u64,
    pub session_id_packet: u64,
    pub incoming_fragments: HashMap<(u64, NodeId ), HashMap<u64, Fragment>>,
    pub fragments_sent: HashMap<u64, Fragment>, //used for sending the correct fragment if was lost in the process
    pub problematic_nodes: Vec<NodeId>,
    pub chat_servers: Vec<NodeId>,
    pub event_send : Sender<ChatClientEvent>,
    pub topology: DiGraphMap<NodeId, u32>,
    pub packet_sent: (NodeId, Vec<Packet>),
}
impl ChatClient {
    pub fn new(
        id: NodeId, receiver_msg: Receiver<Packet>,
        send_packets: HashMap<NodeId, Sender<Packet>>,
        receiver_commands: Receiver<CommandChat>,
        event_send: Sender<ChatClientEvent>
    ) -> Self {
        Self {
            config: Client { id, connected_drone_ids: Vec::new() },
            client_type: ClientType::ChatClient,
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
            chat_servers: Vec::new(),
            event_send,
            topology: DiGraphMap::new(),
            packet_sent: (0, Vec::new()),
        }
    }
    pub fn run(&mut self) {
        self.initiate_flooding();
        self.send_type_client();

        loop{
            select_biased! {
                recv(self.receiver_commands) -> command =>{
                    if let Ok(command) = command {
                        self.build_topology();
                        self.handle_sim_command(command);
                    }
                }
                recv(self.receiver_msg) -> message =>{
                    if let Ok(message) = message {
                        self.build_topology();
                        self.handle_incoming(message)
                    }
                }
            }

        }

    }

    pub fn send_type_client(& mut self){
        if let Err(_) = self.event_send.send(OtherClientType(self.client_type.clone(),self.config.id.clone())){
            println!("Error sending client type");
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
            CommandChat::SearchChatServers => {
                self.search_chat_servers();
            }
            _ => {}
        }
    }
    pub fn handle_incoming(&mut self, message: Packet) {
        match message.pack_type {
            PacketType::MsgFragment(_) => {
                self.handle_fragments(message);
            },
            PacketType::Ack(_) => {self.handle_ack(message);},
            PacketType::Nack(_) => {self.handle_nacks(message);},
            PacketType::FloodRequest(_) => { self.handle_flood_req(message); },
            PacketType::FloodResponse(_) => { self.handle_flood_response(message); },
        }
    }

    pub fn search_chat_servers(&mut self) {
        for server in self.servers.clone(){
            self.ask_server_type(server);
        }
    }

    pub fn ask_server_type(&mut self, id_server: NodeId) {
        if !self.servers.contains(&id_server){
            println!("server not found ");
            return;
        }
        println!("servers: {:?}",self.servers);
        let request_to_send = ChatRequest::ServerType;
        self.fragments_sent = ChatRequest::fragment_message(&request_to_send);

        match self.find_route(&id_server, None) {
            Ok(route) => {
                println!("route: {:?}", route);
                let packets_to_send = ChatRequest::create_packet(&self.fragments_sent, route.clone(), &mut self.session_id_packet);
                for packet in packets_to_send {
                    if let Some(next_hop) = route.get(1){
                        self.send_packet(next_hop, packet);
                    }else { println!("No next hop found") }
                }
                println!("Sent request to get the server type to server: {}", id_server);

            }
            Err(_) => {println!("No route found for the destination server")}
        }
    }
    pub fn register_client(&mut self, id_server: NodeId) {
        if !self.servers.contains(&id_server){
            println!("The server was not found during flooding");
            return;
        }
        let request = ChatRequest::RegisterClient(self.config.id.clone());
        self.fragments_sent = ChatRequest::fragment_message(&request);

        match self.find_route(&id_server, None) {
            Ok(route) => {
                println!("route: {:?}", route);
                self.packet_sent = (id_server, ChatRequest::create_packet(&self.fragments_sent, route.clone(),  & mut self.session_id_packet));
                for packet in self.packet_sent.1.clone() { //odio sti clone
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
        if !self.servers.contains(&id_server){
            println!("server not found after the flooding");
            return;
        }
        let request = ChatRequest::GetListClients;
        self.fragments_sent = ChatRequest::fragment_message(&request);

        match self.find_route(&id_server, None) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&self.fragments_sent, route.clone(), & mut self.session_id_packet);
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
        if !self.servers.contains(&id_server){
            println!("server not found after the flooding");
            return;
        }

        let request = ChatRequest::SendMessage(message, id_server);
        self.fragments_sent = ChatRequest::fragment_message(&request);

        match self.find_route(&id_server, None){
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&self.fragments_sent, route.clone(), & mut self.session_id_packet);
                for packet in packets_to_send {

                    println!("route: {}",packet.routing_header);
                    if let Some(next_hop) = packet.clone().routing_header.hops.get(1){
                        self.send_packet(next_hop, packet);
                    }
                }
            }
            Err(_) => {println!("No route found for the destination client")}
        }

    }

    pub fn end_chat(&mut self, id_server: NodeId) {
        if !self.servers.contains(&id_server){
            println!("server not found after the flooding");
            return;
        }

        let request = ChatRequest::EndChat(self.config.id.clone());
        self.fragments_sent = ChatRequest::fragment_message(&request);

        match self.find_route(&id_server, None) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&self.fragments_sent, route.clone(),  & mut self.session_id_packet);
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

    //incoming messages
    pub fn handle_fragments(& mut self, mut packet: Packet){ //doesn't perfectly respect the protocol [uses hashmap instead of the vector]
        // println!("received packet by server: {}", packet);
        let src_id = packet.routing_header.hops.first().unwrap();
        let check = (packet.session_id, *src_id);

        let ack = self.ack(&packet);
        let prev = packet.routing_header.hops[packet.routing_header.hop_index-1];
        self.send_packet(&prev, ack);

        if let PacketType::MsgFragment(fragment) = packet.pack_type{
            if !self.incoming_fragments.contains_key(&check){
                self.incoming_fragments.insert(check, HashMap::new());
            }
            self.incoming_fragments
                .get_mut(&check)
                .unwrap()
                .insert(fragment.fragment_index, fragment.clone());

            if let Some(fragments) = self.incoming_fragments.get_mut(&check){
                if fragments.len() as u64 == fragment.total_n_fragments {
                    let incoming_message = ChatResponse::reassemble_msg(&fragments).unwrap();
                    match incoming_message {
                        ChatResponse::ServerType(server_type) => {
                            println!("server found is of type: {:?}", server_type);
                            if server_type == ServerType::CommunicationServer && !self.chat_servers.contains(&src_id) {
                                self.chat_servers.push(src_id.clone());
                            }



                            println!("sending to sc");
                            if let Err(err) = self.event_send.send(ChatClientEvent::ChatServers(self.config.id.clone(), self.chat_servers.clone())) {
                                println!("Failed to notify SC about server list: {}", err);
                            }

                    },
                    ChatResponse::EndChat(response) =>{

                            if response {
                                println!("chat ended");
                            }else { println!("error in the request: end the chat") }
                        },
                        ChatResponse::RegisterClient(response) => {
                            if response{
                                println!("registered successfully");
                                if let Err(_) = self.event_send.send(RegisteredSuccess((self.config.id.clone(), src_id.clone()), Ok(()))){
                                    println!("could not send to simulation control");
                                }
                            }else {
                                println!("not registered successfully");
                                if let Err(_) = self.event_send.send(RegisteredSuccess((self.config.id.clone(), src_id.clone()), Err("registration not successful".to_string()))){
                                    println!("could not send to simulation control");
                                }
                            }
                        },
                        ChatResponse::SendMessage(response) => {
                            match response{
                                Ok(str) => println!("{}", str),
                                Err(str) => println!("{}", str),
                            }
                        },
                        ChatResponse::RegisteredClients(registered_clients) => {
                            println!("registered clients: {:?}", registered_clients);
                            if let Err(_) = self.event_send.send(ClientList((self.config.id.clone(), src_id.clone()), registered_clients)){
                                println!("failed to send the registered clients to sc");
                            }
                        },
                        ChatResponse::ForwardMessage(message_chat) =>{
                            let sender = message_chat.from_id;
                            println!("Message from: {}, content:\n{}", sender, message_chat.content);
                            if let Err(str) = self.event_send.send(IncomingMessage((self.config.id.clone(), src_id.clone(), sender), message_chat.content)){
                                println!("failed to send message to simulation control: {}", str);
                            }
                        }
                    }

                    self.incoming_fragments.remove(&check); //removes fragments from tracking
                }
            }
        }
    }

    pub fn handle_nacks(& mut self, mut packet: Packet){
        if let PacketType::Nack(nack) = packet.pack_type{
            match nack.nack_type{
                NackType::ErrorInRouting(id) => {
                    //need to resend packet changing the route, not including the node specified
                    let failed_path = packet.routing_header.clone();
                    println!("failed path: {}", failed_path);

                }
                NackType::Dropped => {
                    //resend packets dropped, which should be the one dropped by the drone and the packet remaining inside the hashmap

                    let failed_src = packet.routing_header.hops[packet.routing_header.hop_index];
                    let failed_node = packet.routing_header.hops[0];

                    if !self.problematic_nodes.contains(&failed_node){
                        self.problematic_nodes.push(failed_node);
                    }

                    println!("failed src: {}", failed_src);
                    println!("failed node: {}", failed_node);

                    let mut copy_topology = self.topology.clone();

                    copy_topology.remove_node(failed_node);

                    // if let Some(edge) = self.topology.edges_directed(failed_node, Direction::Outgoing){}

                    let fragment_to_resend = if let Some(fragment_lost) = self.fragments_sent.get(&nack.fragment_index){
                        println!("fragment lost: {:?}", fragment_lost);
                        fragment_lost.clone()
                    }else {
                        println!("no fragment lost");
                        return
                    };

                    let destination_id = self.packet_sent.0;
                    println!("destination id: {}", destination_id);

                    match self.find_route(&destination_id, Some(&self.problematic_nodes.clone())){
                        Ok(route) => {
                            println!("found new route! {:?}", route);
                            let new_packet = Packet::new_fragment(SourceRoutingHeader::new(route.clone(), 0), packet.session_id, fragment_to_resend);
                            // println!("routing header fragment created: {:?}", new_packet.routing_header );
                            // println!("{:?}", new_packet.routing_header.hop_index);
                            // println!("fragment inside the packet: {:?}", new_packet.pack_type);

                            if let Some(next_hop) = route.get(1){
                                println!("next hop: {}", next_hop);
                                self.send_packet(next_hop, new_packet);
                            }else { println!("No next hop found") }
                        }
                        Err(_) => {
                            println!("failed to find new route!");
                        }
                    }

                }
                _ => {}

            }
        }
    }

    pub fn handle_ack(& mut self, packet: Packet){
        if let PacketType::Ack(ack) = packet.pack_type{
            self.fragments_sent.retain(|index, _| *index != ack.fragment_index); //this filters the hashmap, removing the ones with that index
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

    pub fn handle_flood_req(& mut self, packet: Packet) {
        if let PacketType::FloodRequest(mut flood_request) = packet.clone().pack_type {
            //check if the pair (flood_id, initiator id) has already been received -> self.visited_nodes
            if self.visited_nodes.contains(&(flood_request.flood_id, flood_request.initiator_id)){
                flood_request.path_trace.push((self.config.id.clone(), NodeType::Client));
                // println!("path trace here: {:?}", flood_request.path_trace);
                //
                // if let Some(next_hop) = flood_request.path_trace.iter().rev().nth(1){
                //     println!("next hop: {}", next_hop.0);
                //     self.send_packet(&next_hop.0, flood_request.generate_response(packet.session_id) );
                // }else { println!("No next hop found") }
                self.send_flooding_packet( flood_request.generate_response(packet.session_id) );
            }else {
                flood_request.path_trace.push((self.config.id.clone(), NodeType::Client));
                self.visited_nodes.insert((flood_request.flood_id, flood_request.initiator_id));

                if self.send_packets.len() == 1{
                    // println!("case of no neighbours, path trace: {:?}", flood_request.path_trace);
                    // println!("sender: {:?}", self.send_packets);
                    // if let Some(next_hop) = flood_request.path_trace.iter().rev().nth(1){
                    //     println!("next hop: {}", next_hop.0);
                    //
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


pub fn build_topology(& mut self) {
    self.topology.clear();

    for resp in &self.flood {
        let path = &resp.path_trace;
            for pair in path.windows(2) {
                let (src, _) = pair[0];
                let (dst, _) = pair[1];
                self.topology.add_edge(src.clone(), dst.clone(), 1); // use 1 as weight (hop)
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
                // println!("flood_response: {:?}", self.flood);
            }

        }
    }

    pub fn find_route(& mut self, destination_id : &NodeId, avoid_node: Option<&Vec<NodeId>>)-> Result<Vec<NodeId>, String>{
        let source = self.config.id.clone();

        //da rivedere per handle_nack
        let topology = if let Some(nodes_to_avoid) = avoid_node{
            let mut modified_topology = self.topology.clone();
            for node in nodes_to_avoid{
                modified_topology.remove_node(*node);
            }
            modified_topology
        } else {
            self.topology.clone()
        };

        let result = dijkstra(&topology, source.clone(), Some(destination_id.clone()), |_| 1);

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

    pub fn send_packet(& mut self, destination_id: &NodeId, mut packet: Packet){
        packet.routing_header.hop_index+=1;
        if let Some(sender) = self.send_packets.get(&destination_id){
            if let Err(err) = sender.send(packet.clone()){
                println!("Error sending command: {}", err); //have to send back nack
            }
        }else { println!("no sender") } //have to send back nack
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
