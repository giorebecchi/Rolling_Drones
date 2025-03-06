use bevy::prelude::*;
use crate::network_initializer::network_initializer::*;
use crate::GUI::login_window::{NodeConfig,NodeType};


pub fn spawn_star_decagram() -> Vec<NodeConfig> {
    let config = parse_config("assets/configurations/star.toml");
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
            drone.connected_node_ids.clone()
        ));
        current_index += 1;
    }

    for client in &config.client {
        let position = calculate_position(current_index);
        nodes.push(NodeConfig::new(
            NodeType::Client,
            client.id,
            position,
            client.connected_drone_ids.clone()
        ));
        current_index += 1;
    }

    for server in &config.server {
        let position = calculate_position(current_index);
        nodes.push(NodeConfig::new(
            NodeType::Server,
            server.id,
            position,
            server.connected_drone_ids.clone()
        ));
        current_index += 1;
    }

    nodes
}