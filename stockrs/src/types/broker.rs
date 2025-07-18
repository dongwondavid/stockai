use crate::db_manager::DBManager;
use crate::types::trading::Trading;
use chrono::NaiveDateTime;

#[derive(Debug, Clone, Copy)]
pub enum OrderSide {
    Buy,
    Sell,
}

pub struct Order {
    pub date: NaiveDateTime,
    pub stockcode: String,
    pub side: OrderSide,
    pub quantity: u32,
    pub price: f64,
    pub fee: f64,
    pub strategy: String,
}

impl Order {
    pub fn to_trading(&self) -> Trading {
        Trading::new(
            self.date,
            self.stockcode.clone(),
            matches!(self.side, OrderSide::Buy),
            self.quantity,
            self.price,
            self.fee,
            self.strategy.clone(),
        )
    }
}

pub enum BrokerType {
    REAL,
    PAPER,
    DB,
}

pub trait Broker {
    fn validate(&self, order: &Order) -> Result<(), Box<dyn std::error::Error>>;
    fn execute(&self, order: &Order, db: &DBManager) -> Result<(), Box<dyn std::error::Error>>;
}
