use crossbeam_channel::tick;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use crate::servers::utilities_max::*;
use crate::common_things::common::*;
use crate::common_things::common::ServerType;
use crossbeam_channel::{select, select_biased, Receiver, Sender};
use std::collections::{BinaryHeap, HashMap};
use std::{fs, io};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, FloodRequest, Fragment, Nack, NackType, NodeType, Packet, PacketType};
use bevy::utils::HashSet;
use std::io::{Read};
use crate::simulation_control::simulation_control::MyNodeType;
use std::time::{Duration, Instant};


pub struct Server{
    server_id: NodeId,
    server_type: ServerType,
    next_session_id: u64,
    nodes_map: Vec<(NodeId, NodeType, Vec<NodeId>)>,
    processed_sessions: HashSet<(NodeId, u64)>,
    fragment_recv: HashMap<(NodeId, u64), Data>,
    fragment_send: HashMap<u64, Data>,
    packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    path: String,
    file_list: Vec<(TextId, String)>,
    media_list: Vec<(MediaId, String)>,
    media_others: HashMap<NodeId, Vec<MediaId>>,
    others : HashMap<NodeId, Vec<NodeId>>,
    already_visited: HashSet<(NodeId, u64)>,
    rcv_flood: Receiver<BackGroundFlood>,
    rcv_command: Receiver<ServerCommands>,
    send_event: Sender<ServerEvent>,
}


impl Server {
    pub fn new(server_id: NodeId, packet_recv: Receiver<Packet>, packet_send: HashMap<NodeId, Sender<Packet>>,rcv_command: Receiver<ServerCommands>, send_event: Sender<ServerEvent>, file_path: &str, rcv_flood: Receiver<BackGroundFlood>) -> Self {
        let mut links: Vec<NodeId> = Vec::new();
        for i in packet_send.clone() {
            links.push(i.0.clone());
        }
        let n = NodeType::Server;
        Server {
            server_id,
            server_type: ServerType::TextServer,
            next_session_id: 10_000,
            nodes_map: vec![(server_id, n, links)],
            processed_sessions: HashSet::new(),
            fragment_recv: HashMap::new(),
            fragment_send: HashMap::new(),
            packet_recv: packet_recv,
            packet_send: packet_send,
            path: file_path.to_string(),
            file_list: Vec::new(),
            media_list: Vec::new(),
            media_others: HashMap::new(),
            others: HashMap::new(),
            already_visited: HashSet::new(),
            rcv_flood,
            rcv_command,
            send_event,
        }
    }
    pub fn run(&mut self) {
        // 1️⃣ Caricamento iniziale (immagini e testi)
        if let Err(e) = self.load_image_paths_from_file() {
            log::error!("Errore load_image_paths: {}", e);
        }
        if let Err(e) = self.load_text_paths_from_file() {
            log::error!("Errore load_text_paths: {}", e);
        }

        // 2️⃣ Prima flood per costruire la topologia
        self.floading();

        // 3️⃣ Ticker per il controllo periodico dei timeout
        let check_interval = Duration::from_millis(100);
        let ticker = tick(check_interval);

        loop {
            select_biased! {
                // ◼️ 4.1 Ricevo un pacchetto
                recv(self.packet_recv) -> packet => {
                    match packet {
                        Ok(pkt) => self.handle_packet(pkt),
                        Err(_)  => break,   // canale chiuso => esco
                    }
                },

                // ◼️ 4.2 Ricevo un segnale di flood
                recv(self.rcv_flood) -> flood => {
                    if flood.is_ok() {
                        self.floading();
                    }
                },

                // ◼️ 4.3 Ricevo un comando dal controller
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
                },

                // ◼️ 5️⃣ Ticker: controllo timeout frammenti
                recv(ticker) -> _ => {
                    self.check_timeouts();
                },
            }
        }
    }

    fn handle_packet(&mut self, packet: Packet) {
        let p = packet.clone();
        match p.pack_type {
            PacketType::MsgFragment(fragment) => {
                let session = packet.session_id;
                self.handle_message(fragment, &session, packet);
            }
            PacketType::Ack(ack) => {
                let session = packet.session_id;
                self.handle_ack(ack, session)
            }
            PacketType::FloodRequest(_) => {
                self.handle_flood_request(packet)
            }
            PacketType::FloodResponse(_) => {
                self.handle_flood_response(packet);
            },
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
            println!("HANDLO IL COMANDO ");
            self.handle_command(session_key);
            self.fragment_recv.remove(&session_key);
        }
    }
    /// 1️⃣ Handle NACK: rispedisco subito, resetto backoff
    fn handle_nack(&mut self, fragment: Nack, _pos: &u64, session: &u64) {
        let idx = fragment.fragment_index as usize;
        // Rimuovi drone in errore se serve
        if let NackType::ErrorInRouting(bad) = fragment.nack_type {
            self.remove_drone(bad);
        }
        // Se ho ancora quel frammento da inviare:
        if let Some(d) = self.fragment_send.get_mut(session) {
            if idx < d.total_expected && !d.acked[idx] {
                // aggiorno counters
                d.retry_count[idx] += 1;
                // resetto il backoff a quello di partenza
                d.backoff[idx] = TIMEOUT.min(MAX_BACKOFF);
                // aggiorno last_send prima di inviare
                d.last_send[idx] = Instant::now();
                // **invia subito**, senza attendere un secondo
                // (chiude il borrow di d)
            }
        }
        // fuori dal borrow di fragment_send...
        self.send_single_fragment(*session, idx);
    }

    /// 2️⃣ Handle ACK: sliding window + misura RTT per backoff adattivo
    fn handle_ack(&mut self, ack: Ack, session: u64) {
        let mut next_idx = None;
        if let Some(d) = self.fragment_send.get_mut(&session) {
            let idx = ack.fragment_index as usize;
            if idx < d.total_expected && !d.acked[idx] {
                // calcolo un sample di RTT
                let now = Instant::now();
                let sample = now.duration_since(d.last_send[idx]);
                // EWMA per backoff (opzionale)
                // backoff[idx] = (backoff[idx].mul_f32(7.0) + sample.mul_f32(3.0)) / 10;
                // ma per semplicità lo lasciamo fisso…

                d.acked[idx] = true;
                d.counter = d.counter.saturating_sub(1);

                // se “slot in finestra” libero, preparo il prossimo
                if d.next_to_send < d.total_expected {
                    next_idx = Some(d.next_to_send);
                    d.next_to_send += 1;
                }
            }
        }
        // fuori dal borrow: mando il frammento successivo
        if let Some(i) = next_idx {
            self.send_single_fragment(session, i);
        }
        // rimuovo la sessione se finita
        if let Some(d) = self.fragment_send.get(&session) {
            if d.counter == 0 {
                println!("🎉 Tutti frammenti sessione {} completati", session);
                self.fragment_send.remove(&session);
            }
        }
    }

    /// 3️⃣ Invia SOLO i primi WINDOW_SIZE frammenti
    fn send_data_fragments(
        &mut self,
        id: NodeId,
        dati: Box<[([u8; 128], u8)]>,
        session: u64,
    ) {
        let total = dati.len();
        let now = Instant::now();
        let window = WINDOW_SIZE.min(total);

        let data = Data {
            counter:        total as u64,
            total_expected: total,
            who_ask:        id,
            dati,
            last_send:      vec![now; total],
            backoff:        vec![TIMEOUT; total],
            retry_count:    vec![0; total],
            acked:          vec![false; total],
            next_to_send:   window,
        };
        self.fragment_send.insert(session, data);

        // invio iniziale di tutta la finestra
        for idx in 0..window {
            self.send_single_fragment(session, idx);
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
            return; // Nessun nodo successivo disponibile
        }
        packet.routing_header.hop_index = 1;

        let next = packet.routing_header.hops[1];
        // Invia il pacchetto al prossimo nodo
        if let Some(sender) = self.packet_send.get_mut(&next) {
            if let Err(e) = sender.send(packet) {
                log::warn!("send_packet fallito su {}: {:?}", next, e);
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
        let command = deserialize_comando_text(d);
        let id_client = data.who_ask;
        match command {
            ComandoText::Media(media) => {
                match media {
                    MediaServer::ServerTypeMedia(ServerType) => {
                        println!("SERVERTYPEMEDIA");
                        if self.media_others.contains_key(&id_client) {} else {
                            if ServerType == ServerType::MediaServer {
                                self.media_others.insert(id_client, Vec::new());
                                let response = Risposta::Text(TextServer::PathResolution);
                                self.send_response(id_client, response)
                            }
                        }
                    }
                    MediaServer::SendPath(v) => {
                        println!("SERVERSENDPATH");
                        if self.media_others.contains_key(&id_client) {
                            self.media_others.insert(id_client, v);
                        } else {}
                    }
                    MediaServer::SendMedia(_) => {}
                }
            }
            ComandoText::Text(text) => {
                match text {
                    TextServer::ServerTypeReq => {
                        println!("SERVERTYPEREQ");
                        let response = Risposta::Media(MediaServer::ServerTypeMedia(ServerType::MediaServer));
                        self.send_response(id_client, response);
                    }
                    TextServer::PathResolution => {
                        println!("SERVERSENDPATHRESOLVE");
                        let response = Risposta::Media(MediaServer::SendPath(self.get_list()));
                        self.send_response(id_client, response);
                    }
                    _ => {}
                }
            }
            ComandoText::Chat(chat) => {
                match chat {
                    ChatResponse::ServerTypeChat(_) => {}
                    _ => {}
                }
            }
            ComandoText::Client(client) => {
                match client {
                    WebBrowserCommands::GetList => {
                        println!("CLIENTGETLIST");
                        let list = self.get_list();
                        let response = Risposta::Text(TextServer::SendFileList(list));
                        self.send_response(id_client, response)
                    }
                    WebBrowserCommands::GetPosition(media) => {
                        println!("CLIENTGETPOSITION");
                        let mut id_server = 0;
                        let server = self.find_position_media(media);
                        match server {
                            Ok(id) => {
                                id_server = id;
                                let response = Risposta::Text(TextServer::PositionMedia(id_server));
                                self.send_response(id_client, response);
                            }
                            _ => {}
                        }
                    }
                    WebBrowserCommands::GetMedia(media_name) => {
                        println!("CLIENTGETMEDIA");
                        match self.find_position_media(media_name.clone()) {
                            Ok(owner_id) => {
                                if owner_id != self.server_id {
                                    return;
                                }
                                match self.get_media(media_name.clone()) {
                                    Ok(encoded_media) => {
                                        let (title, extension) = title_and_extension(&media_name);
                                        let file = FileMetaData {
                                            title,
                                            extension,
                                            content: encoded_media,
                                        };
                                        let response = Risposta::Media(MediaServer::SendMedia(file));
                                        self.send_response(id_client, response);
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }

                    WebBrowserCommands::GetText(text_name) => {
                        println!("CLIENTGETTEXT");
                        match self.find_position_text(text_name.clone()) {
                            Ok(owner_id) => {
                                if owner_id != self.server_id {
                                    return;
                                }
                                match self.get_text(text_name.clone()) {
                                    Ok(encoded_text) => {
                                        let (title, extension) = title_and_extension(&text_name);
                                        let file = FileMetaData {
                                            title,
                                            extension,
                                            content: encoded_text,
                                        };
                                        let response = Risposta::Text(TextServer::Text(file));
                                        self.send_response(id_client, response);
                                    }
                                    Err(err_msg) => {}
                                }
                            }
                            Err(e) => {}
                        }
                    }
                    WebBrowserCommands::GetServerType => {
                        println!("CLIENTGETSERVERTYPE");
                        let response = Risposta::Text(TextServer::ServerTypeText(self.server_type.clone()));
                        self.send_response(id_client, response);
                        let response = Risposta::Media(MediaServer::ServerTypeMedia(ServerType::MediaServer));
                        self.send_response(id_client, response);
                    }
                }
            }
            ComandoText::ChatClient(_) => {}
        }
    }
    fn send_response(&mut self, id: NodeId, response: Risposta) {
        let session = self.get_session();
        print!("SEND RESPONSE ");
        match response {
            Risposta::Text(text) => {
                println!("TEXT");
                let dati = serialize(&text);
                let total = dati.len();
                let event: TextServerEvent;
                match text{
                    TextServer::ServerTypeReq => {
                        event = TextServerEvent::SendingServerTypeReq(total as u64);
                    }
                    TextServer::ServerTypeText(_) => {
                        event = TextServerEvent::SendingServerTypeText(total as u64);
                    }
                    TextServer::PathResolution => {
                        event = TextServerEvent::AskingForPathRes(total as u64);
                    }
                    TextServer::SendFileList(_) => {
                        event = TextServerEvent::SendingFileList(total as u64);
                    }
                    TextServer::PositionMedia(_) => {
                        event = TextServerEvent::SendingPosition(total as u64);
                    }
                    TextServer::Text(_) => {
                        event = TextServerEvent::SendingText(total as u64);
                    }
                }
                let type_ = MyNodeType::TextServer;
                let server_event = ServerEvent::TextPacketInfo(self.server_id, type_, event, session);
                let _ = self.send_event.send(server_event);
                self.send_data_fragments(id, dati, session);
            }
            Risposta::Media(media) => {
                println!("MEDIA");
                let dati = serialize(&media);
                let total = dati.len();
                let event: MediaServerEvent;
                match media{
                    MediaServer::ServerTypeMedia(_) => {
                        event = MediaServerEvent::SendingServerTypeMedia(total as u64);
                    }
                    MediaServer::SendPath(_) => {
                        event = MediaServerEvent::SendingPathRes(total as u64);
                    }
                    MediaServer::SendMedia(_) => {
                        event = MediaServerEvent::SendingMedia(total as u64);
                    }
                }
                let type_ = MyNodeType::MediaServer;
                let server_event = ServerEvent::MediaPacketInfo(self.server_id, type_, event, session);
                let _ = self.send_event.send(server_event);
                self.send_data_fragments(id, dati, session);
            }
            _ => {}
        }
    }
    fn send_single_fragment(&mut self, session: u64, idx: usize) {
        let data = &self.fragment_send[&session];
        let (chunk, len) = data.dati[idx];
        if let Some(path) = self.routing(data.who_ask) {
            let hdr = SourceRoutingHeader::new(path, 1);
            let frag = Fragment::new(idx as u64, data.total_expected as u64, chunk);
            let pkt  = Packet::new_fragment(hdr, session, frag);
            self.send_packet(pkt);
            // aggiorna timer
            let now = Instant::now();
            let d_mut = self.fragment_send.get_mut(&session).unwrap();
            d_mut.last_send[idx] = now;
            // backoff rimane invariato (o potresti volerlo resettare qua)
        }
    }

    pub fn load_text_paths_from_file(&mut self) -> io::Result<()> {
        let path_file = Path::new(&self.path);
        if !path_file.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Path non valido"));
        }

        let file = File::open(path_file)?;
        let reader = BufReader::new(file);

        for line in reader.lines().flatten() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let path = PathBuf::from(trimmed);
            if !path.is_file() {
                continue;
            }
            let ext_opt = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            if matches!(ext_opt.as_deref(), Some("txt")) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    self.file_list
                        .push((name.to_string(), path.to_string_lossy().to_string()));
                }
            }
        }

        Ok(())
    }
    pub fn load_image_paths_from_file(&mut self) -> io::Result<()> {
        let path_file = Path::new(&self.path);
        if !path_file.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Path non valido"));
        }
        let file = File::open(path_file)?;
        let reader = BufReader::new(file);

        for line in reader.lines().flatten() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let path = PathBuf::from(trimmed);
            if !path.is_file() {
                continue;
            }

            let ext_opt = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());

            let is_image = matches!(
                ext_opt.as_deref(),
                Some("jpg")
                    | Some("jpeg")
                    | Some("png")
                    | Some("gif")
                    | Some("bmp")
                    | Some("webp")
                    | Some("tiff")
                    | Some("ico")
                    | Some("jfif")
            );
            if is_image {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    self.media_list
                        .push((name.to_string(), path.to_string_lossy().to_string()));
                }
            }
        }
        Ok(())
    }
    fn get_list(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut unique_list = Vec::new();
        for (name, _) in &self.file_list {
            if seen.insert(name.clone()) {
                unique_list.push(name.clone());
            }
        }
        for (name, _) in &self.media_list {
            if seen.insert(name.clone()) {
                unique_list.push(name.clone());
            }
        }
        for (_, vec_strings) in &self.media_others {
            for k in vec_strings {
                if seen.insert(k.clone()) {
                    unique_list.push(k.clone());
                }
            }
        }
        unique_list
    }
    fn find_position_media(&self, media_id: MediaId) -> Result<NodeId, String> {
        for i in self.media_list.clone() {
            if i.0 == media_id {
                return Ok(self.server_id);
            } else {
                continue;
            }
        }
        for i in self.media_others.clone() {
            for k in i.1.clone() {
                if k == media_id {
                    return Ok(i.0);
                } else {
                    continue;
                }
            }
        }
        Err(String::from(format!("Media {} not found anywhere", media_id)))
    }
    fn find_position_text(&self, text_id: TextId) -> Result<NodeId, String> {
        for (name, _) in &self.file_list {
            if name == &text_id {
                return Ok(self.server_id);
            }
        }
        Err(format!("Text file '{}' not found anywhere", text_id))
    }
    fn get_media(&self, media_id: MediaId) -> Result<String, String> {
        if let Some((_, file_path)) = self.media_list.iter().find(|(id, _)| *id == media_id) {
            match fs::read(Path::new(file_path)) {
                Ok(bytes) => {
                    let encoded = BASE64.encode(&bytes);
                    Ok(encoded)
                }
                Err(_) => {
                    Err(format!("File '{}' not found", file_path))
                }
            }
        } else {
            Err(format!("File con ID {:?} non trovato", media_id))
        }
    }
    fn get_text(&self, text_id: TextId) -> Result<String, String> {
        if let Some((_, file_path)) = self.file_list.iter().find(|(id, _)| *id == text_id) {
            match fs::read(Path::new(file_path)) {
                Ok(bytes) => {
                    let encoded = BASE64.encode(&bytes);
                    Ok(encoded)
                }
                Err(_) => {
                    Err(format!("File '{}' not found", file_path))
                }
            }
        } else {
            Err(format!("File con ID {:?} non trovato", text_id))
        }
    }
    fn get_session(&mut self) -> u64 {
        let id = self.next_session_id;
        self.next_session_id += 1;
        id
    }
    fn check_timeouts(&mut self) {
        let mut to_resend = Vec::new();
        let mut to_abort  = Vec::new();

        // 1) raccolta frammenti da retry
        for (&sess, data) in &self.fragment_send {
            for i in 0..data.total_expected {
                if data.acked[i] { continue; }      // 🌟 salto già confermati
                if data.last_send[i].elapsed() > data.backoff[i] {
                    if data.retry_count[i] < MAX_RETRIES {
                        to_resend.push((sess, i));
                    } else {
                        to_abort.push(sess);
                    }
                }
            }
        }

        // 2) eseguo retry
        for (sess, idx) in to_resend {
            let (who, chunk, total) = {
                let d = &self.fragment_send[&sess];
                (d.who_ask, d.dati[idx].0, d.total_expected)
            };
            if let Some(path) = self.routing(who) {
                let hdr  = SourceRoutingHeader::new(path, 1);
                let frag = Fragment::new(idx as u64, total as u64, chunk);
                let pkt  = Packet::new_fragment(hdr, sess, frag);
                self.send_packet(pkt);

                // aggiorno backoff e retry_count
                let now = Instant::now();
                if let Some(dmut) = self.fragment_send.get_mut(&sess) {
                    dmut.last_send[idx]   = now;
                    dmut.retry_count[idx] += 1;
                    dmut.backoff[idx]     = (dmut.backoff[idx] * 2).min(MAX_BACKOFF);
                    println!(
                        "⏳ Timeout-retry frammento {} sess. {} (#{}, backoff={:?})",
                        idx, sess, dmut.retry_count[idx], dmut.backoff[idx]
                    );
                }
            }
        }

        // 3) pulizia sessioni abortite
        for sess in to_abort {
            println!("❌ Abort sessione {} per retry massimi", sess);
            self.fragment_send.remove(&sess);
        }
    }


}
fn title_and_extension(name: &str) -> (String, String) {
    match name.rsplit_once('.') {
        Some((title, ext)) => (title.to_string(), ext.to_string()),
        None => (name.to_string(), String::from("")),
    }
}
fn read_file<P: AsRef<Path>>(path: P) -> Result<String, io::Error> {
    fs::read_to_string(path)
}
