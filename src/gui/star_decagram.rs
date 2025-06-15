use bevy::prelude::*;
use crate::network_initializer::network_initializer::*;
use crate::gui::login_window::{NodeConfig, NodeType};
use crate::gui::shared_info_plugin::{NodeCategory, SeenClients};

pub fn spawn_star_decagram(clients: &SeenClients) -> Vec<NodeConfig> {
    let config = parse_config();
    let radius = 200.0;
    let mut nodes = Vec::new();

    let node_count = config.drone.len() + config.client.len() + config.server.len();

    let calculate_position = |index: usize| -> Vec2 {
        let angle = index as f32 * std::f32::consts::TAU / node_count as f32;
        Vec2::new(
            radius * angle.cos(),
            radius * angle.sin()
        )
    };

    let mut current_index = 0;

    for drone in &config.drone {
        let position = calculate_position(current_index);
        nodes.push(NodeConfig::new(
            NodeType::Drone,
            drone.id,
            position,
            drone.connected_node_ids.clone(),
            drone.pdr
        ));
        current_index += 1;
    }

    for client in &config.client {
        if let Some(NodeCategory::Client(client_type)) = clients.nodes.get(&client.id) {
            let position = calculate_position(current_index);
            let node_type = match client_type {
                NodeType::WebBrowser => NodeType::WebBrowser,
                NodeType::ChatClient => NodeType::ChatClient,
                _ => unreachable!(),
            };
            nodes.push(NodeConfig::new(
                node_type,
                client.id,
                position,
                client.connected_drone_ids.clone(),
                -1.00
            ));
            current_index += 1;
        }
    }

    for server in &config.server {
        if let Some(NodeCategory::Server(server_type)) = clients.nodes.get(&server.id) {
            let position = calculate_position(current_index);
            let node_type = match server_type {
                NodeType::TextServer => NodeType::TextServer,
                NodeType::MediaServer => NodeType::MediaServer,
                NodeType::ChatServer => NodeType::ChatServer,
                _ => unreachable!(),
            };
            nodes.push(NodeConfig::new(
                node_type,
                server.id,
                position,
                server.connected_drone_ids.clone(),
                -1.00
            ));
            current_index += 1;
        }
    }

    nodes
}