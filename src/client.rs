use text_io::read;
use crate::trade::{Trade, TradeType, OrderType};
use crate::trade::OrderType::{Limit, Market};
use crate::trade::TradeType::{Buy, Sell};

pub fn get_trade_from_client() -> Trade {

    let mut trade_type: TradeType;
    let mut temp: u64;
    loop {
        println!("Enter a trade type. 1:Buy 2:Sell");
        temp: u64 = read!("{}\n");
        if temp == 1 {
            trade_type = Buy;
            break;
        } else if temp == 2 {
            trade_type = Sell;
            break;
        } else {
            //TODO might need better logic
            println!("You have entered a wrong command please retry");
        }
    }

    println!("Enter the quantity of shares you want to trade.");
    let qty: u32 = read!("{}\n");


    let order_type: OrderType;
    let mut unit_price: u64 = 0;
    loop {
        println!("Enter an order type. 1:Limit 2:Market");
        temp: u64 = read!("{}\n");
        if temp == 1 {
            order_type = Limit;
            println!("Enter a price per share in cents.");
            unit_price = read!("{}\n");
            break;
        } else if temp == 2 {
            order_type = Market;
            break;
        } else {
            //TODO might need better logic
            println!("You have entered a wrong command please retry");
        }
    }

    let mut new_trade : Trade = Trade {
        trader_id: 0, //the gateway should set this based on the trader ip address
        // needs to know own ID
        order_id: 0, // always hardcode to 0
        stock_id: 0, // hardcode to 0, not checked
        trade_type: trade_type, // second argument
        order_type: order_type, // 3rd argument
        unit_price: unit_price, // 4th argument
        qty: qty, // 5th arg
        partial_fill: true, // always will partial fill in OME
        expiration_date : 0 // unused as well, just set to 0
    } ;
    println!("{:?}", new_trade); //make sure to send this new struct
    return new_trade;
}
