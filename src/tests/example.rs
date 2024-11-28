#![allow(unused)]

use rand::Rng;
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::{fs,thread};
use wg_2024::config::Config;
use wg_2024::controller::{DroneCommand,NodeEvent};
use wg_2024::drone::{Drone, DroneOptions};
use wg_2024::network::NodeId;
use wg_2024::packet::{Packet, PacketType};

struct MyDrone {
    id: NodeId,
    controller_send: Sender<NodeEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pdr: f32,
    packet_send: HashMap<NodeId, Sender<Packet>>,
}

impl Drone for MyDrone {
    fn new(options: DroneOptions) -> Self {
        Self {
            id: options.id,
            controller_send: options.controller_send,
            controller_recv: options.controller_recv,
            packet_recv: options.packet_recv,
            pdr: options.pdr,
            packet_send: HashMap::new(),
        }
    }
    fn run(&mut self){
        loop{
            select_biased!{
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        if let DroneCommand::Crash = command {
                            println!("drone {} crashed", self.id);
                            break;
                        }else if let DroneCommand::SetPacketDropRate(x) = command {
                            println!("set_packet_drop_rate {}", x);
                            break;
                        }
                        self.handle_command(command);
                    }
                }
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        self.handle_packet(packet);
                    }
                },
            }
        }
    }
}
impl MyDrone {
    fn handle_packet(&mut self, packet: Packet) {
        match packet.pack_type {
            PacketType::Nack(_nack) => todo!(),
            PacketType::Ack(_ack) => todo!(),
            PacketType::MsgFragment(_fragment) => todo!(),
            PacketType::FloodRequest(_flood_request) => todo!(),
            PacketType::FloodResponse(_flood_response) => todo!(),
        }
    }
    fn handle_command(&mut self, command: DroneCommand) {
        match command {
            DroneCommand::AddSender(_node_id, _sender) => todo!(),
            DroneCommand::SetPacketDropRate(_pdr) => unreachable!(),
            DroneCommand::Crash => unreachable!(),
        }
    }
}
struct SimulationController {
    drones: HashMap<NodeId, Sender<DroneCommand>>,
    node_event_recv: Receiver<NodeEvent>,
}
impl SimulationController {
    fn crash_all(&mut self) {
        for (_, sender) in self.drones.iter() {
            sender.send(DroneCommand::Crash).unwrap();
        }
    }
    fn crash(&mut self, id : NodeId) {
        for (idd, sender) in self.drones.iter() {
            if idd == &id {
                sender.send(DroneCommand::Crash).unwrap();
            }
        }
    }
    fn pdr(&mut self, id : NodeId) {
        for (idd, sender) in self.drones.iter() {
            if idd == &id {
                let mut rng=rand::rng();
                let rand= rng.random_range(0.0..=100.0);
                sender.send(DroneCommand::SetPacketDropRate(rand)).unwrap()
            }
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
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_node_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        handles.push(thread::spawn(move || {
            let mut drone = MyDrone::new(DroneOptions {
                id: drone.id,
                controller_recv: controller_drone_recv,
                controller_send: node_event_send,
                packet_recv,
                packet_send,
                pdr: drone.pdr,
            });

            drone.run();
        }));
    }
    let mut controller = SimulationController {
        drones: controller_drones,
        node_event_recv,
    };
    controller.crash(1);
    controller.crash(2);
    controller.pdr(3);

    while let Some(handle) = handles.pop() {
        handle.join().unwrap();
    }
}

