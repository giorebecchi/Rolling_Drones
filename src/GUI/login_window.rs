use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
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
use crate::GUI::tree::spawn_tree;
use crate::simulation_control::simulation_control::*;
use egui::widgets::TextEdit;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use crate::common_things::common::CommandChat;

#[derive(Component)]
struct InputText;
#[derive(Resource, Default)]
struct UiCommands{
    show_crash_window: bool,
    show_add_sender_window: bool,
    crash_drone: String,
    target_sender: String,
    sender_neighbours: String
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

#[derive(Default,Debug,Clone)]
pub enum NodeType{
    #[default]
    Drone,
    Server,
    Client,
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
    pub seen_floods: HashSet<(NodeId,u64,NodeId)>
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
        .init_state::<AppState>()
        .add_systems(Update, ui_settings)
        .add_systems(Startup, setup_camera)
        .add_systems(OnEnter(AppState::InGame), (setup_network,start_simulation))
        .add_systems(Update , (draw_connections,set_up_bundle).run_if(in_state(AppState::InGame)))

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
pub fn set_up_bundle(
    node_data: Res<NodesConfig>,
    mut commands: Commands,
    mut entity_vector: ResMut<NodeEntities>,
    asset_server: Res<AssetServer>
) {

    for node_data in node_data.0.iter() {

        let entity=commands.spawn((
            Sprite {
                image: match node_data.node_type{
                    NodeType::Drone=>asset_server.load("images/Rolling_Drone.png"),
                    NodeType::Client=>asset_server.load("images/client.png"),
                    NodeType::Server=>asset_server.load("images/server.png")
                },
                custom_size: Some(Vec2::new(45.,45.)),
                ..default()
            },
            Transform::from_xyz(node_data.position[0],node_data.position[1],0.)
        )).with_children(|parent|{
            parent.spawn((
                Text2d::new(format!("{}",node_data.id)),
                TextFont{
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
                            node_entities.0.clear();
                            next_state.set(AppState::Menu);
                        }
                    });
                    ui.menu_button("Simulation Commands", |ui| {
                        if ui.button("Crash Drone").clicked() {
                            simulation_commands.show_crash_window = true;
                        }
                        if ui.button("Add Sender").clicked {
                            simulation_commands.show_add_sender_window=true;
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

                                    match simulation_commands.crash_drone.parse::<u8>() {
                                        Ok(id) => {
                                            sim.crash(id);
                                            let mut crashed=nodes.0.iter_mut().position(|node| node.id==id).map(|index| nodes.0.remove(index));
                                            if let Some(mut crash)=crashed{
                                                crash.connected_node_ids.clear();
                                            }
                                        }
                                        Err(_) => {eprintln!("not a valid id")},
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
                                    let mut target_id=0;
                                    match simulation_commands.target_sender.parse::<u8>(){
                                        Ok(id)=>{
                                            target_id=id;
                                        },
                                        Err(_)=>println!("please insert a valid id for the target drone")
                                    }
                                    let possible_ids:Vec<String>=simulation_commands.sender_neighbours.split('-').map(String::from).collect();
                                    for possible_id in possible_ids{
                                        match possible_id.parse::<u8>(){
                                            Ok(id)=>{
                                                sim.add_sender(target_id,id);
                                                for mut node in &mut nodes.0{
                                                    if node.id==id{
                                                        node.connected_node_ids.push(target_id);
                                                    }
                                                }
                                            }
                                            Err(_)=>println!("please insert a valid id for the neighbour drones")
                                        }
                                    }

                                }
                                if ui.button("Exit").clicked{
                                    simulation_commands.show_add_sender_window=false;
                                }
                            });


                        });
                }
                if ui
                    .add(egui::widgets::Button::new("Add"))
                    .clicked(){}

                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
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




