

#[cfg(test)]
mod tests {
    //use crate::fair2024::simulation_control::*;
    use wg_2024::tests::*;
    use crate::fair2024::drone::MyDrone;

    #[test]
    fn test_generic_fragment_drop_with_my_drone() {
        generic_fragment_drop::<MyDrone>();
    }
    #[test]
    fn test_generic_fragment_forward(){
        generic_fragment_forward::<MyDrone>();
    }
    #[test]
    fn test_generic_chain_fragment_drop(){
        generic_chain_fragment_drop::<MyDrone>(); //fails because drones don't send any acks when they receive fragments
    }
    #[test]
    fn test_generic_chain_fragment_ack(){
        generic_chain_fragment_ack::<MyDrone>();
    } //fails for same reason as above
}