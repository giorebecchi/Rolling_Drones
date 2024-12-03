#![allow(unused)]

use rand::Rng;
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::{fs,thread};
use wg_2024::config::Config;
use wg_2024::controller::{DroneCommand,DroneEvent};
use wg_2024::drone::{Drone};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, Nack,NackType, Packet, PacketType};

struct MyDrone {
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
            packet_send: HashMap::new(),
        }
    }
    fn run(&mut self){
        loop{
            select_biased!{
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        match command.clone() {
                        DroneCommand::Crash => {
                                println!("drone {} crashed", self.id);
                                self.handle_command(command);
                                break;
                        },
                        DroneCommand::SetPacketDropRate(x) => {
                                // self.pdr = x;
                                println!("set_packet_drop_rate {}", self.pdr);
                                // break;
                        },
                        DroneCommand::AddSender(id, send_pack) => {
                                self.packet_send.insert(id, send_pack);
                                println!("added sender");
                                // break;
                        },
                        DroneCommand::RemoveSender(id) => {
                                self.packet_send.remove(&id);
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
                                println!("drone has received ack {:?}",ack);
                                break;
                            }
                            _=>println!("Not yet done")
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
                self.packet_send.insert(node_id, sender);
                println!("Added sender for neighbor {}", node_id);
            }
            DroneCommand::SetPacketDropRate(pdr) => {
                self.pdr = pdr;
                println!("Set packet drop rate to {}", pdr);
            }
            DroneCommand::Crash => {
                self.handle_crash();
            }
            DroneCommand::RemoveSender(node_id) => {
                if self.packet_send.remove(&node_id).is_some() {
                    println!("Removed sender for neighbor {}", node_id);
                } else {
                    println!("Sender for neighbor {} was not found", node_id);
                }
            }
        }
    }
    fn handle_packet(&mut self, packet: Packet) {
        match packet.pack_type{
            PacketType::Ack(ack)=>{

            }
            PacketType::Nack(nack)=>{

            }
            PacketType::FloodRequest(fl)=>{

            }
            PacketType::FloodResponse(fr)=>{

            }
            PacketType::MsgFragment(msg_fragment)=>{

            }

        }
    }
    fn handle_crash(&mut self) {
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
                            // Fill in additional fields as needed
                        };
                        //self.send_nack(nack);
                    }
                }
            }

            // Clean up internal state
            self.packet_send.clear();
            println!("Drone {} has completed crash process", self.id);
    }
    fn forward_packet(&self, mut packet: Packet) {
        // Check if there are more hops in the routing header
        if packet.routing_header.hop_index < packet.routing_header.hops.len() {
            // Get the next hop
            let next_hop = packet.routing_header.hops[packet.routing_header.hop_index];

            // Increment the hop index
            packet.routing_header.hop_index += 1;

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
    fn crash(&mut self, id : NodeId) {
        if let Some(drone_sender) = self.drones.get(&id) {
            // Notify the crashing drone
            drone_sender.send(DroneCommand::Crash).unwrap();

            // Notify all neighbors to remove the crashing drone as a sender
            if let Some(packet_channel) = self.packet_channel.get(&id) {
                for (&neighbor_id, neighbor_channel) in self.drones.iter() {
                    if neighbor_id != id {
                        neighbor_channel
                            .send(DroneCommand::RemoveSender(id))
                            .unwrap_or_else(|err| {
                                println!(
                                    "Failed to send RemoveSender command to neighbor {}: {:?}",
                                    neighbor_id, err
                                );
                            });
                    }
                }
            }
        } else {
            println!("No drone with ID {:?}", id);
        }
    }
    fn pdr(&mut self, id : NodeId) {
        for (idd, sender) in self.drones.iter() {
            if idd == &id {
                let mut rng=rand::rng();
                let rand= rng.random_range(0.0..=1.0);
                sender.send(DroneCommand::SetPacketDropRate(rand)).unwrap()
            }
        }
    }
    fn add_sender(&mut self,drone_id : NodeId, packet: Packet) {
        if let Some(sender) = self.drones.get(&drone_id) {
            let (sender_packet,_)=unbounded::<Packet>();
            sender.send(DroneCommand::AddSender(drone_id, sender_packet)).unwrap();
        } else {
            println!("No drone with ID {:?}", drone_id);
        }
    }
    fn remove_sender(&mut self, drone_id: NodeId, sender_id: NodeId) {
        if let Some(sender) = self.drones.get(&drone_id) {
            sender
                .send(DroneCommand::RemoveSender(sender_id))
                .unwrap_or_else(|err| println!("Failed to send RemoveSender command: {:?}", err));
        } else {
            println!("No drone with ID {:?}", drone_id);
        }
    }
    fn ack(&mut self, id: NodeId) {
        if let Some(sender) = self.packet_channel.get(&id) {
            let packet = Packet {
                pack_type: PacketType::Ack(Ack { fragment_index: 233748 }),
                routing_header: SourceRoutingHeader { hops: Vec::new(), hop_index: 0 },
                session_id: 0,
            };
            sender.send(packet).unwrap();
        }
    }

}
pub fn parse_config(file: &str) -> Config {
    let file_str = fs::read_to_string(file).unwrap();
    toml::from_str(&file_str).unwrap()
}

pub fn test() {
    let config = parse_config("src/tests/input.toml");
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
        pack_type: PacketType::Ack(Ack{fragment_index:0345}),
        routing_header: SourceRoutingHeader{hop_index:0,hops:Vec::new()},
        session_id: 0,
    };


    controller.crash(1);
    // controller.add_sender(2,my_packet);
    controller.remove_sender(2,3);
    controller.ack(3);
    ///ATTENTO!!!! Devi dare per forza un comando a tutti e tre i droni se vuoi che la simulazione finisca.
    /// In caso contrario la simulazione si fermerà al run del drone successivo che non ha ancora ricevuto un comando!

    while let Some(handle) = handles.pop() {
        handle.join().unwrap();

    }
}

