use std::collections::{HashMap, HashSet};
use crossbeam_channel::{select_biased, Receiver, Sender};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet;
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType, FRAGMENT_DSIZE};
use crate::common_things::common::ServerType;

pub struct Server{
    server_id: NodeId,
    server_type: ServerType,
    flooding: Vec<FloodResponse>,
    already_visited: HashSet<(NodeId,u64)>,
    packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
}

impl Server {
    fn new(server_id: NodeId, packet_recv: Receiver<Packet>, packet_send: HashMap<NodeId,Sender<Packet>> ) -> Server {
        Server {
            server_id,
            server_type: ServerType::TextServer,
            flooding: Vec::new(),
            already_visited: HashSet::new(),
            packet_recv: packet_recv,
            packet_send: packet_send,

        }
    }
    fn run(&mut self){
        &self.flooding;
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

    fn handle_packet(&mut self, packet: Packet){
        let mut fragments:Vec<(u64, [u8; FRAGMENT_DSIZE])> = Vec::new();
        let mut size_message: u64 = 0;
        match packet.pack_type{
            PacketType::MsgFragment(_) => {
                handle_fragment(&mut size_message, &mut fragments, packet);
                if fragments.len() == size_message as usize {

                }
            }
            PacketType::Ack(_) => {}
            PacketType::Nack(_) => {}
            PacketType::FloodRequest(_) => {self.handle_flood_request(packet)}
            PacketType::FloodResponse(_) => {handle_flood_response(packet)}
        }
    }

    fn forward_packet(&mut self, mut packet: Packet) {
        todo!()
    }
    /*
    fn forward_packet(&mut self, mut packet: Packet) {

        if packet.routing_header.hop_index < packet.routing_header.hops.len() -1 {
            packet.routing_header.hop_index += 1;

            let next_hop = packet.routing_header.hops[packet.routing_header.hop_index];


            if let Some(sender) = self.packet_send.get(&next_hop) {
                if let Err(_) = sender.send(packet.clone()) {

                    // self.handle_drone_event(packet.clone());

                }
            } else {
                //self.handle_drone_event(packet.clone());

            }
        } else {
            let nack=create_nack(packet.clone(),NackType::DestinationIsDrone);
            //self.send_nack(nack);
            return;
        }
    }

     */

    fn handle_msg_fragment(&mut self, fragments: Vec<(i32, Packet)> , packet: Packet){

    }
    fn handle_flood_request(&mut self, packet: Packet){}



    fn create_flood_response(&mut self, session_id: u64,flood_id: u64){}
}

fn handle_flood_response(p0: Packet) {
    todo!()
}

fn handle_fragment(size_message: &mut u64, fragments: &mut Vec<(u64, [u8; FRAGMENT_DSIZE])>, packet: Packet) {
    match packet.pack_type {
        PacketType::MsgFragment(ref fragment) => {
            *size_message = fragment.total_n_fragments;
            fragments.push((fragment.fragment_index, fragment.data));
        }
        _ => {
            println!("Il pacchetto non è un MsgFragment.");
        }
    }
}

fn reverse_vector<T: Clone>(input: &[T]) -> Vec<T> {
    let mut reversed: Vec<T> = input.to_vec();
    reversed.reverse();
    reversed
}
fn create_nack(packet: Packet, nack_type: NackType)->Packet{
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



