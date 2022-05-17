use std::io;
use std::time::Duration;
use std::sync::{Arc, Barrier};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{JoinHandle};
use std::time::SystemTime;

use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::trade::{OrderType, OrderUpdate, Trade, TradeType};
use crate::trade::OrderType::{Limit, Market};
use crate::trade::TradeType::{Buy, Sell};
use crate::trade::Status::{Filled, PartiallyFilled, Failed, Success};

use bincode;
use lazy_static::lazy_static;
use crate::OrderBook;

/// This will guarantee we always tell the server to stop
pub struct NotifyServer(pub Arc<AtomicBool>);
impl Drop for NotifyServer {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

pub struct ESB{}

lazy_static! {
    pub static ref IPV4: IpAddr = Ipv4Addr::new(224, 0, 1, 123).into();
}
pub const PORT: u16 = 5021;
pub const GATEWAY_PORT: u16 = 5022;
pub const FORWARDER_PORT: u16 = 5023;
impl ESB {
    //basic helper functions
    // General for creating new socket
    pub fn new_socket(addr: &SocketAddr) -> io::Result<Socket> {
        let domain = if addr.is_ipv4() {
            Domain::ipv4()
        } else {
            Domain::ipv6()
        };

        let socket = Socket::new(domain, Type::dgram(), Some(Protocol::udp()))?;

        // The read timeout prevents waiting for packets
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;

        Ok(socket)
    }


    pub fn connect_multicast(addr: SocketAddr) -> io::Result<std::net::UdpSocket> {
        let ip_addr = addr.ip();
        let socket = ESB::new_socket(&addr)?;

        match ip_addr {
            IpAddr::V4(ref mdns_v4) => {
                socket.join_multicast_v4(mdns_v4, &Ipv4Addr::new(0, 0, 0, 0))?;
            }
            IpAddr::V6(ref mdns_v6) => {
                socket.join_multicast_v6(mdns_v6, 0)?;
                socket.set_only_v6(true)?;
            }
        };

        // bind to the socket addr
        println!("{}", addr);
        ESB::bind_multicast(&socket, &addr)?;
        Ok(socket.into_udp_socket())
    }


    // pub fn multicast_listener(client_done: Arc<AtomicBool>, addr: SocketAddr, ) -> JoinHandle<()> {
    //     let server_barrier = Arc::new(Barrier::new(2));
    //     let client_barrier = Arc::clone(&server_barrier);
    //     let join_handle = std::thread::Builder::new()
    //         .name(format!("ipv4:server"))
    //         .spawn(move || {
    //
    //             // socket creation
    //             let listener = ESB::connect_multicast(addr).expect("failing to create listener");
    //             println!("ipv4:server: joined: {}", addr);
    //
    //             server_barrier.wait();
    //             println!("ipv4:server: is ready");
    //
    //             // Looping until the client is done.
    //             while !client_done.load(std::sync::atomic::Ordering::Relaxed) {
    //                 // test receive and response code will go here...
    //                 let mut buf = [0u8; 64]; // receive buffer
    //
    //                 match listener.recv_from(&mut buf) {
    //                     Ok((len, remote_addr)) => {
    //                         //Adjusted for OrderUpdate data
    //                         let data = &buf[..len];
    //                         let mut decoded: OrderUpdate = bincode::deserialize(data).unwrap();
    //
    //                         let encoded = bincode::serialize(&decoded).unwrap();
    //
    //                         println!("ipv4:server: got data: {} from: {}", String::from_utf8_lossy(data), remote_addr);
    //
    //                         //create a socket to send the response
    //                         let responder = ESB::new_socket(&remote_addr)
    //                             .expect("failing to create responder")
    //                             .into_udp_socket();
    //
    //                         //we send the response that was set at the method beginning
    //                         responder
    //                             .send_to(&encoded, &remote_addr)
    //                             .expect("failing to respond");
    //
    //                         println!("ipv4:server: sent response to: {}", remote_addr);
    //                     }
    //                     Err(err) => {
    //                         println!("ipv4:server: got an error: {}", err);
    //                     }
    //                 }
    //             }
    //
    //             println!("ipv4:server: client is done");
    //         })
    //         .unwrap();
    //
    //     client_barrier.wait();
    //     join_handle
    // }

    pub fn multicast_listener(addr: SocketAddr)  { //-> JoinHandle<()>
        // socket creation
        let listener = ESB::connect_multicast(addr).expect("failing to create listener");
        println!("ipv4:server: joined: {}", addr);

        println!("ipv4:server: is ready");

        // Looping infinitely.
        loop {
            // test receive and response code will go here...
            let mut buf = [0u8; 64]; // receive buffer

            match listener.recv_from(&mut buf) {
                Ok((len, remote_addr)) => {
                    //Adjusted for OrderUpdate data
                    let data = &buf[..len];
                    let mut decoded: Trade = bincode::deserialize(data).unwrap();

                    let encoded = bincode::serialize(&decoded).unwrap();

                    println!("ipv4:server: got data: {} from: {}", String::from_utf8_lossy(data), remote_addr);
                    println!("data: {:?}", decoded);

                    // //create a socket to send the response
                    // let responder = ESB::new_socket(&remote_addr)
                    //     .expect("failing to create responder")
                    //     .into_udp_socket();
                    //
                    // //we send the response that was set at the method beginning
                    // responder
                    //     .send_to(&encoded, &remote_addr)
                    //     .expect("failing to respond");

                    //println!("ipv4:server: sent response to: {}", remote_addr);
                }
                Err(err) => {
                    println!("ipv4:server: got an error: {}", err);
                }
            }
        }
    }

    // Create a new socket on the client
    pub fn new_sender(addr: &SocketAddr) -> io::Result<std::net::UdpSocket> {
        let socket = ESB::new_socket(addr)?;

        if addr.is_ipv4() {
            socket.bind(&SockAddr::from(SocketAddr::new(
                Ipv4Addr::new(0, 0, 0, 0).into(),
                0,
            )))?;
        } else {
            socket.bind(&SockAddr::from(SocketAddr::new(
                Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).into(),
                0,
            )))?;
        }
        Ok(socket.into_udp_socket())
    }


    #[cfg(windows)]
    pub fn bind_multicast(self, socket: &Socket, addr: &SocketAddr) -> io::Result<()> {
        let addr = match *addr {
            SocketAddr::V4(addr) => SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), addr.port()),
            SocketAddr::V6(addr) => {
                SocketAddr::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).into(), addr.port())
            }
        };
        socket.bind(&socket2::SockAddr::from(addr))
    }

    #[cfg(unix)]
    fn bind_multicast(socket: &Socket, addr: &SocketAddr) -> io::Result<()> {
        socket.bind(&socket2::SockAddr::from(*addr))
        //let addr2: &SocketAddr = &SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        //return socket.bind(&socket2::SockAddr::from(*addr2));
    }

    // Serialize and send/receive tp data over multicast
    // send data we get and receive data from OME

}
    #[cfg(test)]
    #[test]
    fn tp_test() {
        assert!(IPV4.is_multicast());
        OrderBook::multicast_sender(*IPV4);
    }

