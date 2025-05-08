use std::collections::HashMap;
use std::error::Error;
use std::string::String;
use wg_2024::packet;
use wg_2024::controller;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use serde::de::Unexpected::Str;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Fragment, NodeType, Packet, FRAGMENT_DSIZE};
use crate::common_things::common::{ChatRequest, ChatResponse, MediaServer, ServerType, TextServer, WebBrowserCommands};
use crate::common_things::common::MessageChat;

pub trait Serialization{
    fn stringify(&self) -> String where Self: Serialize{ //to serialize
        serde_json::to_string(&self).unwrap()
    }
    fn from_string(raw: &str) -> Result<Self, String> where Self: Sized + DeserializeOwned{
        serde_json::from_str(&raw).map_err(|e| e.to_string())
    }
}
pub trait Fragmentation: Serialization{
    fn fragment_message(&self)-> HashMap<u64, Fragment> where Self: Serialize{
        let serialized_message = self.stringify();
        let tot_size = serialized_message.len();
        let tot_fragment = ((tot_size + FRAGMENT_DSIZE - 1 ) / FRAGMENT_DSIZE )as u64;
        let mut fragments = HashMap::new();

        for f in 0..tot_fragment{
            let start = (f as usize) * FRAGMENT_DSIZE;
            let end = usize::min(start + FRAGMENT_DSIZE, tot_size);
            let fragment_data = &serialized_message[start..end];
            let fragment = Fragment::from_string(f, tot_fragment, fragment_data.to_string());
            fragments.insert(fragment.fragment_index, fragment);
        }
        fragments
    }
    fn reassemble_msg(fragments: &HashMap<u64, Fragment>)-> Result<Self, String> where Self:Sized + DeserializeOwned{
        let mut ordered_frag: Vec<(&u64, &Fragment)> = fragments.iter().collect();
        ordered_frag.sort_by_key(|&(k, _)| k);

        let mut reassembled_frag = Vec::new();
        for (_, frag) in ordered_frag{
            reassembled_frag.extend_from_slice(&frag.data[..frag.length as usize]);
        }

        let convert_to_string = String::from_utf8(reassembled_frag).map_err(|e| e.to_string())?;
        let message = Serialization::from_string(&convert_to_string)?;
        Ok(message)
    }
    fn create_packet(fragments: &HashMap<u64, Fragment>, path: Vec<NodeId>, mut session_id: & mut u64) ->Vec<Packet>{
        let mut res = Vec::new();
        for (_, fragment) in fragments{
            let packet = Packet::new_fragment(
                SourceRoutingHeader::new(path.clone(), 0),
                *session_id, //to increment every time you have to send a new message, so every time you call this function
                fragment.clone()
            );
            res.push(packet);
        }
        *session_id += 1;
        res
    }

}

impl MessageChat{
    pub fn new(content: String, from_id: NodeId, to_id: NodeId) -> MessageChat{
        MessageChat{ content, from_id, to_id }
    }
}

impl Serialization for MessageChat{}
impl Fragmentation for MessageChat{}
impl Serialization for ServerType{}
impl Fragmentation for ServerType{}

impl Serialization for ChatRequest{}
impl Fragmentation for ChatRequest{}
impl Serialization for ChatResponse{}
impl Fragmentation for ChatResponse{}

impl Serialization for WebBrowserCommands {}
impl Fragmentation for WebBrowserCommands {}

impl Serialization for TextServer{}
impl Fragmentation for TextServer{}

impl Serialization for MediaServer{}
impl Fragmentation for MediaServer{}

//used in the graph for the topology
#[derive(Debug, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)] //need these implemented to use it in the graph
pub struct NodeData{
    pub forwarded: u32,
    pub dropped: u32,
}

impl NodeData{
    pub fn new()-> Self{
        Self{
            forwarded: 0,
            dropped: 0
        }
    }

    pub fn reliability(&self)-> f32{
        if (self.dropped + self.forwarded) == 0{
            return 1.0
        }
        let tot = self.forwarded as f32 / (self.dropped + self.forwarded) as f32;
        if tot == 0.0{
            return 0.1
        }
        tot
    }
}

pub fn main(){
    let message_test = MessageChat{
        content: "this is a really, really, really, really, really, really, really, really, really, really, really long message that surpasses the 128-byte limit for testing purposes.".to_string(),
        from_id: 12,
        to_id: 9
    };
    let mut session_id = 0;

    let serialized_message = message_test.stringify();
    println!("serialized msg: {}", serialized_message);

    let fragments = message_test.fragment_message();
    for fragment in &fragments {
        // println!("{:?}\n", fragment);
        // println!("fragment index: {}\nfragment: {:?}", fragment.0, fragment.1);
        // println!();
        println!("fragment index: {}, fragment: {}", fragment.0, fragment.1)
    }
    println!();

    let reassembled_message  = MessageChat::reassemble_msg(&fragments);
    match reassembled_message{
        Ok(message) => {println!("Message reassembled: {:?}", message)},
        Err(_) => println!("error in reassembling message")
    }
    println!();
    let packet = MessageChat::create_packet(&fragments, vec![1,3,4], &mut session_id);
    println!("The packet is {:?}", packet);
    println!();

    let server_type = ServerType::CommunicationServer;

    let serialized_server = server_type.stringify();
    println!("serialized server: {}", serialized_server);
    println!();

    let fragments_enum = server_type.fragment_message();
    for fragment in &fragments_enum {
        println!("fragment index: {}, fragment: {}", fragment.0, fragment.1)
    }
    println!();

    let reassembled_enum = ServerType::reassemble_msg(&fragments_enum);
    match reassembled_enum{
        Ok(server_type) => {println!("Message reassembled: {:?}", server_type)},
        Err(_) => println!("error in reassembling message")
    }
    println!();

    let request = ChatRequest::SendMessage(message_test, 2);
    println!("serialized request: {:?}", request.stringify());
    println!();

    let fragment_request = ChatRequest::fragment_message(&request);
    for fragment in &fragment_request{
        println!("fragment index: {}, fragment: {}", fragment.0, fragment.1)
    }
    println!();

    let reassembled_request = ChatRequest::reassemble_msg(&fragment_request);
    match reassembled_request {
        Ok(request) => {println!("reassembled request: {:?}", request)},
        Err(_) => println!("error in reassembling message")
    }
    println!();

    let packets = ChatRequest::create_packet(&fragment_request, vec![3,6,8], & mut session_id );
    println!("The packet is {:?}", packets);

    //test for bool
    let response_registration = ChatResponse::RegisterClient(true);
    println!("serialized message: {}", response_registration.stringify());
    println!();

    let fragments_registration = ChatResponse::fragment_message(&response_registration);
    for fragment in &fragments_registration{
        println!("fragment index: {}, fragment: {}", fragment.0, fragment.1)
    }
    println!();

    let reassembled_registration = ChatResponse::reassemble_msg(&fragments_registration);
    match reassembled_registration {
        Ok(response) => {println!("reassembled response: {:?}", response)},
        Err(_) => println!("error in reassembling message")
    }
    println!();

    let packet_registration = ChatRequest::create_packet(&fragments_registration, vec![1,3,4] , & mut session_id);
    println!("The packet registration is  {:?}", packet_registration);

    let registered_client = ChatResponse::RegisteredClients(vec![3,5,6,9,8,2,1,0]);
    println!("serialized message: {}", registered_client.stringify());
    println!();

    let fragments_registered_client = ChatResponse::fragment_message(&registered_client);
    for fragment in &fragments_registered_client{
        println!("fragment index: {}, fragment: {}", fragment.0, fragment.1)
    }
    println!();

    let reassembled_client = ChatResponse::reassemble_msg(&fragments_registered_client);
    println!("{:?}", reassembled_client);
    println!();

    let packet_to_send = ChatResponse::create_packet(&fragments_registered_client, vec![1,3,4] , & mut session_id);
    println!("{:?}", packet_to_send);




}


