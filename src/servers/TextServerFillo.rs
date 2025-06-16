use std::cmp::Ordering;
use base64::engine::general_purpose::STANDARD as BASE64;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::Debug;
use std::fs;
use petgraph::graph::{Graph, NodeIndex};
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use base64::Engine;
use crossbeam_channel::{select_biased, Receiver, Sender};
use petgraph::data::Build;
use petgraph::Incoming;
use petgraph::prelude::EdgeRef;
use serde::Serialize;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet;
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, NackType, NodeType, Packet, PacketType};
use crate::common_things::common::*;
use crate::servers::assembler::*;
use crate::gui::login_window::NodeType as MyNodeType;

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
    flood_id: u64,
    paths: HashMap<String,String>,
    texts_ids: Vec<TextId>,
    flooding: Vec<FloodResponse>,
    neigh_map: Graph<(NodeId,NodeType), f64, petgraph::Directed>,
    stats: HashMap<NodeIndex, drops>,
    media_servers: Vec<NodeId>,
    media_info: HashMap<NodeId, Vec<MediaId>>,
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
    pub fn new(id:NodeId, packet_recv: Receiver<Packet>, packet_send: HashMap<NodeId,Sender<Packet>>, rcv_flood: Receiver<BackGroundFlood>,  rcv_command: Receiver<ServerCommands>, send_event: Sender<ServerEvent>, file_path:&str)->Self{
        let path = Path::new(file_path);
        let file = File::open(path).unwrap();
        let reader = io::BufReader::new(file);

        let mut all_paths = HashMap::new();
        let mut texts_ids = Vec::new();

        for line in reader.lines() {
            if let Ok(line) = line {
                let parts: Vec<&str> = line.split('/').collect();
                if let Some(last) = parts.last() {
                    texts_ids.push(last.clone().to_string());
                    all_paths.insert(last.to_string(), line.clone().to_string());
                }

            }
        }

        Self{
            server_id:id,
            server_type: ServerType::TextServer,
            session_id:0,
            flood_id: 0,
            paths:all_paths,
            texts_ids:texts_ids,
            flooding: Vec::new(),
            neigh_map: Graph::new(),
            stats: HashMap::new(),
            media_servers: Vec::new(),
            media_info: HashMap::new(),
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
                },
                 recv(self.rcv_command) -> sc_command => {
                    if let Ok(command) = sc_command {
                        match command {
                            ServerCommands::SendTopologyGraph=>{
                                self.send_topology_graph();
                            },
                            ServerCommands::AddSender(id, sender)=>{
                                self.add_sender(id, sender);
                            },
                            ServerCommands::RemoveSender(id)=>{
                                self.remove_sender(id);
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
            let nodeserver = self.find_node(self.server_id, NodeType::Server);
            let node = self.find_node(node_id, NodeType::Drone);
            if node.is_some() && nodeserver.is_some(){
                self.neigh_map.add_edge(nodeserver.unwrap(), node.unwrap(), 0.0);
                self.neigh_map.add_edge(node.unwrap(), nodeserver.unwrap(), 0.0);
            }else {
                println!("Node {} not found, this shouldn't happen", node_id);
            }
        }
    }

    fn remove_sender(&mut self, node_id: NodeId){
        if self.packet_send.contains_key(&node_id){
            self.packet_send.remove_entry(&node_id);
            let nodeserver = self.find_node(self.server_id, NodeType::Server);
            let node = self.find_node(node_id, NodeType::Drone);
            if node.is_some() && nodeserver.is_some() {
                let edge1 = self.neigh_map.find_edge(nodeserver.unwrap(), node.unwrap());
                self.neigh_map.remove_edge(edge1.unwrap());
                let edge2 = self.neigh_map.find_edge(node.unwrap(), nodeserver.unwrap());
                self.neigh_map.remove_edge(edge2.unwrap());
            } else {
                println!("Node {} not found, this shouldn't happen", node_id);
            }
        }
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


    //attualmente il server appena riceve un servertype::media manda direttamente il comando pathresolution
    //quindi questa funzione è sostanzialmente inutile
    //potrebbe servire nell'eventualità che venga fatto spawnare un nuovo server
    fn get_media_list(&mut self){
        for i in self.media_servers.clone(){
            self.send_packet(TextServer::PathResolution,i,NodeType::Server);
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


    fn send_packet(&mut self, p:TextServer, id:NodeId, nt:NodeType){
        // println!("flooding : {:?}", self.flooding); //fa vedere tutte le flood response salvaate nel server
        // println!("graph del textserver {:?}: {:?}",self.server_id ,self.neigh_map); //fa vedere il grafo (tutti i nodi e tutti gli edges)
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
                    TextServer::ServerTypeReq => {self.send_event.send(ServerEvent::TextPacketInfo(self.server_id, MyNodeType::TextServer, TextServerEvent::SendingServerTypeReq(vec.len() as u64),self.session_id)).unwrap();}
                    TextServer::ServerTypeText(_) => {self.send_event.send(ServerEvent::TextPacketInfo(self.server_id, MyNodeType::TextServer, TextServerEvent::SendingServerTypeText(vec.len() as u64),self.session_id)).unwrap();}
                    TextServer::PathResolution => {self.send_event.send(ServerEvent::TextPacketInfo(self.server_id, MyNodeType::TextServer, TextServerEvent::AskingForPathRes(vec.len() as u64),self.session_id)).unwrap();}
                    TextServer::SendFileList(_) => {self.send_event.send(ServerEvent::TextPacketInfo(self.server_id, MyNodeType::TextServer, TextServerEvent::SendingFileList(vec.len() as u64),self.session_id)).unwrap();}
                    TextServer::PositionMedia(_) => {self.send_event.send(ServerEvent::TextPacketInfo(self.server_id, MyNodeType::TextServer, TextServerEvent::SendingPosition(vec.len() as u64),self.session_id)).unwrap();}
                    TextServer::Text(_) => {}
                }
                self.session_id+=1;
                //aggiungere un field nella struct server per salvare tutti i vari pacchetti nel caso in cui fossero droppati ecc.
            }
        }else {
            println!("sono il textserver {:?} no route found for sending packet {:?} to {:?} {:?}!",self.server_id,p,nt,id);
        }
    }

    fn send_text(&mut self, path:&str, id:NodeId, nt:NodeType){
        let pos = path.rfind('.').unwrap();
        let posofslash = path.rfind('/').unwrap();
        let mut filebytes = "".to_string();
        match fs::read(Path::new(path)){
            Ok(fb) => {filebytes = BASE64.encode(&fb);},
            Err(_) => {println!("could not read file");}
        }
        let fmd = FileMetaData{
            title: path[posofslash+1..pos].to_string(),
            extension:path[pos+1..].to_string(),
            content: filebytes,
        };
        if let Some(srh)=self.best_path_custom_cost(id,nt){
            if let Ok(vec) = TextServer::Text(fmd).serialize_data(srh.clone(),self.session_id){
                let mut fragments_send = Vec::new();
                for i in vec.iter(){
                    if let PacketType::MsgFragment(fragment) = i.clone().pack_type{
                        fragments_send.push(fragment);
                    }
                    self.forward_packet(i.clone());
                }
                self.fragments_send.insert(self.session_id.clone(), (id,nt,fragments_send));
                self.send_event.send(ServerEvent::TextPacketInfo(self.server_id, MyNodeType::TextServer, TextServerEvent::SendingText(vec.len() as u64),self.session_id)).unwrap();
                self.session_id+=1;
            }
        }else { 
            println!("Sono il textserver {:?} no route found for sending the text to {:?} {:?}!",self.server_id,nt,path);
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
                    if let Ok(totalmsg) = WebBrowserCommands::deserialize_data(vec){
                        match totalmsg{
                            WebBrowserCommands::GetList => {
                                let mut total_list = Vec::new();
                                for (_,i) in self.media_info.clone(){
                                    for j in i{
                                        total_list.push(j);
                                    }
                                }
                                for i in self.texts_ids.clone(){
                                    total_list.push(i);
                                }
                                self.send_packet(TextServer::SendFileList(total_list),p.routing_header.hops[0],NodeType::Client);
                            }
                            WebBrowserCommands::GetPosition(media_id) => {
                                for i in self.media_info.clone(){
                                    if i.1.contains(&media_id){
                                        println!("il media si trova qui {:?}", i.0);
                                        self.send_packet(TextServer::PositionMedia(i.0),p.routing_header.hops[0],NodeType::Client);
                                    }                                 
                                }
                            }
                            WebBrowserCommands::GetMedia(_) => {println!("I shouldn't receive this command");}
                            WebBrowserCommands::GetText(text_id) => {
                                if self.paths.contains_key(&text_id){
                                    let path = self.paths.get(&text_id).unwrap().clone();
                                    self.send_text(path.as_str(),p.routing_header.hops[0],NodeType::Client);
                                }
                            }
                            WebBrowserCommands::GetServerType => {
                                self.send_packet(TextServer::ServerTypeText(self.clone().server_type), p.routing_header.hops[0], NodeType::Client);
                            }
                        }
                    }else {
                        if let Ok(totalmsg) = ChatResponse::deserialize_data(vec) {
                            match totalmsg {
                                ChatResponse::ServerTypeChat(st) => {
                                    //println!("sono il text {:?} e ho scoperto che {:?} è un chat per sicurezza {:?}",self.server_id,p.routing_header.hops[0], st)
                                }
                                _ => { println!("I shouldn't receive these commands"); }
                            }
                        }else {
                            if let Ok(totalmsg) = MediaServer::deserialize_data(vec){
                                match totalmsg {
                                    MediaServer::ServerTypeMedia(_) => {
                                        //println!("sono il text {:?} e ho scoperto che {:?} è un media",self.server_id,p.routing_header.hops[0]);
                                        self.media_servers.push(p.routing_header.hops[0]);
                                        self.send_packet(TextServer::PathResolution,p.routing_header.hops[0],NodeType::Server);
                                    }
                                    MediaServer::SendPath(path) => {
                                        //println!("sono il text {:?} e ho ricevuto la path res di {:?}",self.server_id,p.routing_header.hops[0]);
                                        self.media_info.insert(p.routing_header.hops[0], path);
                                    }
                                    MediaServer::SendMedia(_) => {println!("I shouldn't receive this command");}
                                }
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
            //println!("i have to recover the packet because something wrong happened, resending to {:?} {:?} the number of packet i have to send is {:?}", info.1, info.0, info.2.clone().len());
            //println!("graph del text prima di fare la recovery {:?}", self.neigh_map);
            for i in info.2.clone().iter(){
                if i.fragment_index==lost_fragment_index{
                    if let Some(srh) = self.best_path_custom_cost(info.0.clone(),info.1.clone()){
                        //println!("new route {:?}",srh.clone());
                        let pack = Packet{
                            routing_header: srh,
                            session_id: s_id.clone(),
                            pack_type: PacketType::MsgFragment(i.clone()),
                        };
                        self.forward_packet(pack);
                        break;
                    }else {
                        println!("i am the text {:?} there isn't a route to reach the packet destination (shouldn't happen)",self.server_id);
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
                    // println!("sono il text {:?} ho ricevuto un errorinrouting with route {:?}, the drone that crashed is {:?}", self.server_id, packet.routing_header.hops, crashed_id.clone());
                    let node1;
                    let node2;
                    if self.node_exists(crashed_id, NodeType::Drone){
                        node1 = self.find_node(crashed_id, NodeType::Drone).unwrap_or_default();
                    } else if  self.node_exists(crashed_id, NodeType::Client){
                        node1 = self.find_node(crashed_id, NodeType::Client).unwrap_or_default();
                    } else { node1 = self.find_node(crashed_id, NodeType::Server).unwrap_or_default(); }
                    if self.node_exists(id, NodeType::Drone){
                        node2 = self.find_node(id, NodeType::Drone).unwrap_or_default();
                    }else if self.node_exists(id, NodeType::Client){
                        node2 = self.find_node(id, NodeType::Client).unwrap_or_default();
                    } else { node2 = self.find_node(id, NodeType::Server).unwrap_or_default() }
                    // println!("i nodi problematici sono {:?} e {:?}", node1,node2);

                    let edge1 = self.neigh_map.find_edge(node2, node1);
                    // println!("edge that failed {:?}", edge1);
                    if edge1.is_some(){
                        self.neigh_map.remove_edge(edge1.unwrap());
                    }
                    let edge2 = self.neigh_map.find_edge(node1, node2);
                    // println!("edge that failed {:?}", edge2);
                    if edge2.is_some(){
                        self.neigh_map.remove_edge(edge2.unwrap());
                    }
                    // println!("graph del text dopo aver tolto gli edges del drone crashato {:?}", self.neigh_map);
                    self.packet_recover(s_id, nack.fragment_index);
                }
                NackType::DestinationIsDrone => {
                    println!("This error shouldn't happen!");
                    self.packet_recover(s_id, nack.fragment_index);
                }
                NackType::Dropped => {
                    // if let Some(node) = self.find_node(id, NodeType::Drone) {
                    //     //non so se il clone di self vada bene, l'ho messo solo perchè dava errore
                    //     for neighbor in self.clone().neigh_map.neighbors(node) {
                    //         if let Some(edge) = self.neigh_map.find_edge(node, neighbor) {
                    //             self.neigh_map[edge] += 1;
                    //         }
                    //     }
                    // }else {
                    //     println!("node not found (shouldn't happen)");
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
                                //println!("graph del textserver {:?}: {:?}",self.server_id, self.neigh_map);
                                //println!("Drone {:?} stats {:?}",i,self.stats.get(&n).unwrap());
                            }
                            None => {}
                        }
                    }
                   // println!("calling packet_recover because a drone dropped");
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
        fr
    }

    fn handle_flood_response(&mut self, p:Packet){
        //println!("media server flood response: {}", p.pack_type);
        if let PacketType::FloodResponse(flood) = p.clone().pack_type{
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
                            if self.node_exists(j.clone(), k.clone()) {
                                let next = self.find_node(j.clone(), k.clone()).unwrap();
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
                        match k.clone() {
                            NodeType::Server => {
                                // println!("io sono il text {:?} sto chiedendo servertype a {:?}", self.server_id,j.clone());
                                self.send_packet(TextServer::ServerTypeReq, j.clone(), k.clone());
                            },
                            _ => {}
                        }
                    }
                }else {
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
        let flood = packet::Packet{
            routing_header: SourceRoutingHeader::empty_route(),
            session_id: self.flood_id,
            pack_type: PacketType::FloodRequest(FloodRequest{
                flood_id: self.flood_id,
                initiator_id: self.server_id,
                path_trace: vec![(self.server_id, NodeType::Server)],
            }),
        };
        self.flood_id+=1;
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
    //     let end = self.find_node(id, nt)?;
    // 
    //     let path = astar(
    //         &self.neigh_map,
    //         start,
    //         |finish| finish == end,
    //         |e| {
    //             let target = e.target();
    //             let (_, node_type) = self.neigh_map[target];
    //             // Allow only Routers as intermediate nodes
    //             if target != start && target != end {
    //                 match node_type {
    //                     NodeType::Client | NodeType::Server => return f64::INFINITY, // Block path
    //                     _ => {}
    //                 }
    //             }
    //             *e.weight() as f64
    //         },
    //         |_| 0.0
    //     ).map(|(cost, path)| {
    //         (
    //             cost,
    //             path.into_iter().map(|idx| self.neigh_map[idx].0).collect::<Vec<NodeId>>()
    //         )
    //     });
    // 
    //     Some(SourceRoutingHeader {
    //         hops: path?.1,
    //         hop_index: 0,
    //     })
    // }

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
        println!("i was trying to reache {:?} {:?} but there is no route for that", nt,id);
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