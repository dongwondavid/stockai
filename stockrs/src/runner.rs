use crate::utility::apis::{BacktestApi, DbApi, KoreaApi};
use crate::broker::StockBroker;
use crate::db_manager::DBManager;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::model::{ApiBundle, Model};
use crate::time::TimeService;
use crate::utility::types::api::{ApiType, SharedApi};
use crate::utility::types::trading::TradingMode;
use crate::utility::config;

use chrono::Timelike;

/// API 구성 정보
struct ApiConfig {
    broker_api: SharedApi,
    db_manager_api: SharedApi,
    api_bundle: ApiBundle,
}

/// prototype.py의 runner 클래스와 동일한 구조
pub struct Runner {
    /// "real" or "paper" or "backtest" - prototype.py의 self.type
    pub api_type: ApiType,

    /// prototype.py의 각 컴포넌트들
    pub model: Box<dyn Model>,
    pub broker: StockBroker,
    pub db_manager: DBManager,

    /// API 번들 (prototype.py의 API 구조 반영)
    pub api_bundle: ApiBundle,

    /// prototype.py의 self.stop_condition
    pub stop_condition: bool,

    /// 같은 날짜에 여러 번 "새로운 거래일 시작" 로그/리셋이 실행되지 않도록 하기 위한 가드
    last_new_day_logged: Option<chrono::NaiveDate>,
}

impl Runner {
    /// ApiType을 TradingMode로 변환
    fn api_type_to_trading_mode(api_type: ApiType) -> TradingMode {
        match api_type {
            ApiType::Real => TradingMode::Real,
            ApiType::Paper => TradingMode::Paper,
            ApiType::Backtest => TradingMode::Backtest,
        }
    }

    /// prototype.py의 __init__과 동일한 초기화 로직
    pub fn new(
        api_type: ApiType,
        model: Box<dyn Model>,
        db_path: std::path::PathBuf,
    ) -> StockrsResult<Self> {
        // TimeService 전역 초기화
        TimeService::init()
            .map_err(|e| StockrsError::general(format!("시간 서비스 초기화 실패: {}", e)))?;

        // 모드별 API 구성
        let api_config = Self::create_api_config(api_type)?;

        println!(
            "🚀 [Runner] {} 모드로 초기화 완료",
            match api_type {
                ApiType::Real => "실거래",
                ApiType::Paper => "모의투자",
                ApiType::Backtest => "백테스팅",
            }
        );

        Ok(Runner {
            api_type,
            model,
            broker: StockBroker::new(api_config.broker_api.clone()),
            db_manager: DBManager::new(db_path, api_config.db_manager_api)?,
            api_bundle: api_config.api_bundle,
            stop_condition: false,
            last_new_day_logged: None,
        })
    }

    /// 모드별 API 구성 생성
    fn create_api_config(api_type: ApiType) -> StockrsResult<ApiConfig> {
        let current_mode = Self::api_type_to_trading_mode(api_type);
        
        match api_type {
            ApiType::Real => {
                // 실전: 정보 API + 실전 API + DB API
                let real_api: SharedApi = std::rc::Rc::new(KoreaApi::new_real()?);
                let info_api: SharedApi = std::rc::Rc::new(KoreaApi::new_info()?);
                let db_api: SharedApi = std::rc::Rc::new(DbApi::new()?);

                let api_bundle = ApiBundle::new(
                    current_mode,
                    real_api.clone(),
                    real_api.clone(),
                    info_api,
                    db_api.clone(),
                );

                Ok(ApiConfig {
                    broker_api: real_api.clone(),
                    db_manager_api: real_api, // 실전투자에서는 real_api 사용 (잔고 조회용)
                    api_bundle,
                })
            }
            ApiType::Paper => {
                // 모의: 정보 API + 모의 API + DB API
                let paper_api: SharedApi = std::rc::Rc::new(KoreaApi::new_paper()?);
                let info_api: SharedApi = std::rc::Rc::new(KoreaApi::new_info()?);
                let db_api: SharedApi = std::rc::Rc::new(DbApi::new()?);

                let api_bundle = ApiBundle::new(
                    current_mode,
                    paper_api.clone(),
                    paper_api.clone(),
                    info_api,
                    db_api.clone(),
                );

                Ok(ApiConfig {
                    broker_api: paper_api.clone(),
                    db_manager_api: paper_api, // 모의투자에서는 paper_api 사용 (잔고 조회용)
                    api_bundle,
                })
            }
            ApiType::Backtest => {
                // 백테스팅: DB API + 백테스팅 API + DB API
                let db_api: SharedApi = std::rc::Rc::new(DbApi::new()?);
                let backtest_api: SharedApi = std::rc::Rc::new(BacktestApi::new(db_api.clone())?);

                let api_bundle = ApiBundle::new_with_backtest_apis(
                    current_mode,
                    backtest_api.clone(),
                    backtest_api.clone(),
                    backtest_api.clone(),
                    db_api.clone(),
                    backtest_api.clone(),
                    db_api.clone(),
                );

                Ok(ApiConfig {
                    broker_api: backtest_api.clone(),
                    db_manager_api: backtest_api, // 백테스팅에서는 BacktestApi 사용
                    api_bundle,
                })
            }
        }
    }

    /// prototype.py의 run() 메서드와 동일한 메인 루프
    pub fn run(&mut self) -> StockrsResult<()> {
        // prototype.py: on start
        TimeService::global_on_start()?;
        
        // 장 중간 진입 처리 (모의투자/실거래에서만)
        let trading_mode = Self::api_type_to_trading_mode(self.api_type);

        match trading_mode {
            TradingMode::Real | TradingMode::Paper => {
                println!("🟢 [Time] 실시간 모드 시작: 현재 시각 기준으로 초기화 및 장 중간 진입 여부 확인");
            }
            TradingMode::Backtest => {
                println!("🔬 [Time] 백테스트 모드 시작: 08:30부터 시뮬레이션 진행");
            }
        }
        TimeService::global_handle_mid_session_entry(trading_mode)?;
        
        self.model.on_start()?;

        // 백테스팅 모드에서는 현재 시간을 전달
        let current_time = if self.api_type == ApiType::Backtest {
            Some(TimeService::global_format_ymdhm()?)
        } else {
            None
        };
        self.db_manager
            .on_start(TimeService::global_now()?.date_naive(), current_time)?;

        self.broker.on_start()?;

        // prototype.py: while not self.stop_condition:
        while !self.stop_condition {
            // prototype.py: wait_until_next_event(self.time)
            self.wait_until_next_event()?;

            // 실전/모의 모드에서는 매 분마다 보류 주문 처리 및 overview 갱신 수행
            if matches!(self.api_type, ApiType::Real | ApiType::Paper) {

                println!(" [Runner] 주문 처리 및 overview 갱신 중");

                if let Err(e) = self.broker.process_pending(&self.db_manager) {
                    println!("⚠️ [Runner] 보류 주문 처리 실패: {}", e);
                }

                if let Err(e) = self
                    .db_manager
                    .update_overview(TimeService::global_now()?.date_naive(), None)
                {
                    println!("⚠️ [Runner] overview 분당 업데이트 실패: {}", e);
                }

                println!(" => 완료");
            }

            // prototype.py: result = self.model.on_event(self.time)
            let result = self.model.on_event(&self.api_bundle)?;

            // prototype.py: broker on event
            if let Some(mut order) = result {
                // prototype.py: broker_result = self.broker.on_event(result)
                let broker_result = self.broker.on_event(&mut order, &self.db_manager);

                // prototype.py: if broker_result is not None: self.db_manager.on_event(broker_result)
                match broker_result {
                    Ok(_) => {
                        // println!("✅ [Runner] 거래 성공 - DB 매니저 이벤트 호출");

                        // 백테스팅 모드에서는 현재 시간을 전달
                        let current_time = if self.api_type == ApiType::Backtest {
                            Some(TimeService::global_format_ymdhm()?)
                        } else {
                            None
                        };

                        if let Err(e) =
                            self.db_manager
                                .on_event(TimeService::global_now()?.date_naive(), current_time, ())
                        {
                            println!("❌ [Runner] DB 매니저 이벤트 처리 실패: {}", e);
                            return Err(StockrsError::general(format!(
                                "DB 매니저 이벤트 처리 실패: {}",
                                e
                            )));
                        }

                        // 실전/모의 모드에서 보류 주문 처리
                        if matches!(self.api_type, ApiType::Real | ApiType::Paper) {
                            if let Err(e) = self.broker.process_pending(&self.db_manager) {
                                println!("⚠️ [Runner] 보류 주문 처리 실패: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ [Runner] 거래 실패: {}", e);
                        // 거래 실패는 치명적 오류가 아니므로 계속 진행
                    }
                }
            }
        }

        // prototype.py: on end
        self.model.on_end()?;

        // 백테스팅 모드에서는 현재 시간을 전달
        let current_time = if self.api_type == ApiType::Backtest {
            Some(TimeService::global_format_ymdhm()?)
        } else {
            None
        };
        self.db_manager
            .on_end(TimeService::global_now()?.date_naive(), current_time)?;
        self.broker.on_end()?;

        Ok(())
    }

    /// prototype.py의 wait_until_next_event 함수와 동일 - 최적화됨
    /// 다음 이벤트 시각까지 블로킹 대기
    fn wait_until_next_event(&mut self) -> StockrsResult<()> {
        use crate::time::TimeSignal;

        // TradingMode 결정
        let trading_mode = Self::api_type_to_trading_mode(self.api_type);

        // end_date 체크 (백테스팅 모드에서만)
        if self.api_type == ApiType::Backtest {
            if let Ok(config) = config::get_config() {
                let end_date_str = &config.time_management.end_date;

                // YYYYMMDD 형식을 NaiveDate로 파싱
                let year = end_date_str[0..4].parse::<i32>().map_err(|_| {
                    format!("설정의 end_date 연도가 잘못되었습니다: {}", end_date_str)
                })?;
                let month = end_date_str[4..6].parse::<u32>().map_err(|_| {
                    format!("설정의 end_date 월이 잘못되었습니다: {}", end_date_str)
                })?;
                let day = end_date_str[6..8].parse::<u32>().map_err(|_| {
                    format!("설정의 end_date 일이 잘못되었습니다: {}", end_date_str)
                })?;

                if let Some(end_date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
                    let current_date = TimeService::global_now()?.date_naive();

                    // 현재 날짜가 end_date를 지났으면 중지
                    if current_date > end_date {
                        println!(
                            "🏁 [Runner] 백테스팅 종료일({}) 도달 - 프로그램 종료",
                            end_date.format("%Y-%m-%d")
                        );
                        return Err(StockrsError::general("백테스팅 종료일 도달".to_string()));
                    }
                }
            }
        }

        // 백테스팅에서 장 마감 시점(15:30)에 finish_overview 호출
        if self.api_type == ApiType::Backtest {
            let current_time = TimeService::global_now()?;
            let hour = current_time.hour();
            let minute = current_time.minute();

            if hour == 15 && minute == 30 {
                // println!("📊 [Runner] 장 마감 시점 - 당일 overview 마감 처리");

                // 백테스팅 모드에서는 현재 시간을 전달
                let current_time_str = if self.api_type == ApiType::Backtest {
                    Some(TimeService::global_now()?.format("%Y%m%d%H%M").to_string())
                } else {
                    None
                };

                if let Err(e) = self
                    .db_manager
                    .finish_overview(TimeService::global_now()?.date_naive(), current_time_str)
                {
                    println!("❌ [Runner] 당일 overview 마감 실패: {}", e);
                    return Err(StockrsError::general(format!(
                        "당일 overview 마감 실패: {}",
                        e
                    )));
                }

                // println!("✅ [Runner] 당일 overview 마감 완료");

                // 백테스팅: 종료일(end_date) 장 마감 시점에 정확히 종료하도록 처리
                if let Ok(cfg) = config::get_config() {
                    let end_date_str = &cfg.time_management.end_date;
                    if end_date_str.len() == 8 {
                        if let (Ok(year), Ok(month), Ok(day)) = (
                            end_date_str[0..4].parse::<i32>(),
                            end_date_str[4..6].parse::<u32>(),
                            end_date_str[6..8].parse::<u32>(),
                        ) {
                            if let Some(end_date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
                                let today = TimeService::global_now()?.date_naive();
                                if today == end_date {
                                    println!(
                                        "🏁 [Runner] 백테스팅 종료: end_date({}) 장 마감 도달 - 종료",
                                        end_date.format("%Y-%m-%d")
                                    );
                                    self.stop_condition = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 현재 신호 확인 (전역 인스턴스에서)
        let current_signal = TimeService::global_now_signal()?;

        // 백테스팅에서 새로운 거래일 시작 시 객체 리셋 (Overnight 또는 DataPrep에서 처리)
        if self.api_type == ApiType::Backtest && (current_signal == TimeSignal::Overnight || current_signal == TimeSignal::DataPrep) {
            let today = TimeService::global_now()?.date_naive();

            // 같은 날짜에 중복 실행 방지
            if self.last_new_day_logged != Some(today) {
                println!(
                    "📅 [Runner] 새로운 거래일 시작: {}",
                    today.format("%Y-%m-%d")
                );

                // 매일 새로운 거래일을 위해 모든 객체 리셋
                if let Err(e) = self.model.reset_for_new_day() {
                    println!("❌ [Runner] 모델 리셋 실패: {}", e);
                    return Err(StockrsError::general(format!("모델 리셋 실패: {}", e)));
                }

                if let Err(e) = self.broker.reset_for_new_day() {
                    println!("❌ [Runner] 브로커 리셋 실패: {}", e);
                    return Err(StockrsError::general(format!("브로커 리셋 실패: {}", e)));
                }

                // 백테스팅 모드에서는 현재 시간을 YYYYMMDDHHMM 형식으로 전달 (분봉 DB 조회용)
                let current_time = if self.api_type == ApiType::Backtest {
                    Some(TimeService::global_format_ymdhm()?)
                } else {
                    None
                };

                if let Err(e) = self
                    .db_manager
                    .reset_for_new_day(TimeService::global_now()?.date_naive(), current_time)
                {
                    println!("❌ [Runner] DB 매니저 리셋 실패: {}", e);
                    return Err(StockrsError::general(format!("DB 매니저 리셋 실패: {}", e)));
                }

                // 오늘 날짜에 대해 한 번만 로깅/리셋하도록 마킹
                self.last_new_day_logged = Some(today);
            }
        }

        // TimeService의 통합된 대기 로직 사용
        TimeService::global_wait_until_next_event(trading_mode)?;

        // 실시간 모드에서 end_date 장 종료까지 운영하도록 종료 조건을 추가
        if matches!(self.api_type, ApiType::Real | ApiType::Paper) {
            if let Ok(config) = config::get_config() {
                let end_date_str = &config.time_management.end_date;
                if end_date_str.len() == 8 {
                    if let (Ok(year), Ok(month), Ok(day)) = (
                        end_date_str[0..4].parse::<i32>(),
                        end_date_str[4..6].parse::<u32>(),
                        end_date_str[6..8].parse::<u32>(),
                    ) {
                        if let Some(end_date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
                            let now_dt = TimeService::global_now()?;
                            let today = now_dt.date_naive();
                            if today > end_date {
                                println!("🏁 [Runner] 실시간 모드 종료: end_date({}) 초과", end_date.format("%Y-%m-%d"));
                                self.stop_condition = true;
                            } else if today == end_date {
                                // end_date의 장 종료 시각 파싱
                                if let Ok(close_naive) = chrono::NaiveTime::parse_from_str(&config.market_hours.market_close_time, "%H:%M:%S") {
                                    let now_time = now_dt.time();
                                    if now_time >= close_naive {
                                        println!("🏁 [Runner] 실시간 모드 종료: end_date 장 종료({}) 도달", end_date.format("%Y-%m-%d"));
                                        self.stop_condition = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// runner 중지 요청
    pub fn stop(&mut self) {
        self.stop_condition = true;
    }
}

/// prototype.py의 구조를 따른 Builder 패턴
pub struct RunnerBuilder {
    api_type: ApiType,
    model: Option<Box<dyn Model>>,
    db_path: Option<std::path::PathBuf>,
}

impl Default for RunnerBuilder {
    fn default() -> Self {
        Self {
            api_type: ApiType::Backtest, // 기본값은 백테스팅
            model: None,
            db_path: None,
        }
    }
}

impl RunnerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn api_type(mut self, api_type: ApiType) -> Self {
        self.api_type = api_type;
        self
    }

    pub fn model(mut self, model: Box<dyn Model>) -> Self {
        self.model = Some(model);
        self
    }

    pub fn db_path<P: Into<std::path::PathBuf>>(mut self, path: P) -> Self {
        self.db_path = Some(path.into());
        self
    }

    pub fn build(self) -> StockrsResult<Runner> {
        let model = self.model.ok_or_else(|| StockrsError::Api {
            message: "Model is required".to_string(),
        })?;
        let db_path = self.db_path.ok_or_else(|| StockrsError::Api {
            message: "DB path is required".to_string(),
        })?;

        Runner::new(self.api_type, model, db_path)
    }
}
