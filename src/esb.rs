use std::io;
use std::time::Duration;
use std::sync::{Arc, Barrier};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{JoinHandle};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::trade::{OrderUpdate, Trade, TradeType};

use bincode;

pub struct ESB{

}
/// This will guarantee we always tell the server to stop
struct NotifyServer(Arc<AtomicBool>);
impl Drop for NotifyServer {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
impl ESB {
    //For now this is just a wrapper around some functions from https://doc.rust-lang.org/stable/std/net/struct.UdpSocket.html

    pub const PORT: u16 = 5021;
    lazy_static! {
    pub static ref IPV4: IpAddr = Ipv4Addr::new(224, 0, 0, 123).into();
    pub static ref IPV6: IpAddr = Ipv6Addr::new(0xFF02, 0, 0, 0, 0, 0, 0, 0x0123).into();
}


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
        let socket = new_socket(&addr)?;

        // IP protocol
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
        bind_multicast(&socket, &addr)?;
        Ok(socket.into_udp_socket())
    }


    pub fn multicast_listener(
        response: &'static str,
        client_done: Arc<AtomicBool>,
        addr: SocketAddr,
    ) -> JoinHandle<()> {
        let server_barrier = Arc::new(Barrier::new(2));
        let client_barrier = Arc::clone(&server_barrier);

        let join_handle = std::thread::Builder::new()
            .name(format!("{}:server", response))
            .spawn(move || {

                // socket creation
                let listener = connect_multicast(addr).expect("failing to create listener");
                println!("{}:server: joined: {}", response, addr);

                server_barrier.wait();
                println!("{}:server: is ready", response);

                // Looping until the client is done.
                while !client_done.load(std::sync::atomic::Ordering::Relaxed) {
                    // test receive and response code will go here...
                    let mut buf = [0u8; 64]; // receive buffer

                    match listener.recv_from(&mut buf) {
                        Ok((len, remote_addr)) => {
                            //Adjusted for Orderupdate data
                            let data = &buf[..len];
                            let mut decoded: OrderUpdate = bincode::deserialize(data).unwrap();
                            decoded.trader_id = 0;
                            decoded.order_id = 0;
                            let encoded = bincode::serialize(&decoded).unwrap();

                            println!(
                                "{}:server: got data: {} from: {}",
                                response,
                                String::from_utf8_lossy(data),
                                remote_addr
                            );

                            // create a socket to send the response
                            let responder = new_socket(&remote_addr)
                                .expect("failing to create responder")
                                .into_udp_socket();

                            // we send the response that was set at the method beginning
                            responder
                                .send_to(&encoded, &remote_addr)
                                .expect("failing to respond");

                            println!("{}:server: sent response to: {}", response, remote_addr);
                        }
                        Err(err) => {
                            println!("{}:server: got an error: {}", response, err);
                        }
                    }

                }

                println!("{}:server: client is done", response);
            })
            .unwrap();

        client_barrier.wait();
        join_handle
    }

    // Create a new socket on the client
    pub fn new_sender(addr: &SocketAddr) -> io::Result<std::net::UdpSocket> {
        let socket = new_socket(addr)?;

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
    pub fn bind_multicast(socket: &Socket, addr: &SocketAddr) -> io::Result<()> {
        let addr = match *addr {
            SocketAddr::V4(addr) => SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), addr.port()),
            SocketAddr::V6(addr) => {
                SocketAddr::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).into(), addr.port())
            }
        };
        socket.bind(&socket2::SockAddr::from(addr))
    }



}