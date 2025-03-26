/*use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

// Define our app state to manage chat input
#[derive(Resource, Default)]
struct ChatState {
    message_input: String,
    messages: Vec<String>,
}

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .init_resource::<ChatState>()
        .add_systems(Update, chat_ui)
        .run();
}

fn chat_ui(mut contexts: EguiContexts, mut chat_state: ResMut<ChatState>) {
    egui::CentralPanel::default()
        .frame(egui::Frame::default().fill(egui::Color32::WHITE))
        .show(contexts.ctx_mut(), |ui| {
            // Display existing messages
            ui.vertical(|ui| {
                ui.heading("Chat Messages");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for message in &chat_state.messages {
                        ui.label(message);
                    }
                });
            });

            // Message input area
            ui.separator();

            // Input field with grey background
            let input_response = ui.add(
                egui::TextEdit::singleline(&mut chat_state.message_input)
                    .desired_width(f32::INFINITY)
                    .frame(true)
                    .background_color(egui::Color32::from_rgb(200, 200, 200))
            );

            // Send button with paper airplane icon
            ui.horizontal(|ui| {
                let send_button = ui.button("📨 Send");

                // Send message logic
                if send_button.clicked() && !chat_state.message_input.is_empty() {
                    let message_input=chat_state.message_input.clone();
                    chat_state.messages.push(message_input);
                    chat_state.message_input.clear();
                }

                // Allow sending with Enter key
                if input_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if !chat_state.message_input.is_empty() {
                        let message_input=chat_state.message_input.clone();
                        chat_state.messages.push(message_input);
                        chat_state.message_input.clear();
                    }
                }
            });
        });
}*/