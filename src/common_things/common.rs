use wg_2024::network::NodeId;

//comandi sim_control
pub enum CommandChat {
    ServerType(NodeId),//node id server
    RegisterClient(NodeId),//node id server
    GetListClients(NodeId),//node id server
    SendMessage(NodeId, String),//node id del client a cui mandare la string
    EndChat(NodeId),//node id del server
}

//comandi da client a server
pub enum ChatRequest{
    ServerType,
    RegisterClient(NodeId),//node id del client stesso
    GetListClients,
    SendMessage(MessageChat),
    EndChat(NodeId),//node id del client stesso
}

pub struct MessageChat{ //which needs to be fragmented
    pub content: String,
    pub from_id: NodeId,//id client sender
    pub to_id: NodeId //id destination client
}

pub enum ServerType{
    ComunicationServer,
    TesxtServer,
    MediaServer
}

//poi bisogna fare la stessa cosa anche per il text e media