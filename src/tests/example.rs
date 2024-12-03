#![allow(unused)]

use rand::Rng;
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::{fs,thread};
use wg_2024::config::Config;
use wg_2024::controller::{DroneCommand,NodeEvent};
use wg_2024::drone::{Drone};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, Packet, PacketType};

struct MyDrone {
    id: NodeId,
    controller_send: Sender<NodeEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pdr: f32,
    packet_send: HashMap<NodeId, Sender<Packet>>,
}

impl Drone for MyDrone {
    fn new(id: NodeId,
           controller_send: Sender<NodeEvent>,
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
                            break;
                        },
                        DroneCommand::SetPacketDropRate(x) => {
                            self.pdr = x;
                            println!("set_packet_drop_rate {}", self.pdr);
                                break;
                        },
                        DroneCommand::AddSender(id, send_pack) => {
                            self.packet_send.insert(id, send_pack);
                            println!("added sender");
                                break;
                        },
                        DroneCommand::RemoveSender(id) => {
                                self.packet_send.remove(&id);
                                println!("removed sender {}", id);
                                break;
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
            DroneCommand::SetPacketDropRate(_pdr) => todo!(),
            DroneCommand::Crash => unreachable!(),
            DroneCommand::RemoveSender(_node_id) => todo!(),
        }
    }
}
struct SimulationController {
    drones: HashMap<NodeId, Sender<DroneCommand>>,
    packet_channel: HashMap<NodeId, Sender<Packet>>,
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

