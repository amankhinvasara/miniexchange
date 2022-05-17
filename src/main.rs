#![feature(linked_list_remove)]
#![feature(type_ascription)]
use std::env;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use crate::esb::ESB;
use crate::orderbook::OrderBook;
use crate::tickerplant::TickerPlant;
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
    }
}


