#![allow(unused)]

use rand::Rng;
use crossbeam_channel::{select_biased, unbounded, Receiver, RecvError, Sender, TryRecvError};
use std::collections::HashMap;
use std::{fs,thread};
use wg_2024::config::Config;
use wg_2024::controller::{DroneCommand,DroneEvent};
use wg_2024::drone::{Drone};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, Fragment, Nack, NackType, Packet, PacketType};
use crate::fair2024::drone::*;


struct SimulationController {
    drones: HashMap<NodeId, Sender<DroneCommand>>,
    packet_channel: HashMap<NodeId, Sender<Packet>>,
    node_event_recv: Receiver<DroneEvent>,
}



impl SimulationController {
    fn run(&mut self) {
        select_biased! {
            recv(self.node_event_recv()) -> command =>{
                if let Ok(command) = command {
                    match command{
                        DroneEvent::PacketSent(packet) => {
                            println!("drone sent :");
                        }
                        DroneEvent::PacketDropped(packet) => {
                            println!("drone dropped :");
                        }
                        DroneEvent::ControllerShortcut(controller_shortcut) => {
                            println!("packet sent to destination");
                        }
                    }
                    self.handle_event(command.clone());
                }

            }
        }

    }

    fn handle_event(&mut self, command: DroneEvent) {
        match command {
            DroneEvent::PacketSent(packet) => {
                self.print_packet(packet);
            }
            DroneEvent::PacketDropped(packet) => {
                self.print_packet(packet);
            }
            DroneEvent::ControllerShortcut(packet) => {
                self.send_to_destination(packet);
            }
        }
    }
    fn print_packet(&mut self, packet: Packet) {
        println!("source id: {:#?}", packet.routing_header.hops[0]);
        println!("destination id: {:#?}", packet.routing_header.hops[packet.routing_header.hops.len() - 1]);
        println!("path: {:#?}", packet.routing_header.hops);
    }
    fn send_to_destination(&mut self, packet: Packet) {
        let addr = packet.routing_header.hops[packet.routing_header.hops.len() - 1];
        self.print_packet(packet.clone());

        if let Some(sender) = self.packet_channel.get(&addr) {
            sender.send(packet).unwrap();
        }

    }




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
                let mut rng = rand::rng();
                // Use `gen_range` to generate a number in the range [0.0, 1.0]
                let mut rand : f32 = rng.random_range(0.0..=1.0);
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
            println!(" hop_index: {}",packet.routing_header.hop_index);
            sender.send(packet).unwrap();
        }
    }
    fn initiate_flood(&mut self, mut packet: Packet){
        if let PacketType::FloodRequest(mut flood_request)=packet.clone().pack_type{
            flood_request.initiator_id=4; //add your client/server id!
            let next_hop=packet.clone().routing_header.hops[packet.routing_header.hop_index+1];
            if let Some(sender) = self.packet_channel.get(&next_hop) {
                println!("Sent Flood packet to : {}", next_hop);
                sender.send(packet).unwrap();
            }else{
                println!("No sender found for hop {}", next_hop);
            }
        }else{
            println!("called function initiate_flood with a wrong packet type : {:?}",packet.pack_type);
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
        routing_header: SourceRoutingHeader{hop_index:0,hops: vec![1,3,2]},
        session_id: 0,
    };
    let my_packet2=Packet{
        pack_type: PacketType::Ack(Ack{fragment_index:345}),
        routing_header: SourceRoutingHeader{hop_index:0,hops: vec![2,3]},
        session_id: 0,
    };
    // let (sender_5, sium)= unbounded();

    controller.crash(2);
    controller.msg_fragment(my_packet);

    // controller.msg_fragment(my_packet);
    // controller.add_sender(2, 5, sender_5);
    // controller.remove_sender(2, 5);
    // controller.crash(1);
    // controller.ack(my_packet);
    // controller.ack(my_packet2);
    // controller.remove_sender(2,3);
    // controller.ack(3);
    // controller.msg_fragment(my_packet);
    ///ATTENTO!!!! Devi dare per forza un comando a tutti e tre i droni se vuoi che la simulazione finisca.
    /// In caso contrario la simulazione si fermerà al run del drone successivo che non ha ancora ricevuto un comando!

    while let Some(handle) = handles.pop() {
        handle.join().unwrap();

    }
}

