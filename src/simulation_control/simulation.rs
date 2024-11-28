use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin},
    prelude::*
};
use bevy::winit::WinitSettings;
use crate::simulation_control::buttons::{button_system,button_setup};
use crate::simulation_control::colorful_name::{title_setup, update_text_color};


pub fn simulate() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_systems(Startup, button_setup)
        .add_systems(Startup,title_setup)
        .add_systems(Update, update_text_color)
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Update, button_system)
        .run();

}
