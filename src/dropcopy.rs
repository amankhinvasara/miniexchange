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

    //send updates do we use TCP? or UDP?
}

