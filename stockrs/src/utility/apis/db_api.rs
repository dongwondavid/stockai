use crate::utility::config::get_config;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::types::api::StockApi;
use crate::utility::types::broker::Order;
use crate::utility::types::trading::AssetInfo;
use rusqlite::Connection;
use tracing::{debug, info};

/// DB 정보 조회 API - 데이터베이스에서 주식 정보 조회만 담당
pub struct DbApi {
    /// 1분봉 DB 연결 (백테스팅용 현재가 조회)
    minute_db_connection: Connection,
    /// 5분봉 DB 연결 (특징 계산용)
    db_connection: Connection,
    /// 일봉 DB 연결 (특징 계산용)
    daily_db_connection: Connection,
}

impl DbApi {
    pub fn new() -> StockrsResult<Self> {
        debug!("🔄 [DbApi::new] DbApi 초기화 시작");

        // config에서 DB 경로 로드
        let config = get_config()?;

        // DB 연결 (필수)
        let minute_db_connection = Connection::open(&config.database.minute_db_path)
            .map_err(|e| StockrsError::database("1분봉 DB 연결", e.to_string()))?;

        let db_connection = Connection::open(&config.database.stock_db_path)
            .map_err(|e| StockrsError::database("5분봉 DB 연결", e.to_string()))?;

        let daily_db_connection = Connection::open(&config.database.daily_db_path)
            .map_err(|e| StockrsError::database("일봉 DB 연결", e.to_string()))?;

        // 성능 최적화: DB 인덱스 추가 및 설정
        Self::optimize_database(&minute_db_connection)?;
        Self::optimize_database(&db_connection)?;
        Self::optimize_database(&daily_db_connection)?;

        info!(
            "✅ [DbApi::new] 1분봉 DB 연결 성공: {}",
            config.database.minute_db_path
        );
        info!(
            "✅ [DbApi::new] 5분봉 DB 연결 성공: {}",
            config.database.stock_db_path
        );
        info!(
            "✅ [DbApi::new] 일봉 DB 연결 성공: {}",
            config.database.daily_db_path
        );

        debug!("✅ [DbApi::new] DbApi 초기화 완료");

        Ok(DbApi {
            minute_db_connection,
            db_connection,
            daily_db_connection,
        })
    }

    /// 데이터베이스 성능 최적화 설정
    fn optimize_database(db: &Connection) -> StockrsResult<()> {
        // WAL 모드 활성화 (쓰기 성능 향상)
        db.execute_batch("PRAGMA journal_mode=WAL;")?;

        // 메모리 사용량 최적화
        db.execute_batch("PRAGMA cache_size=10000;")?;
        db.execute_batch("PRAGMA temp_store=MEMORY;")?;

        // 동기화 설정 (성능과 안정성의 균형)
        db.execute_batch("PRAGMA synchronous=NORMAL;")?;

        // 외래키 제약 조건 비활성화 (성능 향상)
        db.execute_batch("PRAGMA foreign_keys=OFF;")?;

        debug!("✅ [DbApi::optimize_database] DB 최적화 설정 완료");
        Ok(())
    }

    /// DB에서 현재가 조회 (시간 기반) - 1분봉 DB 사용 (최적화됨)
    pub fn get_current_price_at_time(&self, stockcode: &str, time_str: &str) -> StockrsResult<f64> {
        // 정확한 시간의 데이터가 있는지 확인 (1분봉 DB)
        let exact_query = "SELECT close FROM \"{}\" WHERE date = ?";

        // 정확한 시간의 데이터 조회 시도
        let mut stmt = self
            .minute_db_connection
            .prepare(&exact_query.replace("{}", stockcode))
            .map_err(|_e| {
                StockrsError::database_query(format!(
                    "SQL 준비 실패: {} (테이블: {})",
                    exact_query, stockcode
                ))
            })?;

        let exact_result: Result<f64, _> = stmt.query_row([time_str], |row| row.get(0));

        if let Ok(current_price) = exact_result {
            if current_price > 0.0 {
                return Ok(current_price);
            }
        }

        // 정확한 시간이 없으면, 해당 시간 이하의 가장 최근 데이터 조회 (1분봉 DB)
        let fallback_query = "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1";

        let mut stmt = self
            .minute_db_connection
            .prepare(&fallback_query.replace("{}", stockcode))
            .map_err(|_e| {
                StockrsError::database_query(format!(
                    "SQL 준비 실패: {} (테이블: {})",
                    fallback_query, stockcode
                ))
            })?;

        let current_price: f64 = stmt.query_row([time_str], |row| row.get(0)).map_err(|_e| {
            StockrsError::price_inquiry(
                stockcode,
                "현재가",
                format!(
                    "해당 종목의 데이터가 1분봉 DB에 존재하지 않습니다 (시간: {})",
                    time_str
                ),
            )
        })?;

        if current_price > 0.0 {
            Ok(current_price)
        } else {
            Err(StockrsError::price_inquiry(
                stockcode,
                "현재가",
                format!(
                    "해당 종목의 현재가 데이터를 찾을 수 없습니다 (시간: {})",
                    time_str
                ),
            ))
        }
    }

    /// 거래대금 상위 종목 조회 (predict_top_stocks.rs와 동일한 구현) - 5분봉 DB 사용
    pub fn get_top_amount_stocks(&self, date: &str, limit: usize, date_start: &str, date_end: &str) -> StockrsResult<Vec<String>> {
        debug!(
            "🔍 [DbApi::get_top_amount_stocks] 거래대금 상위 종목 조회 시작: 날짜={}, limit={}, 시간대: {}~{}",
            date, limit, date_start, date_end
        );

        // 모든 테이블(종목) 목록 가져오기 (5분봉 DB)
        let tables_query =
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'";
        let mut stmt = self.db_connection.prepare(tables_query).map_err(|e| {
            println!(
                "❌ [DbApi::get_top_amount_stocks:425] SQL 준비 실패: {}",
                tables_query
            );
            println!("❌ [DbApi::get_top_amount_stocks:425] 오류: {}", e);
            StockrsError::database_query(format!("SQL 준비 실패: {}", tables_query))
        })?;

        let tables = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect::<Vec<String>>();

        debug!(
            "📊 [DbApi::get_top_amount_stocks] 5분봉 DB 테이블 개수: {}개",
            tables.len()
        );

        let mut stock_volumes: Vec<(String, i64)> = Vec::new();

        debug!(
            "⏰ [DbApi::get_top_amount_stocks] 조회 시간대: {} ~ {}",
            date_start, date_end
        );

        for table_name in &tables {
            let volume_query = format!(
                "SELECT SUM(volume * close) as total_volume FROM \"{}\" WHERE date >= ? AND date <= ?",
                table_name
            );

            if let Ok(mut volume_stmt) = self.db_connection.prepare(&volume_query) {
                if let Ok(total_volume) = volume_stmt.query_row([&date_start, &date_end], |row| {
                    let value = row.get::<_, Option<i64>>(0)?;
                    value.ok_or_else(|| {
                        rusqlite::Error::InvalidParameterName(
                            "거래대금 데이터가 NULL입니다".to_string(),
                        )
                    })
                }) {
                    if total_volume > 0 {
                        // 테이블명이 그대로 종목코드 (A 접두사 포함)
                        let stock_code = table_name.to_string();
                        stock_volumes.push((stock_code, total_volume));
                    }
                }
            }
        }

        debug!(
            "💰 [DbApi::get_top_amount_stocks] 거래대금 > 0인 종목 개수: {}개",
            stock_volumes.len()
        );

        // 거래대금 기준으로 정렬하고 상위 limit개 선택
        stock_volumes.sort_by(|a, b| b.1.cmp(&a.1));
        let top_stocks: Vec<String> = stock_volumes
            .into_iter()
            .take(limit)
            .map(|(code, _)| code)
            .collect();

        debug!(
            "🎯 [DbApi::get_top_amount_stocks] 최종 선택된 종목 개수: {}개",
            top_stocks.len()
        );

        Ok(top_stocks)
    }

    /// DB 구조 디버깅용 함수 (5분봉 DB)
    pub fn debug_db_structure(&self, stockcode: &str) -> StockrsResult<()> {
        println!(
            "🔍 [DbApi::debug_db_structure] 종목 {} 5분봉 DB 구조 확인",
            stockcode
        );

        // 테이블 존재 여부 확인
        let table_check_query = "SELECT name FROM sqlite_master WHERE type='table' AND name=?";
        let mut stmt = self.db_connection.prepare(table_check_query).map_err(|e| {
            println!(
                "❌ [DbApi::debug_db_structure:485] SQL 준비 실패: {}",
                table_check_query
            );
            println!("❌ [DbApi::debug_db_structure:485] 오류: {}", e);
            StockrsError::database_query(format!("SQL 준비 실패: {}", table_check_query))
        })?;

        let table_exists: Result<String, _> = stmt.query_row([stockcode], |row| row.get(0));

        if table_exists.is_err() {
            println!(
                "❌ [DbApi::debug_db_structure] 테이블이 존재하지 않음: {}",
                stockcode
            );
            return Ok(());
        }

        // 테이블 스키마 확인
        let schema_query = format!("PRAGMA table_info(\"{}\")", stockcode);
        let mut stmt = self.db_connection.prepare(&schema_query).map_err(|e| {
            println!(
                "❌ [DbApi::debug_db_structure:495] SQL 준비 실패: {}",
                schema_query
            );
            println!("❌ [DbApi::debug_db_structure:495] 오류: {}", e);
            StockrsError::database_query(format!(
                "SQL 준비 실패: {} (테이블: {})",
                schema_query, stockcode
            ))
        })?;

        let mut rows = stmt.query([]).map_err(|e| {
            println!(
                "❌ [DbApi::debug_db_structure:500] SQL 실행 실패: {}",
                schema_query
            );
            println!("❌ [DbApi::debug_db_structure:500] 오류: {}", e);
            StockrsError::database_query(format!(
                "SQL 실행 실패: {} (테이블: {})",
                schema_query, stockcode
            ))
        })?;

        println!("📋 [DbApi::debug_db_structure] 테이블 스키마:");
        while let Some(row) = rows.next()? {
            let cid: i32 = row.get(0)?;
            let name: String = row.get(1)?;
            let typ: String = row.get(2)?;
            let notnull: i32 = row.get(3)?;
            let _dflt_value: Option<String> = row.get(4)?;
            let pk: i32 = row.get(5)?;

            println!(
                "  컬럼 {}: {} ({}) - PK: {}, NOT NULL: {}",
                cid, name, typ, pk, notnull
            );
        }

        // 샘플 데이터 확인
        let sample_query = format!("SELECT * FROM \"{}\" ORDER BY date LIMIT 5", stockcode);
        let mut stmt = self.db_connection.prepare(&sample_query).map_err(|e| {
            println!(
                "❌ [DbApi::debug_db_structure:515] SQL 준비 실패: {}",
                sample_query
            );
            println!("❌ [DbApi::debug_db_structure:515] 오류: {}", e);
            StockrsError::database_query(format!(
                "SQL 준비 실패: {} (테이블: {})",
                sample_query, stockcode
            ))
        })?;

        let mut rows = stmt.query([]).map_err(|e| {
            println!(
                "❌ [DbApi::debug_db_structure:520] SQL 실행 실패: {}",
                sample_query
            );
            println!("❌ [DbApi::debug_db_structure:520] 오류: {}", e);
            StockrsError::database_query(format!(
                "SQL 실행 실패: {} (테이블: {})",
                sample_query, stockcode
            ))
        })?;

        println!("📊 [DbApi::debug_db_structure] 샘플 데이터 (처음 5개):");
        while let Some(row) = rows.next()? {
            let date: i64 = row.get(0)?;
            let open: f64 = row.get(1)?;
            let high: f64 = row.get(2)?;
            let low: f64 = row.get(3)?;
            let close: f64 = row.get(4)?;
            let volume: i64 = row.get(5)?;

            println!(
                "  {}: O:{:.0} H:{:.0} L:{:.0} C:{:.0} V:{}",
                date, open, high, low, close, volume
            );
        }

        // 전체 데이터 개수 확인
        let count_query = format!("SELECT COUNT(*) FROM \"{}\"", stockcode);
        let mut stmt = self.db_connection.prepare(&count_query).map_err(|e| {
            println!(
                "❌ [DbApi::debug_db_structure:535] SQL 준비 실패: {}",
                count_query
            );
            println!("❌ [DbApi::debug_db_structure:535] 오류: {}", e);
            StockrsError::database_query(format!(
                "SQL 준비 실패: {} (테이블: {})",
                count_query, stockcode
            ))
        })?;

        let count: i64 = stmt.query_row([], |row| row.get(0)).map_err(|e| {
            println!(
                "❌ [DbApi::debug_db_structure:540] SQL 실행 실패: {}",
                count_query
            );
            println!("❌ [DbApi::debug_db_structure:540] 오류: {}", e);
            StockrsError::database_query(format!(
                "SQL 실행 실패: {} (테이블: {})",
                count_query, stockcode
            ))
        })?;

        println!("📈 [DbApi::debug_db_structure] 전체 데이터 개수: {}", count);

        Ok(())
    }
}

// StockApi trait 구현 - 데이터 조회 전담으로 변경
impl StockApi for DbApi {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn execute_order(&self, _order: &mut Order) -> StockrsResult<String> {
        // DbApi는 주문 실행을 지원하지 않음 (BacktestApi에서 담당)
        Err(StockrsError::order_execution(
            "주문 실행".to_string(),
            "N/A".to_string(),
            0,
            "DbApi는 주문 실행을 지원하지 않습니다. BacktestApi를 사용하세요.".to_string(),
        ))
    }

    fn check_fill(&self, _order_id: &str) -> StockrsResult<bool> {
        // DbApi는 체결 확인을 지원하지 않음
        Err(StockrsError::order_execution(
            "체결 확인".to_string(),
            "N/A".to_string(),
            0,
            "DbApi는 체결 확인을 지원하지 않습니다.".to_string(),
        ))
    }

    fn cancel_order(&self, _order_id: &str) -> StockrsResult<()> {
        // DbApi는 주문 취소를 지원하지 않음
        Err(StockrsError::order_execution(
            "주문 취소".to_string(),
            "N/A".to_string(),
            0,
            "DbApi는 주문 취소를 지원하지 않습니다.".to_string(),
        ))
    }

    fn get_balance(&self) -> StockrsResult<AssetInfo> {
        // DbApi는 잔고 조회를 지원하지 않음 (BacktestApi에서 담당)
        Err(StockrsError::BalanceInquiry {
            reason: "DbApi는 잔고 조회를 지원하지 않습니다. BacktestApi를 사용하세요.".to_string(),
        })
    }

    fn get_avg_price(&self, _stockcode: &str) -> StockrsResult<f64> {
        // DbApi는 평균가 조회를 지원하지 않음 (BacktestApi에서 담당)
        Err(StockrsError::price_inquiry(
            "N/A",
            "평균가",
            "DbApi는 평균가 조회를 지원하지 않습니다. BacktestApi를 사용하세요.".to_string(),
        ))
    }

    fn get_current_price(&self, _stockcode: &str) -> StockrsResult<f64> {
        // DbApi는 현재가 조회를 지원하지 않음 (시간 정보가 필요함)
        Err(StockrsError::price_inquiry(
            "N/A",
            "현재가",
            "DbApi는 현재가 조회를 지원하지 않습니다. get_current_price_at_time을 사용하세요."
                .to_string(),
        ))
    }

    fn get_current_price_at_time(&self, stockcode: &str, time_str: &str) -> StockrsResult<f64> {
        // DbApi는 시간 기반 현재가 조회를 지원
        self.get_current_price_at_time(stockcode, time_str)
    }

    fn set_current_time(&self, _time_str: &str) -> StockrsResult<()> {
        // DbApi는 현재 시간 설정을 지원하지 않음
        Ok(())
    }

    /// DB 연결을 반환 (특징 계산용)
    fn get_db_connection(&self) -> Option<rusqlite::Connection> {
        // Connection을 복제하여 반환 (새로운 연결 생성)
        Connection::open(self.db_connection.path().unwrap_or_default()).ok()
    }

    /// 일봉 DB 연결을 반환 (특징 계산용)
    fn get_daily_db_connection(&self) -> Option<rusqlite::Connection> {
        // Connection을 복제하여 반환 (새로운 연결 생성)
        Connection::open(self.daily_db_connection.path().unwrap_or_default()).ok()
    }
}
