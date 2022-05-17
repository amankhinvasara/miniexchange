use std::collections::{HashMap, LinkedList};
use std::net::{IpAddr, SocketAddr, Ipv4Addr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::SystemTime;
use crate::trade::{OrderUpdate, Trade, TradeType, Status};
use crate::trade::OrderType::{Limit, Market};
use crate::trade::TradeType::{Buy, Sell};
use crate::trade::Status::{Filled, PartiallyFilled, Failed, Success};
use rand::Rng;
use ntest::timeout;
use crate::esb::ESB;
use lazy_static::lazy_static;
use crate::esb;


//TODO make book and prices only visible for tests
pub struct OrderBook {
    pub book: HashMap<u64, Trade>, //order_id, Trade
    //https://stackoverflow.com/questions/28656387/initialize-a-large-fixed-size-array-with-non-copy-types
    //index is price, LL is trades at that price
    pub prices: HashMap<u64, Option<LinkedList<Trade>>>,
    //bids- people who are buying -> should be least to greatest
    bid_max: u64,
    //asks- people who are selling -> should be greatest to least
    ask_min: u64,
}


lazy_static! {
    pub static ref IPV4: IpAddr = Ipv4Addr::new(224, 0, 0, 123).into();
}
pub const PORT: u16 = 5021;
pub const GATEWAY_PORT: u16 = 5022;
pub const FORWARDER_PORT: u16 = 5023;

impl OrderBook {
    //cleanup all trades as the end of the day or need some other scheme to clean expired trades
    pub fn new() -> Self {
        //const SIZE: usize = 18446744073709551615; 64
        //const SIZE: usize = 4294967296; 32
        //const SIZE: usize = 65536; 16
        // const SIZE: usize = 256;
        // const INIT: Option<LinkedList<Trade>> = Some(LinkedList::new());
        // let array: [Option<LinkedList<Trade>>; SIZE] = [INIT; SIZE];
        Self {
            book: HashMap::new(),
            prices: HashMap::new(),
            bid_max: u64::MIN,
            ask_min: u64::MAX,
        }
    }

    pub fn single_trade_to_update(& mut self, input: Trade, input_status: u8) -> OrderUpdate {
        let stat: Status;
        if input_status == 1 {
            stat = Success;
        } else {
            stat = Failed;
        }
        return OrderUpdate {
            trader_id: input.trader_id,
            order_id: input.order_id,
            order_type: input.order_type,
            unit_price: input.unit_price,
            qty: input.qty,
            time_stamp: SystemTime::now(),
            status: stat,
        };
    }

    pub fn remove(&mut self, order_id: u64) -> OrderUpdate {
        //use order_id and get price from book
        //remove from price linked list
        //clone the list,modify the clone,  remove map entry, insert new one
        //modify bid_max and ask_min
        let mut trade = self.book[&order_id];
        if trade.trade_type == Buy && trade.unit_price == self.bid_max {
            //look for the next largest bid count down

            let mut list = self.prices.get_mut(&self.book[&order_id].unit_price).unwrap().as_mut().unwrap();
            for element in list.iter_mut().rev() {
                if element.trade_type == Buy {
                    self.bid_max = element.unit_price;
                    break;
                }
            }

        } else if trade.trade_type == Sell && trade.unit_price == self.ask_min {
            //look for the next smallest min count up
            let mut list = self.prices.get_mut(&trade.unit_price).unwrap().as_mut().unwrap();
            for element in list.iter_mut() {
                if element.trade_type == Sell {
                    self.ask_min = element.unit_price;
                    break;
                }
            }
        }

        let mut list = self.prices.get_mut(&self.book[&order_id].unit_price).unwrap().as_mut().unwrap();
        let mut i = 0;
        for element in list.iter_mut() {
            if element.order_id == order_id {
                //let mut prev = list.prev();
                list.remove(i);
                break;
            }
            i += 1;
        }
        //insert new list into price levels (should override previous list) and remove from book
        let store = self.book[&order_id].clone();
        self.book.remove(&order_id);
        return self.single_trade_to_update(store, 1);
    }


    pub fn insert(&mut self, trade: Trade) -> OrderUpdate {
        //insert into hashmap and then add to the appropriate arrays linked list
        self.book.insert(trade.order_id, trade);
        //true is bid(buyers) and false is ask(seller)
        if self.prices.contains_key(&trade.unit_price) {
            let mut list = self.prices[&trade.unit_price].as_ref().unwrap().clone();
            list.push_back(trade);
            self.prices.insert(trade.unit_price.clone(), Some(list));
        } else {
            let mut list = LinkedList::new();
            list.push_back(trade);
            self.prices.insert(trade.unit_price.clone(), Some(list));
        }

        if trade.trade_type == Buy && trade.unit_price > self.bid_max{
            self.bid_max = trade.unit_price;
        } else if trade.trade_type == Sell && trade.unit_price < self.ask_min {
            self.ask_min = trade.unit_price;
        }
        return self.single_trade_to_update(trade, 1);
    }


    pub fn modify(&mut self, order_id: u64, trade_input: Trade) -> OrderUpdate {
        if trade_input.order_id != order_id {
            return self.single_trade_to_update(trade_input, 0); //modify fails
        }
        self.remove(order_id);
        let two = self.insert(trade_input);
        return two;
    }

    pub fn bbo(&self) -> (u64, u64) {
        return (self.bid_max, self.ask_min);
    }


    //TODO match function -> should call add and remove appropriately
    //am currently assuming that everything is partial fill
    pub fn matching(&mut self, mut incoming_trade: &mut Trade) -> (Trade, Vec<Trade>) {
        let mut orders_filled: Vec<Trade> = Vec::new(); //remember that the last trade in this list might not be fully filled
        if incoming_trade.trade_type == Buy { // if its a buy order
            // println!("2");
            if incoming_trade.unit_price >= self.ask_min { //start at minimum and keep going until order is filled
                // println!("3");
                let mut price = self.ask_min;
                while price <= incoming_trade.unit_price || (incoming_trade.order_type == Market && price < u64::MAX) {
                    if self.prices.contains_key(&price) && self.prices.get_mut(&price).is_some() {
                        let mut linked_list = self.prices.get_mut(&price).unwrap().as_mut().unwrap();
                        for current_trade in linked_list.iter_mut() {
                            if current_trade.trade_type == Sell {
                                if incoming_trade.qty > current_trade.qty {
                                    incoming_trade.qty = incoming_trade.qty - current_trade.qty;
                                    current_trade.qty = 0;
                                    orders_filled.push(current_trade.clone());
                                } else if current_trade.qty >= incoming_trade.qty {
                                    current_trade.qty = current_trade.qty - incoming_trade.qty;
                                    incoming_trade.qty = 0;
                                    orders_filled.push(current_trade.clone());
                                    break;
                                }
                            }
                        }
                    }
                    if incoming_trade.qty == 0 {
                        break;
                    }
                    price += 1;
                }
            }
        } else { // its a sell order
            // println!("NEIN");
            if incoming_trade.unit_price <= self.bid_max { //start at max and keep going down until order is filled
                let mut price = self.bid_max;
                while price >= incoming_trade.unit_price || (incoming_trade.order_type == Market && price > u64::MIN) {
                    if self.prices.contains_key(&price) && self.prices.get_mut(&price).is_some() {
                        let mut linked_list = self.prices.get_mut(&price).unwrap().as_mut().unwrap();
                        for current_trade in linked_list.iter_mut() {
                            if current_trade.trade_type == Buy {
                                if incoming_trade.qty > current_trade.qty {
                                    incoming_trade.qty = incoming_trade.qty - current_trade.qty;
                                    current_trade.qty = 0;
                                    orders_filled.push(current_trade.clone());
                                } else if current_trade.qty >= incoming_trade.qty {
                                    current_trade.qty = current_trade.qty - incoming_trade.qty;
                                    incoming_trade.qty = 0;
                                    orders_filled.push(current_trade.clone());
                                    break;
                                }
                            }
                        }
                    }
                    if incoming_trade.qty == 0 {
                        break;
                    }
                    price -= 1;
                }
            }
        }
        //Orders that reach this point can be filled, partial fill, neither
        //remove filled orders from the book
        for trade in &orders_filled {
            if trade.qty == 0 {
                self.remove(trade.order_id);
            }
        }
        if incoming_trade.qty > 0 && incoming_trade.order_type == Limit {
            self.insert(incoming_trade.clone());
        }
        //make sure return values are correct
        return (incoming_trade.clone(), orders_filled);
    }



    //the last update in the output vector is always the taker
    pub fn trade_to_order_update(&mut self, taker: Trade, trades: Vec<Trade>) -> Vec<OrderUpdate>{
        let mut order_updates : Vec<OrderUpdate> = vec![];
        let mut stat: Status;
        let mut avg_price: u64 = 0;
        //resting trades
        for t in &trades {
            avg_price += t.unit_price;

            if t.qty == 0 {
                stat = Filled;
            } else {
                stat = PartiallyFilled;
            }
            let order_update = OrderUpdate {
                trader_id: t.trader_id,
                order_id: t.order_id,
                order_type: t.order_type,
                unit_price: t.unit_price, //make sure prices are modified correctly earlier
                qty: t.qty, //returns the quantity of that is left in the orderbook. Trader must match with what they sent earlier. //TODO refactor structs to fix tis
                time_stamp: SystemTime::now(),
                status: stat
            };
            order_updates.push(order_update);
        }

        //then taker trade
        if taker.qty == 0 {
            stat = Filled;
        } else {
            stat = PartiallyFilled;
        }
        let order_update = OrderUpdate {
            trader_id: taker.trader_id,
            order_id: taker.order_id,
            order_type: taker.order_type,
            unit_price: avg_price / trades.len() as u64, //make sure prices are modified correctly earlier
            qty: taker.qty, //returns the quantity of that is left in the orderbook. Trader must match with what they sent earlier. //TODO refactor structs to fix tis
            time_stamp: SystemTime::now(),
            status: stat
        };

        order_updates.push(order_update);
        return order_updates;

    }


    //TODO create another function that routes to add/modify/match based on order type
    //Enforce that route and maybe "fn top" are the only point of interaction w the order book
    pub fn route(&mut self, incoming_trade: Trade) -> Vec<OrderUpdate> {
        //if the order id already exists then send it to modify or cancel??????
        if self.book.contains_key(&incoming_trade.order_id) && self.book[&incoming_trade.order_id].trader_id == incoming_trade.trader_id  {
            if incoming_trade.unit_price == 0 { //cancel if the price is 0
                return vec![self.remove(incoming_trade.order_id)];
            } else { //otherwise modify
                return vec![self.modify(incoming_trade.order_id, incoming_trade)]; //returns the current trade
            }
        } else {
            let (mut one, mut two) = self.matching(&mut incoming_trade.clone());
            let updates_to_be_sent = self.trade_to_order_update(one, two);
            return updates_to_be_sent;
        }
    }


    //#[cfg(any(test, test_utilities))]
    pub fn generate_random_trade() -> Trade {
        let mut rng = rand::thread_rng();
        let num = rng.gen::<u8>();
        let mut trade_type;
        if num % 2 == 0 {
            trade_type = Buy;
        } else {
            trade_type = Sell;
        }
        Trade {
            trader_id: rng.gen::<u8>(),
            stock_id: rng.gen::<u16>(),
            order_id: rng.gen::<u64>(),
            trade_type: trade_type, //buy or sell
            order_type: Market, //What type of order (market, limit, etc)
            unit_price: rng.gen::<u64>(), //price of share in cents so we do not have to deal w super long decimals
            qty: rng.gen::<u32>(), //number of the item they want to buy or sell
            partial_fill: true, //is partial fill of orders allowed or not
            expiration_date: rng.gen::<u32>(), //immediate fill, end_of_day, 90 day? unsure what common types there are
        }
    }

    #[cfg(any(test, test_utilities))]
    pub fn generate_random_market(input_trade_type: TradeType) -> Trade {
        let mut rng = rand::thread_rng();
        let num = rng.gen::<u8>();
        Trade {
            trader_id: rng.gen::<u8>(),
            stock_id: rng.gen::<u16>(),
            order_id: rng.gen::<u64>(),
            trade_type: input_trade_type,
            order_type: Market,
            unit_price: rng.gen::<u64>(),
            qty: rng.gen::<u32>(),
            partial_fill: true,
            expiration_date: rng.gen::<u32>(),
        }
    }

    #[cfg(any(test, test_utilities))]
    pub fn generate_random_limit(input_trade_type: TradeType) -> Trade {
        let mut rng = rand::thread_rng();
        let num = rng.gen::<u8>();
        Trade {
            trader_id: rng.gen::<u8>(),
            stock_id: rng.gen::<u16>(),
            order_id: rng.gen::<u64>(),
            trade_type: input_trade_type,
            order_type: Limit,
            unit_price: rng.gen::<u64>(),
            qty: rng.gen::<u32>(),
            partial_fill: true,
            expiration_date: rng.gen::<u32>(),
        }
    }

    // pub fn multicast_sender(addr: IpAddr) {
    //     assert!(addr.is_multicast());
    //
    //     let addr = SocketAddr::new(addr, PORT);
    //     let client_done = Arc::new(AtomicBool::new(false)); //tracks if client socket is running
    //     let notify = crate::esb::NotifyServer(Arc::clone(&client_done));
    //
    //     // ESB::multicast_listener(client_done, addr);
    //
    //     println!("ipv4 :client: running");
    //
    //     //insert data to send
    //     let trade = OrderUpdate {
    //         trader_id: 1,
    //         order_id: 1,
    //         order_type: Market,
    //         unit_price: 1,
    //         qty: 1,
    //         time_stamp: SystemTime::now(),
    //         status: Filled
    //     };
    //
    //     let encoded = bincode::serialize(&trade).unwrap();
    //
    //     println!("ipv4:client: send data: {:?}", trade);
    //     println!("{:?}", encoded);
    //
    //     // Setup sending socket
    //     let socket = ESB::new_sender(&addr).expect("could not create sender!");
    //     socket.send_to(&encoded, &addr).expect("could not send_to!");
    //
    //
    //     let mut buf = [0u8; 64]; // receive buffer
    //
    //     // Expected response
    //     match socket.recv_from(&mut buf) {
    //         Ok((len, remote_addr)) => {
    //             let data = &buf[..len];
    //
    //             println!("{:?}", data);
    //
    //             let mut decoded: OrderUpdate = bincode::deserialize(data).unwrap();
    //             decoded.trader_id = 0;
    //             decoded.order_id = 0;
    //
    //             //let response = data;
    //             println!("ipv4:client: got data: {:?}", decoded);
    //         }
    //
    //         Err(err) => {
    //             println!("ipv4:client: had a problem: {}", err);
    //             assert!(false);
    //         }
    //     }
    //     // Making sure that the server is not notified until the end of the client test
    //     drop(notify);
    // }

    // pub fn interact_only() {
    //     //thread::Builder::new().name("thread1".to_string()).spawn(move || {
    //     //println!("Hello, world!");
    //     let gate_addr = SocketAddr::new(addr, GATEWAY_PORT);
    //     ESB::multicast_listener(gate_addr);
    //     //});
    // }

    pub fn ome_multicast_listener(addr: SocketAddr)  { //-> JoinHandle<()>
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

                    //create a socket to send the response
                    let forward_addr = SocketAddr::new(esb::IPV4.clone(), FORWARDER_PORT);
                    let forwarder = ESB::new_socket(&forward_addr).expect("failing to create responder").into_udp_socket();

                    //we send the response that was set at the method beginning
                    forwarder.send_to(&encoded, &forward_addr).expect("failing to respond");
                }
                Err(err) => {
                    println!("ipv4:server: got an error: {}", err);
                }
            }
        }
    }
    pub fn multicast_sender(addr: IpAddr) {
        let addr = SocketAddr::new(addr, PORT);
        println!("ipv4 :client: running");
        //insert data to send
        // let trade = OrderUpdate {
        //     trader_id: 1,
        //     order_id: 1,
        //     order_type: Market,
        //     unit_price: 1,
        //     qty: 1,
        //     time_stamp: SystemTime::now(),
        //     status: Filled
        // }
        let trade = OrderBook::generate_random_trade();
        //println!("{:?} ", trade);
        let sender = UdpSocket::bind("192.168.50.102:5021").expect("couldn't bind to address");
        //socket.bind(&SockAddr::from(SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), 0,);

        let encoded = bincode::serialize(&trade).unwrap();
        //println!("ipv4:client: send data: {:?}", trade);
        //println!("{:?}", encoded);
        // Setup sending socket
        //let socket = ESB::new_sender(&addr).expect("could not create sender!");
        //socket.send_to(&encoded, &addr).expect("could not send_to!");
        sender.send_to(&encoded, &addr).expect("could not send_to!");
        println!("{:?} Sent", trade);
    }
}

#[cfg(test)]
mod tests {
    use ntest::assert_false;
    use crate::orderbook::OrderBook;
    use crate::trade;
    use crate::trade::TradeType::{Buy, Sell};

    #[test]
    fn single_insert() {
        let mut book = OrderBook::new();
        let mut rand_trade = OrderBook::generate_random_trade();
        book.insert(rand_trade);
        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&rand_trade.unit_price].as_ref().unwrap().len(), 1);
    }

    #[test]
    fn many_inserts() {
        let mut book = OrderBook::new();
        for i in 1..100 {
            let mut rand_trade = OrderBook::generate_random_trade();
            book.insert(rand_trade);
            assert_eq!(book.book.len(), i);
        }
    }

    #[test]
    fn single_remove() {
        let mut book = OrderBook::new();
        let mut rand_trade = OrderBook::generate_random_trade();
        book.insert(rand_trade);
        book.remove(rand_trade.order_id);
        assert_eq!(book.book.len(), 0);
        assert_eq!(book.prices[&rand_trade.unit_price].as_ref().unwrap().len(), 0);
    }

    #[test]
    fn single_modify_fail_no_list() {
        let mut book = OrderBook::new();
        let mut first_trade = OrderBook::generate_random_trade();
        let mut second_trade = OrderBook::generate_random_trade();
        book.insert(first_trade);
        // book.modify(rand_trade.order_id, second_trade);
        if first_trade.order_id == second_trade.order_id {
            second_trade.order_id += 1;
        }
        // the original order should remain unmodified
        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&first_trade.unit_price].as_ref().unwrap().len(), 1);
        assert!(!book.prices.contains_key(&second_trade.unit_price));
    }

    #[test]
    fn single_modify_success() {
        let mut book = OrderBook::new();
        let mut rand_trade = OrderBook::generate_random_trade();
        let mut second_trade = OrderBook::generate_random_trade();
        second_trade.order_id = rand_trade.order_id;
        book.insert(rand_trade);
        book.modify(rand_trade.order_id, second_trade);
        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&rand_trade.unit_price].as_ref().unwrap().len(), 0);
        assert_eq!(book.prices[&second_trade.unit_price].as_ref().unwrap().len(), 1);
    }

    #[test]
    fn single_modify_price_zero_success() {
        let mut book = OrderBook::new();
        let mut rand_trade = OrderBook::generate_random_trade();
        book.insert(rand_trade);
        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&rand_trade.unit_price].as_ref().unwrap().len(), 1);

        let mut second_trade = OrderBook::generate_random_trade();
        second_trade.order_id = rand_trade.order_id;
        second_trade.unit_price = 0;
        book.modify(rand_trade.order_id, second_trade);
        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&rand_trade.unit_price].as_ref().unwrap().len(), 0);
        assert_eq!(book.prices[&second_trade.unit_price].as_ref().unwrap().len(), 1);
    }


    //Tests for Matching engine
    //Market Buy
    //Market Sell
    //Limit Buy -> fail, success (test success w sparse and full)
    //Limit Sell -> fail, success (test success w sparse and full)
    //everytime bid ask spread is changed send to ticket book
    //dropcopy and gateway would get cancelled and modified trades.

    #[test]
    #[ntest::timeout(1000)]
    fn limit_buy_equal_qty() {
        let mut book = OrderBook::new();
        let mut sell_trade = OrderBook::generate_random_limit(Sell);
        let mut buy_trade = OrderBook::generate_random_limit(Buy);
        sell_trade.unit_price = buy_trade.unit_price;
        sell_trade.qty = buy_trade.qty;

        book.insert(sell_trade);
        book.matching(&mut buy_trade); //test return values?

        assert_eq!(book.book.len(), 0);
        assert_eq!(book.prices[&sell_trade.unit_price].as_ref().unwrap().len(), 0);
    }

    #[test]
    #[ntest::timeout(1000)]
    fn limit_sell_equal_qty() {
        let mut book = OrderBook::new();
        let mut sell_trade = OrderBook::generate_random_limit(Sell);
        let mut buy_trade = OrderBook::generate_random_limit(Buy);
        sell_trade.unit_price = buy_trade.unit_price;
        sell_trade.qty = buy_trade.qty;

        book.insert(buy_trade);
        book.matching(&mut sell_trade); //test return values?

        assert_eq!(book.book.len(), 0);
        assert_eq!(book.prices[&sell_trade.unit_price].as_ref().unwrap().len(), 0);
    }

    #[test]
    #[ntest::timeout(1000)]
    fn limit_buy_equal_qty_sparse() {
        let mut book = OrderBook::new();
        let mut sell_trade = OrderBook::generate_random_limit(Sell);
        let mut buy_trade = OrderBook::generate_random_limit(Buy);
        sell_trade.qty = buy_trade.qty;
        sell_trade.unit_price = buy_trade.unit_price - 1000000000;

        book.insert(sell_trade);
        book.matching(&mut buy_trade); //test return values?

        assert_eq!(book.book.len(), 0);
        assert_eq!(book.prices[&sell_trade.unit_price].as_ref().unwrap().len(), 0);
    }

    #[test]
    #[ntest::timeout(1000)]
    fn limit_sell_equal_qty_sparse() {
        let mut book = OrderBook::new();
        let mut sell_trade = OrderBook::generate_random_limit(Sell);
        let mut buy_trade = OrderBook::generate_random_limit(Buy);
        sell_trade.qty = buy_trade.qty;
        sell_trade.unit_price = buy_trade.unit_price - 10000000;

        book.insert(buy_trade);
        book.matching(&mut sell_trade); //test return values?

        assert_eq!(book.book.len(), 0);
        assert_eq!(book.prices[&buy_trade.unit_price].as_ref().unwrap().len(), 0);
    }

    #[test]
    #[ntest::timeout(1000)]
    fn limit_buy_unequal_qty() {
        let mut book = OrderBook::new();
        let mut sell_trade = OrderBook::generate_random_limit(Sell);
        let mut buy_trade = OrderBook::generate_random_limit(Buy);
        sell_trade.qty = buy_trade.qty + 1;
        sell_trade.unit_price = buy_trade.unit_price;

        book.insert(sell_trade);
        book.matching(&mut buy_trade); //test return values?

        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&sell_trade.unit_price].as_ref().unwrap().len(), 1);
    }

    #[test]
    #[ntest::timeout(1000)]
    fn limit_sell_unequal_qty() {
        let mut book = OrderBook::new();
        let mut sell_trade = OrderBook::generate_random_limit(Sell);
        let mut buy_trade = OrderBook::generate_random_limit(Buy);
        sell_trade.qty = buy_trade.qty + 1;
        sell_trade.unit_price = buy_trade.unit_price;

        book.insert(buy_trade);
        book.matching(&mut sell_trade); //test return values?

        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&buy_trade.unit_price].as_ref().unwrap().len(), 1);
    }


    #[test]
    #[ntest::timeout(1000)]
    fn limit_buy_unequal_seller_sparse() {
        let mut book = OrderBook::new();
        let mut sell_trade = OrderBook::generate_random_limit(Sell);
        let mut buy_trade = OrderBook::generate_random_limit(Buy);
        sell_trade.qty = buy_trade.qty + 1;
        sell_trade.unit_price = buy_trade.unit_price - 100000000;

        book.insert(sell_trade);
        book.matching(&mut buy_trade); //test return values?

        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&sell_trade.unit_price].as_ref().unwrap().len(), 1);
        assert_false!(book.prices.contains_key(&buy_trade.unit_price));
    }

    #[test]
    #[ntest::timeout(10000)]
    fn limit_sell_unequal_seller_qty() {
        let mut book = OrderBook::new();
        let mut sell_trade = OrderBook::generate_random_limit(Sell);
        let mut buy_trade = OrderBook::generate_random_limit(Buy);
        buy_trade.qty = sell_trade.qty + 1;
        sell_trade.unit_price = buy_trade.unit_price - 100000;

        book.insert(buy_trade);
        book.matching(&mut sell_trade); //test return values?

        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&buy_trade.unit_price].as_ref().unwrap().len(), 1);
        assert_false!(book.prices.contains_key(&sell_trade.unit_price));
    }

    #[test]
    #[ntest::timeout(10000)]
    fn limit_buy_unequal_buyer_sparse() {
        let mut book = OrderBook::new();
        let mut sell_trade = OrderBook::generate_random_limit(Sell);
        let mut buy_trade = OrderBook::generate_random_limit(Buy);
        sell_trade.qty = buy_trade.qty - 1;
        sell_trade.unit_price = buy_trade.unit_price - 1000000;
        book.insert(sell_trade);
        book.matching(&mut buy_trade); //test return values?
        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&sell_trade.unit_price].as_ref().unwrap().len(), 0);
        assert_eq!(book.prices[&buy_trade.unit_price].as_ref().unwrap().len(), 1);
    }

    #[test]
    #[ntest::timeout(10000)]
    fn limit_sell_unequal_buyer_qty() {
        let mut book = OrderBook::new();
        let mut sell_trade = OrderBook::generate_random_limit(Sell);
        let mut buy_trade = OrderBook::generate_random_limit(Buy);
        buy_trade.qty = sell_trade.qty - 1;
        sell_trade.unit_price = buy_trade.unit_price - 100000;

        book.insert(buy_trade);
        book.matching(&mut sell_trade); //test return values?

        assert_eq!(book.book.len(), 1);
        assert_eq!(book.prices[&buy_trade.unit_price].as_ref().unwrap().len(), 0);
        assert_eq!(book.prices[&sell_trade.unit_price].as_ref().unwrap().len(), 1);
    }
}

