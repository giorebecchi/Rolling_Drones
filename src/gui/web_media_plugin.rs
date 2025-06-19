use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::simulation_control::simulation_control::SimulationController;
use wg_2024::network::NodeId;
use crate::common_data::common::ClientType;
use crate::gui::chat_windows::{handle_clicks, OpenWindows};
use crate::gui::login_window::AppState;

pub struct WebMediaPlugin;

impl Plugin for WebMediaPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<WebState>()
            .init_resource::<ImageState>()
            .init_resource::<TextFileCache>()
            .add_systems(Update, (handle_clicks, window_format).run_if(in_state(AppState::InGame)));
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
enum MediaDisplayType {
    Image (String),
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
    egui_textures: HashMap<NodeId, Option<(egui::TextureId, egui::Vec2)>>
}

#[derive(Resource)]
struct TextFileCache {
    file_lines: HashMap<NodeId, Vec<String>>,
    current_page: HashMap<NodeId, usize>,
    total_lines: HashMap<NodeId, usize>,
    lines_per_page: usize,
    page_input: HashMap<NodeId, String>,
}

impl Default for TextFileCache {
    fn default() -> Self {
        Self {
            file_lines: HashMap::new(),
            current_page: HashMap::new(),
            total_lines: HashMap::new(),
            lines_per_page: 50,
            page_input: HashMap::new(),
        }
    }
}

impl TextFileCache {


    fn load_file(&mut self, window_id: NodeId, path: &str) -> Result<(), std::io::Error> {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;

        let total = lines.len();
        self.file_lines.insert(window_id, lines);
        self.total_lines.insert(window_id, total);
        self.current_page.insert(window_id, 0);
        self.page_input.insert(window_id, "1".to_string());

        Ok(())
    }

    fn get_page_content(&self, window_id: &NodeId) -> Option<String> {
        let lines = self.file_lines.get(window_id)?;
        let page = self.current_page.get(window_id).copied().unwrap_or(0);

        if self.lines_per_page == 0 {
            return Some(String::new());
        }

        let start = page.saturating_mul(self.lines_per_page);
        let end = (page + 1).saturating_mul(self.lines_per_page).min(lines.len());

        if start >= lines.len() {
            return Some(String::new());
        }

        Some(lines[start..end].join("\n"))
    }

    fn get_total_pages(&self, window_id: &NodeId) -> usize {
        let total = self.total_lines.get(window_id).copied().unwrap_or(0);

        if total == 0 || self.lines_per_page == 0 {
            return 1;
        }

        total.saturating_add(self.lines_per_page - 1) / self.lines_per_page
    }

    fn next_page(&mut self, window_id: NodeId) {
        let total_pages = self.get_total_pages(&window_id);
        if let Some(page) = self.current_page.get_mut(&window_id) {
            if *page + 1 < total_pages {
                *page += 1;
                self.page_input.insert(window_id, (*page + 1).to_string());
            }
        }
    }

    fn prev_page(&mut self, window_id: NodeId) {
        if let Some(page) = self.current_page.get_mut(&window_id) {
            *page = page.saturating_sub(1);
            self.page_input.insert(window_id, (*page + 1).to_string());
        }
    }

    fn clear(&mut self, window_id: NodeId) {
        self.file_lines.remove(&window_id);
        self.current_page.remove(&window_id);
        self.total_lines.remove(&window_id);
        self.page_input.remove(&window_id);
    }
}

fn window_format(
    mut state: ResMut<ImageState>,
    asset_server: Res<AssetServer>,
    mut contexts: EguiContexts,
    sim: ResMut<SimulationController>,
    mut open_windows: ResMut<OpenWindows>,
    images: Res<Assets<Image>>,
    mut web_state: ResMut<WebState>,
    mut text_cache: ResMut<TextFileCache>,
) {
    let mut windows_to_close = Vec::new();
    let mut images_to_remove = Vec::new();

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


            if let Some(path) = web_state.actual_media_path.get(&window_id).cloned() {
                let still_relevant = web_state
                    .currently_selected_media
                    .get(&window_id)
                    .and_then(|p| p.as_ref())
                    .map_or(false, |wanted| trim_into_file_name(&path) == wanted.clone());

                if still_relevant && !path.ends_with(".txt") {
                    web_state.current_display_type
                        .insert(window_id, MediaDisplayType::Image(path.clone()));
                }
                let current_path = web_state.last_loaded_path.get(&window_id).cloned().unwrap_or_default();

                if current_path != *path {
                    if let Some(Some(texture)) = state.handles.get(&window_id) {
                        images_to_remove.push(texture.clone());
                    }
                    state.egui_textures.insert(window_id, None);
                    state.handles.insert(window_id, None);

                    web_state.last_loaded_path.insert(window_id, path.clone());

                    let bevy_path = path.strip_prefix("assets/").unwrap_or(&path).to_string();
                    let handle: Handle<Image> = asset_server.load(bevy_path);
                    state.handles.insert(window_id, Some(handle));
                } else if state.handles.get(&window_id).unwrap_or(&None).is_none() {
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
                    }
                }
            }

            let window_ui_id = egui::Id::new(format!("web_browser_window_{}", window_id));

            let window = egui::Window::new(format!("Web Browser: {}", window_id))
                .id(window_ui_id)
                .resizable(true)
                .collapsible(true)
                .default_size([600., 700.]);

            let mut should_close = false;
            let mut should_clear_image = false;

            if let Some(ctx)=contexts.try_ctx_mut() {
                window.show(ctx, |ui| {
                    ui.heading(format!("Web Browser Client: {}", window_id));
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Text Servers: ");
                        let current_server_text = match web_state.selected_text_server.get(&window_id).cloned().flatten() {
                            Some(server_id) => format!("Server {}", server_id),
                            None => "Select a server".to_string(),
                        };

                        let combo_id = egui::Id::new(format!("server_selector_combo_{}", window_id));

                        egui::ComboBox::from_id_salt(combo_id)
                            .selected_text(current_server_text)
                            .show_ui(ui, |ui| {
                                let servers = web_state.text_servers.get(&window_id).cloned();
                                if let Some(text_servers) = servers {
                                    for text_server in text_servers {
                                        let selectable_id = ui.make_persistent_id(format!("text_server_{}_{}", window_id, text_server));
                                        ui.push_id(selectable_id, |ui| {
                                            let selected = web_state.selected_text_server.get(&window_id) == Some(&Some(text_server));
                                            if ui.selectable_label(selected, format!("Text_Server: {}", text_server)).clicked() {
                                                if web_state.selected_text_server.get(&window_id) == Some(&Some(text_server)) {
                                                    web_state.selected_text_server.insert(window_id, None);
                                                } else {
                                                    web_state.selected_text_server.insert(window_id, Some(text_server));
                                                }
                                            }
                                        });
                                    }
                                }
                            });

                        let button_id = ui.make_persistent_id(format!("ask_medias_button_{}", window_id));
                        ui.push_id(button_id, |ui| {
                            if ui.button("Ask for Medias").clicked() {
                                if let Some(selected_text_server) = web_state.selected_text_server.get(&window_id).cloned().flatten() {
                                    web_state.server_for_current_media.insert(window_id, Some(selected_text_server));
                                    sim.get_media_list(window_id, selected_text_server);
                                }
                            }
                        });
                    });

                    ui.separator();
                    ui.heading("Available Medias");

                    let scroll_area_id = ui.make_persistent_id(format!("media_scroll_area_{}", window_id));

                    let current_selected_server = web_state.selected_text_server.get(&window_id).cloned();
                    let server_used_for_media = web_state.server_for_current_media.get(&window_id).cloned();

                    let should_show_media = match (current_selected_server, server_used_for_media) {
                        (Some(current), Some(used)) => current == used,
                        _ => false,
                    };

                    if should_show_media {
                        if let Some(paths) = web_state.media_paths.get(&window_id).cloned() {
                            let height = (paths.len() as f32 * 24.0).min(200.0);
                            ui.push_id(scroll_area_id, |ui| {
                                egui::ScrollArea::vertical().max_height(height).show(ui, |ui| {
                                    for (idx, media_path) in paths.iter().enumerate() {
                                        let media_button_id = ui.make_persistent_id(format!("media_button_{}_{}", window_id, idx));
                                        ui.push_id(media_button_id, |ui| {
                                            let is_currently_selected = web_state.currently_selected_media
                                                .get(&window_id)
                                                .and_then(|selected| selected.as_ref())
                                                == Some(media_path);
                                            let button = egui::Button::new(format!("{}",media_path));
                                            let response = if is_currently_selected {
                                                ui.add_enabled(false, button)
                                            }else{
                                                ui.add(button)
                                            };
                                            if response.clicked(){
                                                if let Some(selected_text_server) = web_state.selected_text_server.get(&window_id).cloned().flatten() {
                                                    if let Some(_) = state.handles.get(&window_id) {
                                                        should_clear_image = true
                                                    }

                                                    web_state.currently_selected_media.insert(window_id, Some(media_path.clone()));
                                                    web_state.actual_media_path.remove(&window_id);
                                                    web_state.media_paths.remove(&window_id);
                                                    web_state.actual_file_path.remove(&window_id);
                                                    web_state.received_medias.remove(&window_id);
                                                    web_state.media_servers.remove(&window_id);
                                                    web_state.target_media_server.remove(&window_id);
                                                    state.handles.insert(window_id, None);
                                                    state.egui_textures.insert(window_id, None);
                                                    web_state.current_display_type.insert(window_id, MediaDisplayType::None);
                                                    web_state.last_loaded_path.remove(&window_id);
                                                    text_cache.clear(window_id);

                                                    if media_path.ends_with(".txt") {
                                                        web_state.loading_image.insert(window_id, String::new());
                                                        web_state.current_display_type.insert(window_id, MediaDisplayType::TextFile);
                                                        sim.get_text_file(window_id, selected_text_server, media_path.clone());
                                                    } else {
                                                        web_state.received_medias.insert(window_id, media_path.clone());
                                                        sim.get_media_position(window_id, selected_text_server, media_path.clone());
                                                    }
                                                } else {
                                                    ui.label("Search failed, text server unreachable");
                                                }
                                            };
                                        });
                                    }
                                });
                            });
                        } else {
                            ui.label("No media files available. Click 'Ask for Medias' to refresh.");
                        }
                    } else {
                        ui.label("No media files available. Click 'Ask for Medias' to refresh.");
                        if current_selected_server != server_used_for_media {
                            web_state.media_paths.remove(&window_id);
                            web_state.server_for_current_media.remove(&window_id);
                        }
                    }

                    ui.separator();

                    if let Some(media_server) = web_state.target_media_server.get(&window_id).cloned() {
                        if let Some(media_path) = web_state.received_medias.remove(&window_id) {
                            web_state.loading_image.insert(window_id, media_path.clone());
                            sim.get_media_from(window_id, media_server, media_path.clone());
                        }
                    }

                    ui.separator();
                    ui.heading("Media View");

                    let media_view_id = ui.make_persistent_id(format!("media_view_area_{}", window_id));
                    ui.push_id(media_view_id, |ui| {
                        match web_state.current_display_type.get(&window_id).unwrap_or(&MediaDisplayType::None) {
                            MediaDisplayType::Image(media_path) => {
                                if let Some(Some((texture_id, original_size))) = state.egui_textures.get(&window_id) {
                                    if let Some(Some(_)) = state.handles.get(&window_id) {
                                        if let Some(arrived)=web_state.loading_image.get(&window_id){
                                            if trim_into_file_name(media_path)==*arrived{
                                                let image_scroll_id = ui.make_persistent_id(format!("image_scroll_{}", window_id));
                                                ui.push_id(image_scroll_id, |ui| {
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
                                                });

                                                ui.label(format!("Original size: {}x{} pixels", original_size.x, original_size.y));

                                                let clear_button_id = ui.make_persistent_id(format!("clear_image_button_{}", window_id));
                                                ui.push_id(clear_button_id, |ui| {
                                                    if ui.button("Clear Image").clicked() {
                                                        should_clear_image = true;

                                                        if let None = web_state.actual_media_path.get(&window_id) {
                                                            println!("Error occured while clearing image");
                                                        }

                                                        state.handles.insert(window_id, None);
                                                        state.egui_textures.insert(window_id, None);
                                                        web_state.received_medias.remove(&window_id);
                                                        web_state.actual_media_path.remove(&window_id);
                                                        web_state.last_loaded_path.remove(&window_id);
                                                        web_state.media_servers.remove(&window_id);
                                                        web_state.target_media_server.remove(&window_id);
                                                        web_state.current_display_type.insert(window_id, MediaDisplayType::None);
                                                    }
                                                });
                                            }else{
                                                ui.label("Loading image...");
                                            }
                                        }
                                    } else {
                                        ui.label("Image handle invalid. Loading...");
                                    }
                                } else {
                                    ui.label("Loading image...");
                                }
                            },
                            MediaDisplayType::TextFile => {
                                if let Some(path_to_file) = web_state.actual_file_path.get(&window_id) {
                                    if !text_cache.file_lines.contains_key(&window_id) {
                                        match text_cache.load_file(window_id, path_to_file) {
                                            Ok(_) => {},
                                            Err(e) => {
                                                ui.label(format!("Error loading file: {}", e));
                                                web_state.actual_file_path.remove(&window_id);
                                                web_state.current_display_type.insert(window_id, MediaDisplayType::None);
                                                return;
                                            }
                                        }
                                    }

                                    if let Some(content) = text_cache.get_page_content(&window_id) {
                                        let text_scroll_id = ui.make_persistent_id(format!("text_scroll_{}", window_id));
                                        ui.push_id(text_scroll_id, |ui| {
                                            egui::ScrollArea::vertical()
                                                .max_height(300.0)
                                                .show(ui, |ui| {
                                                    ui.add(egui::TextEdit::multiline(&mut content.as_str())
                                                        .desired_width(ui.available_width())
                                                        .desired_rows(10)
                                                        .interactive(false)
                                                        .id(ui.make_persistent_id(format!("text_edit_{}", window_id))));
                                                });
                                        });

                                        ui.horizontal(|ui| {
                                            let current_page = text_cache.current_page.get(&window_id).copied().unwrap_or(0);
                                            let total_pages = text_cache.get_total_pages(&window_id);
                                            let total_lines = text_cache.total_lines.get(&window_id).copied().unwrap_or(0);

                                            ui.label(format!("Page {} of {} | Total lines: {}",
                                                             current_page + 1, total_pages, total_lines));

                                            ui.add_space(10.0);

                                            let prev_button_id = ui.make_persistent_id(format!("prev_page_{}", window_id));
                                            ui.push_id(prev_button_id, |ui| {
                                                if ui.add_enabled(current_page > 0, egui::Button::new("◀ Previous")).clicked() {
                                                    text_cache.prev_page(window_id);
                                                }
                                            });

                                            let next_button_id = ui.make_persistent_id(format!("next_page_{}", window_id));
                                            ui.push_id(next_button_id, |ui| {
                                                if ui.add_enabled(current_page + 1 < total_pages, egui::Button::new("Next ▶")).clicked() {
                                                    text_cache.next_page(window_id);
                                                }
                                            });

                                            ui.add_space(10.0);

                                            ui.label("Go to page:");

                                            let page_input = text_cache.page_input
                                                .entry(window_id)
                                                .or_insert_with(|| (current_page + 1).to_string());

                                            let text_input_id = ui.make_persistent_id(format!("page_input_{}", window_id));
                                            ui.push_id(text_input_id, |ui| {
                                                ui.add(egui::TextEdit::singleline(page_input)
                                                    .desired_width(50.0));
                                            });

                                            let go_button_id = ui.make_persistent_id(format!("go_page_{}", window_id));
                                            ui.push_id(go_button_id, |ui| {
                                                if ui.button("Go").clicked() {
                                                    if let Some(input) = text_cache.page_input.get(&window_id) {
                                                        if let Ok(page_num) = input.parse::<usize>() {
                                                            if page_num > 0 && page_num <= total_pages {
                                                                text_cache.current_page.insert(window_id, page_num - 1);
                                                                text_cache.page_input.insert(window_id, page_num.to_string());
                                                            }
                                                        }
                                                    }
                                                }
                                            });
                                        });

                                        let clear_text_id = ui.make_persistent_id(format!("clear_text_button_{}", window_id));
                                        ui.push_id(clear_text_id, |ui| {
                                            if ui.button("Clear Text").clicked() {
                                                if let None = web_state.actual_file_path.get(&window_id) {
                                                    println!("Error occured while clearing text file");
                                                }
                                                web_state.actual_file_path.remove(&window_id);
                                                web_state.current_display_type.insert(window_id, MediaDisplayType::None);
                                                text_cache.clear(window_id);
                                            }
                                        });
                                    }
                                } else {
                                    ui.label("Loading text file...");
                                }
                            },
                            MediaDisplayType::None => {
                                ui.label("No media to display");
                            }
                        }
                    });

                    ui.separator();
                    let close_button_id = ui.make_persistent_id(format!("close_button_{}", window_id));
                    ui.push_id(close_button_id, |ui| {
                        if ui.button("Close Window").clicked() {
                            should_close = true;
                        }
                    });
                });
            }
            if should_clear_image{
                if let Some(Some(texture)) = state.handles.get(&window_id) {
                    contexts.remove_image(texture);
                }
            }

            if should_close {
                windows_to_close.push(i);

                if let Some(Some(texture)) = state.handles.get(&window_id) {
                    contexts.remove_image(texture);
                }

                state.handles.insert(window_id, None);
                state.egui_textures.insert(window_id, None);
                web_state.loading_image.remove(&window_id);
                web_state.currently_selected_media.remove(&window_id);
                web_state.actual_media_path.remove(&window_id);
                web_state.media_paths.remove(&window_id);
                web_state.actual_file_path.remove(&window_id);
                web_state.selected_text_server.remove(&window_id);
                web_state.selected_media_server.remove(&window_id);
                web_state.received_medias.remove(&window_id);
                web_state.current_display_type.remove(&window_id);
                web_state.last_loaded_path.remove(&window_id);
                web_state.server_for_current_media.remove(&window_id);
                text_cache.clear(window_id);
            }
        }
    }
    for image_handle in images_to_remove {
        contexts.remove_image(&image_handle);
    }

    for i in windows_to_close.into_iter().rev() {
        open_windows.windows.remove(i);
    }
}

#[derive(Resource, Default, Debug)]
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
    last_loaded_path: HashMap<NodeId, String>,
    server_for_current_media: HashMap<NodeId, Option<NodeId>>,
    currently_selected_media: HashMap<NodeId, Option<String>>,
    loading_image: HashMap<NodeId, String>,
}
fn trim_into_file_name(actual_path: &String)->String{
    let retval=actual_path.split('/').last().unwrap_or("").to_string();
    retval
}