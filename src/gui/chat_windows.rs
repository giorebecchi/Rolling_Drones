use std::collections::HashMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use crate::gui::login_window::SimulationController;
use wg_2024::network::NodeId;
use crate::common_things::common::ClientType;
use crate::gui::login_window::Clickable;
use crate::gui::login_window::AppState;


pub struct ChatSystemPlugin;

impl Plugin for ChatSystemPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<OpenWindows>()
            .init_resource::<ChatState>()
            .add_systems(Update,(handle_clicks, display_windows).run_if(in_state(AppState::InGame)));
    }
}

#[derive(Resource, Default)]
pub struct ChatState {
    message_input: HashMap<NodeId, String>,
    active_chat_node: HashMap<NodeId, Option<NodeId>>,
    active_chat_server: HashMap<NodeId, Option<NodeId>>,
    pub registered_clients: HashMap<(NodeId, NodeId), bool>,
    pub chat_messages: HashMap<(NodeId, (NodeId, NodeId)), Vec<String>>,
    pub chat_responses: HashMap<(NodeId, (NodeId, NodeId)), Vec<String>>,
    pub chat_clients: Vec<NodeId>,
    pub chat_servers: HashMap<NodeId, Vec<NodeId>>
}
#[derive(Resource, Default)]
pub struct OpenWindows {
    pub windows: Vec<(NodeId,ClientType)>,
    click_count: usize
}
pub fn handle_clicks(
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    clickable_entities: Query<(Entity, &Transform, &Sprite, &Clickable)>,
    sim: Res<SimulationController>,
    mut open_windows: ResMut<OpenWindows>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let window = windows.single();
    let (camera, camera_transform) = camera_q.single();

    if let Some(cursor_position) = window.cursor_position() {
        if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) {
            let cursor_world_position = ray.origin.truncate();

            for (_entity, transform, sprite, clickable) in clickable_entities.iter() {
                let sprite_size = sprite.custom_size.unwrap_or(Vec2::ONE);
                let sprite_pos = transform.translation.truncate();

                let half_width = sprite_size.x / 2.0;
                let half_height = sprite_size.y / 2.0;

                if cursor_world_position.x >= sprite_pos.x - half_width
                    && cursor_world_position.x <= sprite_pos.x + half_width
                    && cursor_world_position.y >= sprite_pos.y - half_height
                    && cursor_world_position.y <= sprite_pos.y + half_height
                {
                    if !open_windows.windows.contains(&(clickable.name,clickable.window_type.clone())) {
                        open_windows.windows.push((clickable.name.clone(),clickable.window_type.clone()));
                        open_windows.click_count+=1;

                        match clickable.window_type{
                            ClientType::ChatClient=>sim.get_chat_servers(),
                            ClientType::WebBrowser=> sim.get_web_servers()

                        }

                    }



                }
            }
        }
    }
}


fn display_windows(
    mut contexts: EguiContexts,
    mut open_windows: ResMut<OpenWindows>,
    mut sim: ResMut<SimulationController>,
    mut chat_state: ResMut<ChatState>
) {
    let mut windows_to_close = Vec::new();

    for (i, &(window_id,ref client_type)) in open_windows.windows.iter().enumerate() {
        if client_type.clone() == ClientType::ChatClient {
            if !chat_state.message_input.contains_key(&window_id) {
                chat_state.message_input.insert(window_id, String::new());
            }
            if !chat_state.active_chat_node.contains_key(&window_id) {
                chat_state.active_chat_node.insert(window_id, None);
            }
            if !chat_state.active_chat_server.contains_key(&window_id) {
                chat_state.active_chat_server.insert(window_id, None);
            }

            let window = egui::Window::new(format!("Client: {}", window_id))
                .id(egui::Id::new(format!("window_{}", i)))
                .resizable(true)
                .collapsible(true)
                .default_size([400.0, 500.0]);

            let mut should_close = false;

            window.show(contexts.ctx_mut(), |ui| {
                ui.label(format!("This is a window for Client {}", window_id));
                ui.separator();
                ui.heading("Available Clients");
                let available_clients = chat_state.chat_clients.iter().filter(|id| **id != window_id).cloned().collect::<Vec<u8>>();

                let active_server = chat_state.active_chat_server.get(&window_id).cloned().flatten();

                for client in available_clients {
                    let is_registered = if let Some(server_id) = active_server {
                        chat_state.registered_clients.get(&(client, server_id))
                            .copied()
                            .unwrap_or(false)
                    } else {
                        false
                    };

                    let button_text = format!("Chat with Client {} {}",
                                              client,
                                              if is_registered { "✓" } else { "" }
                    );


                    let button = ui.button(button_text);

                    if button.clicked() {
                        if chat_state.active_chat_node.get(&window_id) == Some(&Some(client)) {
                            chat_state.active_chat_node.insert(window_id, None);
                        } else if is_registered {
                            chat_state.active_chat_node.insert(window_id, Some(client));
                        }
                    }
                }

                ui.group(|ui| {
                    let available_width = ui.available_width().min(370.0);
                    ui.set_max_width(available_width);

                    ui.vertical(|ui| {
                        let chat_partner = chat_state.active_chat_node.get(&window_id).cloned().flatten();

                        ui.heading(
                            if let Some(partner_id) = chat_partner {
                                format!("Chat with Client {}", partner_id)
                            } else {
                                "Chat with None".to_string()
                            }
                        );

                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                if let (Some(partner_id), Some(server_id)) = (chat_partner, active_server) {
                                    let messages = chat_state.chat_messages.get_mut(&(server_id, (window_id, partner_id)));
                                    let messages = match messages {
                                        Some(m) => {
                                            m.clone()
                                        },
                                        None => Vec::new(),
                                    };
                                    let replies = chat_state.chat_responses.get_mut(&(server_id, (partner_id, window_id)));
                                    let replies = match replies {
                                        Some(r) => {
                                            r.clone()
                                        },
                                        None => Vec::new(),
                                    };


                                    if !messages.is_empty() {
                                        for msg in messages {
                                            ui.horizontal(|ui| {
                                                let text_width = available_width - 10.0;
                                                ui.set_max_width(text_width);
                                                ui.label(format!("You: {}", msg));
                                            });
                                        }
                                    } else {
                                        ui.label("No messages yet. Start the conversation!");
                                    }
                                    if !replies.is_empty() {
                                        for reply in replies {
                                            ui.horizontal(|ui| {
                                                let text_width = available_width - 10.0;
                                                ui.set_max_width(text_width);
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                                    ui.label(format!("Client {} : {}", partner_id, reply));
                                                });
                                            });
                                        }
                                    }
                                }
                            });
                    });

                    ui.separator();


                    let chat_partner = chat_state.active_chat_node.get(&window_id).cloned().flatten();
                    let current_server = chat_state.active_chat_server.get(&window_id).cloned().flatten();

                    let can_chat = if let (Some(partner_id), Some(server_id)) = (chat_partner, current_server) {
                        chat_state.registered_clients.get(&(window_id, server_id)).copied().unwrap_or(false) &&
                            chat_state.registered_clients.get(&(partner_id, server_id)).copied().unwrap_or(false)
                    } else {
                        false
                    };


                    if can_chat {
                        let partner_id = chat_partner.unwrap();
                        let server_id = current_server.unwrap();


                        let current_input = chat_state.message_input.get(&window_id).cloned().unwrap_or_default();


                        let mut input_text = current_input;

                        let input_response = ui.add(
                            egui::TextEdit::singleline(&mut input_text)
                                .frame(true)
                                .hint_text("Type your message here...")
                                .desired_width(ui.available_width() - 80.0)
                        );

                        chat_state.message_input.insert(window_id, input_text.clone());

                        let send_button = ui.button("📨 Send");


                        if (send_button.clicked() ||
                            (input_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                            && !input_text.is_empty()
                        {
                            sim.send_message(
                                input_text.clone(),
                                window_id,
                                partner_id,
                                server_id
                            );
                            if let Some(messages) = chat_state.chat_messages.get_mut(&(server_id, (window_id, partner_id))) {
                                messages.push(input_text.clone());
                            } else {
                                let mut messages = Vec::new();
                                messages.push(input_text.clone());
                                chat_state.chat_messages.insert((server_id, (window_id, partner_id)), messages);
                            }
                            chat_state.message_input.insert(window_id, String::new());
                        }
                    } else {
                        ui.add_enabled(false, egui::TextEdit::singleline(&mut String::new())
                            .hint_text("Select a registered client to chat")
                            .desired_width(ui.available_width() - 80.0));

                        ui.add_enabled(false, egui::Button::new("📨 Send"));
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Server: ");

                    let current_server_text = match chat_state.active_chat_server.get(&window_id).cloned().flatten() {
                        Some(server_id) => format!("Server {}", server_id),
                        None => "Select a server".to_string()
                    };

                    egui::ComboBox::from_id_salt(format!("server_selector_{}", window_id))
                        .selected_text(current_server_text)
                        .show_ui(ui, |ui| {
                            let servers = chat_state.chat_servers.get(&window_id).cloned();
                            if let Some(servers) = servers {
                                for server in servers {
                                    let selected = chat_state.active_chat_server.get(&window_id) == Some(&Some(server));
                                    if ui.selectable_label(selected, format!("Server {}", server)).clicked() {
                                        if chat_state.active_chat_server.get(&window_id) == Some(&Some(server)) {
                                            chat_state.active_chat_server.insert(window_id, None);
                                        } else {
                                            chat_state.active_chat_server.insert(window_id, Some(server));
                                        }
                                    }
                                }
                            }
                        });

                    if ui.button("Register").clicked() {
                        if let Some(server_id) = chat_state.active_chat_server.get(&window_id).cloned().flatten() {
                            sim.register_client(window_id.clone(), server_id.clone());
                        }
                    }
                });


                if let Some(server_id) = chat_state.active_chat_server.get(&window_id).cloned().flatten() {
                    let is_registered = chat_state.registered_clients.get(&(window_id, server_id)).copied().unwrap_or(false);
                    ui.label(format!(
                        "Status: {} to Server {}",
                        if is_registered { "Registered" } else { "Not Registered" },
                        server_id
                    ));
                } else {
                    ui.label("Status: No server selected");
                }

                ui.separator();
                if ui.button("Close Window").clicked() {
                    should_close = true;
                }
            });

            if should_close {
                windows_to_close.push(i);
            }
        }
    }

    for i in windows_to_close.into_iter().rev() {
        open_windows.windows.remove(i);
    }

}