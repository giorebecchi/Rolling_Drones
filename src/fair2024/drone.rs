use std::collections::HashMap;
use crossbeam_channel::{select_biased, Receiver, Sender};
use rand::Rng;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Nack, NackType, Packet, PacketType};

pub struct MyDrone {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pdr: f32,
    packet_send: HashMap<NodeId, Sender<Packet>>,
}

impl Drone for MyDrone {
    fn new(id: NodeId,
           controller_send: Sender<DroneEvent>,
           controller_recv: Receiver<DroneCommand>,
           packet_recv: Receiver<Packet>,
           packet_send: HashMap<NodeId, Sender<Packet>>,
           pdr: f32) -> Self {
        Self {
            id,
            controller_send,
            controller_recv,
            packet_recv,
            pdr,
            packet_send,
        }
    }
    fn run(&mut self){
        let mut crash=true;
        while crash{
            select_biased!{
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        match command.clone() {
                        DroneCommand::Crash => {
                                println!("drone {} crashed", self.id);
                                crash=false;
                                // break;
                        },
                        DroneCommand::SetPacketDropRate(x) => {
                                // self.pdr = x;
                                println!("set_packet_drop_rate, before it was: {} \n now pdr is: {}", self.pdr,x);
                                // break;
                        },
                        DroneCommand::AddSender(id, send_pack) => {
                                println!("added sender {:?} to {}",send_pack,id);
                                // break;
                        },
                        DroneCommand::RemoveSender(id) => {
                                println!("removed sender {}", id);
                                //break;
                            }

                    }
                        self.handle_command(command);
                    }
                }
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        match packet.clone().pack_type{
                            PacketType::Ack(ack)=>{
                                println!("drone {} has received ack {:?}",self.id,ack);
                                //break;
                            }
                            PacketType::Nack(nack)=>{
                                println!("drone {} has received nack {:?}",self.id,nack);
                            }
                            PacketType::MsgFragment(msg)=>{
                                println!("drone {} has received msg fragment {:?}", self.id,msg);
                            }
                            PacketType::FloodRequest(flood_request)=>{
                                println!("drone {} has received flood request {:?}",self.id,flood_request);
                            }
                            PacketType::FloodResponse(flood_response)=>{
                                println!("drone {} has received flood response {:?}",self.id,flood_response);
                            }
                        }
                        self.handle_packet(packet);
                    }
                },
            }
        }
    }
}
impl MyDrone {
    fn handle_command(&mut self, command: DroneCommand) {
        match command {
            DroneCommand::AddSender(node_id, sender) => {
                self.add_sender(node_id, sender);
            }
            DroneCommand::SetPacketDropRate(pdr) => {
                self.set_pdr(pdr);

            }
            DroneCommand::Crash => {
                self.handle_crash();
            }
            DroneCommand::RemoveSender(node_id) => {
                self.remove_sender(node_id);
            }
        }
    }
    pub fn add_sender(&mut self, node_id: NodeId, sender: Sender<Packet>) {
        if self.packet_send.contains_key(&node_id) {
            println!("The drone {} is already a neighbour", node_id);
        }else {
            self.packet_send.insert(node_id, sender);
            println!("Added sender for neighbor {}", node_id);
        }
    }
    pub fn set_pdr(&mut self, pdr: f32) {
        self.pdr = pdr;
        println!("Set packet drop rate to {}", pdr);
    }
    pub fn remove_sender(&mut self, node_id: NodeId) {
        if self.packet_send.remove(&node_id).is_some() {
            println!("Removed sender for neighbor {}", node_id);
        } else {
            println!("Sender for neighbor {} was not found", node_id);
        }
    }
    pub fn handle_crash(&mut self) {
        println!("Drone {} entering crashing state", self.id);

        // Process remaining packets
        while let Ok(packet) = self.packet_recv.try_recv() {
            match packet.pack_type {
                PacketType::Ack(_) | PacketType::Nack(_) | PacketType::FloodResponse(_) => {
                    println!("Forwarding packet during crash: {:?}", packet);
                    self.forward_packet(packet);
                }
                PacketType::FloodRequest(_) => {
                    println!("Dropping FloodRequest during crash");
                }
                _ => {
                    println!("Sending ErrorInRouting Nack for {:?}", packet);
                    let nack=create_nack(packet,NackType::ErrorInRouting(self.id));
                    self.send_nack(nack);
                }
            }
        }

        // Clean up internal state
        self.packet_send.clear();
        println!("Drone {} has completed crash process", self.id);
    }
    fn send_nack(&mut self,nack: Packet) {
        self.forward_packet(nack);
    }
    fn handle_packet(&mut self, packet: Packet) {
        match packet.clone().pack_type{
            PacketType::Ack(ack)=>{
                println!("Received ack: {:?}",ack);
                self.forward_packet(packet.clone());

            }
            PacketType::Nack(nack)=>{
                println!("Received Nack: {:?}",nack);
                self.forward_packet(packet.clone());
                //Add SimulationControl Event log

            }
            PacketType::FloodRequest(fl)=>{
                println!("Received Flood Request: {:?}",fl);

            }
            PacketType::FloodResponse(fr)=>{
                println!("Received Flood Response: {:?}",fr);

            }
            PacketType::MsgFragment(msg_fragment)=>{
                println!("Received MsgFragment: {:?}",msg_fragment);
                self.handle_msg_fragment(packet.clone());
            }
        }
    }
    fn is_dropped(&self)->bool{
        let mut rng=rand::rng();
        let rand :f32 = (rng.random_range(0.0..=1.0) as f32 * 100.0).round() / 100.0;
        if rand<=self.pdr{
            return true;
        }
        false
    }
    fn handle_msg_fragment(&mut self, mut packet: Packet) {
        if packet.routing_header.hop_index + 1 >= packet.routing_header.hops.len(){
            println!("Packet at its destination");
            return;
        }
        let next_hop = packet.routing_header.hops[packet.routing_header.hop_index + 1];
        if let Some(_) = self.packet_send.get(&next_hop) {
            if self.is_dropped() {
                println!("dropped");
                let pack = create_nack(packet.clone(),NackType::Dropped);
                if let Some(sender) = self.packet_send.get(&pack.routing_header.hops[pack.routing_header.hop_index]) {
                    sender.send(pack).unwrap();
                }else {
                    println!("dhdhdh");
                    return;
                }
            }else {
                if let Some(sender)=self.packet_send.get(&next_hop) {
                    println!("Message sent to {}", packet.routing_header.hops[packet.routing_header.hop_index+1]);
                    packet.routing_header.hop_index += 1;
                    sender.send(packet).unwrap();
                }
            }
        }
    }
    fn forward_packet(&self, mut packet: Packet) {
        packet.routing_header.hop_index += 1;
        // Check if there are more hops in the routing header
        if packet.routing_header.hop_index < packet.routing_header.hops.len() {
            // Get the next hop
            let next_hop = packet.routing_header.hops[packet.routing_header.hop_index];

            // Increment the hop index

            // Send the packet to the next hop
            if let Some(sender) = self.packet_send.get(&next_hop) {
                if let Err(e) = sender.send(packet.clone()) {
                    println!(
                        "Failed to send packet to next hop {}: {:?}",
                        next_hop, e
                    );
                } else {
                    println!(
                        "Packet forwarded to next hop {}: {:?}",
                        next_hop, packet
                    );
                }
            } else {
                println!(
                    "No sender found for next hop {}. Packet could not be forwarded.",
                    next_hop
                );
            }
        } else {
            println!("Packet has reached the final destination: {:?}", packet);
        }
    }

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
            hop_index: 1,
            hops: vec.clone(),
        },
        session_id: packet.session_id,
    };
    pack
}