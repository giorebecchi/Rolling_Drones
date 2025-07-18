use std::collections::{HashMap, HashSet};
use crossbeam_channel::{select_biased, Receiver, Sender};
use petgraph::algo::dijkstra;
use petgraph::{Direction};
use petgraph::graphmap::{UnGraphMap};
use wg_2024::config::{Client};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, FloodResponse, Fragment, NackType, NodeType, Packet, PacketType};
use crate::clients::assembler::{Fragmentation, NodeData};
use crate::common_data::common::{ChatRequest, MessageChat, CommandChat, ChatResponse, ServerType, ChatClientEvent, ClientType, BackGroundFlood, RequestEvent};
use crate::common_data::common::ChatClientEvent::{ClientList, ClientType as OtherClientType, IncomingMessage, RegisteredSuccess};

pub struct ChatClient {
    pub config: Client,
    pub receiver_msg: Receiver<Packet>,
    pub receiver_commands: Receiver<CommandChat>, //command received by the simulation control
    pub send_packets: HashMap<NodeId, Sender<Packet>>,
    pub servers: Vec<NodeId>,//to store id server once the flood is done
    pub visited_nodes: HashSet<(u64, NodeId)>,
    pub flood: Vec<FloodResponse> ,//to store all the flood responses found
    pub unique_flood_id: u64,
    pub session_id_packet: u64,
    pub incoming_fragments: HashMap<(u64, NodeId ), HashMap<u64, Fragment>>,
    pub fragments_sent: HashMap<u64, HashMap<u64, Fragment> >, //used for sending the correct fragment if was lost in the process
    pub problematic_nodes: Vec<NodeId>,
    pub chat_servers: Vec<NodeId>,
    pub clients: Vec<NodeId>,
    pub event_send : Sender<ChatClientEvent>,
    pub topology: UnGraphMap<NodeId, u32>,
    pub node_data: HashMap<NodeId, NodeData>,
    pub packet_sent: HashMap<u64, (NodeId, Vec<Packet>)>,
    pub rcv_flood: Receiver<BackGroundFlood>
}
impl ChatClient {
    pub fn new(
        id: NodeId, receiver_msg: Receiver<Packet>,
        send_packets: HashMap<NodeId, Sender<Packet>>,
        receiver_commands: Receiver<CommandChat>,
        event_send: Sender<ChatClientEvent>,
        rcv_flood: Receiver<BackGroundFlood>
    ) -> Self {
        Self {
            config: Client { id, connected_drone_ids: Vec::new() },
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
            clients: vec![id],
            event_send,
            topology: UnGraphMap::new(),
            node_data: HashMap::new(),
            packet_sent: HashMap::new(),
            rcv_flood
        }
    }
    pub fn run(&mut self) {
        self.send_type_client();

        loop{
            select_biased! {
                recv(self.receiver_msg) -> message =>{
                    if let Ok(message) = message {
                        self.handle_incoming(message)
                    }
                }
                recv(self.rcv_flood) -> flood =>{
                    if let Ok(_) = flood{
                        self.initiate_flooding();
                    }
                }
                
                recv(self.receiver_commands) -> command =>{
                    if let Ok(command) = command {
                        self.handle_sim_command(command);
                    }
                }
                
            }

        }

    }

    fn send_type_client(& mut self){
        if let Err(_) = self.event_send.send(OtherClientType(ClientType::ChatClient,self.config.id.clone())){
            println!("Error sending client type");
        }

    }

    fn handle_sim_command(&mut self, command: CommandChat) {
        match command {
            CommandChat::RegisterClient(id_server) => {
                self.register_client(id_server);
            }
            CommandChat::SendMessage(destination_id, id_server,  content) => {
                let message_to_send = MessageChat::new(content, self.config.id.clone(), destination_id);
                self.send_message(message_to_send, id_server);
            }
            CommandChat::SearchChatServers => {
                self.search_chat_servers();
            }
            CommandChat::SendTopologyGraph => {
                self.send_topology_graph();
            }
            CommandChat::AddSender(node_id, sender) => {
                self.add_sender(node_id, sender);
            }
            CommandChat::RemoveSender(node_id) => {
                self.remove_sender(node_id);
            }
            CommandChat::PdrChanged(node_id) => {
                self.reset_data(node_id)
            }
        }
    }
    
    fn reset_data(&mut self, node_id: NodeId) {
        if let Some(data) = self.node_data.get_mut(&node_id) {
            data.reset(); //reset the data of the node id
        }

        let neighbors: Vec<NodeId> = self.topology.neighbors(node_id).collect(); //get neighbors of the node
        for neighbor in neighbors {
            self.topology.add_edge(node_id, neighbor, 1);
        }
    }
    fn send_topology_graph(&self){
        self.event_send.send(ChatClientEvent::Graph(self.config.id, self.topology.clone())).unwrap();
    }
    
    fn handle_incoming(&mut self, message: Packet) {
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
    
    fn add_sender(&mut self, node_to_add: NodeId, sender: Sender<Packet>) {
        if !self.send_packets.contains_key(&node_to_add){
            self.send_packets.insert(node_to_add, sender);
        }else {
            return;
        }
    }
    
    fn remove_sender(&mut self, node: NodeId) {
        if self.send_packets.contains_key(&node){
            self.send_packets.remove(&node);
        }else {
            return;
        }
    }

    fn search_chat_servers(&mut self) {
        for server in self.servers.clone(){
            self.ask_server_type(server);
        }
    }

    fn ask_server_type(&mut self, id_server: NodeId) {
        if !self.servers.contains(&id_server){
            return;
        }
        let request_to_send = ChatRequest::ServerType;
        
        let session_id = self.session_id_packet;
        self.session_id_packet += 1;
        
        let fragments = ChatRequest::fragment_message(&request_to_send);
        self.fragments_sent.insert(session_id, fragments.clone());

        match self.find_best_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(), session_id);
                self.packet_sent.insert(session_id, (id_server, packets_to_send.clone()));
                
                for packet in packets_to_send {
                    if let PacketType::MsgFragment(fragment) = packet.pack_type.clone(){
                        if let Err(_) = self.event_send.send(ChatClientEvent::InfoRequest(self.config.id, RequestEvent::AskType(fragment.total_n_fragments), packet.session_id )){
                            println!("chat client failed to notify SC about ask types request")
                        }
                    }
                    if let Some(next_hop) = route.get(1){
                        if let Err(()) = self.send_packet(next_hop, packet){
                            self.ask_server_type(id_server);
                        }
                    }else { 
                        return;
                    }
                }

            }
            Err(_) => {println!("No route found for the destination server")}
        }
    }
    fn register_client(&mut self, id_server: NodeId) {
        if !self.servers.contains(&id_server){
            return;
        }
        let request = ChatRequest::RegisterClient(self.config.id.clone());
        
        let session_id = self.session_id_packet;
        self.session_id_packet += 1;

        let fragments = ChatRequest::fragment_message(&request);
        self.fragments_sent.insert(session_id, fragments.clone());

        match self.find_best_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(), session_id);
                self.packet_sent.insert(session_id, (id_server, packets_to_send.clone()));
                
                for packet in packets_to_send {
                    if let PacketType::MsgFragment(fragment) = packet.pack_type.clone(){
                        if let Err(_) = self.event_send.send(ChatClientEvent::InfoRequest(self.config.id, RequestEvent::Register(fragment.total_n_fragments), packet.session_id )){
                            println!("chat client failed to notify SC about register request")
                        }
                    }
                    if let Some(next_hop) = route.get(1){
                        if let Err(()) = self.send_packet(next_hop, packet){
                            self.register_client(id_server);
                        }
                    }else { return; }
                }
            }
            Err(_) => {println!("No route found for the destination client")}
        }

    }
    fn send_message(&mut self, message: MessageChat, id_server: NodeId) {
        if !self.servers.contains(&id_server) {
            return;
        }

        let request = ChatRequest::SendMessage(message.clone(), id_server);

        let session_id = self.session_id_packet;
        self.session_id_packet += 1;

        let fragments = ChatRequest::fragment_message(&request);
        self.fragments_sent.insert(session_id, fragments.clone());

        match self.find_best_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(), session_id);
                self.packet_sent.insert(session_id, (id_server, packets_to_send.clone()));
                for packet in packets_to_send {
                    if let PacketType::MsgFragment(fragment) = packet.pack_type.clone(){
                        if let Err(_) = self.event_send.send(ChatClientEvent::InfoRequest(self.config.id, RequestEvent::SendMessage(fragment.total_n_fragments), packet.session_id )){
                            println!("chat client failed to notify SC about send message request")
                        }
                    }
                    if let Some(next_hop) = route.get(1) {
                        if let Err(()) = self.send_packet(next_hop, packet){
                            self.send_message(message.clone(), id_server);
                        }
                    } else { return; }
                }
            }
            Err(_) => { println!("No route found for the destination client") }
        }
    }
    
    //incoming messages
    fn handle_fragments(& mut self, packet: Packet){ 
        let src_id = packet.routing_header.hops.first().unwrap();
        let check = (packet.session_id, *src_id);

        let ack = self.ack(&packet);
        let prev = packet.routing_header.hops[packet.routing_header.hop_index-1];
        if let Err(()) = self.send_packet(&prev, ack){
            self.handle_fragments(packet.clone());
        }

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
                        ChatResponse::ServerTypeChat(server_type) => {
                            if server_type == ServerType::CommunicationServer && !self.chat_servers.contains(&src_id) {
                                self.chat_servers.push(src_id.clone());
                            }

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
                                if let Err(_) = self.event_send.send(RegisteredSuccess((self.config.id.clone(), src_id.clone()), Ok(()))){
                                    println!("could not send to simulation control");
                                }
                            }else {
                                if let Err(_) = self.event_send.send(RegisteredSuccess((self.config.id.clone(), src_id.clone()), Err("registration not successful".to_string()))){
                                    println!("could not send to simulation control");
                                }
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
                            if let Err(str) = self.event_send.send(IncomingMessage((self.config.id.clone(), src_id.clone(), sender), message_chat.content)){
                                println!("failed to send message to simulation control: {}", str);
                            }
                        }
                        _ =>{}
                    }

                    self.incoming_fragments.remove(&check); //removes fragments from tracking
                }
            }
        }
    }

    fn handle_nacks(& mut self, packet: Packet){
        if let PacketType::Nack(nack) = packet.clone().pack_type{
            match nack.nack_type{
                NackType::ErrorInRouting(id) => {
                    if !self.problematic_nodes.contains(&id){
                        self.problematic_nodes.push(id);
                    }

                    let dest= id;
                    let src = packet.routing_header.hops[0];
                    self.topology.remove_edge(src, dest);
                    
                    self.resend_fragment_lost(packet);
                }
                NackType::Dropped => {
                    let failing_node = packet.routing_header.hops[0];
                    let data = self.node_data.get_mut(&failing_node).unwrap();
                    data.dropped += 1;
                    
                    self.resend_fragment_lost(packet);
                }
                _ => {}

            }
        }
    }
    
   
    fn resend_fragment_lost(& mut self, packet: Packet){
        if let PacketType::Nack(nack) = packet.clone().pack_type{
            let fragments_session = if let Some(fragments_session) = self.fragments_sent.get(&packet.session_id){
                fragments_session
            }else { 
                println!("no fragments found for this session id");
                return;
            };
            
            let fragment_lost = if let Some(fragment_lost) = fragments_session.get(&nack.fragment_index){
                fragment_lost.clone()
            }else {
                return;
            };

            let destination_id = match self.packet_sent.get(&packet.session_id){
                Some((destination_id, _)) => destination_id.clone(),
                None => {
                    return;
                }
            };

            match self.find_best_route(&destination_id) {
                Ok(route) => {
                    //println!("route re-computed: {:?}", route);
                    let packet_to_send = Packet::new_fragment(
                        SourceRoutingHeader::new(route.clone(), 0),
                        packet.session_id,
                        fragment_lost
                    );

                    if let Some(next_hop) = route.get(1){
                        if let Err(()) = self.send_packet(next_hop, packet_to_send){
                            self.resend_fragment_lost(packet);
                        }
                    }else { return; }

                }
                Err(_) => println!("no route found to resend packet"),
            }
            
            
        }
    }

    fn handle_ack(& mut self, packet: Packet){
        if let PacketType::Ack(ack) = packet.pack_type{
            self.problematic_nodes.clear();
            
            self.fragments_sent.retain(|index, _| *index != ack.fragment_index); //this filters the hashmap, removing the ones with that index
            let data = self.node_data.get_mut(&packet.routing_header.hops.iter().rev().nth(1).unwrap()).unwrap();
            data.forwarded += 1;
        }
    }

    fn initiate_flooding(&mut self) { //this sends a flood request to its immediate neighbors
        let flood_id = self.unique_flood_id;
        self.unique_flood_id += 1;

        let flood_request = Packet::new_flood_request(
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

    fn handle_flood_req(& mut self, packet: Packet) {
        if let PacketType::FloodRequest(mut flood_request) = packet.clone().pack_type {
            //check if the pair (flood_id, initiator id) has already been received -> self.visited_nodes
            if self.visited_nodes.contains(&(flood_request.flood_id, flood_request.initiator_id)){
                flood_request.path_trace.push((self.config.id.clone(), NodeType::Client));
                self.send_flooding_packet( flood_request.generate_response(packet.session_id) );
            }else {
                flood_request.path_trace.push((self.config.id.clone(), NodeType::Client));
                self.visited_nodes.insert((flood_request.flood_id, flood_request.initiator_id));

                if self.send_packets.len() == 1{
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

    fn handle_flood_response(& mut self, packet: Packet){
        if let PacketType::FloodResponse(flood_response) = packet.clone().pack_type {

            let path = &flood_response.path_trace;
            for pair in path.windows(2) {
                let (src, _) = pair[0];
                let (dst, _) = pair[1];

                if !self.topology.contains_edge(src.clone(), dst.clone()){
                    self.node_data.insert(src, NodeData::new());
                    self.node_data.insert(dst, NodeData::new());

                    self.topology.add_edge(src.clone(), dst.clone(), 1);
                }
            }
            
            let dest = if let Some(dest ) = flood_response.path_trace.get(0){
                dest.clone()
            }else {
                return;
            };
            
            if dest != (self.config.id, NodeType::Client) {
                self.send_flooding_packet(packet);
                return;
            }

            if !flood_response.path_trace.is_empty(){
                for (node_id, node_type) in &flood_response.path_trace{
                    if *node_type == NodeType::Server && !self.servers.contains(&node_id){
                        self.servers.push(*node_id); //no duplicates
                    }
                    if *node_type == NodeType::Client && !self.clients.contains(&node_id){
                        self.clients.push(*node_id);
                    }
                }
                
            }
            self.flood.push(flood_response.clone()); //storing all the flood responses to then access the path traces and find the quickest one
            
        }
    }

    fn find_best_route(& mut self, destination_id: &NodeId)-> Result<Vec<NodeId>, String>{
        let source = self.config.id.clone();

        let result = dijkstra(&self.topology, source.clone(), Some(destination_id.clone()), |edge|{
            let dest = edge.1;
            
            if self.problematic_nodes.contains(&dest) 
                || self.clients.contains(&dest)
                || (self.servers.contains(&dest) && &dest != destination_id){
                1_000
            }else {
                let reliability = self.node_data.get(&dest).map(|data| data.reliability()).unwrap_or(1.0);
                if reliability <= 0.0 {
                    1_000
                } else {
                    ((1.0 / reliability).ceil() as u32).min(1_000)
                }
            }
             //ceil returns integer closest to result
            //this transforms the reliability into a weight. when we have higher reliability we need a smaller cost
        });

        if let Some(_) = result.get(destination_id) {
            let mut path = vec![destination_id.clone()];
            let mut current = destination_id.clone();

            while current != source {
                let mut found = false;
                for edge in self.topology.edges_directed(current.clone(), Direction::Incoming) {
                    let prev = edge.0;

                    if let (Some(&prev_cost), Some(&curr_cost)) = (result.get(&prev), result.get(&current)) {
                        let dest = edge.1;
                        let weight = if self.problematic_nodes.contains(&dest) {
                            1_000
                        } else {
                            let reliability = self.node_data.get(&dest).map(|d| d.reliability()).unwrap_or(1.0);
                            if reliability <= 0.0 {
                                1_000
                            } else {
                                ((1.0 / reliability).ceil() as u32).min(1_000)
                            }
                        };
                        
                        if prev_cost + weight == curr_cost {
                            path.push(prev.clone());
                            current = prev;
                            found = true;
                            break;
                        }
                    }
                }

                if !found {
                    return Err("failed to reconstruct path".to_string());
                }
            }
            path.reverse();
            Ok(path)
        }else {
            Err("no route found!!".to_string())
        }
    }

    fn send_packet(& mut self, destination_id: &NodeId, mut packet: Packet) -> Result<(), ()>{
        packet.routing_header.hop_index+=1;
        if let Some(sender) = self.send_packets.get(&destination_id){
            if let Err(err) = sender.send(packet.clone()){
                println!("Error sending command to SC : {}", err);
            }
            Ok(())
        }else {
            self.topology.remove_edge(self.config.id, *destination_id);
            Err(())
        }
    }

    fn send_flooding_packet(& mut self, mut packet: Packet){
        if packet.routing_header.hop_index < packet.routing_header.hops.len() -1 {
            packet.routing_header.hop_index += 1;
            let next_hop = packet.routing_header.hops[packet.routing_header.hop_index];
            if let Some(sender) = self.send_packets.get(&next_hop) {
                sender.send(packet.clone()).unwrap_or_default();
            }
        } else {
            return;
        }
    }

    fn ack(& mut self, packet: &Packet)-> Packet{
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
