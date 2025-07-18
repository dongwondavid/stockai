use crate::types::broker::Order;

pub fn execute_order_from_db(_order: &Order) -> Result<String, Box<dyn std::error::Error>> {
    todo!("execute order from db");
}

pub fn check_fill_from_db(_order_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
    todo!("check fill from db");
}

pub fn cancel_order_from_db(_order_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    todo!("cancel order from db");
}
