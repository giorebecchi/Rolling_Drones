use bevy::asset::AssetServer;
use bevy::color::Color;
use bevy::prelude::{default, Camera2dBundle, Commands, Component, PositionType, Query, Res, Style, Text, TextBundle, TextStyle, Time, Val, With};


// Marker component for animated color-changing text
#[derive(Component)]
pub struct AnimatedText;

pub fn update_text_color(time: Res<Time>, mut query: Query<&mut Text, With<AnimatedText>>) {
    for mut text in &mut query {
        let seconds = time.elapsed_seconds();
        let color = Color::srgb(
            seconds.sin() * 0.5 + 0.5,
            seconds.cos() * 0.5 + 0.5,
            (seconds * 0.3).sin() * 0.5 + 0.5,
        );
        text.sections[0].style.color = color;
    }
}
pub fn title_setup(mut commands: Commands, asset_server: Res<AssetServer>){
    // Spawn a single 2D camera
    commands.spawn(Camera2dBundle::default());

    // Add animated text
    commands.spawn((
        TextBundle::from_section(
            "Rolling_Drones",
            TextStyle {
                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                font_size: 30.0,
                color: Color::WHITE,
            },
        )
            .with_style(Style {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(10.0),
                ..default()
            }),
        AnimatedText,
    ));

}


