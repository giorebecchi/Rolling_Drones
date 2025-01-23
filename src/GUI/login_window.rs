use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::winit::WinitSettings;
use bevy_dev_tools::states::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use crate::GUI::star_decagram::spawn_star_decagram;
use crate::GUI::double_chain::spawn_double_chain;
use crate::GUI::butterfly::spawn_butterfly;
use crate::GUI::tree::spawn_tree;

pub fn main() {
    App::new()
        .insert_resource(WinitSettings::desktop_app())
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .init_resource::<OccupiedScreenSpace>()
        .init_resource::<UserConfig>()
        .init_state::<AppState>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(AppState::Menu), setup_menu)
        .add_systems(Update, (menu, listen_keyboard_input_events).run_if(in_state(AppState::Menu)))
        .add_systems(OnExit(AppState::Menu), cleanup_menu)
        //.add_systems(OnEnter(AppState::InGame), spawn_star_decagram)
        .add_systems(OnEnter(AppState::InGame), setup_network)
        .add_systems(Update , ui_settings.run_if(in_state(AppState::InGame)))

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

fn setup_menu(mut commands: Commands,asset_server: Res<AssetServer>) {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");
    let button_entity = commands
        .spawn(Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(150.),
                        height: Val::Px(65.),
                        // horizontally center child text
                        justify_content: JustifyContent::Center,
                        // vertically center child text
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(NORMAL_BUTTON),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Start"),
                        TextFont {
                            font_size: 33.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                });
        })
        .id();



    let text_field = commands.spawn((
        Text2d::new(""),
        TextFont {
            font,
            font_size: 100.0,
            ..default()
        },
    ))
        .id();
    commands.insert_resource(MenuData { button_entity, text_field});
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
) {
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
            }

            Key::Character(str) => {
               current_text.push_str(str.as_str());
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

) {

    match (*user_config).0.as_str(){
        "star"=>spawn_star_decagram(&mut commands),
        "double_chain"=>spawn_double_chain(&mut commands),
        "butterfly"=>spawn_butterfly(&mut commands),
        "tree"=>spawn_tree(&mut commands),
        _=>spawn_star_decagram(&mut commands),
    }

    commands.spawn(Camera2d::default());
}
fn ui_settings(
    mut is_last_selected: Local<bool>,
    mut contexts: EguiContexts,
    mut occupied_screen_space: ResMut<OccupiedScreenSpace>,
) {

    if let Some(context)=contexts.try_ctx_mut() {
        let ctx = context;


        occupied_screen_space.left = egui::SidePanel::left("left_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.label("Left resizeable panel");
                if ui
                    .add(egui::widgets::Button::new("A button").selected(!*is_last_selected))
                    .clicked()
                {
                    *is_last_selected = false;
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
                ui.label("Right resizeable panel");
                ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            })
            .response
            .rect
            .width();
    }
}



