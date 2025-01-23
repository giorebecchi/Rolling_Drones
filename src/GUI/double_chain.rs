use bevy::prelude::*;
use crate::GUI::butterfly::set_up_bundle;

pub fn spawn_double_chain(mut commands: &mut Commands) {
    let node_count_per_line = 5;
    let horizontal_spacing = 100.0;
    let vertical_offset = 50.0; // top line at +50, bottom line at -50

    let mut top_positions = Vec::with_capacity(node_count_per_line);
    let mut bottom_positions = Vec::with_capacity(node_count_per_line);

    // 1) Spawn the top line nodes
    for i in 0..node_count_per_line {
        // Position them horizontally centered around x=0
        let x = (i as f32 - (node_count_per_line - 1) as f32 / 2.0) * horizontal_spacing;
        let y = vertical_offset;

        set_up_bundle(x,y,&mut commands);

        top_positions.push(Vec2::new(x, y));
    }

    // 2) Spawn the bottom line nodes
    for i in 0..node_count_per_line {
        let x = (i as f32 - (node_count_per_line - 1) as f32 / 2.0) * horizontal_spacing;
        let y = -vertical_offset;

        set_up_bundle(x,y,&mut commands);

        bottom_positions.push(Vec2::new(x, y));
    }



}