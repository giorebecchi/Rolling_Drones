use bevy::prelude::*;
use crate::gui::login_window::{NodeConfig, NodeType};
use crate::gui::shared_info_plugin::SeenClients;
use crate::network_initializer::network_initializer::parse_config;

pub fn spawn_butterfly(
    clients: &mut SeenClients,
) -> Vec<NodeConfig>
{
    let config = parse_config();
    let horizontal_spacing = 100.0;
    let vertical_spacing = 60.0;

    let mut all_nodes = Vec::new();

    // Add all drones from config
    for drone in config.drone {
        all_nodes.push((NodeType::Drone, drone.id, drone.connected_node_ids, drone.pdr));
    }

    // Add all clients from SeenClients with their correct types
    for (client_type, client_id) in &clients.clients {
        // Find the matching client in config to get its connected_drone_ids
        for client in &config.client {
            if client.id == *client_id {
                all_nodes.push((client_type.clone(), client.id, client.connected_drone_ids.clone(), -1.00));
                break;
            }
        }
    }

    // Add all servers from SeenClients with their correct types
    for (server_type, server_id) in &clients.servers {
        // Find the matching server in config to get its connected_drone_ids
        for server in &config.server {
            if server.id == *server_id {
                all_nodes.push((server_type.clone(), server.id, server.connected_drone_ids.clone(), -1.00));
                break;
            }
        }
    }

    let node_count = all_nodes.len();
    let mut nodes = Vec::with_capacity(node_count);

    if node_count == 0 {
        return nodes;
    }

    let nodes_for_upper_layers = node_count.saturating_sub(2);
    let mut layer_distribution = Vec::new();

    let mut remaining = nodes_for_upper_layers;
    while remaining > 0 {
        let layer_size = remaining.min(4);
        layer_distribution.push(layer_size);
        remaining -= layer_size;
    }

    layer_distribution.push(2.min(node_count));

    layer_distribution.reverse();

    let total_layers = layer_distribution.len();
    let mut current_y = -((total_layers - 1) as f32 * vertical_spacing) / 2.0;

    let mut node_index = 0;

    let bottom_layer_right_x = if layer_distribution[0] == 2 {
        horizontal_spacing / 2.0
    } else {
        0.0
    };

    for (layer_idx, &nodes_in_layer) in layer_distribution.iter().enumerate() {
        if nodes_in_layer == 0 { continue; }

        if layer_idx == 0 {
            let x_positions = match nodes_in_layer {
                1 => vec![0.0],
                2 => vec![-horizontal_spacing / 2.0, horizontal_spacing / 2.0],
                _ => unreachable!("Bottom layer should have at most 2 nodes")
            };

            for x in x_positions {
                if node_index >= node_count { break; }
                let (node_type, id, connected_ids, pdr) = &all_nodes[node_index];
                nodes.push(NodeConfig::new(
                    node_type.clone(),
                    *id,
                    Vec2::new(x, current_y),
                    connected_ids.clone(),
                    *pdr
                ));
                node_index += 1;
            }
        } else {
            if nodes_in_layer == 4 {
                let x_offset = 1.5 * horizontal_spacing;
                for i in 0..4 {
                    if node_index >= node_count { break; }
                    let x = (i as f32 * horizontal_spacing) - x_offset;
                    let (node_type, id, connected_ids, pdr) = &all_nodes[node_index];
                    nodes.push(NodeConfig::new(
                        node_type.clone(),
                        *id,
                        Vec2::new(x, current_y),
                        connected_ids.clone(),
                        *pdr
                    ));
                    node_index += 1;
                }
            } else {
                for i in 0..nodes_in_layer {
                    if node_index >= node_count { break; }
                    let x = bottom_layer_right_x - ((nodes_in_layer - 1 - i) as f32 * horizontal_spacing);
                    let (node_type, id, connected_ids, pdr) = &all_nodes[node_index];
                    nodes.push(NodeConfig::new(
                        node_type.clone(),
                        *id,
                        Vec2::new(x, current_y),
                        connected_ids.clone(),
                        *pdr
                    ));
                    node_index += 1;
                }
            }
        }

        current_y += vertical_spacing;
    }

    nodes
}