use std::collections::{HashMap, HashSet};
use std::sync::{Mutex};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::WinitSettings;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_egui::egui::menu;
use crossbeam_channel::{Receiver, Sender};
use wg_2024::network::{NodeId};
use crate::GUI::star_decagram::spawn_star_decagram;
use crate::GUI::double_chain::spawn_double_chain;
use crate::GUI::butterfly::spawn_butterfly;
use crate::GUI::tree::spawn_tree;
use crate::simulation_control::simulation_control::*;
use egui::widgets::TextEdit;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use crate::common_things::common::{ChatClientEvent, CommandChat};
use bevy_framepace::{FramepacePlugin, FramepaceSettings, Limiter};
use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock};

pub static SHARED_STATE: Lazy<Arc<RwLock<ThreadInfo>>> = Lazy::new(|| {
    Arc::new(RwLock::new(ThreadInfo::default()))
});


#[derive(Default)]
pub struct ThreadInfo {
    pub responses: HashMap<(NodeId,(NodeId,NodeId)),Vec<String>>,
    pub client_list: HashMap<(NodeId,NodeId), Vec<NodeId>>,
    pub registered_clients: HashMap<(NodeId,NodeId), bool>,
    pub is_updated: bool,

}


#[derive(Resource, Default)]
struct StateBridge;


struct BackendBridgePlugin;

impl Plugin for BackendBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateBridge>()
            .add_systems(Update, sync_backend_to_frontend);
    }
}


fn sync_backend_to_frontend(
    mut chat_state: ResMut<ChatState>,
) {

    if let Ok(state) = SHARED_STATE.try_read() {
        if state.is_updated {

            chat_state.chat_responses = state.responses.clone();
            // chat_state.client_list= state.client_list.clone();
            chat_state.registered_clients = state.registered_clients.clone();

            drop(state);

            if let Ok(mut state) = SHARED_STATE.try_write() {
                state.is_updated = false;
            }
        }
    }
}

#[derive(Component)]
struct InputText;
#[derive(Resource, Default)]
struct UiCommands{
    show_crash_window: bool,
    show_add_sender_window: bool,
    show_remove_sender_window: bool,
    show_set_pdr_window: bool,
    show_spawn_new_drone: bool,
    crash_drone: String,
    target_sender: String,
    sender_neighbours: String,
    target_remove: String,
    remove_neighbours: String,
    pdr: String,
    pdr_drone: String,
    new_drone_id: String,
    new_drone_links: String,
    show_client_windows: HashMap<NodeId, bool>
}
#[derive(Default)]
enum DroneBrand{
    #[default]
    LockHeedRustin,
    BagelBomber,
    FungiDrone,
    KrustyC,
    SkyLinkDrone,
    LeDroneJames,
    RustezeDrone,
    Rustafarian,
    RustDrone,
    RustBusterDrone
}
#[derive(Default)]
pub struct SharedSimState{
    pub log: String
}
#[derive(Resource,Default)]
pub struct SimState{
    pub state: Arc<Mutex<SharedSimState>>
}
#[derive(Resource, Default)]
struct NodeEntities(pub Vec<Entity>);

#[derive(Default,Debug,Clone,PartialEq)]
pub enum NodeType{
    #[default]
    Drone,
    Server,
    Client,
}
#[derive(Event)]
struct NewDroneSpawned;
#[derive(Clone,Resource)]
pub struct SimulationController {
    pub drones: HashMap<NodeId, Sender<DroneCommand>>,
    pub packet_channel: HashMap<NodeId, Sender<Packet>>,
    pub node_event_send: Sender<DroneEvent>,
    pub node_event_recv: Receiver<DroneEvent>,
    pub neighbours: HashMap<NodeId, Vec<NodeId>>,
    pub client : HashMap<NodeId, Sender<CommandChat>>,
    pub log: Arc<Mutex<SharedSimState>>,
    pub seen_floods: HashSet<(NodeId,u64,NodeId)>,
    pub client_list: HashMap<(NodeId, NodeId), Vec<NodeId>>,
    pub chat_event: Receiver<ChatClientEvent>,
    pub messages: HashMap<(NodeId,NodeId),Vec<String>>,
    pub incoming_message: HashMap<(NodeId,NodeId,NodeId), Vec<String>>,
    pub register_success: HashMap<(NodeId,NodeId),bool>
}

#[derive(Default,Debug,Clone)]
pub struct NodeConfig{
    pub node_type: NodeType,
    pub id: NodeId,
    pub position: Vec2,
    pub connected_node_ids: Vec<NodeId>,
}

impl NodeConfig {
    pub fn new(node_type: NodeType, id: NodeId, position: Vec2, connected_node_ids: Vec<NodeId>)->Self{
        Self{
            node_type,
            id,
            position,
            connected_node_ids,
        }
    }
}
#[derive(Resource,Default,Debug,Clone)]
pub struct NodesConfig(pub Vec<NodeConfig>);
#[derive(Resource, Default)]
struct ChatState {
    message_input: HashMap<NodeId, String>, // Each client has for every activ
    active_chat_node: HashMap<NodeId, Option<NodeId>>, // Each client's active
    active_chat_server: HashMap<NodeId, Option<NodeId>>, // Each client's acti
    // Tracking registered clients: (client_id, server_id) -> is_registered
    registered_clients: HashMap<(NodeId, NodeId), bool>,
    // Chat messages: (server_id, (sender_id, receiver_id)) -> [messages]
    chat_messages: HashMap<(NodeId, (NodeId, NodeId)), Vec<String>>,
    //Chat responses : (server_id, (receiver_id, sender_id)) -> [messages]
    chat_responses: HashMap<(NodeId, (NodeId, NodeId)), Vec<String>>
}


pub fn main() {
    let shared_state=Arc::new(Mutex::new(SharedSimState::default()));
    App::new()
        .insert_resource(WinitSettings::desktop_app())
        .add_plugins(DefaultPlugins.set(LogPlugin {
            level: bevy::log::Level::ERROR,
            filter: "".to_string(),
            ..default()
        }))
        .add_plugins(BackendBridgePlugin)
        .add_plugins(FramepacePlugin)
        .insert_resource(FramepaceSettings {
            limiter: Limiter::Auto,
        })
        .add_plugins(EguiPlugin)
        .init_resource::<OccupiedScreenSpace>()
        .init_resource::<UserConfig>()
        .init_resource::<NodesConfig>()
        .init_resource::<UiCommands>()
        .insert_resource(SimulationController{
            log: shared_state.clone(),
            ..default()
        })
        .init_resource::<NodeEntities>()
        .insert_resource(SimState{
            state: shared_state.clone(),
        })
        .init_resource::<OpenWindows>()
        .init_resource::<ChatState>()
        .init_state::<AppState>()
        .add_event::<NewDroneSpawned>()
        .add_systems(Update, ui_settings)
        .add_systems(Startup, setup_camera)
        .add_systems(OnEnter(AppState::InGame), (start_simulation,setup_network))
        .add_systems(Update , (draw_connections,set_up_bundle).run_if(in_state(AppState::InGame)))
        .add_systems(
            Update,
            (
                handle_clicks,
                display_windows
            )
                .run_if(in_state(AppState::InGame))
        )
        //.add_systems(Update , recompute_network)

        .run();
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum AppState {
    #[default]
    Menu,
    InGame,
}

#[derive(Default, Resource)]
struct OccupiedScreenSpace {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

#[derive(Resource,Default,Debug,Clone)]
pub struct UserConfig(pub String);

fn setup_camera(mut commands: Commands){
    commands.spawn(Camera2d::default());
}

fn setup_network(
    user_config: Res<UserConfig>,
    mut nodes_config: ResMut<NodesConfig>

) {

    match (*user_config).0.as_str(){
        "star"=>{
            let nodes= spawn_star_decagram();
            (*nodes_config).0=nodes;
        },
        "double_chain"=>{
            let nodes=spawn_double_chain();
            (*nodes_config).0=nodes;
        },
        "butterfly"=>{
            let nodes= spawn_butterfly();
            (*nodes_config).0=nodes;
        },
        "tree"=>{
            let nodes= spawn_tree();
            (*nodes_config).0=nodes;
        },
        _=> {
            let nodes = spawn_star_decagram();
            (*nodes_config).0=nodes;

        },
    }

}
fn recompute_network(
    mut event_reader: EventReader<NewDroneSpawned>,
    user_config : Res<UserConfig>,
    mut nodes_config: ResMut<NodesConfig>
){
    for _ in event_reader.read(){
        match user_config.0.as_str(){
            "star"=>{
                let nodes= spawn_star_decagram();
                (*nodes_config).0=nodes;
            },
            "double_chain"=>{
                let nodes=spawn_double_chain();
                (*nodes_config).0=nodes;
            },
            "butterfly"=>{
                let nodes= spawn_butterfly();
                (*nodes_config).0=nodes;
            },
            "tree"=>{
                let nodes= spawn_tree();
                (*nodes_config).0=nodes;
            },
            _=> {
                let nodes = spawn_star_decagram();
                (*nodes_config).0=nodes;

            }
        }
    }
}
#[derive(Component)]
struct Clickable {
    name: NodeId,
}
pub fn set_up_bundle(
    node_data: Res<NodesConfig>,
    mut commands: Commands,
    mut entity_vector: ResMut<NodeEntities>,
    asset_server: Res<AssetServer>
) {

    for node_data in node_data.0.iter() {

        if node_data.node_type == NodeType::Server || node_data.node_type == NodeType::Drone {
            let entity = commands.spawn((
                Sprite {
                    image: match node_data.node_type {
                        NodeType::Drone => asset_server.load("images/Rolling_Drone.png"),
                        NodeType::Client => unreachable!(),
                        NodeType::Server => asset_server.load("images/server.png")
                    },
                    custom_size: Some(Vec2::new(45., 45.)),
                    ..default()
                },
                Transform::from_xyz(node_data.position[0], node_data.position[1], 0.),
            )).with_children(|parent| {
                parent.spawn((
                    Text2d::new(format!("{}", node_data.id)),
                    TextFont {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 12.,
                        ..default()
                    },
                    TextColor(Color::srgb(1., 0., 0.)),
                    Transform::from_translation(Vec3::new(-30., -30., 0.))
                ));
            }).id();
            entity_vector.0.push(entity);
        }
        else{
            let entity=commands.spawn((
                Sprite {
                    image: asset_server.load("images/client.png"),
                    custom_size: Some(Vec2::new(45., 45.)),
                    ..default()
                },
                Transform::from_xyz(node_data.position[0], node_data.position[1], 0.),
                Clickable {
                    name: node_data.id,
                },
                )).with_children(|parent|{
                parent.spawn((
                    Text2d::new(format!("{}",node_data.id)),
                    TextFont {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 12.,
                        ..default()
                    },
                    TextColor(Color::srgb(1.,0.,0.)),
                    Transform::from_translation(Vec3::new(-30.,-30.,0.))
                    ));
            }).id();
            entity_vector.0.push(entity);
        }
    }


}
#[derive(Resource, Default)]
struct OpenWindows {
    windows: Vec<NodeId>,
}
fn handle_clicks(
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    clickable_entities: Query<(Entity, &Transform, &Sprite, &Clickable)>,
    mut open_windows: ResMut<OpenWindows>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let window = windows.single();
    let (camera, camera_transform) = camera_q.single();

    if let Some(cursor_position) = window.cursor_position() {
        if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) {
            let cursor_world_position = ray.origin.truncate();

            for (_entity, transform, sprite, clickable) in clickable_entities.iter() {
                let sprite_size = sprite.custom_size.unwrap_or(Vec2::ONE);
                let sprite_pos = transform.translation.truncate();

                let half_width = sprite_size.x / 2.0;
                let half_height = sprite_size.y / 2.0;

                if cursor_world_position.x >= sprite_pos.x - half_width
                    && cursor_world_position.x <= sprite_pos.x + half_width
                    && cursor_world_position.y >= sprite_pos.y - half_height
                    && cursor_world_position.y <= sprite_pos.y + half_height
                {
                    if !open_windows.windows.contains(&clickable.name) {
                        open_windows.windows.push(clickable.name.clone());
                        println!("Clicked on: {}", clickable.name);
                    }
                }
            }
        }
    }
}

fn display_windows(
    mut contexts: EguiContexts,
    mut open_windows: ResMut<OpenWindows>,
    mut sim: ResMut<SimulationController>,
    nodes: Res<NodesConfig>,
    mut chat_state: ResMut<ChatState>
) {
    let mut windows_to_close = Vec::new();

    for (i, &window_id) in open_windows.windows.iter().enumerate() {
        if !chat_state.message_input.contains_key(&window_id) {
            chat_state.message_input.insert(window_id, String::new());
        }
        if !chat_state.active_chat_node.contains_key(&window_id) {
            chat_state.active_chat_node.insert(window_id, None);
        }
        if !chat_state.active_chat_server.contains_key(&window_id) {
            chat_state.active_chat_server.insert(window_id, None);
        }

        let window = egui::Window::new(format!("Client: {}", window_id))
            .id(egui::Id::new(format!("window_{}", i)))
            .resizable(true)
            .collapsible(true)
            .default_size([400.0, 500.0]);

        let mut should_close = false;

        window.show(contexts.ctx_mut(), |ui| {
            ui.label(format!("This is a window for Client {}", window_id));
            ui.separator();
            ui.heading("Available Clients");
            let available_clients = nodes.0.iter()
                .filter(|node| node.node_type == NodeType::Client && node.id != window_id)
                .cloned()
                .collect::<Vec<NodeConfig>>();

            let active_server = chat_state.active_chat_server.get(&window_id).cloned().flatten();

            for client in available_clients {
                let is_registered = if let Some(server_id) = active_server {
                    chat_state.registered_clients.get(&(client.id, server_id))
                        .copied()
                        .unwrap_or(false)
                } else {
                    false
                };

                let button_text = format!("Chat with Client {} {}",
                                          client.id,
                                          if is_registered { "✓" } else { "" }
                );


                let button = ui.button(button_text);

                if button.clicked() {
                    if chat_state.active_chat_node.get(&window_id) == Some(&Some(client.id)) {
                        chat_state.active_chat_node.insert(window_id, None);
                    } else if is_registered {
                        chat_state.active_chat_node.insert(window_id, Some(client.id));
                    }
                }
            }

            // Chat display area
            ui.group(|ui| {
                let available_width=ui.available_width().min(370.0);
                ui.set_max_width(available_width);

                ui.vertical(|ui| {
                    // Get current chat partner
                    let chat_partner = chat_state.active_chat_node.get(&window_id).cloned().flatten();

                    ui.heading(
                        if let Some(partner_id) = chat_partner {
                            format!("Chat with Client {}", partner_id)
                        } else {
                            "Chat with None".to_string()
                        }
                    );

                    // Display chat messages in a scrollable area
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            if let (Some(partner_id), Some(server_id)) = (chat_partner, active_server) {
                                // Get full chat history between these clients
                                let messages = chat_state.chat_messages.get_mut(&(server_id,(window_id,partner_id)));
                                let messages=match messages{
                                    Some(m)=>{
                                        m.clone()
                                    },
                                    None=>Vec::new(),
                                };
                                let replies = chat_state.chat_responses.get_mut(&(server_id,(partner_id,window_id)));
                                let replies=match replies{
                                    Some(r)=>{
                                        r.clone()
                                    },
                                    None=>Vec::new(),
                                };


                                if !messages.is_empty() {
                                    // Sort messages by timestamp



                                    // Display all messages in order
                                    for msg in messages {
                                        ui.horizontal(|ui| {

                                            let text_width = available_width - 10.0;
                                            ui.set_max_width(text_width);
                                            ui.label(format!("You: {}", msg));
                                        });
                                    }

                                } else {
                                    ui.label("No messages yet. Start the conversation!");
                                }
                                if !replies.is_empty(){

                                    for reply in replies {
                                        ui.horizontal(|ui| {
                                            let text_width = available_width - 10.0;
                                            ui.set_max_width(text_width);
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                                ui.label(format!("Client {} : {}", partner_id, reply));
                                            });
                                        });
                                    }
                                }
                            }
                        });
                });

                ui.separator();

                // Input field should only be active if a chat partner is selected and both are registered
                let chat_partner = chat_state.active_chat_node.get(&window_id).cloned().flatten();
                let current_server = chat_state.active_chat_server.get(&window_id).cloned().flatten();

                let can_chat = if let (Some(partner_id), Some(server_id)) = (chat_partner, current_server) {
                    chat_state.registered_clients.get(&(window_id, server_id)).copied().unwrap_or(false) &&
                        chat_state.registered_clients.get(&(partner_id, server_id)).copied().unwrap_or(false)
                } else {
                    false
                };

                // Fixed: Handle message sending without multiple mutable borrows
                if can_chat {
                    // Copy current values before borrowing chat_state mutably again
                    let partner_id = chat_partner.unwrap();
                    let server_id = current_server.unwrap();

                    // Get a copy of the current input text
                    let current_input = chat_state.message_input.get(&window_id).cloned().unwrap_or_default();

                    // Message input area - only show if both clients are registered
                    let mut input_text = current_input;

                    let input_response = ui.add(
                        egui::TextEdit::singleline(&mut input_text)
                            .frame(true)
                            .hint_text("Type your message here...")
                            .desired_width(ui.available_width() - 80.0)
                    );

                    chat_state.message_input.insert(window_id, input_text.clone());

                    let send_button = ui.button("📨 Send");


                    if (send_button.clicked() ||
                        (input_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                        && !input_text.is_empty()
                    {

                        sim.send_message(
                            input_text.clone(),
                            window_id,
                            partner_id,
                            // server_id
                        );
                        if let Some(messages)=chat_state.chat_messages.get_mut(&(server_id,(window_id,partner_id))){
                            messages.push(input_text.clone());
                        }else{
                            let mut messages=Vec::new();
                            messages.push(input_text.clone());
                            chat_state.chat_messages.insert((server_id,(window_id,partner_id)),messages);
                        }
                        chat_state.message_input.insert(window_id, String::new());
                    }
                } else {
                    ui.add_enabled(false, egui::TextEdit::singleline(&mut String::new())
                        .hint_text("Select a registered client to chat")
                        .desired_width(ui.available_width() - 80.0));

                    ui.add_enabled(false, egui::Button::new("📨 Send"));
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Server: ");

                let current_server_text = match chat_state.active_chat_server.get(&window_id).cloned().flatten() {
                    Some(server_id) => format!("Server {}", server_id),
                    None => "Select a server".to_string()
                };

                egui::ComboBox::from_id_salt(format!("server_selector_{}", window_id))
                    .selected_text(current_server_text)
                    .show_ui(ui, |ui| {
                        let servers = nodes.0.iter()
                            .filter(|node| node.node_type == NodeType::Server)
                            .cloned()
                            .collect::<Vec<NodeConfig>>();

                        for server in servers {
                            let selected = chat_state.active_chat_server.get(&window_id) == Some(&Some(server.id));
                            if ui.selectable_label(selected, format!("Server {}", server.id)).clicked() {
                                if chat_state.active_chat_server.get(&window_id) == Some(&Some(server.id)) {
                                    chat_state.active_chat_server.insert(window_id, None);
                                } else {
                                    chat_state.active_chat_server.insert(window_id, Some(server.id));
                                }
                            }
                        }
                    });

                if ui.button("Register").clicked() {
                    if let Some(server_id) = chat_state.active_chat_server.get(&window_id).cloned().flatten() {
                        sim.register_client(window_id.clone(), server_id.clone());
                    }
                }
            });


            if let Some(server_id) = chat_state.active_chat_server.get(&window_id).cloned().flatten() {
                let is_registered = chat_state.registered_clients.get(&(window_id, server_id)).copied().unwrap_or(false);
                ui.label(format!(
                    "Status: {} to Server {}",
                    if is_registered { "Registered" } else { "Not Registered" },
                    server_id
                ));
            } else {
                ui.label("Status: No server selected");
            }

            ui.separator();
            if ui.button("Close Window").clicked() {
                should_close = true;
            }
        });

        if should_close {
            windows_to_close.push(i);
        }
    }

    for i in windows_to_close.into_iter().rev() {
        open_windows.windows.remove(i);
    }
}
pub fn draw_connections(
    mut gizmos : Gizmos,
    node_data: Res<NodesConfig>,
) {
    for node in &node_data.0 {
        for connected_id in &node.connected_node_ids {
            if let Some(connected_node) = node_data.0.iter().find(|n| n.id == *connected_id) {

                let start = node.position;
                let end = connected_node.position;
                gizmos.line_2d(start,end,Color::WHITE);

            }
        }
    }
}
fn ui_settings(
    mut contexts: EguiContexts,
    mut commands: Commands,
    mut occupied_screen_space: ResMut<OccupiedScreenSpace>,
    mut nodes : ResMut<NodesConfig>,
    mut topology : ResMut<UserConfig>,
    mut sim : ResMut<SimulationController>,
    mut sim_state: ResMut<SimState>,
    mut node_entities: ResMut<NodeEntities>,
    mut simulation_commands: ResMut<UiCommands>,
    mut next_state: ResMut<NextState<AppState>>,
    mut event_writer: EventWriter<NewDroneSpawned>
) {

    if let Some(context)=contexts.try_ctx_mut() {
        let ctx = context;

        let state = sim_state.state.lock().unwrap();
        occupied_screen_space.left = egui::SidePanel::left("left_panel")
            .resizable(true)
            .show(ctx, |ui| {
                menu::bar(ui, |ui| {
                    ui.menu_button("Topologies", |ui| {
                        if ui.button("Star").clicked() {
                            topology.0="star".to_string();
                            next_state.set(AppState::InGame);
                        }else if ui.button("Double chain").clicked(){
                            topology.0="double_chain".to_string();
                            next_state.set(AppState::InGame);
                        }else if ui.button("Tree").clicked(){
                            topology.0="tree".to_string();
                            next_state.set(AppState::InGame);
                        }else if ui.button("Butterfly").clicked(){
                            topology.0="butterfly".to_string();
                            next_state.set(AppState::InGame);
                        }else if ui.button("Reset").clicked(){
                            nodes.0=Vec::new();
                            for entity in node_entities.0.clone(){
                                commands.entity(entity).despawn_recursive();
                            }
                            sim.crash_all();
                            node_entities.0.clear();
                            next_state.set(AppState::Menu);
                        }
                    });
                    ui.menu_button("Simulation Commands", |ui| {
                        if ui.button("Crash Drone").clicked() {
                            simulation_commands.show_crash_window = true;
                        }
                        if ui.button("Add Sender").clicked() {
                            simulation_commands.show_add_sender_window=true;
                        }
                        if ui.button("Remove Sender").clicked(){
                            simulation_commands.show_remove_sender_window=true;
                        }
                        if ui.button("Set Pdr").clicked(){
                            simulation_commands.show_set_pdr_window=true;
                        }
                        if ui.button("Spawn New Drone").clicked(){
                            simulation_commands.show_spawn_new_drone=true;
                        }
                    });
                });
                if simulation_commands.show_crash_window {
                    egui::Window::new("Crash")
                        .open(&mut simulation_commands.show_crash_window.clone())
                        .show(ctx, |ui| {
                            ui.label("Choose which drone to crash");
                            ui.add(TextEdit::singleline(&mut (*simulation_commands).crash_drone));
                            ui.horizontal(|ui|{

                                if ui.button("Confirm").clicked() {
                                    let id=parse_id(simulation_commands.crash_drone.clone());

                                    sim.crash(id);
                                    let mut crashed=nodes.0.iter_mut().position(|node| node.id==id).map(|index| nodes.0.remove(index));
                                    if let Some(mut crash)=crashed{
                                        crash.connected_node_ids.clear();
                                    }

                                }


                                if ui.button("Exit").clicked(){
                                    simulation_commands.show_crash_window=false;
                                }
                            });
                        });
                }
                if simulation_commands.show_add_sender_window{
                    egui::Window::new("Add Sender")
                        .open(&mut simulation_commands.show_add_sender_window.clone())
                        .show(ctx, |ui|{
                            ui.label("Choose the drone to be added");
                            ui.add(TextEdit::singleline(&mut (*simulation_commands).target_sender));
                            ui.label("Insert all IDs with a '-' between them");
                            ui.add(TextEdit::singleline(&mut (*simulation_commands).sender_neighbours));
                            ui.horizontal(|ui|{

                                if ui.button("Confirm").clicked(){
                                    let target_id=parse_id(simulation_commands.target_sender.clone());
                                    let possible_ids:Vec<String>=simulation_commands.sender_neighbours.split('-').map(String::from).collect();
                                    for possible_id in possible_ids{

                                        let id=parse_id(possible_id.clone());

                                        sim.add_sender(target_id,id);
                                        for mut node in &mut nodes.0{
                                            if node.id==id{
                                                node.connected_node_ids.push(target_id);
                                            }
                                        }

                                    }

                                }
                                if ui.button("Exit").clicked(){
                                    simulation_commands.show_add_sender_window=false;
                                }
                            });


                        });
                }
                if simulation_commands.show_remove_sender_window{
                    egui::Window::new("Remove Sender")
                        .open(&mut simulation_commands.show_remove_sender_window.clone())
                        .show(ctx, |ui|{
                            ui.label("Insert the ID of the drone");
                            ui.add(TextEdit::singleline(&mut simulation_commands.target_remove));
                            ui.label("Insert the ID(s) of the neighbours you want to be removed\nwith a '-' between them");
                            ui.add(TextEdit::singleline(&mut simulation_commands.remove_neighbours));
                            ui.horizontal(|ui|{
                                if ui.button("Confirm")
                                    .clicked(){
                                    let target_id=parse_id(simulation_commands.target_remove.clone());
                                    let possible_ids:Vec<String>=simulation_commands.remove_neighbours.split('-').map(String::from).collect();
                                    for possible_id in possible_ids{
                                        let id=parse_id(possible_id);
                                        sim.remove_sender(target_id,id);
                                        for mut node in &mut nodes.0{
                                            if node.id==target_id{
                                                for i in 0..node.connected_node_ids.len(){
                                                    if node.connected_node_ids[i]==id{
                                                        node.connected_node_ids.remove(i);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if ui.button("Exit")
                                    .clicked(){
                                    simulation_commands.show_remove_sender_window=false;
                                }
                            })
                    });
                }
                if simulation_commands.show_set_pdr_window{
                    egui::Window::new("Change PDR")
                        .open(&mut simulation_commands.show_set_pdr_window.clone())
                        .show(ctx, |ui|{
                            ui.label("Insert the ID of the drone");
                            ui.add(TextEdit::singleline(&mut simulation_commands.pdr_drone));
                            ui.label("Insert the pdr as a float");
                            ui.add(TextEdit::singleline(&mut simulation_commands.pdr));
                            ui.horizontal(|ui| {
                                if ui.button("Confirm")
                                    .clicked() {
                                    let id = parse_id(simulation_commands.pdr_drone.clone());
                                    let pdr = simulation_commands.pdr.parse::<f32>().unwrap_or_else(|_| 0.);
                                    sim.pdr(id, pdr);
                                }
                                if ui.button("Exit")
                                    .clicked() {
                                    simulation_commands.show_set_pdr_window=false;
                                }
                            });
                        });
                }
                if simulation_commands.show_spawn_new_drone{
                    egui::Window::new("Spawn New Drone")
                        .open(&mut simulation_commands.show_spawn_new_drone.clone())
                        .show(ctx, |ui|{
                            ui.horizontal(|ui|{
                                //ui.label("Please choose which drone to spawn");
                                //menu::bar(ui, |ui|{
                                //    ui.menu_button("New Drone",|ui|{
                                //        if ui.button("LockHeedRustin")
                                //            .clicked(){
                                //            simulation_commands.type_new_drone=DroneBrand::LockHeedRustin;
//
//
                                //        }
                                //        else if ui.button("BagelBomber")
                                //            .clicked(){
                                //            simulation_commands.type_new_drone=DroneBrand::BagelBomber;
//
                                //        }
                                //        else if ui.button("FungiDrone")
                                //            .clicked(){
                                //            simulation_commands.type_new_drone=DroneBrand::FungiDrone;
//
                                //        }
                                //        else if ui.button("SkyLinkDrone")
                                //            .clicked(){
                                //            simulation_commands.type_new_drone=DroneBrand::SkyLinkDrone;
//
                                //        }
                                //        else if ui.button("Krusty_C")
                                //            .clicked(){
                                //            simulation_commands.type_new_drone=DroneBrand::KrustyC;
//
                                //        }
                                //        else if ui.button("RustDrone")
                                //            .clicked(){
                                //            simulation_commands.type_new_drone=DroneBrand::RustDrone;
//
                                //        }
                                //        else if ui.button("Rustafarian")
                                //            .clicked(){
                                //            simulation_commands.type_new_drone=DroneBrand::Rustafarian;
//
                                //        }
                                //        else if ui.button("RustBusters")
                                //            .clicked(){
                                //            simulation_commands.type_new_drone=DroneBrand::RustBusterDrone;
//
                                //        }
                                //        else if ui.button("LeDroneJames")
                                //            .clicked(){
                                //            simulation_commands.type_new_drone=DroneBrand::LeDroneJames;
//
                                //        }
                                //        else if ui.button("Rusteze")
                                //            .clicked(){
                                //            simulation_commands.type_new_drone=DroneBrand::RustezeDrone;
//
                                //        }
                                //    });
                                //})
                            });
                            ui.label("Insert the ID of the new drone");
                            ui.add(TextEdit::singleline(&mut simulation_commands.new_drone_id));
                            ui.label("Insert the connections of the new drone\nseparated by a '-'");
                            ui.add(TextEdit::singleline(&mut simulation_commands.new_drone_links));
                            ui.horizontal(|ui|{
                                if ui.button("Confirm")
                                    .clicked(){
                                    let mut links=Vec::new();
                                    let possible_ids:Vec<String>=simulation_commands.new_drone_links.split('-').map(String::from).collect();
                                    for id in possible_ids{
                                        let link=parse_id(id);
                                        links.push(link);
                                    }
                                    let new_id=parse_id(simulation_commands.new_drone_id.clone());
                                    for entity in node_entities.0.clone(){
                                        commands.entity(entity).despawn_recursive();
                                    }
                                    sim.spawn_new_drone(links, new_id);
                                    event_writer.send(NewDroneSpawned);

                                }
                                if ui.button("Exit")
                                    .clicked(){
                                    simulation_commands.show_spawn_new_drone=false;
                                }
                            });
                        });

                }


            })
            .response
            .rect
            .width();
        occupied_screen_space.right = egui::SidePanel::right("right_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.label("Simulation log");
                ui.label(&state.log);
                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            })
            .response
            .rect
            .width();
    }
}
//use std::fs::OpenOptions;
//use std::io::Write;
//fn spawn_new_drone(
//    neighbours: Vec<NodeId>,
//    new_drone_id: NodeId
//){
//    let string_to_append=format!("\n[[drone]]\nid = {}\nconnected_node_ids = {:?}\npdr = 0.00\n",new_drone_id, neighbours);
//    let mut file = OpenOptions::new().append(true).create(true).open("assets/configurations/double_chain.toml").unwrap();
//    file.write_all(string_to_append.as_bytes()).unwrap();
//}
fn parse_id(id: String)->NodeId{
    match id.parse::<u8>(){
        Ok(node_id)=>node_id,
        Err(_)=>{
            eprintln!("Error occured while parsing");
            27
        }
    }
}




