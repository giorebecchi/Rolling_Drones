use bevy::prelude::*;
use crate::gui::login_window::AppState;
use crate::gui::shared_info_plugin::ErrorConfig;

pub struct ErrorMessagePlugin;

impl Plugin for ErrorMessagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ErrorQueue>()
            .init_resource::<ErrorAnimationTimer>()
            .add_systems(Update, error_system)
            .add_systems(Update, spawn_error_messages.run_if(in_state(AppState::SetUp)))
            .add_systems(Update, (
                animate_error_entrance,
                animate_error_pulse,
                auto_dismiss_error,
                handle_close_button_interaction,
            ));
    }
}

// Resource to queue error messages
#[derive(Resource, Default)]
pub struct ErrorQueue {
    message: String,
}

// Resource for animation timing
#[derive(Resource)]
struct ErrorAnimationTimer {
    entrance_timer: Timer,
    pulse_timer: Timer,
    dismiss_timer: Timer,
}

impl Default for ErrorAnimationTimer {
    fn default() -> Self {
        Self {
            entrance_timer: Timer::from_seconds(0.5, TimerMode::Once),
            pulse_timer: Timer::from_seconds(2.0, TimerMode::Repeating),
            dismiss_timer: Timer::from_seconds(5.0, TimerMode::Once),
        }
    }
}

// Component to mark error messages
#[derive(Component)]
struct ErrorMessage {
    spawn_time: f32,
    animation_progress: f32,
}

// Component for the error icon
#[derive(Component)]
struct ErrorIcon;

// Component for the close button
#[derive(Component)]
struct ErrorCloseButton;

// Spawn error messages from queue
fn spawn_error_messages(
    mut commands: Commands,
    mut error_queue: ResMut<ErrorQueue>,
    mut error: Res<ErrorConfig>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
) {
    if error.detected {
        // Main error container with rounded corners and shadow effect
        commands.spawn((
            Node {
                width: Val::Px(450.0),
                height: Val::Auto,
                min_height: Val::Px(120.0),
                max_height: Val::Px(300.0),
                position_type: PositionType::Absolute,
                right: Val::Percent(30.0),
                top: Val::Px(40.0),
                padding: UiRect::all(Val::Px(20.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgba(0.15, 0.12, 0.12, 0.95)),
            BorderColor(Color::srgba(0.8, 0.2, 0.2, 0.8)),
            BorderRadius::all(Val::Px(12.0)),
            Transform::from_translation(Vec3::new(-225.0, 0.0, 100.0)), // Higher z-index
            ErrorMessage {
                spawn_time: time.elapsed_secs(),
                animation_progress: 0.0,
            },
        )).with_children(|parent| {
            // Header row with icon and close button
            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(30.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            )).with_children(|header| {
                // Error icon and title
                header.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                )).with_children(|title_row| {
                    // Animated error icon
                    title_row.spawn((
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Px(24.0),
                            margin: UiRect::right(Val::Px(10.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 0.3, 0.3, 0.2)),
                        BorderRadius::all(Val::Px(12.0)),
                        ErrorIcon,
                    )).with_children(|icon| {
                        icon.spawn((
                            Text::new("!"),
                            TextFont {
                                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.4, 0.4)),
                        ));
                    });

                    // Error title
                    title_row.spawn((
                        Text::new("ERROR"),
                        TextFont {
                            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.4, 0.4)),
                    ));
                });

                // Close button
                header.spawn((
                    Button,
                    Node {
                        width: Val::Px(24.0),
                        height: Val::Px(24.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 0.3, 0.3, 0.1)),
                    BorderRadius::all(Val::Px(4.0)),
                    ErrorCloseButton,
                )).with_children(|close| {
                    close.spawn((
                        Text::new("×"),
                        TextFont {
                            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.8, 0.3, 0.3)),
                    ));
                });
            });

            // Separator line
            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    margin: UiRect::vertical(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.5, 0.2, 0.2, 0.3)),
            ));

            // Error message text
            parent.spawn((
                Text::new(error_queue.message.clone()),
                TextFont {
                    font: asset_server.load("fonts/FiraSans-Regular.ttf"),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                Node {
                    max_width: Val::Percent(100.0),
                    ..default()
                },
            ));
        });
    }
}

// Animate error message entrance
fn animate_error_entrance(
    mut query: Query<(&mut Transform, &mut ErrorMessage, &mut BackgroundColor), With<ErrorMessage>>,
    time: Res<Time>,
    mut timer: ResMut<ErrorAnimationTimer>,
) {
    timer.entrance_timer.tick(time.delta());

    for (mut transform, mut error, mut bg_color) in query.iter_mut() {
        if error.animation_progress < 1.0 {
            error.animation_progress = (error.animation_progress + time.delta_secs() * 2.0).min(1.0);

            // Slide in from top with bounce effect
            let progress = ease_out_back(error.animation_progress);
            transform.translation.y = -50.0 * (1.0 - progress);

            // Fade in
            bg_color.0.set_alpha(0.95 * progress);
        }
    }
}

// Animate error icon pulse
fn animate_error_pulse(
    mut icon_query: Query<&mut BackgroundColor, With<ErrorIcon>>,
    time: Res<Time>,
    mut timer: ResMut<ErrorAnimationTimer>,
) {
    timer.pulse_timer.tick(time.delta());

    let pulse_progress = (time.elapsed_secs() * 2.0).sin() * 0.5 + 0.5;

    for mut bg_color in icon_query.iter_mut() {
        let alpha = 0.2 + (0.3 * pulse_progress);
        bg_color.0 = Color::srgba(1.0, 0.3, 0.3, alpha);
    }
}

// Auto-dismiss error after timeout
fn auto_dismiss_error(
    mut commands: Commands,
    query: Query<(Entity, &ErrorMessage)>,
    time: Res<Time>,
    mut timer: ResMut<ErrorAnimationTimer>,
) {
    timer.dismiss_timer.tick(time.delta());

    for (entity, error) in query.iter() {
        if time.elapsed_secs() - error.spawn_time > 5.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}

// Easing function for smooth animation
fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;

    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

// Handle close button clicks
fn handle_close_button_interaction(
    mut commands: Commands,
    mut interaction_query: Query<
        (Entity, &Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<ErrorCloseButton>),
    >,
    parent_query: Query<&Parent>,
    error_query: Query<Entity, With<ErrorMessage>>,
) {
    for (button_entity, interaction, mut bg_color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                // Navigate up the hierarchy: Button -> Header -> ErrorMessage
                if let Ok(button_parent) = parent_query.get(button_entity) {
                    if let Ok(header_parent) = parent_query.get(button_parent.get()) {
                        let error_entity = header_parent.get();
                        if error_query.get(error_entity).is_ok() {
                            commands.entity(error_entity).despawn_recursive();
                        }
                    }
                }
            }
            Interaction::Hovered => {
                *bg_color = BackgroundColor(Color::srgba(1.0, 0.3, 0.3, 0.3));
            }
            Interaction::None => {
                *bg_color = BackgroundColor(Color::srgba(1.0, 0.3, 0.3, 0.1));
            }
        }
    }
}

pub fn error_system(
    mut error: Res<ErrorConfig>,
    mut error_to_display: ResMut<ErrorQueue>,
) {
    error_to_display.message = format!("{}{}{}", error.error_connection, error.error_isolated, error.error_pdr);
}