use std::error::Error;
use serde::{Serialize, Deserialize};
use wg_2024::packet::{Fragment, Packet, PacketType};
use wg_2024::network::SourceRoutingHeader;

/// Serializza i dati in frammenti di pacchetti
pub fn serialize_data<T: Serialize>(data: &T, routing_header: &SourceRoutingHeader, session_id: u64, ) -> Result<Vec<Packet>, Box<dyn Error>> {
    // Serializza i dati in un vettore di byte
    let serialized_data = bincode::serialize(data)?;

    // Calcola il numero di frammenti necessari
    const FRAGMENT_SIZE: usize = 128;
    let total_n_fragments = (serialized_data.len() + FRAGMENT_SIZE - 1) / FRAGMENT_SIZE;

    // Crea i frammenti
    let mut fragments = Vec::with_capacity(total_n_fragments);
    for (i, chunk) in serialized_data.chunks(FRAGMENT_SIZE).enumerate() {
        let mut data = [0u8; FRAGMENT_SIZE];
        let length = chunk.len();
        data[..length].copy_from_slice(chunk);

        fragments.push(Packet {
            routing_header: routing_header.clone(),
            session_id,
            pack_type: PacketType::MsgFragment(Fragment {
                fragment_index: i as u64,
                total_n_fragments: total_n_fragments as u64,
                length: length as u8,
                data,
            }),
        });
    }

    Ok(fragments)
}

/// Deserializza i frammenti in un oggetto del tipo originale
pub fn deserialize_data<T: for<'de> Deserialize<'de>>(fragments: Vec<Fragment>, ) -> Result<T, Box<dyn Error>> {
    if fragments.is_empty() {
        return Err("Nessun frammento ricevuto!".into());
    }

    // Ordina i frammenti per indice
    let mut fragments = fragments;
    fragments.sort_by_key(|f| f.fragment_index);

    // Verifica che tutti i frammenti siano presenti
    let total_n_fragments = fragments[0].total_n_fragments;
    if fragments.len() != total_n_fragments as usize {
        return Err(format!(
            "Numero di frammenti errato: ricevuti {}, attesi {}",
            fragments.len(),
            total_n_fragments
        )
            .into());
    }

    // Combina i dati da tutti i frammenti
    let mut serialized_data = Vec::with_capacity(total_n_fragments as usize * 128);
    for fragment in fragments {
        serialized_data.extend_from_slice(&fragment.data[..fragment.length as usize]);
    }

    // Deserializza nei dati originali
    let data: T = bincode::deserialize(&serialized_data)?;
    Ok(data)
}
