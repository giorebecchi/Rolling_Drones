use serde::{Serialize, Deserialize};
use bincode;
use crate::common_things::common::{TextRequest, TextResponse};




pub trait Boh {
    fn deserialize_text_request(input: Box<[[u8; 128]]>) -> TextRequest {
        // 1. Determinare la lunghezza effettiva del buffer
        let total_size = input.len() * 128;
        let mut bit_sequence = Vec::with_capacity(total_size);


        // 2. Concatenare i dati in un `Vec<u8>`
        for chunk in input.iter() {
            bit_sequence.extend_from_slice(chunk);
        }


        // 3. Tentare la deserializzazione con `bincode`
        bincode::deserialize::<TextRequest>(&bit_sequence).unwrap_or(TextRequest::ServerType(0))
    }
    fn serialize_text_response(response: &TextResponse) -> Box<[[u8; 128]]> {
        // 1. Serializzare il `TextResponse` in un `Vec<u8>`
        let serialized_data = bincode::serialize(response).expect("Errore nella serializzazione");


        // 2. Calcolare il numero di blocchi necessari (arrotondiamo per eccesso)
        let num_blocks = (serialized_data.len() + 127) / 128;


        // 3. Creiamo il `Box<[[u8; 128]]>` con il numero corretto di blocchi
        let mut boxed_array = vec![[0u8; 128]; num_blocks].into_boxed_slice();


        // 4. Copiamo i dati serializzati nei blocchi
        for (i, chunk) in serialized_data.chunks(128).enumerate() {
            boxed_array[i][..chunk.len()].copy_from_slice(chunk);
        }
        boxed_array
    }
}
