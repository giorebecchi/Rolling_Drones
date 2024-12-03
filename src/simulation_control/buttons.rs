use bevy::prelude::*;

/// Colors for button states
pub const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
pub const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
pub const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.75, 0.35);

/// Event to signal that the "Crash" button was clicked
#[derive(Event)]
pub struct CrashEvent;

pub fn button_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &Children),
        (Changed<Interaction>, With<Button>),
    >,
    mut text_query: Query<&mut Text>,
    mut crash_event_writer: EventWriter<CrashEvent>, // Event writer to send CrashEvent
) {
    for (interaction, mut color, children) in &mut interaction_query {
        let mut text = text_query.get_mut(children[0]).unwrap();
        match *interaction {
            Interaction::Pressed => {
                text.sections[0].value = ":(".to_string();
                *color = PRESSED_BUTTON.into();
                crash_event_writer.send(CrashEvent); // Trigger the crash event
            }
            Interaction::Hovered => {
                text.sections[0].value = "U sure?".to_string();
                *color = HOVERED_BUTTON.into();
            }
            Interaction::None => {
                text.sections[0].value = "Crash".to_string();
                *color = NORMAL_BUTTON.into();
            }
        }
    }
}

pub fn button_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Spawn a single button
    commands
        .spawn(ButtonBundle {
            style: Style {
                position_type: PositionType::Absolute,
                margin: UiRect {
                    left: Val::Auto,
                    right: Val::Px(10.0), // Distance from the right edge
                    top: Val::Auto,
                    bottom: Val::Px(10.0), // Distance from the bottom edge
                },
                justify_content: JustifyContent::Center, // Center the text inside the button
                align_items: AlignItems::Center,        // Align items in the button
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            // Add text to the button
            parent.spawn(TextBundle {
                text: Text::from_section(
                    "Crash",
                    TextStyle {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 33.0,
                        color: Color::WHITE,
                    },
                ),
                ..default()
            });
        });
}
