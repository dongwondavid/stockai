use std::error::Error;
use crate::types::api::StockApi;
use crate::types::broker::{Order, OrderSide};
use crate::types::trading::AssetInfo;

#[derive(Debug, Clone, Copy)]
pub enum ApiMode {
    Real,
    Paper,
}

/// 한국투자증권 API 구현
pub struct KoreaApi {
    mode: ApiMode,
}

impl KoreaApi {
    pub fn new_real() -> Result<Self, Box<dyn Error>> {
        Self::new(ApiMode::Real)
    }
    
    pub fn new_paper() -> Result<Self, Box<dyn Error>> {
        Self::new(ApiMode::Paper)
    }
    
    fn new(mode: ApiMode) -> Result<Self, Box<dyn Error>> {
        Ok(Self { mode })
    }
    
    fn mode_name(&self) -> &'static str {
        match self.mode {
            ApiMode::Real => "실거래",
            ApiMode::Paper => "모의투자",
        }
    }
}

impl StockApi for KoreaApi {
    fn execute_order(&self, _order: &Order) -> Result<String, Box<dyn Error>> {
        todo!("한국투자증권 API 주문 실행 구현")
    }
    
    fn check_fill(&self, _order_id: &str) -> Result<bool, Box<dyn Error>> {
        todo!("한국투자증권 API 체결 확인 구현")
    }
    
    fn cancel_order(&self, _order_id: &str) -> Result<(), Box<dyn Error>> {
        todo!("한국투자증권 API 주문 취소 구현")
    }
    
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>> {
        todo!("한국투자증권 API 잔고 조회 구현")
    }
    
    fn get_avg_price(&self, _stockcode: &str) -> Result<f64, Box<dyn Error>> {
        todo!("한국투자증권 API 평균가 조회 구현")
    }
    
    fn get_current_price(&self, _stockcode: &str) -> Result<f64, Box<dyn Error>> {
        todo!("한국투자증권 API 현재가 조회 구현")
    }
} 