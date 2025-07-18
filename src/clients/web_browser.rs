use crate::common_data::common::{BackGroundFlood, ContentRequest};
use std::collections::{HashMap, HashSet};
use std::fs;
use base64::Engine;
use crossbeam_channel::{select_biased, Receiver, Sender};
use petgraph::algo::dijkstra;
use petgraph::Direction;
use wg_2024::config::Client;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, FloodResponse, Fragment, NackType, NodeType, Packet, PacketType};
use crate::clients::assembler::{Fragmentation, NodeData};
use crate::common_data::common::{ChatRequest, ContentCommands, FileMetaData, MediaId, MediaServer, ServerType, TextServer, WebBrowserCommands, WebBrowserEvents};
use base64::engine::general_purpose::STANDARD as BASE64;
use petgraph::prelude::UnGraphMap;

pub struct WebBrowser {
    pub config: Client, //id and direct neighbours
    pub receiver_msg: Receiver<Packet>, 
    pub receiver_commands: Receiver<ContentCommands>, //commands received by the simulation control
    pub send_packets: HashMap<NodeId, Sender<Packet>>, //hashmap of the direct neighbors with their senders
    pub servers: Vec<NodeId>,//to store id server once the flood is done
    pub visited_nodes: HashSet<(u64, NodeId)>, //save node already visited during the flooding
    pub flood: Vec<FloodResponse> ,//to store all the flood responses found
    pub unique_flood_id: u64, 
    pub session_id_packet: u64,
    pub incoming_fragments: HashMap<(u64, NodeId ), HashMap<u64, Fragment>>, //used to save all the fragments from same sender with same session id, used in handle fragments (session id, source id), fragment index + fragment
    pub fragments_sent: HashMap<u64, HashMap<u64, Fragment>>, //used for re-sending the correct fragment if one was lost in the process, key session id packet, key 2 fragment index
    pub problematic_nodes: Vec<NodeId>, //used when receiving an error in routing, to avoid the node in a next path
    pub send_event: Sender<WebBrowserEvents>, //to send information to the simulation control
    pub media_servers: Vec<NodeId>,
    pub text_servers: Vec<NodeId>,
    pub clients: Vec<NodeId>, //saved during flooding, to avoid them in finding a path
    pub packet_sent: HashMap<u64, (NodeId, Vec<Packet>)>, //the key will be the session id of the packets sent, node id of the destination and all the packets to send
    pub topology_graph: UnGraphMap<NodeId, u32>, //save graph
    pub node_data: HashMap<NodeId, NodeData>, //used in the case of the Dropped error, for every node appearing in the graph
    pub rcv_flood: Receiver<BackGroundFlood> 
}

impl WebBrowser {
    pub fn new(
        id: NodeId, receiver_msg: Receiver<Packet>,
        receiver_commands: Receiver<ContentCommands>,
        send_packets: HashMap<NodeId, Sender<Packet>>,
        rcv_flood: Receiver<BackGroundFlood>,
        send_event: Sender<WebBrowserEvents>

    ) -> Self {
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
            send_event,
            media_servers: Vec::new(),
            text_servers: Vec::new(),
            clients: vec![id],
            packet_sent: HashMap::new(),
            topology_graph: UnGraphMap::new(),
            node_data: HashMap::new(),
            rcv_flood
        }
    }
    pub fn run(& mut self) {
        loop{
            select_biased! {
                 recv(self.receiver_msg) -> message =>{
                    if let Ok(message) = message {
                        self.handle_messages(message);
                    }
                }
                recv(self.rcv_flood) -> flood => {
                    if let Ok(_) = flood {
                        self.flooding();
                    }
                }
                recv(self.receiver_commands) -> command =>{
                    if let Ok(command) = command {
                        self.handle_commands(command);
                    }
                }
               
            }
        }
    }

    fn handle_commands(&mut self, command: ContentCommands) {
        match command {
            ContentCommands::GetTextList(id_server) => {
                self.get_list(id_server)
            },
            ContentCommands::GetMediaPosition(id_server, id_media) => {
                self.get_position(id_server, id_media)
            },
            ContentCommands::GetMedia(id_media_server, id_media) => {
                self.get_media(id_media_server, id_media)
            },
            ContentCommands::GetText(id_server, text_id) => {
                self.get_text(id_server, text_id);
            }
            ContentCommands::SearchTypeServers => {
                self.search_type_servers();
            }
            ContentCommands::SendTopologyGraph => {
                self.send_topology_graph();
            }
            ContentCommands::AddSender(node_id, sender) => {
                self.add_sender(node_id, sender);
            }
            ContentCommands::RemoveSender(node_id) => {
                self.remove_sender(node_id);
            }
            ContentCommands::PdrChanged(node_id) => {
                self.reset_data(node_id);
            }
        }
    }
    
    fn reset_data(&mut self, node_id: NodeId) {
        if let Some(data) = self.node_data.get_mut(&node_id) {
            data.reset(); //reset the data of the node id
        }
        
        let neighbors: Vec<NodeId> = self.topology_graph.neighbors(node_id).collect(); //get neighbors of the node
        for neighbor in neighbors {
            self.topology_graph.add_edge(node_id, neighbor, 1); 
        }
    }
    fn send_topology_graph(&self){
        self.send_event.send(WebBrowserEvents::Graph(self.config.id, self.topology_graph.clone())).unwrap();
    }
    
    fn add_sender(&mut self, node_id: NodeId, sender: Sender<Packet>) {
        if !self.send_packets.contains_key(&node_id) {
            self.send_packets.insert(node_id, sender);
        }else { 
            return;
        }
    }
    
    fn remove_sender(&mut self, node_id: NodeId) {
        if self.send_packets.contains_key(&node_id) {
            self.send_packets.remove(&node_id);
        }else { 
            return;
        }
    }

    fn handle_messages(& mut self, message: Packet){ //handle incoming messages from server
        match message.pack_type{
            PacketType::MsgFragment(_) => {self.handle_fragments(message)},
            PacketType::Ack(_) => {self.handle_acks(message)},
            PacketType::Nack(_) => {self.handle_nacks(message)},
            PacketType::FloodResponse(_) => {self.handle_flood_response(message)},
            PacketType::FloodRequest(_) => {self.handle_flood_request(message)},
        }
    }
    
    fn search_type_servers(& mut self) { //needed to find the different types of servers and send them to simulation control
        for server in self.servers.clone() {
            self.ask_type(server);
        }
    }

    fn ask_type(& mut self, id_server: NodeId) {
        if !self.servers.contains(&id_server) {
            return;
        }

        let request = WebBrowserCommands::GetServerType;
        
        let session_id = self.session_id_packet;
        self.session_id_packet += 1;
        
        let fragments = WebBrowserCommands::fragment_message(&request);
        self.fragments_sent.insert(session_id, fragments.clone());

        match self.find_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(), session_id); //create packet knowing what path they need to take
                self.packet_sent.insert(session_id, (id_server, packets_to_send.clone())); //saving packets with their session id and their destination,
                // useful if we need to resend a packet lost, in this way we can find the destination id (id_server) based on the session id of the packets
                
                for packet in packets_to_send {
                    if let PacketType::MsgFragment(fragment) = packet.pack_type.clone(){
                        if let Err(_) = self.send_event.send(WebBrowserEvents::InfoRequest(self.config.id, ContentRequest::AskTypes(fragment.total_n_fragments), packet.session_id )){
                            println!("web browser failed to notify SC about ask types request")
                        } //notify SC of the request
                    }
                    if let Some(next_hop) = route.get(1) {
                        if let Err(_) = self.send_messages(next_hop, packet){
                            self.ask_type(id_server);
                        }
                    } else { return; }
                }
            }
            Err(_) => { println!("No route found for the destination server") }
        }
    }

    fn get_list(& mut self, id_server: NodeId) {
        if !self.servers.contains(&id_server) { //check if the server was actually found
            return;
        }

        let request = WebBrowserCommands::GetList;

        let session_id = self.session_id_packet;
        self.session_id_packet += 1;

        let fragments = WebBrowserCommands::fragment_message(&request);
        self.fragments_sent.insert(session_id, fragments.clone());

        match self.find_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(), session_id);
                self.packet_sent.insert(session_id, (id_server, packets_to_send.clone()));
                
                for packet in packets_to_send {
                    if let PacketType::MsgFragment(fragment) = packet.pack_type.clone(){
                        if let Err(_) = self.send_event.send(WebBrowserEvents::InfoRequest(self.config.id, ContentRequest::GetList(fragment.total_n_fragments), packet.session_id )){
                            println!("web browser failed to notify SC about get list request")
                        }
                    }
                    if let Some(next_hop) = route.get(1) {
                        if let Err(_) = self.send_messages(next_hop, packet){
                            self.get_list(id_server);
                        }
                    } else { return; }
                }
            }
            Err(_) => { println!("No route found for the destination server") }
        }
    }

    fn get_position (& mut self, id_server: NodeId, media_id: MediaId){
        if !self.servers.contains(&id_server) {
            return;
        }

        let request = WebBrowserCommands::GetPosition(media_id.clone());
        let session_id = self.session_id_packet;
        self.session_id_packet += 1;

        let fragments = WebBrowserCommands::fragment_message(&request);
        self.fragments_sent.insert(session_id, fragments.clone());

        match self.find_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(), session_id);
                self.packet_sent.insert(session_id, (id_server, packets_to_send.clone()));
                
                for packet in packets_to_send {
                    if let PacketType::MsgFragment(fragment) = packet.pack_type.clone(){
                        if let Err(_) = self.send_event.send(WebBrowserEvents::InfoRequest(self.config.id, ContentRequest::GetPosition(fragment.total_n_fragments), packet.session_id )){
                            println!("web browser failed to notify SC about get position request")
                        }
                    }
                    if let Some(next_hop) = route.get(1) {
                        if let Err(_) = self.send_messages(next_hop, packet){
                            self.get_position(id_server, media_id.clone());
                        }
                    } else { return; }
                }
            }
            Err(_) => { println!("No route found for the destination server") }
        }
    }

    fn get_media(& mut self, id_media_server: NodeId, media_id: MediaId) {
        if !self.servers.contains(&id_media_server) {
            return;
        }
        let request = WebBrowserCommands::GetMedia(media_id.clone());
        
        let session_id = self.session_id_packet;
        self.session_id_packet += 1;

        let fragments = WebBrowserCommands::fragment_message(&request);
        self.fragments_sent.insert(session_id, fragments.clone());

        match self.find_route(&id_media_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(), session_id);
                self.packet_sent.insert(session_id, (id_media_server, packets_to_send.clone()));
                
                for packet in packets_to_send {
                    if let PacketType::MsgFragment(fragment) = packet.pack_type.clone(){
                        if let Err(_) = self.send_event.send(WebBrowserEvents::InfoRequest(self.config.id, ContentRequest::GetMedia(fragment.total_n_fragments), packet.session_id )){
                            println!("web browser failed to notify SC about get media request")
                        }
                    }
                    if let Some(next_hop) = route.get(1) {
                        if let Err(_) = self.send_messages(next_hop, packet){
                            self.get_media(id_media_server, media_id.clone());
                        }
                    } else { return; }
                }
            }
            Err(_) => { println!("No route found for the destination server") }
        }
    }

    fn get_text (& mut self, id_server: NodeId, text_id: String) {
        if !self.servers.contains(&id_server) {
            return;
        }
        let request = WebBrowserCommands::GetText(text_id.clone());
        let session_id = self.session_id_packet;
        self.session_id_packet += 1;

        let fragments = WebBrowserCommands::fragment_message(&request);
        self.fragments_sent.insert(session_id, fragments.clone());

        match self.find_route(&id_server) {
            Ok(route) => {
                let packets_to_send = ChatRequest::create_packet(&fragments, route.clone(), session_id);
                self.packet_sent.insert(session_id, (id_server, packets_to_send.clone()));
                
                for packet in packets_to_send {
                    if let PacketType::MsgFragment(fragment) = packet.pack_type.clone(){
                        if let Err(_) = self.send_event.send(WebBrowserEvents::InfoRequest(self.config.id, ContentRequest::GetText(fragment.total_n_fragments), packet.session_id )){
                            println!("web browser failed to notify SC about get text request")
                        }
                    }
                    if let Some(next_hop) = route.get(1) {
                        if let Err(_) = self.send_messages(next_hop, packet){
                            self.get_text(id_server, text_id.clone());
                        }
                    } else { return; }
                }
            }
            Err(_) => { println!("No route found for the destination server") }
        }

    }

    // incoming messages from servers
    fn handle_fragments(& mut self, packet: Packet){
        let src_id = packet.routing_header.hops.first().unwrap();
        let check = (packet.session_id, *src_id); //check to find all the fragments coming from the same source id (sender) with same session id

        let ack = self.create_ack(&packet);
        let prev = packet.routing_header.hops[packet.routing_header.hop_index-1];
        if let Err(_) = self.send_messages(&prev, ack){ //send an ack whenever a fragment was correctly received
            self.handle_fragments(packet.clone());
        }

        if let PacketType::MsgFragment(fragment) = packet.pack_type{
            if !self.incoming_fragments.contains_key(&check){ //if a fragment with the pair check was not yet received, save them as a new element in incoming_fragments
                self.incoming_fragments.insert(check, HashMap::new());
            }
            self.incoming_fragments
                .get_mut(&check)
                .unwrap()
                .insert(fragment.fragment_index, fragment.clone()); //if it was received, update the hashmap

            if let Some(fragments) = self.incoming_fragments.get(&check){
                if fragments.len() as u64 == fragment.total_n_fragments{
                    if let Ok(message) = TextServer::reassemble_msg(fragments) {
                        match message {
                            TextServer::ServerTypeText(server_type) => {
                                if server_type == ServerType::TextServer && !self.text_servers.contains(&src_id) {
                                    self.text_servers.push(src_id.clone());
                                }
                                
                                if let Err(_) = self.send_event.send(WebBrowserEvents::TextServers(self.config.id.clone(), self.text_servers.clone())) {
                                    println!("failed to send list of text servers to simulation control")
                                }
                            }

                            TextServer::SendFileList(list) => {
                                if let Err(_) = self.send_event.send(WebBrowserEvents::ListFiles(self.config.id.clone(), list.clone())) {
                                    println!("failed to send list of files to simulation control")
                                }
                            }

                            TextServer::PositionMedia(media_server_id) => {
                                if let Err(_) = self.send_event.send(WebBrowserEvents::MediaPosition(self.config.id.clone(), media_server_id.clone())) {
                                    println!("failed to send media position to simulation control")
                                }
                            }

                            TextServer::Text(text) => {
                                let path_folder = "assets/multimedia/SC".to_string();
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
                                if server_type == ServerType::MediaServer && !self.media_servers.contains(&src_id){
                                    self.media_servers.push(src_id.clone());
                                }

                                if let Err(_) = self.send_event.send(WebBrowserEvents::MediaServers(self.config.id.clone(), self.media_servers.clone())){
                                    println!("failed to send list of text servers to simulation control")
                                }
                            }

                            MediaServer::SendMedia(media) => {
                                let path_folder = "assets/multimedia/SC".to_string();
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

    fn handle_nacks(& mut self, packet: Packet){
        if let PacketType::Nack(nack) = packet.pack_type.clone(){
            match nack.nack_type{
                NackType::Dropped => {
                    let failing_node = packet.routing_header.hops[0];
                    let data = self.node_data.get_mut(&failing_node).unwrap(); //finding node in the graph to modify the struct data
                    data.dropped += 1; //used to compute reliability of the path
                    
                    self.resend_fragment(packet)
                }

                NackType::ErrorInRouting(id) => {
                    if !self.problematic_nodes.contains(&id){
                        self.problematic_nodes.push(id); //field to save nodes sending error in routing so that we can avoid them during the decision of the best path
                    }

                    let dest= id;
                    let src = packet.routing_header.hops[0];
                    self.topology_graph.remove_edge(src, dest); //remove the edge between the drones so that the graph can be updated
                    
                    self.resend_fragment(packet)
                }

                _ => {}
            }
        }
    }

    fn resend_fragment(& mut self, packet: Packet){
        if let PacketType::Nack(nack) = packet.clone().pack_type{
            
            let session_fragments = if let Some(session_fragments) = self.fragments_sent.get(&packet.session_id){
                session_fragments
            }else {
                return;
            }; //get fragments from the same session id
            
            let fragment_lost = if let Some(fragment_lost) = session_fragments.get(&nack.fragment_index){
                fragment_lost.clone()
            }else {
                return;
            }; //find fragment lost based on the fragment index

            let destination_id = match self.packet_sent.get(&packet.session_id){
                Some((destination_id, _)) => destination_id.clone(),
                None => {
                    return;
                }
            }; //find the destination of the lost packet

            match self.find_route(&destination_id){
                Ok(route) => {
                    
                    let packet_to_send = Packet::new_fragment(
                        SourceRoutingHeader::new(route.clone(), 0),
                        packet.session_id,
                        fragment_lost
                    ); //create a new packet with the new route, but same content and session id

                    if let Some(next_hop) = route.get(1){
                        if let Err(_) = self.send_messages(next_hop, packet_to_send){
                            self.resend_fragment(packet.clone());
                        }
                    }else { return; }
                }
                Err(_) => println!("failed to find the route after receiving nack")
            }

        }
    }

    fn handle_acks(& mut self, packet: Packet){
        if let PacketType::Ack(ack) = packet.pack_type{
            self.problematic_nodes.clear(); //if successful clear the problematic nodes

            self.fragments_sent.retain(|index, _| *index != ack.fragment_index); //this filters the hashmap, removing the ones with that index that were received
            let data = self.node_data.get_mut(&packet.routing_header.hops.iter().rev().nth(1).unwrap()).unwrap();
            data.forwarded += 1; //update the forwarded field to then compute reliability
        }
    }


    fn flooding(& mut self){
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

        for (_, sender) in &self.send_packets { //send the flood request to all the direct neighbors
            if let Err(_) = sender.send(flood_request.clone()) {
                println!("Error sending the flood request")
            }
        }
    }

    fn handle_flood_response(& mut self, packet: Packet){
        if let PacketType::FloodResponse(flood_response) = packet.clone().pack_type {
            if !flood_response.path_trace.is_empty(){

                let path = &flood_response.path_trace;
                for pair in path.windows(2) { //for each pair in the flood responses create an edge
                    let (src, _) = pair[0];
                    let (dst, _) = pair[1];

                    self.node_data.insert(src, NodeData::new()); //initialize "data" used for computation of reliability
                    self.node_data.insert(dst, NodeData::new());

                    self.topology_graph.add_edge(src.clone(), dst.clone(), 1); //initially all edges weight 1
                }
                
                let dest = if let Some(dest) = flood_response.path_trace.get(0){
                    dest.clone()
                }else {
                    return;
                };
                
                if dest != (self.config.id, NodeType::Client) { //if the flood response's destination is not the client it forwards it 
                    self.send_flooding_packet(packet);
                    return;
                }
                
                for (node_id, node_type) in &flood_response.path_trace{
                    if *node_type == NodeType::Server && !self.servers.contains(&node_id){
                        self.servers.push(*node_id);
                    }
                    if *node_type == NodeType::Client && !self.clients.contains(&node_id){
                        self.clients.push(*node_id);
                    }
                }
                self.flood.push(flood_response); //storing all the flood responses to then access the path traces and find the quickest one
            }

        }
    }

    fn handle_flood_request(& mut self, packet: Packet){
        if let PacketType::FloodRequest(mut flood_request) = packet.clone().pack_type {
            //check if the pair (flood_id, initiator id) has already been received -> self.visited_nodes
            if self.visited_nodes.contains(&(flood_request.flood_id, flood_request.initiator_id)){
                flood_request.path_trace.push((self.config.id.clone(), NodeType::Client)); //if flood request received, push client into path trace
                self.send_flooding_packet( flood_request.generate_response(packet.session_id) ); //and send back a flood response
            }else {
                flood_request.path_trace.push((self.config.id.clone(), NodeType::Client)); //push client into path trace
                self.visited_nodes.insert((flood_request.flood_id, flood_request.initiator_id)); //insert it into the visited nodes

                if self.send_packets.len() == 1{ //if the client has only one neighbor
                    self.send_flooding_packet( flood_request.generate_response(packet.session_id)); //send back a flood response
                }else {
                    //if more than one neighbor, create a new flood request and sends it to all the neighbors, expect the one sending the initial flood request
                    let new_packet = Packet::new_flood_request(packet.routing_header, packet.session_id, flood_request.clone()); //create the packet of the flood request that needs to be forwarded
                    
                    for (neighbour, sender) in &self.send_packets {
                        if let Some(sender_flood_req) = flood_request.path_trace.iter().rev().nth(1) { //previous sender
                            if *neighbour != sender_flood_req.0 {
                                sender.send(new_packet.clone()).unwrap()
                            }
                        }
                    }
                }
            }
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

    fn find_route(& mut self, destination_id: &NodeId)-> Result<Vec<NodeId>, String>{
        let source = self.config.id.clone();

        let result = dijkstra(&self.topology_graph, source.clone(), Some(destination_id.clone()), |edge|{
            let dest = edge.1;
            if self.problematic_nodes.contains(&dest) 
                || self.clients.contains(&dest)
                || (self.servers.contains(&dest) && &dest != destination_id){ //completely avoid clients, problematic nodes from error in routing and servers (not the destination one)
                1_000
            }else { 
                let reliability = self.node_data.get(&dest).map(|data| data.reliability()).unwrap_or(1.0); //gets the reliability to arrive to destination
                if reliability <= 0.0 {
                    1_000
                } else {
                    ((1.0 / reliability).ceil() as u32).min(1_000) //cap to 1000
                } 
            }
            //ceil returns integer closest to result
            //this transforms the reliability into a weight. when we have higher reliability, we need a smaller cost (1.0/reliability)
        });

        if let Some(_) = result.get(destination_id) {
            let mut path = vec![destination_id.clone()];
            let mut current = destination_id.clone();

            while current != source {
                let mut found = false;
                for edge in self.topology_graph.edges_directed(current.clone(), Direction::Incoming) { //go backwords in path to check incoming edges
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

                        if prev_cost + weight == curr_cost { //this condition ensures that the edge was used in algorithm, to see if the edge was the one used or not
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

    fn send_messages(& mut self, destination_id: &NodeId, mut packet: Packet)-> Result<(), ()>{
        packet.routing_header.hop_index+=1;
        if let Some(sender) = self.send_packets.get(&destination_id){
            if let Err(err) = sender.send(packet.clone()){
                println!("Error sending command: {}", err);
            }
            Ok(())
        }else {
            self.topology_graph.remove_edge(self.config.id, *destination_id); //if the sender was not found (remove direct neighbor) we erase the edge.
            Err(())
        } 
    }

    fn create_ack(& mut self, packet: &Packet)-> Packet{
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

        let decode = match BASE64.decode(fmd.content){ //decode the content of the file
            Ok(decode) => decode,
            Err(_) => return Err("Failed to decode the file".to_string()),
        };

        fs::write(&full_path, decode) //write the decoded file to the path (into SC folder)
            .map_err(|e| format!("Failed to save the file: {}", e))?;
        
        Ok(full_path)
    }


}


