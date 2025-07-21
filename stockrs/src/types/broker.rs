use crate::db_manager::DBManager;
use crate::types::trading::Trading;
use chrono::NaiveDateTime;

#[derive(Debug, Clone, Copy)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
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

    // Getter 메서드들
    pub fn get_stockcode(&self) -> &str {
        &self.stockcode
    }

    pub fn get_quantity(&self) -> u32 {
        self.quantity
    }

    pub fn get_price(&self) -> f64 {
        self.price
    }

    pub fn get_buy_or_sell(&self) -> bool {
        matches!(self.side, OrderSide::Buy)
    }

    pub fn get_fee(&self) -> f64 {
        self.fee
    }

    pub fn get_strategy(&self) -> &str {
        &self.strategy
    }

    pub fn get_date(&self) -> NaiveDateTime {
        self.date
    }
}

pub enum BrokerType {
    REAL,
    PAPER,
    DB,
}

pub trait Broker {
    fn validate(&self, order: &Order) -> Result<(), Box<dyn std::error::Error>>;
    fn execute(&self, order: &mut Order, db: &DBManager) -> Result<(), Box<dyn std::error::Error>>;
}
