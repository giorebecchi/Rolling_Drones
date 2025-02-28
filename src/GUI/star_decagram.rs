use bevy::prelude::*;
use crate::network_initializer::network_initializer::*;
use crate::GUI::login_window::{NodeConfig,NodeType};


pub fn spawn_star_decagram()->Vec<NodeConfig> {

    let config=parse_config("assets/configurations/star.toml");
    let drone_count = config.drone.len();
    let mut drones=Vec::new();

    let client_count= config.client.len();
    let server_count = config.server.len();

    //let mut clients =Vec::new();

    //let mut servers = Vec::new();

    let radius = 200.0;

    let mut positions = Vec::with_capacity(drone_count);


    for (i, drone) in config.drone.into_iter().enumerate() {
        let angle = i as f32 * std::f32::consts::TAU / drone_count as f32;
        let x = radius * angle.cos();
        let y = radius * angle.sin();


        positions.push(Vec2::new(x, y));
        let node=NodeConfig::new(NodeType::Drone, drone.id, Vec2::new(x, y), drone.connected_node_ids);
        drones.push(node);
    }
    let client_radius=100.;
    for (i, client) in config.client.into_iter().enumerate(){
        if client.connected_drone_ids.len()==1{
            let connected_drone=client.connected_drone_ids[0];
            let mut drone_position=Vec2::new(0.,0.);
            for drone in &drones{
                if drone.id==connected_drone{
                    drone_position=drone.position.clone();
                }
            }
            let angle= i as f32 * std::f32::consts::TAU / client_count as f32;
            let x = client_radius * angle.cos();
            let y = client_radius * angle.sin();
            positions.push(Vec2::new(x,y));
            let node = NodeConfig::new(NodeType::Client, client.id, Vec2::new(x,y),client.connected_drone_ids);
            drones.push(node);
        }else{

        }
    }

    drones

}


