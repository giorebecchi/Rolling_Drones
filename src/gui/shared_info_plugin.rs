use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoSystemConfigs, NextState, Res, ResMut, Resource};
use once_cell::sync::Lazy;
use wg_2024::network::NodeId;
use crate::gui::chat_windows::ChatState;
use crate::gui::login_window::{AppState, NodeType};
use crate::gui::web_media_plugin::WebState;

pub static SHARED_STATE: Lazy<Arc<RwLock<ThreadInfo>>> = Lazy::new(|| {
    Arc::new(RwLock::new(ThreadInfo::default()))
});
pub static ERROR_VERIFY: Lazy<Arc<RwLock<TopologyError>>> = Lazy::new(|| {
    Arc::new(RwLock::new(TopologyError::default()))
});


#[derive(Default,Debug)]
pub struct ThreadInfo {
    pub nodes: HashMap<NodeId, NodeCategory>,
    pub n_clients: usize,
    pub client_types: Vec<(NodeType, NodeId)>,
    pub n_servers: usize,
    pub server_types: Vec<(NodeType, NodeId)>,
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
            .init_resource::<ErrorConfig>()
            .add_systems(Update, (sync_before_setup,sync_topology_error,evaluate_state).run_if(in_state(AppState::SetUp)))
            .add_systems(Update, sync_backend_to_frontend.run_if(in_state(AppState::InGame)));

    }
}
#[derive(Clone, Copy, Debug)]
pub enum NodeCategory {
    Client(NodeType),
    Server(NodeType),
}
#[derive(Resource,Default)]
pub struct SeenClients{
    pub nodes: HashMap<NodeId, NodeCategory>,
    pub clients : Vec<(NodeType,NodeId)>,
    clients_len: usize,
    pub servers: Vec<(NodeType, NodeId)>,
    servers_len: usize,
    pub ready_setup: bool,
}
pub struct TopologyError{
    pub connection_error: (bool, Result<(),String>),
    pub isolated_node: (bool, Result<(),String>),
    pub wrong_pdr: (bool, Result<(),String>),
    pub generic_misconfiguration: (bool, Result<(), String>),
    pub is_updated: bool,

}
impl Default for TopologyError{
    fn default() -> Self {
        TopologyError{
            connection_error: (false, Ok(())),
            isolated_node: (false, Ok(())),
            wrong_pdr: (false, Ok(())),
            generic_misconfiguration: (false, Ok(())),
            is_updated: false
        }
    }
}
#[derive(Resource, Default)]
pub struct ErrorConfig{
    pub error_pdr: String,
    pub error_isolated: String,
    pub error_connection: String,
    pub error_generic: String,
    updated: bool,
    pub detected: bool,
}
fn sync_topology_error(
    mut possible_error: ResMut<ErrorConfig>
){
    if let Ok(state) = ERROR_VERIFY.try_read() {
        if state.is_updated{
            let mut update1=false;
            let mut update2=false;
            let mut update3=false;
            let mut update4 = false;
            let mut error=false;
            possible_error.error_connection=match state.connection_error.clone().1{
                Ok(())=>{
                    update1=true;
                    String::new()
                },
                Err(err)=>{
                    error=true;
                    err
                }
            };
            possible_error.error_pdr=match state.wrong_pdr.clone().1{
                Ok(())=>{
                    update2=true;
                    String::new()
                },
                Err(err)=>{
                    error=true;
                    err
                }
            };
            possible_error.error_isolated=match state.isolated_node.clone().1{
                Ok(())=>{
                    update3=true;
                    String::new()
                },
                Err(err)=>{
                    error=true;
                    err
                },
            };
            possible_error.error_generic=match state.generic_misconfiguration.clone().1{
                Ok(())=>{
                    update4=true;
                    String::new()
                },
                Err(err)=>{
                    error=true;
                    err
                }
            };
            possible_error.updated=update1&&update2&&update3&&update4;
            possible_error.detected=error;


        }
        drop(state);

        if let Ok(mut state) = ERROR_VERIFY.try_write(){
            state.is_updated=false;
        }
    }
}

fn sync_before_setup(
    mut seen_clients: ResMut<SeenClients>,

){
    if let Ok(state) = SHARED_STATE.try_read() {
        if state.is_updated{


            seen_clients.clients=state.client_types.clone();
            seen_clients.clients_len+=state.n_clients;
            seen_clients.servers.extend(state.server_types.clone());
            seen_clients.servers_len+=state.n_servers;
            seen_clients.ready_setup=state.ready_setup;
            seen_clients.nodes=state.nodes.clone();


            }

            drop(state);

            if let Ok(mut state) = SHARED_STATE.try_write() {
                state.is_updated = false;
            }
        }


}


fn sync_backend_to_frontend(
    mut chat_state: ResMut<ChatState>,
    mut web_state: ResMut<WebState>
) {

    if let Ok(state) = SHARED_STATE.try_read() {
        if state.is_updated {

            chat_state.chat_responses = state.responses.clone();
            chat_state.registered_clients = state.registered_clients.clone();
            chat_state.chat_servers = state.chat_servers.clone();
            chat_state.chat_clients=state.chat_clients.clone();
            web_state.text_servers=state.text_servers.clone();
            web_state.media_servers=state.media_servers.clone();
            web_state.media_paths=state.client_medias.clone();
            web_state.target_media_server=state.target_media_server.clone();
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
    topology_error: Res<ErrorConfig>,
    mut next_state: ResMut<NextState<AppState>>
){
    if topology_error.updated{
        next_state.set(AppState::InGame)
    }
}
