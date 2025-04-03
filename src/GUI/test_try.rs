// use bevy::prelude::*;
// use bevy_egui::{egui, EguiContexts, EguiPlugin};
// use std::collections::HashMap;
//
//
// type NodeId = u8;
//
//
// #[derive(Debug, Clone, PartialEq, Eq)]
// enum NodeType {
//     Client,
//     Server,
// }
//
// // Node configuration
// #[derive(Debug, Clone)]
// struct NodeConfig {
//     id: NodeId,
//     node_type: NodeType,
// }
//
// // All nodes available in the system
// #[derive(Debug, Default, Resource)]
// struct NodesConfig(Vec<NodeConfig>);
//
// // Tracking open windows
// #[derive(Debug, Default, Resource)]
// struct OpenWindows {
//     windows: Vec<NodeId>,
// }
//
// // Chat state management
// #[derive(Debug, Resource)]
// struct ChatState {
//     message_input: HashMap<NodeId, String>, // Each client has for every active_chat_server and for every dest_client its own input field
//     active_chat_node: HashMap<NodeId, Option<NodeId>>, // Each client's active chat partner for every server
//     active_chat_server: HashMap<NodeId, Option<NodeId>>, // Each client's active server
//
//     // Tracking registered clients: (client_id, server_id) -> is_registered
//     registered_clients: HashMap<(NodeId, NodeId), bool>,
//
//     // Chat messages: (server_id, (sender_id, receiver_id)) -> [messages]
//     chat_messages: HashMap<(NodeId, (NodeId, NodeId)), Vec<String>>,
//
//     //Chat responses : (server_id, (receiver_id, sender_id)) -> [messages]
//     chat_responses: HashMap<(NodeId, (NodeId, NodeId)), Vec<String>>
// }
//
// // Chat message structure to track sender and message content
//
//
// impl Default for ChatState {
//     fn default() -> Self {
//         Self {
//             message_input: HashMap::new(),
//             active_chat_node: HashMap::new(),
//             active_chat_server: HashMap::new(),
//             registered_clients: HashMap::new(),
//             chat_messages: HashMap::new(),
//             chat_responses: HashMap::new()
//         }
//     }
// }
//
// // Simulation controller to manage message passing and client registration
// #[derive(Debug, Default, Resource)]
// struct SimulationController {
//     time: f64,
// }
//
// impl SimulationController {
//     // Register a client to a server
//     fn register_client(&mut self, client_id: NodeId, server_id: NodeId) {
//         println!("Registering client {} to server {}", client_id, server_id);
//
//     }
//
//     // Send a message from one client to another
//     fn send_message(&mut self, message: String, sender_id: NodeId, receiver_id: NodeId, server_id: NodeId) {
//         println!("Sending message from {} to {} via server {}: {}",
//                  sender_id, receiver_id, server_id, message);
//     }
//
//
// }
//
// // Plugin for the chat system
// pub struct ChatSystemPlugin;
//
// impl Plugin for ChatSystemPlugin {
//     fn build(&self, app: &mut App) {
//         app.add_plugins(EguiPlugin)
//             .init_resource::<OpenWindows>()
//             .init_resource::<ChatState>()
//             .init_resource::<SimulationController>()
//             .init_resource::<NodesConfig>()
//             .add_systems(Startup, setup_nodes)
//             .add_systems(Update, display_windows);
//     }
// }
//
// // Setup initial nodes
// fn setup_nodes(mut commands: Commands) {
//     let mut nodes = Vec::new();
//
//     // Create server nodes
//     nodes.push(NodeConfig {
//         id: 1,
//         node_type: NodeType::Server,
//     });
//     nodes.push(NodeConfig {
//         id: 2,
//         node_type: NodeType::Server,
//     });
//
//     // Create client nodes
//     for i in 10..=15 {
//         nodes.push(NodeConfig {
//             id: i,
//             node_type: NodeType::Client,
//         });
//     }
//
//     commands.insert_resource(NodesConfig(nodes));
//
//     // Setup initial windows
//     let mut open_windows = OpenWindows::default();
//     open_windows.windows.push(10);
//     open_windows.windows.push(11);
//     open_windows.windows.push(12);
//     commands.insert_resource(open_windows);
// }
//
// // Display all client windows
// fn display_windows(
//     mut contexts: EguiContexts,
//     mut open_windows: ResMut<OpenWindows>,
//     mut sim: ResMut<SimulationController>,
//     nodes: Res<NodesConfig>,
//     mut chat_state: ResMut<ChatState>
// ) {
//     let mut windows_to_close = Vec::new();
//
//     for (i, &window_id) in open_windows.windows.iter().enumerate() {
//         if !chat_state.message_input.contains_key(&window_id) {
//             chat_state.message_input.insert(window_id, String::new());
//         }
//         if !chat_state.active_chat_node.contains_key(&window_id) {
//             chat_state.active_chat_node.insert(window_id, None);
//         }
//         if !chat_state.active_chat_server.contains_key(&window_id) {
//             chat_state.active_chat_server.insert(window_id, None);
//         }
//
//         let window = egui::Window::new(format!("Client: {}", window_id))
//             .id(egui::Id::new(format!("window_{}", i)))
//             .resizable(true)
//             .collapsible(true)
//             .default_size([400.0, 500.0]);
//
//         let mut should_close = false;
//
//         window.show(contexts.ctx_mut(), |ui| {
//             ui.label(format!("This is a window for Client {}", window_id));
//             ui.separator();
//             ui.heading("Available Clients");
//             let available_clients = nodes.0.iter()
//                 .filter(|node| node.node_type == NodeType::Client && node.id != window_id)
//                 .cloned()
//                 .collect::<Vec<NodeConfig>>();
//
//             let active_server = chat_state.active_chat_server.get(&window_id).cloned().flatten();
//
//             for client in available_clients {
//                 let is_registered = if let Some(server_id) = active_server {
//                     chat_state.registered_clients.get(&(client.id, server_id))
//                         .copied()
//                         .unwrap_or(false)
//                 } else {
//                     false
//                 };
//
//                 let button_text = format!("Chat with Client {} {}",
//                                           client.id,
//                                           if is_registered { "✓" } else { "" }
//                 );
//
//
//                 let button = ui.button(button_text);
//
//                 if button.clicked() {
//                     if chat_state.active_chat_node.get(&window_id) == Some(&Some(client.id)) {
//                         chat_state.active_chat_node.insert(window_id, None);
//                     } else if is_registered {
//                         chat_state.active_chat_node.insert(window_id, Some(client.id));
//                     }
//                 }
//             }
//
//             // Chat display area
//             ui.group(|ui| {
//                 let available_width=ui.available_width().min(370.0);
//                 ui.set_max_width(available_width);
//
//                 ui.vertical(|ui| {
//                     // Get current chat partner
//                     let chat_partner = chat_state.active_chat_node.get(&window_id).cloned().flatten();
//
//                     ui.heading(
//                         if let Some(partner_id) = chat_partner {
//                             format!("Chat with Client {}", partner_id)
//                         } else {
//                             "Chat with None".to_string()
//                         }
//                     );
//
//                     // Display chat messages in a scrollable area
//                     egui::ScrollArea::vertical()
//                         .max_height(200.0)
//                         .show(ui, |ui| {
//                             if let (Some(partner_id), Some(server_id)) = (chat_partner, active_server) {
//                                 // Get full chat history between these clients
//                                 let messages = chat_state.chat_messages.get_mut(&(server_id,(window_id,partner_id)));
//                                 let messages=match messages{
//                                     Some(m)=>{
//                                         m.clone()
//                                     },
//                                     None=>Vec::new(),
//                                 };
//                                 let replies = chat_state.chat_responses.get_mut(&(server_id,(partner_id,window_id)));
//                                 let replies=match replies{
//                                     Some(r)=>{
//                                         r.clone()
//                                     },
//                                     None=>Vec::new(),
//                                 };
//
//
//                                 if !messages.is_empty() {
//                                     // Sort messages by timestamp
//
//
//
//                                     // Display all messages in order
//                                     for msg in messages {
//                                         ui.horizontal(|ui| {
//
//                                             let text_width = available_width - 10.0;
//                                             ui.set_max_width(text_width);
//                                             ui.label(format!("You: {}", msg));
//                                         });
//                                     }
//
//                                 } else {
//                                     ui.label("No messages yet. Start the conversation!");
//                                 }
//                                 if !replies.is_empty(){
//
//                                     for reply in replies {
//                                         ui.horizontal(|ui| {
//                                             let text_width = available_width - 10.0;
//                                             ui.set_max_width(text_width);
//                                             ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
//                                                 ui.label(format!("Client {} : {}", partner_id, reply));
//                                             });
//                                         });
//                                     }
//                                 }
//                             }
//                         });
//                 });
//
//                 ui.separator();
//
//                 // Input field should only be active if a chat partner is selected and both are registered
//                 let chat_partner = chat_state.active_chat_node.get(&window_id).cloned().flatten();
//                 let current_server = chat_state.active_chat_server.get(&window_id).cloned().flatten();
//
//                 let can_chat = if let (Some(partner_id), Some(server_id)) = (chat_partner, current_server) {
//                     chat_state.registered_clients.get(&(window_id, server_id)).copied().unwrap_or(false) &&
//                         chat_state.registered_clients.get(&(partner_id, server_id)).copied().unwrap_or(false)
//                 } else {
//                     false
//                 };
//
//                 // Fixed: Handle message sending without multiple mutable borrows
//                 if can_chat {
//                     // Copy current values before borrowing chat_state mutably again
//                     let partner_id = chat_partner.unwrap();
//                     let server_id = current_server.unwrap();
//
//                     // Get a copy of the current input text
//                     let current_input = chat_state.message_input.get(&window_id).cloned().unwrap_or_default();
//
//                     // Message input area - only show if both clients are registered
//                     let mut input_text = current_input;
//
//                     let input_response = ui.add(
//                         egui::TextEdit::singleline(&mut input_text)
//                             .frame(true)
//                             .hint_text("Type your message here...")
//                             .desired_width(ui.available_width() - 80.0)
//                     );
//
//                     chat_state.message_input.insert(window_id, input_text.clone());
//
//                     let send_button = ui.button("📨 Send");
//
//
//                     if (send_button.clicked() ||
//                         (input_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
//                         && !input_text.is_empty()
//                     {
//
//                         sim.send_message(
//                             input_text.clone(),
//                             window_id,
//                             partner_id,
//                             server_id
//                         );
//                         if let Some(messages)=chat_state.chat_messages.get_mut(&(server_id,(window_id,partner_id))){
//                             messages.push(input_text.clone());
//                         }else{
//                             let mut messages=Vec::new();
//                             messages.push(input_text.clone());
//                             chat_state.chat_messages.insert((server_id,(window_id,partner_id)),messages);
//                         }
//                         if let Some(responses)=chat_state.chat_responses.get_mut(&(server_id,(window_id,partner_id))){
//                             responses.push(input_text.clone());
//                         }else{
//                             let mut responses=Vec::new();
//                             responses.push(input_text.clone());
//                             chat_state.chat_responses.insert((server_id,(window_id,partner_id)),responses);
//                         }
//
//
//                         chat_state.message_input.insert(window_id, String::new());
//                     }
//                 } else {
//                     ui.add_enabled(false, egui::TextEdit::singleline(&mut String::new())
//                         .hint_text("Select a registered client to chat")
//                         .desired_width(ui.available_width() - 80.0));
//
//                     ui.add_enabled(false, egui::Button::new("📨 Send"));
//                 }
//             });
//
//             ui.separator();
//
//             // Server selection and registration
//             ui.horizontal(|ui| {
//                 ui.label("Server: ");
//
//                 let current_server_text = match chat_state.active_chat_server.get(&window_id).cloned().flatten() {
//                     Some(server_id) => format!("Server {}", server_id),
//                     None => "Select a server".to_string()
//                 };
//
//                 egui::ComboBox::from_id_salt(format!("server_selector_{}", window_id))
//                     .selected_text(current_server_text)
//                     .show_ui(ui, |ui| {
//                         let servers = nodes.0.iter()
//                             .filter(|node| node.node_type == NodeType::Server)
//                             .cloned()
//                             .collect::<Vec<NodeConfig>>();
//
//                         for server in servers {
//                             let selected = chat_state.active_chat_server.get(&window_id) == Some(&Some(server.id));
//                             if ui.selectable_label(selected, format!("Server {}", server.id)).clicked() {
//                                 // Just select the server, but don't register yet
//                                 if chat_state.active_chat_server.get(&window_id) == Some(&Some(server.id)) {
//                                     // Deselect if already selected
//                                     chat_state.active_chat_server.insert(window_id, None);
//                                 } else {
//                                     chat_state.active_chat_server.insert(window_id, Some(server.id));
//                                 }
//                             }
//                         }
//                     });
//
//                 if ui.button("Register").clicked() {
//                     if let Some(server_id) = chat_state.active_chat_server.get(&window_id).cloned().flatten() {
//                         sim.register_client(window_id.clone(), server_id.clone());
//                         chat_state.registered_clients.insert((window_id,server_id),true);
//                     }
//                 }
//             });
//
//             // Show registration status
//             if let Some(server_id) = chat_state.active_chat_server.get(&window_id).cloned().flatten() {
//                 let is_registered = chat_state.registered_clients.get(&(window_id, server_id)).copied().unwrap_or(false);
//                 ui.label(format!(
//                     "Status: {} to Server {}",
//                     if is_registered { "Registered" } else { "Not Registered" },
//                     server_id
//                 ));
//             } else {
//                 ui.label("Status: No server selected");
//             }
//
//             // Window controls
//             ui.separator();
//             if ui.button("Close Window").clicked() {
//                 should_close = true;
//             }
//         });
//
//         if should_close {
//             windows_to_close.push(i);
//         }
//     }
//
//     for i in windows_to_close.into_iter().rev() {
//         open_windows.windows.remove(i);
//     }
// }
//
// pub fn main() {
//     App::new()
//         .add_plugins(DefaultPlugins)
//         .add_plugins(ChatSystemPlugin)
//         .run();
// }