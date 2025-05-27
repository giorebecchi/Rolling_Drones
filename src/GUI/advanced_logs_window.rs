use std::collections::HashMap;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use wg_2024::network::NodeId;
use wg_2024::packet::{Fragment};
use crate::GUI::login_window::{AppState, DisplayableLog, NodeConfig, NodeType, NodesConfig, SimWindows, SimulationController};
use crate::simulation_control::simulation_control::MyNodeType;
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

        let ctx = contexts.ctx_mut();

        window.show(ctx, |ui| {
            ui.label("Node: ");
            let current_selected_client= match log_info.selected_node.clone(){
                Some((id,node_type))=>format!("{:?} :{}",node_type,id),
                None=>"Select Node".to_string()
            };
            egui::ComboBox::from_id_salt("msg_select")
                .selected_text(current_selected_client)
                .show_ui(ui, |ui| {
                    let nodes: Vec<&NodeConfig> = nodes.0.iter()
                        .filter(|node| node.node_type != NodeType::Drone)
                        .collect();

                    for node in nodes {
                        let selected = log_info.selected_node == Some((node.id, node.node_type.clone()));
                        if ui.selectable_label(selected,format!("{:?} {}", node.node_type, node.id)).clicked() {
                            log_info.selected_node=Some((node.id, node.node_type.clone()));
                        }
                    }
                });
            if let Some(node)=log_info.selected_node.clone(){
                if ui.button(format!("Connections found by {:?}: {}",node.1, node.0)).clicked(){
                    sim.ask_topology_graph(node.0, node.1);
                    log_info.show_graph = true;
                }

                if log_info.show_graph {
                    // Check which graph contains the selected node and render accordingly
                    let has_ungraph = sim_log.graph.get(&node.0).is_some();
                    let has_server_graph = sim_log.server_graph.get(&node.0).is_some();

                    if has_ungraph || has_server_graph {
                        if ui.button("Hide Graph").clicked() {
                            log_info.show_graph = false;
                        }
                        ui.label("Topology:");

                        let graph_response = ui.allocate_rect(
                            ui.available_rect_before_wrap().shrink(20.0),
                            egui::Sense::hover()
                        );
                        let painter = ui.painter_at(graph_response.rect);

                        if let Some(graph) = sim_log.graph.get(&node.0) {
                            // Handle UnGraphMap<NodeId, u32>
                            let mut all_nodes = Vec::new();
                            let mut connections = Vec::new();

                            for node in graph.node_identifiers() {
                                all_nodes.push(node);
                            }

                            for edge in graph.edge_references() {
                                let (source, target, _) = edge.clone();
                                connections.push((source, target));
                            }

                            render_graph_visualization(&painter, &graph_response.rect, all_nodes, connections);

                        } else if let Some(graph) = sim_log.server_graph.get(&node.0) {
                            // Handle Graph<(NodeId, NodeType), f64>
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

                            render_graph_visualization(&painter, &graph_response.rect, all_nodes, connections);
                        }
                    } else {
                        ui.label("No graph data available. Try requesting topology data first.");
                        if ui.button("Hide Graph View").clicked() {
                            log_info.show_graph = false;
                        }
                    }
                }

                ui.label("last sent message: ");
                let msg:Vec<(&(MyNodeType, NodeId), &(u64,String))> = sim_log.msg_log.iter().filter(|(id, _)| id.1==node.0).collect();

                if !msg.is_empty() {
                    ui.label(format!("{}", msg[0].1.1));

                    egui::ScrollArea::vertical()
                        .max_height(200.)
                        .show(ui, |ui| {
                            ui.label("fragments dropped: ");
                            for (_, (msg_session, _)) in msg.iter() {
                                let fragments: Vec<(&(NodeId, u64), &Vec<Fragment>)> = sim_log.lost_msg.iter().filter(|session_id| session_id.0.1 == msg_session.clone()).collect();
                                for (&(id, session), fragment) in fragments.clone() {
                                    for fragment_info in fragment {
                                        ui.label(format!("fragment: {} was dropped by {}", fragment_info.fragment_index, id));
                                    }
                                }
                            }
                            ui.label("route attempts:");
                            for (_, (session_id, _)) in msg.iter() {
                                for ((id, session), route) in sim_log.route_attempt.iter() {
                                    if session_id == session  {
                                        ui.label(format!("routes chosen {:?}\n", route));
                                    }
                                }
                            }

                        });
                }
                ui.label("Other errors:");
                let lost_nack = sim_log.lost_nack.clone()
                    .into_iter()
                    .map(|((node_id, seq), nacks)| format!("Lost nack, (drone:{:?} session:{}) nack:{:?}", node_id, seq, nacks))
                    .collect::<Vec<String>>()
                    .join("\n");
                ui.label(lost_nack);
                let lost_flood_resp = sim_log.lost_flood_resp.clone()
                    .into_iter()
                    .map(|((node_id, session), flood)| format!("Lost FloodResp, (drone:{:?} session:{}) response:{:?}", node_id, session, flood))
                    .collect::<Vec<String>>()
                    .join("\n");
                ui.label(lost_flood_resp);
                let lost_ack = sim_log.lost_ack.clone()
                    .into_iter()
                    .map(|((node_id, session), ack)| format!("Lost Ack, (drone:{:?} session:{}) ack:{:?}", node_id, session, ack))
                    .collect::<Vec<String>>()
                    .join("\n");
                ui.label(lost_ack);
                let lost_flood_resp = sim_log.lost_flood_req.clone()
                    .into_iter()
                    .map(|((node_id, session), flood_req) | format!("Lost FloodReq, (drone:{:?} session:{}): request:{:?}", node_id, session,flood_req ))
                    .collect::<Vec<String>>()
                    .join("\n");
                ui.label(lost_flood_resp);


            }
            if ui.button("Close Window").clicked(){
                open.advanced_logs=false;
            }
        });
    }
}
// Helper function to render the graph visualization
fn render_graph_visualization(
    painter: &egui::Painter,
    rect: &egui::Rect,
    all_nodes: Vec<NodeId>,
    connections: Vec<(NodeId, NodeId)>
) {
    let mut node_positions = HashMap::new();

    if !all_nodes.is_empty() {
        let center_x = rect.center().x;
        let center_y = rect.center().y;
        let radius = rect.height() * 0.4;

        for (i, node_id) in all_nodes.iter().enumerate() {
            let angle = 2.0 * std::f32::consts::PI * (i as f32) / (all_nodes.len() as f32);
            let x = center_x + radius * angle.cos();
            let y = center_y + radius * angle.sin();
            node_positions.insert(*node_id, egui::pos2(x, y));
        }

        // Draw connections
        for (source, target) in &connections {
            if let (Some(start_pos), Some(end_pos)) = (node_positions.get(source), node_positions.get(target)) {
                painter.line_segment(
                    [*start_pos, *end_pos],
                    egui::Stroke::new(1.0, egui::Color32::GRAY),
                );
            }
        }

        // Draw nodes
        for (node_id, pos) in &node_positions {
            painter.circle(
                *pos,
                15.0,
                egui::Color32::from_rgb(100, 100, 255),
                egui::Stroke::new(1.0, egui::Color32::BLACK)
            );
            painter.text(
                *pos,
                egui::Align2::CENTER_CENTER,
                format!("{}", node_id),
                egui::FontId::default(),
                egui::Color32::BLACK,
            );
        }
    }
}
#[derive(Resource, Default)]
struct LogInfo{
    selected_node: Option<(NodeId, NodeType)>,
    drop_rate: HashMap<NodeId, (u64,usize)>,
    show_graph: bool
}