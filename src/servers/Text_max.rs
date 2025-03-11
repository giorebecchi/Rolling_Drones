use crate::common_things::common::*;
use crate::common_things::common::ServerType;
use crossbeam_channel::{select_biased, Receiver, Sender};
use std::collections::{BinaryHeap, HashMap};
use bevy::render::render_resource::encase::private::RuntimeSizedArray;
use bevy_egui::egui::emath::OrderedFloat;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet;
use wg_2024::packet::{Ack, FloodRequest, Fragment, Nack, NackType, NodeType, Packet, PacketType};
use crate::servers::assembler_max::Boh;


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
    nodes: Vec<(NodeId, Vec<NodeId>)>,
    statistics: Vec<(NodeId, i64, i64)>,
    fragment_recv: HashMap<u64, Data>,
    fragment_send: HashMap<u64, Data>,
    packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    file_list: Vec<String>
}


impl Server {
    pub fn new(server_id: NodeId, recv: Receiver<Packet>, send: HashMap<NodeId,Sender<Packet>>, links: Vec<NodeId> ) -> Self {
        let mut n = Vec::new();
        n.push((server_id, links));
        Server {
            server_id,
            server_type: ServerType::TextServer,
            nodes: n,
            statistics: Vec::new(),
            fragment_recv: HashMap::new(),
            fragment_send: HashMap::new(),
            packet_recv: recv,
            packet_send: send,
            file_list: Vec::new(),
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
            PacketType::FloodRequest(flood_request) => {
                let session = packet.session_id;
                self.handle_flood_request(p)
            }
            PacketType::FloodResponse(flood_response) => {
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
        if let Some(boxed) = self.fragment_recv.get_mut(&session) {
            match fragment.nack_type{
                NackType::ErrorInRouting(id) => {
                    self.remove_drone(id);
                    let destination = self.fragment_send
                        .iter()
                        .find(|(&key, _)| key == *session)
                        .unwrap()
                        .1
                        .who_ask;
                    let root:Vec<NodeId> = self.floading_without(destination);
                    let total = self.fragment_send.get(&session).unwrap().dati.len() -1;
                    let source = SourceRoutingHeader::new(root, 1);
                    let fr= Fragment::new(*position, total as u64, self.fragment_send.get(&session).unwrap().dati[destination as usize]);
                    let pack = Packet::new_fragment(source, *session, fr );
                    self.send_packet(pack)
                }
                NackType::Dropped => {
                    let id = packet.routing_header.hops[0];
                    for i in 0..self.statistics.len()-1 {
                        if self.statistics[i].0 == id {
                            self.statistics[i].2 += 1;
                        }
                    }
                    let destination = self.fragment_send
                        .iter()
                        .find(|(&key, _)| key == *session)
                        .unwrap()
                        .1
                        .who_ask;
                    let root = self.floading_without(destination);
                    let total = self.fragment_send.get(&session).unwrap().dati.len()-1; /// chiedi se è meglio usare il meno 1
                    let source = SourceRoutingHeader::new(root, 1);
                    let fr= Fragment::new(*position, total as u64, self.fragment_send.get(&session).unwrap().dati[destination as usize]);
                    let pack = Packet::new_fragment(source, *session, fr );
                    self.send_packet(pack)
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
            let node_id = *flood_response.path_trace.last().map(|(id, _)| id).unwrap_or(&(packet.session_id as u8));
            if let Some((_, neighbors)) = self.nodes.iter_mut().find(|(id, _)| *id == node_id) {
                for (neighbor_id, _) in &flood_response.path_trace {
                    if !neighbors.contains(neighbor_id) && *neighbor_id != node_id {
                        neighbors.push(*neighbor_id);
                    }
                }
            } else {
                let neighbors: Vec<NodeId> = flood_response
                    .path_trace
                    .iter()
                    .map(|(id, _)| *id)
                    .filter(|&id| id != node_id)
                    .collect();


                self.nodes.push((node_id, neighbors));
            }
        }
    }
    fn remove_drone(&mut self, node_id: NodeId) {
        self.nodes.retain(|(id, _)| *id != node_id);
        for (_, neighbors) in &mut self.nodes {
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
    fn floading_without(&mut self, destination: NodeId) -> Vec<NodeId> {todo!()}
    /*
    fn floading_without(&self, destination: NodeId) -> Vec<NodeId> {
        let mut heap = BinaryHeap::new(); // Coda con priorità per Dijkstra
        let mut distances: HashMap<NodeId, f64> = HashMap::new(); // Distanza minima trovata
        let mut previous: HashMap<NodeId, NodeId> = HashMap::new(); // Traccia il percorso
        let mut path_length: HashMap<NodeId, usize> = HashMap::new(); // Conta i nodi nel percorso


        // Inizializzazione: il server è il nodo di partenza
        distances.insert(self.server_id, 0.0);
        path_length.insert(self.server_id, 0);
        heap.push((OrderedFloat(0.0), 0, self.server_id)); // (costo, lunghezza del percorso, nodo)


        while let Some((cost, path_len, node)) = heap.pop() {
            // Se arriviamo alla destinazione, possiamo terminare
            if node == destination {
                break;
            }


            // Se abbiamo trovato un percorso più costoso, ignoriamo questo
            if cost > *distances.get(&node).unwrap_or(&f64::INFINITY) {
                continue;
            }


            // Esaminiamo tutti i vicini
            if let Some((_, neighbors)) = self.nodes.iter().find(|(id, _)| *id == node) {
                for &neighbor in neighbors {
                    let drop_prob = self.compute_drop_probability(neighbor);
                    let new_cost = cost + drop_prob;
                    let is_better = match distances.get(&neighbor) {
                        Some(&current_cost) => {
                            (new_cost < current_cost) || (new_cost == current_cost && path_len + 1 < *path_length.get(&neighbor).unwrap_or(&usize::MAX))
                        }
                        None => true, // Se il nodo non ha ancora una distanza, sicuramente è migliore
                    };
                    if is_better {
                        distances.insert(neighbor, new_cost);
                        previous.insert(neighbor, node);
                        path_length.insert(neighbor, path_len + 1);
                        heap.push((new_cost, path_len + 1, neighbor));
                    }
                }
            }
        }
        // Ricostruiamo il percorso
        let mut path = vec![self.server_id]; // Assicuriamoci che il server sia sempre il primo nodo
        let mut current = destination;
        while let Some(&prev) = previous.get(&current) {
            path.push(current);
            current = prev;
            if current == self.server_id {
                break;
            }
        }
        path.reverse();
        path
    }


     */
    fn compute_drop_probability(&self, node_id: NodeId) -> f64 {
        if let Some((_, sent, dropped)) = self.statistics.iter().find(|(id, _, _)| *id == node_id) {
            if *sent == 0 {
                return 0.0; // Se il nodo non ha inviato pacchetti, consideriamolo affidabile
            }
            return *dropped as f64 / *sent as f64;
        }
        0.0 // Se il nodo non è nella lista statistiche, consideriamolo affidabile
    }
    fn floading(&self) {
        println!("server {} is starting a flooding", self.server_id);
        let mut flood_id = 0;
        let flood = packet::Packet {
            routing_header: SourceRoutingHeader { hop_index: 1, hops: Vec::new() },
            session_id: flood_id,
            pack_type: PacketType::FloodRequest(FloodRequest {
                flood_id,
                initiator_id: self.server_id,
                path_trace: vec![(self.server_id, NodeType::Server)],
            }),
        };
        for (id, neighbour) in self.packet_send.clone() {
            neighbour.send(flood.clone()).unwrap();
        }
    }
    fn handle_command(&mut self, session: &u64) {todo!()}
    /*
    fn handle_command(&mut self, session: &u64) {
        let data = self.fragment_recv.get(session).unwrap();
        let who = data.who_ask;
        let d = Box::new(data.dati);
        let command :TextRequest = Boh::deserialize_text_request(*d);
        match command {
            TextRequest::ServerType(id) => {
                let response = TextResponse::ServerType(self.server_type.clone());
                self.send_response(id, response, session)
            }
            TextRequest::GetFiles(id) => {
                let files= self.file_list;
                let response: TextResponse = TextResponse::FileList(files);
                self.send_response(id, response, session)
            }
            TextRequest::File(id, file) => {}
        }
    }
    fn send_response(&mut self, id: NodeId, response: TextResponse, session: &u64) {
        let data = self.fragment_recv.get(session).unwrap();
        let who = data.who_ask;
        let dati = Boh::serialize_text_response(&response);
        let total = dati.len();
        let d = Data{counter: total as u64, dati: dati, who_ask:who};
        self.fragment_send.insert(*session, d);
        for i in 0..total-1{
            let root = self.floading_without(id);
            let routing = SourceRoutingHeader{hop_index:1, hops: root};
            let data_to_send = data.dati[i];
            let fragment =  Fragment{fragment_index: i as u64, total_n_fragments: total as u64, length: 128, data: data_to_send };///chiedi per il total +- 1
            let p = Packet{routing_header: routing, session_id: *session, pack_type: PacketType::MsgFragment(fragment)};
            self.send_packet(p)
        }
    }


     */
}




fn create_ack(packet: Packet)->Packet {
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


