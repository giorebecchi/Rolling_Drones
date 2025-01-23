use bevy::prelude::*;

pub fn spawn_butterfly(mut commands: &mut Commands) {
    let horizontal_spacing=100.;
    let vertical_offset=50.;

    let mut top_position= Vec::with_capacity(4);
    let mut middle_position = Vec::with_capacity(4);
    let mut bottom_position= Vec::with_capacity(2);

    for i in 0..4{
        let x = (i as f32 - (4 - 1) as f32 / 2.0) * horizontal_spacing;
        let y = vertical_offset;

        set_up_bundle(x,y,&mut commands);

        top_position.push(Vec2::new(x, y));
    }
    for i in 0..4{
        let x = (i as f32 - (4 - 1) as f32 / 2.0) * horizontal_spacing;
        let y = -vertical_offset;

        set_up_bundle(x,y,&mut commands);

        middle_position.push(Vec2::new(x, y));
    }
    for i in 0..2{
        let x = (i as f32 - (2 - 1) as f32 / 2.0) * horizontal_spacing;
        let y = -(vertical_offset*3.0);

        set_up_bundle(x,y,&mut commands);

        bottom_position.push(Vec2::new(x, y));
    }
}
pub fn set_up_bundle(x: f32, y:f32,commands: &mut Commands){

    commands.spawn(SpriteBundle {
        transform: Transform::from_xyz(x, y, 0.0),
        sprite: Sprite {
            color: Color::srgb(0.9,0.1,0.),
            custom_size: Some(Vec2::splat(15.0)),
            ..default()
        },
        ..default()
    });
}