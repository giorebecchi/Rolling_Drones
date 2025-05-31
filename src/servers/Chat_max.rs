use crate::servers::utilities_max::*;
use crate::common_things::common::*;
use crate::common_things::common::ServerType;
use crossbeam_channel::{select_biased, Receiver, Sender};
use std::collections::{BinaryHeap, HashMap};
use bevy::render::render_resource::encase::private::RuntimeSizedArray;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, FloodRequest, Fragment, Nack, NackType, NodeType, Packet, PacketType};
use bevy::utils::HashSet;


pub struct Server{
    server_id: NodeId,
    server_type: ServerType,
    nodes_map: Vec<(NodeId, NodeType, Vec<NodeId>)>,
    fragment_recv: HashMap<u64, Data>,
    fragment_send: HashMap<u64, Data>,
    packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    already_visited: HashSet<(NodeId, u64)>,
    registered_clients: Vec<NodeId>,
    rcv_flood: Receiver<BackGroundFlood>,
    rcv_command: Receiver<ServerCommands>,
    send_event: Sender<ServerEvent>
}
impl Server {
    pub fn new(id: NodeId, packet_recv: Receiver<Packet>, packet_send: HashMap<NodeId, Sender<Packet>>, rcv_flood: Receiver<BackGroundFlood>, rcv_command: Receiver<ServerCommands>, send_event: Sender<ServerEvent>) -> Self {
        let mut links: Vec<NodeId> = Vec::new();
        for i in packet_send.clone() {
            links.push(i.0.clone());
        }
        let n = NodeType::Server;
        Server {
            server_id: id,
            server_type: ServerType::CommunicationServer,
            nodes_map: vec![(id,n, links)],
            fragment_recv: HashMap::new(),
            fragment_send: HashMap::new(),
            packet_recv,
            packet_send,
            already_visited: HashSet::new(),
            registered_clients: Vec::new(),
            rcv_flood,
            rcv_command,
            send_event
        }
    }
    pub fn run(&mut self) {
        self.floading();
        loop {
            select_biased! {
                    recv(self.packet_recv) -> packet => {
                        if let Ok(packet) = packet {
                            self.handle_packet(packet);
                        }
                        else{
                            break
                        }
                    },
                    recv(self.rcv_flood) -> flood => {
                        if let Ok(_) = flood {
                         self.floading();
                        }
                    },
                    recv(self.rcv_command) -> sc_command => {
                        if let Ok(command) = sc_command {
                            match command {
                                ServerCommands::SendTopologyGraph=>{
                                   // self.send_topology_graph();
                                },
                                ServerCommands::AddSender(id, sender)=>{
                                //todo
                                },
                                ServerCommands::RemoveSender(id)=>{
                                //todo
                                },
                                ServerCommands::TopologyChanged=>{
                                //todo
                                }
                            }
                        }
                    }
                }
        }
    }

    fn handle_packet(&mut self, packet: Packet) {
        let p = packet.clone();
        match packet.pack_type {
            PacketType::MsgFragment(fragment) => {
                let session = packet.session_id;
                self.handle_message(fragment, &session, p);
            }
            PacketType::Ack(ack) => {
                let session = packet.session_id;
                self.handle_ack(ack, &session)
            }
            PacketType::Nack(nack) => {
                let session = packet.session_id;
                let position = nack.fragment_index;
                self.handle_nack(nack, &position, &session)
            }
            PacketType::FloodRequest(_) => {
                self.handle_flood_request(p)
            }
            PacketType::FloodResponse(_) => {
                self.handle_flood_response(p);
            }
        }
    }
    fn handle_message(&mut self, fragment: Fragment, session: &u64, packet: Packet) {
        let ack = create_ack(packet.clone());
        self.send_packet(ack);
        if let Some(boxed) = self.fragment_recv.get_mut(&session) {
            let d = fragment.data;
            let number = fragment.length;
            boxed.dati[fragment.fragment_index as usize] = (d, number);
            boxed.counter += 1;
            if boxed.counter == fragment.total_n_fragments {
                self.handle_command(session);
            }
        } else {
            let asker = packet.routing_header.hops[0];
            let number = fragment.length;
            let d = (fragment.data, number);
            let data = Data::new(d, fragment.fragment_index, fragment.total_n_fragments, 1, asker);
            self.fragment_recv.insert(*session, data);
            if let Some(boxed) = self.fragment_recv.get_mut(&session) {
                if boxed.counter == fragment.total_n_fragments {
                    self.handle_command(session);
                }
            }
        }
    }
    fn handle_ack(&mut self, fragment: Ack, session: &u64) {
        if let Some(boxed) = self.fragment_send.get_mut(&session) {
            boxed.counter -= 1;
            if boxed.counter == 0 {
                self.fragment_send.remove(session);
            }
        }
    }
    fn handle_nack(&mut self, fragment: Nack, position: &u64, session: &u64) {
        if let Some(boxed) = self.fragment_send.get_mut(&session) {
            match fragment.nack_type {
                NackType::ErrorInRouting(id) => {
                    let destination = boxed.who_ask;
                    self.remove_drone(id);
                    let root = self.routing(destination);
                    match root {
                        None => {
                            println!("path not found");
                        }
                        Some(root) => {
                            println!("{:?}", root.clone());
                            let total = self.fragment_send.get(&session).unwrap().dati.len() - 1;
                            let source = SourceRoutingHeader::new(root, 1);
                            let nacked = fragment.fragment_index;
                            let fr = Fragment::new(*position, total as u64, self.fragment_send.get(&session).unwrap().dati[nacked as usize].0);
                            let pack = Packet::new_fragment(source, *session, fr);
                            self.send_packet(pack);
                            return;
                        }
                    }
                }
                NackType::Dropped => {
                    let destination = boxed.who_ask;
                    let root = self.routing(destination);
                    match root {
                        None => {
                            println!("path not found");
                        }
                        Some(root) => {
                            let total = self.fragment_send.get(&session).unwrap().dati.len() - 1;
                            let source = SourceRoutingHeader::new(root, 1);
                            let nacked = fragment.fragment_index;
                            let fr = Fragment::new(*position, total as u64, self.fragment_send.get(&session).unwrap().dati[nacked as usize].0);
                            let pack = Packet::new_fragment(source, *session, fr);
                            self.send_packet(pack);
                            return;
                        }
                    }
                }
                _ => { unreachable!() }
            }
        }
    }
    fn handle_flood_request(&mut self, packet: Packet) {
        if let PacketType::FloodRequest(mut flood) = packet.pack_type {
            if self.already_visited.contains(&(flood.initiator_id, flood.flood_id)) {
                flood.path_trace.push((self.server_id, NodeType::Server));
                let response = FloodRequest::generate_response(&flood, packet.session_id);
                self.send_packet(response);
                return;
            } else {
                self.already_visited.insert((flood.initiator_id, flood.flood_id));
                if self.packet_send.len() == 1 {
                    flood.path_trace.push((self.server_id, NodeType::Server));
                    let response = FloodRequest::generate_response(&flood, packet.session_id);
                    self.send_packet(response);
                } else {
                    flood.path_trace.push((self.server_id, NodeType::Server));
                    let new_packet = Packet {
                        pack_type: PacketType::FloodRequest(flood.clone()),
                        routing_header: packet.routing_header,
                        session_id: packet.session_id,
                    };
                    let (previous, _) = flood.path_trace[flood.path_trace.len() - 2];
                    for (idd, neighbour) in self.packet_send.clone() {
                        if idd == previous {} else {
                            neighbour.send(new_packet.clone()).unwrap();
                        }
                    }
                }
            }
        }
    }
    fn handle_flood_response(&mut self, packet: Packet) {
        if let PacketType::FloodResponse(flood_response) = packet.pack_type {
            let len = flood_response.path_trace.len();
            if len < 2 {
                return; // Se la traccia è troppo corta, non ci sono connessioni da aggiungere
            }


            for i in 0..len {
                let (current_node, current_type) = flood_response.path_trace[i];
                let prev_node = if i > 0 { Some(flood_response.path_trace[i - 1].0) } else { None };
                let next_node = if i < len - 1 { Some(flood_response.path_trace[i + 1].0) } else { None };


                // Trova o aggiunge il nodo corrente con il suo tipo
                if let Some(k) = self.nodes_map.iter_mut().find(|(id, _, _)| *id == current_node) {
                    // Aggiorna il tipo di nodo se già esiste (assumiamo che il tipo non cambi)
                    if k.1 != current_type {
                        k.1 = current_type;
                    }


                    // Aggiunge i collegamenti se non già presenti
                    if let Some(prev) = prev_node {
                        if !k.2.contains(&prev) {
                            k.2.push(prev);
                        }
                    }
                    if let Some(next) = next_node {
                        if !k.2.contains(&next) {
                            k.2.push(next);
                        }
                    }
                } else {
                    // Crea un nuovo nodo con i collegamenti
                    let mut connections = Vec::new();
                    if let Some(prev) = prev_node {
                        connections.push(prev);
                    }
                    if let Some(next) = next_node {
                        connections.push(next);
                    }
                    self.nodes_map.push((current_node, current_type, connections));
                }
            }
        }
    }
    fn remove_drone(&mut self, node_id: NodeId) {
        self.nodes_map.retain(|(id,_ , _)| *id != node_id);
        for (_,_, neighbors) in &mut self.nodes_map {
            neighbors.retain(|&neighbor_id| neighbor_id != node_id);
        }
    }
    fn send_packet(&mut self, mut packet: Packet) {
        if packet.routing_header.hops.len() < 2 {
            return; // Nessun nodo successivo disponibile
        }
        packet.routing_header.hop_index = 1;
        let next = packet.routing_header.hops[1];
        // Invia il pacchetto al prossimo nodo
        if let Some(sender) = self.packet_send.get_mut(&next) {
            if let Err(e) = sender.send(packet) {
                eprintln!("Errore nell'invio del pacchetto a {}: {:?}", next, e);
            }
        }
    }
    fn routing(&self, destination: NodeId) -> Option<Vec<NodeId>> {
        let mut table: HashMap<NodeId, (i64, Option<NodeId>)> = HashMap::new();
        let mut queue: BinaryHeap<State> = BinaryHeap::new();
        // Inizializza la tabella delle distanze
        for (node_id, _, _) in &self.nodes_map {
            table.insert(*node_id, (i64::MAX, None));
        }
        table.insert(self.server_id, (0, None));
        // Inseriamo il nodo sorgente nella coda
        queue.push(State { node: self.server_id, cost: 0 });


        while let Some(State { node, cost }) = queue.pop() {
            // Se abbiamo trovato la destinazione, possiamo ricostruire il percorso
            if node == destination {
                let mut path = Vec::new();
                let mut current = destination;
                while let Some(prev) = table.get(&current).and_then(|&(_, prev)| prev) {
                    path.push(current);
                    current = prev;
                }
                path.push(self.server_id);
                path.reverse();
                return Some(path);
            }


            // Se il costo attuale è maggiore di quello già registrato, saltiamo
            if cost > table.get(&node)?.0 {
                continue;
            }


            // Iteriamo sui vicini
            if let Some((_, _, neighbors)) = self.nodes_map.iter().find(|(id, _, _)| *id == node) {
                for &neighbor in neighbors {
                    // Controllo: i nodi intermedi (tranne il primo e l'ultimo) devono essere droni
                    if neighbor != destination && neighbor != self.server_id {
                        if let Some((_, neighbor_type, _)) = self.nodes_map.iter().find(|(id, _, _)| *id == neighbor) {
                            if *neighbor_type != NodeType::Drone {
                                continue; // Salta il nodo se non è un drone
                            }
                        }
                    }


                    let new_cost = cost + 1; // Supponiamo un costo uniforme di 1 per ogni collegamento
                    if new_cost < table.get(&neighbor).unwrap_or(&(i64::MAX, None)).0 {
                        table.insert(neighbor, (new_cost, Some(node)));
                        queue.push(State { node: neighbor, cost: new_cost });
                    }
                }
            }
        }


        // Se il ciclo termina senza trovare la destinazione, non esiste un percorso valido
        None
    }
    fn floading(&self) {
        println!("server {} is starting a flooding", self.server_id);
        let flood_id = 0;
        let flood = Packet {
            routing_header: SourceRoutingHeader { hop_index: 1, hops: Vec::new() },
            session_id: flood_id,
            pack_type: PacketType::FloodRequest(FloodRequest {
                flood_id,
                initiator_id: self.server_id,
                path_trace: vec![(self.server_id, NodeType::Server)],
            }),
        };
        for (id, neighbour) in self.packet_send.clone() {
            if id == self.server_id {} else {
                neighbour.send(flood.clone()).unwrap();
                println!("fload request mandata a {}", id)
            }
        }
    }
    fn handle_command(&mut self, session: &u64) {
        let data = self.fragment_recv.get(session).unwrap();
        let d = data.dati.clone();
        let command: ComandoChat = deserialize_comando_chat(d);
        let id_client = data.who_ask;
        match command{
            ComandoChat::Client(reuquest) => {
                match reuquest{
                    ChatRequest::ServerType => {
                        let response = Risposta::Chat(ChatResponse::ServerTypeChat(self.server_type.clone()));
                        self.send_response(id_client, response, session)
                    }
                    ChatRequest::RegisterClient(client_id) => {
                        self.registered_clients.push(client_id);
                        let response = Risposta::Chat(ChatResponse::RegisterClient(true));
                        self.send_response(id_client, response, session);
                    }
                    ChatRequest::GetListClients => {
                        let response = Risposta::Chat(ChatResponse::RegisteredClients(self.registered_clients.clone()));
                        self.send_response(id_client, response, session);
                    }
                    ChatRequest::SendMessage(message, _) => {
                        let sender = message.from_id;
                        let receiver = message.to_id;
                        let present = self.is_present(receiver, sender);
                        match present {
                            Ok(string) => {
                                let r1 = Risposta::Chat(ChatResponse::SendMessage(Ok(string)));
                                let r2 = Risposta::Chat(ChatResponse::ForwardMessage(message.clone()));
                                self.send_response(sender, r1, session);
                                self.send_response(receiver, r2, session);
                            }
                            Err(string) => {
                                let r1 = Risposta::Chat(ChatResponse::SendMessage(Err(string)));
                                self.send_response(sender, r1, session);
                            }
                        }
                    }
                    ChatRequest::EndChat(id) => {
                        self.registered_clients.retain(|&x| x != id);
                        let response = Risposta::Chat(ChatResponse::EndChat(true));
                        self.send_response(id_client, response, session);
                    }
                }
            }
            ComandoChat::Text(text) => {
                match text{
                    TextServer::ServerTypeReq => {
                        let response = Risposta::Chat(ChatResponse::ServerTypeChat(self.server_type.clone()));
                        self.send_response(id_client, response, session);
                    }
                    _ => {unreachable!()}
                }
            }
            ComandoChat::WebBrowser(req) => {
                match req {
                    WebBrowserCommands::GetServerType => {
                        //let response = Risposta::Chat(ChatResponse::ServerTypeChat(self.server_type.clone()));
                        //self.send_response(id_client, response, session);
                    }
                    _ => {}
                }
            }
        }
    }
    fn is_present(&self, receiver: NodeId, sender: NodeId) -> Result<String, String> {
        if self.registered_clients.contains(&sender) && self.registered_clients.contains(&receiver) {
            Ok("The server will forward the message to the final client".to_string())
        }else{
            Err("Error with the registration of the two involved clients".to_string())
        }
    }
    fn send_response(&mut self, id: NodeId, response: Risposta, session: &u64) {
        println!{"risposta: {:#?}", response}
        match response {
            Risposta::Chat(chat) => {
                let dati = serialize(&chat);
                let total = dati.len();
                let d_to_send = Data { counter: total as u64, dati, who_ask: id };
                self.fragment_send.insert(*session, d_to_send);
                let root = self.routing(id);
                match root {
                    None => {
                        println!("path not found")
                    }
                    Some(root) => {
                        let mut i = 0;
                        for fragment in self.fragment_send[session].dati.clone() {
                            let routing = SourceRoutingHeader { hop_index: 1, hops: root.clone() };
                            let f = Fragment { fragment_index: i as u64, total_n_fragments: total as u64, length: fragment.1, data: fragment.0 };
                            let p = Packet { routing_header: routing, session_id: *session, pack_type: PacketType::MsgFragment(f) };
                            self.send_packet(p);
                            i += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}