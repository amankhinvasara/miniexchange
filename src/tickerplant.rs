use std::net::SocketAddr;
use crate::{ESB, esb, Trade};
use crate::esb::PUBLIC_TP_FORWARDER_PORT;
use crate::trade::{OrderUpdate};

pub struct TickerPlant {}

impl TickerPlant {
    //get updates, scrub, then send updates via UDP multicast
    //when you get an update from orderbook scrub data and send it
    fn scrub_data(mut input: OrderUpdate) -> OrderUpdate {
        input.trader_id = 0;
        input.order_id = 0;
        return input;
    }

    pub fn tp_multicast_main(addr: SocketAddr)  { //-> JoinHandle<()>
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
                    let scrubbed_output = TickerPlant::scrub_data(decoded);
                    let encoded = bincode::serialize(&scrubbed_output).unwrap();

                    println!("ipv4:server: got data: {} from: {}", String::from_utf8_lossy(data), remote_addr);
                    println!("data: {:?}", scrubbed_output);

                    //create a socket to forward scrubed data to the public multicast address.
                    let forward_addr = SocketAddr::new(esb::IPV4.clone(), esb::PUBLIC_TP_FORWARDER_PORT);
                    let forwarder = ESB::new_socket(&forward_addr).expect("failing to create responder").into_udp_socket();

                    forwarder.send_to(&encoded, &forward_addr).expect("failing to respond");
                }
                Err(err) => {
                    println!("ipv4:server: got an error: {}", err);
                }
            }
        }
    }

}