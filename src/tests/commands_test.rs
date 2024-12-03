#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crossbeam_channel::unbounded;
    use crate::fair2024::simulation_control::*;
    use wg_2024::drone::Drone;


    #[test]
    fn test_add_sender() {
        let (controller_send, _controller_recv) = unbounded();
        let (packet_send, _) = unbounded();
        let (node_event_send, _) = unbounded();
        let packet_recv = unbounded().1;

        // Create the drone directly without threads
        let mut drone = MyDrone::new(
            1,
            node_event_send,
            _controller_recv, // We don't need the receiver for this test
            packet_recv,
            HashMap::new(),
            0.1,
        );

        // Simulate receiving the AddSender command directly
        let neighbor_id = 2;
        drone.add_sender(neighbor_id, packet_send.clone());

        // Verify that the neighbor was added
        assert!(drone.packet_send.contains_key(&neighbor_id));
        println!("AddSender command processed successfully.");
    }
    #[test]
    fn test_remove_sender() {
        let (controller_send, _controller_recv) = unbounded();
        let (node_event_send, _) = unbounded();
        let packet_recv = unbounded().1;

        // Create the drone directly without threads
        let mut drone = MyDrone::new(
            1,
            node_event_send,
            _controller_recv, // We don't need the receiver for this test
            packet_recv,
            HashMap::new(),
            0.1,
        );

        let neighbor_id = 2;
        drone.remove_sender(neighbor_id);


        // Verify that the neighbor was removed
        assert!(!drone.packet_send.contains_key(&neighbor_id));
        println!("RemoveSender command processed successfully.");
    }
}
