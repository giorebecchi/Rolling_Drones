use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use crossbeam_channel;

// ----------------------------------------------------------------------------
// Resources
// ----------------------------------------------------------------------------

#[derive(Default, Resource)]
struct OccupiedScreenSpace {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

/// Holds the crossbeam channel endpoints
#[derive(Resource)]
struct NodeChannels {
    sender: crossbeam_channel::Sender<String>,
    receiver: crossbeam_channel::Receiver<String>,
}

/// Holds the string we want to display in the right Egui panel
#[derive(Resource, Default)]
struct Input {
    pub string: String,
}

// Implement a manual Default so we can do .init_resource::<NodeChannels>()
impl Default for NodeChannels {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        NodeChannels { sender, receiver }
    }
}

// ----------------------------------------------------------------------------
// Main
// ----------------------------------------------------------------------------

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        // Initialize our resources:
        .init_resource::<NodeChannels>()
        .init_resource::<OccupiedScreenSpace>()
        .init_resource::<Input>()
        // IMPORTANT: run poll_node_messages FIRST, then ui_system
        .add_systems(Update, (poll_node_messages, ui_system))
        .run();
}

// ----------------------------------------------------------------------------
// Systems
// ----------------------------------------------------------------------------

/// This system runs *first* each frame, reading (non-blocking) any messages from the channel
/// and storing them in the `input.string` resource.
/// You will see it in the right Egui panel as soon as `ui_system` runs afterward.
fn poll_node_messages(channels: Res<NodeChannels>, mut input: ResMut<Input>) {
    // Drain all available messages this frame
    while let Ok(msg) = channels.receiver.try_recv() {
        println!("Main thread polled: {}", msg);
        // Append each message on a new line (optional)
        if !input.string.is_empty() {
            input.string.push('\n');
        }
        input.string.push_str(&msg);
    }
}

/// This system runs *after* poll_node_messages and creates the Egui panels.
/// When you press the "Crash drone" button, it sends "hello" via the channel.
fn ui_system(
    mut is_last_selected: Local<bool>,
    mut contexts: EguiContexts,
    mut occupied_screen_space: ResMut<OccupiedScreenSpace>,
    channels: Res<NodeChannels>,
    input: Res<Input>,
) {
    if let Some(ctx) = contexts.try_ctx_mut() {
        // Left panel
        occupied_screen_space.left = egui::SidePanel::left("left_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.label("Simulation Commands");

                if ui.button("Crash drone").clicked() {
                    // Send a message to the channel
                    let _ = channels.sender.send("hello".to_owned());
                    *is_last_selected = false;
                }

                if ui.button("Another button").clicked() {
                    *is_last_selected = true;
                }

                // Fill remaining space (optional)
                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            })
            .response
            .rect
            .width();

        // Right panel: shows the latest value of `input.string`
        occupied_screen_space.right = egui::SidePanel::right("right_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.label("Simulation events");
                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                ui.label(&input.string);
            })
            .response
            .rect
            .width();
    }
}
