use bevy::prelude::*;
use crate::GUI::login_window::{NodeConfig, NodeType};
use crate::network_initializer::network_initializer::parse_config;

/// Generates a tree layout of nodes based on configuration from a TOML file.
///
/// This function creates a balanced tree structure where:
/// - Nodes are arranged in levels with the root at the top
/// - Complete levels are filled first
/// - Any remaining nodes are placed at the next level from left to right
/// - The tree accommodates Drone, Client, and Server node types
pub fn spawn_tree() -> Vec<NodeConfig> {

    let config = parse_config("assets/configurations/tree.toml");
    let base_horizontal_spacing = 100.0;
    let vertical_spacing = 80.0;


    let mut all_nodes = Vec::new();
    for drone in config.drone {
        all_nodes.push((NodeType::Drone, drone.id, drone.connected_node_ids));
    }
    for client in config.client {
        all_nodes.push((NodeType::Client, client.id, client.connected_drone_ids));
    }
    for server in config.server {
        all_nodes.push((NodeType::Server, server.id, server.connected_drone_ids));
    }

    let node_count = all_nodes.len();
    let mut nodes = Vec::with_capacity(node_count);


    let complete_levels = calculate_complete_levels(node_count);

    position_nodes(
        &all_nodes,
        &mut nodes,
        complete_levels,
        base_horizontal_spacing,
        vertical_spacing
    );

    nodes
}

fn calculate_complete_levels(node_count: usize) -> usize {
    let mut complete_levels = 0;
    let mut total_nodes = 0;

    while total_nodes < node_count {
        complete_levels += 1;
        let nodes_at_level = 2_usize.pow((complete_levels - 1) as u32);
        total_nodes += nodes_at_level;
    }

    complete_levels
}

fn position_nodes(
    all_nodes: &[(NodeType, u8, Vec<u8>)],
    nodes: &mut Vec<NodeConfig>,
    complete_levels: usize,
    base_horizontal_spacing: f32,
    vertical_spacing: f32
) {
    let node_count = all_nodes.len();
    let mut node_index = 0;

    for level in 0..complete_levels {
        if node_index >= node_count {
            break;
        }

        let nodes_at_level = 2_usize.pow(level as u32);
        let level_width = base_horizontal_spacing * 2_f32.powf(complete_levels as f32 - 1.0);

        node_index = place_nodes_at_level(
            all_nodes,
            nodes,
            level,
            nodes_at_level,
            level_width,
            vertical_spacing,
            node_index,
            node_count
        );
    }

    if node_index < node_count {
        let level = complete_levels;
        let remaining_nodes = node_count - node_index;
        let level_width = base_horizontal_spacing * 2_f32.powf(complete_levels as f32 - 1.0);

        place_nodes_at_level(
            all_nodes,
            nodes,
            level,
            remaining_nodes,
            level_width,
            vertical_spacing,
            node_index,
            node_count
        );
    }
}

fn place_nodes_at_level(
    all_nodes: &[(NodeType, u8, Vec<u8>)],
    nodes: &mut Vec<NodeConfig>,
    level: usize,
    nodes_to_place: usize,
    level_width: f32,
    vertical_spacing: f32,
    start_index: usize,
    node_count: usize
) -> usize {
    let mut node_index = start_index;
    let segment_width = level_width / 2_usize.pow(level as u32) as f32;
    let y = -(level as f32) * vertical_spacing;

    for position in 0..nodes_to_place {
        if node_index >= node_count {
            break;
        }

        let x = (position as f32 * segment_width) + (segment_width / 2.0) - (level_width / 2.0);

        let (node_type, id, connected_ids) = &all_nodes[node_index];

        nodes.push(NodeConfig::new(
            node_type.clone(),
            *id,
            Vec2::new(x, y),
            connected_ids.clone()
        ));

        node_index += 1;
    }

    node_index
}