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

    let horizontal_spacing = 80.0;
    let vertical_spacing = 120.0;

    let mut all_nodes = Vec::with_capacity(node_count);

    for drone in &config.drone {
        all_nodes.push((NodeType::Drone, drone.id, &drone.connected_node_ids, drone.pdr));
    }

    for client in &config.client {
        if let Some((client_type, _)) = clients.clients.iter()
            .find(|(_, id)| *id == client.id) {
            match client_type {
                NodeType::WebBrowser => all_nodes.push((NodeType::WebBrowser, client.id, &client.connected_drone_ids, -1.00)),
                NodeType::ChatClient => all_nodes.push((NodeType::ChatClient, client.id, &client.connected_drone_ids, -1.00)),
                _ => unreachable!()
            }
        }
    }

    for server in &config.server {
        if let Some((server_type, _)) = clients.servers.iter()
            .find(|(_, id)| *id == server.id) {
            match server_type {
                NodeType::TextServer => all_nodes.push((NodeType::TextServer, server.id, &server.connected_drone_ids, -1.00)),
                NodeType::MediaServer => all_nodes.push((NodeType::MediaServer, server.id, &server.connected_drone_ids, -1.00)),
                NodeType::ChatServer => all_nodes.push((NodeType::ChatServer, server.id, &server.connected_drone_ids, -1.00)),
                _ => unreachable!()
            }
        }
    }

    let top_count = node_count / 2 + 1;
    let bottom_count = node_count / 2;

    let (top_nodes, bottom_nodes) = all_nodes.split_at(top_count);

    let mut nodes = Vec::with_capacity(node_count);

    for (i, (node_type, id, connected_ids, pdr)) in top_nodes.iter().enumerate() {
        let x = (i as f32 - (top_count - 1) as f32 / 2.0) * horizontal_spacing;
        let y = vertical_spacing / 2.0;

        nodes.push(NodeConfig::new(
            node_type.clone(),
            *id,
            Vec2::new(x, y),
            (*connected_ids).clone(),
            *pdr
        ));
    }

    for (i, (node_type, id, connected_ids, pdr)) in bottom_nodes.iter().enumerate() {
        let x = (i as f32 - (bottom_count - 1) as f32 / 2.0) * horizontal_spacing;
        let y = -vertical_spacing / 2.0;

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
