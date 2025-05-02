use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine; // questo serve per il metodo .encode()
use crate::servers::utilities_max::*;
use crate::common_things::common::*;
use crate::common_things::common::ServerType;
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::{BinaryHeap, HashMap};
use std::{fs, io, thread};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType};
use bevy::utils::HashSet;
use rand::Rng;


pub struct Server{
    server_id: NodeId,
    server_type: ServerType,
    nodes_map: Vec<(NodeId, NodeType, Vec<NodeId>)>,
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
}


impl Server {
    fn new(server_id: NodeId, packet_recv: Receiver<Packet>, packet_send: HashMap<NodeId, Sender<Packet>>, file_path: &str) -> Self {
        let mut links: Vec<NodeId> = Vec::new();
        for i in packet_send.clone() {
            links.push(i.0.clone());
        }
        let n = NodeType::Server;
        Server {
            server_id,
            server_type: ServerType::TextServer,
            nodes_map: vec![(server_id, n, links)],
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
        }
    }
    fn run(&mut self) {
        let path = self.path.clone();
        self.load_files_from_directory(Path::new(&path));
        println!("{:?}", self.file_list);
        println!("{:?}", self.media_list);
        //self.floading();


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
                let response = FloodRequest::generate_response(&flood, packet.session_id);
                self.send_packet(response);
                return;
            } else {
                self.already_visited.insert((flood.initiator_id, flood.flood_id));
                if self.packet_send.len() == 1 {
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
            let (last_node, last_type) = flood_response.path_trace[len - 1];
            if matches!(last_type, NodeType::Server) {
                let mut w = false;
                for i in self.media_others.clone(){
                    if i.0 == last_node{
                        w = true;
                    }
                }
                if w == false{
                    let risposta = Risposta::Text(TextServer::ServerTypeReq);
                    let mut rng = rand::thread_rng();
                    let session: u64 = rng.gen_range(0..100);
                    self.send_response(last_node, risposta, &session);
                }
            }


        }
    }
    fn remove_drone(&mut self, node_id: NodeId) {
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
        let command: ComandoText = deserialize_comando_text(d);
        let id_client = data.who_ask;
        match command {
            ComandoText::Media(media) => {
                match media {
                    MediaServer::ServerTypeMedia(ServerType) => {
                        if self.media_others.contains_key(&id_client) {} else {
                            if ServerType == ServerType::MediaServer {
                                self.media_others.insert(id_client, Vec::new());
                                let response = Risposta::Text(TextServer::PathResolution);
                                self.send_response(id_client, response, session)
                            }
                        }
                    }
                    MediaServer::SendPath(v) => {
                        if self.media_others.contains_key(&id_client) {
                            self.media_others.insert(id_client, v);
                        } else {}
                    }
                    MediaServer::SendMedia(_) => {
                        unreachable!()
                    }
                }
            }
            ComandoText::Text(text) => {
                match text {
                    TextServer::ServerTypeReq => {
                        let response = Risposta::Media(MediaServer::ServerTypeMedia(ServerType::MediaServer));
                        self.send_response(id_client, response, session);
                    }
                    TextServer::PathResolution => {
                        let response = Risposta::Media(MediaServer::SendPath(self.get_list()));
                        self.send_response(id_client, response, session);
                    }


                    _ => {
                        unreachable!()
                    }
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
                        self.send_response(id_client, response, session)
                    }
                    WebBrowserCommands::GetPosition(media) => {
                        let mut id_server = 0;
                        let server = self.find_position_media(media);
                        match server {
                            Ok(id) => {
                                id_server = id;
                                let response = Risposta::Text(TextServer::PositionMedia(id_server));
                                self.send_response(id_client, response, session);
                            }
                            _ => {} // chiedi per la gestione degli errori
                        }
                    }
                    WebBrowserCommands::GetMedia(media_name) => {
                        let media = self.get_media(media_name.clone());
                        let name = title_and_extension(media_name);
                        let file = FileMetaData{
                            title: name.0,
                            extension: name.1,
                            content : media.unwrap(),
                        };
                        let response = Risposta::Media(MediaServer::SendMedia(file));
                        self.send_response(id_client, response, session);
                    }
                    WebBrowserCommands::GetText(text_name) => {
                        let text = self.get_text_from_file(text_name.clone());
                        let name = title_and_extension(text_name);
                        let file = FileMetaData{
                            title: name.0,
                            extension: name.1,
                            content: text.unwrap(),
                        };
                        let response = Risposta::Text(TextServer::Text(file));
                        self.send_response(id_client, response, session);
                    }
                    WebBrowserCommands::GetServerType => {
                        let response = Risposta::Text(TextServer::ServerTypeText(self.server_type.clone()));
                        self.send_response(id_client, response, session);
                    }
                }
            }
        }
    }
    fn send_response(&mut self, id: NodeId, response: Risposta, session: &u64) {
        match response {
            Risposta::Text(text) => {
                let dati = serialize(&text);
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
            Risposta::Media(media) => {
                let dati = serialize(&media);
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
    fn load_files_from_directory(&mut self, file_path: &Path) {
        let file = match File::open(&file_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Errore nell'aprire il file '{}': {}", file_path.display(), e);
                return;
            }
        };


        let reader = BufReader::new(file);


        for line in reader.lines() {
            match line {
                Ok(path_str) => {
                    let path = Path::new(&path_str);
                    if !path.exists() {
                        eprintln!("Path non trovato: {}", path_str);
                        continue;
                    }


                    match path.extension().and_then(|ext| ext.to_str()).map(|s| s.to_lowercase()) {
                        Some(ext) if ext == "txt" => {
                            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                                self.file_list.push((file_name.to_string(), path_str.clone()));
                            }
                        }
                        Some(ext) if ext == "jpg" || ext == "jpeg" => {
                            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                                self.media_list.push((file_name.to_string(), path_str.clone()));
                            }
                        }
                        _ => {
                            // Estensione non supportata
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Errore nella lettura di una riga: {}", e);
                }
            }
        }
    }
    fn get_list(&self) -> Vec<String> {
        let mut list = Vec::new();
        for i in self.file_list.clone() {
            list.push(i.0.clone());
        }
        for i in self.media_list.clone() {
            list.push(i.0.clone());
        }
        for i in self.media_others.clone() {
            for k in i.1.clone() {
                list.push(k);
            }
        }
        list
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
    fn get_text_from_file(&self, text_id: TextId) -> Result<String, String> {
        if let Some((_, file_path)) = self.file_list.iter().find(|(id, _)| *id == text_id) {
            let reading = read_file(Path::new(file_path));
            match reading {
                Ok(contenuto) => Ok(contenuto),
                Err(_) => {
                    eprintln!("Errore nell'aprire il file '{}'", file_path);
                    Err(format!("File '{}' not found", file_path))
                }
            }
        } else {
            Err(format!("File con ID {:?} non trovato", text_id))
        }
    }
    fn get_media(&self, media_id: MediaId) -> Result<String, String> {
        if let Some((_, file_path)) = self.file_list.iter().find(|(id, _)| *id == media_id) {
            match fs::read(Path::new(file_path)) {
                Ok(bytes) => {
                    let encoded = BASE64.encode(&bytes);
                    Ok(encoded)
                }
                Err(_) => {
                    eprintln!("Errore nell'aprire il file '{}'", file_path);
                    Err(format!("File '{}' not found", file_path))
                }
            }
        } else {
            Err(format!("File con ID {:?} non trovato", media_id))
        }
    }

}

fn title_and_extension(name: String) -> (String, String) {
    let s2 = name[name.len()-3..].to_string();
    let s1 = name[0..name.len()-4].to_string();
    (s1, s2)
}

fn read_file<P: AsRef<Path>>(path: P) -> Result<String, io::Error> {
    fs::read_to_string(path)
}






pub(crate) fn main() -> () {
    // ID del server
    let server_id: NodeId = 0;


    // Simuliamo una rete connessa: ad esempio, il server è collegato direttamente ai nodi 2 e 3
    let direct_neighbors = vec![1, 4, 7];








    // Canali per ricezione pacchetti del server
    let (packet_send_server, packet_recv_server) = unbounded();








    // Mappa dei canali di comunicazione (solo i vicini diretti)
    let mut packet_send_map: HashMap<NodeId, _> = HashMap::new();
    packet_send_map.insert(server_id, packet_send_server.clone());








    // Creiamo i canali per i nodi vicini e simuliamo i loro thread di ricezione
    for &neighbor in &direct_neighbors {
        let (tx, rx) = unbounded();
        packet_send_map.insert(neighbor, tx);








        // Simuliamo un nodo vicino che stampa i pacchetti ricevuti
        thread::spawn(move || {
            while let Ok(packet) = rx.recv() {
                println!("🔹 Nodo {} ha ricevuto un pacchetto: {:?}", neighbor, packet);
            }
        });
    }




    let path = r"C:\Users\Massimo\RustroverProjects\Rolling_Drone\src\multimedia\paths\0.txt";


    // Creiamo il server con la topologia di rete (mappa completa)
    let mut server = Server::new(server_id, packet_recv_server, packet_send_map, path);








    // -------------------test flooding già fatto
    // -------------------test handle_flood_response, routing, remove_drone... funzia
    /*
    let v0:Vec<(NodeId, NodeType)> = vec![(7, NodeType::Server), (6, NodeType::Server)];
    let v1:Vec<(NodeId, NodeType)> = vec![(1, NodeType::Drone)];
    let v2:Vec<(NodeId, NodeType)> = vec![(1, NodeType::Drone), (2, NodeType::Drone)];
    let v3:Vec<(NodeId, NodeType)> = vec![(1, NodeType::Drone), (2, NodeType::Drone), (6, NodeType::Server)];
    let v4:Vec<(NodeId, NodeType)> = vec![(1, NodeType::Drone), (3, NodeType::Drone)];
    let v5:Vec<(NodeId, NodeType)> = vec![(1, NodeType::Drone), (3, NodeType::Drone), (5, NodeType::Drone)];
    let v6:Vec<(NodeId, NodeType)> = vec![(1, NodeType::Drone), (3, NodeType::Drone), (5, NodeType::Drone), (6, NodeType::Server)];
    let v7:Vec<(NodeId, NodeType)> = vec![(1, NodeType::Drone), (2, NodeType::Drone), (5, NodeType::Server)];
    let v8:Vec<(NodeId, NodeType)> = vec![(4, NodeType::Drone)];
    let v9:Vec<(NodeId, NodeType)> = vec![(4, NodeType::Drone), (3, NodeType::Drone)];
    let v10:Vec<(NodeId, NodeType)> = vec![(4, NodeType::Drone), (3, NodeType::Drone), (5, NodeType::Drone)];
    let v11:Vec<(NodeId, NodeType)> = vec![(4, NodeType::Drone), (3, NodeType::Drone), (5, NodeType::Drone), (6, NodeType::Server)];






    let flood0 = FloodResponse{flood_id:0, path_trace:v0};
    let flood1 = FloodResponse{ flood_id: 1, path_trace: v1 };
    let flood2 = FloodResponse{ flood_id: 1, path_trace: v2 };
    let flood3 = FloodResponse{ flood_id: 1, path_trace: v3 };
    let flood4 = FloodResponse{ flood_id: 1, path_trace: v4 };
    let flood5 = FloodResponse{ flood_id: 1, path_trace: v5 };
    let flood6 = FloodResponse{ flood_id: 1, path_trace: v6 };
    let flood7 = FloodResponse{ flood_id: 1, path_trace: v7 };
    let flood8 = FloodResponse{ flood_id: 1, path_trace: v8 };
    let flood9 = FloodResponse{ flood_id: 1, path_trace: v9 };
    let flood10 = FloodResponse{ flood_id: 1, path_trace: v10 };
    let flood11 = FloodResponse{ flood_id: 1, path_trace: v11 };






    let p0 = Packet{routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood0)};
    let p1 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood1)};
    let p2 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood2)};
    let p3 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood3)};
    let p4 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood4)};
    let p5 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood5)};
    let p6 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood6)};
    let p7 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood7)};
    let p8 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood8)};
    let p9 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood9)};
    let p10 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood10)};
    let p11 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood11)};






    server.handle_flood_response(p0);
    server.handle_flood_response(p1);
    server.handle_flood_response(p2);
    server.handle_flood_response(p3);
    server.handle_flood_response(p4);
    server.handle_flood_response(p5);
    server.handle_flood_response(p6);
    server.handle_flood_response(p7);
    server.handle_flood_response(p8);
    server.handle_flood_response(p9);
    server.handle_flood_response(p10);
    server.handle_flood_response(p11);








    println!("{:?}", server.nodes_map);








    let route = server.routing(6);
    if let Some(route) = route {
        println!("{:?}", route);
    }else{
        println!("no route");
    }


     */










    // -------------------test serialize e deserialize
    /*
    let e = TextResponse::File("unga bunga".to_string());
    let serialized = serialize_text_response(&e);
    println!("{:?}", serialized);
    let i = deserialize_text_r(serialized);
    println!("{:?}", i);








     */




    //--------------------test handle_paket
    /*
    let e = TextResponse::File("unga bunga".to_string());
    let serialized = serialize_text_response(&e);
    let data = serialized[0].0;
    let len = serialized[0].1;








    let fragment = Fragment{
        fragment_index: 0,
        total_n_fragments: 1,
        length: len,
        data: data,
    };








    let p = Packet{
        routing_header: SourceRoutingHeader{ hop_index: 0, hops: vec![1, 2, 3, 0] },
        session_id: 0,
        pack_type: PacketType::MsgFragment(fragment),
    };
    server.handle_packet(p);
    println!("{:?}", server.fragment_recv);








    let ack = Ack{fragment_index: 0};
    let pa = Packet {
        routing_header: SourceRoutingHeader {
            hop_index: 0,
            hops: vec![1, 2, 3, 0],
        },
        session_id: 0,
        pack_type: PacketType::Ack(ack),
    };
    server.handle_packet(pa);








    let nack = Nack{ fragment_index: 0, nack_type: NackType::Dropped };
    let pac = Packet{
        routing_header: Default::default(),
        session_id: 0,
        pack_type: PacketType::Nack(nack),
    };
    server.handle_packet(pac);








    let req = FloodRequest{
        flood_id: 0,
        initiator_id: 0,
        path_trace: vec![(1, NodeType::Drone)],
    };
    let pack = Packet{
        routing_header: Default::default(),
        session_id: 0,
        pack_type: PacketType::FloodRequest(req.clone()),
    };
    server.handle_packet(pack);








    let resp = FloodResponse{ flood_id: 0, path_trace: vec![] };
    let packe = Packet{
        routing_header: Default::default(),
        session_id: 0,
        pack_type: PacketType::FloodResponse(resp),
    };
    server.handle_packet(packe);








     */




    //--------------------test handle message
    /*
    let e = TextRequest::File(6, "file1.txt".to_string());
    let serialized = serialize_text_r(&e);
    let data = serialized[0].0;
    let len = serialized[0].1;








    let fragment = Fragment{
        fragment_index: 0,
        total_n_fragments: 1,
        length: len,
        data: data,
    };








    let p = Packet{
        routing_header: SourceRoutingHeader{ hop_index: 0, hops: vec![1, 2, 3, 0] },
        session_id: 0,
        pack_type: PacketType::MsgFragment(fragment),
    };
    server.handle_packet(p);




     */




    //--------------------test handle ack
    /*
    let e = TextRequest::File(6, "file1.txt".to_string());
    let serialized = serialize_text_r(&e);
    let data = serialized[0].0;
    let len = serialized[0].1;








    let fragment = Fragment{
        fragment_index: 0,
        total_n_fragments: 1,
        length: len,
        data: data,
    };








    let p = Packet{
        routing_header: SourceRoutingHeader{ hop_index: 0, hops: vec![1, 2, 3, 0] },
        session_id: 0,
        pack_type: PacketType::MsgFragment(fragment),
    };
    server.handle_packet(p);
    println!("fragment_ send  {:?}", server.fragment_send);




    let ack = Ack{fragment_index: 0};
    let pa = Packet {
        routing_header: SourceRoutingHeader {
            hop_index: 0,
            hops: vec![1, 2, 3, 0],
        },
        session_id: 0,
        pack_type: PacketType::Ack(ack),
    };
    server.handle_packet(pa);
    let ack = Ack{fragment_index: 1};
    let pa = Packet {
        routing_header: SourceRoutingHeader {
            hop_index: 0,
            hops: vec![1, 2, 3, 0],
        },
        session_id: 0,
        pack_type: PacketType::Ack(ack),
    };
    server.handle_packet(pa);




    println!("{:?}", server.fragment_send)




     */




    //--------------------test handle nack




    let v1:Vec<(NodeId, NodeType)> = vec![(1, NodeType::Drone)];
    let v3:Vec<(NodeId, NodeType)> = vec![(1, NodeType::Drone), (2, NodeType::Drone), (6, NodeType::Server)];
    let v6:Vec<(NodeId, NodeType)> = vec![(1, NodeType::Drone), (3, NodeType::Drone), (5, NodeType::Drone), (6, NodeType::Server)];
    let v11:Vec<(NodeId, NodeType)> = vec![(4, NodeType::Drone), (3, NodeType::Drone), (5, NodeType::Drone), (6, NodeType::Server)];








    let flood1 = FloodResponse{ flood_id: 1, path_trace: v1 };
    let flood3 = FloodResponse{ flood_id: 1, path_trace: v3 };
    let flood6 = FloodResponse{ flood_id: 1, path_trace: v6 };
    let flood11 = FloodResponse{ flood_id: 1, path_trace: v11 };








    let p1 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood1)};


    let p3 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood3)};


    let p6 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood6)};


    let p11 = Packet{ routing_header: Default::default(), session_id: 0, pack_type: PacketType::FloodResponse(flood11)};








    server.handle_flood_response(p1);


    server.handle_flood_response(p3);


    server.handle_flood_response(p6);


    server.handle_flood_response(p11);


    server.run();




    println!("{:?}", server.nodes_map);


    println!("{:#?}", server.file_list);


    /*


    let route = server.routing(6);
    if let Some(route) = route {
        println!("{:?}", route);
    }else{
        println!("no route");
    }
    let e = WebBrowser::GetText("file1.txt".to_string());
    let serialized = serialize(&e);
    let data = serialized[0].0;
    let len = serialized[0].1;








    let fragment = Fragment{
        fragment_index: 0,
        total_n_fragments: 1,
        length: len,
        data,
    };








    let p = Packet{
        routing_header: SourceRoutingHeader{ hop_index: 0, hops: vec![6, 2, 1, 0] },
        session_id: 0,
        pack_type: PacketType::MsgFragment(fragment),
    };
    server.handle_packet(p);








    let nack = Nack{ fragment_index: 1, nack_type: NackType::ErrorInRouting(2) };
    let pac = Packet{
        routing_header: SourceRoutingHeader{
            hop_index: 3,
            hops: vec![6, 2, 1 ,0],
        } ,
        session_id: 0,
        pack_type: PacketType::Nack(nack),
    };
    server.handle_packet(pac);




    println!("{:?}", server.nodes_map);




     */








}