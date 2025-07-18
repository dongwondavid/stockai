use std::error::Error;
use crate::db_manager::DBManager;
use crate::types::broker::{Broker, Order};
use crate::types::api::StockApi;

/// 통합된 Broker 구현체
/// prototype.py의 broker(broker_api) 패턴과 동일
pub struct StockBroker {
    api: Box<dyn StockApi>,
}

impl StockBroker {
    pub fn new(api: Box<dyn StockApi>) -> Self {
        Self { api }
    }
}

impl Broker for StockBroker {
    fn validate(&self, _order: &Order) -> Result<(), Box<dyn Error>> {
        // TODO: 주문 유효성 검증 로직
        Ok(())
    }

    fn execute(&self, order: &Order, db: &DBManager) -> Result<(), Box<dyn Error>> {
        self.validate(order)?;
        
        // API를 통한 주문 실행
        let order_id = self.api.execute_order(order)?;
        
        // 체결 확인
        let filled = self.api.check_fill(&order_id)?;
        
        if filled {
            // 거래 결과를 DB에 저장
            db.save_trading(order.to_trading())?;
        } else {
            // 미체결 시 주문 취소
            self.api.cancel_order(&order_id)?;
        }
        
        Ok(())
    }
}

/// 생명주기 패턴 추가 - prototype.py와 동일
impl StockBroker {
    /// broker 시작 시 호출
    pub fn on_start(&mut self) -> Result<(), Box<dyn Error>> {
        // TODO: broker 초기화 로직
        Ok(())
    }

    /// broker 이벤트 처리 - prototype.py의 broker.on_event(result)와 동일
    pub fn on_event(&mut self, order: Order, db: &DBManager) -> Result<(), Box<dyn Error>> {
        self.execute(&order, db)
    }

    /// broker 종료 시 호출
    pub fn on_end(&mut self) -> Result<(), Box<dyn Error>> {
        // TODO: broker 정리 로직
        Ok(())
    }
}
