use crossbeam_channel::Sender;
use petgraph::Graph;
use petgraph::prelude::UnGraphMap;
use serde::{Deserialize, Serialize};
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;
use crate::gui::login_window::NodeType as MyNodeType;

//comandi sim_control
#[derive(Clone)]
pub enum CommandChat {
    SearchChatServers,
    RegisterClient(NodeId),//node id server
    SendMessage(NodeId, NodeId, String),//node id del client a cui mandare la string, node id server da cui passare
    SendTopologyGraph,
    RemoveSender(NodeId),
    AddSender(NodeId, Sender<Packet>), //works the same as drones
    PdrChanged(NodeId)
}
///The NodeId identifies the client that sent the ChatClientEvent
#[derive(Debug)]
pub enum ChatClientEvent{
    ClientList((NodeId, NodeId) ,Vec<NodeId>), //NodeId Client, NodeId Server, Vec<ClientIds>
    IncomingMessage((NodeId,NodeId,NodeId),String), //NodeId Client a cui è arrivato msg, NodeId server, NodeId del client da cui il messaggio è arrivato msg
    RegisteredSuccess((NodeId,NodeId),Result<(), String>), //NodeId registered client and NodeId server { either Ok(()) or Err("something".to_string()) }
    ChatServers(NodeId, Vec<NodeId>),
    ClientType(ClientType,NodeId),
    InfoRequest(NodeId, RequestEvent, u64),
    Graph(NodeId, UnGraphMap<NodeId, u32>)
}

#[derive(Debug)]
pub enum RequestEvent{
    AskType(u64),
    Register(u64),
    SendMessage(u64),
}

pub enum ServerCommands{
    SendTopologyGraph,
    AddSender(NodeId, Sender<Packet>),
    RemoveSender(NodeId),
    PdrChanged(NodeId)
}

#[derive(Debug)]
pub enum ServerEvent{
    Graph(NodeId, Graph<(NodeId, wg_2024::packet::NodeType), f64, petgraph::Directed>),
    GraphMax(NodeId, Vec<(NodeId, wg_2024::packet::NodeType, Vec<NodeId>)>), // (id server, Vec<(id of the node, packet type of the node, node connections)>)
    TextPacketInfo(NodeId, MyNodeType, TextServerEvent, u64), //(id server, server_type (ChatServer, TextServer,...), type of message, session_id)
    MediaPacketInfo(NodeId, MyNodeType, MediaServerEvent, u64), //(id server, server_type (ChatServer, TextServer,...), type of message, session_id)
    ChatPacketInfo(NodeId, MyNodeType, ChatServerEvent, u64)  //(id server, server_type (ChatServer, TextServer,...), type of message, session_id)
}
#[derive(Debug,Clone)]
pub enum ChatServerEvent{
    SendingServerTypeChat(u64),
    ClientRegistration(u64),
    SendingClientList(u64),
    ForwardingMessage(u64),
    ClientElimination(u64),
}
#[derive(Debug,Clone)]
pub enum TextServerEvent{
    SendingFileList(u64),
    SendingPosition(u64),
    SendingText(u64),
    SendingServerTypeText(u64),
    SendingServerTypeReq(u64),
    AskingForPathRes(u64),
}
#[derive(Debug,Clone)]
pub enum MediaServerEvent{
    SendingServerTypeMedia(u64),
    SendingPathRes(u64), //send paths to the text server
    SendingMedia(u64)
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

#[derive(Debug)]
pub enum ContentCommands{
    GetTextList(NodeId), //sent to client, client needs to ask text server, node id text server? probably better if automated
    GetMediaPosition(NodeId, MediaId), //sent to client with id of media needed, same problem with id of text server
    GetMedia(NodeId, MediaId), //sent to client with id of media, node id of the media server, probably better automated if possible
    GetText(NodeId, TextId), //sent to client, text id of the text file needed
    SearchTypeServers,
    SendTopologyGraph,
    AddSender(NodeId, Sender<Packet>),
    RemoveSender(NodeId),
    PdrChanged(NodeId)
}
pub enum BackGroundFlood{
    Start
}

//from client to SC
#[derive(Debug)]
pub enum WebBrowserEvents{ //not complete
    MediaServers(NodeId, Vec<NodeId>), //node id client, list of media servers found after the SearchTypeServers command is sent
    TextServers(NodeId, Vec<NodeId>), //node id client, list of test servers found after the SearchTypeServers command is sent
    ListFiles(NodeId, Vec<String>), //node id client, list of all the available files
    MediaPosition(NodeId, NodeId), //node id of client and the node id of the media server where the media is located
    SavedTextFile(NodeId, String), //node id client, path to file saved in SC folder in multimedia
    SavedMedia(NodeId, String),
    InfoRequest(NodeId, ContentRequest,  u64),
    Graph(NodeId, UnGraphMap<NodeId, u32>)
}
#[derive(Debug)]
pub enum ContentRequest{
    AskTypes(u64),
    GetList(u64),
    GetPosition(u64),
    GetMedia(u64),
    GetText(u64)
}
