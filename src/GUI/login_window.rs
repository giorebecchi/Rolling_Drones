use std::collections::{HashMap, HashSet};
use std::sync::{Mutex};
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
use crate::common_things::common::{ChatClientEvent, CommandChat};
use bevy_framepace::{FramepacePlugin, FramepaceSettings, Limiter};
use std::sync::{Arc};
use crate::GUI::chat_windows::ChatSystemPlugin;
use crate::GUI::shared_info_plugin::BackendBridgePlugin;


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
    Server,
    Client,
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
        .add_plugins(EguiPlugin)
        .init_resource::<OccupiedScreenSpace>()
        .init_resource::<AddedDrone>()
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
        .init_state::<AppState>()
        .add_event::<NewDroneSpawned>()
        .add_systems(Update, ui_settings)
        .add_systems(Startup, setup_camera)
        .add_systems(OnEnter(AppState::InGame), (start_simulation,setup_network))
        .add_systems(Update, recompute_network.run_if(in_state(AppState::InGame)))
        .add_systems(Update , (draw_connections,set_up_bundle).run_if(in_state(AppState::InGame)))


        .run();
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum AppState {
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
            let nodes= spawn_star_decagram(None);
            println!("Nodes {:?}",nodes);
            (*nodes_config).0=nodes;
        },
        "double_chain"=>{
            let nodes=spawn_double_chain(None);
            (*nodes_config).0=nodes;
        },
        "butterfly"=>{
            let nodes= spawn_butterfly(None);
            (*nodes_config).0=nodes;
        },
        _=> {
            let nodes = spawn_star_decagram(None);
            (*nodes_config).0=nodes;

        },
    }

}
fn recompute_network(
    mut event_reader: EventReader<NewDroneSpawned>,
    user_config : Res<UserConfig>,
    mut nodes_config: ResMut<NodesConfig>,
    mut added_drone: ResMut<AddedDrone>
){
    for new_drone in event_reader.read(){
        added_drone.drone=new_drone.drone.clone();
        match user_config.0.as_str(){
            "star"=>{
                let nodes= spawn_star_decagram(Some(added_drone.clone()));
                println!("Nodes {:?}",nodes);
                (*nodes_config).0=nodes;
            },
            "double_chain"=>{
                let nodes=spawn_double_chain(Some(added_drone.clone()));
                (*nodes_config).0=nodes;
            },
            "butterfly"=>{
                let nodes= spawn_butterfly(Some(added_drone.clone()));
                (*nodes_config).0=nodes;
            },
            _=> {
                let nodes = spawn_star_decagram(Some(added_drone.clone()));
                (*nodes_config).0=nodes;

            }
        }
    }
}
#[derive(Component)]
pub struct Clickable {
    pub name: NodeId,
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
fn parse_id(id: String)->NodeId{
    match id.parse::<u8>(){
        Ok(node_id)=>node_id,
        Err(_)=>{
            eprintln!("Error occured while parsing");
            27
        }
    }
}




