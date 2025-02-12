use bevy::prelude::*;
use crate::GUI::login_window::{NodeConfig, NodeType};
use crate::network_initializer::network_initializer::parse_config;

pub fn spawn_double_chain()->Vec<NodeConfig> {
    let config=parse_config("assets/configurations/double_chain.toml");
    let node_count_per_line = 5;
    let horizontal_spacing = 100.0;
    let vertical_offset = 50.0; // top line at +50, bottom line at -50

    let mut top_positions = Vec::with_capacity(node_count_per_line);
    let mut bottom_positions = Vec::with_capacity(node_count_per_line);

    let mut drones= Vec::new();
    for (i,drone) in config.drone.into_iter().enumerate() {
        if i<5{
            let x = (i as f32 - (node_count_per_line - 1) as f32 / 2.0) * horizontal_spacing;
            let y = vertical_offset;

            top_positions.push(Vec2::new(x, y));
            let node = NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids);
            drones.push(node);
        }else{
            let x = ((i-5) as f32 - (node_count_per_line - 1) as f32 / 2.0) * horizontal_spacing;
            let y = -vertical_offset;

            bottom_positions.push(Vec2::new(x, y));
            let node = NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids);
            drones.push(node);
        }


    }
    drones

}