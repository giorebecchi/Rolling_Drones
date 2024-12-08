//use std::sync::Arc;
//use std::sync::Mutex;
use std::collections::{HashMap, HashSet};
use crossbeam_channel::{select_biased, Receiver, Sender};
use rand::Rng;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, FloodResponse, Nack, NackType, NodeType, Packet, PacketType};
//use lazy_static::lazy_static;


//lazy_static! { static ref CONSOLE_MUTEX: Arc<Mutex<()>> = Arc::new(Mutex::new(())); }
pub struct MyDrone {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pub pdr: f32,
    already_visited: HashSet<(NodeId,u64)>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
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
        let mut crash=false;
        while !crash{
            select_biased!{
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        match command.clone() {
                        DroneCommand::Crash => {
                                println!("Drone {} received Crash command", self.id);
                                crash=true;
                        },
                        DroneCommand::SetPacketDropRate(x) => {
                                println!("Drone {} received SetPacketDropRate command", self.id);
                                println!("Initial pdr of drone: {}", self.pdr);

                                },
                        DroneCommand::AddSender(id, send_pack) => {
                               // println!("added sender {:?} to {}",send_pack,id);
                        },
                        DroneCommand::RemoveSender(id) => {
                                //println!("removed sender {}, from drone {}", id, self.id);
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
                                println!();
                                //break;
                            }
                            PacketType::Nack(nack)=>{
                                println!("drone {} received a nack of type {:?} regarding fragment index {:?} ",self.id,nack.nack_type, nack.fragment_index);
                                println!();
                            }
                            PacketType::MsgFragment(msg)=>{
                                println!();
                                println!("drone {} has received msg fragment {:?}", self.id,msg);
                                //println!();
                            }
                            PacketType::FloodRequest(_)=>{
                                //println!("drone {} has received flood request {:?}",self.id,flood_request);
                            }
                            PacketType::FloodResponse(flood_response)=>{
                               println!("drone {} has received flood response {:?}",self.id,flood_response);
                                println!();
                            }
                        }
                        self.handle_packet(packet);
                    }
                },

            }

        }
        println!("Drone {} has exited the run loop", self.id);
    }
}
impl MyDrone {
    pub fn handle_command(&mut self, command: DroneCommand) {
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
            println!("Failed to add sender because the drone {} is already a neighbour of drone {}", node_id, self.id);
        }else {
            self.packet_send.insert(node_id, sender);
            println!("Added sender of new neighbor {} to neighbors of drone {}", node_id, self.id);
        }
    }
    pub fn set_pdr(&mut self, pdr: f32) {
        self.pdr = pdr;
        println!("Updated pdr to {}", pdr);
    }
    pub fn remove_sender(&mut self, node_id: NodeId) {
        if self.packet_send.remove_entry(&node_id).is_some() {
            println!("Removed sender for neighbor {} of drone {}", node_id, self.id);
        } else {
            println!("unable to remove: sender for neighbor {}  of drone {} was not found", node_id, self.id);
        }
    }
    pub fn handle_crash(&mut self) {
        println!("Drone {} entering crashing state", self.id);

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
    pub fn handle_packet(&mut self, packet: Packet) {
        match packet.clone().pack_type{
            PacketType::Ack(_)=>{
                self.forward_packet(packet.clone());
            }
            PacketType::Nack(_)=>{
                self.forward_packet(packet.clone());

            }
            PacketType::FloodRequest(_)=>{
                self.handle_flood_request(packet);

            }
            PacketType::FloodResponse(_)=>{
                self.forward_packet(packet.clone());

            }
            PacketType::MsgFragment(_)=>{
                self.handle_msg_fragment(packet.clone());
            }
        }
    }
     fn is_dropped(&self, packet: Packet) ->bool{
        let mut rng=rand::rng();
        let rand :f32 = (rng.random_range(0.0..=1.0) as f32 * 100.0).round() / 100.0;
        if rand<=self.pdr{
            let event = DroneEvent::PacketDropped(packet.clone());
            self.controller_send.send(event).unwrap();
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
            if self.is_dropped(packet.clone()) {

                println!("The message fragment was dropped");
                let mut pack = create_nack(packet.clone(),NackType::Dropped);
                if let Some(sender) = self.packet_send.get(&pack.routing_header.hops[pack.routing_header.hop_index+1]) {
                    println!("Drone {} is sending back a nack",self.id);
                    pack.routing_header.hop_index+=1;
                    sender.send(pack).unwrap();
                }
            }else {

                 let event = DroneEvent::PacketSent(packet.clone());
                 self.controller_send.send(event).unwrap();

                println!("No problems were found.");

                if let Some(sender)=self.packet_send.get(&next_hop) {
                    println!("Message fragment forwarded to drone {}", packet.routing_header.hops[packet.routing_header.hop_index+1]);
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
        // Check if there are more hops in the routing header
        if packet.routing_header.hop_index < packet.routing_header.hops.len() -1 {
            packet.routing_header.hop_index += 1;

            let next_hop = packet.routing_header.hops[packet.routing_header.hop_index];


            if let Some(sender) = self.packet_send.get(&next_hop) {
                if let Err(e) = sender.send(packet.clone()) {
                    println!(
                        "Failed to send packet to next hop {}: {:?}",
                        next_hop, e
                    );

                    self.handle_drone_event(packet.clone());

                } else {
                    // println!(
                    //     "Packet forwarded to next hop {}: {:?}",
                    //     next_hop, packet
                    // );
                }
            } else {
                println!(
                    "No sender found for next hop {}. Packet could not be forwarded.",
                    next_hop
                );
                self.handle_drone_event(packet.clone());

            }
        } else {
            println!("Error, The final destination of packet: {:?} is drone: {}",packet.clone(),self.id);
            let nack=create_nack(packet.clone(),NackType::DestinationIsDrone);
            self.send_nack(nack);
            return;
        }
    }
    fn handle_drone_event(&self, packet: Packet){
        match packet.pack_type{
            PacketType::Ack(_) => {
                let event = DroneEvent::ControllerShortcut(packet.clone());
                self.controller_send.send(event).unwrap();
            }
            PacketType::Nack(_) => {
                let event = DroneEvent::ControllerShortcut(packet.clone());
                self.controller_send.send(event).unwrap();
            }
            PacketType::FloodResponse(_)=>{
                let event = DroneEvent::ControllerShortcut(packet.clone());
                self.controller_send.send(event).unwrap();
            }
            _ => {}
        }
    }

    fn handle_flood_request(&mut self, packet : Packet){
        if let PacketType::FloodRequest(mut flood) = packet.pack_type{
            if self.already_visited.contains(&(flood.initiator_id, flood.flood_id)){
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
                    } else {
                        neighbour.send(new_packet.clone()).unwrap();
                    }
                }
            }
        }else{
            println!("Error: not a FloodRequest");
        }


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
    let mut reversed: Vec<T> = input.to_vec();
    reversed.reverse();
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