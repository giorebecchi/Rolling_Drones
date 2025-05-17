use bevy::prelude::*;
use bevy_egui::EguiContexts;
use wg_2024::network::NodeId;
use crate::GUI::login_window::{AppState, DisplayableLog, NodeConfig, NodeType, NodesConfig, SimWindows};
use crate::simulation_control::simulation_control::MyNodeType;

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
    open: Res<SimWindows>
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
            ui.label("Client: ");
            let current_selected_client= match log_info.selected_client.clone(){
                Some((id,node_type))=>format!("{:?} :{}",node_type,id),
                None=>"Select Client".to_string()
            };
            egui::ComboBox::from_id_salt("msg_select")
                .selected_text(current_selected_client)
                .show_ui(ui, |ui| {
                    let clients: Vec<&NodeConfig> = nodes.0.iter()
                        .filter(|node| node.node_type == NodeType::ChatClient || node.node_type == NodeType::WebBrowser)
                        .collect();

                    for client in clients {
                        let selected = log_info.selected_client == Some((client.id, client.node_type.clone()));
                        if ui.selectable_label(selected,format!("{:?} {}", client.node_type, client.id)).clicked() {
                            log_info.selected_client=Some((client.id, client.node_type.clone()));
                        }
                    }
                    if log_info.selected_client.is_some(){

                    }
                });
        });
    }
}
#[derive(Resource, Default)]
struct LogInfo{
    selected_client: Option<(NodeId, NodeType)>
}