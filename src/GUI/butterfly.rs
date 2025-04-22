use bevy::prelude::*;
use crate::GUI::login_window::{NodeConfig, NodeType,AddedDrone};
use crate::network_initializer::network_initializer::parse_config;

pub fn spawn_butterfly(added_drone: Option<AddedDrone>) -> Vec<NodeConfig> {
    let config = parse_config("assets/configurations/double_chain.toml");
    let horizontal_spacing = 100.0;
    let vertical_spacing = 60.0;

    let mut all_nodes = Vec::new();
    for drone in config.drone {
        all_nodes.push((NodeType::Drone, drone.id, drone.connected_node_ids));
    }
    if let Some(added_drone)=added_drone{
        all_nodes.push((NodeType::Drone, added_drone.drone.1, added_drone.drone.0.clone()));
    }
    for client in config.client {
        all_nodes.push((NodeType::Client, client.id, client.connected_drone_ids));
    }
    for server in config.server {
        all_nodes.push((NodeType::Server, server.id, server.connected_drone_ids));
    }

    let node_count = all_nodes.len();
    let mut nodes = Vec::with_capacity(node_count);
    let mut node_index = 0;

    // Always fixed structure for the first three layers
    let base_structure = [2, 4, 4];
    let mut current_y = -vertical_spacing * 3.0;

    for &count in &base_structure {
        let x_offset = (count as f32 - 1.0) * horizontal_spacing / 2.0;
        for i in 0..count {
            if node_index >= node_count { break; }
            let x = (i as f32 * horizontal_spacing) - x_offset;
            let (node_type, id, connected_ids) = &all_nodes[node_index];
            nodes.push(NodeConfig::new(
                node_type.clone(),
                *id,
                Vec2::new(x, current_y),
                connected_ids.clone()
            ));
            node_index += 1;
        }
        current_y += vertical_spacing;
    }

    // If more than 10 nodes, add additional layers of 4 nodes each
    while node_index < node_count {
        let nodes_in_row = (node_count - node_index).min(4);
        let x_offset = (nodes_in_row as f32 - 1.0) * horizontal_spacing / 2.0;
        for i in 0..nodes_in_row {
            let x = (i as f32 * horizontal_spacing) - x_offset;
            let (node_type, id, connected_ids) = &all_nodes[node_index];
            nodes.push(NodeConfig::new(
                node_type.clone(),
                *id,
                Vec2::new(x, current_y),
                connected_ids.clone()
            ));
            node_index += 1;
        }
        current_y += vertical_spacing;
    }

    nodes
}
