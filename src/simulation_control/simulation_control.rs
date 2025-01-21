#![allow(unused)]

use rand::Rng;
use crossbeam_channel::{select_biased, unbounded, Receiver, RecvError, Sender, TryRecvError};
use std::collections::HashMap;
use std::{fs,thread};
use std::sync::{Arc,Mutex};
use lazy_static::lazy_static;
use wg_2024::packet::NodeType;
use wg_2024::config::Config;
use wg_2024::controller::{DroneCommand,DroneEvent};
use wg_2024::drone::{Drone};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, FloodRequest, Fragment, Nack, NackType, Packet, PacketType};
use fungi_drone::FungiDrone;
use bagel_bomber::BagelBomber;
use Krusty_Club::Krusty_C;
use skylink::SkyLinkDrone;
use LeDron_James::Drone as Le_Drone;
use lockheedrustin_drone::LockheedRustin;
use wg_2024_rust::drone::RustDrone;
use rustbusters_drone::RustBustersDrone;
use rusteze_drone::RustezeDrone;
use rustafarian_drone::RustafarianDrone;





lazy_static! { static ref CONSOLE_MUTEX: Arc<Mutex<()>> = Arc::new(Mutex::new(())); }
#[derive(Clone)]
struct SimulationController {
    drones: HashMap<NodeId, Sender<DroneCommand>>,
    packet_channel: HashMap<NodeId, Sender<Packet>>,
    node_event_recv: Receiver<DroneEvent>,
    neighbours: HashMap<NodeId, Vec<NodeId>>,
}



impl SimulationController {
    fn run(&mut self) {
        loop{
            select_biased! {
            recv(self.node_event_recv) -> command =>{
                if let Ok(drone_event) = command {
                    let _lock = CONSOLE_MUTEX.lock().unwrap();
                     match drone_event{
                        DroneEvent::PacketSent(ref packet) => {
                            println!("Simulation control: drone sent packet");
                        }
                        DroneEvent::PacketDropped(ref packet) => {
                            println!("Simulation control: drone dropped packet");
                        }
                        DroneEvent::ControllerShortcut(ref controller_shortcut) => {
                            println!("Simulation control: packet sent to destination");
                        }
                    }
                    self.handle_event(drone_event.clone());
                }

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
        // print!("  source id: {:#?}  |", packet.routing_header.hops[0]);
        // print!("  destination id: {:#?}  |", packet.routing_header.hops[packet.routing_header.hops.len() - 1]);
        // print!("  path: [");
        // for i in 0..packet.routing_header.hops.len()-1 {
        //     print!("{}, ", packet.routing_header.hops[i]);
        // }
        // println!("{}]", packet.routing_header.hops[packet.routing_header.hops.len() - 1]);
    }
    fn send_to_destination(&mut self, mut packet: Packet) {
        let addr = packet.routing_header.hops[packet.routing_header.hops.len() - 1];
        self.print_packet(packet.clone());
        packet.routing_header.hop_index = packet.routing_header.hops.len()-1;

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
        let nghb = self.neighbours.get(&id).unwrap();
        for neighbour in nghb.iter(){
            if let Some(sender) = self.drones.get(&neighbour) {
                sender.send(DroneCommand::RemoveSender(id)).unwrap();
            }
        }

        // Send the Crash command to the target drone
        if let Some(drone_sender) = self.drones.get(&id) {
            if let Err(err) = drone_sender.send(DroneCommand::Crash) {
                println!("Failed to send Crash command to drone {}: {:?}", id, err);
                return;
            }
           // println!("Sent Crash command to drone {}", id);
        } else {
            println!("No drone with ID {:?}", id);
            return;
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
        let next_hop=packet.routing_header.hops[packet.routing_header.hop_index +1];
        if let Some(sender) = self.packet_channel.get(&next_hop) {
            packet.routing_header.hop_index+=1;
            sender.send(packet).unwrap();
        }else{
            println!("No sender found for hop {}", next_hop);
        }
    }
    fn msg_fragment(&mut self, mut packet: Packet){
        println!("You're tying to send a message fragment");
        let mut next_hop=packet.routing_header.hops[packet.routing_header.hop_index+1];
        if let Some(sender) = self.packet_channel.get(&next_hop) {
            packet.routing_header.hop_index+=1;
            println!("Starting from hop_index: {} \n ",packet.routing_header.hop_index);
            sender.send(packet).unwrap();
        }
    }
    fn initiate_flood(&mut self, packet: Packet){
        println!("Initiating flood");
        if let PacketType::FloodRequest(flood_request)=packet.clone().pack_type {
            for node_neighbours in packet.clone().routing_header.hops{
                if let Some(sender) = self.packet_channel.get(&node_neighbours) {
                    sender.send(packet.clone()).unwrap();
                }else{
                    println!("No sender found for neighbours {:?}", node_neighbours);
                }
            }
        }else{
            println!("Unexpected error occurred, message wasn't a flood request");
        }
    }
}
pub fn parse_config(file: &str) -> Config {
    let file_str = fs::read_to_string(file).unwrap();
    toml::from_str(&file_str).unwrap()
}


pub fn test() {
    let config = parse_config("assets/configurations/double_chain.toml");
    let mut neighbours = HashMap::new();
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


    for (i, cfg_drone) in config.drone.into_iter().enumerate() {
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(cfg_drone.id, controller_drone_send);
        packet_drones.insert(cfg_drone.id, packet_channels[&cfg_drone.id].0.clone());


        let mut vec = Vec::new();
        for neigh in &cfg_drone.connected_node_ids {
            vec.push(*neigh);
        }
        neighbours.insert(cfg_drone.id, vec);

        let node_event_send = node_event_send.clone();
        let packet_recv = packet_channels[&cfg_drone.id].1.clone();
        let packet_send = cfg_drone
            .connected_node_ids
            .clone()
            .into_iter()
            .map(|nid| (nid, packet_channels[&nid].0.clone()))
            .collect::<HashMap<_, _>>();


        let handle = thread::spawn(move || {
            match i {
                0 => {
                    let mut drone = BagelBomber::new(
                        cfg_drone.id,
                        node_event_send,
                        controller_drone_recv,
                        packet_recv,
                        packet_send,
                        cfg_drone.pdr,
                    );
                    drone.run();
                }
                1 => {

                    let mut drone = FungiDrone::new(
                        cfg_drone.id,
                        node_event_send,
                        controller_drone_recv,
                        packet_recv,
                        packet_send,
                        cfg_drone.pdr,
                    );
                    drone.run();
                }
                2 => {

                    let mut drone = Krusty_C::new(
                        cfg_drone.id,
                        node_event_send,
                        controller_drone_recv,
                        packet_recv,
                        packet_send,
                        cfg_drone.pdr,
                    );
                    drone.run();
                }
                3=> {
                    let mut drone = SkyLinkDrone::new(
                        cfg_drone.id,
                        node_event_send,
                        controller_drone_recv,
                        packet_recv,
                        packet_send,
                        cfg_drone.pdr,
                    );
                    drone.run();
                }
                4=>{
                    let mut drone = Le_Drone::new(
                        cfg_drone.id,
                        node_event_send,
                        controller_drone_recv,
                        packet_recv,
                        packet_send,
                        cfg_drone.pdr,
                    );
                    drone.run();
                }
                5=>{
                    let mut drone = LockheedRustin::new(
                        cfg_drone.id,
                        node_event_send,
                        controller_drone_recv,
                        packet_recv,
                        packet_send,
                        cfg_drone.pdr,
                    );
                    drone.run();
                }
                6=>{
                    let mut drone = RustDrone::new(
                        cfg_drone.id,
                        node_event_send,
                        controller_drone_recv,
                        packet_recv,
                        packet_send,
                        cfg_drone.pdr,
                    );
                    drone.run();

                }
                7=>{
                    let mut drone = RustBustersDrone::new(
                        cfg_drone.id,
                        node_event_send,
                        controller_drone_recv,
                        packet_recv,
                        packet_send,
                        cfg_drone.pdr,
                    );
                    drone.run();
                }
                8=>{
                    let mut drone = RustezeDrone::new(
                        cfg_drone.id,
                        node_event_send,
                        controller_drone_recv,
                        packet_recv,
                        packet_send,
                        cfg_drone.pdr,
                    );
                    drone.run();
                }
                9=>{
                    let mut drone = RustafarianDrone::new(
                        cfg_drone.id,
                        node_event_send,
                        controller_drone_recv,
                        packet_recv,
                        packet_send,
                        cfg_drone.pdr,
                    );
                    drone.run();
                }
                _ => {

                    panic!("We only support 10 drones in this example.");
                }
            }
        });

        handles.push(handle);
    }

    let controller = Arc::new(Mutex::new(SimulationController {
        drones: controller_drones,
        node_event_recv: node_event_recv.clone(),
        neighbours,
        packet_channel: packet_drones,
    }));

    let controller_clone = Arc::clone(&controller);
    let controller_handle = thread::spawn(move || {
        let mut controller = controller_clone.lock().unwrap();
        controller.run()
    });


    let fragment_double_chain = create_fragments(vec![0,1,2,3,4,5,10,11]);
    {
        let mut controller = controller.lock().unwrap();
        controller.msg_fragment(fragment_double_chain);
    }

    for handle in handles {
        handle.join().unwrap();
    }
    controller_handle.join().unwrap();
}


fn create_fragments(hops: Vec<NodeId>) -> Packet {
    Packet {
        pack_type: PacketType::MsgFragment(Fragment {
            fragment_index: 1,
            total_n_fragments: 1,
            length: 1,
            data: [1; 128],
        }),
        routing_header: SourceRoutingHeader {
            hop_index: 0,
            hops,
        },
        session_id: 0,
    }
}
