use std::collections::HashMap;
use std::num::NonZero;
use std::sync::{Arc, RwLock};
use bevy::app::{App, AppExit, Plugin, Update};
use bevy::prelude::{in_state, EventWriter, IntoSystemConfigs, NextState, Res, ResMut, Resource};
use once_cell::sync::Lazy;
use wg_2024::network::NodeId;
use crate::gui::chat_windows::ChatState;
use crate::gui::login_window::AppState;
use crate::gui::web_media_plugin::WebState;
use crate::simulation_control::simulation_control::MyNodeType;

pub static SHARED_STATE: Lazy<Arc<RwLock<ThreadInfo>>> = Lazy::new(|| {
    Arc::new(RwLock::new(ThreadInfo::default()))
});


#[derive(Default,Debug)]
pub struct ThreadInfo {
    pub nodes: HashMap<NodeId, NodeCategory>,
    pub n_clients: usize,
    pub client_types: Vec<(MyNodeType, NodeId)>,
    pub n_servers: usize,
    pub server_types: Vec<(MyNodeType, NodeId)>,
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
    pub wrong_connections: HashMap<NodeId, Vec<NodeId>>,
    pub incomplete_connections: Vec<(NodeId, NodeId)>,
    pub wrong_pdr: HashMap<NodeId, bool>,
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
            .init_resource::<VerifyTopology>()
            .init_resource::<SeenClients>()
            .add_systems(Update, (sync_before_setup,evaluate_state).run_if(in_state(AppState::SetUp)))
            .add_systems(Update, sync_backend_to_frontend.run_if(in_state(AppState::InGame)))
            .add_systems(Update, check_topology);
    }
}
#[derive(Clone, Copy, Debug)]
pub enum NodeCategory {
    Client(MyNodeType),
    Server(MyNodeType),
}
#[derive(Resource,Default)]
pub struct SeenClients{
    pub nodes: HashMap<NodeId, NodeCategory>,
    pub clients : Vec<(MyNodeType,NodeId)>,
    clients_len: usize,
    pub servers: Vec<(MyNodeType, NodeId)>,
    servers_len: usize,
    pub ready_setup: bool,
}
#[derive(Resource, Default)]
pub struct VerifyTopology{
    node_connection: HashMap<NodeId, Vec<NodeId>>,
    incomplete_connections: Vec<(NodeId, NodeId)>,
    wrong_pdr: HashMap<NodeId, bool>

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
    mut web_state: ResMut<WebState>,
    mut topology: ResMut<VerifyTopology>
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
            topology.node_connection=state.wrong_connections.clone();
            topology.incomplete_connections=state.incomplete_connections.clone();
            topology.wrong_pdr=state.wrong_pdr.clone();



            drop(state);

            if let Ok(mut state) = SHARED_STATE.try_write() {
                state.is_updated = false;
            }
        }
    }
}
fn evaluate_state(
    seen_clients: ResMut<SeenClients>,
    mut next_state: ResMut<NextState<AppState>>
){

    if seen_clients.clients_len==seen_clients.clients.len()&&seen_clients.servers_len==seen_clients.servers.len()&&seen_clients.ready_setup{
        println!("is it true {}", seen_clients.ready_setup);
        next_state.set(AppState::InGame);

    }

}
fn check_topology(
    topology: Res<VerifyTopology>,
    mut exit : EventWriter<AppExit>
){
    if !topology.node_connection.is_empty() || !topology.incomplete_connections.is_empty() || !topology.wrong_pdr.is_empty(){
        let exit_code=NonZero::new(12).unwrap();
        println!("Check the toml for errors in the configuration!!");
        println!("Errors can be: \nclients/servers without a connection: {:?}\nnot full duplex connections: {:?}\n wrong pdr: {:?}", topology.node_connection, topology.incomplete_connections, topology.wrong_pdr);
        exit.send(AppExit::Error(exit_code));

    }
}