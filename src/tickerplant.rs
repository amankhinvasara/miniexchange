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

}