use std::fs;
use bevy::prelude::Resource;
use wg_2024::config::{Config, Drone};
use wg_2024::network::NodeId;

pub fn parse_config(file: &str) -> Config {
    let file_str = fs::read_to_string(file).unwrap();
    toml::from_str(&file_str).unwrap()
}
