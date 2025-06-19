use std::collections::{HashMap, HashSet, VecDeque};
use wg_2024::network::NodeId;
use crate::gui::login_window::{NodeConfig, NodeType};

fn build_adjacency_list(nodes: &[NodeConfig]) -> HashMap<NodeId, Vec<NodeId>> {
    let mut graph = HashMap::new();

    for node in nodes {
        graph.insert(node.id, node.connected_node_ids.clone());
    }

    graph
}

fn has_path(
    graph: &HashMap<NodeId, Vec<NodeId>>,
    nodes: &[NodeConfig],
    source: NodeId,
    target: NodeId
) -> bool {
    if source == target {
        return true;
    }

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    queue.push_back(source);
    visited.insert(source);

    while let Some(current) = queue.pop_front() {
        if let Some(neighbors) = graph.get(&current) {
            for &neighbor in neighbors {
                if neighbor == target {
                    return true;
                }

                let is_drone = nodes.iter()
                    .find(|n| n.id == neighbor)
                    .map(|n| n.node_type == NodeType::Drone)
                    .unwrap_or(false);

                let is_source = neighbor == source;
                let is_target = neighbor == target;


                if (is_drone || is_source || is_target) && !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }
    }

    false
}

fn validate_chat_connectivity(nodes: &[NodeConfig]) -> Result<(), String> {
    let graph = build_adjacency_list(nodes);

    let chat_clients: Vec<&NodeConfig> = nodes.iter()
        .filter(|n| n.node_type == NodeType::ChatClient)
        .collect();

    let chat_servers: Vec<&NodeConfig> = nodes.iter()
        .filter(|n| n.node_type == NodeType::ChatServer)
        .collect();

    for client in &chat_clients {
        let mut can_reach_server = false;
        for server in &chat_servers {
            if has_path(&graph, nodes, client.id, server.id) {
                can_reach_server = true;
                break;
            }
        }
        if !can_reach_server {
            return Err(format!("ChatClient {} cannot reach any ChatServer", client.id));
        }
    }

    for server in &chat_servers {
        let mut reachable_client_ids = Vec::new();
        for client in &chat_clients {
            if has_path(&graph, nodes, server.id, client.id) {
                reachable_client_ids.push(client.id);
            }
        }

        if reachable_client_ids.len() < 2 {
            return Err(format!(
                "ChatServer {} must be able to reach at least 2 different ChatClients through drones, but can only reach {}",
                server.id,
                reachable_client_ids.len()
            ));
        }
    }

    Ok(())
}

fn validate_web_media_connectivity(nodes: &[NodeConfig]) -> Result<(), String> {
    let graph = build_adjacency_list(nodes);

    let web_browsers: Vec<&NodeConfig> = nodes.iter()
        .filter(|n| n.node_type == NodeType::WebBrowser)
        .collect();

    let text_servers: Vec<&NodeConfig> = nodes.iter()
        .filter(|n| n.node_type == NodeType::TextServer)
        .collect();

    let media_servers: Vec<&NodeConfig> = nodes.iter()
        .filter(|n| n.node_type == NodeType::MediaServer)
        .collect();

    for browser in &web_browsers {
        for text in &text_servers {
            if !has_path(&graph, nodes, browser.id, text.id) {
                return Err(format!("WebBrowser {} cannot reach TextServer {}", browser.id,text.id));
            }
        }
        for media in &media_servers {
            if !has_path(&graph, nodes, browser.id, media.id) {
                return Err(format!("WebBrowser {} cannot reach MediaServer {} ", browser.id, media.id));
            }
        }


    }

    for text in &text_servers {
        for media in &media_servers {
            if !has_path(&graph, nodes, text.id, media.id) {
                return Err(format!("TextServer {} cannot reach MediaServer {} ", text.id, media.id));
            }
        }
        for check_server in &text_servers {
            if text.id != check_server.id {
                if !has_path(&graph, nodes, text.id, check_server.id) {
                    return Err(format!("TextServer {} cannot reach Text/MediaServer {}", text.id, check_server.id));
                }
            }
        }
    }

    Ok(())
}
pub fn validate_drone_pdr(nodes: &[NodeConfig]) -> Result<(), String>{
    for node in nodes{
        if node.node_type==NodeType::Drone{
            if node.pdr > 1.00 || node.pdr < 0.00{
                return Err(format!("Drone {} was set with a pdr of {}", node.id, node.pdr));
            }
        }
    }
    Ok(())
}
pub fn validate_duplex_connections(nodes: &[NodeConfig]) -> Result<(), String>{

    let mut connection_map: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

    for node in nodes {
        let connected_set: HashSet<NodeId> = node.connected_node_ids.iter().cloned().collect();
        connection_map.insert(node.id, connected_set);
    }


    for node in nodes {
        for &neighbor_id in &node.connected_node_ids {
            if let Some(neighbor_connections) = connection_map.get(&neighbor_id) {
                if !neighbor_connections.contains(&node.id) {
                    return Err(format!("Missing connection between {}-{}",neighbor_id,node.id));
                }
            } else {
                return Err(format!("Missing connection between {}-{}",node.id,neighbor_id));
            }
        }
    }
    Ok(())
}

pub fn simulate_network_change<F>(nodes: &[NodeConfig], change: F) -> Vec<NodeConfig>
where
    F: Fn(&mut Vec<NodeConfig>),
{
    let mut simulated_nodes = nodes.to_vec();
    change(&mut simulated_nodes);
    simulated_nodes
}

pub fn would_break_connectivity(simulated_nodes: &[NodeConfig]) -> Result<(), String> {
    validate_chat_connectivity(&simulated_nodes)?;
    validate_web_media_connectivity(&simulated_nodes)?;
    Ok(())
}