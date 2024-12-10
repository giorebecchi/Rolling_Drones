use bevy::prelude::*;
use crate::simulation_control::buttons::*;

#[derive(Component)]
pub struct DisplayText;

pub fn setup_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera
    commands.spawn(Camera2dBundle{
        camera: Camera {
            order: 2, // Set the priority explicitly to avoid ambiguity
            ..Default::default()
        },
        ..Default::default()
    });

    // Root UI node
    commands.spawn(NodeBundle {
        style: Style {
            justify_content: JustifyContent::FlexStart, // Align content to the left
            align_items: AlignItems::FlexStart,         // Align items to the top
            flex_direction: FlexDirection::Column,      // Vertical layout
            margin: UiRect{
                left: Val::Px(10.),
                top: Val::Px(150.),
                right: Val::Auto,
                bottom: Val::Auto,
            },         // Add padding/margin
            ..Default::default()
        },
        ..Default::default()
    }).with_children(|parent| {
        // Text field (initially empty)
        parent.spawn(TextBundle {
            style: Style {
                margin: UiRect {
                    left: Val::Px(10.0),   // Add margin to the left
                    top: Val::Px(10.0),    // Add margin to the top
                    ..Default::default()
                },
                ..Default::default()
            },
            text: Text {
                sections: vec![TextSection {
                    value: "".to_string(), // Initially empty
                    style: TextStyle {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 25.0,
                        color: Color::WHITE,
                    },
                }],
                ..Default::default()
            },
            ..Default::default()
        }).insert(DisplayText);
    });
}


/// System to update the text when the crash event is triggered
pub fn update_text_on_crash(
    mut crash_events: EventReader<CrashEvent>,
    mut query: Query<&mut Text, With<DisplayText>>,
) {
    for _ in crash_events.read().by_ref() {
        if let Ok(mut text) = query.get_single_mut() {
            text.sections[0].value = "Drone Crashed".to_string();
        }
    }
}
