use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

pub struct Trading {
    date: NaiveDateTime,
    stockcode: String,
    buy_or_sell: bool,
    quantity: u32,
    price: f64,
    fee: f64,
    strategy: String,
}

pub struct TradingResult {
    date: NaiveDate,
    time: NaiveTime,
    stockcode: String,
    buy_or_sell: bool,
    quantity: u32,
    price: f64,
    fee: f64,
    strategy: String,
    avg_price: f64,
    profit: f64,
    roi: f64,
}

pub struct AssetInfo {
    date: NaiveDateTime,
    asset: f64,
}

/*
------------------- impl -------------------
*/

impl AssetInfo {
    pub fn new(date: NaiveDateTime, asset: f64) -> Self {
        Self { date, asset }
    }

    pub fn get_date(&self) -> NaiveDateTime { self.date }
    pub fn get_asset(&self) -> f64 { self.asset }
}

impl Trading {
    pub fn new(date: NaiveDateTime, stockcode: String, buy_or_sell: bool, quantity: u32, price: f64, fee: f64, strategy: String) -> Self {
        Self { date, stockcode, buy_or_sell, quantity, price, fee, strategy }
    }

    pub fn get_date(&self) -> NaiveDateTime { self.date }
    pub fn get_stockcode(&self) -> &str { &self.stockcode }
    pub fn get_buy_or_sell(&self) -> bool { self.buy_or_sell }
    pub fn get_quantity(&self) -> u32 { self.quantity }
    pub fn get_price(&self) -> f64 { self.price }
    pub fn get_fee(&self) -> f64 { self.fee }
    pub fn get_strategy(&self) -> &str { &self.strategy }

    pub fn to_trading_result(&self, avg_price: f64) -> TradingResult {
        let profit = match self.buy_or_sell {
            true => -self.fee as f64, // 매수면 손실금 = 수수료 
            false => (self.price - avg_price) * self.quantity as f64 - self.fee as f64, // 매도면 손익금 = (매도가 - 평균매입가) * 수량 - 수수료
        };
        let roi = profit / (avg_price * self.quantity as f64) * 100.0;
        TradingResult::new(self.date.date(), self.date.time(), self.stockcode.clone(), self.buy_or_sell, self.quantity, self.price, self.fee, self.strategy.clone(), avg_price, profit, roi)
    }
}

impl TradingResult {
    pub fn new(date: NaiveDate, time: NaiveTime, stockcode: String, buy_or_sell: bool, quantity: u32, price: f64, fee: f64, strategy: String, avg_price: f64, profit: f64, roi: f64) -> Self {
        Self { date, time, stockcode, buy_or_sell, quantity, price, fee, strategy, avg_price, profit, roi }
    }

    // Getter methods
    pub fn get_date(&self) -> NaiveDate { self.date }
    pub fn get_time(&self) -> NaiveTime { self.time }
    pub fn get_stockcode(&self) -> &str { &self.stockcode }
    pub fn get_buy_or_sell(&self) -> bool { self.buy_or_sell }
    pub fn get_quantity(&self) -> u32 { self.quantity }
    pub fn get_price(&self) -> f64 { self.price }
    pub fn get_fee(&self) -> f64 { self.fee }
    pub fn get_strategy(&self) -> &str { &self.strategy }
    pub fn get_avg_price(&self) -> f64 { self.avg_price }
    pub fn get_profit(&self) -> f64 { self.profit }
    pub fn get_roi(&self) -> f64 { self.roi }

    // Convert stockcode to string (now just returns the string directly)
    pub fn get_stockcode_string(&self) -> String {
        self.stockcode.clone()
    }

    // Convert buy_or_sell boolean to string
    pub fn get_buy_or_sell_string(&self) -> String {
        if self.buy_or_sell { "buy".to_string() } else { "sell".to_string() }
    }

    // Return tuple for database insertion
    pub fn to_db_tuple(&self) -> (
        String, String, String, String, u32, f64, f64, String, f64, f64, f64
    ) {
        (
            self.get_date().to_string(),
            self.get_time().to_string(),
            self.get_stockcode_string(),
            self.get_buy_or_sell_string(),
            self.get_quantity(),
            self.get_price(),
            self.get_fee(),
            self.get_strategy().to_string(),
            self.get_avg_price(),
            self.get_profit(),
            self.get_roi()
        )
    }
} 