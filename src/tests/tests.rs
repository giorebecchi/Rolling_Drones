#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::{thread};
    use std::time::Duration;
    use crossbeam_channel::{unbounded, Receiver, Sender};
    use wg_2024::controller::DroneCommand;
    use wg_2024::drone::Drone;
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::{FloodRequest, NodeType, Packet, PacketType};
    use crate::fair2024::drone::RollingDrone;
    use wg_2024::tests::{generic_fragment_forward,generic_fragment_drop};

    #[test]
    fn test_generic_fragment_forward(){// To run this test remember to comment the DroneEvent that is
                                        //  sent to the simulation controller in the handle_msg_fragment();.
        generic_fragment_forward::<RollingDrone>();
    }
    #[test]
    fn test_generic_fragment_drop(){ // To run this test remember to comment the DroneEvent that is
                                     //  sent to the simulation controller in the is_dropped().
        generic_fragment_drop::<RollingDrone>();
    }

    #[test]
    fn test_drone_crash() { //Passed
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


    pub struct Client {
        d: u8,
        send_channel: HashMap<NodeId, Sender<Packet>>,
        recv_channel: Receiver<Packet>,
    }

    impl Client {
        pub fn new(d: u8) -> Self {
            let (client_sender, client_receiver) = unbounded();
            let mut send_channel = HashMap::new();

            // Add the neighbor drone (NodeId 2)
            send_channel.insert(2, client_sender);

            Self {
                d,
                send_channel,
                recv_channel: client_receiver,
            }
        }
    }
    #[test]
    pub fn test() {
        // Create the client
        let client = Client::new(1);

        // Set up the channels
        let (drone2_sender, drone2_receiver) = unbounded::<Packet>();
        let (drone3_sender, drone3_receiver) = unbounded::<Packet>();
        let client_sender_to_drone2 = client.send_channel.get(&2).unwrap().clone();

        // Initialize Drone 2
        let mut drone2 = RollingDrone::new(2, unbounded().0, unbounded().1, drone2_receiver.clone(), {
            let mut map = HashMap::new();
            map.insert(1, client_sender_to_drone2.clone());
            map.insert(3, drone3_sender.clone());
            map
        }, 0.05);

        // Initialize Drone 3
        let mut drone3 = RollingDrone::new(3, unbounded().0, unbounded().1, drone3_receiver.clone(), {
            let mut map = HashMap::new();
            map.insert(2, drone2_sender.clone());
            map
        }, 0.05);


        let drone2_handle = thread::spawn(move || {
            drone2.run();
        });

        let drone3_handle = thread::spawn(move || {
            drone3.run();
        });

        let mut packet_to_drone2 = Packet {
            pack_type: PacketType::FloodRequest(FloodRequest {
                flood_id: 100,
                path_trace: vec![(1, NodeType::Client)],
                initiator_id: 1,
            }),
            routing_header: SourceRoutingHeader {
                hops: vec![1,2],
                hop_index: 0,
            },
            session_id: 0,
        };
        packet_to_drone2.routing_header.hop_index += 1;

        if let Some(sender) = client.send_channel.get(&2) {
            sender.send(packet_to_drone2.clone()).unwrap();
        }

        if let Ok(received_packet) = drone2_receiver.recv() {
            assert_eq!(
                received_packet.routing_header.hops[1],
                2,
                "Drone 2 should be the next hop"
            );
            assert_eq!(
                received_packet.routing_header.hop_index, 1,
                "Hop index should increment correctly"
            );
        } else {
            panic!("Drone 2 did not receive the packet");
        }

        let mut packet_to_drone3 = packet_to_drone2.clone();
        packet_to_drone3.routing_header.hop_index += 1;

        if drone3_sender.send(packet_to_drone3.clone()).is_err() {
            panic!("Failed to send the packet from Drone 2 to Drone 3");
        }


        if let Ok(received_packet) = drone3_receiver.recv() {
            assert_eq!(
                received_packet.routing_header.hops[2],
                3,
                "Drone 3 should be the next hop"
            );
            assert_eq!(
                received_packet.routing_header.hop_index, 2,
                "Hop index should increment correctly"
            );
        } else {
            panic!("Drone 3 did not receive the packet");
        }

        let mut packet_back_to_client = packet_to_drone3.clone();
        packet_back_to_client.routing_header.hop_index = 0;
        if drone2_sender.send(packet_back_to_client.clone()).is_err() {
            panic!("Failed to send the packet back to the client");
        }

        if let Ok(received_packet) = client.recv_channel.recv() {
            assert_eq!(
                received_packet.routing_header.hops[0],
                1,
                "Client should be the final destination"
            );
        } else {
            panic!("Client did not receive the packet");
        }

        // Wait for the drone threads to finish
        //drone2_handle.join().unwrap();
        //drone3_handle.join().unwrap();
    }

}
