use bevy::prelude::*;
use crate::GUI::butterfly::set_up_bundle;

pub fn spawn_tree(mut commands: &mut Commands) {
    let horizontal_spacing=100.;
    let vertical_offset=50.;

    let mut root_position= Vec::with_capacity(1);
    let mut top_position = Vec::with_capacity(2);
    let mut middle_position= Vec::with_capacity(3);
    let mut bottom_position= Vec::with_capacity(4);

    let x = 0.;
    let y = vertical_offset;
    root_position.push(Vec2::new(x, y));

    set_up_bundle(0.,y,&mut commands);
    for i in 0..2{
        let x= (i as f32 - (2 - 1) as f32 / 2.0) * horizontal_spacing;
        let y = -vertical_offset;

        set_up_bundle(x,y,&mut commands);

        top_position.push(Vec2::new(x, y));
    }
    for i in 0..3{
        let x = (i as f32- (3 - 1) as f32 /2.0)*horizontal_spacing;
        let y= -(vertical_offset * 3.0);

        set_up_bundle(x,y,&mut commands);

        middle_position.push(Vec2::new(x,y))
    }
    for i in 0..4{
        let x = (i as f32 - (4 - 1) as f32 / 2.0) * horizontal_spacing;
        let y = -(vertical_offset*5.0);

        set_up_bundle(x,y,&mut commands);

        bottom_position.push(Vec2::new(x, y));
    }





}