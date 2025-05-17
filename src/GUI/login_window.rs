use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, RwLock};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::winit::WinitSettings;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_egui::egui::menu;
use crossbeam_channel::{Receiver, Sender};
use wg_2024::network::{NodeId};
use crate::GUI::star_decagram::spawn_star_decagram;
use crate::GUI::double_chain::spawn_double_chain;
use crate::GUI::butterfly::spawn_butterfly;
use crate::simulation_control::simulation_control::*;
use egui::widgets::TextEdit;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use std::fmt::{Display, Formatter};
use crate::common_things::common::{BackGroundFlood, ChatClientEvent, ClientType, CommandChat, ContentCommands, WebBrowserEvents};
use bevy_framepace::{FramepacePlugin, FramepaceSettings, Limiter};
use std::sync::{Arc};
use egui::{Color32, RichText};
use once_cell::sync::Lazy;
use crate::GUI::chat_windows::ChatSystemPlugin;
use crate::GUI::shared_info_plugin::{BackendBridgePlugin, SeenClients};
use crate::GUI::web_media_plugin::WebMediaPlugin;

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
struct NodeEntities(pub Vec<Entity>);

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
#[derive(Event)]
pub struct NewDroneSpawned{
    pub drone: (Vec<NodeId>, NodeId)
}
#[derive(Resource,Clone,Default)]
pub struct AddedDrone{
    pub drone: (Vec<NodeId>,NodeId)
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
    pub text_servers: Vec<NodeId>,
    pub media_servers: Vec<NodeId>,
    pub chat_servers: Vec<NodeId>,
    pub seen_floods: HashSet<(NodeId,u64,NodeId)>,
    pub client_list: HashMap<(NodeId, NodeId), Vec<NodeId>>,
    pub chat_event: Receiver<ChatClientEvent>,
    pub web_event : Receiver<WebBrowserEvents>,
    pub messages: HashMap<(NodeId,NodeId),Vec<String>>,
    pub incoming_message: HashMap<(NodeId,NodeId,NodeId), Vec<String>>,
    pub register_success: HashMap<(NodeId,NodeId),bool>,
    pub background_flooding: HashMap<NodeId, Sender<BackGroundFlood>>
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
        .add_plugins(ChatSystemPlugin)
        .insert_resource(FramepaceSettings {
            limiter: Limiter::Auto,
        })
        .add_plugins(WebMediaPlugin)
        .add_plugins(EguiPlugin)
        .init_resource::<OccupiedScreenSpace>()
        .init_resource::<AddedDrone>()
        .init_resource::<UserConfig>()
        .init_resource::<NodesConfig>()
        .init_resource::<UiCommands>()
        .init_resource::<SimulationController>()
        .init_resource::<SimLog>()
        .init_resource::<DisplayableLog>()
        .init_resource::<NodeEntities>()
        .insert_resource(SimState{
            state: shared_state.clone(),
        })
        .init_state::<AppState>()
        .add_event::<NewDroneSpawned>()
        .add_systems(Update, (ui_settings,sync_log))
        .add_systems(Startup, setup_camera)
        .add_systems(OnEnter(AppState::SetUp), start_simulation)
        .add_systems(OnEnter(AppState::InGame), (setup_network, initiate_flood))
        .add_systems(Update, recompute_network.run_if(in_state(AppState::InGame)))
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

            let nodes= spawn_star_decagram(None,&mut seen_clients);
            (*nodes_config).0=nodes;
            println!("nodes :{:?}",nodes_config.clone());
        },
        "double_chain"=>{
            let nodes=spawn_double_chain(None,&mut seen_clients);
            (*nodes_config).0=nodes;
        },
        "butterfly"=>{
            let nodes= spawn_butterfly(None, &mut seen_clients);
            (*nodes_config).0=nodes;
        },
        _=> {
            let nodes = spawn_star_decagram(None, &mut seen_clients);
            (*nodes_config).0=nodes;

        },
    }

}
fn recompute_network(
    mut event_reader: EventReader<NewDroneSpawned>,
    user_config : Res<UserConfig>,
    mut nodes_config: ResMut<NodesConfig>,
    mut added_drone: ResMut<AddedDrone>,
    mut seen_clients: ResMut<SeenClients>
){
    for new_drone in event_reader.read(){
        added_drone.drone=new_drone.drone.clone();
        match user_config.0.as_str(){
            "star"=>{
                let nodes= spawn_star_decagram(Some(added_drone.clone()),&mut seen_clients);
                (*nodes_config).0=nodes;
            },
            "double_chain"=>{
                let nodes=spawn_double_chain(Some(added_drone.clone()),&mut seen_clients);
                (*nodes_config).0=nodes;
            },
            "butterfly"=>{
                let nodes= spawn_butterfly(Some(added_drone.clone()), &mut seen_clients);
                (*nodes_config).0=nodes;
            },
            _=> {
                let nodes = spawn_star_decagram(Some(added_drone.clone()),&mut seen_clients);
                (*nodes_config).0=nodes;

            }
        }
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
    mut commands: Commands,
    mut occupied_screen_space: ResMut<OccupiedScreenSpace>,
    mut nodes : ResMut<NodesConfig>,
    mut topology : ResMut<UserConfig>,
    mut sim : ResMut<SimulationController>,
    sim_log: Res<DisplayableLog>,
    mut node_entities: ResMut<NodeEntities>,
    mut simulation_commands: ResMut<UiCommands>,
    mut next_state: ResMut<NextState<AppState>>,
    mut event_writer: EventWriter<NewDroneSpawned>
) {

    if let Some(context)=contexts.try_ctx_mut() {
        let ctx = context;

        //let state = sim_state.state.lock().unwrap();
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
                                    node_entities.0.clear();
                                    sim.spawn_new_drone(links.clone(), new_id);
                                    event_writer.send(NewDroneSpawned{
                                        drone: (links,new_id)
                                    });

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
        occupied_screen_space.right = {
            // Store panel's collapsed state in the UI using a unique ID
            let mut collapsed = ctx.data_mut(|d| *d.get_persisted_mut_or_default::<bool>(egui::Id::new("right_panel_collapsed")));

            // Create a resizable right panel with a show/hide button
            let panel = egui::SidePanel::right("right_panel")
                .resizable(true)
                .default_width(300.0)
                .min_width(if collapsed { 24.0 } else { 150.0 })
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        // Toggle button with icon
                        if ui.button(if collapsed { "show" } else { "collapse" }).clicked() {
                            collapsed = !collapsed;
                            ctx.data_mut(|d| d.insert_persisted(egui::Id::new("right_panel_collapsed"), collapsed));
                        }

                        // Only show the panel title when expanded
                        if !collapsed {
                            ui.label("Simulation log");
                            if ui.button("Clear Log").clicked() {
                                clear_log();
                            }
                        }
                    });
                    let mut client_log = String::new();
                    let mut server_log = String::new();
                    for ((node_type, _), node_content) in sim_log.flooding_log.iter(){
                        match node_type{
                            MyNodeType::WebBrowser=> client_log.push_str(node_content),
                            MyNodeType::ChatClient=> client_log.push_str(node_content),
                            MyNodeType::TextServer=> server_log.push_str(node_content),
                            MyNodeType::MediaServer=> server_log.push_str(node_content),
                            MyNodeType::ChatServer=>server_log.push_str(node_content),
                        }
                    }

                    for (_, node_content) in sim_log.msg_log.iter(){
                        client_log.push_str(node_content);
                    }

                    // Only show content when not collapsed
                    if !collapsed {
                        egui::ScrollArea::vertical()
                            .show(ui, |ui| {
                                ui.horizontal(|ui|{
                                    ui.label(RichText::new(client_log).color(Color32::GREEN));
                                    ui.separator();
                                    ui.label(RichText::new(server_log).color(Color32::WHITE));
                                });
                            });
                    }

                    ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                });

            // Return the panel width
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
#[derive(Resource, Default)]
pub struct DisplayableLog{
    flooding_log: HashMap<(MyNodeType, NodeId), String>,
    msg_log: HashMap<(MyNodeType, NodeId), String>,
    nack_log: HashMap<(MyNodeType, NodeId), String>,
}

#[derive(Resource, Default)]
pub struct SimLog{
    pub flooding_log: HashMap<(MyNodeType,NodeId), String>,
    pub msg_log: HashMap<(MyNodeType,NodeId), String>,
    pub nack_log: HashMap<(MyNodeType,NodeId), String>,
    pub is_updated: bool,
}
fn sync_log(
    mut displayable_log: ResMut<DisplayableLog>
){
    if let Ok(state)=SHARED_LOG.try_read(){
        if state.is_updated {
            displayable_log.flooding_log = state.flooding_log.clone();
            displayable_log.msg_log = state.msg_log.clone();
            displayable_log.nack_log=state.nack_log.clone();

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



