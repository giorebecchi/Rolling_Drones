use std::fs;
use serde::{Serialize, Deserialize};
use wg_2024::packet::{Fragment, Packet, PacketType, FRAGMENT_DSIZE};
use std::error::Error;
use serde::de::DeserializeOwned;
use wg_2024::network::SourceRoutingHeader;
use wg_2024::packet::PacketType::MsgFragment;
use crate::common_things::common::{ChatRequest, ChatResponse, MediaServer, MessageChat, ServerType, TextServer, WebBrowser};

pub trait Fragmentation{
    fn serialize_data(&self, routing_header:SourceRoutingHeader, session_id : u64)->Result<Vec<Packet>, Box<dyn Error>> where Self:Serialize{
        let serialized_data = serde_json::to_string(&self)?;
        let tot_size = serialized_data.len();
        let tot_fragment = ((tot_size + FRAGMENT_DSIZE - 1 ) / FRAGMENT_DSIZE )as u64;
        let mut vec = Vec::new();

        for i in 0..tot_fragment{
            let start = i as usize * FRAGMENT_DSIZE;
            let end = usize::min(start+FRAGMENT_DSIZE,tot_size);
            let fragment = &serialized_data[start..end];
            let packet = Packet{
                routing_header: routing_header.clone(),
                session_id:session_id,
                pack_type:MsgFragment(Fragment::from_string(i, tot_fragment, fragment.to_string()))
            };
            vec.push(packet);
        }
        Ok(vec)
    }

    fn deserialize_data(fragments:&mut Vec<Fragment>) ->Result<Self,String> where Self:DeserializeOwned + Sized{
        fragments.sort_by_key(|f| f.fragment_index);
        let serialized_data = fragments.iter()
            .flat_map(|fragment| &fragment.data[..fragment.length as usize])
            .cloned()
            .collect();
        let convert_to_string = String::from_utf8(serialized_data).map_err(|e| e.to_string())?;
        let message = serde_json::from_str(&convert_to_string).map_err(|e| e.to_string())?;
        Ok(message)
    }
}

impl Fragmentation for ServerType{}
impl Fragmentation for MessageChat{}
impl Fragmentation for ChatRequest{}
impl Fragmentation for ChatResponse{}
impl Fragmentation for TextServer{}
impl Fragmentation for MediaServer{}
impl Fragmentation for WebBrowser{}

