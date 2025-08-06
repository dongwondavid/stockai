use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::types::api::SharedApi;
use crate::utility::types::trading::Trading;
use chrono::NaiveDate;
use rusqlite::{Connection, Result as SqliteResult};
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

/// DB 작업 결과를 위한 NewType 패턴
/// rusqlite::Result와 StockrsResult 간의 변환을 안전하게 처리
#[derive(Debug)]
pub struct DBResult<T>(pub Result<T, StockrsError>);

impl<T> DBResult<T> {
    /// 성공 결과 생성
    pub fn ok(value: T) -> Self {
        Self(Ok(value))
    }

    /// 오류 결과 생성
    pub fn err(error: StockrsError) -> Self {
        Self(Err(error))
    }

    /// 내부 Result 반환
    pub fn into_result(self) -> Result<T, StockrsError> {
        self.0
    }

    /// 참조로 내부 Result 반환
    pub fn as_result(&self) -> &Result<T, StockrsError> {
        &self.0
    }
}

impl<T> From<SqliteResult<T>> for DBResult<T> {
    fn from(result: SqliteResult<T>) -> Self {
        match result {
            Ok(value) => Self::ok(value),
            Err(e) => Self::err(StockrsError::from(e)),
        }
    }
}

impl<T> From<StockrsResult<T>> for DBResult<T> {
    fn from(result: StockrsResult<T>) -> Self {
        Self(result)
    }
}

/// 백테스팅 모드 감지를 위한 NewType 패턴
#[derive(Debug, Clone)]
pub struct BacktestMode {
    pub is_backtest: bool,
    pub current_time: Option<String>,
}

impl BacktestMode {
    /// 일반 모드 (실전/모의투자)
    pub fn normal() -> Self {
        Self {
            is_backtest: false,
            current_time: None,
        }
    }

    /// 백테스팅 모드
    pub fn backtest(current_time: String) -> Self {
        Self {
            is_backtest: true,
            current_time: Some(current_time),
        }
    }

    /// 현재 시간 문자열 반환
    pub fn time_str(&self) -> Option<&str> {
        self.current_time.as_deref()
    }
}

/// API 타입 감지를 위한 NewType 패턴
pub struct ApiTypeDetector {
    api: SharedApi,
}

impl ApiTypeDetector {
    pub fn new(api: SharedApi) -> Self {
        Self { api }
    }

    /// 백테스팅 모드에서 잔고 계산
    pub fn calculate_balance_in_backtest(
        &self,
        time: &str,
    ) -> StockrsResult<crate::utility::types::trading::AssetInfo> {
        // BacktestApi의 시간 기반 잔고 계산 사용
        if let Some(backtest_api) = self.api.as_any().downcast_ref::<crate::utility::apis::BacktestApi>() {
            backtest_api.calculate_balance_at_time(time)
        } else {
            // BacktestApi가 아닌 경우 일반 잔고 조회
            self.api.get_balance()
        }
    }
}

pub struct DBManager {
    conn: Connection,
    api: SharedApi,
    /// API 타입 감지기
    api_detector: ApiTypeDetector,
}

impl DBManager {
    pub fn new(path: PathBuf, api: SharedApi) -> SqliteResult<Self> {
        let conn = Connection::open(path)?;

        // Create trading table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS trading (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT,
                time TEXT,
                stockcode TEXT,
                buy_or_sell TEXT,
                quantity INTEGER,
                price REAL,
                fee REAL,
                strategy TEXT,
                avg_price REAL,
                profit REAL,
                roi REAL
            )",
            (),
        )?;

        // Create overview table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS overview (
                date TEXT PRIMARY KEY,
                open REAL,
                high REAL,
                low REAL,
                close REAL,
                volume INTEGER,
                turnover REAL,
                profit REAL,
                roi REAL,
                fee REAL
            )",
            (),
        )?;

        let api_detector = ApiTypeDetector::new(api.clone());

        Ok(Self {
            conn,
            api,
            api_detector,
        })
    }

    // Save trading data to database
    pub fn save_trading(&self, trading: Trading, avg_price: f64) -> SqliteResult<()> {
        let trading_result = trading.to_trading_result(avg_price);

        // Insert trading data
        self.conn.execute(
            "INSERT INTO trading (
                date, time, stockcode, buy_or_sell, quantity, 
                price, fee, strategy, avg_price, profit, roi
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            trading_result.to_db_tuple(),
        )?;

        Ok(())
    }

    /// 백테스팅 모드에서 잔고 조회
    fn get_balance_with_mode(
        &self,
        mode: BacktestMode,
    ) -> DBResult<crate::utility::types::trading::AssetInfo> {
        if mode.is_backtest {
            if let Some(time) = mode.time_str() {
                // 백테스팅 모드에서 특정 시간의 잔고 계산
                self.api_detector.calculate_balance_in_backtest(time).into()
            } else {
                // 백테스팅 모드이지만 시간이 지정되지 않은 경우
                self.api.get_balance().into()
            }
        } else {
            // 일반 모드에서 현재 잔고 조회
            self.api.get_balance().into()
        }
    }

    // Initialize today's overview data
    pub fn insert_overview(
        &self,
        current_date: NaiveDate,
        current_time: Option<String>,
    ) -> SqliteResult<()> {
        debug!(
            "🔄 [DBManager::insert_overview] 시작 - 날짜: {}, 시간: {:?}",
            current_date, current_time
        );

        // 백테스팅 모드 감지
        let mode = if let Some(time) = current_time {
            BacktestMode::backtest(time)
        } else {
            BacktestMode::normal()
        };

        // 잔고 조회
        let balance_result: DBResult<crate::utility::types::trading::AssetInfo> =
            self.get_balance_with_mode(mode);
        let result = balance_result.into_result().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("잔고 조회 실패: {}", e)),
            )
        })?;

        let asset = result.get_asset();
        let date_str = current_date.to_string();
        debug!("💰 [DBManager::insert_overview] 현재 자산: {:.0}원", asset);

        // Check if today's data already exists
        let existing_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM overview WHERE date = ?",
            (date_str.clone(),),
            |row| row.get(0),
        ).unwrap_or(0);

        if existing_count > 0 {
            info!(
                "📊 [DBManager::insert_overview] 당일 overview 데이터가 이미 존재합니다: {}",
                date_str
            );
            return Ok(());
        }

        // Insert overview data only if it doesn't exist
        self.conn.execute(
            "INSERT INTO overview (date, open, high, low) VALUES (?, ?, ?, ?)",
            (date_str, asset, asset, asset),
        )?;

        info!(
            "📊 [DBManager::insert_overview] 당일 overview 데이터 초기화 완료: {} (자산: {:.0}원)",
            current_date, asset
        );
        Ok(())
    }

    // Update today's overview data
    pub fn update_overview(
        &self,
        current_date: NaiveDate,
        current_time: Option<String>,
    ) -> SqliteResult<()> {
        debug!(
            "🔄 [DBManager::update_overview] 시작 - 날짜: {}, 시간: {:?}",
            current_date, current_time
        );

        // 백테스팅 모드 감지
        let mode = if let Some(time) = current_time {
            BacktestMode::backtest(time)
        } else {
            BacktestMode::normal()
        };

        // 잔고 조회
        let balance_result: DBResult<crate::utility::types::trading::AssetInfo> =
            self.get_balance_with_mode(mode);
        let result = balance_result.into_result().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("잔고 조회 실패: {}", e)),
            )
        })?;

        let asset = result.get_asset();
        debug!("💰 [DBManager::update_overview] 현재 자산: {:.0}원", asset);

        // 먼저 해당 날짜의 overview 데이터가 존재하는지 확인
        let date_str = current_date.to_string();
        let data_exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM overview WHERE date = ?",
            (date_str.clone(),),
            |row| row.get(0),
        ).unwrap_or(0);

        if data_exists == 0 {
            info!("📊 [DBManager::update_overview] {} 날짜의 overview 데이터가 없습니다. 새로 생성합니다.", date_str);

            // overview 데이터가 없으면 새로 생성
            self.conn.execute(
                "INSERT INTO overview (date, open, high, low) VALUES (?, ?, ?, ?)",
                (date_str.clone(), asset, asset, asset),
            )?;

            info!(
                "✅ [DBManager::update_overview] {} 날짜 overview 데이터 생성 완료",
                date_str
            );
            return Ok(());
        }

        // Get today's high and low values
        let (high, low) = match self.conn.query_row(
            "SELECT high, low FROM overview WHERE date = ?",
            (date_str.clone(),),
            |row| {
                let high: f64 = row.get(0)?;
                let low: f64 = row.get(1)?;
                Ok((high, low))
            },
        ) {
            Ok(values) => values,
            Err(e) => {
                warn!(
                    "⚠️ [DBManager::update_overview] overview 데이터 조회 실패: {}, 현재 자산으로 초기화",
                    e
                );
                (asset, asset) // 데이터가 없으면 현재 자산으로 초기화
            }
        };

        debug!(
            "📊 [DBManager::update_overview] 기존 high: {:.0}원, low: {:.0}원",
            high, low
        );

        // Update with new values - low는 실제로 자산이 감소할 때만 업데이트
        let new_high = high.max(asset);
        let new_low = if asset < low { asset } else { low };

        self.conn.execute(
            "UPDATE overview SET high = ?, low = ? WHERE date = ?",
            (new_high, new_low, date_str),
        )?;

        debug!(
            "✅ [DBManager::update_overview] 업데이트 완료 - new_high: {:.0}원, new_low: {:.0}원",
            new_high, new_low
        );
        Ok(())
    }

    // Finalize today's overview data
    pub fn finish_overview(
        &self,
        current_date: NaiveDate,
        current_time: Option<String>,
    ) -> SqliteResult<()> {
        debug!(
            "🔄 [DBManager::finish_overview] 시작 - 날짜: {}, 시간: {:?}",
            current_date, current_time
        );

        // 백테스팅 모드 감지
        let mode = if let Some(time) = current_time {
            BacktestMode::backtest(time)
        } else {
            BacktestMode::normal()
        };

        // 잔고 조회
        let balance_result: DBResult<crate::utility::types::trading::AssetInfo> =
            self.get_balance_with_mode(mode);
        let result = balance_result.into_result().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("잔고 조회 실패: {}", e)),
            )
        })?;

        let asset = result.get_asset();
        let date_str = current_date.to_string();
        debug!("💰 [DBManager::finish_overview] 현재 자산: {:.0}원", asset);

        let open: f64 = match self.conn.query_row(
            "SELECT open FROM overview WHERE date = ?",
            (date_str.clone(),),
            |row| row.get(0),
        ) {
            Ok(value) => value,
            Err(e) => {
                warn!("⚠️ [DBManager::finish_overview] open 값 조회 실패: {}, 현재 자산으로 대체", e);
                asset // open 값이 없으면 현재 자산으로 대체
            }
        };

        let close = asset;
        debug!(
            "📊 [DBManager::finish_overview] open: {:.0}원, close: {:.0}원",
            open, close
        );

        let daily_profit = close - open;
        let daily_roi = daily_profit / open * 100.0;

        // 오늘 날짜의 수수료, 총 거래대금, 총 거래량 조회 (거래가 없어도 안전하게 처리)
        let fee_sum: Option<f64> = self.conn.query_row(
            "SELECT COALESCE(SUM(fee), 0.0) FROM trading WHERE date = ?",
            (date_str.clone(),),
            |row| row.get(0),
        ).ok();

        let turnover_sum: Option<f64> = self.conn.query_row(
            "SELECT COALESCE(SUM(price * quantity), 0.0) FROM trading WHERE date = ?",
            (date_str.clone(),),
            |row| row.get(0),
        ).ok();

        let volume_sum: Option<i64> = self.conn.query_row(
            "SELECT COALESCE(SUM(quantity), 0) FROM trading WHERE date = ?",
            (date_str.clone(),),
            |row| row.get(0),
        ).ok();

        // 거래 기록이 없는 경우 기본값 사용
        let fee = fee_sum.unwrap_or(0.0);
        let turnover = turnover_sum.unwrap_or(0.0);
        let volume = volume_sum.unwrap_or(0);

        if volume == 0 {
            info!("📊 [DBManager::finish_overview] 당일 거래 기록이 없습니다 - 기본값으로 처리 (수수료: 0원, 거래대금: 0원, 거래량: 0주)");
        } else {
            debug!("📊 [DBManager::finish_overview] 거래 기록: 수수료 {:.0}원, 거래대금 {:.0}원, 거래량 {}주", fee, turnover, volume);
        }

        self.conn.execute(
            "UPDATE overview SET close = ?, profit = ?, roi = ?, fee = ?, turnover = ?, volume = ? WHERE date = ?",
            (close, daily_profit, daily_roi, fee, turnover, volume, date_str),
        )?;

        info!("📊 [DBManager::finish_overview] 당일 overview 마감 완료: 수익 {:.0}원 ({:.2}%), 거래량 {}주", daily_profit, daily_roi, volume);
        Ok(())
    }
}

/// 생명주기 패턴 추가 - prototype.py와 동일
impl DBManager {
    /// db_manager 시작 시 호출 - prototype.py의 self.db_manager.on_start()
    /// 당일 거래 시작 시 overview 테이블 초기화
    pub fn on_start(
        &mut self,
        current_date: NaiveDate,
        current_time: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔄 [DBManager::on_start] 거래 시작 - 당일 overview 데이터 초기화");
        match self.insert_overview(current_date, current_time) {
            Ok(_) => info!("✅ [DBManager::on_start] 완료"),
            Err(e) => {
                error!("❌ [DBManager::on_start] 실패: {}", e);
                return Err(e.into());
            }
        }
        Ok(())
    }

    /// db_manager 이벤트 처리 - prototype.py의 self.db_manager.on_event(broker_result)
    /// 거래 이벤트 발생 시 현재 자산 상태를 overview에 업데이트
    pub fn on_event(
        &mut self,
        current_date: NaiveDate,
        current_time: Option<String>,
        _broker_result: (),
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!(
            "🔄 [DBManager::on_event] 시작 - 날짜: {}, 시간: {:?}",
            current_date, current_time
        );

        // 거래가 발생했으므로 overview 업데이트
        match self.update_overview(current_date, current_time) {
            Ok(()) => {
                debug!("✅ [DBManager::on_event] overview 업데이트 완료");
            }
            Err(e) => {
                error!("❌ [DBManager::on_event] overview 업데이트 실패: {}", e);
                return Err(e.into());
            }
        }

        Ok(())
    }

    /// db_manager 종료 시 호출 - prototype.py의 self.db_manager.on_end()
    /// 거래 종료 시 최종 수익률, 수수료 등을 계산하여 overview 완료
    pub fn on_end(
        &mut self,
        current_date: NaiveDate,
        current_time: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔄 [DBManager::on_end] 거래 종료 - 당일 overview 데이터 마감");
        match self.finish_overview(current_date, current_time) {
            Ok(_) => info!("✅ [DBManager::on_end] 완료"),
            Err(e) => {
                error!("❌ [DBManager::on_end] 실패: {}", e);
                return Err(e.into());
            }
        }
        Ok(())
    }

    /// 매일 새로운 거래일을 위해 DB 매니저 상태 리셋
    pub fn reset_for_new_day(
        &mut self,
        current_date: NaiveDate,
        current_time: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔄 [DBManager::reset_for_new_day] 새로운 거래일을 위해 DB 매니저 리셋");

        // 새로운 거래일을 위해 overview 데이터 초기화
        match self.insert_overview(current_date, current_time) {
            Ok(_) => info!("✅ [DBManager::reset_for_new_day] DB 매니저 리셋 완료"),
            Err(e) => {
                error!("❌ [DBManager::reset_for_new_day] 실패: {}", e);
                return Err(e.into());
            }
        }
        Ok(())
    }
}
