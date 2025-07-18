use std::error::Error;
use crate::types::api::StockApi;
use crate::types::broker::{Order, OrderSide};
use crate::types::trading::AssetInfo;

/// 백테스팅용 더미 API (추후 제대로 구현 예정)
pub struct DbApi {
    // 일단 간단하게 구현
}

impl DbApi {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        println!("🔗 [DbApi] 백테스팅 API 초기화 (더미 구현)");
        Ok(DbApi {})
    }
}

impl StockApi for DbApi {
    fn execute_order(&self, order: &Order) -> Result<String, Box<dyn Error>> {
        println!("📈 [DbApi] 더미 주문 실행: {} {} {}주", 
                 order.stockcode, 
                 match order.side { OrderSide::Buy => "매수", _ => "매도" },
                 order.quantity);
        Ok("DUMMY_ORDER_001".to_string())
    }
    
    fn check_fill(&self, _order_id: &str) -> Result<bool, Box<dyn Error>> {
        Ok(true) // 즉시 체결된다고 가정
    }
    
    fn cancel_order(&self, _order_id: &str) -> Result<(), Box<dyn Error>> {
        println!("❌ [DbApi] 주문 취소 (더미)");
        Ok(())
    }
    
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>> {
        use chrono::Local;
        let balance = 10_000_000.0; // 1천만원 고정
        println!("💰 [DbApi] 잔고 조회 (더미): {}원", balance);
        Ok(AssetInfo::new(Local::now().naive_local(), balance))
    }
    
    fn get_avg_price(&self, stockcode: &str) -> Result<f64, Box<dyn Error>> {
        let price = 50000.0; // 5만원 고정
        println!("📊 [DbApi] 평균가 조회 (더미): {} -> {}원", stockcode, price);
        Ok(price)
    }
    
    fn get_current_price(&self, stockcode: &str) -> Result<f64, Box<dyn Error>> {
        let price = 52000.0; // 5만2천원 고정
        println!("📊 [DbApi] 현재가 조회 (더미): {} -> {}원", stockcode, price);
        Ok(price)
    }
} 