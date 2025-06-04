use bevy::prelude::*;
use crate::network_initializer::network_initializer::*;
use crate::gui::login_window::{NodeConfig, NodeType};
use crate::gui::shared_info_plugin::SeenClients;
use crate::simulation_control::simulation_control::MyNodeType;

pub fn spawn_star_decagram(
    clients: &mut SeenClients
) -> Vec<NodeConfig>
{
    let config = parse_config();
    let radius = 200.0;
    let mut nodes = Vec::new();

    let mut node_count = config.drone.len() + config.client.len() + config.server.len();


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

        for (client_type, id) in &clients.clients {
            if id.clone() == client.id {
                let position = calculate_position(current_index);
                let node_type=match client_type{
                    MyNodeType::WebBrowser=>NodeType::WebBrowser,
                    MyNodeType::ChatClient=>NodeType::ChatClient,
                    _=>unreachable!(),
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
    }

    for server in &config.server {
        for (server_type, id) in &clients.servers {
            if id.clone() == server.id {
                let position = calculate_position(current_index);
                let node_type=match server_type {
                    MyNodeType::TextServer=>NodeType::TextServer,
                    MyNodeType::MediaServer=>NodeType::MediaServer,
                    MyNodeType::ChatServer=>NodeType::ChatServer,
                    _=>unreachable!(),
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
    }

    nodes
}