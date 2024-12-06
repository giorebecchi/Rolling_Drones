use std::sync::Arc;
use std::sync::Mutex;
use std::collections::{HashMap, HashSet};
use crossbeam_channel::{select_biased, Receiver, Sender};
use rand::Rng;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, FloodResponse, Nack, NackType, NodeType, Packet, PacketType};
use lazy_static::lazy_static;


lazy_static! { static ref CONSOLE_MUTEX: Arc<Mutex<()>> = Arc::new(Mutex::new(())); }
pub struct MyDrone {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pdr: f32,
    already_visited: HashSet<(NodeId,u64)>,
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
            already_visited: HashSet::new(),
            packet_send,
        }
    }
    fn run(&mut self){
        let mut crash=true;
        while crash{
            select_biased!{
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        let _lock = CONSOLE_MUTEX.lock().unwrap();
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
                                //println!("drone {} has received flood request {:?}",self.id,flood_request);
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
                // println!("Received ack: {:?}",ack);
                self.forward_packet(packet.clone());

            }
            PacketType::Nack(nack)=>{
                //println!("Received Nack: {:?}",nack);
                self.forward_packet(packet.clone());
                //Add SimulationControl Event log

            }
            PacketType::FloodRequest(fl)=>{
                // println!("Received Flood Request: {:?}",fl);
                self.handle_flood_request(packet);

            }
            PacketType::FloodResponse(fr)=>{
                // println!("Received Flood Response: {:?}",fr);
                self.forward_packet(packet.clone());

            }
            PacketType::MsgFragment(msg_fragment)=>{
                //println!("Received MsgFragment: {:?}",msg_fragment);
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
            let p = create_nack(packet,NackType::DestinationIsDrone);
            println!("Packet at its wrong destination: {}",self.id);
            self.send_nack(p);
            return;
        }

        if self.id != packet.routing_header.hops[packet.routing_header.hop_index]{
            let p = create_nack(packet,NackType::UnexpectedRecipient(self.id));
            self.send_nack(p);
            println!("Packet received by the wrong drone: {}",self.id);
            return;
        }
        let next_hop = packet.routing_header.hops[packet.routing_header.hop_index + 1];
        if let Some(_) = self.packet_send.get(&next_hop) {
            if self.is_dropped() {

                //////////////////////////////
                let event = DroneEvent::PacketDropped(packet.clone());
                self.controller_send.send(event).unwrap();
                /////////////////////////////

                println!("dropped");
                let pack = create_nack(packet.clone(),NackType::Dropped);
                if let Some(sender) = self.packet_send.get(&pack.routing_header.hops[pack.routing_header.hop_index]) {
                    sender.send(pack).unwrap();
                }else {
                    println!("dhdhdh");
                    return;
                }
            }else {

                //////////////////////////////
                let event = DroneEvent::PacketSent(packet.clone());
                self.controller_send.send(event).unwrap();
                /////////////////////////////

                if let Some(sender)=self.packet_send.get(&next_hop) {
                    println!("Message sent to {}", packet.routing_header.hops[packet.routing_header.hop_index+1]);
                    packet.routing_header.hop_index += 1;
                    sender.send(packet).unwrap();
                }

            }

        }else{
            let p = create_nack(packet, NackType::ErrorInRouting(next_hop));
            self.send_nack(p);
            return;
        }
    }
    fn forward_packet(&mut self, mut packet: Packet) {
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

                    //////////////////////////////

                    match packet.pack_type{
                        PacketType::Ack(_) => {
                            let event = DroneEvent::ControllerShortcut(packet.clone());
                            self.controller_send.send(event).unwrap();
                        }
                        PacketType::Nack(_) => {
                            let event = DroneEvent::ControllerShortcut(packet.clone());
                            self.controller_send.send(event).unwrap();
                        }
                        (_) => {}
                    }
                    ///////////////////////////////

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
                //////////////////////////////

                match packet.pack_type{
                    PacketType::Ack(_) => {
                        let event = DroneEvent::ControllerShortcut(packet.clone());
                        self.controller_send.send(event).unwrap();
                    }
                    PacketType::Nack(_) => {
                        let event = DroneEvent::ControllerShortcut(packet.clone());
                        self.controller_send.send(event).unwrap();
                    }
                    (_) => {}
                }
                ///////////////////////////////
            }
        } else {
            if packet.routing_header.hop_index>=packet.routing_header.hops.len()-1 {
                println!("Error, The final destination of packet: {:?} is drone: {}",packet.clone(),self.id);
                // let nack=create_nack(packet.clone(),NackType::DestinationIsDrone);
                // self.send_nack(nack);

                //////////////////////////////

                match packet.pack_type{
                    PacketType::Ack(_) => {
                        let event = DroneEvent::ControllerShortcut(packet.clone());
                        self.controller_send.send(event).unwrap();
                    }
                    PacketType::Nack(_) => {
                        let event = DroneEvent::ControllerShortcut(packet.clone());
                        self.controller_send.send(event).unwrap();
                    }
                    (_) => {}
                }
                ///////////////////////////////

                return;
            }
            println!("Packet has reached the final destination: {:?}", packet);
        }
    }

    fn handle_flood_request(&mut self, mut packet : Packet){
        if let PacketType::FloodRequest(mut flood) = packet.pack_type{
            if self.already_visited.contains(&(flood.initiator_id, flood.flood_id)){
                println!("error, already received!!");
                self.forward_packet(self.create_flood_response(packet.session_id,flood));
                return;
            }else {
                self.already_visited.insert((flood.initiator_id, flood.flood_id));
                flood.path_trace.push((self.id, NodeType::Drone));
                let new_packet = Packet{
                    pack_type : PacketType::FloodRequest(flood.clone()),
                    routing_header: packet.routing_header,
                    session_id: packet.session_id,
                };
                let (previous, _) = flood.path_trace[flood.path_trace.len() - 2];
                for (idd, neighbour) in self.packet_send.clone() {
                    if idd == previous {
                        //don't send to previous
                        // println!("not sent to {}, because it was the previous", previous);
                    } else {
                        //println!("sent flood request to : {}", idd);
                        neighbour.send(new_packet.clone()).unwrap();
                    }
                }
            }
        }else{
            println!("error not a floodrequest!!");
        }


        // if let PacketType::FloodRequest(mut flood) = packet.pack_type{
        //     for (node_id, _) in flood.path_trace.clone() {
        //         if self.id == node_id {
        //             //println!("error, already received!!");
        //             self.forward_packet(self.create_flood_response(packet.session_id,flood));
        //             return;
        //         }
        //     }
        //     flood.path_trace.push((self.id, NodeType::Drone));
        //     let new_packet = Packet{
        //         pack_type : PacketType::FloodRequest(flood.clone()),
        //         routing_header: packet.routing_header,
        //         session_id: packet.session_id,
        //     };
        //     let (previous,_)=flood.path_trace[flood.path_trace.len()-2];
        //     if self.packet_send.clone().len() == 1{
        //         self.forward_packet(self.create_flood_response(packet.session_id,flood));
        //         return;
        //     }else{
        //         for (idd,neighbour) in self.packet_send.clone(){
        //             if idd==previous{
        //                 //don't send to previous
        //                // println!("not sent to {}, because it was the previous", previous);
        //             }else{
        //                 //println!("sent flood request to : {}", idd);
        //                 neighbour.send(new_packet.clone()).unwrap();
        //             }
        //         }
        //     }
        // }else{
        //     println!("error not a flood request")
        // }
    }
    fn create_flood_response(&self, s_id: u64, mut flood : FloodRequest )->Packet{
        let mut src_header=Vec::new();
        flood.path_trace.push((self.id, NodeType::Drone));
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

}
fn reverse_vector<T: Clone>(input: &[T]) -> Vec<T> {
    let mut reversed: Vec<T> = input.to_vec(); // Convert to Vec
    reversed.reverse(); // Reverse in place
    reversed
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