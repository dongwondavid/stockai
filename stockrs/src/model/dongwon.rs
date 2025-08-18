use crate::model::{ApiBundle, Model};
use crate::time::TimeService;
use crate::utility::config::get_config;
use crate::utility::types::broker::{Order, OrderSide};
use chrono::Timelike;
use std::error::Error;
use tracing::{debug, info, warn};

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


