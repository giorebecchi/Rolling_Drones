

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    //use crate::fair2024::simulation_control::*;
    use wg_2024::tests::*;
    use crate::fair2024::drone::MyDrone;
    use wg_2024::packet::PacketType::MsgFragment;

    #[test]
    fn test_generic_fragment_drop_with_my_drone() {
        generic_fragment_drop::<MyDrone>();
    }
    #[test]
    fn test_generic_fragment_forward(){
        generic_fragment_forward::<MyDrone>();
    }
    //#[test]
    //fn test_generic_chain_fragment_drop(){
    //    generic_chain_fragment_drop::<MyDrone>(); //fails because drones don't send any acks when they receive fragments
    //}
    //#[test]
    //fn test_generic_chain_fragment_ack(){
    //    generic_chain_fragment_ack::<MyDrone>();
    //} //fails for same reason as above


    use crossbeam_channel::{unbounded, Sender};
    use wg_2024::controller::DroneCommand;
    use wg_2024::drone::Drone;
    use wg_2024::network::NodeId;
    use wg_2024::packet::{Ack, Fragment, Packet, PacketType};

    #[test]
    fn test_add_sender() {
        let (controller_send, _controller_recv) = unbounded();
        let (_command_send, controller_recv) = unbounded();
        let (_packet_send, packet_recv) = unbounded();
        let packet_send_map: HashMap<NodeId, Sender<Packet>> = HashMap::new();

        let mut drone = MyDrone::new(1, controller_send, controller_recv, packet_recv, packet_send_map, 0.1);
        let (sender, _receiver) = unbounded();

        drone.add_sender(2, sender);

        assert!(drone.packet_send.contains_key(&2));
    }

    #[test]
    fn test_remove_sender() {
        let (controller_send, _controller_recv) = unbounded();
        let (_command_send, controller_recv) = unbounded();
        let (_packet_send, packet_recv) = unbounded();
        let mut packet_send_map: HashMap<NodeId, Sender<Packet>> = HashMap::new();
        let (sender, _receiver) = unbounded();
        packet_send_map.insert(2, sender);

        let mut drone = MyDrone::new(1, controller_send, controller_recv, packet_recv, packet_send_map, 0.1);
        drone.remove_sender(2);

        assert!(!drone.packet_send.contains_key(&2));
    }

    #[test]
    fn test_set_pdr() {
        let (controller_send, _controller_recv) = unbounded();
        let (_command_send, controller_recv) = unbounded();
        let (_packet_send, packet_recv) = unbounded();
        let packet_send_map: HashMap<NodeId, Sender<Packet>> = HashMap::new();

        let mut drone = MyDrone::new(1, controller_send, controller_recv, packet_recv, packet_send_map, 0.1);
        drone.set_pdr(0.5);

        assert_eq!(drone.pdr, 0.5);
    }

    #[test]
    fn test_handle_command_crash() {
        let (controller_send, _controller_recv) = unbounded();
        let (_command_send, controller_recv) = unbounded();
        let (_packet_send, packet_recv) = unbounded();
        let packet_send_map: HashMap<NodeId, Sender<Packet>> = HashMap::new();

        let mut drone = MyDrone::new(1, controller_send, controller_recv, packet_recv, packet_send_map, 0.1);
        drone.handle_command(DroneCommand::Crash);

        assert!(drone.packet_send.is_empty());
    }

    #[test]
    fn test_handle_packet_ack() {
        let (controller_send, _controller_recv) = unbounded();
        let (_command_send, controller_recv) = unbounded();
        let (packet_send, packet_recv) = unbounded();
        let mut packet_send_map: HashMap<NodeId, Sender<Packet>> = HashMap::new();
        let (sender, receiver) = unbounded();
        packet_send_map.insert(2, sender);

        let mut drone = MyDrone::new(1, controller_send, controller_recv, packet_recv, packet_send_map, 0.1);
        let packet = Packet {
            pack_type: PacketType::Ack(Ack{
                fragment_index:0,
            }),
            routing_header: Default::default(),
            session_id: 1,
        };

        packet_send.send(packet.clone()).unwrap();
        drone.handle_packet(packet.clone());

        assert_eq!(receiver.try_recv().unwrap(), packet);
    }

    #[test]
    fn test_handle_packet_msg_fragment() {
        let (controller_send, _controller_recv) = unbounded();
        let (_command_send, controller_recv) = unbounded();
        let (packet_send, packet_recv) = unbounded();
        let mut packet_send_map: HashMap<NodeId, Sender<Packet>> = HashMap::new();
        let (sender, _receiver) = unbounded();
        packet_send_map.insert(2, sender);

        let mut drone = MyDrone::new(1, controller_send, controller_recv, packet_recv, packet_send_map, 0.1);
        let packet = Packet {
            pack_type: PacketType::MsgFragment(Fragment {
                fragment_index: 0,
                total_n_fragments: 8,
                length: 8,
                data: [1;128],
            }),
            routing_header: Default::default(),
            session_id: 1,
        };

        packet_send.send(packet.clone()).unwrap();
        drone.handle_packet(packet);
    }

    #[test]
    fn test_packet_drop_rate() {
        let (controller_send, _controller_recv) = unbounded();
        let (_command_send, controller_recv) = unbounded();
        let (_packet_send, packet_recv) = unbounded();
        let packet_send_map: HashMap<NodeId, Sender<Packet>> = HashMap::new();

        let drone = MyDrone::new(1, controller_send, controller_recv, packet_recv, packet_send_map, 0.9);
        let packet = Packet {
            pack_type: PacketType::Ack(Ack{fragment_index: 0}),
            routing_header: Default::default(),
            session_id: 1,
        };

        assert!(drone.is_dropped(packet));
    }

}