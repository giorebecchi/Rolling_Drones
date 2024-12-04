#![allow(unused)]

use rand::Rng;
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::{fs,thread};
use wg_2024::config::Config;
use wg_2024::controller::{DroneCommand,DroneEvent};
use wg_2024::drone::{Drone};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, Fragment, Nack, NackType, Packet, PacketType};

pub struct MyDrone {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pdr: f32,
    pub(crate) packet_send: HashMap<NodeId, Sender<Packet>>,
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
                                println!("set_packet_drop_rate {}", self.pdr);
                                // break;
                        },
                        DroneCommand::AddSender(id, send_pack) => {
                                println!("added sender");
                                // break;
                        },
                        DroneCommand::RemoveSender(id) => {
                                // println!("removed sender {}", id);
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
                            _=>println!("not yet done"),
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
                    let nack = Packet {
                        pack_type: PacketType::Nack(Nack{
                            fragment_index: 0,
                            nack_type: NackType::ErrorInRouting(self.id),
                        }),
                        routing_header: packet.routing_header,
                        session_id: packet.session_id,
                    };
                    self.send_nack(nack);
                }
            }
        }

        // Clean up internal state
        self.packet_send.clear();
        println!("Drone {} has completed crash process", self.id);
    }
    fn send_nack(&mut self,nack:Packet){

    }
    fn handle_packet(&mut self, mut packet: Packet) {
        match packet.clone().pack_type{
            PacketType::Ack(ack)=>{
                println!("Received ack");
                self.forward_packet(packet.clone());

            }
            PacketType::Nack(nack)=>{
                println!("Received Nack");
                self.forward_packet(packet.clone());

            }
            PacketType::FloodRequest(fl)=>{

            }
            PacketType::FloodResponse(fr)=>{

            }
            PacketType::MsgFragment(msg_fragment)=>{

                let mut rng=rand::rng();
                let rand=rng.random_range(0.0..=1.0);
                let mut next_hop=packet.routing_header.hops[packet.routing_header.hop_index+1];
                if let Some(sender)=self.packet_send.get(&next_hop){
                    if rand<=self.pdr{
                        println!("dropped");
                        let mut vec=Vec::new();
                        for node_id in (0..packet.routing_header.hop_index).rev(){
                            vec.push(packet.routing_header.hops[node_id]);
                        }
                        let nack=Nack{
                            fragment_index:0,
                            nack_type:NackType::DestinationIsDrone,
                        };
                        let mut pack=Packet{
                           pack_type: PacketType::Nack(nack.clone()),
                            routing_header: SourceRoutingHeader{
                                hop_index:0,
                                hops:vec.clone(),
                            },
                            session_id: 1,
                        };
                        if let Some(sender)=self.packet_send.get(&pack.routing_header.hops[1]){
                            sender.send(pack).unwrap();
                        }else{
                            packet.routing_header.hop_index+=1;
                            println!("ack, hop_index: {}",packet.routing_header.hop_index);
                            sender.send(packet).unwrap();
                        }
                    }
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

struct SimulationController {
    drones: HashMap<NodeId, Sender<DroneCommand>>,
    packet_channel: HashMap<NodeId, Sender<Packet>>,
    node_event_recv: Receiver<DroneEvent>,
}



impl SimulationController {
    fn crash_all(&mut self) {
        for (_, sender) in self.drones.iter() {
            sender.send(DroneCommand::Crash).unwrap();
        }
    }
    fn crash(&mut self, id: NodeId) {
        // Send the Crash command to the target drone
        if let Some(drone_sender) = self.drones.get(&id) {
            if let Err(err) = drone_sender.send(DroneCommand::Crash) {
                println!("Failed to send Crash command to drone {}: {:?}", id, err);
                return;
            }
            println!("Sent Crash command to drone {}", id);
        } else {
            println!("No drone with ID {:?}", id);
            return;
        }

        // Notify neighbors to remove the crashing drone as a sender
        if let Some(packet_channel) = self.packet_channel.get(&id) {
            let neighbor_ids: Vec<NodeId> = self.drones.keys().cloned().filter(|&nid| nid != id).collect();

            thread::scope(|s| {
                for neighbor_id in neighbor_ids {
                    if let Some(neighbor_sender) = self.drones.get(&neighbor_id) {
                        s.spawn(move || {
                            if let Err(err) = neighbor_sender.send(DroneCommand::RemoveSender(id)) {
                                println!(
                                    "Failed to send RemoveSender command to neighbor {}: {:?}",
                                    neighbor_id, err
                                );
                            } else {
                                println!(
                                    "Sent RemoveSender command to neighbor {} for drone {}",
                                    neighbor_id, id
                                );
                            }
                        });
                    }
                }
            });
        }
    }

    fn pdr(&mut self, id : NodeId) {
        for (idd, sender) in self.drones.iter() {
            if idd == &id {
                let mut rng = rand::thread_rng();
                // Use `gen_range` to generate a number in the range [0.0, 1.0]
                let mut rand : f32 = rng.gen_range(0.0..=1.0);
                // Round to two decimal places
                rand = (rand * 100.0).round() / 100.0;
                sender.send(DroneCommand::SetPacketDropRate(rand)).unwrap()
            }
        }
    }
    fn add_sender(&mut self, dst_id: NodeId, nghb_id: NodeId, sender: Sender<Packet>) {
        if let Some(drone_sender) = self.drones.get(&dst_id) {
            // Send the AddSender command to the target drone
            if let Err(err) = drone_sender.send(DroneCommand::AddSender(nghb_id, sender)) {
                println!(
                    "Failed to send AddSender command to drone {}: {:?}",
                    dst_id, err
                );
            } else {
                println!("Sent AddSender command to drone {}", dst_id);
            }
        } else {
            println!("No drone found with ID {}", dst_id);
        }
    }

    fn remove_sender(&mut self, drone_id: NodeId, nghb_id: NodeId) {///to be reviewed
        if let Some(drone_sender) = self.drones.get(&drone_id) {
            // Send the RemoveSender command to the target drone
            if let Err(err) = drone_sender.send(DroneCommand::RemoveSender(nghb_id)) {
                println!(
                    "Failed to send RemoveSender command to drone {} for neighbor {}: {:?}",
                    drone_id, nghb_id, err
                );
            } else {
                println!("Sent RemoveSender command to drone {} for neighbor {}", drone_id, nghb_id);
            }
        } else {
            println!("No drone found with ID {}", drone_id);
        }
    }
    fn ack(&mut self, mut packet: Packet) {
        let next_hop=packet.routing_header.hops[packet.routing_header.hop_index];
        if let Some(sender) = self.packet_channel.get(&next_hop) {
            sender.send(packet).unwrap();
        }else{
            println!("No sender found for hop {}", next_hop);
        }
    }
    fn msg_fragment(&mut self, mut packet: Packet){
        let mut next_hop=packet.routing_header.hops[packet.routing_header.hop_index+1];
        if let Some(sender) = self.packet_channel.get(&next_hop) {
            packet.routing_header.hop_index+=1;
            println!("ack, hop_index: {}",packet.routing_header.hop_index);
            sender.send(packet).unwrap();
        }
    }

}
pub fn parse_config(file: &str) -> Config {
    let file_str = fs::read_to_string(file).unwrap();
    toml::from_str(&file_str).unwrap()
}

pub fn test() {
    let config = parse_config("src/fair2024/input.toml");
    let mut controller_drones = HashMap::new();
    let mut packet_drones = HashMap::new();
    let (node_event_send, node_event_recv) = unbounded();

    let mut packet_channels = HashMap::new();
    for drone in config.drone.iter() {
        packet_channels.insert(drone.id, unbounded());
    }
    for client in config.client.iter() {
        packet_channels.insert(client.id, unbounded());
    }
    for server in config.server.iter() {
        packet_channels.insert(server.id, unbounded());
    }
    let mut handles = Vec::new();
    for drone in config.drone.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        packet_drones.insert(drone.id, packet_channels[&drone.id].0.clone());
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_node_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        handles.push(thread::spawn(move || {
            let mut drone = MyDrone::new(drone.id,node_event_send,controller_drone_recv,packet_recv,packet_send,drone.pdr);


            drone.run();
        }));
    }
    let mut controller = SimulationController {
        drones: controller_drones,
        node_event_recv: node_event_recv.clone(),
        packet_channel: packet_drones,
    };
    let my_packet=Packet{
        pack_type: PacketType::MsgFragment(Fragment{
            fragment_index: 1,
            total_n_fragments:1,
            length: 1,
            data: [1;128],
        }),
        routing_header: SourceRoutingHeader{hop_index:0,hops: vec![1,2,3]},
        session_id: 0,
    };
    let my_packet2=Packet{
        pack_type: PacketType::Ack(Ack{fragment_index:345}),
        routing_header: SourceRoutingHeader{hop_index:0,hops: vec![2,3]},
        session_id: 0,
    };
    let (sender_5, sium)= unbounded();

    controller.msg_fragment(my_packet);
    controller.add_sender(2, 5, sender_5);
    controller.remove_sender(2, 5);
    // controller.crash(1);
    // controller.ack(my_packet);
    // controller.ack(my_packet2);
    // controller.remove_sender(2,3);
    // controller.ack(3);
    ///ATTENTO!!!! Devi dare per forza un comando a tutti e tre i droni se vuoi che la simulazione finisca.
    /// In caso contrario la simulazione si fermerà al run del drone successivo che non ha ancora ricevuto un comando!

    while let Some(handle) = handles.pop() {
        handle.join().unwrap();

    }
}

