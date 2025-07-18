use std::error::Error;
use chrono::NaiveDateTime;
use crate::types::broker::Order;
use crate::types::trading::AssetInfo;

/// 실행 환경 타입 - prototype.py의 self.type과 동일
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApiType {
    Real,      // "real"
    Paper,     // "paper" 
    Backtest,  // "backtest"
}

/// 모든 API가 구현해야 하는 기본 trait
/// prototype.py의 real_api, paper_api, db_api가 동일한 인터페이스를 가지는 것처럼
pub trait StockApi {
    /// 주문 실행
    fn execute_order(&self, order: &Order) -> Result<String, Box<dyn Error>>;
    
    /// 주문 체결 확인  
    fn check_fill(&self, order_id: &str) -> Result<bool, Box<dyn Error>>;
    
    /// 주문 취소
    fn cancel_order(&self, order_id: &str) -> Result<(), Box<dyn Error>>;
    
    /// 잔고 조회 (한국투자증권 API 기준)
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>>;
}

/// API 팩토리 함수 - prototype.py의 조건부 생성 로직과 동일
pub fn create_api(api_type: ApiType, for_trading: bool) -> Box<dyn StockApi> {
    match (api_type, for_trading) {
        (ApiType::Real, true) => Box::new(RealApi),
        (ApiType::Paper, true) => Box::new(PaperApi),
        (ApiType::Backtest, _) => Box::new(DbApi),
        (_, false) => Box::new(DbApi), // data-only operations
    }
}

/// 실제 한국투자증권 API 구현체
pub struct RealApi;

impl StockApi for RealApi {
    fn execute_order(&self, order: &Order) -> Result<String, Box<dyn Error>> {
        // TODO: 실제 한국투자증권 API 호출
        todo!("실제 API 주문 실행")
    }
    
    fn check_fill(&self, order_id: &str) -> Result<bool, Box<dyn Error>> {
        // TODO: 실제 API로 체결 확인
        todo!("실제 API 체결 확인")
    }
    
    fn cancel_order(&self, order_id: &str) -> Result<(), Box<dyn Error>> {
        // TODO: 실제 API로 주문 취소
        todo!("실제 API 주문 취소")
    }
    
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>> {
        // TODO: 실제 API로 잔고 조회
        todo!("실제 API 잔고 조회")
    }
}

/// 모의투자 API 구현체
pub struct PaperApi;

impl StockApi for PaperApi {
    fn execute_order(&self, order: &Order) -> Result<String, Box<dyn Error>> {
        // TODO: 모의투자 API 호출
        todo!("모의투자 API 주문 실행")
    }
    
    fn check_fill(&self, order_id: &str) -> Result<bool, Box<dyn Error>> {
        // TODO: 모의투자 API로 체결 확인
        todo!("모의투자 API 체결 확인")
    }
    
    fn cancel_order(&self, order_id: &str) -> Result<(), Box<dyn Error>> {
        // TODO: 모의투자 API로 주문 취소
        todo!("모의투자 API 주문 취소")
    }
    
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>> {
        // TODO: 모의투자 API로 잔고 조회
        todo!("모의투자 API 잔고 조회")
    }
}

/// 백테스팅용 DB API 구현체
pub struct DbApi;

impl StockApi for DbApi {
    fn execute_order(&self, _order: &Order) -> Result<String, Box<dyn Error>> {
        // 더미 구현: 항상 성공하는 주문
        println!("🔹 [DbApi] Order executed successfully (simulated)");
        Ok("DUMMY_ORDER_123".to_string())
    }
    
    fn check_fill(&self, _order_id: &str) -> Result<bool, Box<dyn Error>> {
        // 더미 구현: 항상 체결됨
        println!("🔹 [DbApi] Order filled: {} (simulated)", _order_id);
        Ok(true)
    }
    
    fn cancel_order(&self, _order_id: &str) -> Result<(), Box<dyn Error>> {
        // 더미 구현: 항상 취소 성공
        println!("🔹 [DbApi] Order cancelled: {} (simulated)", _order_id);
        Ok(())
    }
    
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>> {
        // 더미 구현: 가상의 잔고 정보
        use chrono::Local;
        println!("🔹 [DbApi] Balance retrieved (simulated)");
        Ok(AssetInfo::new(Local::now().naive_local(), 1000000.0))
    }
} 
