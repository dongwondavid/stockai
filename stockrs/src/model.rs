use std::error::Error;
use crate::time::TimeService;
use crate::types::broker::Order;

/// 모델이 구현해야 하는 기본 trait
/// prototype.py의 model 클래스와 동일한 생명주기 패턴
pub trait Model {
    /// 모델 시작 시 호출 (초기화, 리소스 로딩 등)
    fn on_start(&mut self) -> Result<(), Box<dyn Error>>;

    /// 시간 이벤트 발생 시 호출되는 메인 로직
    /// time 정보를 받아서 주문을 생성하거나 None 반환
    /// prototype.py: result = self.model.on_event(self.time)
    fn on_event(&mut self, time: &TimeService) -> Result<Option<Order>, Box<dyn Error>>;

    /// 모델 종료 시 호출 (리소스 정리, 상태 저장 등)
    fn on_end(&mut self) -> Result<(), Box<dyn Error>>;
}
