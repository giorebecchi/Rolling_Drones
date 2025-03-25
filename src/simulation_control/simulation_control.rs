use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::{HashMap, HashSet};
use std::thread;
use std::sync::{Arc,Mutex};
use std::time::Duration;
use wg_2024::controller::{DroneCommand,DroneEvent};
use wg_2024::drone::{Drone};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Fragment, Packet, PacketType};
use bevy::prelude::{Res, ResMut};
use bagel_bomber::BagelBomber;
use fungi_drone::FungiDrone;
use Krusty_Club::Krusty_C;
use skylink::SkyLinkDrone;
use LeDron_James::Drone as Le_Drone;
use lockheedrustin_drone::LockheedRustin;
use wg_2024_rust::drone::RustDrone;
use rustbusters_drone::RustBustersDrone;
use rusteze_drone::RustezeDrone;
use rustafarian_drone::RustafarianDrone;
use wg_2024::packet::PacketType::FloodRequest;
use crate::clients::assembler::Fragmentation;
use crate::GUI::login_window::{ SharedSimState, SimulationController, UserConfig};
use crate::network_initializer::network_initializer::parse_config;
use crate::servers::ChatServer::Server;
use crate::clients::chat_client::ChatClient;
use crate::common_things::common::{ChatRequest, ChatResponse, CommandChat, ServerType};
use crate::common_things::common::ServerType::CommunicationServer;
use crate::servers::Text_max::Server as ServerMax;



impl Default for SimulationController{
    fn default() -> Self {
        let (sender, receiver) = unbounded();
        Self {
            node_event_send: sender,
            node_event_recv: receiver,
            drones: HashMap::new(),
            packet_channel: HashMap::new(),
            neighbours: HashMap::new(),
            client:  HashMap::new(),
            log : Arc::new(Mutex::new(SharedSimState::default())),
            seen_floods: HashSet::new()
        }
    }
}



impl SimulationController {
    fn run(&mut self) {
        let mut flood_req_hash=HashSet::new();
        loop{
            select_biased! {
            recv(self.node_event_recv) -> command =>{
                if let Ok(drone_event) = command {
                     match drone_event{
                        DroneEvent::PacketSent(ref packet) => {
                                let mut state=self.log.lock().unwrap();
                                match packet.pack_type.clone(){
                                    FloodRequest(flood_req)=>{
                                        if flood_req_hash.insert((flood_req.initiator_id,flood_req.flood_id)){
                                            state.log.push_str(&format!("{:?} with id {} has initiated a flood with id {}\n",flood_req.path_trace[0].1,flood_req.initiator_id,flood_req.flood_id));
                                            thread::sleep(Duration::from_millis(100));

                                        }
                                    }
                                    _=>{}
                                }
                        }
                        DroneEvent::PacketDropped(ref packet) => {
                            println!("Simulation control: drone dropped packet");
                        }
                        DroneEvent::ControllerShortcut(ref controller_shortcut) => {

                            println!("Simulation control: packet {:?} sent to destination",controller_shortcut.pack_type);
                        }
                    }

                    self.handle_event(drone_event.clone());
                }

            }
        }
        }

    }

    fn handle_event(&mut self, command: DroneEvent) {


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




    pub fn crash_all(&mut self) {
        for (_, sender) in self.drones.iter() {
            sender.send(DroneCommand::Crash).unwrap();
            println!("Sent Crash");
        }
    }
    pub fn crash(&mut self, id: NodeId) {
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

    pub fn pdr(&mut self, id : NodeId, pdr: f32) {
        for (idd, sender) in self.drones.iter() {
            if idd == &id {
                println!("pdr of drone {idd} changed to {pdr}");
                sender.send(DroneCommand::SetPacketDropRate(pdr)).unwrap()
            }
        }
    }
    pub fn add_sender(&mut self, dst_id: NodeId, nghb_id: NodeId) {
       let sender=self.packet_channel.get(&dst_id).unwrap().clone();
        if let Some(drone_sender) = self.drones.get(&nghb_id) {
            if let Err(err) = drone_sender.send(DroneCommand::AddSender(dst_id, sender)) {
                println!(
                    "Failed to send AddSender command to drone {}: {:?}",
                    nghb_id, err
                );
            }
        } else {
            println!("No drone found with ID {}", nghb_id);
        }
    }

    pub fn remove_sender(&mut self, drone_id: NodeId, nghb_id: NodeId) {
        if let Some(drone_sender) = self.drones.get(&drone_id) {
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
        let next_hop=packet.routing_header.hops[packet.routing_header.hop_index+1];
        if let Some(sender) = self.packet_channel.get(&next_hop) {
            packet.routing_header.hop_index+=1;
            println!("Starting from hop_index: {} \n ",packet.routing_header.hop_index);
            sender.send(packet).unwrap();
        }
    }
    pub fn initiate_flood(&mut self, packet: Packet){
        println!("Initiating flood");
        if let FloodRequest(_)=packet.clone().pack_type {
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
    pub fn spawn_new_drone(&mut self, links: Vec<NodeId>, id: NodeId){
        let node_event_send_clone=self.node_event_send.clone();
        let (sender_command, recv_command)=unbounded();
        self.drones.insert(id, sender_command);
        let mut packet_channels = HashMap::new();
        packet_channels.insert(id, unbounded());
        for id in links.clone(){
            packet_channels.insert(id, unbounded());
        }
        let packet_recv = packet_channels[&id].1.clone();
        let packet_send = links.iter().map(|nid|(*nid, packet_channels[nid].0.clone())).collect::<HashMap<_,_>>();
        thread::spawn(move || {
            let mut drone = create_drone(
                id,
                node_event_send_clone,
                recv_command,
                packet_recv,
                packet_send,
                0.0
            );
            if let Some(mut drone) = drone {
                drone.run();
            }
        });
    }
    pub fn send_message(&mut self, message: String, client_id: NodeId, destination_client: NodeId){
        self.client.get(&client_id).unwrap().send(CommandChat::SendMessage(destination_client, 12, message)).unwrap()
    }
    pub fn register_client(&mut self, client_id: NodeId, server_id: NodeId){
        self.client.get(&client_id).unwrap().send(CommandChat::RegisterClient(server_id)).unwrap();
    }
}
pub fn start_simulation(
    mut simulation_controller: ResMut<SimulationController>
) {
    let file_path = "assets/configurations/double_chain.toml";

    let config = parse_config(file_path);

    let mut packet_channels = HashMap::new();
    let mut command_chat_channel = HashMap::new();

    for node_id in config.drone.iter().map(|d| d.id)
        .chain(config.client.iter().map(|c| c.id))
        .chain(config.server.iter().map(|s| s.id)) {
        packet_channels.insert(node_id, unbounded());
    }

    for client in &config.client {
        command_chat_channel.insert(client.id, unbounded());
    }

    let mut neighbours = HashMap::new();
    for drone in &config.drone {
        neighbours.insert(drone.id, drone.connected_node_ids.clone());
    }

    let mut controller_drones = HashMap::new();
    let mut packet_drones = HashMap::new();
    let node_event_send = simulation_controller.node_event_send.clone();
    let node_event_recv = simulation_controller.node_event_recv.clone();
    let log = simulation_controller.log.clone();
    let mut client = simulation_controller.client.clone();


    for cfg_drone in config.drone.into_iter() {
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(cfg_drone.id, controller_drone_send);
        packet_drones.insert(cfg_drone.id, packet_channels[&cfg_drone.id].0.clone());

        let node_event_send_clone = node_event_send.clone();
        let packet_recv = packet_channels[&cfg_drone.id].1.clone();
        let packet_send = cfg_drone.connected_node_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_, _>>();

        thread::spawn(move || {
            let mut drone = create_drone(
                cfg_drone.id,
                node_event_send_clone,
                controller_drone_recv,
                packet_recv,
                packet_send,
                cfg_drone.pdr,
            );

            if let Some(mut drone) = drone {
                drone.run();
            }
        });
    }

    for cfg_server in config.server {
        let rcv = packet_channels[&cfg_server.id].1.clone();
        let packet_send = cfg_server.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect::<HashMap<_,_>>();

        let server_baia = Arc::new(Mutex::new(Server::new(cfg_server.id, rcv, packet_send)));
        // let mut server_max = ServerMax::new(cfg_server.id,rcv.clone(),packet_send);

        thread::spawn(move || {
            // server_max.run();
            server_baia.lock().unwrap().run();
        });
    }

    for cfg_client in config.client {
        let rcv_packet = packet_channels[&cfg_client.id].1.clone();
        let rcv_command = command_chat_channel[&cfg_client.id].1.clone();
        client.insert(cfg_client.id, command_chat_channel[&cfg_client.id].0.clone());

        let packet_send: HashMap<NodeId, Sender<Packet>> = cfg_client.connected_drone_ids.iter()
            .map(|nid| (*nid, packet_channels[nid].0.clone()))
            .collect();

        let client_instance = Arc::new(Mutex::new(ChatClient::new(
            cfg_client.id,
            rcv_packet,
            packet_send.clone(),
            rcv_command,
            packet_send,
        )));

        thread::spawn(move || {
            client_instance.lock().unwrap().run();
        });
    }

    simulation_controller.node_event_send = node_event_send.clone();
    simulation_controller.drones = controller_drones;
    simulation_controller.node_event_recv = node_event_recv;
    simulation_controller.neighbours = neighbours;
    simulation_controller.packet_channel = packet_drones;
    simulation_controller.client = client;

    let controller = Arc::new(Mutex::new(SimulationController {
        node_event_send,
        drones: simulation_controller.drones.clone(),
        node_event_recv: simulation_controller.node_event_recv.clone(),
        neighbours: simulation_controller.neighbours.clone(),
        packet_channel: simulation_controller.packet_channel.clone(),
        client: simulation_controller.client.clone(),
        log,
        seen_floods: HashSet::new(),
    }));

    thread::spawn(move || {
        controller.lock().unwrap().run();
    });

     // thread::sleep(Duration::from_millis(200)); //questo da scommentare sempre se vuoi testare
    // simulation_controller.client.get(&0).unwrap()
    //    .send(CommandChat::SendMessage(11, 12, "ciao".to_string()))
    //    .unwrap();
    // simulation_controller.client.get(&11).unwrap() //scommenta questo che abbiamo i print
    //     .send(CommandChat::ServerType(12)).unwrap()
}

fn create_drone(
    id: NodeId,
    node_event_send: Sender<DroneEvent>,
    controller_drone_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<u8, Sender<Packet>>,
    pdr: f32,
) -> Option<Box<dyn Drone>> {
    match id % 10 {
        0 => Some(Box::new(BagelBomber::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        1 => Some(Box::new(FungiDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        2 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        3 => Some(Box::new(SkyLinkDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        4 => Some(Box::new(Le_Drone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        5 => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        6 => Some(Box::new(RustDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        7 => Some(Box::new(RustBustersDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        8 => Some(Box::new(RustezeDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        9 => Some(Box::new(RustafarianDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
        _ => Some(Box::new(LockheedRustin::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr))),
    }
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
