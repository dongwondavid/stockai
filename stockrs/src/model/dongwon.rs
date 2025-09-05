use crate::model::{ApiBundle, Model};
use crate::time::TimeService;
use crate::utility::config::get_config;
use crate::utility::types::broker::{Order, OrderSide};
use chrono::Timelike;
use chrono::Local;
use std::error::Error;
use tracing::{debug, info, warn};
use crate::utility::apis::KoreaApi;

pub struct DongwonModel {
    stockcode: String,
    entry_time: String, // HH:MM:SS
    exit_time: String,  // HH:MM:SS
    quantity: u32,
    holding_qty: u32,
}

impl Default for DongwonModel {
    fn default() -> Self {
        // 기본값: 삼성전자, 09:10 매수, 15:15 전량 매도, 1주
        Self {
            stockcode: "005930".to_string(),
            entry_time: "13:44:00".to_string(),
            exit_time: "13:46:00".to_string(),
            quantity: 100,
            holding_qty: 0,
        }
    }
}

impl DongwonModel {
    pub fn new() -> Self {
        // 설정에서 로드하되, 없으면 기본값으로 동작
        let mut s = Self::default();
        if let Ok(cfg) = get_config() {
            if !cfg.dongwon.stockcode.is_empty() {
                s.stockcode = cfg.dongwon.stockcode.clone();
            }
            if !cfg.dongwon.entry_time.is_empty() {
                s.entry_time = cfg.dongwon.entry_time.clone();
            }
            if !cfg.dongwon.exit_time.is_empty() {
                s.exit_time = cfg.dongwon.exit_time.clone();
            }
            if cfg.dongwon.quantity > 0 {
                s.quantity = cfg.dongwon.quantity;
            }
        }
        s
    }

    fn parse_hm(s: &str) -> Result<(u32, u32), Box<dyn Error>> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err(format!("잘못된 시간 형식: {}", s).into());
        }
        let h = parts[0].parse::<u32>()?;
        let m = parts[1].parse::<u32>()?;
        Ok((h, m))
    }

    fn try_entry(&mut self, apis: &ApiBundle) -> Result<Option<Order>, Box<dyn Error>> {
        if self.holding_qty > 0 {
            return Ok(None);
        }
        let now = TimeService::global_now()?;
        let (eh, em) = Self::parse_hm(&self.entry_time)?;
        if now.hour() == eh && now.minute() == em {
            let price = apis.get_current_price(&self.stockcode)?;
            let qty = self.quantity;
            if qty == 0 {
                warn!("[dongwon] 수량이 0입니다 - 주문 생략");
                return Ok(None);
            }
            let order = Order {
                date: now.naive_local(),
                stockcode: self.stockcode.clone(),
                side: OrderSide::Buy,
                quantity: qty,
                price,
                fee: 0.0,
                strategy: "dongwon_entry".to_string(),
            };
            self.holding_qty = qty;
            info!("[dongwon] 매수 주문 생성: {} {}주 @{:.0}", self.stockcode, qty, price);
            return Ok(Some(order));
        }
        Ok(None)
    }

    fn try_exit(&mut self, apis: &ApiBundle) -> Result<Option<Order>, Box<dyn Error>> {
        if self.holding_qty == 0 {
            return Ok(None);
        }
        let now = TimeService::global_now()?;
        let (xh, xm) = Self::parse_hm(&self.exit_time)?;
        if now.hour() == xh && now.minute() == xm {
            let price = apis.get_current_price(&self.stockcode)?;
            let qty = self.holding_qty;
            let order = Order {
                date: now.naive_local(),
                stockcode: self.stockcode.clone(),
                side: OrderSide::Sell,
                quantity: qty,
                price,
                fee: 0.0,
                strategy: "dongwon_exit".to_string(),
            };
            self.holding_qty = 0;
            info!("[dongwon] 전량 매도 주문 생성: {} {}주 @{:.0}", self.stockcode, qty, price);
            return Ok(Some(order));
        }
        Ok(None)
    }
}

impl Model for DongwonModel {
    fn on_start(&mut self) -> Result<(), Box<dyn Error>> {
        info!(
            "🚀 [dongwon] 시작 - 종목: {}, 매수: {}, 매도: {}, 수량: {}",
            self.stockcode, self.entry_time, self.exit_time, self.quantity
        );
        // 시작 시점에 정보 API로 거래대금 상위 30종목 조회 및 출력
        let api = KoreaApi::new_info()?;
        // 모델 시작 시 오전 5분봉 집계 수행
        let today = Local::now().format("%Y%m%d").to_string();
        match api.get_morning_5min_ohlcv(&self.stockcode, &today) {
            Ok((closes, _opens, _highs, _lows, _volumes)) => {
                println!("closes: {:?}", closes);
                println!(
                    "[dongwon] {} {} 오전 5분봉 집계 완료: bars={}",
                    self.stockcode,
                    today,
                    closes.len()
                );
            }
            Err(e) => {
                warn!("[dongwon] 오전 5분봉 집계 실패: {} {} - {}", self.stockcode, today, e);
            }
        }
        let top = api.get_top_amount_stocks(30)?;
        println!("[dongwon] 시작시 거래대금 상위 30: {}", top.join(", "));

        // 최상위 종목의 분봉 중 '오늘' 데이터만 출력 (09:00:00부터, 과거 포함 조회 후 필터링)
        if let Some(best) = top.first() {
            let minutes = api.get_minute_price_chart(best, "090000", true)?;
            let todays_minutes: Vec<_> = minutes
                .into_iter()
                .filter(|(d, _, _, _, _, _, _, _)| d == &today)
                .collect();
            println!("[dongwon] {} 분봉(오늘 {}개):", best, todays_minutes.len());
            for (date, time, close, open, high, low, volume, amount) in todays_minutes.iter() {
                println!(
                    "  {} {} O:{:.0} H:{:.0} L:{:.0} C:{:.0} V:{:.0} A:{:.0}",
                    date, time, open, high, low, close, volume, amount
                );
            }
        }
        Ok(())
    }

    fn on_event(&mut self, apis: &ApiBundle) -> Result<Option<Order>, Box<dyn Error>> {
        // 우선 매도 시점 확인 (우선순위 높음)
        if let Some(o) = self.try_exit(apis)? { return Ok(Some(o)); }
        // 그 다음 매수 시점 확인
        if let Some(o) = self.try_entry(apis)? { return Ok(Some(o)); }
        debug!("[dongwon] 동작 없음");
        Ok(None)
    }

    fn on_end(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🏁 [dongwon] 종료");
        Ok(())
    }

    fn reset_for_new_day(&mut self) -> Result<(), Box<dyn Error>> {
        self.holding_qty = 0;
        info!("🔄 [dongwon] 새로운 거래일 리셋");
        Ok(())
    }
}


