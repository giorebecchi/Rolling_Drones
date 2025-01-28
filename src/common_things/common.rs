use serde::{Deserialize, Serialize};
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;

//comandi sim_control
pub enum CommandChat {
    ServerType(NodeId),//node id server
    RegisterClient(NodeId),//node id server
    GetListClients(NodeId),//node id server
    SendMessage(NodeId, String),//node id del client a cui mandare la string
    EndChat(NodeId),//node id del server
}

//comandi da client a server
#[derive(Serialize, Deserialize, Debug)]
pub enum ChatRequest{
    ServerType,
    RegisterClient(NodeId),//node id del client stesso
    GetListClients,
    SendMessage(MessageChat),
    EndChat(NodeId),//node id del client stesso
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ChatResponse{
    ServerType(ServerType),
    RegisterClient(bool),
    RegisteredClients(Vec<NodeId>),
    SendMessage(Result<String, String>),
    EndChat(bool),

}

#[derive(Serialize,Deserialize)]
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
    ComunicationServer,
    TesxtServer,
    MediaServer
}

//poi bisogna fare la stessa cosa anche per il text e media