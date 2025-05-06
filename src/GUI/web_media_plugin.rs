use std::collections::HashMap;
use std::fs;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::GUI::login_window::SimulationController;
use crate::GUI::login_window::NodesConfig;
use wg_2024::network::NodeId;
use crate::common_things::common::ClientType;
use crate::GUI::chat_windows::{handle_clicks, OpenWindows};
use crate::GUI::login_window::AppState;

pub struct WebMediaPlugin;

impl Plugin for WebMediaPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<WebState>()
            .init_resource::<ImageState>()
            .add_systems(Update, (handle_clicks, window_format).run_if(in_state(AppState::InGame)));
    }
}


#[derive(PartialEq, Eq, Clone, Debug)]
enum MediaDisplayType {
    Image,
    TextFile,
    None,
}

impl Default for MediaDisplayType {
    fn default() -> Self {
        MediaDisplayType::None
    }
}

#[derive(Resource, Default)]
struct ImageState {
    handles: HashMap<NodeId, Option<Handle<Image>>>,
    egui_textures: HashMap<NodeId, Option<(egui::TextureId, egui::Vec2)>>,
}

fn window_format(
    mut state: ResMut<ImageState>,
    asset_server: Res<AssetServer>,
    mut contexts: EguiContexts,
    mut sim: ResMut<SimulationController>,
    mut open_windows: ResMut<OpenWindows>,
    nodes: Res<NodesConfig>,
    images: Res<Assets<Image>>,
    mut web_state: ResMut<WebState>
) {
    let mut windows_to_close = Vec::new();

    for (i, &(window_id, ref client_type)) in open_windows.windows.iter().enumerate() {
        if client_type.clone() == ClientType::WebBrowser {
            if !web_state.selected_text_server.contains_key(&window_id) {
                web_state.selected_text_server.insert(window_id, None);
            }
            if !web_state.selected_media_server.contains_key(&window_id) {
                web_state.selected_media_server.insert(window_id, None);
            }
            if !state.handles.contains_key(&window_id) {
                state.handles.insert(window_id, None);
            }
            if !state.egui_textures.contains_key(&window_id) {
                state.egui_textures.insert(window_id, None);
            }
            if !web_state.current_display_type.contains_key(&window_id) {
                web_state.current_display_type.insert(window_id, MediaDisplayType::None);
            }


            if let Some(path) = web_state.actual_media_path.get(&window_id) {
                if state.handles.get(&window_id).unwrap_or(&None).is_none() {
                    println!("Loading image: {}", path);
                    let bevy_path = path.strip_prefix("assets/").unwrap_or(&path).to_string();
                    let handle: Handle<Image> = asset_server.load(bevy_path);
                    state.handles.insert(window_id, Some(handle));
                }
            }


            if let Some(Some(handle)) = state.handles.get(&window_id) {
                if state.egui_textures.get(&window_id).unwrap_or(&None).is_none() {
                    if let Some(image) = images.get(handle) {
                        let size = egui::Vec2::new(image.width() as f32, image.height() as f32);
                        let texture_id = contexts.add_image(handle.clone());
                        state.egui_textures.insert(window_id, Some((texture_id, size)));
                        println!("Image registered with egui: {}x{}", image.width(), image.height());
                    }
                }
            }


            let window = egui::Window::new(format!("Web Browser: {}", window_id))
                .id(egui::Id::new(format!("window_{}", window_id)))
                .resizable(true)
                .collapsible(true)
                .default_size([600., 700.]);

            let mut should_close = false;

            window.show(contexts.ctx_mut(), |ui| {
                ui.heading(format!("Web Browser Client: {}", window_id));
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Text Servers: ");
                    let current_server_text = match web_state.selected_text_server.get(&window_id).cloned().flatten() {
                        Some(server_id) => format!("Server {}", server_id),
                        None => "Select a server".to_string(),
                    };

                    egui::ComboBox::from_id_salt(format!("server_selector_{}", window_id))
                        .selected_text(current_server_text)
                        .show_ui(ui, |ui| {
                            let servers = web_state.text_servers.get(&window_id).cloned();
                            if let Some(text_servers) = servers {
                                for text_server in text_servers {
                                    let selected = web_state.selected_text_server.get(&window_id) == Some(&Some(text_server));
                                    if ui.selectable_label(selected, format!("Text_Server: {}", text_server)).clicked() {
                                        if web_state.selected_text_server.get(&window_id) == Some(&Some(text_server)) {
                                            web_state.selected_text_server.insert(window_id, None);
                                        } else {
                                            web_state.selected_text_server.insert(window_id, Some(text_server));
                                        }
                                    }
                                }
                            }
                        });

                    if ui.button("Ask for Medias").clicked() {
                        if let Some(selected_text_server) = web_state.selected_text_server.get(&window_id).cloned().flatten() {
                            sim.get_media_list(window_id, selected_text_server);
                        }
                    }
                });

                ui.separator();
                ui.heading("Available Medias");

                if let Some(paths) = web_state.media_paths.get(&window_id).cloned() {
                    let height = (paths.len() as f32 * 24.0).min(200.0);
                    egui::ScrollArea::vertical().max_height(height).show(ui, |ui| {
                        for media_path in paths {
                            if ui.button(format!("{}", media_path)).clicked() {
                                if let Some(selected_text_server) = web_state.selected_text_server.get(&window_id).cloned().flatten() {
                                    web_state.actual_media_path.remove(&window_id);
                                    web_state.actual_file_path.remove(&window_id);
                                    state.handles.insert(window_id, None);
                                    state.egui_textures.insert(window_id, None);
                                    web_state.current_display_type.insert(window_id, MediaDisplayType::None);

                                    web_state.received_medias.insert(window_id, media_path.clone());

                                    println!("Media clicked: {}", media_path);

                                    if media_path.ends_with(".txt") {
                                        println!("Processing as text file");
                                        web_state.current_display_type.insert(window_id, MediaDisplayType::TextFile);

                                        sim.get_text_file(window_id, selected_text_server, media_path.clone());
                                    } else {
                                        println!("Processing as image");
                                        web_state.current_display_type.insert(window_id, MediaDisplayType::Image);

                                        sim.get_media_position(window_id, selected_text_server, media_path.clone());
                                    }
                                } else {
                                    ui.label("Search failed, text server unreachable");
                                }
                            }
                        }
                    });
                } else {
                    ui.label("No media files available. Click 'Ask for Medias' to refresh.");
                }

                ui.separator();

                if let Some(media_server) = web_state.target_media_server.get(&window_id).cloned() {
                    if let Some(media_path) = web_state.received_medias.get(&window_id).cloned() {
                        println!("GUI called get_media_from for path: {}", media_path);
                        sim.get_media_from(window_id, media_server, media_path.clone());
                        web_state.received_medias.remove(&window_id);
                    }
                }

                ui.separator();
                ui.heading("Media View");

                match web_state.current_display_type.get(&window_id).unwrap_or(&MediaDisplayType::None) {
                    MediaDisplayType::Image => {
                        if let Some(Some((texture_id, original_size))) = state.egui_textures.get(&window_id) {
                            if let Some(Some(_)) = state.handles.get(&window_id) {
                                egui::ScrollArea::both().show(ui, |ui| {
                                    let available_width = ui.available_width();
                                    let available_height = 400.0;

                                    let scale_factor = (available_width / original_size.x)
                                        .min(available_height / original_size.y)
                                        .min(1.0);

                                    let display_size = egui::Vec2::new(
                                        original_size.x * scale_factor,
                                        original_size.y * scale_factor
                                    );

                                    let sized_texture = egui::load::SizedTexture::new(*texture_id, *original_size);
                                    let image_source = egui::ImageSource::Texture(sized_texture);

                                    let image_widget = egui::Image::new(image_source)
                                        .fit_to_exact_size(display_size);

                                    ui.add(image_widget);
                                });

                                ui.label(format!("Original size: {}x{} pixels", original_size.x, original_size.y));

                                if ui.button("Clear Image").clicked() {
                                    state.handles.insert(window_id, None);
                                    state.egui_textures.insert(window_id, None);
                                    web_state.actual_media_path.remove(&window_id);
                                    web_state.current_display_type.insert(window_id, MediaDisplayType::None);
                                }
                            } else {
                                ui.label("Image handle invalid. Loading...");
                            }
                        } else if web_state.actual_media_path.contains_key(&window_id) {
                            ui.label("Loading image...");
                        } else {
                            ui.label("Loading image...");
                        }
                    },
                    MediaDisplayType::TextFile => {
                        if let Some(path_to_file) = web_state.actual_file_path.get(&window_id) {
                            let text = fs::read_to_string(path_to_file);
                            if let Ok(content) = text {
                                println!("Displaying text file content from: {}", path_to_file);
                                egui::ScrollArea::vertical()
                                    .max_height(300.0)
                                    .show(ui, |ui| {
                                        ui.add(egui::TextEdit::multiline(&mut content.as_str())
                                            .desired_width(ui.available_width())
                                            .desired_rows(10)
                                            .interactive(false));
                                    });

                                if ui.button("Clear Text").clicked() {
                                    web_state.actual_file_path.remove(&window_id);
                                    web_state.current_display_type.insert(window_id, MediaDisplayType::None);
                                }
                            } else {
                                ui.label(format!("The path to text file: {} was incorrect", path_to_file));
                                web_state.actual_file_path.remove(&window_id);
                                web_state.current_display_type.insert(window_id, MediaDisplayType::None);
                            }
                        } else {
                            ui.label("Loading text file...");
                        }
                    },
                    MediaDisplayType::None => {
                        ui.label("No media to display");
                    }
                }

                ui.separator();
                if ui.button("Close Window").clicked() {
                    should_close = true;
                }
            });

            if should_close {
                windows_to_close.push(i);
                state.handles.insert(window_id, None);
                state.egui_textures.insert(window_id, None);
                web_state.actual_media_path.remove(&window_id);
                web_state.actual_file_path.remove(&window_id);
                web_state.selected_text_server.remove(&window_id);
                web_state.selected_media_server.remove(&window_id);
                web_state.received_medias.remove(&window_id);
                web_state.current_display_type.remove(&window_id);
            }
        }
    }

    for i in windows_to_close.into_iter().rev() {
        open_windows.windows.remove(i);
    }
}
#[derive(Resource, Default)]
pub struct WebState {
    pub text_servers: HashMap<NodeId, Vec<NodeId>>,
    pub media_servers: HashMap<NodeId, Vec<NodeId>>,
    pub media_paths: HashMap<NodeId, Vec<String>>,
    pub target_media_server: HashMap<NodeId, NodeId>,
    pub actual_media_path: HashMap<NodeId, String>,
    pub actual_file_path: HashMap<NodeId, String>,
    selected_text_server: HashMap<NodeId, Option<NodeId>>,
    selected_media_server: HashMap<NodeId, Option<NodeId>>,
    received_medias: HashMap<NodeId, String>,
    current_display_type: HashMap<NodeId, MediaDisplayType>,
}