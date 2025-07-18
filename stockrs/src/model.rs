pub mod onnx_predictor;
// pub mod joonwoo; // 일시 비활성화 - 오류 수정 필요

// 공용 타입들
use std::error::Error;
use crate::types::broker::Order;
use crate::time::TimeService;

/// 모든 모델이 구현해야 하는 기본 trait
/// prototype.py의 model 클래스와 동일한 인터페이스
pub trait Model {
    /// 모델 시작 시 호출 - 리소스 초기화
    fn on_start(&mut self) -> Result<(), Box<dyn Error>>;
    
    /// 이벤트 발생 시 호출 - 거래 결정
    fn on_event(&mut self, time: &TimeService) -> Result<Option<Order>, Box<dyn Error>>;
    
    /// 모델 종료 시 호출 - 리소스 정리
    fn on_end(&mut self) -> Result<(), Box<dyn Error>>;
}

// 재수출
pub use onnx_predictor::ONNXPredictor;
// pub use joonwoo::JoonwooModel; // 일시 비활성화 