use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin},
    prelude::*
};
use bevy::winit::WinitSettings;
use crate::simulation_control::buttons::{button_system,button_setup, CrashEvent};
use crate::simulation_control::colorful_name::{title_setup, update_text_color};
use crate::simulation_control::drone_image::{image_setup,update_image_on_crash};
use crate::simulation_control::text_output::{setup_ui,update_text_on_crash};


pub fn simulate() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_systems(Startup, button_setup)
        .add_systems(Startup,title_setup)
        .add_systems(Startup,image_setup)
        .add_systems(Update,update_image_on_crash)
        .add_systems(Update, update_text_color)
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Update,button_system)
        .add_event::<CrashEvent>()
        .add_systems(Startup,setup_ui)
        .add_systems(Update,update_text_on_crash)
        .run();

}
