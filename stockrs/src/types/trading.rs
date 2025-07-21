use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

#[derive(Debug, Clone, PartialEq)]
pub enum TradingMode {
    /// 실전 투자 (정보 API 사용)
    Real,
    /// 모의 투자 (정보 API 사용, 모의 API 지원 안함)
    Paper,
    /// 백테스팅 (DB 데이터 사용)
    Backtest,
}

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

/// TradingResult 생성을 위한 파라미터 구조체
#[derive(Debug, Clone)]
pub struct TradingResultParams {
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub stockcode: String,
    pub buy_or_sell: bool,
    pub quantity: u32,
    pub price: f64,
    pub fee: f64,
    pub strategy: String,
    pub avg_price: f64,
    pub profit: f64,
    pub roi: f64,
}

/// TradingResult 생성을 위한 빌더
pub struct TradingResultBuilder {
    params: TradingResultParams,
}

impl Default for TradingResultParams {
    fn default() -> Self {
        Self {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            time: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            stockcode: String::new(),
            buy_or_sell: false,
            quantity: 0,
            price: 0.0,
            fee: 0.0,
            strategy: String::new(),
            avg_price: 0.0,
            profit: 0.0,
            roi: 0.0,
        }
    }
}

impl TradingResultParams {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for TradingResultBuilder {
    fn default() -> Self {
        Self {
            params: TradingResultParams::new(),
        }
    }
}

impl TradingResultBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn date(mut self, date: NaiveDate) -> Self {
        self.params.date = date;
        self
    }

    pub fn time(mut self, time: NaiveTime) -> Self {
        self.params.time = time;
        self
    }

    pub fn stockcode(mut self, stockcode: String) -> Self {
        self.params.stockcode = stockcode;
        self
    }

    pub fn buy_or_sell(mut self, buy_or_sell: bool) -> Self {
        self.params.buy_or_sell = buy_or_sell;
        self
    }

    pub fn quantity(mut self, quantity: u32) -> Self {
        self.params.quantity = quantity;
        self
    }

    pub fn price(mut self, price: f64) -> Self {
        self.params.price = price;
        self
    }

    pub fn fee(mut self, fee: f64) -> Self {
        self.params.fee = fee;
        self
    }

    pub fn strategy(mut self, strategy: String) -> Self {
        self.params.strategy = strategy;
        self
    }

    pub fn avg_price(mut self, avg_price: f64) -> Self {
        self.params.avg_price = avg_price;
        self
    }

    pub fn profit(mut self, profit: f64) -> Self {
        self.params.profit = profit;
        self
    }

    pub fn roi(mut self, roi: f64) -> Self {
        self.params.roi = roi;
        self
    }

    pub fn build(self) -> TradingResult {
        TradingResult::new_from_params(self.params)
    }
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

    pub fn get_date(&self) -> NaiveDateTime {
        self.date
    }
    pub fn get_asset(&self) -> f64 {
        self.asset
    }
}

impl Trading {
    pub fn new(
        date: NaiveDateTime,
        stockcode: String,
        buy_or_sell: bool,
        quantity: u32,
        price: f64,
        fee: f64,
        strategy: String,
    ) -> Self {
        Self {
            date,
            stockcode,
            buy_or_sell,
            quantity,
            price,
            fee,
            strategy,
        }
    }

    pub fn get_date(&self) -> NaiveDateTime {
        self.date
    }
    pub fn get_stockcode(&self) -> &str {
        &self.stockcode
    }
    pub fn get_buy_or_sell(&self) -> bool {
        self.buy_or_sell
    }
    pub fn get_quantity(&self) -> u32 {
        self.quantity
    }
    pub fn get_price(&self) -> f64 {
        self.price
    }
    pub fn get_fee(&self) -> f64 {
        self.fee
    }
    pub fn get_strategy(&self) -> &str {
        &self.strategy
    }

    pub fn to_trading_result(&self, avg_price: f64) -> TradingResult {
        let profit = match self.buy_or_sell {
            true => -self.fee, // 매수면 손실금 = 수수료
            false => (self.price - avg_price) * self.quantity as f64 - self.fee, // 매도면 손익금 = (매도가 - 평균매입가) * 수량 - 수수료
        };
        let roi = profit / (avg_price * self.quantity as f64) * 100.0;
        TradingResultBuilder::new()
            .date(self.date.date())
            .time(self.date.time())
            .stockcode(self.stockcode.clone())
            .buy_or_sell(self.buy_or_sell)
            .quantity(self.quantity)
            .price(self.price)
            .fee(self.fee)
            .strategy(self.strategy.clone())
            .avg_price(avg_price)
            .profit(profit)
            .roi(roi)
            .build()
    }
}

impl TradingResult {
    /// 내부 구현 함수
    fn new_from_params(params: TradingResultParams) -> Self {
        Self {
            date: params.date,
            time: params.time,
            stockcode: params.stockcode,
            buy_or_sell: params.buy_or_sell,
            quantity: params.quantity,
            price: params.price,
            fee: params.fee,
            strategy: params.strategy,
            avg_price: params.avg_price,
            profit: params.profit,
            roi: params.roi,
        }
    }

    // Getter methods
    pub fn get_date(&self) -> NaiveDate {
        self.date
    }
    pub fn get_time(&self) -> NaiveTime {
        self.time
    }
    pub fn get_stockcode(&self) -> &str {
        &self.stockcode
    }
    pub fn get_buy_or_sell(&self) -> bool {
        self.buy_or_sell
    }
    pub fn get_quantity(&self) -> u32 {
        self.quantity
    }
    pub fn get_price(&self) -> f64 {
        self.price
    }
    pub fn get_fee(&self) -> f64 {
        self.fee
    }
    pub fn get_strategy(&self) -> &str {
        &self.strategy
    }
    pub fn get_avg_price(&self) -> f64 {
        self.avg_price
    }
    pub fn get_profit(&self) -> f64 {
        self.profit
    }
    pub fn get_roi(&self) -> f64 {
        self.roi
    }

    // Convert stockcode to string (now just returns the string directly)
    pub fn get_stockcode_string(&self) -> String {
        self.stockcode.clone()
    }

    // Convert buy_or_sell boolean to string
    pub fn get_buy_or_sell_string(&self) -> String {
        if self.buy_or_sell {
            "buy".to_string()
        } else {
            "sell".to_string()
        }
    }

    // Return tuple for database insertion
    pub fn to_db_tuple(
        &self,
    ) -> (
        String,
        String,
        String,
        String,
        u32,
        f64,
        f64,
        String,
        f64,
        f64,
        f64,
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
            self.get_roi(),
        )
    }
}
