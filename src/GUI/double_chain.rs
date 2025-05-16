use bevy::prelude::*;
use crate::common_things::common::ClientType;
use crate::GUI::login_window::{NodeConfig, NodeType, AddedDrone};
use crate::GUI::shared_info_plugin::SeenClients;
use crate::network_initializer::network_initializer::parse_config;
use crate::simulation_control::simulation_control::MyNodeType;

pub fn spawn_double_chain(
    added_drone: Option<AddedDrone>,
    clients : &mut SeenClients,
) -> Vec<NodeConfig>
{
    let config = parse_config("assets/configurations/double_chain.toml");
    let mut node_count = config.client.len() + config.server.len() + config.drone.len();
    if let Some(_)=added_drone{
        node_count+=1;
    }

    // Calculate how many nodes go in each row
    let nodes_in_first_line = node_count / 2;
    let nodes_in_second_line = if node_count % 2 == 0 { node_count / 2 } else { node_count / 2 + 1 };

    // Define spacing parameters
    let horizontal_spacing = 100.0;
    let vertical_offset = 50.0;

    let mut nodes = Vec::with_capacity(node_count);

    // Combine all node types into a single processing stream
    let mut all_nodes = Vec::with_capacity(node_count);

    // Add drones
    for drone in &config.drone {
        all_nodes.push((NodeType::Drone, drone.id, &drone.connected_node_ids));
    }
    let mut links=Vec::new();
    if let Some(added_drone)=added_drone{
        links=added_drone.drone.0.clone();
        all_nodes.push((NodeType::Drone, added_drone.drone.1, &links));
    }

    // Add clients
    for client in &config.client {
        for (client_type, id) in &clients.clients{
            if id.clone() == client.id{
                match client_type{
                    MyNodeType::WebBrowser=>all_nodes.push((NodeType::WebBrowser, client.id, &client.connected_drone_ids)),
                    MyNodeType::ChatClient=>all_nodes.push((NodeType::ChatClient, client.id, &client.connected_drone_ids)),
                    _=>unreachable!()
                }
            }
        }
    }

    // Add servers
    for server in &config.server {
        for (server_type, id) in &clients.servers {
            if id.clone() == server.id {
                match server_type {
                    MyNodeType::TextServer=>all_nodes.push((NodeType::TextServer, server.id, &server.connected_drone_ids)),
                    MyNodeType::MediaServer=>all_nodes.push((NodeType::MediaServer, server.id, &server.connected_drone_ids)),
                    MyNodeType::ChatServer=>all_nodes.push((NodeType::ChatServer, server.id, &server.connected_drone_ids)),
                    _=>unreachable!()

                }
            }
        }
    }

    // Calculate positions and create node configs
    for (i, (node_type, id, connected_ids)) in all_nodes.iter().enumerate() {
        let (x, y) = if i < nodes_in_first_line {
            // First line positioning
            let position_in_line = i;
            let x = (position_in_line as f32 - (nodes_in_first_line - 1) as f32 / 2.0) * horizontal_spacing;
            (x, vertical_offset)
        } else {
            // Second line positioning
            let position_in_line = i - nodes_in_first_line;
            let x = (position_in_line as f32 - (nodes_in_second_line - 1) as f32 / 2.0) * horizontal_spacing;
            (x, -vertical_offset)
        };

        nodes.push(NodeConfig::new(
            node_type.clone(),
            *id,
            Vec2::new(x, y),
            (*connected_ids).clone()
        ));
    }

    nodes
}