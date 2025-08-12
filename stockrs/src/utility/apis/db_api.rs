use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::config;
use crate::utility::types::api::StockApi;
use crate::utility::types::broker::Order;
use crate::utility::types::trading::AssetInfo;
use rusqlite::Connection;
use tracing::debug;

/// DB 기반 API 구현 (백테스팅용)
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
        let config = config::get_config()?;

        let minute_db_connection = Connection::open(&config.database.minute_db_path)
            .map_err(|e| StockrsError::general(format!("1분봉 DB 연결 실패: {}", e)))?;
        let db_connection = Connection::open(&config.database.stock_db_path)
            .map_err(|e| StockrsError::general(format!("5분봉 DB 연결 실패: {}", e)))?;
        let daily_db_connection = Connection::open(&config.database.daily_db_path)
            .map_err(|e| StockrsError::general(format!("일봉 DB 연결 실패: {}", e)))?;

        Self::optimize_database(&minute_db_connection)?;
        Self::optimize_database(&db_connection)?;
        Self::optimize_database(&daily_db_connection)?;

        debug!(
            "✅ [DbApi::new] DB 연결 완료 - 1분봉: {}, 5분봉: {}, 일봉: {}",
            config.database.minute_db_path,
            config.database.stock_db_path,
            config.database.daily_db_path
        );

        Ok(DbApi {
            minute_db_connection,
            db_connection,
            daily_db_connection,
        })
    }

    /// 데이터베이스 성능 최적화 설정
    fn optimize_database(db: &Connection) -> StockrsResult<()> {
        // 성능 최적화: DB 인덱스 추가 및 설정
        db.execute_batch("PRAGMA journal_mode = WAL")
            .map_err(|e| StockrsError::general(format!("WAL 모드 설정 실패: {}", e)))?;
        db.execute_batch("PRAGMA synchronous = NORMAL")
            .map_err(|e| StockrsError::general(format!("동기화 설정 실패: {}", e)))?;
        db.execute_batch("PRAGMA cache_size = 10000")
            .map_err(|e| StockrsError::general(format!("캐시 크기 설정 실패: {}", e)))?;
        db.execute_batch("PRAGMA temp_store = MEMORY")
            .map_err(|e| StockrsError::general(format!("임시 저장소 설정 실패: {}", e)))?;

        Ok(())
    }

    /// 특정 시간의 현재가 조회 (1분봉 DB 사용)
    /// 시간대별 처리: 
    /// - trading_start_time 이전: trading_start_time의 값 반환
    /// - trading_start_time ~ trading_end_time: 해당 시간의 값 반환
    /// - trading_end_time 이후: trading_end_time의 값 반환
    pub fn get_current_price_at_time(&self, stockcode: &str, time_str: &str) -> StockrsResult<f64> {
        debug!(
            "🔍 [DbApi::get_current_price_at_time] 현재가 조회: 종목={}, 시간={}",
            stockcode, time_str
        );

        // 설정에서 거래 시간 가져오기
        let config = config::get_config()?;
        let trading_start_time = &config.market_hours.trading_start_time;
        let trading_end_time = &config.market_hours.trading_end_time;

        // 시간 형식 변환: "HH:MM:SS" -> "HHMM"
        let start_time_hhmm = trading_start_time.replace(":", "").chars().take(4).collect::<String>();
        let end_time_hhmm = trading_end_time.replace(":", "").chars().take(4).collect::<String>();

        if time_str.len() < 12 {
            return Err(StockrsError::price_inquiry(
                stockcode,
                "현재가",
                format!("잘못된 시간 형식: {}", time_str),
            ));
        }

        let date_part = &time_str[0..8]; // YYYYMMDD
        let time_part = &time_str[8..12]; // HHMM

        // 시간대별 처리
        let target_time = if time_part < start_time_hhmm.as_str() {
            // 거래 시작 시간 이전: 거래 시작 시간의 값 사용
            debug!(
                "🕐 [DbApi::get_current_price_at_time] 거래 시작 시간 이전: {} -> {} (종목: {})",
                time_str, format!("{}{}", date_part, start_time_hhmm), stockcode
            );
            format!("{}{}", date_part, start_time_hhmm)
        } else if time_part > end_time_hhmm.as_str() {
            // 거래 종료 시간 이후: 거래 종료 시간의 값 사용
            debug!(
                "🕐 [DbApi::get_current_price_at_time] 거래 종료 시간 이후: {} -> {} (종목: {})",
                time_str, format!("{}{}", date_part, end_time_hhmm), stockcode
            );
            format!("{}{}", date_part, end_time_hhmm)
        } else {
            // 거래 시간 내: 원래 시간 사용
            time_str.to_string()
        };

        // SQL 쿼리 실행 (테이블명 정규화: A 접두사 허용)
        let table_name = if stockcode.starts_with('A') {
            stockcode.to_string()
        } else {
            format!("A{}", stockcode)
        };
        let query = format!("SELECT close FROM \"{}\" WHERE date = ?", table_name);
        let mut stmt = self
            .minute_db_connection
            .prepare(&query)
            .map_err(|_e| {
                StockrsError::general(format!(
                    "SQL 준비 실패: {} (테이블: {})",
                    query, table_name
                ))
            })?;

        let result: Result<f64, _> = stmt.query_row([&target_time], |row| row.get(0));

        match result {
            Ok(current_price) if current_price > 0.0 => {
                debug!(
                    "✅ [DbApi::get_current_price_at_time] 현재가 조회 성공: 종목={}, 시간={}, 가격={}",
                    stockcode, target_time, current_price
                );
                Ok(current_price)
            }
            Ok(_) => Err(StockrsError::price_inquiry(
                stockcode,
                "현재가",
                format!("유효하지 않은 가격 데이터 (시간: {})", target_time),
            )),
            Err(_) => Err(StockrsError::price_inquiry(
                stockcode,
                "현재가",
                format!(
                    "해당 종목의 데이터가 1분봉 DB에 존재하지 않습니다 (시간: {})",
                    target_time
                ),
            )),
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
            StockrsError::general(format!("SQL 준비 실패: {}", tables_query))
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
            StockrsError::general(format!("SQL 준비 실패: {}", table_check_query))
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
            StockrsError::general(format!(
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
            StockrsError::general(format!(
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
            StockrsError::general(format!(
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
            StockrsError::general(format!(
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
            StockrsError::general(format!(
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
            StockrsError::general(format!(
                "SQL 실행 실패: {} (테이블: {})",
                count_query, stockcode
            ))
        })?;

        println!("📈 [DbApi::debug_db_structure] 전체 데이터 개수: {}", count);

        Ok(())
    }


}

// StockApi trait 구현
impl StockApi for DbApi {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn execute_order(&self, _order: &mut Order) -> StockrsResult<String> {
        Err(StockrsError::order_execution(
            "주문 실행".to_string(),
            "N/A".to_string(),
            0,
            "DbApi는 주문 실행을 지원하지 않습니다.".to_string(),
        ))
    }

    fn check_fill(&self, _order_id: &str) -> StockrsResult<bool> {
        Err(StockrsError::order_execution(
            "체결 확인".to_string(),
            "N/A".to_string(),
            0,
            "DbApi는 체결 확인을 지원하지 않습니다.".to_string(),
        ))
    }

    fn cancel_order(&self, _order_id: &str) -> StockrsResult<()> {
        Err(StockrsError::order_execution(
            "주문 취소".to_string(),
            "N/A".to_string(),
            0,
            "DbApi는 주문 취소를 지원하지 않습니다.".to_string(),
        ))
    }

    fn get_balance(&self) -> StockrsResult<AssetInfo> {
        Err(StockrsError::BalanceInquiry {
            reason: "DbApi는 잔고 조회를 지원하지 않습니다.".to_string(),
        })
    }

    fn get_avg_price(&self, _stockcode: &str) -> StockrsResult<f64> {
        Err(StockrsError::price_inquiry(
            "N/A",
            "평균가",
            "DbApi는 평균가 조회를 지원하지 않습니다.".to_string(),
        ))
    }

    fn get_current_price(&self, _stockcode: &str) -> StockrsResult<f64> {
        Err(StockrsError::price_inquiry(
            "N/A",
            "현재가",
            "DbApi는 현재가 조회를 지원하지 않습니다.".to_string(),
        ))
    }

    fn get_current_price_at_time(&self, stockcode: &str, time_str: &str) -> StockrsResult<f64> {
        self.get_current_price_at_time(stockcode, time_str)
    }

    fn set_current_time(&self, _time_str: &str) -> StockrsResult<()> {
        Ok(())
    }

    fn get_db_connection(&self) -> Option<rusqlite::Connection> {
        Connection::open(self.db_connection.path().unwrap_or_default()).ok()
    }

    fn get_daily_db_connection(&self) -> Option<rusqlite::Connection> {
        Connection::open(self.daily_db_connection.path().unwrap_or_default()).ok()
    }
}
