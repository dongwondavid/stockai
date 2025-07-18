use std::error::Error;
use crate::types::api::StockApi;
use crate::types::broker::{Order, OrderSide};
use crate::types::trading::AssetInfo;

#[derive(Debug, Clone, Copy)]
pub enum ApiMode {
    Real,
    Paper,
}

/// 한국투자증권 API 더미 구현 (추후 제대로 구현 예정)
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
        let mode_name = match mode {
            ApiMode::Real => "실거래",
            ApiMode::Paper => "모의투자",
        };
        println!("🔗 [KoreaApi] {} API 초기화 (더미 구현)", mode_name);
        
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
    fn execute_order(&self, order: &Order) -> Result<String, Box<dyn Error>> {
        let order_id = format!("{}_{}", self.mode_name(), chrono::Utc::now().timestamp());
        
        println!("📈 [{}Api] 더미 주문 실행: {} {} {}주 -> 주문번호: {}", 
                 self.mode_name(),
                 order.stockcode, 
                 match order.side { OrderSide::Buy => "매수", _ => "매도" },
                 order.quantity, 
                 order_id);
        
        Ok(order_id)
    }
    
    fn check_fill(&self, order_id: &str) -> Result<bool, Box<dyn Error>> {
        println!("🔍 [{}Api] 더미 체결 확인: {} -> 체결됨", self.mode_name(), order_id);
        Ok(true)
    }
    
    fn cancel_order(&self, order_id: &str) -> Result<(), Box<dyn Error>> {
        println!("❌ [{}Api] 더미 주문 취소: {}", self.mode_name(), order_id);
        Ok(())
    }
    
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>> {
        use chrono::Local;
        let balance = match self.mode {
            ApiMode::Real => 5_000_000.0,  // 실거래: 500만원
            ApiMode::Paper => 10_000_000.0, // 모의투자: 1천만원
        };
        
        println!("💰 [{}Api] 더미 잔고 조회: {}원", self.mode_name(), balance);
        Ok(AssetInfo::new(Local::now().naive_local(), balance))
    }
    
    fn get_avg_price(&self, stockcode: &str) -> Result<f64, Box<dyn Error>> {
        let price = 48000.0; // 4만8천원 고정
        println!("📊 [{}Api] 더미 평균가 조회: {} -> {}원", self.mode_name(), stockcode, price);
        Ok(price)
    }
    
    fn get_current_price(&self, stockcode: &str) -> Result<f64, Box<dyn Error>> {
        let price = 49500.0; // 4만9천5백원 고정  
        println!("📊 [{}Api] 더미 현재가 조회: {} -> {}원", self.mode_name(), stockcode, price);
        Ok(price)
    }
} 