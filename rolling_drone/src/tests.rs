#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use crossbeam_channel::unbounded;
    use wg_2024::controller::DroneCommand;
    use wg_2024::drone::Drone;
    use crate::drone::RollingDrone;
    use wg_2024::tests::{generic_chain_fragment_ack, generic_chain_fragment_drop, generic_fragment_drop, generic_fragment_forward};


    #[test]
    fn test_generic_fragment_forward(){

        generic_fragment_forward::<RollingDrone>();
    }
    #[test]
    fn test_generic_fragment_drop(){

        generic_fragment_drop::<RollingDrone>();
    }
    #[test]
    fn test_generic_chain_fragment_drop(){
        generic_chain_fragment_drop::<RollingDrone>();
    }
    #[test]
    fn test_generic_chain_fragment_ack(){
        generic_chain_fragment_ack::<RollingDrone>();
    }

    #[test]
    fn test_drone_crash() {
        let (send_drone_command, recv_drone_command) = unbounded();
        let (_, packet_recv) = unbounded();
        let packet_send_map: HashMap<u8, _> = HashMap::new();

        let drone = Arc::new(Mutex::new(RollingDrone::new(
            1,
            unbounded().0,
            recv_drone_command,
            packet_recv,
            packet_send_map,
            0.0,
        )));

        let drone_clone = Arc::clone(&drone);
        let drone_handle = thread::spawn(move || {
            let mut drone_instance = drone_clone.lock().unwrap();
            drone_instance.run();
        });


        send_drone_command.send(DroneCommand::Crash).unwrap();

        thread::sleep(Duration::from_millis(100));

        let result = drone_handle.join();
        assert!(result.is_ok(), "Drone thread did not exit as expected");

        let drone_locked = drone.lock().unwrap();
        assert!(drone_locked.packet_send.is_empty(), "Packet send map should be cleared after crash");
    }





}
