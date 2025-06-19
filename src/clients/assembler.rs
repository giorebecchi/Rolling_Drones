use std::collections::HashMap;
use std::string::String;
use serde::Serialize;
use serde::de::DeserializeOwned;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Fragment, Packet, FRAGMENT_DSIZE};
use crate::common_data::common::{ChatRequest, ChatResponse, MediaServer, ServerType, TextServer, WebBrowserCommands};
use crate::common_data::common::MessageChat;

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
        let serialized_message = self.stringify(); //serialize message in a string
        let tot_size = serialized_message.len();
        let tot_fragment = ((tot_size + FRAGMENT_DSIZE - 1 ) / FRAGMENT_DSIZE )as u64; //amount of fragments needed
        let mut fragments = HashMap::new();

        for f in 0..tot_fragment{ //f is the fragment index
            let start = (f as usize) * FRAGMENT_DSIZE; //starting index for the fragment, multiply because we're skipping all data already assigned
            let end = usize::min(start + FRAGMENT_DSIZE, tot_size); //end index, ensuring that maximum size of fragment is respected
            let fragment_data = &serialized_message[start..end]; //the fragmented data using indexes
            let fragment = Fragment::from_string(f, tot_fragment, fragment_data.to_string()); //function from wgc
            fragments.insert(fragment.fragment_index, fragment); //hashmap with key: fragment index and the corresponding fragment
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
    fn create_packet(fragments: &HashMap<u64, Fragment>, path: Vec<NodeId>, session_id: u64) ->Vec<Packet>{
        let mut res = Vec::new();
        for (_, fragment) in fragments{
            let packet = Packet::new_fragment(
                SourceRoutingHeader::new(path.clone(), 0),
                session_id,
                fragment.clone()
            );
            res.push(packet);
        }
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