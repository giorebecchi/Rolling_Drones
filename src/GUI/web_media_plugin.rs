use std::collections::HashMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use crate::GUI::login_window::SimulationController;
use crate::GUI::login_window::NodesConfig;
use wg_2024::network::NodeId;
use crate::GUI::chat_windows::handle_clicks;
use crate::GUI::login_window::Clickable;
use crate::GUI::login_window::AppState;

pub struct WebMediaPlugin;

impl Plugin for WebMediaPlugin{
    fn build(&self, app: &mut App){
        //app
           // .init_resource::<>()
            // .add_systems(Update, (handle_clicks,window_format).run_if(in_state(AppState::InGame)));

    }
}
#[derive(Resource,Default)]
pub struct WebState{
    received_medias: HashMap<NodeId, String>,

}
fn window_format(

)
{

}
