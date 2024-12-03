use bevy::prelude::*;
use crate::simulation_control::buttons::CrashEvent;

#[derive(Component)]
pub struct DroneImage;

pub fn image_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera
    commands.spawn(Camera2dBundle {
        camera: Camera {
            order: 0, // Set the priority explicitly to avoid ambiguity
            ..Default::default()
        },
        ..Default::default()
    });

    // Load the image from the assets folder
    let texture_handle = asset_server.load("images/Rolling_Drone.png");

    // Spawn the sprite with the image and scale it down
    commands.spawn((
        SpriteBundle {
            texture: texture_handle,
            transform: Transform {
                scale: Vec3::new(0.25, 0.25, 0.5), // Adjust the scale (0.5 = 50% size)
                ..Default::default()
            },
            ..Default::default()
        },
        DroneImage, // Marker to identify the drone image
    ));
}

pub fn update_image_on_crash(
    mut crash_events: EventReader<CrashEvent>,
    mut commands: Commands,
    query: Query<Entity, With<DroneImage>>, // Query the entity with the DroneImage marker
    asset_server: Res<AssetServer>,
) {
    for _ in crash_events.read().by_ref() {
        // Despawn the old image
        for entity in query.iter() {
            commands.entity(entity).despawn();
        }

        // Load the new image
        let new_texture_handle = asset_server.load("images/Rolling_Drone_with_X.png");

        // Spawn a new sprite with the new image
        commands.spawn((
            SpriteBundle {
                texture: new_texture_handle.clone(),
                transform: Transform {
                    scale: Vec3::new(0.25, 0.25, 0.5), // Adjust scale if needed
                    ..Default::default()
                },
                ..Default::default()
            },
            DroneImage, // Marker for the new drone image
        ));
    }
}
