use bevy::utils::HashMap;
use wg_2024::packet;
use wg_2024::controller;
use wg_2024::config;
use serde::{Serialize, Deserialize};
use wg_2024::config::Client;
use wg_2024::network::NodeId;
use wg_2024::packet::{Packet, FRAGMENT_DSIZE};
use crate::clients::assembler::{Fragmentation, MessageChat, Serialization};

#[derive(Clone)]
pub struct ChatClient {
    config: Client,
}

#[derive(Debug)]
pub enum CommandChat {
    ServerType,
    RegisterClient,
    GetListClients,
    SendMessage(MessageChat),
    EndChat
}
pub struct CommunicationServer{ //just to have a reference (sarà implementato dagli altri)
    registered_clients: Vec<ChatClient>,
}


impl ChatClient {
    pub fn get_server_type(&self)-> CommandChat{ CommandChat::ServerType }
    pub fn register_client(&self)-> CommandChat{
        CommandChat::RegisterClient
    }
    pub fn get_list(&self)->CommandChat{
        CommandChat::GetListClients
    }
    pub fn send_message(&self, content: String, to_id: NodeId)-> CommandChat{
        let message_client = MessageChat{ //to create the message given the content and the destination, needs to be fragmented
            content,
            from_id: self.config.id.clone(),
            to_id
        };
        CommandChat::SendMessage(message_client)
    }
    pub fn end_chat(&self)->CommandChat{
        CommandChat::EndChat
    }
}

pub fn run_commands_chat(client: &ChatClient, server: &mut CommunicationServer, command: CommandChat){
    match command{
        CommandChat::ServerType=>{
            println!("Asking server what type it is");

        }
        CommandChat::RegisterClient => {
            println!("Registering Client {} to Communication Server: ", client.config.id);
            register_clients(&client, server)
        },
        CommandChat::GetListClients => {
            println!("Get the list of registered clients: ");

        },
        CommandChat::SendMessage(message_client) => {
            println!("Sending message to Server to Communication server");
            send_message(&client, &message_client);

        },
        CommandChat::EndChat => {
            println!("Ending the chat");

        }

    }
}

pub fn register_clients(client: &ChatClient, communication_server: & mut CommunicationServer){
    communication_server.registered_clients.push(client.clone())
}
pub fn send_message(client: &ChatClient, message_chat: &MessageChat){
    //to find at which drone it has to send the message it needs to start a flood
    //serialize message
    let serialized_message = message_chat.clone().stringify();
    let mut fragments = HashMap::new();
    //fragment message
    if message_chat.content.len() <= FRAGMENT_DSIZE{
        fragments = message_chat.fragment_message();
    }
    println!("fragments sent: {:?}", fragments)

}

pub fn main(){
    let chat_client = ChatClient{
        config: Client{
            id: 2,
            connected_drone_ids: vec![1,3]
        }
    };
    let mut server_test = CommunicationServer{
        registered_clients: vec![ChatClient{
            config: Client{
                id: 5,
                connected_drone_ids: vec![1,3]
            }
        }],
    };
    let command = chat_client.send_message("hello".to_string(), 3);
    println!("{:?}", command);
    run_commands_chat(&chat_client,&mut server_test,command );
}
