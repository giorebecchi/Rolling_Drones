use std::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use wg_2024::network::NodeId;
use crate::gui::login_window::{AppState, DisplayableLog, NodeConfig, NodeType, NodesConfig, SimWindows};
use crate::simulation_control::simulation_control::SimulationController;
use petgraph::visit::{EdgeRef, IntoEdgeReferences, IntoNodeIdentifiers};

pub struct AdvancedLogsPlugin;
impl Plugin for AdvancedLogsPlugin{
    fn build(&self, app: &mut App){
        app
            .init_resource::<LogInfo>()
            .add_systems(Update, log_window.run_if(in_state(AppState::InGame)));
    }
}

fn log_window(
    mut contexts: EguiContexts,
    nodes: Res<NodesConfig>,
    mut log_info: ResMut<LogInfo>,
    sim_log: ResMut<DisplayableLog>,
    sim: Res<SimulationController>,
    mut open: ResMut<SimWindows>
) {
    if open.advanced_logs {
        let window_id = egui::Id::new("advanced_logs");
        let window = egui::Window::new("advanced_logs_window")
            .id(window_id)
            .resizable(true)
            .collapsible(true)
            .default_size([600., 700.]);

        if let Some(contexts)=contexts.try_ctx_mut() {
            window.show(contexts, |ui| {
                ui.label("Node: ");
                let current_selected_client = match log_info.selected_node.clone() {
                    Some((id, node_type)) => format!("{:?} :{}", node_type, id),
                    None => "Select Node".to_string()
                };

                egui::ComboBox::from_id_salt("msg_select")
                    .selected_text(current_selected_client)
                    .show_ui(ui, |ui| {
                        let nodes: Vec<&NodeConfig> = nodes.0.iter()
                            .filter(|node| node.node_type != NodeType::Drone)
                            .collect();

                        for node in nodes {
                            let selected = log_info.selected_node == Some((node.id, node.node_type.clone()));
                            if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                log_info.selected_node = Some((node.id, node.node_type.clone()));
                            }
                        }
                    });

                if let Some(node) = log_info.selected_node.clone() {
                    ui.horizontal(|ui| {
                        if ui.button(format!("Connections found by {:?}: {}", node.1, node.0)).clicked() {
                            sim.ask_topology_graph(node.0, node.1.clone());
                            log_info.show_graph = true;
                        }

                        if log_info.show_graph {
                            ui.checkbox(&mut log_info.show_missed_connections, "Show missed connections");
                            ui.checkbox(&mut log_info.show_incorrect_connections, "Show incorrect connections");
                        }
                    });

                    if log_info.show_graph {
                        let has_ungraph = sim_log.graph.get(&node.0).is_some();
                        let has_server_graph = sim_log.server_graph.get(&node.0).is_some();

                        if has_ungraph || has_server_graph {
                            if ui.button("Hide Graph").clicked() {
                                log_info.show_graph = false;
                            }

                            ui.collapsing("Network Topology Graph", |ui| {
                                ui.label("Topology:");

                                let (discovered_connections, all_nodes_in_graph) = if let Some(graph) = sim_log.graph.get(&node.0) {
                                    let mut connections = HashSet::new();
                                    let mut nodes_set = HashSet::new();

                                    for node_id in graph.node_identifiers() {
                                        nodes_set.insert(node_id);
                                    }

                                    for edge in graph.edge_references() {
                                        let (source, target, _) = edge.clone();
                                        connections.insert((source.min(target), source.max(target)));
                                    }
                                    (connections, nodes_set)
                                } else if let Some(graph) = sim_log.server_graph.get(&node.0) {
                                    let mut connections = HashSet::new();
                                    let mut nodes_set = HashSet::new();

                                    for node_index in graph.node_indices() {
                                        if let Some((node_id, _)) = graph.node_weight(node_index) {
                                            nodes_set.insert(*node_id);
                                        }
                                    }

                                    for edge in graph.edge_references() {
                                        let source_idx = edge.source();
                                        let target_idx = edge.target();

                                        if let (Some((source_id, _)), Some((target_id, _))) =
                                            (graph.node_weight(source_idx), graph.node_weight(target_idx)) {
                                            let min_id = (*source_id).min(*target_id);
                                            let max_id = (*source_id).max(*target_id);
                                            connections.insert((min_id, max_id));
                                        }
                                    }
                                    (connections, nodes_set)
                                } else {
                                    (HashSet::new(), HashSet::new())
                                };

                                let mut actual_connections = HashSet::new();
                                for node_config in nodes.0.iter() {
                                    if all_nodes_in_graph.contains(&node_config.id) {
                                        for &connected_id in &node_config.connected_node_ids {
                                            if all_nodes_in_graph.contains(&connected_id) {
                                                let min_id = node_config.id.min(connected_id);
                                                let max_id = node_config.id.max(connected_id);
                                                actual_connections.insert((min_id, max_id));
                                            }
                                        }
                                    }
                                }

                                let missed_connections: Vec<(NodeId, NodeId)> = actual_connections
                                    .difference(&discovered_connections)
                                    .cloned()
                                    .collect();

                                let incorrect_connections: Vec<(NodeId, NodeId)> = discovered_connections
                                    .difference(&actual_connections)
                                    .cloned()
                                    .collect();

                                ui.horizontal(|ui| {
                                    ui.label(format!("Discovered: {} connections", discovered_connections.len()));
                                    ui.label("|");
                                    ui.label(format!("Actual: {} connections", actual_connections.len()));
                                    ui.label("|");
                                    ui.colored_label(
                                        egui::Color32::RED,
                                        format!("Missed: {} connections", missed_connections.len())
                                    );
                                    ui.label("|");
                                    ui.colored_label(
                                        egui::Color32::from_rgb(255, 140, 0),
                                        format!("Incorrect: {} connections", incorrect_connections.len())
                                    );
                                });

                                egui::Frame::new()
                                    .fill(egui::Color32::from_gray(240))
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(180)))
                                    .inner_margin(5.0)
                                    .show(ui, |ui| {
                                        egui::ScrollArea::both()
                                            .max_height(300.0)
                                            .max_width(ui.available_width())
                                            .show(ui, |ui| {
                                                let graph_size = egui::vec2(
                                                    ui.available_width().max(400.0),
                                                    280.0
                                                );

                                                let graph_response = ui.allocate_rect(
                                                    egui::Rect::from_min_size(ui.cursor().min, graph_size),
                                                    egui::Sense::hover()
                                                );

                                                let painter = ui.painter_at(graph_response.rect);

                                                if let Some(graph) = sim_log.graph.get(&node.0) {
                                                    let mut all_nodes = Vec::new();
                                                    let mut connections = Vec::new();

                                                    for node_id in graph.node_identifiers() {
                                                        all_nodes.push(node_id);
                                                    }

                                                    for edge in graph.edge_references() {
                                                        let (source, target, _) = edge.clone();
                                                        connections.push((source, target));
                                                    }

                                                    render_graph_visualization_with_errors(
                                                        &painter,
                                                        &graph_response.rect,
                                                        all_nodes,
                                                        connections,
                                                        if log_info.show_missed_connections { missed_connections } else { vec![] },
                                                        if log_info.show_incorrect_connections { incorrect_connections } else { vec![] }
                                                    );
                                                } else if let Some(graph) = sim_log.server_graph.get(&node.0) {
                                                    let mut all_nodes = Vec::new();
                                                    let mut connections = Vec::new();

                                                    for node_index in graph.node_indices() {
                                                        if let Some((node_id, _node_type)) = graph.node_weight(node_index) {
                                                            all_nodes.push(*node_id);
                                                        }
                                                    }

                                                    for edge in graph.edge_references() {
                                                        let source_idx = edge.source();
                                                        let target_idx = edge.target();

                                                        if let (Some((source_id, _)), Some((target_id, _))) =
                                                            (graph.node_weight(source_idx), graph.node_weight(target_idx)) {
                                                            connections.push((*source_id, *target_id));
                                                        }
                                                    }

                                                    render_graph_visualization_with_errors(
                                                        &painter,
                                                        &graph_response.rect,
                                                        all_nodes,
                                                        connections,
                                                        if log_info.show_missed_connections { missed_connections } else { vec![] },
                                                        if log_info.show_incorrect_connections { incorrect_connections } else { vec![] }
                                                    );
                                                }
                                            });
                                    });
                            });
                        } else {
                            ui.label("No graph data available. Try requesting topology data first.");
                            if ui.button("Hide Graph View").clicked() {
                                log_info.show_graph = false;
                            }
                        }
                    }

                    ui.separator();
                    ui.label("Last sent message:");

                    let mut node_messages: Vec<_> = sim_log.msg_log.iter()
                        .filter(|((initiator_id, _), _)| *initiator_id == node.0)
                        .map(|((node_id, session_id), msg)| (*node_id, *session_id, msg))
                        .collect();

                    node_messages.sort_by(|a, b| b.1.cmp(&a.1));

                    if let Some((_, session_id, msg_content)) = node_messages.first() {
                        ui.group(|ui| {
                            ui.label(format!("Session ID: {}", session_id));
                            ui.label(msg_content.to_string());
                        });

                        ui.separator();


                        ui.group(|ui| {
                            ui.label("Routes taken:");

                            if let Some(routes) = sim_log.route_attempt.get(&(node.0, *session_id)) {
                                if routes.is_empty() {
                                    ui.label("  No route information available");
                                } else {
                                    for (idx, route) in routes.iter().enumerate() {
                                        ui.horizontal(|ui| {
                                            ui.label(format!("  Route {}:", idx + 1));

                                            let route_str = route.iter()
                                                .map(|node| format!("{}", node))
                                                .collect::<Vec<_>>()
                                                .join(" → ");

                                            ui.monospace(&route_str);


                                            let reliability = calculate_route_reliability(route, &nodes);
                                            let reliability_color = if reliability >= 0.9 {
                                                egui::Color32::GREEN
                                            } else if reliability >= 0.7 {
                                                egui::Color32::YELLOW
                                            } else {
                                                egui::Color32::RED
                                            };

                                            ui.label(" | ");
                                            ui.colored_label(
                                                reliability_color,
                                                format!("Reliability: {:.1}%", reliability * 100.0)
                                            );
                                        });


                                        ui.indent(format!("route_details_{}", idx), |ui| {
                                            ui.collapsing("Show node details", |ui| {
                                                for &node_id in route.iter() {
                                                    if let Some(node_config) = nodes.0.iter().find(|n| n.id == node_id) {
                                                        let pdr = node_config.pdr;
                                                        let success_rate = 1.0 - pdr;
                                                        ui.horizontal(|ui| {
                                                            ui.label(format!("  Node {} ({:?}): ",
                                                                             node_id,
                                                                             node_config.node_type
                                                            ));
                                                            ui.colored_label(
                                                                if success_rate >= 0.9 { egui::Color32::GREEN } else if success_rate >= 0.7 { egui::Color32::YELLOW } else { egui::Color32::RED },
                                                                format!("PDR: {:.2}, Success: {:.1}%", pdr, success_rate * 100.0)
                                                            );
                                                        });
                                                    }
                                                }
                                            });
                                        });
                                    }
                                }
                            } else {
                                ui.label("  No route information recorded");
                            }
                        });

                        ui.separator();

                        ui.group(|ui| {
                            ui.label("Fragments dropped:");

                            let lost_fragments: Vec<_> = sim_log.lost_msg.iter()
                                .filter(|((_, session), _)| *session == *session_id)
                                .collect();

                            if lost_fragments.is_empty() {
                                ui.label("  No fragments were lost");
                            } else {
                                egui::ScrollArea::vertical()
                                    .max_height(150.0)
                                    .show(ui, |ui| {
                                        for ((drone_id, _), fragments) in lost_fragments {
                                            for fragment in fragments {
                                                ui.label(format!(
                                                    "  Fragment {} was dropped by Drone {}",
                                                    fragment.fragment_index, drone_id
                                                ));
                                            }
                                        }
                                    });
                            }
                        });

                        ui.separator();
                        ui.collapsing("Other errors for this session", |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(200.0)
                                .show(ui, |ui| {
                                    let mut has_any_errors = false;

                                    // Lost NACKs
                                    let lost_nacks: Vec<_> = sim_log.lost_nack.iter()
                                        .filter(|((_, session), _)| *session == *session_id)
                                        .collect();

                                    if !lost_nacks.is_empty() {
                                        has_any_errors = true;
                                        ui.group(|ui| {
                                            ui.label("Lost NACKs:");
                                            for ((node_id, _), nacks) in lost_nacks {
                                                ui.label(format!("  • Lost NACK at Drone {}: {:?}", node_id, nacks));
                                            }
                                        });
                                        ui.add_space(5.0);
                                    }

                                    // Lost ACKs
                                    let lost_acks: Vec<_> = sim_log.lost_ack.iter()
                                        .filter(|((_, session), _)| *session == *session_id)
                                        .collect();

                                    if !lost_acks.is_empty() {
                                        has_any_errors = true;
                                        ui.group(|ui| {
                                            ui.label("Lost ACKs:");
                                            for ((node_id, _), acks) in lost_acks {
                                                ui.label(format!("  • Lost ACK at Drone {}: {:?}", node_id, acks));
                                            }
                                        });
                                        ui.add_space(5.0);
                                    }

                                    // Lost Flood Requests
                                    let lost_flood_req: Vec<_> = sim_log.lost_flood_req.iter()
                                        .filter(|((_, session), _)| *session == *session_id)
                                        .collect();

                                    if !lost_flood_req.is_empty() {
                                        has_any_errors = true;
                                        ui.group(|ui| {
                                            ui.label("Lost Flood Requests:");
                                            for ((node_id, _), reqs) in lost_flood_req {
                                                ui.label(format!("  • Lost FloodReq at Drone {}: {:?}", node_id, reqs));
                                            }
                                        });
                                        ui.add_space(5.0);
                                    }

                                    // Lost Flood Responses
                                    let lost_flood_resp: Vec<_> = sim_log.lost_flood_resp.iter()
                                        .filter(|((_, session), _)| *session == *session_id)
                                        .collect();

                                    if !lost_flood_resp.is_empty() {
                                        has_any_errors = true;
                                        ui.group(|ui| {
                                            ui.label("Lost Flood Responses:");
                                            for ((node_id, _), resps) in lost_flood_resp {
                                                ui.label(format!("  • Lost FloodResp at Drone {}: {:?}", node_id, resps));
                                            }
                                        });
                                    }

                                    if !has_any_errors {
                                        ui.label("No other errors found for this session.");
                                    }
                                });
                        });

                    } else {
                        ui.label("No messages sent by this node found.");
                    }
                }

                if ui.button("Close Window").clicked() {
                    open.advanced_logs = false;
                }
            });
        }
    }
}

#[derive(Resource, Default)]
struct LogInfo {
    selected_node: Option<(NodeId, NodeType)>,
    show_graph: bool,
    show_missed_connections: bool,
    show_incorrect_connections: bool,
}

fn render_graph_visualization_with_errors(
    painter: &egui::Painter,
    rect: &egui::Rect,
    all_nodes: Vec<NodeId>,
    connections: Vec<(NodeId, NodeId)>,
    missed_connections: Vec<(NodeId, NodeId)>,
    incorrect_connections: Vec<(NodeId, NodeId)>
) {
    let mut node_positions = HashMap::new();

    if !all_nodes.is_empty() {
        let padding = 30.0;
        let center_x = rect.center().x;
        let center_y = rect.center().y;
        let radius = (rect.height().min(rect.width()) - padding * 2.0) * 0.4;

        for (i, node_id) in all_nodes.iter().enumerate() {
            let angle = 2.0 * std::f32::consts::PI * (i as f32) / (all_nodes.len() as f32);
            let x = center_x + radius * angle.cos();
            let y = center_y + radius * angle.sin();
            node_positions.insert(*node_id, egui::pos2(x, y));
        }

        for (source, target) in &missed_connections {
            if let (Some(start_pos), Some(end_pos)) = (node_positions.get(source), node_positions.get(target)) {
                let num_dashes = 10;
                let dash_length = 0.6 / num_dashes as f32;
                let gap_length = 0.4 / num_dashes as f32;

                for i in 0..num_dashes {
                    let t_start = (i as f32) * (dash_length + gap_length);
                    let t_end = t_start + dash_length;

                    if t_end <= 1.0 {
                        let dash_start = egui::pos2(
                            start_pos.x + (end_pos.x - start_pos.x) * t_start,
                            start_pos.y + (end_pos.y - start_pos.y) * t_start
                        );
                        let dash_end = egui::pos2(
                            start_pos.x + (end_pos.x - start_pos.x) * t_end,
                            start_pos.y + (end_pos.y - start_pos.y) * t_end
                        );

                        painter.line_segment(
                            [dash_start, dash_end],
                            egui::Stroke::new(2.0, egui::Color32::RED),
                        );
                    }
                }
            }
        }

        for (source, target) in &incorrect_connections {
            if let (Some(start_pos), Some(end_pos)) = (node_positions.get(source), node_positions.get(target)) {
                let num_dots = 20;
                let dot_length = 0.3 / num_dots as f32;
                let gap_length = 0.7 / num_dots as f32;

                for i in 0..num_dots {
                    let t_start = (i as f32) * (dot_length + gap_length);
                    let t_end = t_start + dot_length;

                    if t_end <= 1.0 {
                        let dot_start = egui::pos2(
                            start_pos.x + (end_pos.x - start_pos.x) * t_start,
                            start_pos.y + (end_pos.y - start_pos.y) * t_start
                        );
                        let dot_end = egui::pos2(
                            start_pos.x + (end_pos.x - start_pos.x) * t_end,
                            start_pos.y + (end_pos.y - start_pos.y) * t_end
                        );

                        painter.line_segment(
                            [dot_start, dot_end],
                            egui::Stroke::new(3.0, egui::Color32::from_rgb(255, 140, 0)),
                        );
                    }
                }
            }
        }

        for (source, target) in &connections {
            if let (Some(start_pos), Some(end_pos)) = (node_positions.get(source), node_positions.get(target)) {
                painter.line_segment(
                    [*start_pos, *end_pos],
                    egui::Stroke::new(1.0, egui::Color32::GRAY),
                );
            }
        }

        for (node_id, pos) in &node_positions {
            painter.circle(
                *pos,
                12.0,
                egui::Color32::from_rgb(100, 100, 255),
                egui::Stroke::new(1.0, egui::Color32::BLACK)
            );
            painter.text(
                *pos,
                egui::Align2::CENTER_CENTER,
                format!("{}", node_id),
                egui::FontId::default(),
                egui::Color32::WHITE,
            );
        }
    }
}


fn calculate_route_reliability(route: &[NodeId], nodes: &NodesConfig) -> f32 {
    let mut reliability = 1.0;

    for &node_id in route.iter().skip(1).take(route.len().saturating_sub(2)) {
        if let Some(node_config) = nodes.0.iter().find(|n| n.id == node_id) {
            let pdr = node_config.pdr;
            let node_success_rate = 1.0 - pdr;
            reliability *= node_success_rate;
        }
    }

    reliability
}