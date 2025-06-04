use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, RwLock};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::winit::WinitSettings;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_egui::egui::menu;
use crossbeam_channel::{Receiver, Sender};
use wg_2024::network::{NodeId};
use crate::gui::star_decagram::spawn_star_decagram;
use crate::gui::double_chain::spawn_double_chain;
use crate::gui::butterfly::spawn_butterfly;
use crate::simulation_control::simulation_control::*;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, Packet};
use std::fmt::Display;
use crate::common_things::common::{BackGroundFlood, ChatClientEvent, ClientType, CommandChat, ContentCommands, WebBrowserEvents, ServerCommands, ServerEvent};
use bevy_framepace::{FramepacePlugin, FramepaceSettings, Limiter};
use std::sync::{Arc};
use egui::{Color32, RichText};
use once_cell::sync::Lazy;
use petgraph::Graph;
use petgraph::prelude::UnGraphMap;
use crate::gui::chat_windows::ChatSystemPlugin;
use crate::gui::shared_info_plugin::{BackendBridgePlugin, SeenClients};
use crate::gui::web_media_plugin::WebMediaPlugin;
use crate::gui::advanced_logs_window::AdvancedLogsPlugin;
use crate::gui::simulation_commands::SimulationCommandsPlugin;
use crate::network_initializer::network_initializer::start_simulation;

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
#[derive(Resource, Default, Debug)]
pub struct NodeEntities(pub Vec<Entity>);

#[derive(Default,Debug,Clone,PartialEq)]
pub enum NodeType{
    #[default]
    Drone,
    TextServer,
    MediaServer,
    ChatServer,
    WebBrowser,
    ChatClient
}

#[derive(Clone,Resource)]
pub struct SimulationController {
    pub drones: HashMap<NodeId, Sender<DroneCommand>>,
    pub packet_channel: HashMap<NodeId, Sender<Packet>>,
    pub node_event_send: Sender<DroneEvent>,
    pub node_event_recv: Receiver<DroneEvent>,
    pub neighbours: HashMap<NodeId, Vec<NodeId>>,
    pub client : HashMap<NodeId, Sender<CommandChat>>,
    pub web_client : HashMap<NodeId, Sender<ContentCommands>>,
    pub text_server: HashMap<NodeId, Sender<ServerCommands>>,
    pub chat_server: HashMap<NodeId, Sender<ServerCommands>>,
    pub media_server: HashMap<NodeId, Sender<ServerCommands>>,
    pub seen_floods: HashSet<(NodeId,u64,NodeId)>,
    pub client_list: HashMap<(NodeId, NodeId), Vec<NodeId>>,
    pub chat_event: Receiver<ChatClientEvent>,
    pub web_event : Receiver<WebBrowserEvents>,
    pub server_event: Receiver<ServerEvent>,
    pub messages: HashMap<(NodeId,NodeId),Vec<String>>,
    pub incoming_message: HashMap<(NodeId,NodeId,NodeId), Vec<String>>,
    pub register_success: HashMap<(NodeId,NodeId),bool>,
    pub background_flooding: HashMap<NodeId, Sender<BackGroundFlood>>,
    pub chat_active: bool,
    pub web_active: bool,
}

#[derive(Default,Debug,Clone)]
pub struct NodeConfig{
    pub node_type: NodeType,
    pub id: NodeId,
    pub position: Vec2,
    pub connected_node_ids: Vec<NodeId>,
    pub pdr: f32,
}

impl NodeConfig {
    pub fn new(node_type: NodeType, id: NodeId, position: Vec2, connected_node_ids: Vec<NodeId>, pdr: f32)->Self{
        Self{
            node_type,
            id,
            position,
            connected_node_ids,
            pdr
        }
    }
}
#[derive(Resource,Default,Debug,Clone)]
pub struct NodesConfig(pub Vec<NodeConfig>);



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
        .add_plugins(AdvancedLogsPlugin)
        .add_plugins(FramepacePlugin)
        .add_plugins(ChatSystemPlugin)
        .add_plugins(SimulationCommandsPlugin)
        .insert_resource(FramepaceSettings {
            limiter: Limiter::Auto,
        })
        .add_plugins(WebMediaPlugin)
        .add_plugins(EguiPlugin)
        .init_resource::<OccupiedScreenSpace>()
        .init_resource::<UserConfig>()
        .init_resource::<NodesConfig>()
        .init_resource::<UiCommands>()
        .init_resource::<SimWindows>()
        .init_resource::<SimulationController>()
        .init_resource::<SimLog>()
        .init_resource::<DisplayableLog>()
        .init_resource::<NodeEntities>()
        .insert_resource(SimState{
            state: shared_state.clone(),
        })
        .init_state::<AppState>()
        .add_systems(Update, (ui_settings,sync_log))
        .add_systems(Startup, setup_camera)
        .add_systems(OnEnter(AppState::SetUp), start_simulation)
        .add_systems(OnEnter(AppState::InGame), (setup_network,initiate_flood))
        .add_systems(Update , (draw_connections,set_up_bundle).run_if(in_state(AppState::InGame)))

        .run();
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[default]
    Menu,
    SetUp,
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
    mut nodes_config: ResMut<NodesConfig>,
    mut seen_clients: ResMut<SeenClients>

) {

    match (*user_config).0.as_str(){
        "star"=>{

            let nodes= spawn_star_decagram(&mut seen_clients);
            (*nodes_config).0=nodes;
        },
        "double_chain"=>{
            let nodes=spawn_double_chain(&mut seen_clients);
            (*nodes_config).0=nodes;
        },
        "butterfly"=>{
            let nodes= spawn_butterfly(&mut seen_clients);
            (*nodes_config).0=nodes;
        },
        _=> {
            let nodes = spawn_star_decagram(&mut seen_clients);
            (*nodes_config).0=nodes;

        },
    }

}

#[derive(Component)]
pub struct Clickable {
    pub name: NodeId,
    pub window_type: ClientType
}
pub fn set_up_bundle(
    node_data: Res<NodesConfig>,
    mut commands: Commands,
    mut entity_vector: ResMut<NodeEntities>,
    asset_server: Res<AssetServer>
) {

    for node_data in node_data.0.iter() {

        if node_data.node_type == NodeType::Drone {
            let entity = commands.spawn((
                Sprite {
                    image: asset_server.load("images/Rolling_Drone.png"),
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
        } else if node_data.node_type==NodeType::ChatClient{
            let entity=commands.spawn((
                Sprite {
                    image: asset_server.load("images/client.png"),
                    custom_size: Some(Vec2::new(45., 45.)),
                    ..default()
                },
                Transform::from_xyz(node_data.position[0], node_data.position[1], 0.),
                Clickable {
                    name: node_data.id,
                    window_type: ClientType::ChatClient
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
        }else if node_data.node_type==NodeType::WebBrowser{
            let entity=commands.spawn((
                Sprite {
                    image: asset_server.load("images/web_browser.png"),
                    custom_size: Some(Vec2::new(45., 45.)),
                    ..default()
                },
                Transform::from_xyz(node_data.position[0], node_data.position[1], 0.),
                Clickable {
                    name: node_data.id,
                    window_type: ClientType::WebBrowser
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

        } else if node_data.node_type==NodeType::TextServer{
            let entity=commands.spawn((
                Sprite {
                    image: asset_server.load("images/server.png"),
                    custom_size: Some(Vec2::new(45., 45.)),
                    ..default()
                },
                Transform::from_xyz(node_data.position[0], node_data.position[1], 0.)

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

        }else if node_data.node_type==NodeType::MediaServer{
            let entity=commands.spawn((
                Sprite {
                    image: asset_server.load("images/mediaserver_icon.png"),
                    custom_size: Some(Vec2::new(45., 45.)),
                    ..default()
                },
                Transform::from_xyz(node_data.position[0], node_data.position[1], 0.)
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

        }else{
            let entity=commands.spawn((
                Sprite {
                    image: asset_server.load("images/chatserver_icon.png"),
                    custom_size: Some(Vec2::new(45., 45.)),
                    ..default()
                },
                Transform::from_xyz(node_data.position[0], node_data.position[1], 0.)
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
    mut occupied_screen_space: ResMut<OccupiedScreenSpace>,
    mut nodes : ResMut<NodesConfig>,
    mut topology : ResMut<UserConfig>,
    sim_log: Res<DisplayableLog>,
    mut sim_windows: ResMut<SimWindows>,
    mut next_state: ResMut<NextState<AppState>>
) {

    if let Some(context)=contexts.try_ctx_mut() {
        let ctx = context;

        occupied_screen_space.left = egui::SidePanel::left("left_panel")
            .resizable(true)
            .show(ctx, |ui| {
                menu::bar(ui, |ui| {
                    ui.menu_button("Topologies", |ui| {
                        if ui.button("Star").clicked() {
                            topology.0="star".to_string();
                            next_state.set(AppState::SetUp);
                        }else if ui.button("Double chain").clicked(){
                            topology.0="double_chain".to_string();
                            next_state.set(AppState::SetUp);
                        }else if ui.button("Butterfly").clicked(){
                            topology.0="butterfly".to_string();
                            next_state.set(AppState::SetUp);
                        }
                    });
                    if ui.button("Simulation Commands").clicked() {
                        sim_windows.simulation_commands = true;
                    }
                });




            })
            .response
            .rect
            .width();
        occupied_screen_space.right = {
            let mut collapsed = ctx.data_mut(|d| *d.get_persisted_mut_or_default::<bool>(egui::Id::new("right_panel_collapsed")));

            let panel = egui::SidePanel::right("right_panel")
                .resizable(true)
                .default_width(300.0)
                .min_width(if collapsed { 24.0 } else { 150.0 })
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(if collapsed { "show" } else { "collapse" }).clicked() {
                            collapsed = !collapsed;
                            ctx.data_mut(|d| d.insert_persisted(egui::Id::new("right_panel_collapsed"), collapsed));
                        }

                        if !collapsed {
                            ui.label("Simulation log");
                            if ui.button("Clear Log").clicked() {
                                clear_log();
                            }
                        }
                    });
                    let client_types: HashSet<MyNodeType> = HashSet::from([
                        MyNodeType::WebBrowser,
                        MyNodeType::ChatClient,
                    ]);

                    let mut client_log = String::with_capacity(1024);
                    let mut server_log = String::with_capacity(1024);


                    for ((node_type, _), node_content) in sim_log.flooding_log.iter() {
                        if client_types.contains(node_type) {
                            client_log.push_str(node_content);
                        } else {
                            server_log.push_str(node_content);
                        }
                    }

                    let node_map: HashMap<NodeId, NodeType> = nodes.0.iter()
                        .map(|node| (node.id.clone(), node.node_type.clone()))
                        .collect();

                    for ((id, _), node_content) in sim_log.msg_log.iter() {
                        if let Some(node_type) = node_map.get(id) {
                            if *node_type == NodeType::WebBrowser || *node_type == NodeType::ChatClient {
                                client_log.push_str(node_content);
                            } else {
                                server_log.push_str(node_content);
                            }
                        }
                    }


                    if !collapsed {
                        egui::ScrollArea::vertical()
                            .max_height(450.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui|{
                                    ui.label(RichText::new(client_log).color(Color32::ORANGE));
                                    ui.separator();
                                    ui.label(RichText::new(server_log).color(Color32::MAGENTA));
                                });

                            });
                        if ui.button("Advanced Logs").clicked(){
                            sim_windows.advanced_logs=true;
                        }
                    }

                    ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                });

            panel.response.rect.width()
        };
    }
}
fn parse_id(id: String)->NodeId{
    match id.parse::<u8>(){
        Ok(node_id)=>node_id,
        Err(_)=>{
            eprintln!("Error occured while parsing");
            27
        }
    }
}
#[derive(Resource,Default)]
pub struct SimWindows{
    pub advanced_logs: bool,
    pub simulation_commands: bool,
}
#[derive(Resource, Clone, Default)]
pub struct DisplayableLog{
    pub flooding_log: HashMap<(MyNodeType, NodeId), String>,
    pub msg_log: HashMap<(NodeId, u64), String>,
    pub lost_msg: HashMap<(NodeId, u64), Vec<Fragment>>,
    pub lost_ack: HashMap<(NodeId, u64), Vec<Ack>>,
    pub lost_flood_req: HashMap<(NodeId, u64), Vec<FloodRequest>>,
    pub lost_flood_resp: HashMap<(NodeId, u64), Vec<FloodResponse>>,
    pub lost_nack : HashMap<(NodeId, u64), Vec<Nack>>,
    pub route_attempt: HashMap<(NodeId,u64) , Vec<Vec<NodeId>>>,
    pub nack_log: HashMap<(MyNodeType, NodeId), String>,
    pub graph : HashMap<NodeId,UnGraphMap<NodeId, u32>>,
    pub server_graph : HashMap<NodeId, Graph<(NodeId,wg_2024::packet::NodeType), f64, petgraph::Directed>>,
}

#[derive(Resource, Default)]
pub struct SimLog{
    pub flooding_log: HashMap<(MyNodeType,NodeId), String>,
    pub msg_log: HashMap<(NodeId, u64), String>,
    pub lost_msg: HashMap<(NodeId, u64), Vec<Fragment>>,
    pub lost_ack: HashMap<(NodeId, u64), Vec<Ack>>,
    pub lost_flood_req: HashMap<(NodeId, u64), Vec<FloodRequest>>,
    pub lost_flood_resp: HashMap<(NodeId, u64), Vec<FloodResponse>>,
    pub lost_nack : HashMap<(NodeId, u64), Vec<Nack>>,
    pub route_attempt: HashMap<(NodeId,u64), Vec<Vec<NodeId>>>,
    pub nack_log: HashMap<(MyNodeType,NodeId), String>,
    pub graph : HashMap<NodeId,UnGraphMap<NodeId, u32>>,
    pub server_graph : HashMap<NodeId, Graph<(NodeId,wg_2024::packet::NodeType), f64, petgraph::Directed>>,
    pub is_updated: bool,
}
fn sync_log(
    mut displayable_log: ResMut<DisplayableLog>
){
    if let Ok(state)=SHARED_LOG.try_read(){
        if state.is_updated {
            displayable_log.flooding_log = state.flooding_log.clone();
            displayable_log.msg_log = state.msg_log.clone();
            displayable_log.lost_msg = state.lost_msg.clone();
            displayable_log.lost_ack = state.lost_ack.clone();
            displayable_log.lost_nack = state.lost_nack.clone();
            displayable_log.lost_flood_req = state.lost_flood_req.clone();
            displayable_log.lost_flood_resp = state.lost_flood_resp.clone();
            displayable_log.route_attempt = state.route_attempt.clone();
            displayable_log.nack_log=state.nack_log.clone();
            displayable_log.graph=state.graph.clone();
            displayable_log.server_graph=state.server_graph.clone();

            if let Ok(mut state) = SHARED_LOG.try_write() {
                state.is_updated = false;
            }
        }
    }
}
fn clear_log(){
    if let Ok(mut state)=SHARED_LOG.write(){
        state.flooding_log=HashMap::new();
        state.msg_log=HashMap::new();
        state.nack_log=HashMap::new();
        state.is_updated=true;
    }
}
fn initiate_flood(
    sim: Res<SimulationController>
){
    sim.initiate_flood();

}
pub static SHARED_LOG: Lazy<Arc<RwLock<SimLog>>>=Lazy::new (||{
    Arc::new(RwLock::new(SimLog::default()))
});



