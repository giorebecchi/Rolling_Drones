use std::string::String;
use bevy::utils::HashMap;
use wg_2024::packet;
use wg_2024::controller;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use serde::de::Unexpected::Str;
use wg_2024::network::NodeId;
use wg_2024::packet::{Fragment, FRAGMENT_DSIZE};


#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MessageChat{ //which needs to be fragmented
    pub content: String,
    pub from_id: NodeId,
    pub to_id: NodeId //id communication server
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MessageWeb{
    pub file_name: String,
    pub media: bool
}

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
        // println!("{:?}", ordered_frag);

        let mut reassembled_frag = Vec::new();
        for (_, frag) in ordered_frag{
            reassembled_frag.extend_from_slice(&frag.data[..frag.length as usize]);
        }

        let convert_to_string = String::from_utf8(reassembled_frag).map_err(|e| e.to_string())?;
        let message = Serialization::from_string(&convert_to_string)?;
        Ok(message)
    }
}

impl MessageChat{
    pub fn new(content: String, from_id: NodeId, to_id: NodeId) -> MessageChat{
        MessageChat{ content, from_id, to_id }
    }
}
impl MessageWeb{
    pub fn new(file_name: String, media: bool) -> MessageWeb{
        MessageWeb{ file_name, media }
    }
}

impl Serialization for MessageChat{}
impl Serialization for MessageWeb{}
impl Fragmentation for MessageChat{}
impl Fragmentation for MessageWeb{}

pub fn main(){
    let message_test = MessageChat{
        content: "this is a really, really, really, really, really, really, really, really, really, really, really long message that surpasses the 128-byte limit for testing purposes.".to_string(),
        from_id: 12,
        to_id: 9
    };

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

    let message_web_test = MessageWeb{
        file_name: "ciao kleppa come stai foto?".to_string(),
        media: true
    };

    let serialized_message = message_web_test.stringify();
    println!("serialized msg: {}", serialized_message);

    let fragments = message_web_test.fragment_message();
    for fragment in &fragments {
        println!("fragment index: {}, fragment: {}", fragment.0, fragment.1)
    }
    println!();

    let reassembled_message = MessageWeb::reassemble_msg(&fragments);
    match reassembled_message{
        Ok(message) => {println!("Message reassembled: {:?}", message)},
        Err(_) => println!("error in reassembling message")
    }
}


