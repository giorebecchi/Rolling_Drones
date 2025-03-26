use std::cmp::Ordering;
use std::fs;
use std::path::Path;
use bincode;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, Packet, PacketType};
use crate::common_things::common::{ChatRequest, ChatResponse, RequestWeb, WebResponse};


pub fn serialize_web_response(response: &WebResponse) -> Box<[([u8; 128], u8)]> {
    let serialized_data = serde_json::to_string(response).expect("Errore nella serializzazione");

    // Calcoliamo il numero di blocchi necessari
    let num_blocks = (serialized_data.len() + 127) / 128;
    let mut boxed_array: Vec<([u8; 128], u8)> = Vec::with_capacity(num_blocks);

    // Dividiamo i dati in blocchi da 128 byte
    for chunk in serialized_data.as_bytes().chunks(128) {
        let mut block = [0u8; 128]; // Inizializziamo un blocco pieno di zeri
        block[..chunk.len()].copy_from_slice(chunk); // Copiamo solo i byte utili
        boxed_array.push((block, chunk.len() as u8)); // Aggiungiamo la tupla (dati, lunghezza)
    }

    boxed_array.into_boxed_slice()
}
pub fn serialize_chat_response(response: &ChatResponse) -> Box<[([u8; 128], u8)]> {
    let serialized_data = serde_json::to_string(response).expect("Errore nella serializzazione");




    // Calcoliamo il numero di blocchi necessari
    let num_blocks = (serialized_data.len() + 127) / 128;
    let mut boxed_array: Vec<([u8; 128], u8)> = Vec::with_capacity(num_blocks);




    // Dividiamo i dati in blocchi da 128 byte
    for chunk in serialized_data.as_bytes().chunks(128) {
        let mut block = [0u8; 128]; // Inizializziamo un blocco pieno di zeri
        block[..chunk.len()].copy_from_slice(chunk); // Copiamo solo i byte utili
        boxed_array.push((block, chunk.len() as u8)); // Aggiungiamo la tupla (dati, lunghezza)
    }




    boxed_array.into_boxed_slice()
}
pub fn deserialize_web_request(input: Box<[([u8; 128], u8)]>) -> RequestWeb {
    // Creiamo un buffer abbastanza grande per contenere tutti i byte utili
    let total_length: usize = input.iter().map(|(_, len)| *len as usize).sum();
    let mut all_bytes = Vec::with_capacity(total_length);




    // Concatenare tutti i byte validi
    for (chunk, len) in input.iter() {
        all_bytes.extend_from_slice(&chunk[..*len as usize]);
    }




    // Convertire in stringa UTF-8
    let serialized_string = match String::from_utf8(all_bytes) {
        Ok(s) => s,
        Err(e) => panic!("Errore nella conversione da UTF-8: {}", e),
    };




    println!("JSON ricevuto: {}", serialized_string);




    // 4️⃣ Deserializzare in `TextRequest`
    match serde_json::from_str::<RequestWeb>(&serialized_string) {
        Ok(request) => request,
        Err(e) => panic!("Errore nella deserializzazione: {}", e),
    }
}
pub fn deserialize_chat_request(input: Box<[([u8; 128], u8)]>) -> ChatRequest {
    // Creiamo un buffer abbastanza grande per contenere tutti i byte utili
    let total_length: usize = input.iter().map(|(_, len)| *len as usize).sum();
    let mut all_bytes = Vec::with_capacity(total_length);




    // Concatenare tutti i byte validi
    for (chunk, len) in input.iter() {
        all_bytes.extend_from_slice(&chunk[..*len as usize]);
    }




    // Convertire in stringa UTF-8
    let serialized_string = match String::from_utf8(all_bytes) {
        Ok(s) => s,
        Err(e) => panic!("Errore nella conversione da UTF-8: {}", e),
    };




    println!("JSON ricevuto: {}", serialized_string);




    // 4️⃣ Deserializzare in `TextRequest`
    match serde_json::from_str::<ChatRequest>(&serialized_string) {
        Ok(request) => request,
        Err(e) => panic!("Errore nella deserializzazione: {}", e),
    }
}
pub fn deserialize_text_r(input: Box<[([u8; 128], u8)]>) -> WebResponse {
    // Creiamo un buffer abbastanza grande per contenere tutti i byte utili
    let total_length: usize = input.iter().map(|(_, len)| *len as usize).sum();
    let mut all_bytes = Vec::with_capacity(total_length);

    // Concatenare tutti i byte validi
    for (chunk, len) in input.iter() {
        all_bytes.extend_from_slice(&chunk[..*len as usize]);
    }

    // Convertire in stringa UTF-8
    let serialized_string = match String::from_utf8(all_bytes) {
        Ok(s) => s,
        Err(e) => panic!("Errore nella conversione da UTF-8: {}", e),
    };

    println!("JSON ricevuto: {}", serialized_string);
    // 4️⃣ Deserializzare in `TextRequest`
    match serde_json::from_str::<WebResponse>(&serialized_string) {
        Ok(request) => request,
        Err(e) => panic!("Errore nella deserializzazione: {}", e),
    }
}
pub fn serialize_text_r(response: &RequestWeb) -> Box<[([u8; 128], u8)]> {
    let serialized_data = serde_json::to_string(response).expect("Errore nella serializzazione");

    // Calcoliamo il numero di blocchi necessari
    let num_blocks = (serialized_data.len() + 127) / 128;
    let mut boxed_array: Vec<([u8; 128], u8)> = Vec::with_capacity(num_blocks);

    // Dividiamo i dati in blocchi da 128 byte
    for chunk in serialized_data.as_bytes().chunks(128) {
        let mut block = [0u8; 128]; // Inizializziamo un blocco pieno di zeri
        block[..chunk.len()].copy_from_slice(chunk); // Copiamo solo i byte utili
        boxed_array.push((block, chunk.len() as u8)); // Aggiungiamo la tupla (dati, lunghezza)
    }

    boxed_array.into_boxed_slice()
}


//----------------------------------------------------------------------------------------------------

#[derive(Eq, PartialEq)]
pub(crate) struct State {
    pub node: NodeId,
    pub cost: i64,
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
#[derive(Clone, Debug)]
pub(crate) struct Data{
    pub counter: u64,
    pub dati: Box<[([u8; 128], u8)]>,
    pub who_ask: NodeId,
}
impl Data {
    pub fn new(data: ([u8; 128], u8) , position: u64, total: u64, count: u64, asker: NodeId) -> Data {
        let mut v = vec![([0;128], 0); total as usize].into_boxed_slice();
        v[position as usize] = data;
        Data{counter: count, dati: v, who_ask: asker }
    }
}
pub(crate) fn get_file(file_name: String) -> Option<String> {
    let file_path = Path::new(r"C:\Users\Massimo\RustroverProjects\Rolling_Drone\src\servers\Files").join(file_name);
    fs::read_to_string(file_path).ok()
}
pub(crate) fn create_ack(packet: Packet) ->Packet {
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





