use bevy::prelude::*;
use crate::gui::login_window::{NodeConfig, NodeType};
use crate::gui::shared_info_plugin::SeenClients;
use crate::network_initializer::network_initializer::parse_config;

pub fn spawn_double_chain(
    clients: &mut SeenClients,
) -> Vec<NodeConfig>
{
    let config = parse_config();
    let node_count = config.client.len() + config.server.len() + config.drone.len();


    let nodes_in_first_row = (node_count + 1) / 2;
    let nodes_in_second_row = node_count - nodes_in_first_row;

    let horizontal_spacing = 100.0;
    let vertical_spacing = 100.0;

    let first_row_y = vertical_spacing / 2.0;
    let second_row_y = -vertical_spacing / 2.0;

    let mut nodes = Vec::with_capacity(node_count);
    let mut all_nodes = Vec::with_capacity(node_count);


    for drone in &config.drone {
        all_nodes.push((NodeType::Drone, drone.id, &drone.connected_node_ids, drone.pdr));
    }

    for client in &config.client {
        for (client_type, id) in &clients.clients {
            if id.clone() == client.id {
                match client_type {
                    NodeType::WebBrowser => all_nodes.push((NodeType::WebBrowser, client.id, &client.connected_drone_ids, -1.00)),
                    NodeType::ChatClient => all_nodes.push((NodeType::ChatClient, client.id, &client.connected_drone_ids, -1.00)),
                    _ => unreachable!()
                }
            }
        }
    }

    for server in &config.server {
        for (server_type, id) in &clients.servers {
            if id.clone() == server.id {
                match server_type {
                    NodeType::TextServer => all_nodes.push((NodeType::TextServer, server.id, &server.connected_drone_ids, -1.00)),
                    NodeType::MediaServer => all_nodes.push((NodeType::MediaServer, server.id, &server.connected_drone_ids, -1.00)),
                    NodeType::ChatServer => all_nodes.push((NodeType::ChatServer, server.id, &server.connected_drone_ids, -1.00)),
                    _ => unreachable!()
                }
            }
        }
    }

    for (i, (node_type, id, connected_ids, pdr)) in all_nodes.iter().enumerate() {
        let (row, nodes_in_row, position_in_row) = if i < nodes_in_first_row {
            (0, nodes_in_first_row, i)
        } else {
            (1, nodes_in_second_row, i - nodes_in_first_row)
        };

        let x = (position_in_row as f32 - (nodes_in_row - 1) as f32 / 2.0) * horizontal_spacing;
        let y = if row == 0 { first_row_y } else { second_row_y };

        nodes.push(NodeConfig::new(
            node_type.clone(),
            *id,
            Vec2::new(x, y),
            (*connected_ids).clone(),
            *pdr
        ));
    }

    nodes
}