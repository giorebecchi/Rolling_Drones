
use bevy::prelude::*;
use crate::network_initializer::network_initializer::*;
use crate::GUI::login_window::{NodeConfig,NodeType};


pub fn spawn_star_decagram(mut commands: &mut Commands)->Vec<NodeConfig> {
    //let node_count = 10;
    let config=parse_config("assets/configurations/star.toml");
    let node_count = config.drone.len();
    let mut drones=Vec::new();
    //let mut clients =Vec::new();
    //let mut servers = Vec::new();
    let radius = 200.0;

    let mut positions = Vec::with_capacity(node_count);


    for (i, drone) in config.drone.into_iter().enumerate() {
        let angle = i as f32 * std::f32::consts::TAU / node_count as f32;
        let x = radius * angle.cos();
        let y = radius * angle.sin();

        let sprite_color = Color::srgb(0.9,0.1,0.);
        let sprite_size = Vec2::splat(15.0);


        commands.spawn(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(x, y, 0.0)),
            sprite: Sprite {
                color: sprite_color,
                custom_size: Some(sprite_size),
                ..default()
            },
            ..default()
        });

        positions.push(Vec2::new(x, y));
        let node=NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids);
        drones.push(node);
    }
    drones

}


