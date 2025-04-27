use std::fs;
use serde::{Serialize, Deserialize};
use wg_2024::packet::{Fragment, Packet, PacketType, FRAGMENT_DSIZE};
use std::error::Error;
use serde::de::DeserializeOwned;
use wg_2024::network::SourceRoutingHeader;
use wg_2024::packet::PacketType::MsgFragment;
use crate::common_things::common::{ChatRequest, ChatResponse, MediaServer, MessageChat, ServerType, TextServer, WebBrowserCommands};

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
impl Fragmentation for WebBrowserCommands{}

// pub trait FileFragmentation {
//     fn serialize_file_from_path(path: &str, routing_header: SourceRoutingHeader, session_id: u64)
//                                 -> Result<Vec<Packet>, Box<dyn Error>>;
// 
//     fn deserialize_file_to_bytes(fragments: &mut Vec<Fragment>) -> Result<Vec<u8>, String>;
// }
// 
// impl FileFragmentation for Vec<u8> {
//     fn serialize_file_from_path(path: &str,routing_header: SourceRoutingHeader,session_id: u64,) -> Result<Vec<Packet>, Box<dyn Error>> {
//         let file_data = fs::read(path)?; // Read file as bytes
//         let total_size = file_data.len();
//         let total_fragments = ((total_size + FRAGMENT_DSIZE - 1) / FRAGMENT_DSIZE) as u64;
//         let mut packets = Vec::new();
// 
//         for i in 0..total_fragments {
//             let start = i as usize * FRAGMENT_DSIZE;
//             let end = usize::min(start + FRAGMENT_DSIZE, total_size);
//             let chunk = &file_data[start..end];
// 
//             // Copy to fixed-size array
//             let mut fixed_data = [0u8; FRAGMENT_DSIZE];
//             fixed_data[..chunk.len()].copy_from_slice(chunk);
// 
//             let fragment = Fragment {
//                 fragment_index: i,
//                 total_n_fragments: total_fragments,
//                 length: chunk.len() as u8,
//                 data: fixed_data,
//             };
// 
//             let packet = Packet {
//                 routing_header: routing_header.clone(),
//                 session_id,
//                 pack_type: MsgFragment(fragment),
//             };
// 
//             packets.push(packet);
//         }
// 
//         Ok(packets)
//     }
// 
//     fn deserialize_file_to_bytes(fragments: &mut Vec<Fragment>) -> Result<Vec<u8>, String> {
//         fragments.sort_by_key(|f| f.fragment_index);
// 
//         let mut output = Vec::new();
//         for frag in fragments.iter() {
//             output.extend_from_slice(&frag.data[..frag.length as usize]);
//         }
// 
//         Ok(output)
//     }
// }