use serde::{Deserialize, Serialize};
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;

//comandi sim_control
#[derive(Clone)]
pub enum CommandChat {
    ServerType(NodeId),//node id server
    RegisterClient(NodeId),//node id server
    GetListClients(NodeId),//node id server
    SendMessage(NodeId, NodeId, String),//node id del client a cui mandare la string, node id server da cui passare
    EndChat(NodeId),//node id del server
    Crash
}

//comandi da client a server
#[derive(Serialize, Deserialize, Debug)]
pub enum ChatRequest{
    ServerType,
    RegisterClient(NodeId),//node id del client stesso
    GetListClients,
    SendMessage(MessageChat, NodeId), //message and server id communcation server (per ora poi se riesco a trovare modo cambio)
    EndChat(NodeId),//node id del client stesso
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ChatResponse{
    ServerType(ServerType),
    RegisterClient(bool),
    RegisteredClients(Vec<NodeId>),
    SendMessage(Result<String, String>),
    ForwardMessage(MessageChat, NodeId),
    EndChat(bool),

}


#[derive(Serialize,Deserialize,Debug, Clone)]
pub struct MessageChat{ //which needs to be fragmented
    //pub general: Packet,
    pub content: String,
    pub from_id: NodeId,//id client sender
    pub to_id: NodeId //id destination client
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MessageWeb{
    pub file_name: String,
    pub media: bool
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerType{
    CommunicationServer,
    TextServer,
    MediaServer
}

// client to server
#[derive(Serialize, Deserialize, Debug)]
pub enum TextRequest{
    ServerType(NodeId),     // id del client
    GetFiles(NodeId),       // id del client
    File(NodeId, String),   // id del client, Nome del file
}
// ho pensato di far inserire l'id del client direttamente li perchè così è più facile per me da raggiungere
// anzicchè dovermelo ricavare dal pacchetto, comunque si può sempre cambiare come cosa




// server to client
#[derive(Serialize, Deserialize, Debug)]
pub enum TextResponse{
    ServerType(ServerType),
    FileList(Vec<String>),
    File(String),           // la stringa con tutto il file di testo
}
//poi bisogna fare la stessa cosa anche per il text e media
