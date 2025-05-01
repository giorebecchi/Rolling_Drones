use std::collections::HashMap;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::GUI::login_window::SimulationController;
use crate::GUI::login_window::NodesConfig;
use wg_2024::network::NodeId;
use crate::common_things::common::ClientType;
use crate::GUI::chat_windows::{handle_clicks, OpenWindows};
use crate::GUI::login_window::AppState;

pub struct WebMediaPlugin;

impl Plugin for WebMediaPlugin{
    fn build(&self, app: &mut App){
        app
            .init_resource::<WebState>()
            .add_systems(Update, (handle_clicks,window_format).run_if(in_state(AppState::InGame)));

    }
}
#[derive(Resource,Default)]
pub struct WebState{
    pub text_servers: HashMap<NodeId, Vec<NodeId>>,
    pub media_servers: HashMap<NodeId, Vec<NodeId>>,
    pub media_paths: HashMap<NodeId, Vec<String>>,
    pub target_media_server: HashMap<NodeId,NodeId>,
    pub actual_media_path: HashMap<NodeId, String>,
    selected_text_server: HashMap<NodeId, Option<NodeId>>,
    selected_media_server: HashMap<NodeId, Option<NodeId>>,
    received_medias: HashMap<NodeId, Vec<String>>,

}
fn window_format(
    mut contexts: EguiContexts,
    mut sim: ResMut<SimulationController>,
    mut open_windows: ResMut<OpenWindows>,
    nodes: Res<NodesConfig>,
    mut web_state: ResMut<WebState>
)
{
    let mut windows_to_close=Vec::new();
    for (i, &(window_id, ref client_type)) in open_windows.windows.iter().enumerate(){
        if client_type.clone()==ClientType::WebBrowser{
            if !web_state.selected_text_server.contains_key(&window_id){
                web_state.selected_text_server.insert(window_id, None);
            }
            if !web_state.selected_media_server.contains_key(&window_id){
                web_state.selected_media_server.insert(window_id, None);
            }

            let window=egui::Window::new(format!("Client: {}",window_id))
                .id(egui::Id::new(format!("window_{}",window_id)))
                .resizable(true)
                .collapsible(true)
                .default_size([400.,500.]);
            let mut should_close=false;

            window.show(contexts.ctx_mut(), |ui|{
                ui.label(format!("This is a window for Client: {}",window_id));
                ui.separator();
                ui.horizontal(|ui|{
                    ui.label("Text Servers: ");
                    let current_server_text=match web_state.selected_text_server.get(&window_id).cloned().flatten(){
                        Some(server_id)=>format!("Server {}",server_id),
                        None=>"Select a server".to_string(),
                    };
                    egui::ComboBox::from_id_salt(format!("server_selector_{}", window_id))
                        .selected_text(current_server_text)
                        .show_ui(ui, |ui|{
                            let servers=web_state.text_servers.get(&window_id).cloned();
                            if let Some(text_servers)=servers{
                                for text_server in text_servers{
                                    let selected= web_state.selected_text_server.get(&window_id) == Some(&Some(text_server));
                                    if ui.selectable_label(selected, format!("Text_Server: {}",text_server)).clicked(){
                                        if web_state.selected_text_server.get(&window_id)==Some(&Some(text_server)) {
                                            web_state.selected_text_server.insert(window_id, None);
                                        } else{
                                            web_state.selected_text_server.insert(window_id, Some(text_server));
                                        }
                                    }
                                }
                            }

                        });
                    if ui.button("Ask for Medias").clicked(){
                        if let Some(selected_text_server)=web_state.selected_text_server.get(&window_id).cloned().flatten(){
                            sim.get_media_list(window_id, selected_text_server);
                        }

                    }
                });
                ui.separator();
                ui.heading("Available Medias");
                if let Some(paths)=web_state.media_paths.get(&window_id).cloned(){
                    for media_path in paths{
                        if ui.button(format!("{}",media_path)).clicked(){
                            if let Some(selected_text_server)=web_state.selected_text_server.get(&window_id).cloned().flatten() {
                                ui.label(format!("Searching for media: {}", media_path));
                                if let Some(medias)=web_state.received_medias.get_mut(&window_id){
                                    medias.push(media_path.clone());
                                }
                                sim.get_media_position(window_id, selected_text_server, media_path.clone());
                            }else{
                                ui.label("Searched failed, text server unreachable");
                            }

                        }
                    }
                }
                ui.separator();
                if let Some(media_server)=web_state.target_media_server.get(&window_id){
                    if let Some(medias)=web_state.received_medias.clone().get_mut(&window_id) {
                        if let Some(req_media)=medias.pop() {
                            sim.get_media_from(window_id, media_server.clone(), req_media);
                        }
                    }else{
                        ui.label("Failed to locate media");
                    }
                }
                if let Some(path_to_media)=web_state.actual_media_path.get(&window_id){
                    ui.image(path_to_media.clone());
                }
                ui.separator();
                if ui.button("Close Window").clicked() {
                    should_close = true;
                }
                if should_close {
                    windows_to_close.push(i);
                }



            });


        }
    }
    for i in windows_to_close.into_iter().rev() {
        open_windows.windows.remove(i);
    }
}
