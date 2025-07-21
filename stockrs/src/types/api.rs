use crate::errors::StockrsResult;
use crate::types::broker::Order;
use crate::types::trading::AssetInfo;
use std::any::Any;

/// 단일 스레드용 공유 API 타입 별칭
/// 나중에 멀티스레드가 필요할 때는 Arc<Mutex<dyn StockApi + Send + Sync>>로 변경
pub type SharedApi = std::rc::Rc<dyn StockApi>;

/// 모든 API 구현체가 구현해야 하는 기본 trait
/// prototype.py의 API 인터페이스와 동일
pub trait StockApi: Any {
    /// Any trait의 as_any 메서드 제공 (trait object에서 사용 가능)
    fn as_any(&self) -> &dyn Any;
    /// 주문 실행
    ///
    /// # 인수
    /// * `order` - 실행할 주문 정보
    ///
    /// # 반환값
    /// * `Ok(String)` - 주문번호
    /// * `Err(StockrsError)` - 주문 실행 실패 시 구체적인 오류
    fn execute_order(&self, order: &mut Order) -> StockrsResult<String>;

    /// 주문 체결 확인  
    ///
    /// # 인수
    /// * `order_id` - 확인할 주문번호
    ///
    /// # 반환값
    /// * `Ok(bool)` - true: 체결됨, false: 미체결
    /// * `Err(StockrsError)` - 체결 확인 실패 시 구체적인 오류
    fn check_fill(&self, order_id: &str) -> StockrsResult<bool>;

    /// 주문 취소
    ///
    /// # 인수
    /// * `order_id` - 취소할 주문번호
    ///
    /// # 반환값
    /// * `Ok(())` - 취소 성공
    /// * `Err(StockrsError)` - 취소 실패 시 구체적인 오류
    fn cancel_order(&self, order_id: &str) -> StockrsResult<()>;

    /// 잔고 조회 (한국투자증권 API 기준)
    ///
    /// # 반환값
    /// * `Ok(AssetInfo)` - 현재 잔고 정보
    /// * `Err(StockrsError)` - 잔고 조회 실패 시 구체적인 오류
    fn get_balance(&self) -> StockrsResult<AssetInfo>;

    /// 평균가 조회 (data_reader 역할 통합)
    ///
    /// # 인수
    /// * `stockcode` - 조회할 종목코드 (예: "005930")
    ///
    /// # 반환값
    /// * `Ok(f64)` - 평균 단가
    /// * `Err(StockrsError)` - 평균가 조회 실패 시 구체적인 오류
    fn get_avg_price(&self, stockcode: &str) -> StockrsResult<f64>;

    /// 현재가 조회
    ///
    /// # 인수
    /// * `stockcode` - 조회할 종목코드 (예: "005930")
    ///
    /// # 반환값
    /// * `Ok(f64)` - 현재 가격
    /// * `Err(StockrsError)` - 현재가 조회 실패 시 구체적인 오류
    fn get_current_price(&self, stockcode: &str) -> StockrsResult<f64>;

    /// 특정 시간의 현재가 조회 (백테스팅용)
    ///
    /// # 인수
    /// * `stockcode` - 조회할 종목코드
    /// * `time_str` - 조회할 시간 (YYYYMMDDHHMM 형식)
    ///
    /// # 반환값
    /// * `Ok(f64)` - 해당 시간의 가격
    /// * `Err(StockrsError)` - 가격 조회 실패 시 구체적인 오류
    fn get_current_price_at_time(&self, stockcode: &str, time_str: &str) -> StockrsResult<f64>;

    /// 현재 시간 설정 (백테스팅용)
    ///
    /// # 인수
    /// * `time_str` - 설정할 시간 (YYYYMMDDHHMM 형식)
    ///
    /// # 반환값
    /// * `Ok(())` - 설정 성공
    /// * `Err(StockrsError)` - 설정 실패 시 구체적인 오류
    fn set_current_time(&self, time_str: &str) -> StockrsResult<()>;

    /// DB 연결 가져오기 (백테스팅용)
    ///
    /// # 반환값
    /// * `Ok(Connection)` - SQLite 연결
    /// * `Err(StockrsError)` - 연결 실패 시 구체적인 오류
    fn get_db_connection(&self) -> Option<rusqlite::Connection>;

    /// 일봉 DB 연결 가져오기 (백테스팅용)
    ///
    /// # 반환값
    /// * `Ok(Connection)` - 일봉 SQLite 연결
    /// * `Err(StockrsError)` - 연결 실패 시 구체적인 오류
    fn get_daily_db_connection(&self) -> Option<rusqlite::Connection>;
}

/// API 타입 구분
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApiType {
    Real,     // 실전 거래
    Paper,    // 모의투자
    Backtest, // 백테스팅
}
