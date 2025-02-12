use bevy::prelude::*;
use crate::GUI::login_window::{NodeConfig, NodeType};
use crate::network_initializer::network_initializer::parse_config;

pub fn spawn_tree() -> Vec<NodeConfig> {
    let config = parse_config("assets/configurations/tree.toml");
    let base_horizontal_spacing = 100.0;
    let vertical_offset = 50.0;

    let mut root_position = Vec::with_capacity(1);
    let mut top_position = Vec::with_capacity(2);
    let mut middle_position = Vec::with_capacity(3);
    let mut bottom_position = Vec::with_capacity(4);

    let mut drones = Vec::new();



    for (i, drone) in config.drone.into_iter().enumerate() {
        if i == 0 {
            // Root node
            let x = 0.0;
            let y = vertical_offset;

            let node = NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids);
            drones.push(node);
            root_position.push(Vec2::new(x, y));
        } else if i > 0 && i <= 2 {
            // Top layer (2 nodes)
            let horizontal_spacing = base_horizontal_spacing * 1.5; // Wider for higher layers
            let x = (i as f32 - 1.5) * horizontal_spacing; // Centered
            let y = -vertical_offset;


            let node = NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids);
            drones.push(node);
            top_position.push(Vec2::new(x, y));
        } else if i > 2 && i <= 5 {
            // Middle layer (3 nodes)
            let horizontal_spacing = base_horizontal_spacing * 1.2; // Slightly narrower
            let x = (i as f32 - 4.0) * horizontal_spacing; // Centered
            let y = -vertical_offset * 3.0;



            let node = NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids);
            drones.push(node);
            middle_position.push(Vec2::new(x, y));
        } else if i > 5 && i <= 9 {
            // Bottom layer (4 nodes)
            let horizontal_spacing = base_horizontal_spacing * 1.5; // Wider spacing for side nodes
            let x = match i {
                6 => -1.5 * horizontal_spacing, // Far left node
                7 => -0.5 * horizontal_spacing, // Left-center node
                8 => 0.5 * horizontal_spacing,  // Right-center node
                9 => 1.5 * horizontal_spacing,  // Far right node
                _ => 0.0, // Fallback (should not occur)
            };
            let y = -vertical_offset * 5.0;


            let node = NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids);
            drones.push(node);
            bottom_position.push(Vec2::new(x, y));
        }
    }

    drones
}
