use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui::{Color32, RichText};
use wg_2024::network::NodeId;
use crate::gui::highlighted_routes::ConnectionUpdateQueue;
use crate::gui::login_window::{AppState, NodeConfig, NodeType, NodesConfig, SimWindows};
use crate::network_initializer::connection_validity::{simulate_network_change, would_break_connectivity};
use crate::simulation_control::simulation_control::SimulationController;

pub struct SimulationCommandsPlugin;

impl Plugin for SimulationCommandsPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SimulationCommandsState>()
            .add_systems(Update, simulation_commands_window.run_if(in_state(AppState::InGame)));
    }
}

#[derive(Resource, Default)]
struct SimulationCommandsState {
    selected_crash_drone: Option<NodeId>,
    selected_add_target: Option<NodeConfig>,
    selected_add_neighbor: Option<NodeConfig>,
    selected_remove_target: Option<NodeId>,
    selected_remove_neighbor: Option<NodeId>,
    selected_pdr_drone: Option<NodeId>,
    pdr_value: String,
    pdr_error: Option<String>,
    connectivity_error: Option<String>,
}


fn simulation_commands_window(
    mut contexts: EguiContexts,
    mut nodes: ResMut<NodesConfig>,
    mut sim: ResMut<SimulationController>,
    mut connections: ResMut<ConnectionUpdateQueue>,
    mut sim_windows: ResMut<SimWindows>,
    mut sim_commands: ResMut<SimulationCommandsState>
) {
    if sim_windows.simulation_commands {
        let window_id = egui::Id::new("simulation_commands");
        let window = egui::Window::new("Simulation Commands")
            .id(window_id)
            .resizable(true)
            .collapsible(true)
            .default_size([450., 400.]);

        if let Some(contexts)=contexts.try_ctx_mut() {
            window.show(contexts, |ui| {
                ui.heading("Simulation Controls");

                if let Some(error) = &sim_commands.connectivity_error {
                    ui.colored_label(Color32::RED, error);
                    ui.separator();
                }

                ui.separator();
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.label("🔥 Crash Drone");
                        let current_selected_drone = match sim_commands.selected_crash_drone.clone() {
                            Some(id) => format!("Drone :{}", id),
                            None => "Select Drone".to_string()
                        };
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("drone_crash")
                                .selected_text(current_selected_drone)
                                .show_ui(ui, |ui| {
                                    let nodes: Vec<&NodeConfig> = nodes.0.iter()
                                        .filter(|node| node.node_type == NodeType::Drone)
                                        .collect();

                                    for node in nodes {
                                        let selected = sim_commands.selected_crash_drone == Some(node.id);
                                        if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                            sim_commands.selected_crash_drone = Some(node.id);
                                        }
                                    }
                                });

                            if ui.button("Crash").clicked() {
                                if let Some(id) = sim_commands.selected_crash_drone {
                                    let simulated = simulate_network_change(&nodes.0, |nodes| {
                                        if let Some(index) = nodes.iter().position(|node| node.id == id) {
                                            nodes.remove(index);
                                        }
                                        for node in nodes.iter_mut() {
                                            node.connected_node_ids.retain(|&conn_id| conn_id != id);
                                        }
                                    });

                                    match would_break_connectivity(&simulated) {
                                        Ok(_) => {
                                            sim.crash(id);
                                            if let Some(index) = nodes.0.iter().position(|node| node.id == id) {
                                                nodes.0.remove(index);
                                            }
                                            connections.remove_all_connections_for_node(id);
                                            sim_commands.selected_crash_drone = None;
                                            sim_commands.connectivity_error = None;
                                        }
                                        Err(e) => {
                                            sim_commands.connectivity_error = Some(format!("Cannot crash drone: {}", e));
                                        }
                                    }
                                }
                            }
                        });
                    });

                    ui.separator();

                    ui.group(|ui| {
                        ui.label("➕ Add Connection");
                        ui.horizontal(|ui| {
                            let current_selected_add_node = match sim_commands.selected_add_neighbor.clone() {
                                Some(add_neigh_node) => format!("{}", add_neigh_node.id),
                                None => "Select Node".to_string()
                            };
                            egui::ComboBox::from_id_salt("node_add_select")
                                .selected_text(current_selected_add_node)
                                .show_ui(ui, |ui| {
                                    let nodes = nodes.0.clone();

                                    for node in nodes {
                                        if let Some(add_target_node) = sim_commands.selected_add_target.clone() {
                                            if add_target_node.id != node.id {
                                                if add_target_node.node_type != NodeType::Drone {
                                                    if node.node_type == NodeType::Drone {
                                                        let selected = sim_commands.selected_add_neighbor == Some(node.clone());
                                                        if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                            sim_commands.selected_add_neighbor = Some(node);
                                                        }
                                                    }
                                                }else{
                                                    let selected = sim_commands.selected_add_neighbor == Some(node.clone());
                                                    if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                        sim_commands.selected_add_neighbor = Some(node);
                                                    }
                                                }
                                            }
                                        } else {
                                            let selected = sim_commands.selected_add_neighbor == Some(node.clone());
                                            if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                sim_commands.selected_add_neighbor = Some(node);
                                            }
                                        }
                                    }
                                });
                        });

                        ui.horizontal(|ui| {
                            let current_selected_add_node2 = match sim_commands.selected_add_target.clone() {
                                Some(add_target_node) => format!("{}", add_target_node.id),
                                None => "Select Node".to_string()
                            };
                            egui::ComboBox::from_id_salt("node_add_2_select")
                                .selected_text(current_selected_add_node2)
                                .show_ui(ui, |ui| {
                                    let nodes = nodes.0.clone();

                                    for node in nodes {
                                        if let Some(add_neigh_node) = sim_commands.selected_add_neighbor.clone() {
                                            if add_neigh_node.id != node.id {
                                                if add_neigh_node.node_type != NodeType::Drone {
                                                    if node.node_type == NodeType::Drone {
                                                        let selected = sim_commands.selected_add_target == Some(node.clone());
                                                        if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                            sim_commands.selected_add_target = Some(node);
                                                        }
                                                    }
                                                }else{
                                                    let selected = sim_commands.selected_add_target == Some(node.clone());
                                                    if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                        sim_commands.selected_add_target = Some(node);
                                                    }
                                                }
                                            }
                                        } else {
                                            let selected = sim_commands.selected_add_target == Some(node.clone());
                                            if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                sim_commands.selected_add_target = Some(node);
                                            }
                                        }
                                    }
                                });

                            if ui.button("Add Connection").clicked() {
                                if let (Some(from_node), Some(to_node)) =
                                    (sim_commands.selected_add_neighbor.clone(), sim_commands.selected_add_target.clone()) {
                                    sim.add_sender(to_node.id, from_node.id);
                                    sim.initiate_flood();

                                    connections.add_connection(from_node.id, to_node.id);

                                    sim_commands.selected_add_target = None;
                                    sim_commands.selected_add_neighbor = None;
                                    sim_commands.connectivity_error = None;
                                }
                            }
                        });
                    });

                    ui.separator();

                    ui.group(|ui| {
                        ui.label("➖ Remove Connection");
                        ui.horizontal(|ui| {
                            let current_selected_remove_node = match sim_commands.selected_remove_neighbor.clone() {
                                Some(id) => format!("{}", id),
                                None => "Select Node".to_string()
                            };
                            egui::ComboBox::from_id_salt("node_remove_select")
                                .selected_text(current_selected_remove_node)
                                .show_ui(ui, |ui| {
                                    let nodes = nodes.0.clone();

                                    for node in nodes {
                                        if !node.connected_node_ids.is_empty() {
                                            let selected = sim_commands.selected_remove_neighbor == Some(node.id);
                                            if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                sim_commands.selected_remove_neighbor = Some(node.id);
                                            }
                                        }
                                    }
                                });
                        });

                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("node_remove_2_select")
                                .selected_text(
                                    sim_commands.selected_remove_target
                                        .and_then(|id| nodes.0.iter().find(|n| n.id == id))
                                        .map(|node| format!("{:?} {}", node.node_type, node.id))
                                        .unwrap_or_else(|| "Choose destination...".to_string())
                                )
                                .show_ui(ui, |ui| {
                                    if let Some(from_id) = sim_commands.selected_remove_neighbor {
                                        if let Some(from_node) = nodes.0.iter().find(|n| n.id == from_id) {
                                            for &connected_id in &from_node.connected_node_ids {
                                                if let Some(connected_node) = nodes.0.iter().find(|n| n.id == connected_id) {
                                                    let selected = sim_commands.selected_remove_target == Some(connected_id.clone());
                                                    if ui.selectable_label(
                                                        selected,
                                                        format!("{:?} {}", connected_node.node_type, connected_id)
                                                    ).clicked() {
                                                        sim_commands.selected_remove_target = Some(connected_id.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });

                            if ui.button("Remove Connection").clicked() {
                                if let (Some(from_id), Some(to_id)) =
                                    (sim_commands.selected_remove_neighbor, sim_commands.selected_remove_target) {
                                    let simulated = simulate_network_change(&nodes.0, |nodes| {
                                        if let Some(from_node) = nodes.iter_mut().find(|n| n.id == from_id) {
                                            from_node.connected_node_ids.retain(|&id| id != to_id);
                                        }
                                        if let Some(to_node) = nodes.iter_mut().find(|n| n.id == to_id) {
                                            to_node.connected_node_ids.retain(|&id| id != from_id);
                                        }
                                    });

                                    match would_break_connectivity(&simulated) {
                                        Ok(_) => {
                                            sim.remove_sender(to_id, from_id);

                                            connections.remove_connection(from_id, to_id);

                                            sim_commands.selected_remove_target = None;
                                            sim_commands.selected_remove_neighbor = None;
                                            sim_commands.connectivity_error = None;
                                        }
                                        Err(e) => {
                                            sim_commands.connectivity_error = Some(format!("Cannot remove connection: {}", e));
                                        }
                                    }
                                }
                            }
                        });
                    });

                    ui.separator();

                    ui.group(|ui| {
                        ui.label("📊 Set Packet Drop Rate");
                        let current_selected_drone_pdr = match sim_commands.selected_pdr_drone.clone() {
                            Some(id) => format!("Drone :{}", id),
                            None => "Select Drone".to_string()
                        };
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("drone_pdr")
                                .selected_text(current_selected_drone_pdr)
                                .show_ui(ui, |ui| {
                                    let nodes: Vec<&NodeConfig> = nodes.0.iter()
                                        .filter(|node| node.node_type == NodeType::Drone)
                                        .collect();

                                    for node in nodes {
                                        let selected = sim_commands.selected_pdr_drone == Some(node.id);
                                        if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                            sim_commands.selected_pdr_drone = Some(node.id);
                                        }
                                    }
                                });

                            ui.label("PDR (0.0 - 1.00):");
                            ui.text_edit_singleline(&mut sim_commands.pdr_value);

                            if ui.button("Set PDR").clicked() {
                                if let Some(id) = sim_commands.selected_pdr_drone {
                                    match sim_commands.pdr_value.parse::<f32>() {
                                        Ok(pdr) => {
                                            if (0.0..=1.00).contains(&pdr) {
                                                if pdr==1.00 {
                                                    let simulated = simulate_network_change(&nodes.0, |nodes| {
                                                        if let Some(index) = nodes.iter().position(|node| node.id == id) {
                                                            nodes[index].pdr=1.00;
                                                        }
                                                    });
                                                    match would_break_connectivity(&simulated) {
                                                        Ok(_) => {
                                                            sim.pdr(id, pdr);
                                                            for drone in nodes.0.iter_mut().filter(|node| node.id == id) {
                                                                drone.pdr = pdr;
                                                            }
                                                            sim_commands.selected_pdr_drone = None;
                                                            sim_commands.pdr_value.clear();
                                                            sim_commands.pdr_error = None;
                                                        }
                                                        Err(_) => {
                                                            sim_commands.pdr_error=Some("Failed to set PDR to 1.00, this would break the network".to_string());
                                                        }
                                                    }
                                                }else{
                                                    sim.pdr(id, pdr);
                                                    for drone in nodes.0.iter_mut().filter(|node| node.id == id) {
                                                        drone.pdr = pdr;
                                                    }
                                                    sim_commands.selected_pdr_drone = None;
                                                    sim_commands.pdr_value.clear();
                                                    sim_commands.pdr_error = None;
                                                }
                                            } else {
                                                sim_commands.pdr_error = Some("PDR must be between 0.0 and 1.00".to_string());
                                            }
                                        }
                                        Err(_) => {
                                            sim_commands.pdr_error = Some("Invalid PDR value".to_string());
                                        }
                                    }
                                }
                            }
                        });

                        if let Some(error) = &sim_commands.pdr_error {
                            ui.label(RichText::new(error).color(Color32::RED));
                        }
                    });
                });

                ui.separator();

                if ui.button("Close Window").clicked() {
                    sim_windows.simulation_commands = false;
                }
            });
        }
    }
}