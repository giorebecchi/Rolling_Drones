use std::collections::{HashMap, HashSet};
use petgraph::graph::{Graph, NodeIndex};
use petgraph::algo::{astar, dijkstra};
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use crossbeam_channel::{select_biased, Receiver, Sender};
use petgraph::data::Build;
use serde::Serialize;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet;
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType};
use crate::common_things::common::*;
use crate::servers::assembler::*;

#[derive(Clone)]
pub struct Server{
    server_id: NodeId,
    server_type: ServerType,
    session_id: u64,
    paths: HashMap<String,String>,
    images_ids: Vec<MediaId>,
    flooding: Vec<FloodResponse>,
    neigh_map: Graph<(NodeId,NodeType), u32, petgraph::Directed>,
    packet_recv: Receiver<Packet>,
    already_visited: HashSet<(NodeId,u64)>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    fragments_recv : HashMap<(NodeId,u64),Vec<Fragment>>,
    fragments_send : HashMap<u64,(NodeId,NodeType,Vec<Fragment>)>,
}

impl Server{
    pub fn new(id:NodeId, packet_recv: Receiver<Packet>, packet_send: HashMap<NodeId,Sender<Packet>>, file_path:&str)->Self{
        let path = Path::new(file_path);
        let file = File::open(path).unwrap();
        let reader = io::BufReader::new(file);

        let mut all_paths = HashMap::new();
        let mut images_ids = Vec::new();

        for line in reader.lines() {
            if let Ok(line) = line {
                let parts: Vec<&str> = line.split('/').collect();
                if let Some(last) = parts.last() {
                    images_ids.push(last.clone().to_string());
                    all_paths.insert(last.to_string(), line.clone().to_string());
                }

            }
        }

        Self{
            server_id:id,
            server_type: ServerType::TextServer,
            session_id:0,
            paths:all_paths,
            images_ids:images_ids,
            flooding: Vec::new(),
            neigh_map: Graph::new(),
            packet_recv:packet_recv,
            already_visited:HashSet::new(),
            packet_send:packet_send,
            fragments_recv : HashMap::new(),
            fragments_send : HashMap::new()
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


    fn send_packet<T>(&mut self, p:T, id:NodeId, nt:NodeType)where T : Fragmentation+Serialize{
        // println!("flooding : {:?}", self.flooding); //fa vedere tutte le flood response salvaate nel server
        // println!("graph : {:?}", self.neigh_map); //fa vedere il grafo (tutti i nodi e tutti gli edges)
        if let Some(srh) = self.get_route(id,nt){
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
                self.session_id+=1;
                //aggiungere un field nella struct server per salvare tutti i vari pacchetti nel caso in cui fossero droppati ecc.
            }
        }else {
            println!("no route found for {:?} {:?}!",nt,id);
        }
    }

    fn send_image(&mut self, path:&str, id:NodeId, nt:NodeType){
        let pos = path.rfind('.').unwrap();
        let fmd = FileMetaData{
            title: path[..pos].to_string(),
            extension:path[pos+1..].to_string(),
            s_id: self.session_id.clone() + 1,
        };
        if let Some(srh)=self.get_route(id,nt){
            if let Ok(vec) = TextServer::Text(fmd).serialize_data(srh.clone(),self.session_id){
                let mut fragments_send = Vec::new();
                for i in vec.iter(){
                    if let PacketType::MsgFragment(fragment) = i.clone().pack_type{
                        fragments_send.push(fragment);
                    }
                    self.forward_packet(i.clone());
                }
                self.fragments_send.insert(self.session_id.clone(), (id,nt,fragments_send));
                self.session_id+=1;
            }
            if let Ok(vec) = Vec::<u8>::serialize_file_from_path(path, srh, self.session_id){
                let mut fragments_send = Vec::new();
                for i in vec.iter(){
                    if let PacketType::MsgFragment(fragment) = i.clone().pack_type{
                        fragments_send.push(fragment);
                    }
                    self.forward_packet(i.clone());
                }
                self.fragments_send.insert(self.session_id.clone(), (id,nt,fragments_send));
                self.session_id+=1;
            }
        }else {
            println!("no route found for {:?} {:?}!",nt,path);
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
                    if let Ok(totalmsg) = WebBrowser::deserialize_data(vec){
                        match totalmsg{
                            WebBrowser::GetList => {println!("I shouldn't receive this command");}
                            WebBrowser::GetPosition(_) => {println!("I shouldn't receive this command");}
                            WebBrowser::GetMedia(media_id) => {
                                if self.paths.contains_key(&media_id){
                                    let path = self.paths.get(&media_id).unwrap().clone();
                                    self.send_image(path.as_str(),p.routing_header.hops[0],NodeType::Client);
                                }
                            }
                            WebBrowser::GetText(_) => {println!("I shouldn't receive this command");}
                            WebBrowser::GetServerType => {
                                self.send_packet(MediaServer::ServerType(self.clone().server_type), p.routing_header.hops[0], NodeType::Client);
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
                                        self.send_packet(MediaServer::ServerType(self.clone().server_type), p.routing_header.hops[0], NodeType::Server);
                                    }
                                    TextServer::PathResolution => {
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
    }

    fn packet_recover(&mut self, s_id: u64, lost_fragment_index: u64){
        if self.fragments_send.contains_key(&s_id){
            let info = self.fragments_send.get(&s_id).unwrap();
            for i in info.2.clone().iter(){
                if i.fragment_index==lost_fragment_index{
                    if let Some(srh) = self.get_route(info.0.clone(),info.1.clone()){
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
                    // self.neigh_map.remove_node(self.find_node(crashed_id,NodeType::Drone).unwrap());
                    self.neigh_map.remove_edge(self.neigh_map.find_edge(self.find_node(crashed_id,NodeType::Drone).unwrap_or_default(), self.find_node(packet.routing_header.hops[0], NodeType::Drone).unwrap_or_default()).unwrap_or_default());
                    self.packet_recover(s_id, nack.fragment_index);
                }
                NackType::DestinationIsDrone => {
                    println!("This error shouldn't happen!");
                    self.packet_recover(s_id, nack.fragment_index);
                }
                NackType::Dropped => {
                    if let Some(node) = self.find_node(id, NodeType::Drone) {
                        //non so se il clone di self vada bene, l'ho messo solo perchè dava errore
                        for neighbor in self.clone().neigh_map.neighbors(node) {
                            if let Some(edge) = self.neigh_map.find_edge(node, neighbor) {
                                self.neigh_map[edge] += 1;
                            }
                        }
                    }else {
                        println!("node not found (shouldn't happen)");
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
        if let PacketType::FloodResponse(mut flood) = p.pack_type{
            // println!("server {} has received flood response {}", self.server_id,flood.clone());
            let mut safetoadd = true;
            for i in self.flooding.iter(){
                if i.flood_id<flood.flood_id{
                    println!("the server is starting to receive new flood responses");
                    self.flooding.clear();
                    break;
                }else if i.flood_id==flood.flood_id{

                }else { safetoadd = false; break; }
            }
            if safetoadd {
                self.flooding.push(flood.clone());

                let mut first=true;
                let mut prev;
                match self.find_node(self.server_id, NodeType::Server) {
                    None => { prev = self.neigh_map.add_node((self.server_id, NodeType::Server)) }
                    Some(ni) => { prev = ni }
                }

                for &(j, k) in flood.path_trace.iter().skip(1) {
                    if let Some(&(prev_id, prev_nt)) = self.neigh_map.node_weight(prev) {
                        if self.node_exists(j.clone(), k.clone()) {
                            let next = self.find_node(j.clone(), k.clone()).unwrap();
                            if first {
                                if self.neigh_map.find_edge(prev, next).is_none() {
                                    self.neigh_map.add_edge(prev, next, 1);
                                }
                                if self.neigh_map.find_edge(next, prev).is_none() {
                                    self.neigh_map.add_edge(next, prev, 1);
                                }
                                first = false;
                            } else {
                                if prev_nt == NodeType::Drone && k == NodeType::Drone {
                                    if self.neigh_map.find_edge(prev, next).is_none() {
                                        self.neigh_map.add_edge(prev, next, 1);
                                    }
                                    if self.neigh_map.find_edge(next, prev).is_none() {
                                        self.neigh_map.add_edge(next, prev, 1);
                                    }
                                } else {
                                    if prev_nt == NodeType::Drone {
                                        if self.neigh_map.find_edge(prev, next).is_none() {
                                            self.neigh_map.add_edge(prev, next, 1);
                                        }
                                    }
                                    if k == NodeType::Drone {
                                        if self.neigh_map.find_edge(next, prev).is_none() {
                                            self.neigh_map.add_edge(next, prev, 1);
                                        }
                                    }
                                }
                            }

                            prev = next;
                        } else {
                            let newnodeid = self.neigh_map.add_node((j.clone(), k.clone()));
                            if prev_nt == NodeType::Drone && k == NodeType::Drone {
                                if self.neigh_map.find_edge(prev, newnodeid).is_none() {
                                    self.neigh_map.add_edge(prev, newnodeid, 1);
                                }
                                if self.neigh_map.find_edge(newnodeid, prev).is_none() {
                                    self.neigh_map.add_edge(newnodeid, prev, 1);
                                }
                            } else {
                                if prev_nt == NodeType::Drone {
                                    if self.neigh_map.find_edge(prev, newnodeid).is_none() {
                                        self.neigh_map.add_edge(prev, newnodeid, 1);
                                    }
                                }
                                if k == NodeType::Drone {
                                    if self.neigh_map.find_edge(newnodeid, prev).is_none() {
                                        self.neigh_map.add_edge(newnodeid, prev, 1);
                                    }
                                }
                            }
                            prev = newnodeid;
                        }
                    }
                }
            }else {
                println!("you received an outdated version of the flooding");
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
    fn get_route(&self, id:NodeId, nt:NodeType) -> Option<(SourceRoutingHeader)> {
        // Return Vec<NodeId> instead of Vec<NodeIndex>
        let start = self.find_node(self.server_id, NodeType::Server)?;
        let end = self.find_node(id,nt)?;
        let path = astar(
            &self.neigh_map,
            start,
            |finish| finish == end, // Stop when reaching the end node
            |e| *e.weight(),        // Edge cost
            |_| 0                   // No heuristic (Dijkstra behavior)
        ).map(|(cost, path)| (cost, path.into_iter().map(|idx| self.neigh_map[idx].0).collect())); // Convert NodeIndex -> NodeId
        Some(SourceRoutingHeader{hops:path?.1, hop_index:0})
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

