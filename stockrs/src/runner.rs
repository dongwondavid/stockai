use crate::utility::apis::{BacktestApi, DbApi, KoreaApi};
use crate::broker::StockBroker;
use crate::db_manager::DBManager;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::model::{ApiBundle, Model};
use crate::time::TimeService;
use crate::utility::types::api::{ApiType, SharedApi};
use crate::utility::config;
use crate::utility::holiday_checker::HolidayChecker;
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
    pub time: TimeService,
    pub model: Box<dyn Model>,
    pub broker: StockBroker,
    pub db_manager: DBManager,

    /// API 번들 (prototype.py의 API 구조 반영)
    pub api_bundle: ApiBundle,

    /// prototype.py의 self.stop_condition
    pub stop_condition: bool,
}

impl Runner {
    /// prototype.py의 __init__과 동일한 초기화 로직
    pub fn new(
        api_type: ApiType,
        model: Box<dyn Model>,
        db_path: std::path::PathBuf,
    ) -> StockrsResult<Self> {
        // TimeService 먼저 생성
        let time_service = TimeService::new()
            .map_err(|e| StockrsError::general(format!("시간 서비스 초기화 실패: {}", e)))?;

        // 모드별 API 구성 (TimeService 전달)
        let api_config = Self::create_api_config(api_type, &time_service)?;

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
            time: time_service,
            model,
            broker: StockBroker::new(api_config.broker_api.clone()),
            db_manager: DBManager::new(db_path, api_config.db_manager_api)?,
            api_bundle: api_config.api_bundle,
            stop_condition: false,
        })
    }

    /// 모드별 API 구성 생성
    fn create_api_config(api_type: ApiType, time_service: &TimeService) -> StockrsResult<ApiConfig> {
        match api_type {
            ApiType::Real => {
                // 실전: 정보 API + 실전 API + DB API
                let real_api: SharedApi = std::rc::Rc::new(KoreaApi::new_real()?);
                let info_api: SharedApi = std::rc::Rc::new(KoreaApi::new_info()?);
                let db_api: SharedApi = std::rc::Rc::new(DbApi::new()?);

                let api_bundle =
                    ApiBundle::new(real_api.clone(), real_api.clone(), info_api, db_api.clone());

                Ok(ApiConfig {
                    broker_api: real_api,
                    db_manager_api: db_api,
                    api_bundle,
                })
            }
            ApiType::Paper => {
                // 모의: 정보 API + 모의 API + DB API
                let paper_api: SharedApi = std::rc::Rc::new(KoreaApi::new_paper()?);
                let info_api: SharedApi = std::rc::Rc::new(KoreaApi::new_info()?);
                let db_api: SharedApi = std::rc::Rc::new(DbApi::new()?);

                let api_bundle = ApiBundle::new(
                    paper_api.clone(),
                    paper_api.clone(),
                    info_api,
                    db_api.clone(),
                );

                Ok(ApiConfig {
                    broker_api: paper_api,
                    db_manager_api: db_api,
                    api_bundle,
                })
            }
            ApiType::Backtest => {
                // 백테스팅: DB API + 백테스팅 API + DB API
                let db_api: SharedApi = std::rc::Rc::new(DbApi::new()?);
                let time_service_rc = std::rc::Rc::new(time_service.clone());
                let backtest_api: SharedApi = std::rc::Rc::new(BacktestApi::new(db_api.clone(), time_service_rc)?);

                let api_bundle = ApiBundle::new_with_backtest_apis(
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
        self.time.on_start()?;
        self.model.on_start()?;

        // 백테스팅 모드에서는 현재 시간을 전달
        let current_time = if self.api_type == ApiType::Backtest {
            Some(self.time.format_ymdhm())
        } else {
            None
        };
        self.db_manager
            .on_start(self.time.now().date_naive(), current_time)?;



        self.broker.on_start()?;

        // prototype.py: while not self.stop_condition:
        while !self.stop_condition {
            // prototype.py: self.time.update()
            self.time.update()?;



            // prototype.py: wait_until_next_event(self.time)
            self.wait_until_next_event()?;

            // prototype.py: result = self.model.on_event(self.time)
            let result = self.model.on_event(&self.time, &self.api_bundle)?;

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
                            Some(self.time.format_ymdhm())
                        } else {
                            None
                        };

                        if let Err(e) =
                            self.db_manager
                                .on_event(self.time.now().date_naive(), current_time, ())
                        {
                            println!("❌ [Runner] DB 매니저 이벤트 처리 실패: {}", e);
                            return Err(StockrsError::general(format!(
                                "DB 매니저 이벤트 처리 실패: {}",
                                e
                            )));
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
            Some(self.time.format_ymdhm())
        } else {
            None
        };
        self.db_manager
            .on_end(self.time.now().date_naive(), current_time)?;
        self.broker.on_end()?;

        Ok(())
    }

    /// prototype.py의 wait_until_next_event 함수와 동일 - 최적화됨
    /// 다음 이벤트 시각까지 블로킹 대기
    fn wait_until_next_event(&mut self) -> StockrsResult<()> {
        use crate::time::TimeSignal;

        // 현재 시간과 신호 확인
        let current_time = self.time.now();
        let current_signal = self.time.now_signal();

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
                    let current_date = current_time.date_naive();

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

        // 백테스팅에서 장 마감 시점(15:20)에 finish_overview 호출
        if self.api_type == ApiType::Backtest {
            let hour = current_time.hour();
            let minute = current_time.minute();

            if hour == 15 && minute == 20 {
                println!("📊 [Runner] 장 마감 시점 - 당일 overview 마감 처리");

                // 백테스팅 모드에서는 현재 시간을 전달
                let current_time_str = if self.api_type == ApiType::Backtest {
                    Some(self.time.now().format("%Y%m%d%H%M").to_string())
                } else {
                    None
                };

                if let Err(e) = self
                    .db_manager
                    .finish_overview(self.time.now().date_naive(), current_time_str)
                {
                    println!("❌ [Runner] 당일 overview 마감 실패: {}", e);
                    return Err(StockrsError::general(format!(
                        "당일 overview 마감 실패: {}",
                        e
                    )));
                }

                println!("✅ [Runner] 당일 overview 마감 완료");
            }
        }

        // 통합된 "다음 거래일로 이동해야 하는 상황" 체크 - 최적화됨
        let current_date = current_time.date_naive();
        let mut holiday_checker = HolidayChecker::default();
        let is_weekend = holiday_checker.is_weekend(current_date);
        let is_holiday = holiday_checker.is_holiday(current_date);

        // Overnight 신호는 이미 TimeService에서 다음 거래일로 이동했으므로 제외
        let should_skip_to_next_day =
            (is_weekend || is_holiday) && current_signal != TimeSignal::Overnight;

        if should_skip_to_next_day {
            // TimeService의 다음 거래일로 이동 메서드 사용
            if let Err(e) = self.time.skip_to_next_trading_day() {
                println!("❌ [Runner] 다음 거래일 이동 실패: {}", e);
                return Err(StockrsError::general(format!(
                    "다음 거래일 이동 실패: {}",
                    e
                )));
            }

            let next_date = self.time.now().date_naive();

            // 백테스팅에서는 새로운 거래일 시작 시 객체 리셋
            if self.api_type == ApiType::Backtest {
                println!(
                    "📅 [Runner] 새로운 거래일 시작: {}",
                    next_date.format("%Y-%m-%d")
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

                // 백테스팅 모드에서는 현재 시간을 전달
                let current_time = if self.api_type == ApiType::Backtest {
                    Some(self.time.now().format("%H:%M:%S").to_string())
                } else {
                    None
                };

                if let Err(e) = self
                    .db_manager
                    .reset_for_new_day(self.time.now().date_naive(), current_time)
                {
                    println!("❌ [Runner] DB 매니저 리셋 실패: {}", e);
                    return Err(StockrsError::general(format!("DB 매니저 리셋 실패: {}", e)));
                }

                return Ok(());
            } else {
                // 실거래/모의투자는 실제 대기
                self.time.wait_until(self.time.now());
            }
        }

        // Overnight 신호일 때는 백테스팅에서 새로운 거래일 시작 처리
        if current_signal == TimeSignal::Overnight && self.api_type == ApiType::Backtest {
            let next_date = self.time.now().date_naive();
            println!(
                "📅 [Runner] 새로운 거래일 시작: {}",
                next_date.format("%Y-%m-%d")
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

            // 백테스팅 모드에서는 현재 시간을 전달
            let current_time = if self.api_type == ApiType::Backtest {
                Some(self.time.now().format("%H:%M:%S").to_string())
            } else {
                None
            };

            if let Err(e) = self
                .db_manager
                .reset_for_new_day(self.time.now().date_naive(), current_time)
            {
                println!("❌ [Runner] DB 매니저 리셋 실패: {}", e);
                return Err(StockrsError::general(format!("DB 매니저 리셋 실패: {}", e)));
            }

            return Ok(());
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
