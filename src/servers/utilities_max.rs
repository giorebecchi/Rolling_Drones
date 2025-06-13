use std::cmp::Ordering;
use std::fs;
use std::path::Path;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, Packet, PacketType};
use std::time::{Duration, Instant};




use serde::{Deserialize, Serialize};
use crate::common_things::common::{ChatRequest, ChatResponse, MediaServer, TextServer, WebBrowserCommands};


pub fn serialize<T>(response: &T) -> Box<[([u8; 128], u8)]>
where
    T: Serialize,
{
    let serialized_data = serde_json::to_string(response)
        .expect("Errore nella serializzazione");


    let num_blocks = (serialized_data.len() + 127) / 128;
    let mut boxed_array: Vec<([u8; 128], u8)> = Vec::with_capacity(num_blocks);


    for chunk in serialized_data.as_bytes().chunks(128) {
        let mut block = [0u8; 128];
        block[..chunk.len()].copy_from_slice(chunk);
        boxed_array.push((block, chunk.len() as u8));
    }


    boxed_array.into_boxed_slice()
}


pub fn deserialize_comando_text(input: Box<[([u8; 128], u8)]>) -> ComandoText {
    let total_length: usize = input.iter().map(|(_, len)| *len as usize).sum();
    let mut all_bytes = Vec::with_capacity(total_length);


    for (chunk, len) in input.iter() {
        all_bytes.extend_from_slice(&chunk[..*len as usize]);
    }


    let serialized_string = match String::from_utf8(all_bytes) {
        Ok(s) => s,
        Err(e) => panic!("Errore nella conversione da UTF-8: {}", e),
    };


    //println!("JSON ricevuto: {}", serialized_string);


    // Prova in ordine: MediaServer -> TextServer -> ChatResponse
    if let Ok(media) = serde_json::from_str::<MediaServer>(&serialized_string) {
        return ComandoText::Media(media);
    }


    if let Ok(text) = serde_json::from_str::<TextServer>(&serialized_string) {
        return ComandoText::Text(text);
    }


    if let Ok(chat) = serde_json::from_str::<ChatResponse>(&serialized_string) {
        return ComandoText::Chat(chat);
    }


    if let Ok(client) = serde_json::from_str::<WebBrowserCommands>(&serialized_string) {
        return ComandoText::Client(client);
    }

    if let Ok(req) = serde_json::from_str::<ChatRequest>(&serialized_string) {
        return ComandoText::ChatClient(req);
    }

    panic!("Errore: Nessuna delle deserializzazioni ha avuto successo per ComandoText.");
}

pub fn deserialize_comando_chat(input: Box<[([u8; 128], u8)]>) -> ComandoChat {
    let total_length: usize = input.iter().map(|(_, len)| *len as usize).sum();
    let mut all_bytes = Vec::with_capacity(total_length);


    for (chunk, len) in input.iter() {
        all_bytes.extend_from_slice(&chunk[..*len as usize]);
    }


    let serialized_string = match String::from_utf8(all_bytes) {
        Ok(s) => s,
        Err(e) => panic!("Errore nella conversione da UTF-8: {}", e),
    };


    //println!("JSON ricevuto: {}", serialized_string);


    // Prova in ordine: MediaServer -> TextServer -> ChatResponse
    if let Ok(text) = serde_json::from_str::<TextServer>(&serialized_string) {
        return ComandoChat::Text(text);
    }

    if let Ok(client ) = serde_json::from_str::<ChatRequest>(&serialized_string) {
        return ComandoChat::Client(client);
    }

    if let Ok(web) = serde_json::from_str::<WebBrowserCommands>(&serialized_string) {
        return ComandoChat::WebBrowser(web);
    }


    panic!("Errore: Nessuna delle deserializzazioni ha avuto successo per ComandoChat.");
}






//----------------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub enum ComandoChat{
    Client(ChatRequest),
    Text(TextServer),
    WebBrowser(WebBrowserCommands),
}

#[derive(Debug, Serialize)]
pub enum Risposta{
    Text(TextServer),
    Media(MediaServer),
    Chat(ChatResponse)
}
#[derive(Deserialize)]
pub enum ComandoText{
    Media(MediaServer),
    Text(TextServer),
    Chat(ChatResponse),
    Client(WebBrowserCommands),
    ChatClient(ChatRequest)
}
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

// —— 1️⃣ Costanti di protocollo ——
pub const TIMEOUT:      Duration = Duration::from_millis(100);
pub const MAX_BACKOFF:  Duration = Duration::from_secs(4);
pub const MAX_RETRIES:  usize    = 100000;
pub const WINDOW_SIZE:  usize    = 100;

// —— 2️⃣ Data struct estesa ——
pub struct Data {
    pub dati: Box<[([u8;128], u8)]>,
    pub counter: u64,
    pub total_expected: usize,
    pub who_ask: NodeId,

    // per retry/timeout
    pub last_send:   Vec<Instant>,
    pub backoff:     Vec<Duration>,
    pub retry_count: Vec<usize>,

    // sliding window
    pub next_to_send: usize,
    pub acked:        Vec<bool>,
}


impl Data {
    pub fn new(
        data: ([u8; 128], u8),
        position: u64,
        total: u64,
        count: u64,
        asker: NodeId,
    ) -> Data {
        let total_usize = total as usize;
        // Creo un Box<[([u8;128], u8)]> di lunghezza total_usize,
        // inizializzato a ([0; 128], 0) per ogni slot
        let mut v = vec![( [0u8; 128], 0u8 ); total_usize].into_boxed_slice();
        // Posiziono subito il frammento corrente
        v[position as usize] = data;
        Data {
            counter: count,
            total_expected: total_usize,
            dati: v,
            who_ask: asker,
            last_send: vec![Instant::now(); total as usize],
            backoff: vec![],
            retry_count: vec![],
            next_to_send: 0,
            acked: vec![],
        }
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