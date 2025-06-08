use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::Debug;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::algo::{astar, dijkstra};
use crossbeam_channel::{select_biased, Receiver, Sender};
use petgraph::data::Build;
use petgraph::{Incoming, Outgoing};
use petgraph::prelude::EdgeRef;
use serde::Serialize;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet;
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType};
use crate::common_things::common::*;
use crate::servers::assembler::*;
use crate::simulation_control::simulation_control::MyNodeType;

#[derive(Serialize, Clone, Debug)]
pub struct drops{
    dropped : u64,
    forwarded : u64,
}


#[derive(Clone)]
pub struct Server{
    server_id: NodeId,
    server_type: ServerType,
    session_id: u64,
    registered_clients: Vec<NodeId>,
    flooding: Vec<FloodResponse>,
    neigh_map: Graph<(NodeId,NodeType), f64, petgraph::Directed>,
    stats: HashMap<NodeIndex, drops>,
    packet_recv: Receiver<Packet>,
    already_visited: HashSet<(NodeId,u64)>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    fragments_recv : HashMap<(NodeId,u64),Vec<Fragment>>,
    fragments_send : HashMap<u64,(NodeId,NodeType,Vec<Fragment>)>,
    rcv_flood: Receiver<BackGroundFlood>,
    rcv_command: Receiver<ServerCommands>,
    send_event: Sender<ServerEvent>
}

impl Server{
    pub fn new(id:NodeId, packet_recv: Receiver<Packet>, packet_send: HashMap<NodeId,Sender<Packet>>, rcv_flood: Receiver<BackGroundFlood>, rcv_command: Receiver<ServerCommands>, send_event: Sender<ServerEvent>)->Self{
        Self{
            server_id:id,
            server_type: ServerType::CommunicationServer,
            session_id:0,
            registered_clients: Vec::new(),
            flooding: Vec::new(),
            neigh_map: Graph::new(),
            stats: HashMap::new(),
            packet_recv:packet_recv,
            already_visited:HashSet::new(),
            packet_send:packet_send,
            fragments_recv : HashMap::new(),
            fragments_send : HashMap::new(),
            rcv_flood,
            rcv_command,
            send_event
        }
    }
    pub(crate) fn run(&mut self) {
        self.flooding();
        loop {
            select_biased!{
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        self.handle_packet(packet);
                    }
                },
                recv(self.rcv_flood) -> flood => {
                    if let Ok(_) = flood {
                        self.flooding();
                    }
                }
                recv(self.rcv_command) -> sc_command => {
                    if let Ok(command) = sc_command {
                        match command {
                            ServerCommands::SendTopologyGraph=>{
                                self.send_topology_graph();
                            }
                            ServerCommands::AddSender(id, sender)=>{
                                self.add_sender(id, sender);
                            },
                            ServerCommands::RemoveSender(id)=>{
                                self.remove_sender(id)
                            },
                            ServerCommands::TopologyChanged=>{
                                //todo
                            }
                        }
                    }
                }
            }
        }
    }
    fn send_topology_graph(&self){
        self.send_event.send(ServerEvent::Graph(self.server_id, self.neigh_map.clone())).unwrap();
    }
    fn add_sender(&mut self, node_id: NodeId, sender: Sender<Packet>){
        if !self.packet_send.contains_key(&node_id) {
            self.packet_send.insert(node_id, sender);
        }
    }
    
    fn remove_sender(&mut self, node_id: NodeId){
        self.packet_send.remove_entry(&node_id);
    }
    
    pub fn handle_packet(&mut self, p:Packet){
        match p.clone().pack_type {
            PacketType::MsgFragment(_) => {println!("received packet {p}");self.handle_msg_fragment(p)}
            PacketType::Ack(_) => {self.handle_ack(p)}
            PacketType::Nack(_) => {self.handle_nack(p)}
            PacketType::FloodRequest(_) => {self.handle_flood_request(p)}
            PacketType::FloodResponse(_) => {self.handle_flood_response(p)}
        }
    }

    fn forward_packet(&self, mut packet: Packet) {
        if packet.routing_header.hop_index < packet.routing_header.hops.len() -1 {
            packet.routing_header.hop_index += 1;
            let next_hop = packet.routing_header.hops[packet.routing_header.hop_index];
            if let Some(sender) = self.packet_send.get(&next_hop) {
                sender.send(packet.clone()).unwrap_or_default();
            }
        } else {
            println!("destination reached!!");
            return;
        }
    }


    fn send_packet(&mut self, p:ChatResponse, id:NodeId, nt:NodeType){
        // println!("flooding : {:?}", self.flooding); //fa vedere tutte le flood response salvaate nel server
        // println!("graph del chatserver {:?}: {:?}",self.server_id, self.neigh_map); //fa vedere il grafo (tutti i nodi e tutti gli edges)
        if let Some(srh) = self.best_path_custom_cost(id,nt){
            println!("srh : {:?}",srh);
            if let Ok(vec) = p.serialize_data(srh,self.session_id){
                let mut fragments_send = Vec::new();
                for i in vec.iter(){
                    if let PacketType::MsgFragment(fragment) = i.clone().pack_type{
                        fragments_send.push(fragment);
                    }
                    self.forward_packet(i.clone());
                }
                self.fragments_send.insert(self.session_id.clone(), (id,nt,fragments_send));
                match p {
                    ChatResponse::ServerTypeChat(_) => {self.send_event.send(ServerEvent::ChatPacketInfo(self.server_id, MyNodeType::ChatServer, ChatServerEvent::SendingServerTypeChat(vec.len() as u64),self.session_id)).unwrap();}
                    ChatResponse::RegisterClient(_) => {self.send_event.send(ServerEvent::ChatPacketInfo(self.server_id, MyNodeType::ChatServer, ChatServerEvent::ClientRegistration(vec.len() as u64),self.session_id)).unwrap();}
                    ChatResponse::RegisteredClients(_) => {self.send_event.send(ServerEvent::ChatPacketInfo(self.server_id, MyNodeType::ChatServer, ChatServerEvent::SendingClientList(vec.len() as u64),self.session_id)).unwrap();}
                    ChatResponse::SendMessage(_) => {self.send_event.send(ServerEvent::ChatPacketInfo(self.server_id, MyNodeType::ChatServer, ChatServerEvent::ForwardingMessage(vec.len() as u64),self.session_id)).unwrap();}
                    ChatResponse::EndChat(_) => {self.send_event.send(ServerEvent::ChatPacketInfo(self.server_id, MyNodeType::ChatServer, ChatServerEvent::ClientElimination(vec.len() as u64),self.session_id)).unwrap();}
                    ChatResponse::ForwardMessage(_) => {}
                }
                self.session_id+=1;
                //aggiungere un field nella struct server per salvare tutti i vari pacchetti nel caso in cui fossero droppati ecc.
            }
        }else {
            println!("sono il chatserver {:?} no route found for sending packet {:?} to {:?} {:?}!",self.server_id,p,nt,id);
        }
    }

    fn handle_msg_fragment(&mut self, p:Packet){
        self.forward_packet(create_ack(p.clone()));
        if let PacketType::MsgFragment(fragment) = p.pack_type{
            if self.fragments_recv.contains_key(&(p.routing_header.hops.clone()[0],p.session_id)){
                if let Some(vec) = self.fragments_recv.get_mut(&(p.routing_header.hops[0],p.session_id)){
                    vec.push(fragment.clone());
                }else {
                    println!("This else shouldn't be reached, it means that the server has no vec of fragments (received) associated to the key");
                }
            }else {
                let mut vec = Vec::new();
                vec.push(fragment.clone());
                self.fragments_recv.insert((p.routing_header.hops.clone()[0], p.session_id), vec);
            }
            if let Some(vec) = self.fragments_recv.get_mut(&(p.routing_header.hops[0],p.session_id)){
                if fragment.total_n_fragments == vec.len() as u64{
                    if let Ok(totalmsg) = ChatRequest::deserialize_data(vec){
                        match totalmsg{
                            ChatRequest::ServerType => {
                                println!("Server type request received from client: {:?}!", p.routing_header.hops.clone()[0]);
                                self.send_packet(ChatResponse::ServerTypeChat(self.clone().server_type), p.routing_header.hops[0], NodeType::Client);
                            }
                            ChatRequest::RegisterClient(n) => {
                                println!("Register client request received from client: {:?}!", p.routing_header.hops.clone()[0]);
                                self.registered_clients.push(n);
                                self.send_packet(ChatResponse::RegisterClient(true), p.routing_header.hops[0], NodeType::Client);
                            }
                            ChatRequest::GetListClients => {
                                println!("Get client list request received from client: {:?}!", p.routing_header.hops.clone()[0]);
                                self.send_packet(ChatResponse::RegisteredClients(self.clone().registered_clients), p.routing_header.hops[0], NodeType::Client);
                            }
                            ChatRequest::SendMessage(mc, _) => {
                                println!("Send message request received from client: {:?}!", p.routing_header.hops.clone()[0]);
                                println!("Registered clients: {:?}",self.registered_clients);
                                if self.registered_clients.contains(&mc.from_id) && self.registered_clients.contains(&mc.to_id){
                                    self.send_packet(ChatResponse::SendMessage(Ok("The server will forward the message to the final client".to_string())), p.routing_header.hops[0], NodeType::Client);
                                    self.send_packet(ChatResponse::ForwardMessage(mc.clone()), mc.to_id, NodeType::Client);
                                }else {
                                    self.send_packet(ChatResponse::SendMessage(Err("Error with the registration of the two involved clients".to_string())), p.routing_header.hops[0], NodeType::Client);
                                }
                            }
                            ChatRequest::EndChat(n) => {
                                println!("end chat request received from client: {:?}!", p.routing_header.hops.clone()[0]);
                                self.registered_clients.retain(|x| *x != n);
                                self.send_packet(ChatResponse::EndChat(true), p.routing_header.hops[0], NodeType::Client);
                            }
                        }
                    }else {
                        if let Ok(totalmsg) = TextServer::deserialize_data(vec) {
                            match totalmsg {
                                TextServer::ServerTypeReq => {
                                    println!("Server type request received from server: {:?}!", p.routing_header.hops.clone()[0]);
                                    self.send_packet(ChatResponse::ServerTypeChat(self.clone().server_type), p.routing_header.hops[0], NodeType::Server);
                                }
                                _ => { println!("I shouldn't receive these commands"); }
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_ack(&mut self, packet : Packet){
        let s_id=packet.session_id;
        if let PacketType::Ack(ack) = packet.pack_type{
            self.fragments_send.get_mut(&s_id).unwrap().2.retain(|x| x.fragment_index!=ack.fragment_index);
        }
        //forse da sistemare perchè c'è un unwrap, anche se in teoria gli ack che arrivano al mio server hanno un session id corretto
        for i in packet.routing_header.hops.iter().skip(1){
            let ni = self.find_node(*i, NodeType::Drone);
            match  ni{
                Some(n) => {
                    self.stats.get_mut(&n).unwrap().forwarded+=1;
                    let mut edges = Vec::new();
                    for edge_idx in self.neigh_map.edges_directed(n, Incoming).map(|e| e.id()){
                        edges.push(edge_idx);
                    }
                    for e in edges{
                        if let Some(weight) = self.neigh_map.edge_weight_mut(e) {
                            let drops = self.stats.get(&n).unwrap();
                            *weight =  drops.dropped as f64/(drops.forwarded+drops.dropped) as f64;
                        }
                    }
                }
                None => {}
            }
        }
    }

    fn packet_recover(&mut self, s_id: u64, lost_fragment_index: u64){
        if self.fragments_send.contains_key(&s_id){
            let info = self.fragments_send.get(&s_id).unwrap();
            for i in info.2.clone().iter(){
                if i.fragment_index==lost_fragment_index{
                    if let Some(srh) = self.best_path_custom_cost(info.0.clone(),info.1.clone()){
                        let pack = Packet{
                            routing_header: srh,
                            session_id: s_id.clone(),
                            pack_type: PacketType::MsgFragment(i.clone()),
                        };
                        self.forward_packet(pack);
                        break;
                    }else {
                        println!("there isn't a route to reach the packet destination (shouldn't happen)");
                    }
                }
            }
        }else {
            println!("This else shouldn't be reached, it means that the server has no vec of fragments (sent) associated to the key");
        }
    }


    fn handle_nack(&mut self, packet : Packet){
        let id = packet.routing_header.hops[0];
        let s_id=packet.session_id;
        if let PacketType::Nack(nack) = packet.pack_type{
            match nack.clone().nack_type{
                NackType::ErrorInRouting(crashed_id) => {
                    println!("sono il chat {:?} ho ricevuto un errorinrouting with route {:?}, the drone that crashed is {:?}", self.server_id, packet.routing_header.hops, crashed_id.clone());
                    let mut node1;
                    let mut node2;
                    if self.node_exists(crashed_id, NodeType::Drone){
                        node1 = self.find_node(crashed_id, NodeType::Drone).unwrap_or_default();
                    } else if  self.node_exists(crashed_id, NodeType::Client){ 
                        node1 = self.find_node(crashed_id, NodeType::Client).unwrap_or_default();
                    } else { node1 = self.find_node(crashed_id, NodeType::Server).unwrap_or_default(); }
                    if self.node_exists(id, NodeType::Drone){
                        node2 = self.find_node(id, NodeType::Drone).unwrap_or_default();
                    }else if   self.node_exists(id, NodeType::Client){
                        node2 = self.find_node(id, NodeType::Client).unwrap_or_default();
                    } else { node2 = self.find_node(id, NodeType::Server).unwrap_or_default() }
                    
                    let edge = self.neigh_map.find_edge(node2, node1).unwrap_or_default();
                    // println!("edge that failed {:?}", edge);
                    self.neigh_map.remove_edge(edge);
                    // println!("graph del chat dopo aver tolto gli edges del drone crashato {:?}", self.neigh_map);
                    self.packet_recover(s_id, nack.fragment_index);
                }
                NackType::DestinationIsDrone => {
                    println!("This error shouldn't happen!");
                    self.packet_recover(s_id, nack.fragment_index);
                }
                NackType::Dropped => {
                        //non so se il clone di self vada bene, l'ho messo solo perchè dava errore
                        // for neighbor in self.clone().neigh_map.neighbors(node) {
                        //     if let Some(edge) = self.neigh_map.find_edge(node, neighbor) {
                        //         self.neigh_map[edge] += 1.0;
                        //     }
                        // }
                        let mut first = true;
                        for i in packet.routing_header.hops.iter(){
                            let ni = self.find_node(*i, NodeType::Drone);
                            match  ni{
                                Some(n) => {
                                    if first == true{
                                        self.stats.get_mut(&n).unwrap().dropped+=1;
                                        first = false;
                                    }else {
                                        self.stats.get_mut(&n).unwrap().forwarded+=1;
                                    }
                                    let mut edges = Vec::new();
                                    for edge_idx in self.neigh_map.edges_directed(n, Incoming).map(|e| e.id()){
                                        edges.push(edge_idx);
                                    }
                                    for e in edges{
                                        if let Some(weight) = self.neigh_map.edge_weight_mut(e) {
                                            let drops = self.stats.get(&n).unwrap();
                                            *weight =  drops.dropped as f64/(drops.forwarded+drops.dropped) as f64;
                                        }
                                    }
                                    //println!("graph del chatserver {:?}: {:?}",self.server_id, self.neigh_map);
                                    //println!("Drone {:?} stats {:?}",i,self.stats.get(&n).unwrap());
                                }
                                None => {}
                            }
                        }
                    self.packet_recover(s_id, nack.fragment_index);
                }
                NackType::UnexpectedRecipient(_) => {
                    println!("This error shouldn't happen!");
                    self.packet_recover(s_id, nack.fragment_index);
                }
            }
        }
    }

    fn handle_flood_request(&mut self, packet : Packet){
        if let PacketType::FloodRequest(mut flood) = packet.pack_type{
            if self.already_visited.contains(&(flood.initiator_id, flood.flood_id)){
                self.forward_packet(self.create_flood_response(packet.session_id,flood));
                return;
            }else {
                self.already_visited.insert((flood.initiator_id, flood.flood_id));
                if self.packet_send.len()==1{
                    self.forward_packet(self.create_flood_response(packet.session_id,flood));
                }else {
                    flood.path_trace.push((self.server_id, NodeType::Server));
                    let new_packet = Packet{
                        pack_type : PacketType::FloodRequest(flood.clone()),
                        routing_header: packet.routing_header,
                        session_id: packet.session_id,
                    };
                    let (previous, _) = flood.path_trace[flood.path_trace.len() - 2];
                    for (idd, neighbour) in self.packet_send.clone() {
                        if idd == previous {
                        } else {
                            // println!("i am the chat server {:?}, i am forwarding the floodrequest to {:?}, flood request: {:?}",self.server_id,idd, new_packet);
                            neighbour.send(new_packet.clone()).unwrap();
                        }
                    }
                }
            }
        }


    }
    fn create_flood_response(&self, s_id: u64, mut flood : FloodRequest )->Packet{
        let mut src_header=Vec::new();
        flood.path_trace.push((self.server_id, NodeType::Server));
        for (id,_) in flood.path_trace.clone(){
            src_header.push(id);
        }
        let reversed_src_header=reverse_vector(&src_header);
        let fr = Packet{
            pack_type: PacketType::FloodResponse(FloodResponse{flood_id:flood.flood_id.clone(), path_trace:flood.path_trace.clone()}),
            routing_header: SourceRoutingHeader{
                hops: reversed_src_header,
                hop_index: 0,
            },
            session_id: s_id,
        };
        // println!("i am the chatserver {:?} the flood response generated is: {:?}", self.server_id,fr);
        fr
    }

    fn handle_flood_response(&mut self, p:Packet){
        //println!("chat server flood response: {}", p.pack_type);
        if let PacketType::FloodResponse(mut flood) = p.clone().pack_type{
            // println!("server {} has received flood response {}", self.server_id,flood.clone());
            if flood.path_trace[0].0 == self.server_id {
                let mut safetoadd = true;
                for i in self.flooding.iter() {
                    if i.flood_id < flood.flood_id {
                        println!("the server is starting to receive new flood responses");
                        self.flooding.clear();
                        break;
                    } else if i.flood_id == flood.flood_id {} else {
                        safetoadd = false;
                        break;
                    }
                }
                if safetoadd {
                    self.flooding.push(flood.clone());

                    let mut prev;
                    match self.find_node(self.server_id, NodeType::Server) {
                        None => { prev = self.neigh_map.add_node((self.server_id, NodeType::Server)) }
                        Some(ni) => { prev = ni }
                    }

                    for &(j, k) in flood.path_trace.iter().skip(1) {
                        if let Some(&(prev_id, prev_nt)) = self.neigh_map.node_weight(prev) {
                            // println!("trying to connect {:?} to {:?}", j, prev_id);
                            if self.node_exists(j.clone(), k.clone()) {
                                let next = self.find_node(j.clone(), k.clone()).unwrap();
                                // println!("trying to connect {:?} to {:?}", prev, next);
                                if self.neigh_map.find_edge(prev, next).is_none() {
                                    self.neigh_map.add_edge(prev, next, 0.0);
                                }
                                if self.neigh_map.find_edge(next, prev).is_none() {
                                    self.neigh_map.add_edge(next, prev, 0.0);
                                }

                                prev = next;
                            } else {
                                let newnodeid = self.neigh_map.add_node((j.clone(), k.clone()));
                                self.stats.insert(newnodeid.clone(), drops { dropped: 0, forwarded: 1 });
                                if self.neigh_map.find_edge(prev, newnodeid).is_none() {
                                    self.neigh_map.add_edge(prev, newnodeid, 0.0);
                                }
                                if self.neigh_map.find_edge(newnodeid, prev).is_none() {
                                    self.neigh_map.add_edge(newnodeid, prev, 0.0);
                                }
                                prev = newnodeid;
                            }
                        }
                    }
                    //println!("graph del chatserver {:?}, {:?}", self.server_id, self.neigh_map);
                } else {
                    println!("you received an outdated version of the flooding");
                }
            }else { 
                println!("forwarding the flood because it is not mine");
                self.forward_packet(p);
            }
        }
    }


    pub(crate) fn flooding(&mut self){
        println!("server {} is starting a flooding",self.server_id);
        let mut flood_id = 0;
        for i in self.flooding.iter(){
            flood_id = i.flood_id+1;
        }
        let flood = packet::Packet{
            routing_header: SourceRoutingHeader::empty_route(),
            session_id: flood_id,
            pack_type: PacketType::FloodRequest(FloodRequest{
                flood_id,
                initiator_id: self.server_id,
                path_trace: vec![(self.server_id, NodeType::Server)],
            }),
        };
        for (id,neighbour) in self.packet_send.clone(){
            if let Err(_)=neighbour.send(flood.clone()){
                println!("error flood request");
            };
        }
    }


    //funzione commentata perchè non ha senso, ho trasferito quello che facevo qui all'interno di handle_flood_request, perchè altrimenti il self.flooding risultava sempre vuoto
    // fn neigh_mapping(&mut self){
    //     let mut prev = self.neigh_map.add_node((self.server_id, NodeType::Server));
    //     for i in self.flooding.iter(){
    //         for (j,k) in i.path_trace.iter(){
    //             if Self::node_exists(self.clone().neigh_map, j.clone(), k.clone()){
    //                 let nodeid = self.find_node(j.clone(),k.clone()).unwrap();
    //                 self.neigh_map.add_edge(nodeid, prev, 1);
    //                 prev = nodeid;
    //             } else{
    //                 let newnodeid = self.neigh_map.add_node((j.clone(),k.clone()));
    //                 self.neigh_map.add_edge(newnodeid, prev, 1);
    //                 prev = newnodeid;
    //             }
    //         }
    //     }
    // }

    fn node_exists(&self, id:NodeId, nt:NodeType) -> bool {
        self.neigh_map.node_indices().any(|i| self.neigh_map[i] == (id,nt))
    }

    fn find_node(&self, id: NodeId, nt: NodeType) -> Option<NodeIndex> {
        self.neigh_map.node_indices().find(|&i| self.neigh_map[i] == (id, nt))
    }


    //classico get_route con djikstra

    // fn get_route(&mut self, id:NodeId, nt:NodeType)->Option<SourceRoutingHeader>{
    //     let start = self.find_node(self.server_id, NodeType::Server).unwrap_or_default();
    //     let end = self.find_node(id,nt).unwrap_or_default();
    //
    //     let paths: HashMap<NodeIndex, u32> = dijkstra(&self.neigh_map, start, Some(end), |e| *e.weight());
    //
    //     if !paths.contains_key(&end) {
    //         return None;
    //     }
    //
    //     // Manual backtracking to reconstruct the shortest path
    //     let mut path = vec![id];
    //     let mut current = end;
    //
    //     while current != start {
    //         let mut found = false;
    //         for neighbor in self.neigh_map.neighbors(current) {
    //             if let Some(weight) = self.neigh_map.find_edge(neighbor, current).map(|e| self.neigh_map[e]) {
    //                 if let Some(&neighbor_dist) = paths.get(&neighbor) {
    //                         if neighbor_dist + weight == paths[&current] {
    //                             path.push(self.neigh_map[neighbor].0);
    //                             current = neighbor;
    //                             found = true;
    //                             break;
    //                         }
    //                 }
    //             }
    //         }
    //         if !found {
    //             return None; // Shouldn't happen unless the graph is modified during traversal
    //         }
    //     }
    //     path.reverse();
    //     Some(SourceRoutingHeader{hops: path, hop_index:0})
    // }


    //altro metodo che permette di trovare la route più veloce basandosi sul costo dei collegamenti, ma senza bisogno di fare hashmap e robe varie come tocca fare per dijkstra
    // fn get_route(&self, id:NodeId, nt:NodeType) -> Option<(SourceRoutingHeader)> {
    //     // Return Vec<NodeId> instead of Vec<NodeIndex>
    //     let start = self.find_node(self.server_id, NodeType::Server)?;
    //     let end = self.find_node(id,nt)?;
    //     let path = astar(
    //         &self.neigh_map,
    //         start,
    //         |finish| finish == end, // Stop when reaching the end node
    //         |e| *e.weight(),        // Edge cost
    //         |_| 0.0                   // No heuristic (Dijkstra behavior)
    //     ).map(|(cost, path)| (cost, path.into_iter().map(|idx| self.neigh_map[idx].0).collect())); // Convert NodeIndex -> NodeId
    // Some(SourceRoutingHeader{hops:path?.1, hop_index:0})
    // }


    // Main function to calculate best path with non-linear cost
    fn best_path_custom_cost(&self, id: NodeId, nt: NodeType) -> Option<SourceRoutingHeader> {
        let start = self.find_node(self.server_id, NodeType::Server)?;
        let end = self.find_node(id,nt)?;
        let mut heap = BinaryHeap::new();
        let mut best_cost: HashMap<NodeIndex, f64> = HashMap::new();
        let mut predecessors: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        heap.push((PathCost(0.0), start, vec![start]));
        best_cost.insert(start, 0.0);

        while let Some((PathCost(cost), node, path)) = heap.pop() {
            if node == end {
                let hops = path.into_iter().map(|idx| self.neigh_map[idx].0).collect();
                let src =  SourceRoutingHeader{
                    hops:hops,
                    hop_index:0,
                };
                return Some(src);
            }

            for edge in self.neigh_map.edges(node) {
                let neighbor = edge.target();

                if neighbor != end && neighbor != start {
                    let (_, node_type) = self.neigh_map[neighbor];
                    if node_type == NodeType::Client || node_type == NodeType::Server {
                        continue;
                    }
                }
                
                let edge_cost = edge.weight();

                let new_cost = 1.0 - (1.0 - cost) * (1.0 - edge_cost);

                if best_cost.get(&neighbor).map_or(true, |&c| new_cost < c) {
                    best_cost.insert(neighbor, new_cost);
                    let mut new_path = path.clone();
                    new_path.push(neighbor);
                    predecessors.insert(neighbor, node);
                    heap.push((PathCost(new_cost), neighbor, new_path));
                }
            }
        }

        None
    }

    // fn create_ack(&mut self, packet: Packet) ->Packet{
    //     let ack = Ack{
    //         fragment_index: if let PacketType::MsgFragment(fragment)=packet.pack_type{
    //             fragment.fragment_index
    //         }else {
    //             0
    //         },
    //     };
    //     let pack = Packet {
    //         pack_type: PacketType::Ack(ack.clone()),
    //         routing_header: self.get_route(packet.routing_header.hops[0], NodeType::Client).unwrap_or_default(),
    //         session_id: packet.session_id,
    //     };
    //     pack
    // }
}


fn reverse_vector<T: Clone>(input: &[T]) -> Vec<T> {
    let mut reversed: Vec<T> = input.to_vec();
    reversed.reverse();
    reversed
}
fn create_ack(packet: Packet)->Packet{
    let mut vec = Vec::new();
    for node_id in (0..=packet.routing_header.hop_index).rev() {
        vec.push(packet.routing_header.hops[node_id]);
    }
    let ack = Ack{
        fragment_index: if let PacketType::MsgFragment(fragment)=packet.pack_type{
            fragment.fragment_index
        }else {
            0
        },
    };
    let pack = Packet {
        pack_type: PacketType::Ack(ack.clone()),
        routing_header: SourceRoutingHeader {
            hop_index: 0,
            hops: vec.clone(),
        },
        session_id: packet.session_id,
    };
    pack
}

//stesso problema della funzione sopra, conviene usare l'algoritmo di routing per sicurezza
// fn create_nack(packet: Packet,nack_type: NackType)->Packet{
//     let mut vec = Vec::new();
//     for node_id in (0..=packet.routing_header.hop_index).rev() {
//         vec.push(packet.routing_header.hops[node_id]);
//     }
//     let nack = Nack {
//         fragment_index: if let PacketType::MsgFragment(fragment) = packet.pack_type {
//             fragment.fragment_index
//         } else {
//             0
//         },
//         nack_type,
//     };
//     let pack = Packet {
//         pack_type: PacketType::Nack(nack.clone()),
//         routing_header: SourceRoutingHeader {
//             hop_index: 0,
//             hops: vec.clone(),
//         },
//         session_id: packet.session_id,
//     };
//     pack
// }


#[derive(Debug,PartialEq,Clone,Copy)]
pub struct PathCost(f64);

impl Eq for PathCost {}

impl PartialOrd for PathCost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse order because BinaryHeap is a max-heap and we want the smallest cost
        other.0.partial_cmp(&self.0)
    }
}

impl Ord for PathCost {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}