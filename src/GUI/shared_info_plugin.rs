use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoSystemConfigs, NextState, ResMut, Resource};
use once_cell::sync::Lazy;
use wg_2024::network::NodeId;
use crate::common_things::common::ClientType;
use crate::GUI::chat_windows::ChatState;
use crate::GUI::login_window::{AppState, NodeConfig, NodeType, NodesConfig};
use crate::GUI::web_media_plugin::WebState;

pub static SHARED_STATE: Lazy<Arc<RwLock<ThreadInfo>>> = Lazy::new(|| {
    Arc::new(RwLock::new(ThreadInfo::default()))
});


#[derive(Default)]
pub struct ThreadInfo {
    pub n_clients: usize,
    pub client_types: Vec<(ClientType, NodeId)>,
    pub responses: HashMap<(NodeId,(NodeId,NodeId)),Vec<String>>,
    pub client_list: HashMap<(NodeId,NodeId), Vec<NodeId>>,
    pub chat_servers: HashMap<NodeId, Vec<NodeId>>,
    pub registered_clients: HashMap<(NodeId,NodeId), bool>,
    pub chat_clients: Vec<NodeId>,
    pub web_clients: Vec<NodeId>,
    pub text_servers: HashMap<NodeId, Vec<NodeId>>,
    pub media_servers: HashMap<NodeId, Vec<NodeId>>,
    pub client_medias: HashMap<NodeId, Vec<String>>,
    pub target_media_server: HashMap<NodeId, NodeId>,
    pub actual_media_path: HashMap<NodeId, String>,
    pub actual_file_path: HashMap<NodeId, String>,
    pub is_updated: bool,
    pub ready_setup: bool,

}


#[derive(Resource, Default)]
struct StateBridge;


pub struct BackendBridgePlugin;

impl Plugin for BackendBridgePlugin {
    fn build(&self, app: &mut App) {
        app

            .init_resource::<StateBridge>()
            .init_resource::<SeenClients>()
            .add_systems(Update, (sync_before_setup,evaluate_state).run_if(in_state(AppState::SetUp)))
            .add_systems(Update, sync_backend_to_frontend.run_if(in_state(AppState::InGame)));
    }
}
#[derive(Resource,Default)]
pub struct SeenClients{
    pub clients : Vec<(ClientType,NodeId)>,
    len: usize,
}

fn sync_before_setup(
    mut seen_clients: ResMut<SeenClients>,

){
    if let Ok(state) = SHARED_STATE.try_read() {
        if state.is_updated{

                seen_clients.clients=state.client_types.clone();
                seen_clients.len=state.n_clients;


            }

            drop(state);

            if let Ok(mut state) = SHARED_STATE.try_write() {
                state.is_updated = false;
            }
        }


}


fn sync_backend_to_frontend(
    mut chat_state: ResMut<ChatState>,
    mut web_state: ResMut<WebState>,
) {

    if let Ok(state) = SHARED_STATE.try_read() {
        if state.is_updated {

            chat_state.chat_responses = state.responses.clone();
            // chat_state.client_list= state.client_list.clone();
            chat_state.registered_clients = state.registered_clients.clone();
            chat_state.chat_servers = state.chat_servers.clone();
            println!("chat_servers: {:?}",chat_state.chat_servers);
            chat_state.chat_clients=state.chat_clients.clone();
            web_state.text_servers=state.text_servers.clone();
            println!("text_servers: {:?}", web_state.text_servers);
            web_state.media_servers=state.media_servers.clone();
            println!("media_servers: {:?}", web_state.media_servers);
            web_state.media_paths=state.client_medias.clone();
            web_state.target_media_server=state.target_media_server.clone();
            println!("target_media_server: {:?}",web_state.target_media_server);
            web_state.actual_media_path=state.actual_media_path.clone();
            web_state.actual_file_path=state.actual_file_path.clone();

            drop(state);

            if let Ok(mut state) = SHARED_STATE.try_write() {
                state.is_updated = false;
            }
        }
    }
}
fn evaluate_state(
    mut seen_clients: ResMut<SeenClients>,
    mut next_state: ResMut<NextState<AppState>>
){
    if seen_clients.len==seen_clients.clients.len(){
        next_state.set(AppState::InGame);
    }

}