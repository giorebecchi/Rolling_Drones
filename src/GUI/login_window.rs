use bevy::prelude::*;
use bevy::winit::WinitSettings;
use bevy_dev_tools::states::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_egui::egui::menu;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use crate::GUI::star_decagram::spawn_star_decagram;
use crate::GUI::double_chain::spawn_double_chain;
use crate::GUI::butterfly::spawn_butterfly;
use crate::GUI::tree::spawn_tree;
use crate::simulation_control::simulation_control::*;
use egui::widgets::TextEdit;
use wg_2024::packet::Packet;
use wg_2024::packet::PacketType::FloodRequest;

#[derive(Component)]
struct InputText;

#[derive(Resource, Default)]
struct DroneCrash{
    content: String
}

#[derive(Default,Debug,Clone)]
pub enum NodeType{
    #[default]
    Drone,
    Server,
    Client,
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
    App::new()
        .insert_resource(WinitSettings::desktop_app())
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .init_resource::<OccupiedScreenSpace>()
        .init_resource::<UserConfig>()
        .init_resource::<NodesConfig>()
        .init_resource::<DroneCrash>()
        .init_resource::<SimulationController>()
        .init_state::<AppState>()
        .add_systems(Update, ui_settings)
        .add_systems(OnEnter(AppState::InGame), (setup_network,test))
        .add_systems(Update , (draw_connections,set_up_bundle).run_if(in_state(AppState::InGame)))


        .add_systems(Update, log_transitions::<AppState>)
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


fn setup_network(
    mut commands: Commands,
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

    commands.spawn(Camera2d::default());
}
pub fn set_up_bundle(
    node_data: Res<NodesConfig>,
    mut commands: Commands,
    asset_server: Res<AssetServer>
) {
    for node_data in node_data.0.iter() {



        commands.spawn((
            Sprite {
                image: asset_server.load("images/Rolling_Drone.png"),
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
                Transform::from_translation(Vec3::new(-20.,-20.,0.))

            ));
        });

    }



}
pub fn draw_connections(
    mut gizmos : Gizmos,
    node_data: Res<NodesConfig>,
) {
    for node in &node_data.0 {
        for &connected_id in &node.connected_node_ids {
            if let Some(connected_node) = node_data.0.iter().find(|n| n.id == connected_id) {

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
    mut sim : ResMut<SimulationController>,
    mut drone_crash: ResMut<DroneCrash>,
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
                        }
                    });
                });
                ui.label("Simulation Commands");
                if ui
                    .add(egui::widgets::Button::new("Crash drone"))
                    .clicked()
                {
                    let id=match drone_crash.content.clone().parse::<u8>(){
                        Ok(x)=>x,
                        Err(_)=>panic!("Unexpected input in the Crash field"),
                    };
                    sim.crash(id);


                    let mut crashed=nodes.0.iter_mut().position(|node| node.id==id).map(|index| nodes.0.remove(index));
                    if let Some(mut crash)=crashed{
                        crash.connected_node_ids.clear();
                    }

                }

                ui.add(TextEdit::singleline(&mut (*drone_crash).content));
                if ui
                    .add(egui::widgets::Button::new("Flood"))
                    .clicked()
                {
                    sim.initiate_flood(Packet{
                        routing_header: SourceRoutingHeader{
                            hop_index:0,
                            hops: vec![1],
                        },
                        pack_type: FloodRequest(wg_2024::packet::FloodRequest{
                            flood_id: 10,
                            initiator_id: 0,
                            path_trace: vec![(0, wg_2024::packet::NodeType::Client)],
                        }),
                        session_id: 20,
                    });

                }

                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            })
            .response
            .rect
            .width();
        occupied_screen_space.right = egui::SidePanel::right("right_panel")
            .resizable(true)
            .show(ctx, |ui| {
                if sim.access {
                    ui.label(format!("{}", sim.log));
                }
                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            })
            .response
            .rect
            .width();
    }
}




