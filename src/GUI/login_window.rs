use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::winit::WinitSettings;
use bevy_dev_tools::states::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use wg_2024::network::NodeId;
use crate::GUI::star_decagram::spawn_star_decagram;
use crate::GUI::double_chain::spawn_double_chain;
use crate::GUI::butterfly::spawn_butterfly;
use crate::GUI::tree::spawn_tree;
use crate::simulation_control::simulation_control::*;

#[derive(Component)]
struct InputText;

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
        .init_resource::<EventLog>()
        .init_state::<AppState>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(AppState::Menu), setup_menu)
        .add_systems(Update, (menu, listen_keyboard_input_events).run_if(in_state(AppState::Menu)))
        .add_systems(OnExit(AppState::Menu), cleanup_menu)
        .add_systems(OnEnter(AppState::InGame), setup_network)
        .add_systems(Update , (ui_settings,draw_connections).run_if(in_state(AppState::InGame)))


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
const CAMERA_TARGET: Vec3 = Vec3::ZERO;
#[derive(Resource, Deref, DerefMut)]
struct OriginalCameraTransform(Transform);

#[derive(Resource)]
struct MenuData {
    button_entity: Entity,
    text_field: Entity,
}
#[derive(Resource,Default,Debug,Clone)]
struct UserConfig(String);


const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

}

fn setup_menu(mut commands: Commands,asset_server: Res<AssetServer>)  {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");

    // We will store these entities temporarily so we can place them in our resource
    let mut button_entity = None;
    let mut text_field_entity = None;


    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,  // children laid out side by side
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..Default::default()
        })
        // within the closure, we spawn the button and text field as child entities
        .with_children(|parent| {
            // 1) Spawn the button
            let b = parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(150.0),
                        height: Val::Px(65.0),
                        // horizontally center child text
                        justify_content: JustifyContent::Center,
                        // vertically center child text
                        align_items: AlignItems::Center,
                        ..Default::default()
                    },
                    // background color for the button
                    BackgroundColor(NORMAL_BUTTON),
                ))
                // the text within the button
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Start"),
                        // Adjust styles as needed
                        TextFont {
                            font: font.clone().into(),
                            font_size: 33.0,
                            ..Default::default()

                        },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                })
                .id();
            button_entity = Some(b);

            // 2) Spawn the text field
            let t = parent
                .spawn(Node {
                    width: Val::Px(250.0),
                    height: Val::Px(65.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..Default::default()
                })
                .with_children(|parent| {
                    parent.spawn((
                        Text::new(""),
                        TextFont {
                            font: font.clone().into(),
                            font_size: 60.0,
                            ..Default::default()
                        },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                        InputText,
                    ));
                })
                .id();
            text_field_entity = Some(t);
        });

    // Unwrap the newly created entities from the closure
    let button_entity = button_entity.expect("Button entity not spawned!");
    let text_field = text_field_entity.expect("Text field entity not spawned!");

    // Insert them into a resource for later use
    commands.insert_resource(MenuData {
        button_entity,
        text_field,
    })
}

fn menu(
    mut next_state: ResMut<NextState<AppState>>,
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = PRESSED_BUTTON.into();
                next_state.set(AppState::InGame);
            }
            Interaction::Hovered => {
                *color = HOVERED_BUTTON.into();
            }
            Interaction::None => {
                *color = NORMAL_BUTTON.into();
            }
        }
    }
}


fn listen_keyboard_input_events(
    mut events: EventReader<KeyboardInput>,
    mut current_text: Local<String>,
    mut user_config : ResMut<UserConfig>,
    mut query: Query<&mut Text, With<InputText>>,

) {
    let mut text=match query.get_single_mut(){
        Ok(t)=>t,
        Err(_)=>return,
    };
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }


        match &event.logical_key {
            Key::Enter => {
                if !current_text.is_empty() {
                    println!("User typed: {}", *current_text);
                    user_config.0=current_text.clone();
                    current_text.clear();
                }
            }

            Key::Backspace => {
                current_text.pop();
                text.0.pop();
            }

            Key::Character(str) => {
                current_text.push_str(str.as_str());
                text.0.push_str(str.as_str());
            }

            _ => {}
        }
    }
}

fn cleanup_menu(mut commands: Commands, menu_data: Res<MenuData>,query: Query<Entity, With<Camera2d>>) {
    commands.entity(menu_data.button_entity).despawn_recursive();
    commands.entity(menu_data.text_field).despawn_recursive();
    for camera_entity in &query{
        commands.entity(camera_entity).despawn_recursive();
    }
}

fn setup_network(
    mut commands: Commands,
    user_config: Res<UserConfig>,
    mut nodes_config: ResMut<NodesConfig>

) {

    match (*user_config).0.as_str(){
        "star"=>{
            let nodes= spawn_star_decagram(&mut commands);
            (*nodes_config).0=nodes;
        },
        "double_chain"=>{
            let nodes=spawn_double_chain(&mut commands);
            (*nodes_config).0=nodes;
        },
        "butterfly"=>{
            let nodes= spawn_butterfly(&mut commands);
            (*nodes_config).0=nodes;
        },
        "tree"=>{
            let nodes= spawn_tree(&mut commands);
            (*nodes_config).0=nodes;
        },
        _=> {
            let nodes = spawn_star_decagram(&mut commands);
            (*nodes_config).0=nodes;

        },
    }

    commands.spawn(Camera2d::default());
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
    mut is_last_selected: Local<bool>,
    mut contexts: EguiContexts,
    mut occupied_screen_space: ResMut<OccupiedScreenSpace>,
    mut nodes : ResMut<NodesConfig>,
    mut logs: Res<EventLog>
) {
    test();

    if let Some(context)=contexts.try_ctx_mut() {
        let ctx = context;


        occupied_screen_space.left = egui::SidePanel::left("left_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.label("Simulation Commands");
                if ui
                    .add(egui::widgets::Button::new("Crash drone").selected(!*is_last_selected))
                    .clicked()
                {
                    *is_last_selected = false;
                    let mut crashed=nodes.0.iter_mut().position(|node| node.id==1).map(|index| nodes.0.remove(index));
                    if let Some(mut crash)=crashed{
                        crash.connected_node_ids.clear();
                    }

                }
                if ui
                    .add(egui::widgets::Button::new("Another button").selected(*is_last_selected))
                    .clicked()
                {
                    *is_last_selected = true;
                }
                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            })
            .response
            .rect
            .width();
        occupied_screen_space.right = egui::SidePanel::right("right_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.label("Simulation events");
                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
                ui.add(egui::widgets::Label::new(format!("{:?}",*logs)));
            })
            .response
            .rect
            .width();
    }
}




