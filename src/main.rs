#![feature(linked_list_remove)]
#![feature(type_ascription)]
use std::env;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use crate::esb::ESB;
use crate::orderbook::OrderBook;
use crate::trade::Trade;


mod trade;
mod orderbook;
mod esb;
mod client;
mod dropcopy;
mod tickerplant;


/**
 * 5 Args should look like the following
 * 1. trader_id - this client's trader id, should be int
 * 2. trade_type - 1 for buy, 0 for sell
 * 3. order_type - 1 for Limit, defaults to Market
 * 4. unit_price_ - price to place order at, directly as an int
 * 5. qty_ - quantity to order, directly as int
 */
fn main() {
    let args: Vec<String> = env::args().collect();
    let input = &args[1];
    // loop {
    if input == "1" || input == "ome" {
        //create ome
        //listen for stuff
        //listen function will call OME route! Keep looping inside listener
        //let ome = OrderBook::new();
        let addr = esb::IPV4.clone();
        //loop {
        OrderBook::multicast_sender(addr);
        //}

        // ome.listen();

    } else if input == "2" {
        //let addr = SocketAddr::new(*esb::IPV4, esb::PORT);
        let listener = UdpSocket::bind("224.0.1.123:5021").expect("couldn't bind to address");
        listener.join_multicast_v4(&Ipv4Addr::new(224, 0, 1, 123), &Ipv4Addr::new(0, 0, 0, 0));


        loop {
            //ESB::multicast_listener(Arc::new(AtomicBool::new(false)), addr);
            //ESB::multicast_listener(addr);
            // test receive and response code will go here...
            let mut buf = [0u8; 64]; // receive buffer
            println!("listening");
            match listener.recv_from(&mut buf) {
                Ok((len, remote_addr)) => {
                    //Adjusted for OrderUpdate data
                    let data = &buf[..len];
                    let mut decoded: Trade = bincode::deserialize(data).unwrap();
                    let encoded = bincode::serialize(&decoded).unwrap();

                    println!("ipv4:server: got data: {} from: {}", String::from_utf8_lossy(data), remote_addr);
                    println!("data: {:?}", decoded);

                    // //create a socket to send the response
                    // let forward_addr = SocketAddr::new(esb::IPV4.clone(), FORWARDER_PORT);
                    // let forwarder = ESB::new_socket(&forward_addr).expect("failing to create responder").into_udp_socket();
                    //
                    // //we send the response that was set at the method beginning
                    // forwarder.send_to(&encoded, &forward_addr).expect("failing to respond");
                }
                Err(err) => {
                    println!("ipv4:server: got an error: {}", err);
                }
            }
        }

    } else if input == "3" {
        let addr = SocketAddr::new(*esb::IPV4, esb::PORT);
        //loop {
        //ESB::multicast_listener(Arc::new(AtomicBool::new(false)), addr);
        OrderBook::ome_multicast_listener(addr);
        //}

    }
    // else if input == "2" || input == "client" {
    //     //run client
    //     client::get_trade_from_client();
    // }
}


