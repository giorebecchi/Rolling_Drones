use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

pub fn main() {
    App::new()
        .add_plugins((DefaultPlugins, EguiPlugin))
        .init_resource::<ImageState>()
        .add_systems(Startup, load_image)
        .add_systems(Update, display_image_ui)
        .run();
}

#[derive(Resource, Default)]
struct ImageState {
    handle: Option<Handle<Image>>,
    egui_texture: Option<(egui::TextureId, egui::Vec2)>,
}

fn load_image(
    mut state: ResMut<ImageState>,
    asset_server: Res<AssetServer>,
) {
    let handle: Handle<Image> = asset_server.load("icon.png");
    state.handle = Some(handle);
}

fn display_image_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<ImageState>,
    images: Res<Assets<Image>>,
) {
    // Try to get the image and register it with egui if needed
    if state.egui_texture.is_none() {
        if let Some(handle) = &state.handle {
            if let Some(image) = images.get(handle) {
                let size = egui::Vec2::new(image.width() as f32, image.height() as f32);
                let texture_id = contexts.add_image(handle.clone());
                state.egui_texture = Some((texture_id, size));
            }
        }
    }

    egui::Window::new("Image Window").show(contexts.ctx_mut(), |ui| {
        if let Some((texture_id, size)) = state.egui_texture {
            // Create an ImageSource from the TextureId with size
            let sized_texture = egui::load::SizedTexture::new(texture_id, size);
            let image_source = egui::ImageSource::Texture(sized_texture);

            // Create the Image widget with the ImageSource
            let image_widget = egui::Image::new(image_source);
            if let Some(_)=state.handle {

                // Add the widget to the UI
                ui.add(image_widget);
            }

            // Add a button to remove the image
            if ui.button("Remove Image").clicked() {
                state.handle=None; // Reset the texture, removing the image
            }
        } else {
            ui.label("Loading image...");
        }
    });
}
