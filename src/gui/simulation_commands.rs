use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui::{Color32, RichText};
use wg_2024::network::NodeId;
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
    selected_add_target: Option<NodeId>,
    selected_add_neighbor: Option<NodeId>,
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

        let ctx = contexts.ctx_mut();

        window.show(ctx, |ui| {
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
                                        for node in nodes.0.iter_mut() {
                                            node.connected_node_ids.retain(|&conn_id| conn_id != id);
                                        }
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
                            Some(id) => format!("{}", id),
                            None => "Select Node".to_string()
                        };
                        egui::ComboBox::from_id_salt("node_add_select")
                            .selected_text(current_selected_add_node)
                            .show_ui(ui, |ui| {
                                let nodes = nodes.0.clone();

                                for node in nodes {
                                    if !node.connected_node_ids.is_empty() {
                                        if let Some(id) = sim_commands.selected_add_target {
                                            if id != node.id {
                                                let selected = sim_commands.selected_add_neighbor == Some(node.id);
                                                if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                    sim_commands.selected_add_neighbor = Some(node.id);
                                                }
                                            }
                                        } else {
                                            let selected = sim_commands.selected_add_neighbor == Some(node.id);
                                            if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                sim_commands.selected_add_neighbor = Some(node.id);
                                            }
                                        }
                                    }
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        let current_selected_add_node2 = match sim_commands.selected_add_target.clone() {
                            Some(id) => format!("{}", id),
                            None => "Select Node".to_string()
                        };
                        egui::ComboBox::from_id_salt("node_add_2_select")
                            .selected_text(current_selected_add_node2)
                            .show_ui(ui, |ui| {
                                let nodes = nodes.0.clone();

                                for node in nodes {
                                    if !node.connected_node_ids.is_empty() {
                                        if let Some(id) = sim_commands.selected_add_neighbor {
                                            if id != node.id {
                                                let selected = sim_commands.selected_add_target == Some(node.id);
                                                if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                    sim_commands.selected_add_target = Some(node.id);
                                                }
                                            }
                                        } else {
                                            let selected = sim_commands.selected_add_target == Some(node.id);
                                            if ui.selectable_label(selected, format!("{:?} {}", node.node_type, node.id)).clicked() {
                                                sim_commands.selected_add_target = Some(node.id);
                                            }
                                        }
                                    }
                                }
                            });

                        if ui.button("Add Connection").clicked() {
                            if let (Some(from_id), Some(to_id)) =
                                (sim_commands.selected_add_neighbor, sim_commands.selected_add_target) {

                                sim.add_sender(to_id, from_id);
                                sim.initiate_flood();

                                if let Some(from_node) = nodes.0.iter_mut().find(|n| n.id == from_id) {
                                    if !from_node.connected_node_ids.contains(&to_id) {
                                        from_node.connected_node_ids.push(to_id);
                                    }
                                }
                                if let Some(to_node) = nodes.0.iter_mut().find(|n| n.id == to_id) {
                                    if !to_node.connected_node_ids.contains(&from_id) {
                                        to_node.connected_node_ids.push(from_id);
                                    }
                                }

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
                                                ).clicked(){
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

                                        if let Some(from_node) = nodes.0.iter_mut().find(|n| n.id == from_id) {
                                            from_node.connected_node_ids.retain(|&id| id != to_id);
                                        }
                                        if let Some(to_node) = nodes.0.iter_mut().find(|n| n.id == to_id) {
                                            to_node.connected_node_ids.retain(|&id| id != from_id);
                                        }

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

                        ui.label("PDR (0.0 - 0.99):");
                        ui.text_edit_singleline(&mut sim_commands.pdr_value);

                        if ui.button("Set PDR").clicked() {
                            if let Some(id) = sim_commands.selected_pdr_drone {
                                match sim_commands.pdr_value.parse::<f32>() {
                                    Ok(pdr) => {
                                        if (0.0..=0.99).contains(&pdr) {
                                            sim.pdr(id, pdr);
                                            for drone in nodes.0.iter_mut().filter(|node| node.id == id) {
                                                drone.pdr = pdr;
                                            }
                                            sim_commands.selected_pdr_drone = None;
                                            sim_commands.pdr_value.clear();
                                            sim_commands.pdr_error = None;
                                        } else {
                                            sim_commands.pdr_error = Some("PDR must be between 0.0 and 0.99".to_string());
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