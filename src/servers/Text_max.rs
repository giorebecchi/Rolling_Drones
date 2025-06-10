use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use crate::servers::utilities_max::*;
use crate::common_things::common::*;
use crate::common_things::common::ServerType;
use crossbeam_channel::{select_biased, Receiver, Sender};
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

pub struct Server{
    server_id: NodeId,
    server_type: ServerType,
    next_session_id: u64,
    nodes_map: Vec<(NodeId, NodeType, Vec<NodeId>)>,
    processed_sessions: HashSet<(NodeId, u64)>,
    fragment_recv: HashMap<u64, Data>,
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
    send_event: Sender<ServerEvent>
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
            send_event
        }
    }
    pub fn run(&mut self) {
        self.load_image_paths_from_file();
        self.load_text_paths_from_file();
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
                    }
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
        let ack = create_ack(packet.clone());
        self.send_packet(ack);

        let total_frags = fragment.total_n_fragments as usize;
        let frag_idx = fragment.fragment_index as usize;

        if frag_idx >= total_frags {
            return;
        }

        let data_chunk = fragment.data;
        let length_byte = fragment.length;

        if let Some(data_struct) = self.fragment_recv.get_mut(session) {
            let session_key = (data_struct.who_ask, *session);

            if self.processed_sessions.contains(&session_key) {
                return;
            }

            if data_struct.dati.len() != total_frags {
                let mut new_buf = vec![([0u8; 128], 0u8); total_frags];
                for (i, val) in data_struct.dati.iter().enumerate().take(total_frags) {
                    new_buf[i] = *val;
                }
                data_struct.dati = new_buf.into_boxed_slice();
                data_struct.counter = data_struct
                    .dati
                    .iter()
                    .filter(|(_, len)| *len != 0u8)
                    .count() as u64;
            }

            if data_struct.dati[frag_idx].1 == 0 {
                data_struct.dati[frag_idx] = (data_chunk, length_byte);
                data_struct.counter += 1;
            } else {
                return;
            }

            if data_struct.counter == data_struct.total_expected as u64 {
                self.processed_sessions.insert(session_key);
                self.handle_command(session);
                self.fragment_recv.remove(session);
            }

            return;
        }

        let who_ask = if let Some(first_hop) = packet.routing_header.hops.get(0).cloned() {
            first_hop
        } else {
            return;
        };

        let session_key = (who_ask, *session);

        if self.processed_sessions.contains(&session_key) {
            return;
        }

        let mut buf = vec![([0u8; 128], 0u8); total_frags];
        buf[frag_idx] = (data_chunk, length_byte);

        let data = Data {
            dati: buf.into_boxed_slice(),
            counter: 1,
            total_expected: total_frags,
            who_ask,
        };

        self.fragment_recv.insert(*session, data);

        if total_frags == 1 {
            self.processed_sessions.insert(session_key);
            self.handle_command(session);
            self.fragment_recv.remove(session);
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
            } else {
            }
        } else {
        }
    }
    fn handle_nack(&mut self, fragment: Nack, position: &u64, session: &u64) {

        if !self.fragment_send.contains_key(session) {
            return;
        }

        let data_struct = self.fragment_send.get(session).unwrap();
        let destination = data_struct.who_ask;
        let buf_len = data_struct.dati.len();

        // 3. Se buffer vuoto, niente da ritrasmettere
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
        } else {

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
        if let PacketType::FloodResponse(ref flood_response) = packet.pack_type {
            let path = &flood_response.path_trace;
            if path.is_empty() {
                return;
            }

            // Se la flood non parte da noi, devo semplicemente inoltrarla
            if path[0].0 != self.server_id {
                // trovo la mia posizione nella traccia
                if let Some(my_idx) = path.iter().position(|&(node, _)| node == self.server_id) {
                    // se c'è un passo successivo, lo inoltro
                    if let Some(&(next_node, _)) = path.get(my_idx + 1) {
                        if let Some(chan) = self.packet_send.get(&next_node) {
                            // rilancio lo stesso Packet, mantenendo routing_header originale
                            let _ = chan.send(packet);
                        }
                    }
                }
                return;
            }

            // Altrimenti (path[0] == self.server_id) è una risposta ai nostri flood: la processiamo
            let len = path.len();
            if len < 2 {
                return;
            }

            // Aggiorno il grafo con tutti i salti nella path_trace
            for i in 0..len {
                let (node_id, node_type) = path[i];
                let prev = if i > 0 { Some(path[i - 1].0) } else { None };
                let next = if i + 1 < len { Some(path[i + 1].0) } else { None };

                if let Some(entry) = self.nodes_map.iter_mut().find(|(id, _, _)| *id == node_id) {
                    // aggiorno tipo se necessario
                    if entry.1 != node_type {
                        entry.1 = node_type;
                    }
                    // aggiorno vicinanze
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
                    // nuovo nodo
                    let mut conns = Vec::new();
                    if let Some(p) = prev { conns.push(p); }
                    if let Some(n) = next { conns.push(n); }
                    self.nodes_map.push((node_id, node_type, conns));
                }
            }
        }
    }

    fn remove_drone(&mut self, node_id: NodeId)

    {
        self.nodes_map.retain(|(id, _, _)| *id != node_id);
        for (_, _, neighbors) in &mut self.nodes_map {
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
            if let Err(_) = sender.send(packet) {

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
    fn floading(&self) {
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
            }
        }
    }
    fn handle_command(&mut self, session: &u64) {
        let data = self.fragment_recv.get(session).unwrap();
        let d = data.dati.clone();
        let command = deserialize_comando_text(d);
        let id_client = data.who_ask;
        match command {
            ComandoText::Media(media) => {
                match media {
                    MediaServer::ServerTypeMedia(ServerType) => {
                        if self.media_others.contains_key(&id_client) {} else {
                            if ServerType == ServerType::MediaServer {
                                self.media_others.insert(id_client, Vec::new());
                                let response = Risposta::Text(TextServer::PathResolution);
                                self.send_response(id_client, response)
                            }
                        }
                    }
                    MediaServer::SendPath(v) => {
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
                        let response = Risposta::Media(MediaServer::ServerTypeMedia(ServerType::MediaServer));
                        self.send_response(id_client, response);
                    }
                    TextServer::PathResolution => {
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
                        let list = self.get_list();
                        let response = Risposta::Text(TextServer::SendFileList(list));
                        self.send_response(id_client, response)
                    }
                    WebBrowserCommands::GetPosition(media) => {
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

        match response {
            Risposta::Text(text) => {
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
    fn send_data_fragments(&mut self, id: NodeId, dati: Box<[( [u8; 128], u8)]>, session: u64) {
        let total = dati.len();

        let d_to_send = Data {
            counter: total as u64,
            total_expected: total,
            dati,
            who_ask: id,
        };

        self.fragment_send.insert(session, d_to_send);

        if let Some(root) = self.routing(id) {
            for (i, fragment) in self.fragment_send[&session].dati.clone().iter().enumerate() {
                let routing = SourceRoutingHeader { hop_index: 1, hops: root.clone() };
                let f = Fragment {
                    fragment_index: i as u64,
                    total_n_fragments: total as u64,
                    length: fragment.1,
                    data: fragment.0,
                };
                let p = Packet {
                    routing_header: routing,
                    session_id: session,
                    pack_type: PacketType::MsgFragment(f),
                };
                self.send_packet(p);
            }
        } else {}
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