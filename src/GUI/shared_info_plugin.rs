use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use bevy::app::{App, Plugin, Update};
use bevy::prelude::{ResMut, Resource};
use once_cell::sync::Lazy;
use wg_2024::network::NodeId;
use crate::GUI::chat_windows::ChatState;

pub static SHARED_STATE: Lazy<Arc<RwLock<ThreadInfo>>> = Lazy::new(|| {
    Arc::new(RwLock::new(ThreadInfo::default()))
});


#[derive(Default)]
pub struct ThreadInfo {
    pub responses: HashMap<(NodeId,(NodeId,NodeId)),Vec<String>>,
    pub client_list: HashMap<(NodeId,NodeId), Vec<NodeId>>,
    pub chat_servers: HashMap<NodeId, Vec<NodeId>>,
    pub registered_clients: HashMap<(NodeId,NodeId), bool>,
    pub is_updated: bool,

}


#[derive(Resource, Default)]
struct StateBridge;


pub struct BackendBridgePlugin;

impl Plugin for BackendBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateBridge>()
            .add_systems(Update, sync_backend_to_frontend);
    }
}


fn sync_backend_to_frontend(
    mut chat_state: ResMut<ChatState>,
) {

    if let Ok(state) = SHARED_STATE.try_read() {
        if state.is_updated {

            chat_state.chat_responses = state.responses.clone();
            // chat_state.client_list= state.client_list.clone();
            chat_state.registered_clients = state.registered_clients.clone();
            chat_state.chat_servers = state.chat_servers.clone();

            drop(state);

            if let Ok(mut state) = SHARED_STATE.try_write() {
                state.is_updated = false;
            }
        }
    }
}