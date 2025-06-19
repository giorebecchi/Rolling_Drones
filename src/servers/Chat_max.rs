use std::cmp::Reverse;
use crossbeam_channel::select_biased;
use crate::servers::utilities_max::*;
use crate::common_data::common::*;
use crate::common_data::common::ServerType;
use crossbeam_channel::{select, Receiver, Sender};
use std::collections::{BinaryHeap, HashMap};
use std::time::Instant;
use bevy::render::render_resource::encase::private::RuntimeSizedArray;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType};
use bevy::utils::HashSet;
use crate::gui::login_window::NodeType as MyNodeType;
use std::time::Duration;


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
    statistics : HashMap<NodeId, f64>,
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
            statistics: HashMap::new(),
        }
    }
    pub fn run(&mut self) {
        self.flooding();

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
                    if flood.is_ok() { self.flooding(); }
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
                                self.flooding();
                            }
                            ServerCommands::RemoveSender(id) => {
                                self.remove_drone(id);
                            }
                            ServerCommands::PdrChanged(_) => {
                                self.statistics.clear();
                                for (&peer, _) in &self.packet_send {
                                    if peer != self.server_id {
                                        self.statistics.insert(peer, 0.5);
                                    }
                                }
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
                // ricava il path dal routing_header (hops contiene già il vettore completo)
                let path: Vec<NodeId> = packet.routing_header.hops.clone();
                self.handle_ack(ack, session, &path);
            }
            PacketType::FloodRequest(_) => {
                self.handle_flood_request(p)
            }
            PacketType::FloodResponse(_) => {
                self.handle_flood_response(p);
            }
            PacketType::Nack(ref nack) => {
                let session = packet.session_id;
                let position = nack.fragment_index;
                self.handle_nack(nack.clone(), &position, &session, packet);
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
    fn send_response(&mut self, id: NodeId, response: Risposta, session: &u64) {
        if let Risposta::Chat(chat) = response {
            // 1) serializzo in Box<[chunk]>
            let dati: Box<[([u8;128], u8)]> = serialize(&chat);
            let total = dati.len();

            // 2) event tracing (unchanged) …
            let event: ChatServerEvent;
            match chat{
                ChatResponse::ServerTypeChat(_) => {
                    event = ChatServerEvent::SendingServerTypeChat(total as u64);
                }
                ChatResponse::RegisterClient(_) => {
                    event = ChatServerEvent::ClientRegistration(total as u64);
                }
                ChatResponse::RegisteredClients(_) => {
                    event = ChatServerEvent::SendingClientList(total as u64);
                }
                ChatResponse::SendMessage(_) => {
                    event = ChatServerEvent::ForwardingMessage(total as u64);
                }
                ChatResponse::EndChat(_) => {
                    event = ChatServerEvent::ClientElimination(total as u64);
                }
                ChatResponse::ForwardMessage(_) => {
                    event = ChatServerEvent::ForwardingMessage(total as u64);
                }
            }
            let type_ = MyNodeType::ChatServer;
            let server_event = ServerEvent::ChatPacketInfo(self.server_id, type_, event, *session);
            let _ = self.send_event.send(server_event);

            // 3) costruisco Data *semplificato*
            let data = Data {
                dati,
                total_expected: total,
                counter: total as u64,
                who_ask: id,
                acked: vec![false; total],
                retry_count: vec![0; total],
                next_to_send: WINDOW_SIZE.min(total),
            };
            self.fragment_send.insert(*session, data);

            // 4) invio i primi frammenti della finestra
            for idx in 0..WINDOW_SIZE.min(total) {
                self.send_single_fragment(*session, idx);
            }
        }
    }
    fn send_single_fragment(&mut self, session: u64, idx: usize) {
        if let Some(data) = self.fragment_send.get(&session) {
            let who = data.who_ask;
            if let Some(path) = self.routing(who) {
                // Costruisci frammento e packet
                let chunk = data.dati[idx].0;
                let frag = Fragment::new(idx as u64, data.total_expected as u64, chunk);
                let pkt = Packet::new_fragment(SourceRoutingHeader::new(path.clone(), 1), session, frag);
                self.send_packet(pkt);
            } else {
                log::warn!("send_single_fragment: nessun percorso a {} (session {})", who, session);
            }
        }
    }
    fn handle_ack(&mut self, ack: Ack, session: u64, path: &[NodeId]) {
        let alpha = 0.2;
        for &hop in &path[1..] {
            let entry = self.statistics.entry(hop).or_insert(0.5);
            *entry = (1.0 - alpha) * (*entry) + alpha * 1.0;
        }

        // Sliding-window e cleanup classici
        let mut next_idx = None;
        if let Some(d) = self.fragment_send.get_mut(&session) {
            let idx = ack.fragment_index as usize;
            if idx < d.total_expected && !d.acked[idx] {
                d.acked[idx]    = true;
                d.counter       = d.counter.saturating_sub(1);
                if d.next_to_send < d.total_expected {
                    next_idx = Some(d.next_to_send);
                    d.next_to_send += 1;
                }
            }
        }
        if let Some(i) = next_idx {
            self.send_single_fragment(session, i);
        }
        if let Some(d) = self.fragment_send.get(&session) {
            if d.counter == 0 {
                self.fragment_send.remove(&session);
            }
        }
    }
    fn handle_nack(&mut self, fragment: Nack, _pos: &u64, session: &u64, packet: Packet) {

        if let NackType::ErrorInRouting(bad) = fragment.nack_type{
            let bad_next = packet.routing_header.hops[0];
            self.remove_link(bad, bad_next);
            self.flooding()
        }

        if let NackType::Dropped = fragment.nack_type {
            let bad_hop = packet.routing_header.hops[0];
            // Aggiorna p_success verso 0
            let alpha = 0.2;
            let entry = self.statistics.entry(bad_hop).or_insert(0.5);
            *entry = (1.0 - alpha) * (*entry) + alpha * 0.0;
        }
        // Logica standard di retry
        let mut retries = 0;
        {
            let data = match self.fragment_send.get_mut(session) {
                Some(d) => d,
                None    => return,
            };
            let idx = fragment.fragment_index as usize;
            if idx >= data.total_expected || data.acked[idx] { return; }
            if data.retry_count[idx] < (MAX_RETRIES as usize).try_into().unwrap() {
                data.retry_count[idx] += 1;
                retries = data.retry_count[idx];
            } else { return; }
        }

        // Riprova su nuovo percorso
        let data = &self.fragment_send[session];
        let who   = data.who_ask;
        let chunk = data.dati[fragment.fragment_index as usize].0;
        let total = data.total_expected as u64;
        if let Some(path) = self.routing(who) {
            log::debug!("handle_nack: retry #{} frag {} sess {} via {:?}",
                         retries, fragment.fragment_index, session, path);
            let frag2 = Fragment::new(fragment.fragment_index, total, chunk);
            let pkt2  = Packet::new_fragment(SourceRoutingHeader::new(path, 1), *session, frag2);
            self.send_packet(pkt2);
        } else {
            log::warn!("handle_nack: nessun percorso a {} per retry frag {} sess {}",
                       who, fragment.fragment_index, session);
        }
    }
    fn handle_flood_request(&mut self, packet: Packet) {
        if let PacketType::FloodRequest(mut flood) = packet.pack_type {
            let key = (flood.initiator_id, flood.flood_id);
            if self.already_visited.contains(&key) {
                flood.path_trace.push((self.server_id, NodeType::Server));
                let response = FloodRequest::generate_response(&flood, packet.session_id);
                self.send_packet(response);
                return;
            }

            self.already_visited.insert(key.clone());
            flood.path_trace.push((self.server_id, NodeType::Server));

            if self.packet_send.len() == 1 {
                // ultimo nodo: risposta diretta
                let response = FloodRequest::generate_response(&flood, packet.session_id);
                self.send_packet(response);
            } else {
                // inoltro a tutti tranne il precedente e sé stesso
                let prev = packet
                    .routing_header
                    .hops
                    .get(packet.routing_header.hop_index as usize - 1)
                    .cloned();

                let new_packet = Packet {
                    pack_type: PacketType::FloodRequest(flood.clone()),
                    routing_header: packet.routing_header.clone(),
                    session_id: packet.session_id,
                };

                for (idd, mut neighbour) in self.packet_send.clone() {
                    if idd == self.server_id {
                        continue;
                    }
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
            if path.is_empty() { return; }

            // L'iniziatore è il primo elemento
            let initiator_id = path[0].0;

            // 1) Aggiorno topologia: per ogni segmento del path
            for i in 0..path.len() {
                let (node_id, node_type) = path[i];
                // Assicuro inizializzazione EWMA a 0.5 se nuovo
                self.statistics.entry(node_id).or_insert(0.5);

                let prev = if i > 0 { Some(path[i - 1].0) } else { None };
                let next = if i + 1 < path.len() { Some(path[i + 1].0) } else { None };

                if let Some(entry) = self.nodes_map.iter_mut().find(|(id, _, _)| *id == node_id) {
                    // aggiorno tipo
                    if entry.1 != node_type { entry.1 = node_type; }
                    // aggiungo prev/next se mancanti
                    if let Some(p) = prev {
                        if !entry.2.contains(&p) { entry.2.push(p); }
                    }
                    if let Some(n) = next {
                        if !entry.2.contains(&n) { entry.2.push(n); }
                    }
                } else {
                    // nuovo nodo
                    let mut conns = Vec::new();
                    if let Some(p) = prev { conns.push(p); }
                    if let Some(n) = next { conns.push(n); }
                    self.nodes_map.push((node_id, node_type, conns));
                }
            }

            // 2) Forward flood-response se non sono iniziator
            if initiator_id != self.server_id {
                let previous_node = packet.routing_header.hops
                    .get(packet.routing_header.hop_index as usize - 1)
                    .cloned();

                for (neighbor_id, sender) in &self.packet_send {
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
        let scale = 10_000.0;
        let mut dist: HashMap<NodeId, i64> = HashMap::new();
        let mut prev: HashMap<NodeId, NodeId> = HashMap::new();
        // Inizializzo distanze
        for (id, _, _) in &self.nodes_map {
            dist.insert(*id, i64::MAX);
        }
        dist.insert(self.server_id, 0);
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((0, self.server_id)));

        while let Some(Reverse((cost, node))) = heap.pop() {
            // Se questo percorso non è più ottimo, salto
            if cost > *dist.get(&node).unwrap_or(&i64::MAX) {
                continue;
            }
            // Se ho raggiunto la destinazione, ricostruisco il path
            if node == destination {
                let mut path = Vec::new();
                let mut cur = destination;
                while cur != self.server_id {
                    path.push(cur);
                    // Controllo esplicito: se non trovo un prev, abortisco
                    cur = match prev.get(&cur) {
                        Some(&p) => p,
                        None => return None,
                    };
                }
                path.push(self.server_id);
                path.reverse();
                return Some(path);
            }

            // Prendo i vicini: se non trovo alcun nodo corrispondente, salto
            let neighbors = match self.nodes_map.iter()
                .find(|(id, _, _)| *id == node)
            {
                Some((_, _, neigh)) => neigh,
                None => continue,
            };

            for &nei in neighbors {
                // Filtro: solo droni su quelli non estremi
                if nei != destination && nei != self.server_id {
                    let ty = match self.nodes_map.iter()
                        .find(|(id, _, _)| *id == nei)
                    {
                        Some((_, ty, _)) => ty,
                        None => continue,
                    };
                    if *ty != NodeType::Drone {
                        continue;
                    }
                }
                // Prendo p_success con default 0.5
                let p = *self.statistics.get(&nei).unwrap_or(&0.5);
                let success = p.max(1e-6).min(1.0 - 1e-6);
                let pena = (-success.ln() * scale) as i64;
                let next_cost = cost.saturating_add(pena);

                if next_cost < *dist.get(&nei).unwrap_or(&i64::MAX) {
                    dist.insert(nei, next_cost);
                    prev.insert(nei, node);
                    heap.push(Reverse((next_cost, nei)));
                }
            }
        }

        // Nessun percorso trovato
        None
    }
    fn flooding(&mut self) {

        // Genera nuovo flood packet
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

        // Invia a tutti i peer tranne me
        for (id, mut neighbour) in self.packet_send.clone() {
            if id == self.server_id {
                continue;
            }
            if let Err(e) = neighbour.send(flood.clone()) {
                log::warn!("flooding: invio flood a {} fallito: {:?}", id, e);
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
    fn get_session(&mut self) -> u64 {
        let id = self.next_session_id;
        self.next_session_id += 1;
        id
    }
    fn remove_link(&mut self, bad: NodeId, bad_next: NodeId) {
        // Rimuove bad_next dai vicini di bad
        if let Some((_, _, neighbors)) = self
            .nodes_map
            .iter_mut()
            .find(|(id, _, _)| *id == bad)
        {
            neighbors.retain(|&n| n != bad_next);
        }

        // Rimuove bad dai vicini di bad_next
        if let Some((_, _, neighbors)) = self
            .nodes_map
            .iter_mut()
            .find(|(id, _, _)| *id == bad_next)
        {
            neighbors.retain(|&n| n != bad);
        }
    }
}


