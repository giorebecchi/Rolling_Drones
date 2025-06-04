use bevy::prelude::*;
use crate::gui::login_window::{NodeConfig, NodeType};
use crate::gui::shared_info_plugin::SeenClients;
use crate::network_initializer::network_initializer::parse_config;
use crate::simulation_control::simulation_control::MyNodeType;

pub fn spawn_butterfly(
    clients: &mut SeenClients,
) -> Vec<NodeConfig>
{
    let config = parse_config();
    let horizontal_spacing = 100.0;
    let vertical_spacing = 60.0;

    let mut all_nodes = Vec::new();
    for drone in config.drone {
        all_nodes.push((NodeType::Drone, drone.id, drone.connected_node_ids, drone.pdr));
    }
    for client in &config.client {
        for (client_type, id) in &clients.clients{
            if id.clone() == client.id{
                match client_type{
                    MyNodeType::WebBrowser=>all_nodes.push((NodeType::WebBrowser, client.id, client.connected_drone_ids.clone(), -1.00)),
                    MyNodeType::ChatClient=>all_nodes.push((NodeType::ChatClient, client.id, client.connected_drone_ids.clone(), -1.00)),
                    _=>unreachable!()
                }
            }
        }
    }
    for server in config.server {
        for (server_type, id) in &clients.servers {
            if id.clone()==server.id {
                match server_type {
                    MyNodeType::TextServer => all_nodes.push((NodeType::TextServer, server.id, server.connected_drone_ids.clone(), -1.00)),
                    MyNodeType::MediaServer => all_nodes.push((NodeType::MediaServer, server.id, server.connected_drone_ids.clone(), -1.00)),
                    MyNodeType::ChatServer=>all_nodes.push((NodeType::ChatServer, server.id, server.connected_drone_ids.clone(), -1.00)),
                    _ => unreachable!()
                }
            }
        }
    }

    let node_count = all_nodes.len();
    let mut nodes = Vec::with_capacity(node_count);
    let mut node_index = 0;

    let base_structure = [2, 4, 4];
    let mut current_y = -vertical_spacing * 3.0;

    for &count in &base_structure {
        let x_offset = (count as f32 - 1.0) * horizontal_spacing / 2.0;
        for i in 0..count {
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
        current_y += vertical_spacing;
    }

    while node_index < node_count {
        let nodes_in_row = (node_count - node_index).min(4);
        let x_offset = (nodes_in_row as f32 - 1.0) * horizontal_spacing / 2.0;
        for i in 0..nodes_in_row {
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
        current_y += vertical_spacing;
    }

    nodes
}
