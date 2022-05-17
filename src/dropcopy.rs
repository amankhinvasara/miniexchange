use std::net::SocketAddr;
use crate::{ESB, esb};
use crate::trade::{OrderUpdate};

//Receives OrderUpdate
//store them
//Sends aggregated trades based on the owner
//unsure what time interval to do this when requested?
//then can clear array?
pub struct Dropcopy {
    pub updates: [Vec<OrderUpdate>; 2],
}

impl Dropcopy {
    fn insert_update(&mut self, input: OrderUpdate) {
        if input.trader_id == 1 || input.trader_id == 2 {
            self.updates[0].push(input);
        } else if input.trader_id == 3 {
            self.updates[1].push(input);
        }
    }

    pub fn dropcopy_multicast_main(addr: SocketAddr)  { //-> JoinHandle<()>
        // socket creation
        let listener = ESB::connect_multicast(addr).expect("failing to create listener");
        println!("ipv4:server: joined: {}", addr);

        // Looping infinitely.
        loop {
            // test receive and response code will go here...
            let mut buf = [0u8; 64]; // receive buffer

            match listener.recv_from(&mut buf) {
                Ok((len, remote_addr)) => {
                    //Adjusted for OrderUpdate data
                    let data = &buf[..len];
                    let mut decoded: OrderUpdate = bincode::deserialize(data).unwrap();

                    let encoded = bincode::serialize(&decoded).unwrap();

                    println!("ipv4:server: got data: {} from: {}", String::from_utf8_lossy(data), remote_addr);
                    println!("data: {:?}", decoded);

                    //create a socket to forward scrubed data to the public multicast address.
                    let forward_addr = SocketAddr::new(esb::IPV4.clone(), esb::PUBLIC_DPCP_FORWARDER_PORT);
                    let forwarder = ESB::new_socket(&forward_addr).expect("failing to create responder").into_udp_socket();

                    forwarder.send_to(&encoded, &forward_addr).expect("failing to respond");
                }
                Err(err) => {
                    println!("ipv4:server: got an error: {}", err);
                }
            }
        }
    }

    //TODO add functionality to only send aggregation when requested.

}

