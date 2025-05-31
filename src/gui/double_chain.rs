use bevy::prelude::*;
use crate::gui::login_window::{NodeConfig, NodeType};
use crate::gui::shared_info_plugin::SeenClients;
use crate::network_initializer::network_initializer::parse_config;
use crate::simulation_control::simulation_control::MyNodeType;

pub fn spawn_double_chain(
    clients: &mut SeenClients,
) -> Vec<NodeConfig>
{
    let config = parse_config("assets/configurations/double_chain.toml");
    let node_count = config.client.len() + config.server.len() + config.drone.len();

    let nodes_per_row = if node_count % 2 == 0 {
        node_count / 2
    } else {
        (node_count + 1) / 2
    };

    let num_rows = (node_count + nodes_per_row - 1) / nodes_per_row;

    let horizontal_spacing = 100.0;
    let vertical_spacing = 100.0;
    let base_y_offset = ((num_rows - 1) as f32 * vertical_spacing) / 2.0;

    let mut nodes = Vec::with_capacity(node_count);
    let mut all_nodes = Vec::with_capacity(node_count);

    for drone in &config.drone {
        all_nodes.push((NodeType::Drone, drone.id, &drone.connected_node_ids));
    }

    for client in &config.client {
        for (client_type, id) in &clients.clients {
            if id.clone() == client.id {
                match client_type {
                    MyNodeType::WebBrowser => all_nodes.push((NodeType::WebBrowser, client.id, &client.connected_drone_ids)),
                    MyNodeType::ChatClient => all_nodes.push((NodeType::ChatClient, client.id, &client.connected_drone_ids)),
                    _ => unreachable!()
                }
            }
        }
    }

    for server in &config.server {
        for (server_type, id) in &clients.servers {
            if id.clone() == server.id {
                match server_type {
                    MyNodeType::TextServer => all_nodes.push((NodeType::TextServer, server.id, &server.connected_drone_ids)),
                    MyNodeType::MediaServer => all_nodes.push((NodeType::MediaServer, server.id, &server.connected_drone_ids)),
                    MyNodeType::ChatServer => all_nodes.push((NodeType::ChatServer, server.id, &server.connected_drone_ids)),
                    _ => unreachable!()
                }
            }
        }
    }

    for (i, (node_type, id, connected_ids)) in all_nodes.iter().enumerate() {
        let row = i / nodes_per_row;
        let position_in_row = i % nodes_per_row;

        let nodes_in_current_row = if row == num_rows - 1 {
            node_count - (row * nodes_per_row)
        } else {
            nodes_per_row
        };

        let x = (position_in_row as f32 - (nodes_in_current_row - 1) as f32 / 2.0) * horizontal_spacing;

        let y = base_y_offset - (row as f32 * vertical_spacing);

        nodes.push(NodeConfig::new(
            node_type.clone(),
            *id,
            Vec2::new(x, y),
            (*connected_ids).clone()
        ));
    }

    nodes
}