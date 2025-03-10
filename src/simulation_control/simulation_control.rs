use rand::Rng;
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
use crate::GUI::login_window::{SharedSimState, SimulationController, UserConfig};
use crate::network_initializer::network_initializer::parse_config;
use crate::servers::ChatServer::Server;
use crate::clients::chat_client::ChatClient;
use crate::common_things::common::{ChatRequest, ChatResponse, CommandChat, ServerType};
use crate::common_things::common::ServerType::ComunicationServer;



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




    fn crash_all(&mut self) {
        for (_, sender) in self.drones.iter() {
            sender.send(DroneCommand::Crash).unwrap();
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
    pub fn initiate_flood(&mut self, packet: Packet){
        println!("Initiating flood");
        if let FloodRequest(flood_request)=packet.clone().pack_type {
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
pub fn test_gui(mut simulation_controller: ResMut<SimulationController>, config: Res<UserConfig>){

}


pub fn test(mut simulation_controller: ResMut<SimulationController>, config: Res<UserConfig>) {
    let file_path= match (*config).0.as_str(){
        "star"=>"assets/configurations/star.toml",
        "double_chain"=>"assets/configurations/double_chain.toml",
        "butterfly"=>"assets/configurations/butterfly.toml",
        "tree"=>"assets/configurations/tree.toml",
        _=>"assets/configurations/star.toml",
    };
    let config = parse_config(file_path);
    let mut neighbours = simulation_controller.neighbours.clone();
    let mut controller_drones = simulation_controller.drones.clone();
    let mut packet_drones = simulation_controller.packet_channel.clone();
    let mut node_event_send = simulation_controller.node_event_send.clone();
    let mut node_event_recv=simulation_controller.node_event_recv.clone();
    let mut log = simulation_controller.log.clone();


    let mut packet_channels = HashMap::new();
    let mut command_chat_channel = HashMap::new();
    let mut client= simulation_controller.client.clone();


    for drone in config.drone.iter() {
        packet_channels.insert(drone.id, unbounded());
    }
    for client in config.client.iter() {
        packet_channels.insert(client.id, unbounded());
        command_chat_channel.insert(client.id, unbounded());
    }
    for server in config.server.iter() {
        packet_channels.insert(server.id, unbounded());
    }



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

        thread::spawn(move || {
            match i {
                0 => {
                    let mut drone = BagelBomber::new(//BagelBomber
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

                    let mut drone = FungiDrone::new(//Fungi--Problema nella flooding
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

                    let mut drone = Krusty_C::new(//Krusty_C
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
                    let mut drone = SkyLinkDrone::new(//SkyLink
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
                    let mut drone = Le_Drone::new(//Le_Drone--Problema nella flooding
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
                    let mut drone = LockheedRustin::new(//LockheedRustin
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
                    let mut drone = RustDrone::new(//RustDrone
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
                    let mut drone = RustBustersDrone::new(//RustBustersDrone
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
                    let mut drone = RustezeDrone::new(//RustezeDrone
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
                    let mut drone = RustafarianDrone::new(//RustafarianDrone--Problema con flooding e "Environment variable RUSTAFARIAN_LOG_LEVEL couldn't be read,logger will be disabled by default"
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

    }

    for  cfg_server in config.server.into_iter() {
        let rcv = packet_channels[&cfg_server.id].1.clone();
        let packet_send = cfg_server
            .connected_drone_ids
            .clone()
            .into_iter()
            .map(|nid| (nid, packet_channels[&nid].0.clone()))
            .collect::<HashMap<_, _>>();
        let server = Arc::new(Mutex::new(Server::new(cfg_server.id,rcv,packet_send)));
        let mut server_clone = Arc::clone(&server);
        thread::spawn(move || {
            let mut server = server_clone.lock().unwrap();
            server.run();
        });
    }

    for cfg_client in config.client.into_iter() {
        let rcv_packet = packet_channels[&cfg_client.id].1.clone();
        let rcv_command: Receiver<CommandChat> = command_chat_channel[&cfg_client.id].1.clone();
        client.insert(cfg_client.id,command_chat_channel[&cfg_client.id].0.clone());
        let packet_send =
            cfg_client
            .connected_drone_ids
            .clone()
            .into_iter()
            .map(|nid| (nid, packet_channels[&nid].0.clone()))
            .collect::<HashMap<_, _>>();
        let control_send = cfg_client
            .connected_drone_ids
            .clone()
            .into_iter()
            .map(|node_id| (node_id, packet_channels[&node_id].0.clone()))
            .collect::<HashMap<_, _>>();

        let mut client = Arc::new(Mutex::new(ChatClient::new(cfg_client.id, rcv_packet, packet_send, rcv_command, control_send )));
        let mut client_clone = Arc::clone(&client);
        let handle = thread::spawn(move || {
            let mut client = client_clone.lock().unwrap();
            client.run();
        });
        {
             let mut client_try=client.lock().unwrap();
             // client_try.initiate_flooding()
            // let message = ChatResponse::ServerType(ServerType::ComunicationServer); //do it for both clients(?)
            // let fragmented_message = message.fragment_message();
            // let vec_packet = ChatResponse::create_packet(&fragmented_message, vec![0, 1, 12], & mut 0 );
            // println!("{:?}", vec_packet);
            // for pack in vec_packet{
            //     client_try.handle_fragments(pack);
            // }
        }
        // handles.push(handle);
    }


    let controller = Arc::new(Mutex::new(SimulationController {
        node_event_send: node_event_send.clone(),
        drones: controller_drones.clone(),
        node_event_recv: node_event_recv.clone(),
        neighbours: neighbours.clone(),
        packet_channel: packet_drones.clone(),
        client,
        log: log.clone(),
        seen_floods: HashSet::new()
    }));
    simulation_controller.node_event_send= node_event_send.clone();
    simulation_controller.drones=controller_drones.clone();
    simulation_controller.node_event_recv= node_event_recv.clone();
    simulation_controller.neighbours=neighbours;
    simulation_controller.packet_channel=packet_drones;


    let controller_clone = Arc::clone(&controller);
    thread::spawn(move || {
        let mut controller = controller_clone.lock().unwrap();
        controller.run()
    });

    //let fragment_double_chain = create_fragments(vec![0,1,2,3,4,5,10,11]);
    {

        let mut controller = controller.lock().unwrap();
        thread::sleep(Duration::from_millis(200));
          // controller.client.get(&0).unwrap().send(CommandChat::SendMessage(11,12,"ciao".to_string())).unwrap();
           controller.client.get(&0).unwrap().send(CommandChat::ServerType(11)).unwrap(); //star topology


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
