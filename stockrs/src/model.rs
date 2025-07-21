pub mod joonwoo;
pub mod onnx_predictor; // 재활성화

// 공용 타입들
use crate::time::TimeService;
use crate::utility::types::api::{SharedApi, StockApi};
use crate::utility::types::broker::Order;
use crate::utility::apis;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::types::trading;
use std::error::Error;

/// 모든 모델이 구현해야 하는 기본 trait
/// prototype.py의 model 클래스와 동일한 인터페이스
pub trait Model {
    /// 모델 시작 시 호출 - 리소스 초기화
    /// prototype.py: def __init__(self, real_api, paper_api, db_api)
    fn on_start(&mut self) -> Result<(), Box<dyn Error>>;

    /// 이벤트 발생 시 호출 - 거래 결정
    /// prototype.py처럼 다양한 API 참조를 통해 거래 결정
    fn on_event(
        &mut self,
        time: &TimeService,
        apis: &ApiBundle,
    ) -> Result<Option<Order>, Box<dyn Error>>;

    /// 모델 종료 시 호출 - 리소스 정리
    fn on_end(&mut self) -> Result<(), Box<dyn Error>>;

    /// 매일 새로운 거래일을 위해 모델 상태 리셋
    fn reset_for_new_day(&mut self) -> Result<(), Box<dyn Error>>;
}

/// prototype.py의 API 구조를 반영한 API 번들
/// runner에서 초기화하여 모델에 전달
pub struct ApiBundle {
    pub real_api: SharedApi,
    pub paper_api: SharedApi,
    pub info_api: SharedApi,
    pub db_api: SharedApi,
    /// 백테스팅용 BacktestApi 직접 참조 (잔고 관리용)
    pub backtest_api: Option<SharedApi>,
    /// 백테스팅용 DbApi 직접 참조 (타입 안전성 보장)
    pub db_api_direct: Option<SharedApi>,
}

impl ApiBundle {
    pub fn new(
        real_api: SharedApi,
        paper_api: SharedApi,
        info_api: SharedApi,
        db_api: SharedApi,
    ) -> Self {
        Self {
            real_api,
            paper_api,
            info_api,
            db_api,
            backtest_api: None,
            db_api_direct: None,
        }
    }

    /// 백테스팅용 생성자 - BacktestApi와 DbApi 직접 참조 포함
    pub fn new_with_backtest_apis(
        real_api: SharedApi,
        paper_api: SharedApi,
        info_api: SharedApi,
        db_api: SharedApi,
        backtest_api: SharedApi,
        db_api_direct: SharedApi,
    ) -> Self {
        Self {
            real_api,
            paper_api,
            info_api,
            db_api,
            backtest_api: Some(backtest_api),
            db_api_direct: Some(db_api_direct),
        }
    }

    /// BacktestApi에 직접 접근 (백테스팅용)
    pub fn get_backtest_api(&self) -> Option<&apis::BacktestApi> {
        self.backtest_api
            .as_ref()
            .and_then(|api| api.as_any().downcast_ref::<apis::BacktestApi>())
    }

    /// DbApi에 직접 접근 (백테스팅용)
    pub fn get_db_api(&self) -> Option<&apis::DbApi> {
        // db_api를 다운캐스팅하여 직접 접근
        self.db_api.as_any().downcast_ref::<apis::DbApi>()
    }

    /// 잔고 조회 (백테스팅 모드에서는 BacktestApi 사용)
    pub fn get_balance(&self) -> StockrsResult<trading::AssetInfo> {
        if let Some(backtest_api) = self.get_backtest_api() {
            // 백테스팅 모드: BacktestApi 사용
            backtest_api.get_balance()
        } else {
            // 실시간/모의투자 모드: db_api 사용
            self.db_api.get_balance()
        }
    }

    /// 시간 기반 현재가 조회 (백테스팅용)
    pub fn get_current_price_at_time(
        &self,
        stockcode: &str,
        time_str: &str,
    ) -> StockrsResult<f64> {
        // DbApi에 직접 접근하여 시간 기반 조회
        if let Some(db_api) = self.get_db_api() {
            db_api.get_current_price_at_time(stockcode, time_str)
        } else {
            // 백테스팅 모드가 아닌 경우 에러 발생
            Err(StockrsError::UnsupportedFeature {
                feature: "시간 기반 현재가 조회".to_string(),
                phase: "실시간/모의투자 모드".to_string(),
            })
        }
    }
}

// 재수출
pub use joonwoo::JoonwooModel;
pub use onnx_predictor::ONNXPredictor;
