use bevy::prelude::*;
use bevy_egui::{egui,EguiContexts, EguiPlugin};
use bevy_framepace::{FramepacePlugin, FramepaceSettings, Limiter};


// Resource to track UI state
#[derive(Resource)]
struct UiState {
    is_open: bool,
}
impl Default for UiState {
    fn default() -> Self {
        Self { is_open: true }
    }
}

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .init_resource::<UiState>()
        .add_plugins(FramepacePlugin)
        // Configure frame rate limiter
        .insert_resource(FramepaceSettings {
            // Limit to 144 FPS for smooth performance
            limiter: Limiter::from_framerate(144.0),

            // Optional: Use automatic framerate adjustment
            // limiter: Limiter::Auto,
        })
        .add_systems(Update, ui_system)
        .run();
}

fn ui_system(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<UiState>,
) {
    // Create a persistent window that stays open
    egui::Window::new("Stable UI Window")
        .open(&mut ui_state.is_open)
        .show(contexts.ctx_mut(), |ui| {
            // Basic content that always renders
            ui.label("This is a stable UI window");

            // Example of adding interactive elements
            if ui.button("Toggle Something").clicked() {
                // Example of state interaction
                println!("Button clicked!");
            }
        });
}