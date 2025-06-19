#![allow(dead_code)]
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
use petgraph::Incoming;
use petgraph::prelude::EdgeRef;
use serde::Serialize;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, NackType, NodeType, Packet, PacketType};
use crate::common_data::common::*;
use crate::servers::assembler::*;
use crate::gui::login_window::NodeType as MyNodeType;

#[derive(Serialize, Clone, Debug)]
struct Drops{
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
    images_ids: Vec<MediaId>,
    flooding: Vec<FloodResponse>,
    neigh_map: Graph<(NodeId,NodeType), f64, petgraph::Directed>,
    stats: HashMap<NodeIndex, Drops>,
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
    pub fn new(id:NodeId, packet_recv: Receiver<Packet>, packet_send: HashMap<NodeId,Sender<Packet>>, rcv_flood: Receiver<BackGroundFlood>, rcv_command: Receiver<ServerCommands>, send_event: Sender<ServerEvent>, file_path:&str)->Self{
        let path = Path::new(file_path);
        let file = File::open(path).unwrap();
        let reader = io::BufReader::new(file);

        let mut all_paths = HashMap::new();
        let mut images_ids = Vec::new();

        for line in reader.lines() {
            if let Ok(line) = line {
                let parts: Vec<&str> = line.split('/').collect();
                if let Some(last) = parts.last() {
                    images_ids.push(last.to_string());
                    all_paths.insert(last.to_string(), line.clone().to_string());
                }

            }
        }

        Self{
            server_id:id,
            server_type: ServerType::MediaServer,
            session_id:0,
            flood_id: 0,
            paths:all_paths,
            images_ids,
            flooding: Vec::new(),
            neigh_map: Graph::new(),
            stats: HashMap::new(),
            packet_recv,
            already_visited:HashSet::new(),
            packet_send,
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
                }
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
                            ServerCommands::PdrChanged(id)=>{
                                self.reset_drone_stats(id);
                            }
                        }
                    }
                }
            }
        }
    }
    fn send_topology_graph(&self) {
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

    fn reset_drone_stats(&mut self, id: NodeId){
        if let Some(index) = self.find_node(id, NodeType::Drone){
            if self.stats.contains_key(&index){
                self.stats.insert(index,Drops{dropped: 0, forwarded: 1});
            }
            let mut edges = Vec::new();
            for edge_idx in self.neigh_map.edges_directed(index, Incoming).map(|e| e.id()){
                edges.push(edge_idx);
            }
            for e in edges{
                if let Some(weight) = self.neigh_map.edge_weight_mut(e) {
                    *weight=0.0;
                }
            }
        }
        // println!("graph del media {:?} dopo il reset stats del drone {:?} :  {:?}", self.server_id,id,self.neigh_map);
        // println!("stats resetted successfully");
    }

    fn handle_packet(&mut self, p:Packet){
        match p.clone().pack_type {
            PacketType::MsgFragment(_) => {/*println!("received packet {p}");*/self.handle_msg_fragment(p)}
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


    fn send_packet(&mut self, p:MediaServer, id:NodeId, nt:NodeType){
        //println!("flooding : {:?}", self.flooding); //fa vedere tutte le flood response salvate nel server
        //println!("graph del media {:?} : {:?}",self.server_id , self.neigh_map); //fa vedere il grafo (tutti i nodi e tutti gli edges)
        if let Some(srh) = self.best_path_custom_cost(id,nt){
            // println!("srh : {:?}",srh);
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
                    MediaServer::ServerTypeMedia(_) => {self.send_event.send(ServerEvent::MediaPacketInfo(self.server_id, MyNodeType::MediaServer, MediaServerEvent::SendingServerTypeMedia(vec.len() as u64),self.session_id)).unwrap();}
                    MediaServer::SendPath(_) => {self.send_event.send(ServerEvent::MediaPacketInfo(self.server_id, MyNodeType::MediaServer, MediaServerEvent::SendingPathRes(vec.len() as u64),self.session_id)).unwrap();}
                    MediaServer::SendMedia(_) => {self.send_event.send(ServerEvent::MediaPacketInfo(self.server_id, MyNodeType::MediaServer, MediaServerEvent::SendingMedia(vec.len() as u64),self.session_id)).unwrap();}
                }
                self.session_id+=1;
                //aggiungere un field nella struct server per salvare tutti i vari pacchetti nel caso in cui fossero droppati ecc.
            }
        }else {
            // println!("sono il mediaserver {:?} no route found for sending packet {:?} to {:?} {:?}!",self.server_id,p,nt,id);
        }
    }

    fn send_image(&mut self, path:&str, id:NodeId, nt:NodeType){
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
            if let Ok(vec) = MediaServer::SendMedia(fmd).serialize_data(srh.clone(),self.session_id){
                let mut fragments_send = Vec::new();
                for i in vec.iter(){
                    if let PacketType::MsgFragment(fragment) = i.clone().pack_type{
                        fragments_send.push(fragment);
                    }
                    self.forward_packet(i.clone());
                }
                // println!("finito di mandare l'immagine richiesta");
                self.fragments_send.insert(self.session_id.clone(), (id,nt,fragments_send));
                self.send_event.send(ServerEvent::MediaPacketInfo(self.server_id, MyNodeType::MediaServer, MediaServerEvent::SendingMedia(vec.len() as u64),self.session_id)).unwrap();
                self.session_id+=1;
            }
        }else {
            println!("sono il mediaserver {:?} no route found for sending the image to {:?} {:?}!",self.server_id,nt,path);
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
                            WebBrowserCommands::GetList => {println!("I shouldn't receive this command");}
                            WebBrowserCommands::GetPosition(_) => {println!("I shouldn't receive this command");}
                            WebBrowserCommands::GetMedia(media_id) => {
                                if self.paths.contains_key(&media_id){
                                    let path = self.paths.get(&media_id).unwrap().clone();
                                    self.send_image(path.as_str(),p.routing_header.hops[0],NodeType::Client); 
                                }
                            }
                            WebBrowserCommands::GetText(_) => {println!("I shouldn't receive this command");}
                            WebBrowserCommands::GetServerType => {
                                // println!("problems in sending servertype");
                                self.send_packet(MediaServer::ServerTypeMedia(self.clone().server_type), p.routing_header.hops[0], NodeType::Client);
                            }
                        }
                    }else {
                        if let Ok(totalmsg) = ChatResponse::deserialize_data(vec) {
                            match totalmsg {
                                _ => { println!("I shouldn't receive these commands"); }
                            }
                        }else {
                            if let Ok(totalmsg) = TextServer::deserialize_data(vec){
                                match totalmsg {
                                    TextServer::ServerTypeReq => {
                                        // println!("sono il media {:?} e sto mandando il mio servertype {:?} al text {:?}",self.server_id,self.server_type,p.routing_header.hops[0]);
                                        // println!("grafo attuale del media {:?}: \n{:?} \nmentre cerca di mandare il suo server type a {:?}", self.server_id, self.neigh_map ,p.routing_header.hops[0]);
                                        self.send_packet(MediaServer::ServerTypeMedia(self.clone().server_type), p.routing_header.hops[0], NodeType::Server);
                                    }
                                    TextServer::PathResolution => {
                                        //println!("sono il media {:?} e sto mandando il mio pathres {:?} al text {:?}",self.server_id,self.server_type,p.routing_header.hops[0]);
                                        self.send_packet(MediaServer::SendPath(self.clone().images_ids),p.routing_header.hops[0], NodeType::Server);
                                    }
                                    _ => {println!("I shouldn't receive these commands");}
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
                    //println!("graph del mediaserver post ack {:?}: {:?}",self.server_id, self.neigh_map);
                    //println!("Drone {:?} stats post ack {:?}",i,self.stats.get(&n).unwrap())
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
                        println!("i am the media {:?} there isn't a route to reach the packet destination (shouldn't happen)", self.server_id);
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
                    // println!("sono il media {:?} ho ricevuto un errorinrouting with route {:?}, the drone that crashed is {:?}", self.server_id, packet.routing_header.hops, crashed_id.clone());
                    let node1;
                    let node2;
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
                    // println!("graph del media dopo aver tolto gli edges del drone crashato {:?}", self.neigh_map);
                    self.packet_recover(s_id, nack.fragment_index);
                }
                NackType::DestinationIsDrone => {
                    println!("This error shouldn't happen!");
                    self.packet_recover(s_id, nack.fragment_index);
                }
                NackType::Dropped => {
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
                                //println!("graph del mediaserver post nack {:?}: {:?}",self.server_id, self.neigh_map);
                                //println!("Drone {:?} stats post nack {:?}",i,self.stats.get(&n).unwrap());
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
                        // println!("the server is starting to receive new flood responses");
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
                                self.stats.insert(newnodeid.clone(), Drops { dropped: 0, forwarded: 1 });
                                if self.neigh_map.find_edge(prev, newnodeid).is_none() {
                                    self.neigh_map.add_edge(prev, newnodeid, 0.0);
                                }
                                if self.neigh_map.find_edge(newnodeid, prev).is_none() {
                                    self.neigh_map.add_edge(newnodeid, prev, 0.0);
                                }
                                prev = newnodeid;
                            }
                    }
                } else {
                    // println!("you received an outdated version of the flooding");
                }
            }else {
                // println!("forwarding the flood because it is not mine");
                self.forward_packet(p);
            }
        }
    }

    fn flooding(&mut self){
        // println!("server {} is starting a flooding",self.server_id);
        let flood = Packet{
            routing_header: SourceRoutingHeader::empty_route(),
            session_id: self.flood_id,
            pack_type: PacketType::FloodRequest(FloodRequest{
                flood_id: self.flood_id,
                initiator_id: self.server_id,
                path_trace: vec![(self.server_id, NodeType::Server)],
            }),
        };
        self.flood_id+=1;
        for (_,neighbour) in self.packet_send.clone(){
            if let Err(_)=neighbour.send(flood.clone()){
                println!("error flood request");
            };
        }
    }

    fn node_exists(&self, id:NodeId, nt:NodeType) -> bool {
        self.neigh_map.node_indices().any(|i| self.neigh_map[i] == (id,nt))
    }

    fn find_node(&self, id: NodeId, nt: NodeType) -> Option<NodeIndex> {
        self.neigh_map.node_indices().find(|&i| self.neigh_map[i] == (id, nt))
    }
    
    fn best_path_custom_cost(&self, id: NodeId, nt: NodeType) -> Option<SourceRoutingHeader> {
        let start = self.find_node(self.server_id, NodeType::Server)?;
        let end = self.find_node(id,nt)?;
        let mut heap = BinaryHeap::new();
        let mut best_cost: HashMap<NodeIndex, f64> = HashMap::new();
        let mut predecessors: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        heap.push((PathCostandlen(0.0, 0), start, vec![start]));
        best_cost.insert(start, 0.0);

        while let Some((PathCostandlen(cost,_), node, path)) = heap.pop() {
            if node == end {
                let hops = path.into_iter().map(|idx| self.neigh_map[idx].0).collect();
                let src =  SourceRoutingHeader{
                    hops,
                    hop_index:0,
                };
                // println!("the graph of the media {:?}, in this moment is {:?}", self.server_id, self.neigh_map);
                // println!("best src found {:?}, with cost of {:?}", src, cost);
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
                    // println!(
                    //     "Pushing path: {:?} → {:?}, cost: {:.4}, length: {}",
                    //     path.iter().map(|idx| self.neigh_map[*idx].0).collect::<Vec<_>>(),
                    //     self.neigh_map[neighbor].0,
                    //     new_cost,
                    //     path.len() + 1
                    // );
                    heap.push((PathCostandlen(new_cost, new_path.len()), neighbor, new_path));
                }
            }
        }

        None
    }
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

#[derive(Debug,PartialEq,Clone,Copy)]
pub struct PathCostandlen(f64, usize);

impl Eq for PathCostandlen {}


impl Ord for PathCostandlen {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse for min-heap behavior
        match self.0.partial_cmp(&other.0).unwrap() {
            std::cmp::Ordering::Equal => other.1.cmp(&self.1), // Shorter path wins
            ord => ord.reverse(),
        }
    }
}

impl PartialOrd for PathCostandlen {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}