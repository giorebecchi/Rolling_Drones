// use bevy::asset::AssetServer;
// use bevy::color::Color;
// use bevy::input::keyboard::{Key, KeyboardInput};
// use bevy::prelude::{AlignItems, BackgroundColor, Button, Camera2d, Changed, Commands, Entity, EventReader, FlexDirection, Interaction, JustifyContent, Local, NextState, Node, Query, Res, ResMut, Text, TextColor, TextFont, Val, With};
// use crate::GUI::login_window::UserConfig;
//
// fn setup_menu(mut commands: Commands, asset_server: Res<AssetServer>)  {
//     let font = asset_server.load("fonts/FiraSans-Bold.ttf");
//
//     // We will store these entities temporarily so we can place them in our resource
//     let mut button_entity = None;
//     let mut text_field_entity = None;
//
//
//     commands
//         .spawn(Node {
//             width: Val::Percent(100.0),
//             height: Val::Percent(100.0),
//             flex_direction: FlexDirection::Column,  // children laid out side by side
//             justify_content: JustifyContent::Center,
//             align_items: AlignItems::Center,
//             ..Default::default()
//         })
//         // within the closure, we spawn the button and text field as child entities
//         .with_children(|parent| {
//             // 1) Spawn the button
//             let b = parent
//                 .spawn((
//                     Button,
//                     Node {
//                         width: Val::Px(150.0),
//                         height: Val::Px(65.0),
//                         // horizontally center child text
//                         justify_content: JustifyContent::Center,
//                         // vertically center child text
//                         align_items: AlignItems::Center,
//                         ..Default::default()
//                     },
//                     // background color for the button
//                     BackgroundColor(crate::GUI::login_window::NORMAL_BUTTON),
//                 ))
//                 // the text within the button
//                 .with_children(|parent| {
//                     parent.spawn((
//                         Text::new("Start"),
//                         // Adjust styles as needed
//                         TextFont {
//                             font: font.clone().into(),
//                             font_size: 33.0,
//                             ..Default::default()
//
//                         },
//                         TextColor(Color::srgb(0.9, 0.9, 0.9)),
//                     ));
//                 })
//                 .id();
//             button_entity = Some(b);
//
//             // 2) Spawn the text field
//             let t = parent
//                 .spawn(Node {
//                     width: Val::Px(250.0),
//                     height: Val::Px(65.0),
//                     justify_content: JustifyContent::Center,
//                     align_items: AlignItems::Center,
//                     ..Default::default()
//                 })
//                 .with_children(|parent| {
//                     parent.spawn((
//                         Text::new(""),
//                         TextFont {
//                             font: font.clone().into(),
//                             font_size: 60.0,
//                             ..Default::default()
//                         },
//                         TextColor(Color::srgb(0.9, 0.9, 0.9)),
//                         crate::GUI::login_window::InputText,
//                     ));
//                 })
//                 .id();
//             text_field_entity = Some(t);
//         });
//
//     // Unwrap the newly created entities from the closure
//     let button_entity = button_entity.expect("Button entity not spawned!");
//     let text_field = text_field_entity.expect("Text field entity not spawned!");
//
//     // Insert them into a resource for later use
//     commands.insert_resource(crate::GUI::login_window::MenuData {
//         button_entity,
//         text_field,
//     })
// }
//
// fn menu(
//     mut next_state: ResMut<NextState<crate::GUI::login_window::AppState>>,
//     mut interaction_query: Query<
//         (&Interaction, &mut BackgroundColor),
//         (Changed<Interaction>, With<Button>),
//     >,
// ) {
//     for (interaction, mut color) in &mut interaction_query {
//         match *interaction {
//             Interaction::Pressed => {
//                 *color = crate::GUI::login_window::PRESSED_BUTTON.into();
//                 next_state.set(crate::GUI::login_window::AppState::InGame);
//             }
//             Interaction::Hovered => {
//                 *color = crate::GUI::login_window::HOVERED_BUTTON.into();
//             }
//             Interaction::None => {
//                 *color = crate::GUI::login_window::NORMAL_BUTTON.into();
//             }
//         }
//     }
// }
//
//
// fn listen_keyboard_input_events(
//     mut events: EventReader<KeyboardInput>,
//     mut current_text: Local<String>,
//     mut user_config : ResMut<UserConfig>,
//     mut query: Query<&mut Text, With<crate::GUI::login_window::InputText>>,
//
// ) {
//     let mut text=match query.get_single_mut(){
//         Ok(t)=>t,
//         Err(_)=>return,
//     };
//     for event in events.read() {
//         if !event.state.is_pressed() {
//             continue;
//         }
//
//
//         match &event.logical_key {
//             Key::Enter => {
//                 if !current_text.is_empty() {
//                     println!("User typed: {}", *current_text);
//                     user_config.0=current_text.clone();
//                     current_text.clear();
//                 }
//             }
//
//             Key::Backspace => {
//                 current_text.pop();
//                 text.0.pop();
//             }
//
//             Key::Character(str) => {
//                 current_text.push_str(str.as_str());
//                 text.0.push_str(str.as_str());
//             }
//
//             _ => {}
//         }
//     }
// }
//
// fn cleanup_menu(mut commands: Commands, menu_data: Res<crate::GUI::login_window::MenuData>, query: Query<Entity, With<Camera2d>>) {
//     commands.entity(menu_data.button_entity).despawn_recursive();
//     commands.entity(menu_data.text_field).despawn_recursive();
//     for camera_entity in &query{
//         commands.entity(camera_entity).despawn_recursive();
//     }
// }
