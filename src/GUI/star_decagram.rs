use bevy::prelude::*;

pub fn spawn_star_decagram(mut commands: &mut Commands) {
    let node_count = 10;
    let radius = 200.0;

    // Track node positions so we can draw lines between them
    let mut positions = Vec::with_capacity(node_count);

    // 1) Spawn 10 nodes around a circle
    for i in 0..node_count {
        let angle = i as f32 * std::f32::consts::TAU / node_count as f32;
        let x = radius * angle.cos();
        let y = radius * angle.sin();

        let sprite_color = Color::srgb(0.9,0.1,0.); // pick your color
        let sprite_size = Vec2::splat(15.0);


        commands.spawn(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(x, y, 0.0)),
            sprite: Sprite {
                color: sprite_color,
                custom_size: Some(sprite_size),
                ..default()
            },
            ..default()
        });

        positions.push(Vec2::new(x, y));
    }

}
