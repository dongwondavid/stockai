pub mod joonwoo;
pub mod onnx_predictor; // 재활성화
pub mod dongwon;

// 공용 타입들
use crate::utility::types::api::{SharedApi, StockApi};
use crate::utility::types::broker::Order;
use crate::utility::apis;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::types::trading::{self, TradingMode};
use std::error::Error;

/// 모든 모델이 구현해야 하는 기본 trait
/// prototype.py의 model 클래스와 동일한 인터페이스
pub trait Model {
    /// 모델 시작 시 호출 - 리소스 초기화
    /// prototype.py: def __init__(self, real_api, paper_api, db_api)
    fn on_start(&mut self) -> Result<(), Box<dyn Error>>;

    /// 이벤트 발생 시 호출 - 거래 결정
    /// prototype.py처럼 다양한 API 참조를 통해 거래 결정
    /// 전역 TimeService 인스턴스를 사용
    fn on_event(
        &mut self,
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
    /// 현재 운영 모드 (실전/모의/백테스팅)
    pub current_mode: TradingMode,
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
        current_mode: TradingMode,
        real_api: SharedApi,
        paper_api: SharedApi,
        info_api: SharedApi,
        db_api: SharedApi,
    ) -> Self {
                        Self {
                    current_mode,
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
        current_mode: TradingMode,
        real_api: SharedApi,
        paper_api: SharedApi,
        info_api: SharedApi,
        db_api: SharedApi,
        backtest_api: SharedApi,
        db_api_direct: SharedApi,
    ) -> Self {
                        Self {
                    current_mode,
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

    /// 잔고 조회 (현재 모드에 따라 완벽하게 분류)
    pub fn get_balance(&self) -> StockrsResult<trading::AssetInfo> {
        match self.current_mode {
            TradingMode::Backtest => {
                // 백테스팅 모드: BacktestApi 사용
                if let Some(backtest_api) = self.get_backtest_api() {
                    backtest_api.get_balance()
                } else {
                    Err(StockrsError::BalanceInquiry {
                        reason: "백테스팅 모드에서 BacktestApi를 찾을 수 없습니다.".to_string(),
                    })
                }
            }
            TradingMode::Real => {
                // 실전투자 모드: real_api 사용 (KoreaApi 실전 API)
                self.real_api.get_balance()
            }
            TradingMode::Paper => {
                // 모의투자 모드: paper_api 사용 (KoreaApi 모의투자 API)
                self.paper_api.get_balance()
            }
        }
    }

    /// 현재 운영 모드 확인
    pub fn get_current_mode(&self) -> &TradingMode {
        &self.current_mode
    }

    /// 백테스팅 모드인지 확인
    pub fn is_backtest_mode(&self) -> bool {
        matches!(self.current_mode, TradingMode::Backtest)
    }

    /// 실전투자 모드인지 확인
    pub fn is_real_mode(&self) -> bool {
        matches!(self.current_mode, TradingMode::Real)
    }

    /// 모의투자 모드인지 확인
    pub fn is_paper_mode(&self) -> bool {
        matches!(self.current_mode, TradingMode::Paper)
    }

    /// 현재 모드에 맞는 API 반환
    pub fn get_current_api(&self) -> &SharedApi {
        match self.current_mode {
            TradingMode::Backtest => {
                // 백테스팅 모드에서는 backtest_api가 있으면 사용, 없으면 db_api 사용
                self.backtest_api.as_ref().unwrap_or(&self.db_api)
            }
            TradingMode::Real => &self.real_api,
            TradingMode::Paper => &self.paper_api,
        }
    }

    /// 실시간 현재가 조회 (실전/모의투자용)
    pub fn get_current_price(&self, stockcode: &str) -> StockrsResult<f64> {
        match self.current_mode {
            TradingMode::Backtest => {
                // 백테스팅 모드에서는 전역 TimeService의 현재 시간 사용
                let current_time = crate::time::TimeService::global_format_ymdhm()
                    .map_err(|e| StockrsError::general(format!("전역 TimeService 접근 실패: {}", e)))?;
                self.get_current_price_at_time(stockcode, &current_time)
            }
            TradingMode::Real => {
                // 실전투자 모드: real_api 사용
                self.real_api.get_current_price(stockcode)
            }
            TradingMode::Paper => {
                // 모의투자 모드: paper_api 사용
                self.paper_api.get_current_price(stockcode)
            }
        }
    }

    /// 시간 기반 현재가 조회 (백테스팅용)
    pub fn get_current_price_at_time(
        &self,
        stockcode: &str,
        time_str: &str,
    ) -> StockrsResult<f64> {
        // 백테스팅 모드에서만 사용 가능
        if !self.is_backtest_mode() {
            return Err(StockrsError::UnsupportedFeature {
                feature: "시간 기반 현재가 조회".to_string(),
                phase: format!("{:?} 모드", self.current_mode),
            });
        }

        // DbApi에 직접 접근하여 시간 기반 조회
        if let Some(db_api) = self.get_db_api() {
            db_api.get_current_price_at_time(stockcode, time_str)
        } else {
            Err(StockrsError::BalanceInquiry {
                reason: "백테스팅 모드에서 DbApi를 찾을 수 없습니다.".to_string(),
            })
        }
    }
}

// 재수출
pub use joonwoo::JoonwooModel;
pub use onnx_predictor::ONNXPredictor;
pub use dongwon::DongwonModel;
