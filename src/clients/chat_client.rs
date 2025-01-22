use bevy::log::tracing_subscriber::field::display::Messages;
use wg_2024::packet;
use wg_2024::controller;
use wg_2024::config;
use serde::{Serialize, Deserialize};
use wg_2024::config::Client;

pub struct ChatClient {
    config: config::Client,
}

pub enum CommandChat {
    RegisterClient,
    GetListClients,
    SendMessage(Messages<MessageClient>), //?
    EndChat
}
pub struct CommunicationServer{} //destination of the message sent by the struct

pub struct MessageClient{
    content: String,
    //from:
    //to:
}
impl  ChatClient {
    pub fn register_client(&self)-> CommandChat{
        CommandChat::RegisterClient
    }
    pub fn get_list(&self)->CommandChat{
        CommandChat::GetListClients
    }
    pub fn send_message(&self, message_client: MessageClient){
        CommandChat::SendMessage(Messages::new(message_client)); //?
    }
    pub fn end_chat(&self)->CommandChat{
        CommandChat::EndChat
    }
}

pub fn run_commands_chat(command: CommandChat){
    match command{
        _=>todo!(),
    }
}

