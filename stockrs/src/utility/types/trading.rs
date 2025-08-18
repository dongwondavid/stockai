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
            date: NaiveDate::from_ymd_opt(2024, 1, 1)
                .expect("Invalid default date"),
            time: NaiveTime::from_hms_opt(9, 0, 0)
                .expect("Invalid default time"),
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
    asset: f64,                    // 기존 호환성을 위한 총 자산
    available_amount: f64,         // 주문 가능 금액 (D+2 예수금)
    securities_value: f64,         // 유가증권 평가금액
    total_asset: f64,              // 실제 총 자산 (API에서 제공하는 값)
}

/*
------------------- impl -------------------
*/

impl AssetInfo {
    pub fn new(date: NaiveDateTime, asset: f64) -> Self {
        // 기존 호환성을 위해 asset을 총 자산으로 간주하고, 주문가능금액과 유가증권평가금액을 추정
        Self { 
            date, 
            asset, 
            available_amount: asset,      // 기본값으로 총 자산을 주문가능금액으로 설정
            securities_value: 0.0,        // 유가증권평가금액은 별도 설정 필요
            total_asset: asset            // 기본값으로 asset을 총 자산으로 설정
        }
    }

    /// 주문가능금액과 유가증권평가금액을 모두 포함한 총 자산 생성
    pub fn new_with_stocks(date: NaiveDateTime, available_amount: f64, securities_value: f64) -> Self {
        let calculated_total = available_amount + securities_value;
        Self { 
            date, 
            asset: calculated_total,           // 기존 호환성을 위해 asset 필드도 설정
            available_amount, 
            securities_value,
            total_asset: calculated_total      // 계산된 총 자산
        }
    }

    /// API에서 제공하는 총평가금액을 포함한 생성자
    pub fn new_with_api_total(date: NaiveDateTime, available_amount: f64, securities_value: f64, api_total: f64) -> Self {
        Self { 
            date, 
            asset: api_total,                 // API 총평가금액을 asset으로 설정
            available_amount, 
            securities_value,
            total_asset: api_total            // API에서 제공하는 총평가금액
        }
    }

    pub fn get_date(&self) -> NaiveDateTime {
        self.date
    }

    /// 기존 호환성을 위한 메서드 - 총 자산 반환
    pub fn get_asset(&self) -> f64 {
        self.asset
    }

    /// 주문 가능 금액 반환 (D+2 예수금)
    pub fn get_available_amount(&self) -> f64 {
        self.available_amount
    }

    /// 유가증권 평가금액 반환
    pub fn get_securities_value(&self) -> f64 {
        self.securities_value
    }

    /// 총 자산 반환 (API에서 제공하는 값 또는 계산된 값)
    pub fn get_total_asset(&self) -> f64 {
        self.total_asset
    }

    // 기존 호환성을 위한 별칭 메서드들
    pub fn get_cash(&self) -> f64 {
        self.available_amount
    }

    pub fn get_stock_value(&self) -> f64 {
        self.securities_value
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
