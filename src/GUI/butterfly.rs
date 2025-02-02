use bevy::prelude::*;
use wg_2024::network::NodeId;
use crate::GUI::login_window::{NodeConfig, NodeType};
use crate::network_initializer::network_initializer::parse_config;

pub fn spawn_butterfly(mut commands: &mut Commands)->Vec<NodeConfig> {
    let config= parse_config("assets/configurations/butterfly.toml");
    let horizontal_spacing=100.;
    let vertical_offset=50.;

    let mut top_position= Vec::with_capacity(4);
    let mut middle_position = Vec::with_capacity(4);
    let mut bottom_position= Vec::with_capacity(2);

    let mut drones= Vec::new();

    for (i,drone) in config.drone.into_iter().enumerate(){
        if i<4 {
            let x = (i as f32 - (4 - 1) as f32 / 2.0) * horizontal_spacing;
            let y = vertical_offset;

            set_up_bundle(x, y, &mut commands, drone.id);

            top_position.push(Vec2::new(x, y));
            let node = NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids.clone());
            drones.push(node);
        }
        else if i>=4 && i<8{
            let x = ((i-4) as f32 - (4 - 1) as f32 / 2.0) * horizontal_spacing;
            let y = -vertical_offset;

            set_up_bundle(x,y,&mut commands,drone.id);

            middle_position.push(Vec2::new(x, y));
            let node = NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids.clone());
            drones.push(node);
        }else if i>=8{
            let x = ((i-8) as f32 - (2 - 1) as f32 / 2.0) * horizontal_spacing;
            let y = -(vertical_offset*3.0);

            set_up_bundle(x,y,&mut commands,drone.id);

            bottom_position.push(Vec2::new(x, y));
            let node = NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids.clone());
            drones.push(node);
        }
    }
    drones

}
pub fn set_up_bundle(
    x: f32,
    y: f32,
    commands: &mut Commands,
    id: NodeId,
) {

    commands.spawn((
        SpriteBundle {
            transform: Transform::from_translation(Vec3::new(x, y, 0.0)),
            sprite: Sprite {
                color: Color::srgb(0., 0., 1.),
                custom_size: Some(Vec2::splat(30.0)),
                ..Default::default()
            },
            ..Default::default()
        },
        Name::new(format!("Point_{id}")),
    ));

    commands.spawn((
        Text2d::new(format!("{id}")),
        TextFont {
            font_size: 12.0,
            ..Default::default()
        },
        TextColor(Color::srgb(1.,0.,0.)),
        TextLayout::new_with_justify(JustifyText::Center),
        TransformBundle::from_transform(
            Transform::from_translation(Vec3::new(x+20., y+10., 0.0)),
        ),
        Name::new(format!("Text_{id}")),
    ));



}