use std::fs;
use serde::{Serialize, Deserialize};
use wg_2024::packet::{Fragment, Packet, PacketType};
use std::error::Error;
use wg_2024::network::SourceRoutingHeader;

pub fn serialize_data<T: Serialize>(data: &T, routing_header:SourceRoutingHeader) -> Result<Vec<Packet>, Box<dyn Error>> {
    // Serialize the data into bytes
    let serialized_data = bincode::serialize(&data)?;

    // Calculate the number of fragments needed
    let fragment_size = 128; // Fixed fragment size
    let total_n_fragments = (serialized_data.len() as f64 / fragment_size as f64).ceil() as u64;

    let mut fragments = Vec::new();

    // Create the fragments
    for (i, chunk) in serialized_data.chunks(fragment_size).enumerate() {
        let mut data = [0u8; 128];
        let length = chunk.len() as u8;
        data[..length as usize].copy_from_slice(chunk);

        fragments.push(Packet{
            routing_header: routing_header.clone(),
            session_id: 0,
            pack_type: PacketType::MsgFragment(Fragment{
                fragment_index: i as u64,
                total_n_fragments,
                length,
                data,
            }),
        });
    }

    Ok(fragments)
}

pub fn deserialize_data<T: for<'de> Deserialize<'de>>(fragments: Vec<Fragment>) -> Result<T, Box<dyn Error>> {
    // Ensure fragments are sorted by index
    let mut fragments = fragments;
    fragments.sort_by_key(|f| f.fragment_index);

    // Combine the data from all fragments
    let mut serialized_data = Vec::new();
    for fragment in fragments {
        serialized_data.extend_from_slice(&fragment.data[..fragment.length as usize]);
    }

    // Deserialize into the original type
    let data: T = bincode::deserialize(&serialized_data)?;
    Ok(data)
}
