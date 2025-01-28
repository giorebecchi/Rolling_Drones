use std::collections::{HashMap, HashSet};
use crossbeam_channel::{select_biased, Receiver, Sender};
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
    registered_clients: Vec<NodeId>,
    flooding: Vec<FloodResponse>,
    packet_recv: Receiver<Packet>,
    already_visited: HashSet<(NodeId,u64)>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    fragments_recv : HashMap<(NodeId,u64),Vec<Fragment>>,
    fragments_send : HashMap<(u64),Vec<Fragment>>,
}

impl Server{
    pub fn new(id:NodeId, packet_recv: Receiver<Packet>, packet_send: HashMap<NodeId,Sender<Packet>>)->Self{
        Self{
            server_id:id,
            server_type: ServerType::ComunicationServer,
            session_id:0,
            registered_clients: Vec::new(),
            flooding: Vec::new(),
            packet_recv:packet_recv,
            already_visited:HashSet::new(),
            packet_send:packet_send,
            fragments_recv : HashMap::new(),
            fragments_send : HashMap::new()
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
            }
        }
    }
    pub fn handle_packet(&mut self, p:Packet){
        match p.clone().pack_type {
            PacketType::MsgFragment(_) => {println!("received packet {p}");self.handle_msg_fragment(p)}
            PacketType::Ack(_) => {}
            PacketType::Nack(_) => {}
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

    fn send_packet<T>(&mut self, p:T, header: SourceRoutingHeader)where T : Fragmentation+Serialize{
        if let Ok(vec) = p.serialize_data(header,self.session_id){
            let mut fragments_send = Vec::new();
            for i in vec.iter(){
                if let PacketType::MsgFragment(fragment) = i.clone().pack_type{
                    fragments_send.push(fragment);
                }
            }
            self.fragments_send.insert(self.session_id.clone(), fragments_send);
            self.session_id+=1;
            //aggiungere un field nella struct server per salvare tutti i vari pacchetti nel caso in cui fossero droppati ecc.
            for i in vec.iter(){
                self.forward_packet(i.clone());
            }
        }
    }

    fn handle_msg_fragment(&mut self, p:Packet){
        self.forward_packet(create_ack(p.clone()));
        if let PacketType::MsgFragment(fragment) = p.pack_type{
            if self.fragments_recv.contains_key(&(p.routing_header.hops.clone()[0],p.session_id)){
                if let Some((mut vec)) = self.fragments_recv.get_mut(&(p.routing_header.hops[0],p.session_id)){
                    vec.push(fragment.clone());
                    if fragment.total_n_fragments == vec.len() as u64{
                        if let Ok(totalmsg) = ChatRequest::deserialize_data(vec){
                            match totalmsg{
                                ChatRequest::ServerType => {let route = self.get_route(p.routing_header.hops[0], NodeType::Client);self.send_packet(self.clone().server_type, route);}
                                ChatRequest::RegisterClient(_) => {}
                                ChatRequest::GetListClients => {}
                                ChatRequest::SendMessage(_) => {}
                                ChatRequest::EndChat(_) => {}
                            }
                        }
                    }
                }
            }else {
                let mut vec = Vec::new();
                vec.push(fragment.clone());
                self.fragments_recv.insert((p.routing_header.hops.clone()[0], p.session_id), vec);
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
            println!("server {} has received flood response {}", self.server_id,flood.clone());
            let mut safetoadd = true;
            for i in self.flooding.iter(){
                if i.flood_id<flood.flood_id{
                    self.flooding.clear();
                    break;
                }else if i.flood_id==flood.flood_id{

                }else { safetoadd = false; break; }
            }
            if safetoadd{
                self.flooding.push(flood.clone());
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
            routing_header: SourceRoutingHeader{hop_index:1, hops:Vec::new()},
            session_id: flood_id,
            pack_type: PacketType::FloodRequest(FloodRequest{
                flood_id,
                initiator_id: self.server_id,
                path_trace: vec![(self.server_id, NodeType::Server)],
            }),
        };
        for (id,neighbour) in self.packet_send.clone(){
            neighbour.send(flood.clone()).unwrap();
        }
    }
    fn get_route(&mut self, id:NodeId, nt:NodeType)->SourceRoutingHeader{
        self.flooding();
        let mut possible_route=Vec::new();
        for i in self.flooding.iter(){
            if i.path_trace.contains(&(id,nt)){
                let mut vec = Vec::new();
                for z in 0..i.path_trace.len(){
                    vec=i.path_trace.iter().map(|x| x.0).collect::<Vec<NodeId>>();
                    if i.path_trace[z] == (id,nt) {
                        possible_route.push(vec);
                        break;
                    }
                }
            }
        }
        let mut final_route = possible_route[0].clone();
        for alternative_route in possible_route.iter(){
            if alternative_route.len()<final_route.len(){
                final_route = alternative_route.clone();
            }
        }
        SourceRoutingHeader{
            hop_index: 0,
            hops: final_route,
        }
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
fn create_nack(packet: Packet,nack_type: NackType)->Packet{
    let mut vec = Vec::new();
    for node_id in (0..=packet.routing_header.hop_index).rev() {
        vec.push(packet.routing_header.hops[node_id]);
    }
    let nack = Nack {
        fragment_index: if let PacketType::MsgFragment(fragment) = packet.pack_type {
            fragment.fragment_index
        } else {
            0
        },
        nack_type,
    };
    let pack = Packet {
        pack_type: PacketType::Nack(nack.clone()),
        routing_header: SourceRoutingHeader {
            hop_index: 0,
            hops: vec.clone(),
        },
        session_id: packet.session_id,
    };
    pack
}

