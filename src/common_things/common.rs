use std::fmt::{Display, Formatter};
use petgraph::Graph;
use petgraph::prelude::UnGraphMap;
use serde::{Deserialize, Serialize};
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;
use crate::GUI::login_window::NodeType;
use crate::servers::Text_max::Server;
use crate::simulation_control::simulation_control::MyNodeType;

//comandi sim_control
#[derive(Clone)]
pub enum CommandChat {
    ServerType(NodeId),//node id server
    SearchChatServers,
    RegisterClient(NodeId),//node id server
    GetListClients(NodeId),//node id server
    SendMessage(NodeId, NodeId, String),//node id del client a cui mandare la string, node id server da cui passare
    EndChat(NodeId),//node id del server
    SendTopologyGraph,
    Crash
}
///The NodeId identifies the client that sent the ChatClientEvent
#[derive(Debug)]
pub enum ChatClientEvent{
    ClientList((NodeId, NodeId) ,Vec<NodeId>), //NodeId Client, NodeId Server, Vec<ClientIds>
    IncomingMessage((NodeId,NodeId,NodeId),String), //NodeId Client a cui è arrivato msg, NodeId server, NodeId del client da cui il messaggio è arrivato msg
    RegisteredSuccess((NodeId,NodeId),Result<(), String>), //NodeId registered client and NodeId server { either Ok(()) or Err("something".to_string()) }
    Error(NodeId),//Generic Error to send to SC
    ChatServers(NodeId, Vec<NodeId>),
    ClientType(ClientType,NodeId),
    PacketInfo(NodeId, ChatEvent, u64),
    InfoRequest(NodeId, RequestEvent, u64),
    Graph(NodeId, UnGraphMap<NodeId, u32>)
}
#[derive(Debug,Clone)]
pub enum ChatEvent{
    ClientList(u64),
    IncomingMessage(u64),
    RegisteredSuccess(u64),
    ChatServers(u64),
    ClientType(u64)
}

#[derive(Debug)]
pub enum RequestEvent{
    AskType(u64),
    Register(u64),
    GetList(u64),
    SendMessage(u64),
}

pub enum ServerCommands{
    SendTopologyGraph,
}
pub enum ServerEvent{
    Graph(NodeId, Graph<(NodeId, wg_2024::packet::NodeType), f64, petgraph::Directed>),
    WebPacketInfo(NodeId, MyNodeType, ContentType, u64), //(id server, server_type (ChatServer, TextServer,...), type of message, session_id)
    ChatPacketInfo(NodeId, MyNodeType, ChatEvent, u64)  //(id server, server_type (ChatServer, TextServer,...), type of message, session_id)

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
    ServerTypeChat(ServerType),
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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum ServerType{
    CommunicationServer,
    TextServer,
    MediaServer
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum ClientType{
    ChatClient,
    WebBrowser
}


//COMMANDS TEXT, MEDIA SERVERS AND WEB BROWSER
pub type MediaId = String;
pub type TextId = String;
#[derive(Serialize, Deserialize, Debug)]
pub struct FileMetaData{
    pub(crate) title: String, 
    pub(crate) extension: String,
    pub(crate) content: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub enum WebBrowserCommands{
    GetList, //to have the text list resolved by the text server
    GetPosition(MediaId), //to ask the position of the media (need an id of the media, could be u8)
    GetMedia(MediaId), //to ask the media to the correct media server (also here need the id of the wanted media),
    GetText(TextId), //to ask a text file a text
    GetServerType
}
//probably also need a way to ask the server type
#[derive(Serialize, Deserialize, Debug)]
pub enum TextServer{
    ServerTypeReq,
    ServerTypeText(ServerType),
    PathResolution, //text server asks all media servers which media he has
    SendFileList(Vec<String>), //send the resolved text list to the client
    PositionMedia(NodeId), //send exact position of the media to the client
    Text(FileMetaData)
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MediaServer{
    ServerTypeMedia(ServerType),
    SendPath(Vec<MediaId>), //send paths to the text server
    SendMedia(FileMetaData) //send correct media to the client who asked
}

//need to add the simulation control commands
//possibility:
#[derive(Serialize, Deserialize, Debug)]
pub enum ContentCommands{
    GetPathResolution, //sent to the text server, to resolve all the text files from media servers (1st step)
    GetTextList(NodeId), //sent to client, client needs to ask text server, node id text server? probably better if automated
    GetMediaPosition(NodeId, MediaId), //sent to client with id of media needed, same problem with id of text server
    GetMedia(NodeId, MediaId), //sent to client with id of media, node id of the media server, probably better automated if possible
    GetServerType(NodeId), //sent to client, node id of the server needed,
    GetText(NodeId, TextId), //sent to client, text id of the text file needed
    SearchTypeServers,
    SendTopologyGraph,
    Crash
}
pub enum BackGroundFlood{
    Start
}

//from client to SC
pub enum WebBrowserEvents{ //not complete
    TypeClient(ClientType, NodeId), //type and id client, sent to sc at the start
    MediaServers(NodeId, Vec<NodeId>), //node id client, list of media servers found after the SearchTypeServers command is sent
    TextServers(NodeId, Vec<NodeId>), //node id client, list of test servers found after the SearchTypeServers command is sent
    ListFiles(NodeId, Vec<String>), //node id client, list of all the available files
    MediaPosition(NodeId, NodeId), //node id of client and the node id of the media server where the media is located
    SavedTextFile(NodeId, String), //node id client, path to file saved in SC folder in multimedia
    SavedMedia(NodeId, String), //node id client, path to correct file save in SC folder in multimedia
    PacketInfo(NodeId, ContentType, u64),
    InfoRequest(NodeId, ContentRequest,  u64),
    Graph(NodeId, UnGraphMap<NodeId, u32>)
}
#[derive(Clone)]
pub enum ContentType{
    TextServerList(u64), //client sends to SC when {ContentType} is asked to server (u64 is #fragments)
    MediaServerList(u64),//server sends to SC when {ContentType} is sent back to client (u64 is #fragments)
    FileList(u64),
    MediaPosition(u64),
    SavedMedia(u64),
    SavedText(u64),
}

pub enum ContentRequest{
    AskTypes(u64),
    GetList(u64),
    GetPosition(u64),
    GetMedia(u64),
    GetText(u64)
}
impl Display for ContentType{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self{
            ContentType::TextServerList(_)=>{
                write!(f,"Fragment of a TextServer List packet")
            },
            ContentType::MediaServerList(_)=>{
                write!(f,"Fragment of a MediaServer List packet")
            },
            ContentType::FileList(_)=>{
                write!(f,"Fragment of a File List packet")
            },
            ContentType::MediaPosition(_)=>{
                write!(f,"Fragment of a MediaPosition packet")
            },
            ContentType::SavedMedia(_)=>{
                write!(f,"Fragment of a SavedMedia packet")
            },
            ContentType::SavedText(_)=>{
                write!(f,"Fragment of a SavedText packet")
            }
        }
    }
}