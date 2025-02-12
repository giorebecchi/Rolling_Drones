#![cfg(test)]
use rusty_tester::*;
use std::time::Duration;
use fungi_drone::FungiDrone;
use rolling_drone::RollingDrone;
use bagel_bomber::BagelBomber;
use Krusty_Club::Krusty_C;
use skylink::SkyLinkDrone;
use LeDron_James::Drone as Le_Drone;
use lockheedrustin_drone::LockheedRustin;
use wg_2024_rust::drone::RustDrone;
use rustbusters_drone::RustBustersDrone;
use rusteze_drone::RustezeDrone;
use rustafarian_drone::RustafarianDrone;

type Tested = Le_Drone;
const TIMEOUT: Duration = Duration::from_millis(40);
const FLOOD_TIMEOUT: Duration = Duration::from_millis(100);

#[test]
fn drone_destination_is_drone() {
    test_drone_destination_is_drone::<Tested>(TIMEOUT);
}

#[test]
fn drone_error_in_routing() {
    test_drone_error_in_routing::<Tested>(TIMEOUT);
}

#[test]
fn drone_packet_1_hop() {
    test_drone_packet_1_hop::<Tested>(TIMEOUT);
}

#[test]
fn drone_packet_3_hop() {
    test_drone_packet_3_hop::<Tested>(TIMEOUT);
}

#[test]
fn drone_packet_3_hop_crash() {
    test_drone_packet_3_hop_crash::<Tested>(TIMEOUT);
}

#[test]
fn easiest_flood() {
    test_easiest_flood::<Tested>(FLOOD_TIMEOUT);
}

#[test]
fn loop_flood() {
    test_loop_flood::<Tested>(FLOOD_TIMEOUT);
}

#[test]
fn hard_loop_flood() {
    test_hard_loop_flood::<Tested>(FLOOD_TIMEOUT);
}

#[test]
fn matrix_loop_flood() {
    test_matrix_loop_flood::<Tested>(FLOOD_TIMEOUT);
}

#[test]
fn star_loop_flood() {
    test_star_loop_flood::<Tested>(FLOOD_TIMEOUT);
}

#[test]
fn butterfly_loop_flood() {
    test_butterfly_loop_flood::<Tested>(FLOOD_TIMEOUT);
}

#[test]
fn tree_loop_flood() {
    test_tree_loop_flood::<Tested>(FLOOD_TIMEOUT);
}
