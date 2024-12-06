#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use bevy::prelude::{Has, Mut};
    use crossbeam_channel::{unbounded, Sender};
    use wg_2024::controller::DroneCommand;
    use wg_2024::drone::Drone;
    use wg_2024::packet::Packet;
    use crate::fair2024::drone::MyDrone;
    use wg_2024::tests::{generic_fragment_forward,generic_fragment_drop};

    #[test]
    fn test_generic_fragment_forward(){// To run this test remember to comment the DroneEvent that is
                                        //  sent to the simulation controller in the handle_msg_fragment();.
        generic_fragment_forward::<MyDrone>();
    }
    #[test]
    fn test_generic_fragment_drop(){ // To run this test remember to comment the DroneEvent that is
                                     //  sent to the simulation controller in the is_dropped().
        generic_fragment_drop::<MyDrone>();
    }
    #[test]
    fn test_pdr_command() { //Passes
        let (send_drone_command, rcv_drone_command) = unbounded();
        let (_, packet_recv) = unbounded();
        let packet_send_map: HashMap<u8, _> = HashMap::new();

        let drone = Arc::new(Mutex::new(MyDrone::new(
            1,
            unbounded().0,
            rcv_drone_command,
            packet_recv,
            packet_send_map,
            0.0,
        )));


        let drone_clone = Arc::clone(&drone);
        let drone_handle = thread::spawn(move || {
            let mut drone_instance = drone_clone.lock().unwrap();
            drone_instance.run();
        });

        send_drone_command.send(DroneCommand::SetPacketDropRate(0.5)).unwrap();
        send_drone_command.send(DroneCommand::Crash).unwrap();


        thread::sleep(Duration::from_millis(100));

        let drone_locked = drone.lock().unwrap();
        assert_eq!(drone_locked.pdr, 0.5, "PDR should be updated to 0.5");
        drone_handle.join().unwrap();

    }

    #[test]
    fn test_drone_crash() { //Passes
        let (send_drone_command, recv_drone_command) = unbounded();
        let (_, packet_recv) = unbounded();
        let packet_send_map: HashMap<u8, _> = HashMap::new();

        let drone = Arc::new(Mutex::new(MyDrone::new(
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

        thread::sleep(Duration::from_millis(100));

        send_drone_command.send(DroneCommand::Crash).unwrap();

        thread::sleep(Duration::from_millis(100));

        let result = drone_handle.join();
        assert!(result.is_ok(), "Drone thread did not exit as expected");

        let drone_locked = drone.lock().unwrap();
        assert!(drone_locked.packet_send.is_empty(), "Packet send map should be cleared after crash");
    }

    #[test]
    fn test_add_sender(){
        let (send_drone_command, rcv_drone_command) = unbounded(); //channel to send and receive commands
        let (_, packet_recv) = unbounded();
        let packet_send_map: HashMap<u8, _> = HashMap::new();

        let drone = Arc::new(Mutex::new(MyDrone::new(
            1,
            unbounded().0,
            rcv_drone_command,
            packet_recv,
            packet_send_map,
            0.0
        )));

        let drone_clone = Arc::clone(&drone);
        let drone_handle = thread::spawn(move || {
            let mut drone_instance = drone_clone.lock().unwrap();
            drone_instance.run();
        });

        let (new_sender, _) = unbounded();

        send_drone_command.send(DroneCommand::AddSender(3, new_sender)).unwrap();
        thread::sleep(Duration::from_millis(100));

        let res = drone_handle.join();
        assert!(res.is_ok(), "Drone thread did not exit as expected");

        let drone_locked = drone.lock().unwrap();
        assert!(drone_locked.packet_send.get(&3).is_some(), "Sender with node id 3 should have been added to the map" );

    }

    #[test]
    fn test_remove_sender(){
        let (send_drone_command, rcv_drone_command) = unbounded();
        let (_, packet_recv) = unbounded();
        let mut packet_send_map: HashMap<u8, _> = HashMap::new();
        let (new_sender, _) = unbounded();
        let (new_sender2, _) = unbounded();

        packet_send_map.insert(3, new_sender);
        packet_send_map.insert(2, new_sender2);


        let drone = Arc::new(Mutex::new(MyDrone::new(
            1,
            unbounded().0,
            rcv_drone_command,
            packet_recv,
            packet_send_map,
            0.0
        )));

        let drone_clone = Arc::clone(&drone);
        let drone_handle = thread::spawn(move || {
            let mut drone_instance = drone_clone.lock().unwrap();
            drone_instance.run();
        });

        send_drone_command.send(DroneCommand::RemoveSender(3)).unwrap();
        thread::sleep(Duration::from_millis(100));

        let result = drone_handle.join();
        assert!(result.is_ok(), "Drone thread did not exit as expected");

        let drone_locked = drone.lock().unwrap();
        assert_eq!(drone_locked.packet_send.len(), 1);
        assert!(drone_locked.packet_send.get(&3).is_none(), "Sender with node id 3 should have been removed");


    }


}
