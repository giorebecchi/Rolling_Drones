use crossbeam_channel::select_biased;
use crate::servers::utilities_max::*;
use crate::common_things::common::*;
use crate::common_things::common::ServerType;
use crossbeam_channel::{select, Receiver, Sender};
use std::collections::{BinaryHeap, HashMap};
use std::time::Instant;
use bevy::render::render_resource::encase::private::RuntimeSizedArray;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType};
use bevy::utils::HashSet;
use crate::simulation_control::simulation_control::MyNodeType;use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(1);


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
            nodes_map: vec![(id, n, links)],
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
        // flood iniziale per costruire la topologia
        self.floading();

        let mut last_check   = Instant::now();
        let   check_interval = Duration::from_millis(100);

        loop {
            select_biased! {
                // 🟢 pacchetti in entrata
                recv(self.packet_recv) -> packet => {
                    match packet {
                        Ok(pkt) => self.handle_packet(pkt),
                        Err(_)  => break,
                    }
                },
                // 🔄 flood-trigger
                recv(self.rcv_flood) -> flood => {
                    if flood.is_ok() { self.floading(); }
                },
                // ⚙️ comandi topologia
                recv(self.rcv_command) -> cmd => {
                    if let Ok(command) = cmd {
                        match command {
                            ServerCommands::SendTopologyGraph => {
                                let _ = self.send_event
                                    .send(ServerEvent::GraphMax(self.server_id, self.nodes_map.clone()));
                            }
                            ServerCommands::AddSender(id, sender) => {
                                self.packet_send.insert(id, sender);
                                self.floading();
                            }
                            ServerCommands::RemoveSender(id) => {
                                self.remove_drone(id);
                            }
                        }
                    }
                }
            }

            // 🔁 ogni check_interval faccio retry/timeouts
            if last_check.elapsed() >= check_interval {
                self.check_timeouts();
                last_check = Instant::now();
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
        let frag_idx = fragment.fragment_index as usize;
        if frag_idx >= total_frags { return; }

        let who_ask = match packet.routing_header.hops.get(0).cloned() {
            Some(h) => h,
            None => return,
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


    fn handle_ack(&mut self, ack: Ack, session: u64) {
        // estraggo next_to_send se serve
        let mut maybe_to_send = None;

        if let Some(data) = self.fragment_send.get_mut(&session) {
            let idx = ack.fragment_index as usize;
            if idx < data.total_expected && !data.acked[idx] {
                data.acked[idx]      = true;
                data.counter         = data.counter.saturating_sub(1);
                data.last_send[idx]  = Instant::now();

                println!("ACK frammento {} sessione {}", idx, session);

                // avanzo sliding window
                if data.next_to_send < data.total_expected {
                    let send_idx = data.next_to_send;
                    data.next_to_send += 1;
                    maybe_to_send = Some(send_idx);
                }

                // fine sessione?
                if data.counter == 0 {
                    println!("Sessione {} completata, rimuovo", session);
                    self.fragment_send.remove(&session);
                }
            }
        }

        // fuori dal borrow mut, invio il nuovo frammento se esiste
        if let Some(to_send) = maybe_to_send {
            self.send_single_fragment(session, to_send);
        }
    }
    fn handle_nack(&mut self, fragment: Nack, position: &u64, session: &u64) {
        // passo 1: esistenza sessione e indice
        if !self.fragment_send.contains_key(session) { return; }
        let total = self.fragment_send[session].total_expected;
        let nidx  = fragment.fragment_index as usize;
        if nidx >= total { return; }

        // ErrorInRouting → aggiorna topologia
        if let NackType::ErrorInRouting(bad) = fragment.nack_type {
            println!("ErrorInRouting nodo {}, rimuovo", bad);
            self.remove_drone(bad);
        }

        // passo 2: retry immediato se non ackato
        let mut should_retry = false;
        if let Some(data) = self.fragment_send.get_mut(session) {
            if !data.acked[nidx] {
                data.backoff[nidx]     = TIMEOUT;
                data.retry_count[nidx] += 1;
                data.last_send[nidx]   = Instant::now();
                should_retry = true;
            }
        }

        // passo 3: effettua l’invio fuori dal borrow mut
        if should_retry {
            let who_ask = self.fragment_send[session].who_ask;
            let (chunk, _) = self.fragment_send[session].dati[nidx];
            if let Some(path) = self.routing(who_ask) {
                let hdr  = SourceRoutingHeader::new(path, 1);
                let frag = Fragment::new(*position, total as u64, chunk);
                let pkt  = Packet::new_fragment(hdr, *session, frag);
                self.send_packet(pkt);

                println!(
                    "NACK-retry frammento {} sess {} (#{})",
                    nidx, session, self.fragment_send[session].retry_count[nidx]
                );
            }
        }
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
        } else {
            Err("Error with the registration of the two involved clients".to_string())
        }
    }
    fn send_response(&mut self, id: NodeId, response: Risposta, session: &u64) {
        if let Risposta::Chat(chat) = response {
            // 1) serializzo → ottengo un Box<[([u8;128], u8)]>
            let dati_boxed: Box<[([u8;128], u8)]> = serialize(&chat);

            // 2) ricavo dimensioni
            let total = dati_boxed.len();

            // 3) event tracing (unchanged)
            let event = match chat {
                ChatResponse::ServerTypeChat(_)    => Some(ChatServerEvent::SendingServerTypeChat(total as u64)),
                ChatResponse::RegisterClient(_)    => Some(ChatServerEvent::ClientRegistration(total as u64)),
                ChatResponse::RegisteredClients(_) => Some(ChatServerEvent::SendingClientList(total as u64)),
                ChatResponse::SendMessage(_)       => None,
                ChatResponse::EndChat(_)           => Some(ChatServerEvent::ClientElimination(total as u64)),
                ChatResponse::ForwardMessage(_)    => Some(ChatServerEvent::ForwardingMessage(total as u64)),
            };
            if let Some(e) = event {
                let server_event = ServerEvent::ChatPacketInfo(
                    self.server_id,
                    MyNodeType::ChatServer,
                    e,
                    *session,
                );
                let _ = self.send_event.send(server_event);
            }

            // 4) costruisco la struttura Data (sliding-window, backoff, ecc)
            let now = Instant::now();
            let data = Data {
                counter:        total as u64,
                total_expected: total,
                dati:           dati_boxed,               // ← qui non chiamo more into_boxed_slice()
                who_ask:        id,
                last_send:      vec![now; total],
                backoff:        vec![TIMEOUT; total],
                retry_count:    vec![0; total],
                acked:          vec![false; total],
                next_to_send:   0,
            };
            self.fragment_send.insert(*session, data);

            // 5) invio i primi WINDOW_SIZE frammenti
            let win = WINDOW_SIZE.min(total);
            for idx in 0..win {
                self.send_single_fragment(*session, idx);
                // avanzamento finestra
                self.fragment_send.get_mut(session).unwrap().next_to_send += 1;
            }
        }
    }
    fn get_session(&mut self) -> u64 {
        let id = self.next_session_id;
        self.next_session_id += 1;
        id
    }
    fn send_single_fragment(&mut self, session: u64, idx: usize) {
        let data = &self.fragment_send[&session];
        let (chunk, _) = data.dati[idx];
        if let Some(path) = self.routing(data.who_ask) {
            let hdr  = SourceRoutingHeader::new(path, 1);
            let frag = Fragment::new(idx as u64, data.total_expected as u64, chunk);
            let pkt  = Packet::new_fragment(hdr, session, frag);
            self.send_packet(pkt);

            // aggiorno il timer
            let now = Instant::now();
            let dmut = self.fragment_send.get_mut(&session).unwrap();
            dmut.last_send[idx] = now;
        }
    }
    fn check_timeouts(&mut self) {
        let mut to_resend = Vec::new();
        let mut to_abort  = Vec::new();

        // passo 1: raccolta
        for (&sess, data) in &self.fragment_send {
            for i in 0..data.total_expected {
                if data.acked[i] { continue; }
                if data.last_send[i].elapsed() > data.backoff[i] {
                    if data.retry_count[i] < MAX_RETRIES {
                        to_resend.push((sess, i));
                    } else {
                        to_abort.push(sess);
                    }
                }
            }
        }

        // passo 2: retry con backoff
        for (sess, idx) in to_resend {
            // estraggo i valori essenziali fuori dal borrow mut
            let (who, chunk, total_expected) = {
                let d = &self.fragment_send[&sess];
                (d.who_ask, d.dati[idx].0, d.total_expected)
            };

            // invio
            if let Some(path) = self.routing(who) {
                let hdr  = SourceRoutingHeader::new(path, 1);
                let frag = Fragment::new(idx as u64, total_expected as u64, chunk);
                let pkt  = Packet::new_fragment(hdr, sess, frag);
                self.send_packet(pkt);

                // aggiorno timer, retry_count e backoff
                let now = Instant::now();
                let dmut = self.fragment_send.get_mut(&sess).unwrap();
                dmut.last_send[idx]   = now;
                dmut.retry_count[idx] += 1;
                dmut.backoff[idx]     = (dmut.backoff[idx] * 2).min(MAX_BACKOFF);

                println!(
                    "Timeout-retry frammento {} sess {} (#{}, backoff={:?})",
                    idx, sess, dmut.retry_count[idx], dmut.backoff[idx]
                );
            }
        }

        // passo 3: abort delle sessioni “inarrestabili”
        for sess in to_abort {
            println!("Aborto sessione {} per retry massimi", sess);
            self.fragment_send.remove(&sess);
        }
    }
}


