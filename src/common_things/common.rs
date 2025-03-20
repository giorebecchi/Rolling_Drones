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
    EndChat(bool),
    ForwardMessage(MessageChat)
}


#[derive(Serialize,Deserialize,Debug, Clone)]
pub struct MessageChat{ //which needs to be fragmented
    //pub general: Packet,
    pub content: String,
    pub from_id: NodeId,//id client sender
    pub to_id: NodeId //id destination client
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerType{
    CommunicationServer,
    TextServer,
    MediaServer
}

// text/media server and client

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MessageWeb{
    pub file_name: String,
    pub media: bool
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RequestWeb {
    ServerType,
    TextList, //to retrieve text file list
    TextFile (String), //title file
    MediaList,
    Media (String)
}

//solo per max per sistemare il suo server, da cancellare appena è a posto
#[derive(Serialize, Deserialize, Debug)]
pub enum TextRequest{
    ServerType(NodeId),
    GetFiles(NodeId),
    File(NodeId, String)
}


// server to client
#[derive(Serialize, Deserialize, Debug)]
pub enum TextResponse{
    ServerType(ServerType),
    FileList(Vec<String>),
    File(String), // la stringa con tutto il file di testo
    MediaList(Vec<String>),
    Media (String),
    Error(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum CommandText{ //questi vengono mandati al client dal simulation control
    ServerType(NodeId), //node id del server
    GetFiles(NodeId), //node id del server a cui chiedere
    File(NodeId, String), //node id del server, titolo del file da richiedere, se vogliamo il media o no [possiamo anche separare i comandi]
    Media(NodeId), //se vogliamo separare le richieste
    MediaList(NodeId),
    Crash
}
