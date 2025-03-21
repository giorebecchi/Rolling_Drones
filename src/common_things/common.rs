use serde::{Deserialize, Serialize};
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;
use crate::GUI::login_window::NodeType;
use crate::servers::Text_max::Server;

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

//NEW DRAFT
#[derive(Serialize, Deserialize, Debug)]
pub enum WebBrowser{
    GetList, //to have the text list resolved by the text server
    GetPosition(u8), //to ask the position of the media (need an id of the media, could be u8)
    GetMedia(u8), //to ask the media to the correct media server (also here need the id of the wanted media)
    GetServerType
}
//probably also need a way to ask the server type
#[derive(Serialize, Deserialize, Debug)]
pub enum TextServer{
    PathResolution, //text server asks all media servers which media he has
    SendTextList(Vec<String>), //send the resolved text list to the client
    PositionMedia(NodeId), //send exact position of the media to the client
    ServerType(ServerType) //probably needed
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MediaServer{
    SendPath(Vec<String>), //send paths to the text server
    SendMedia(Vec<u8>), //send correct media to the client who asked
    ServerType(ServerType) //probably needed
}

//need to add the simulation control commands
//possibility:
#[derive(Serialize, Deserialize, Debug)]
pub enum ContentCommands{
    GetPathResolution, //sent to the text server, to resolve all the text files from media servers (1st step)
    GetTextList(NodeId), //sent to client, client needs to ask text server, node id text server? probably better if automated
    GetMediaPosition(NodeId, u8), //sent to client with id of media needed, same problem with id of text server
    GetMedia(NodeId, u8), //sent to client with id of media, node id of the media server, probably better automated if possible
    GetServerType(NodeId), //sent to client, node id of the server needed
    Crash
}
