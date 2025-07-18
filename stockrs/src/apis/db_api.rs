use std::error::Error;
use crate::types::api::StockApi;
use crate::types::broker::{Order, OrderSide};
use crate::types::trading::AssetInfo;

/// 백테스팅용 DB API
pub struct DbApi {
    // TODO: 실제 DB 연결 구현 필요
}

impl DbApi {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(DbApi {})
    }
}

impl StockApi for DbApi {
    fn execute_order(&self, _order: &Order) -> Result<String, Box<dyn Error>> {
        todo!("백테스팅 DB API 주문 실행 구현")
    }
    
    fn check_fill(&self, _order_id: &str) -> Result<bool, Box<dyn Error>> {
        todo!("백테스팅 DB API 체결 확인 구현")
    }
    
    fn cancel_order(&self, _order_id: &str) -> Result<(), Box<dyn Error>> {
        todo!("백테스팅 DB API 주문 취소 구현")
    }
    
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>> {
        todo!("백테스팅 DB API 잔고 조회 구현")
    }
    
    fn get_avg_price(&self, _stockcode: &str) -> Result<f64, Box<dyn Error>> {
        todo!("백테스팅 DB API 평균가 조회 구현")
    }
    
    fn get_current_price(&self, _stockcode: &str) -> Result<f64, Box<dyn Error>> {
        todo!("백테스팅 DB API 현재가 조회 구현")
    }
} 