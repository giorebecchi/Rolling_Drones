use bincode;
use crate::common_things::common::{TextRequest, TextResponse};


pub fn serialize_text_response(response: &TextResponse) -> Box<[([u8; 128], u8)]> {
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




pub fn deserialize_text_request(input: Box<[([u8; 128], u8)]>) -> TextRequest {
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
    match serde_json::from_str::<TextRequest>(&serialized_string) {
        Ok(request) => request,
        Err(e) => panic!("Errore nella deserializzazione: {}", e),
    }
}
/*
pub fn deserialize_text_r(input: Box<[([u8; 128], u8)]>) -> TextResponse {
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
   match serde_json::from_str::<TextResponse>(&serialized_string) {
       Ok(request) => request,
       Err(e) => panic!("Errore nella deserializzazione: {}", e),
   }
}


*/

