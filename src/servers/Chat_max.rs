use crossbeam_channel::select_biased;
use crate::servers::utilities_max::*;
use crate::common_things::common::*;
use crate::common_things::common::ServerType;
use crossbeam_channel::{select, Receiver, Sender};
use std::collections::{BinaryHeap, HashMap};
use bevy::render::render_resource::encase::private::RuntimeSizedArray;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType};
use bevy::utils::HashSet;
use crate::simulation_control::simulation_control::MyNodeType;

pub struct Server {
    server_id: NodeId,
    server_type: ServerType,
    next_session_id: u64,
    nodes_map: Vec<(NodeId, NodeType, Vec<NodeId>)>,
    processed_sessions: HashSet<(NodeId, u64)>,
    fragment_recv: HashMap<(NodeId, u64), Data>,
    fragment_send: HashMap<u64, Data>,
    packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    already_visited: HashSet<(NodeId, u64)>,
    registered_clients: Vec<NodeId>,
    rcv_flood: Receiver<BackGroundFlood>,
    rcv_command: Receiver<ServerCommands>,
    send_event: Sender<ServerEvent>,
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
            next_session_id: 10_000,
            nodes_map: vec![(id,n, links)],
            processed_sessions: HashSet::new(),
            fragment_recv: HashMap::new(),
            fragment_send: HashMap::new(),
            packet_recv,
            packet_send,
            already_visited: HashSet::new(),
            registered_clients: Vec::new(),
            rcv_flood,
            rcv_command,
            send_event,
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
                                   let _ = self.send_event.send(ServerEvent::GraphMax(self.server_id, self.nodes_map.clone()));
                                },
                                ServerCommands::AddSender(id, sender)=>{
                                    self.packet_send.insert(id, sender);
                                    self.floading()
                                },
                                ServerCommands::RemoveSender(id)=>{
                                    self.remove_drone(id);
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
                self.handle_ack(ack, session)
            }
            PacketType::FloodRequest(_) => {
                self.handle_flood_request(p)
            }
            PacketType::FloodResponse(_) => {
                self.handle_flood_response(p);
            }
            PacketType::Nack(nack) => {
                let session = packet.session_id;
                let position = nack.fragment_index;
                self.handle_nack(nack, &position, &session);
            }
        }
    }
    fn handle_message(&mut self, fragment: Fragment, session: &u64, packet: Packet) {
        // 1) ACK immediato
        let ack = create_ack(packet.clone());
        self.send_packet(ack);

        // 2) Estrai index, total e chi chiede
        let total_frags = fragment.total_n_fragments as usize;
        let frag_idx    = fragment.fragment_index       as usize;
        if frag_idx >= total_frags { return; }

        let who_ask = match packet.routing_header.hops.get(0).cloned() {
            Some(h) => h,
            None    => return,
        };
        let session_key = (who_ask, *session);

        // 3) Inserisco/aggiorno il dato nel buffer
        let entry = self.fragment_recv.entry(session_key)
            .or_insert_with(|| Data::new(
                ([0u8; 128], 0u8),      // placeholder per ogni slot
                0,                      // posizione iniziale dummy
                total_frags as u64,     // numero totale di frammenti
                0,                      // contatore iniziale = 0
                who_ask,                // chi chiede
            ));

        // 4) Se non ricevuto prima, memorizzo
        if entry.dati[frag_idx].1 == 0 {
            entry.dati[frag_idx] = (fragment.data, fragment.length);
            entry.counter += 1;
        }

        // 5) Se completo, processo e pulisco
        if entry.counter == entry.total_expected as u64 {
            self.processed_sessions.insert(session_key);
            self.handle_command(session_key);
            self.fragment_recv.remove(&session_key);
        }
    }

    fn handle_ack(&mut self, _ack: Ack, session: u64) {
        if let Some(data_struct) = self.fragment_send.get_mut(&session) {
            if data_struct.counter > 0 {
                data_struct.counter -= 1;
                if data_struct.counter == 0 {
                    drop(data_struct);
                    self.fragment_send.remove(&session);
                }
            } else {}
        } else {}
    }
    fn handle_nack(&mut self, fragment: Nack, position: &u64, session: &u64) {
        if !self.fragment_send.contains_key(session) {
            return;
        }
        let data_struct = self.fragment_send.get(session).unwrap();
        let destination = data_struct.who_ask;
        let buf_len = data_struct.dati.len();

        if buf_len == 0 {
            return;
        }

        let nacked_idx = fragment.fragment_index as usize;
        if nacked_idx >= buf_len {
            return;
        }

        let chunk_data = data_struct.dati[nacked_idx].0.clone();
        match fragment.nack_type {
            NackType::ErrorInRouting(bad_node) => {
                self.remove_drone(bad_node);
            }
            NackType::Dropped => {
                // Non rimuovo nodi, continuo a ritrasmettere
            }
            _ => {
                return;
            }
        }
        if let Some(root) = self.routing(destination) {
            let total = buf_len as u64;
            let source = SourceRoutingHeader::new(root.clone(), 1);
            let fr = Fragment::new(*position, total, chunk_data);
            let pack = Packet::new_fragment(source, *session, fr);
            self.send_packet(pack);
        }
        else {}
    }
    fn handle_flood_request(&mut self, packet: Packet) {
        if let PacketType::FloodRequest(mut flood) = packet.pack_type {
            // Se già visitato: rispondo subito
            let key = (flood.initiator_id, flood.flood_id);
            if self.already_visited.contains(&key) {
                flood.path_trace.push((self.server_id, NodeType::Server));
                let response = FloodRequest::generate_response(&flood, packet.session_id);
                self.send_packet(response);
                return;
            }

            // Altrimenti segno come visitato e proseguo
            self.already_visited.insert(key.clone());
            flood.path_trace.push((self.server_id, NodeType::Server));

            if self.packet_send.len() == 1 {
                // ultimo nodo: mando risposta
                let response = FloodRequest::generate_response(&flood, packet.session_id);
                self.send_packet(response);
            } else {
                // inoltro a tutti tranne il precedente
                let prev = packet.routing_header
                    .hops
                    .get(packet.routing_header.hop_index as usize - 1)
                    .cloned();

                let new_packet = Packet {
                    pack_type: PacketType::FloodRequest(flood.clone()),
                    routing_header: packet.routing_header.clone(),
                    session_id: packet.session_id,
                };

                for (idd, mut neighbour) in self.packet_send.clone() {
                    if Some(idd) == prev {
                        continue;
                    }
                    if let Err(e) = neighbour.send(new_packet.clone()) {
                        log::warn!("handle_flood_request: forward a {} fallito: {:?}", idd, e);
                        self.packet_send.remove(&idd);
                    }
                }
            }
        }
    }
    fn handle_flood_response(&mut self, packet: Packet) {
        if let PacketType::FloodResponse(ref flood_response) = packet.pack_type {
            let path = &flood_response.path_trace;
            if path.is_empty() {
                return;
            }

            let initiator_id = path[0].0;
            let flood_key = (initiator_id, flood_response.flood_id);

            // Processiamo sempre (aggiorna la topologia)
            for i in 0..path.len() {
                let (node_id, node_type) = path[i];
                let prev = if i > 0 { Some(path[i - 1].0) } else { None };
                let next = if i + 1 < path.len() { Some(path[i + 1].0) } else { None };

                if let Some(entry) = self.nodes_map.iter_mut().find(|(id, _, _)| *id == node_id) {
                    if entry.1 != node_type {
                        entry.1 = node_type;
                    }
                    if let Some(p) = prev {
                        if !entry.2.contains(&p) {
                            entry.2.push(p);
                        }
                    }
                    if let Some(n) = next {
                        if !entry.2.contains(&n) {
                            entry.2.push(n);
                        }
                    }
                } else {
                    let mut conns = Vec::new();
                    if let Some(p) = prev { conns.push(p); }
                    if let Some(n) = next { conns.push(n); }
                    self.nodes_map.push((node_id, node_type, conns));
                }
            }

            // Forwardiamo sempre, tranne se la flood è nostra
            if initiator_id != self.server_id {
                let previous_node = packet.routing_header.hops.get(packet.routing_header.hop_index as usize - 1).cloned();

                for (neighbor_id, sender) in self.packet_send.iter() {
                    if Some(*neighbor_id) != previous_node {
                        let _ = sender.send(packet.clone());
                    }
                }
            }
        }
    }
    fn remove_drone(&mut self, node_id: NodeId) {
        // 1) rimuovo dalla topologia
        self.nodes_map.retain(|(id, _, _)| *id != node_id);
        for (_, _, neighbors) in &mut self.nodes_map {
            neighbors.retain(|&neighbor_id| neighbor_id != node_id);
        }

        // 2) rimuovo anche il sender corrispondente (evita futuri panic)
        self.packet_send.remove(&node_id);

        // (facoltativo) pulizia di frammenti relativi a questo client
        self.fragment_recv.retain(|&(who, _), _| who != node_id);
        self.fragment_send.retain(|_, data| data.who_ask != node_id);
    }

    fn send_packet(&mut self, mut packet: Packet) {
        if packet.routing_header.hops.len() < 2 {
            return;
        }
        packet.routing_header.hop_index = 1;
        let next = packet.routing_header.hops[1];
        // Invia il pacchetto al prossimo nodo
        if let Some(sender) = self.packet_send.get_mut(&next) {
            if let Err(e) = sender.send(packet) {
                log::warn!("send_packet: canale pieno o disconnesso su {:?}: {:?}", next, e);
            }
        }
    }
    fn routing(&self, destination: NodeId) -> Option<Vec<NodeId>> {
        let mut table: HashMap<NodeId, (i64, Option<NodeId>)> = HashMap::new();
        let mut queue: BinaryHeap<State> = BinaryHeap::new();
        for (node_id, _, _) in &self.nodes_map {
            table.insert(*node_id, (i64::MAX, None));
        }
        table.insert(self.server_id, (0, None));
        queue.push(State { node: self.server_id, cost: 0 });


        while let Some(State { node, cost }) = queue.pop() {
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
            if cost > table.get(&node)?.0 {
                continue;
            }
            if let Some((_, _, neighbors)) = self.nodes_map.iter().find(|(id, _, _)| *id == node) {
                for &neighbor in neighbors {
                    if neighbor != destination && neighbor != self.server_id {
                        if let Some((_, neighbor_type, _)) = self.nodes_map.iter().find(|(id, _, _)| *id == neighbor) {
                            if *neighbor_type != NodeType::Drone {
                                continue;
                            }
                        }
                    }
                    let new_cost = cost + 1;
                    if new_cost < table.get(&neighbor).unwrap_or(&(i64::MAX, None)).0 {
                        table.insert(neighbor, (new_cost, Some(node)));
                        queue.push(State { node: neighbor, cost: new_cost });
                    }
                }
            }
        }
        None
    }
    fn floading(&mut self) {
        let flood_id = self.get_session();
        let flood = Packet {
            routing_header: SourceRoutingHeader { hop_index: 1, hops: Vec::new() },
            session_id: flood_id,
            pack_type: PacketType::FloodRequest(FloodRequest {
                flood_id,
                initiator_id: self.server_id,
                path_trace: vec![(self.server_id, NodeType::Server)],
            }),
        };

        for (id, mut neighbour) in self.packet_send.clone() {
            if id == self.server_id {
                continue;
            }
            if let Err(e) = neighbour.send(flood.clone()) {
                log::warn!("floading: invio flood a {} fallito: {:?}", id, e);
                // rimuovo il sender ormai chiuso
                self.packet_send.remove(&id);
            }
        }
    }
    fn handle_command(&mut self, session_key: (NodeId, u64)) {
        let (id_client, session) = session_key;

        let data = self.fragment_recv.get(&session_key).unwrap();
        let d = data.dati.clone();
        let command: ComandoChat = deserialize_comando_chat(d);

        match command {
            ComandoChat::Client(request) => match request {
                ChatRequest::ServerType => {
                    let response = Risposta::Chat(ChatResponse::ServerTypeChat(self.server_type.clone()));
                    let session_id = self.get_session();
                    self.send_response(id_client, response, &session_id);
                }
                ChatRequest::RegisterClient(client_id) => {
                    self.registered_clients.push(client_id);
                    let response = Risposta::Chat(ChatResponse::RegisterClient(true));
                    let session_id = self.get_session();
                    self.send_response(id_client, response, &session_id);
                }
                ChatRequest::GetListClients => {
                    let response = Risposta::Chat(ChatResponse::RegisteredClients(self.registered_clients.clone()));
                    let session_id = self.get_session();
                    self.send_response(id_client, response, &session_id);
                }
                ChatRequest::SendMessage(message, _) => {
                    let sender = message.from_id;
                    let receiver = message.to_id;
                    match self.is_present(receiver, sender) {
                        Ok(string) => {
                            let r1 = Risposta::Chat(ChatResponse::SendMessage(Ok(string)));
                            let r2 = Risposta::Chat(ChatResponse::ForwardMessage(message.clone()));

                            let session_sender = self.get_session();
                            let session_receiver = self.get_session();

                            self.send_response(sender, r1, &session_sender);
                            self.send_response(receiver, r2, &session_receiver);
                        }
                        Err(string) => {
                            let r1 = Risposta::Chat(ChatResponse::SendMessage(Err(string)));
                            let session_id = self.get_session();
                            self.send_response(sender, r1, &session_id);
                        }
                    }
                }
                ChatRequest::EndChat(id) => {
                    self.registered_clients.retain(|&x| x != id);
                    let response = Risposta::Chat(ChatResponse::EndChat(true));
                    let session_id = self.get_session();
                    self.send_response(id_client, response, &session_id);
                }
            },
            ComandoChat::Text(text) => match text {
                TextServer::ServerTypeReq => {
                    let response = Risposta::Chat(ChatResponse::ServerTypeChat(self.server_type.clone()));
                    let session_id = self.get_session();
                    self.send_response(id_client, response, &session_id);
                }
                _ => {}
            },
            ComandoChat::WebBrowser(_) => {}
        }
        self.processed_sessions.remove(&session_key);
    }
    fn is_present(&self, receiver: NodeId, sender: NodeId) -> Result<String, String> {
        if self.registered_clients.contains(&sender) && self.registered_clients.contains(&receiver) {
            Ok("The server will forward the message to the final client".to_string())
        }else{
            Err("Error with the registration of the two involved clients".to_string())
        }
    }
    fn send_response(&mut self, id: NodeId, response: Risposta, session: &u64) {
        match response {
            Risposta::Chat(chat) => {
                let dati = serialize(&chat);
                let total = dati.len();
                let total_usize = dati.len();
                let event :Option<ChatServerEvent>;
                match chat{
                    ChatResponse::ServerTypeChat(_) => {
                        event = Some(ChatServerEvent::SendingServerTypeChat(total as u64));
                    }
                    ChatResponse::RegisterClient(_) => {
                        event = Some(ChatServerEvent::ClientRegistration(total as u64));
                    }
                    ChatResponse::RegisteredClients(_) => {
                        event = Some(ChatServerEvent::SendingClientList(total as u64));
                    }
                    ChatResponse::SendMessage(_) =>{
                        event = None;
                    }
                    ChatResponse::EndChat(_) => {
                        event = Some(ChatServerEvent::ClientElimination(total as u64));
                    }
                    ChatResponse::ForwardMessage(_) => {
                        event = Some(ChatServerEvent::ForwardingMessage(total as u64));
                    }
                }
                match event {
                    None => {}
                    Some(e) => {
                        let type_ = MyNodeType::ChatServer;
                        let server_event = ServerEvent::ChatPacketInfo(self.server_id, type_, e, *session);
                        let _ = self.send_event.send(server_event);
                    }
                }
                let d_to_send = Data {
                    counter: total as u64,
                    total_expected: total_usize,
                    dati,
                    who_ask: id,
                };
                self.fragment_send.insert(*session, d_to_send);
                match self.routing(id) {
                    None => {
                        println!("No route found for client {:?}, response not sent", id);
                    }
                    Some(root) => {
                        let mut i = 0;
                        for fragment in self.fragment_send[session].dati.clone() {
                            let routing = SourceRoutingHeader {
                                hop_index: 1,
                                hops: root.clone(),
                            };
                            let f = Fragment {
                                fragment_index: i as u64,
                                total_n_fragments: total as u64,
                                length: fragment.1,
                                data: fragment.0,
                            };
                            let p = Packet {
                                routing_header: routing,
                                session_id: *session,
                                pack_type: PacketType::MsgFragment(f),
                            };
                            self.send_packet(p);
                            i += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    fn get_session(&mut self) -> u64 {
        let id = self.next_session_id;
        self.next_session_id += 1;
        id
    }

}