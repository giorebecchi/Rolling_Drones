use crate::servers::assembler_max::{serialize_text_response, deserialize_text_request};
use crate::common_things::common::*;
use crate::common_things::common::ServerType;
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::{BinaryHeap, HashMap};
use std::{fs, thread};
use std::path::Path;
use bevy::render::render_resource::encase::private::RuntimeSizedArray;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType};
use std::cmp::Ordering;

#[derive(Eq, PartialEq)]
struct State {
    node: NodeId,
    cost: i64,
}
impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost) // Ordine inverso per ottenere un min-heap
    }
}
impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone)]
struct Data{
    counter: u64,
    dati: Box<[[u8; 128]]>,
    who_ask: NodeId,
}
impl Data {
    fn new(data: [u8; 128], position: u64, total: u64, count: u64, asker: NodeId) -> Data {
        let mut v = vec![[0;128]; total as usize].into_boxed_slice();
        v[position as usize-1] = data;
        Data{counter: count, dati: v, who_ask: asker }
    }
}

pub struct Server{
    server_id: NodeId,
    server_type: ServerType,
    nodes_map: Vec<(NodeId, Vec<NodeId>)>,
    fragment_recv: HashMap<u64, Data>,
    fragment_send: HashMap<u64, Data>,
    packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    file_list: Vec<String>
}
impl Server {
    fn new(server_id: NodeId, recv: Receiver<Packet>, send: HashMap<NodeId,Sender<Packet>> ) -> Self {
        let mut links :Vec<NodeId> = Vec::new();
        for i in send.clone(){
            links.push(i.0.clone());
        }
        Server {
            server_id,
            server_type: ServerType::TextServer,
            nodes_map: vec![(server_id, links)],
            fragment_recv: HashMap::new(),
            fragment_send: HashMap::new(),
            packet_recv: recv,
            packet_send: send,
            file_list: Vec::new(),
        }
    }
    fn run(&mut self) {
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
                self.handle_nack(nack, &position, &session, p)
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
            boxed.dati[fragment.fragment_index as usize] = d;
            boxed.counter += 1;
            if boxed.counter == fragment.total_n_fragments-1 {
                self.handle_command(session);
            }
        }else{
            let asker = packet.routing_header.hops[0];
            let d = fragment.data;
            let data = Data::new(d, fragment.fragment_index, fragment.total_n_fragments, 0, asker);
            self.fragment_recv.insert(*session, data);
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
    fn handle_nack(&mut self, fragment: Nack, position: &u64, session: &u64, packet: Packet) {
        if let Some(boxed) = self.fragment_send.get_mut(&session) {
            match fragment.nack_type{
                NackType::ErrorInRouting(id) => {
                    let destination = boxed.who_ask;
                    self.remove_drone(id);
                    let root = self.routing(destination);
                    match root {
                        None => {
                            println!("path not found");
                        }
                        Some(root) => {
                            let total = self.fragment_send.get(&session).unwrap().dati.len() -1;
                            let source = SourceRoutingHeader::new(root, 1);
                            let fr= Fragment::new(*position, total as u64, self.fragment_send.get(&session).unwrap().dati[destination as usize]);
                            let pack = Packet::new_fragment(source, *session, fr );
                            self.send_packet(pack)
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
                            let total = self.fragment_send.get(&session).unwrap().dati.len() -1;
                            let source = SourceRoutingHeader::new(root, 1);
                            let fr= Fragment::new(*position, total as u64, self.fragment_send.get(&session).unwrap().dati[destination as usize]);
                            let pack = Packet::new_fragment(source, *session, fr );
                            self.send_packet(pack)
                        }
                    }
                }
                _ => {unreachable!()}
            }
        }
    }
    fn handle_flood_request(&mut self, packet: Packet) {
        if let PacketType::FloodRequest(mut flood) = packet.pack_type {
            flood.path_trace.push((self.server_id, NodeType::Server));
            let response_packet = flood.generate_response(packet.session_id);
            self.send_packet(response_packet);
        }
    }
    fn handle_flood_response(&mut self, packet: Packet) {
        if let PacketType::FloodResponse(flood_response) = packet.pack_type {
            let len = flood_response.path_trace.len();
            let n = flood_response.path_trace.last().unwrap().0;
            if self.nodes_map.iter().find(|&x| x.0 == n).is_some(){
                let previous = flood_response.path_trace[len - 2].0;
                for mut i in self.nodes_map.iter_mut().rev(){
                    if i.0 == n{
                       i.1.push(previous);
                    }
                }
            }

        }
    }
    fn remove_drone(&mut self, node_id: NodeId) {
        self.nodes_map.retain(|(id, _)| *id != node_id);
        for (_, neighbors) in &mut self.nodes_map {
            neighbors.retain(|&neighbor_id| neighbor_id != node_id);
        }
    }
    fn send_packet(&mut self, packet: Packet) {
        if packet.routing_header.hops.len() < 2 {
            return; // Nessun nodo successivo disponibile
        }
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
        for (node_id, _) in &self.nodes_map {
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
            if cost > table.get(&node).unwrap().0 {
                continue;
            }

            // Iteriamo sui vicini
            if let Some((_, neighbors)) = self.nodes_map.iter().find(|(id, _)| *id == node) {
                for &neighbor in neighbors {
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
        for (id , neighbour) in self.packet_send.clone() {
            if id == self.server_id {}
            else {
                neighbour.send(flood.clone()).unwrap();
                println!("fload request mandata a {}", id)
            }
        }
    }
    fn handle_command(&mut self, session: &u64) {
        let data = self.fragment_recv.get(session).unwrap();
        let d = data.dati.clone();
        let command :TextRequest = deserialize_text_request(d);
        match command {
            TextRequest::ServerType(id) => {
                let response = TextResponse::ServerType(self.server_type.clone());
                self.send_response(id, response, session)
            }
            TextRequest::GetFiles(id) => {
                let files = self.file_list.clone();
                let response: TextResponse = TextResponse::FileList(files);
                self.send_response(id, response, session)
            }
            TextRequest::File(id, file) => {
                let f = get_file(file);
                let response;
                match f {
                    Some(content) => {
                        response = TextResponse::File(content);
                    },
                    None => {
                        response = TextResponse::Error("file not found".to_string());
                    },
                }
                self.send_response(id, response, session);
            }
        }
    }
    fn send_response(&mut self, id: NodeId, response: TextResponse, session: &u64) {

        let dati = serialize_text_response(&response);
        let total = dati.len();
        let data = self.fragment_recv.get(session).cloned().unwrap();
        let who = data.who_ask;
        let d = Data{counter: total as u64, dati, who_ask:who};
        self.fragment_send.insert(*session, d);
        for i in 0..total-1{
            let root = self.routing(id);
            match root {
                None => {
                    println!("path not found")
                }
                Some(root) => {
                    let routing = SourceRoutingHeader{hop_index:1, hops: root};
                    let data_to_send = data.dati[i];
                    let fragment =  Fragment{fragment_index: i as u64, total_n_fragments: total as u64, length: 128, data: data_to_send };
                    let p = Packet{routing_header: routing, session_id: *session, pack_type: PacketType::MsgFragment(fragment)};
                    self.send_packet(p)
                }
            }

        }
    }

}

fn get_file(file_name: String) -> Option<String> {
        let file_path = Path::new("Files").join(file_name);
        fs::read_to_string(file_path).ok()
}

fn create_ack(packet: Packet) ->Packet {
    let mut vec = Vec::new();
    for node_id in (0..=packet.routing_header.hop_index).rev() {
        vec.push(packet.routing_header.hops[node_id]);
    }
    let ack = Ack {
        fragment_index: if let PacketType::MsgFragment(fragment) = packet.pack_type {
            fragment.fragment_index
        } else {
            0
        },
    };
    let pack = Packet {
        pack_type: PacketType::Ack(ack.clone()),
        routing_header: SourceRoutingHeader {
            hop_index: 0,
            hops: vec.clone(),
        },
        session_id: packet.session_id,
    };
    pack
}

pub(crate) fn main() {
        // ID del server
        let server_id: NodeId = 0;

        // Simuliamo una rete connessa: ad esempio, il server è collegato direttamente ai nodi 2 e 3
        let direct_neighbors = vec![1, 4];

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

        // Creiamo il server con la topologia di rete (mappa completa)
        let mut server = Server::new(server_id, packet_recv_server, packet_send_map);
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
}
