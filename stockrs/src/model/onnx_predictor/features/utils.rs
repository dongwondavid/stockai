use crate::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;
use tracing::debug;

/// EMA (지수이동평균) 계산 함수
pub fn calculate_ema(prices: &[f64], period: usize) -> f64 {
    if prices.is_empty() || period == 0 {
        return 0.0;
    }

    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut ema = prices[0];

    for &price in prices.iter().skip(1) {
        ema = (price * multiplier) + (ema * (1.0 - multiplier));
    }

    ema
}

/// 9시반 이전 5분봉 데이터 구조체
#[derive(Debug)]
pub struct MorningData {
    pub closes: Vec<f64>,
    pub opens: Vec<f64>,
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
}

impl MorningData {
    pub fn get_last_close(&self) -> Option<f64> {
        self.closes.last().copied()
    }

    pub fn get_last_open(&self) -> Option<f64> {
        self.opens.last().copied()
    }

    pub fn get_max_high(&self) -> Option<f64> {
        self.highs
            .iter()
            .fold(None, |max, &val| Some(max.map_or(val, |m| m.max(val))))
    }

    pub fn get_min_low(&self) -> Option<f64> {
        self.lows
            .iter()
            .fold(None, |min, &val| Some(min.map_or(val, |m| m.min(val))))
    }

    pub fn get_last_candle(&self) -> Option<(f64, f64, f64, f64)> {
        if self.closes.is_empty() {
            None
        } else {
            let idx = self.closes.len() - 1;
            Some((
                self.closes[idx],
                self.opens[idx],
                self.highs[idx],
                self.lows[idx],
            ))
        }
    }
}

/// 일봉 데이터 구조체
#[derive(Debug)]
pub struct DailyData {
    pub closes: Vec<f64>,
}

impl DailyData {
    pub fn get_close(&self) -> Option<f64> {
        self.closes.first().copied()
    }
}

/// 9시반 이전 5분봉 데이터를 조회하는 공통 함수 - 최적화됨
pub fn get_morning_data(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<MorningData> {
    let table_name = stock_code.to_string();
    let date_start = format!("{}0900", date);
    let date_end = format!("{}0930", date);

    // 테이블 존재 여부 확인 (최적화된 쿼리)
    let table_exists: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            rusqlite::params![&table_name],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query(format!("테이블 존재 여부 확인 실패: {}", table_name))
        })?;

    if table_exists == 0 {
        return Err(StockrsError::database_query(format!(
            "5분봉 테이블이 존재하지 않습니다: {} (종목: {})",
            table_name, stock_code
        )));
    }

    // 해당 날짜의 데이터 존재 여부 확인 (최적화된 쿼리)
    let data_exists: i64 = db
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM \"{}\" WHERE date >= ? AND date <= ?",
                table_name
            ),
            rusqlite::params![&date_start, &date_end],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query(format!(
                "데이터 존재 여부 확인 실패: {} (범위: {} ~ {})",
                table_name, date_start, date_end
            ))
        })?;

    if data_exists == 0 {
        return Err(StockrsError::database_query(format!(
            "9시반 이전 데이터가 없습니다: {} (종목: {}, 범위: {} ~ {})",
            stock_code, table_name, date_start, date_end
        )));
    }

    // 최적화된 쿼리 (한 번에 모든 데이터 조회)
    let query = format!(
        "SELECT close, open, high, low FROM \"{}\" WHERE date >= ? AND date <= ? ORDER BY date",
        table_name
    );

    let mut stmt = db.prepare(&query)?;
    let rows = stmt.query_map([&date_start, &date_end], |row| {
        Ok((
            row.get::<_, i32>(0)?, // close
            row.get::<_, i32>(1)?, // open
            row.get::<_, i32>(2)?, // high
            row.get::<_, i32>(3)?, // low
        ))
    })?;

    // 벡터 사전 할당으로 메모리 최적화
    let mut closes = Vec::new();
    let mut opens = Vec::new();
    let mut highs = Vec::new();
    let mut lows = Vec::new();

    for row in rows {
        let (close, open, high, low) = row?;
        closes.push(close as f64);
        opens.push(open as f64);
        highs.push(high as f64);
        lows.push(low as f64);
    }

    Ok(MorningData {
        closes,
        opens,
        highs,
        lows,
    })
}

/// 일봉 데이터를 조회하는 함수 - 최적화됨
pub fn get_daily_data(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<DailyData> {
    let table_name = stock_code.to_string();

    // 테이블 존재 여부 확인 (최적화된 쿼리)
    let table_exists: i64 = daily_db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            rusqlite::params![&table_name],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query(format!("일봉 테이블 존재 여부 확인 실패: {}", table_name))
        })?;

    if table_exists == 0 {
        return Err(StockrsError::database_query(format!(
            "일봉 테이블이 존재하지 않습니다: {} (종목: {})",
            table_name, stock_code
        )));
    }

    // 해당 날짜의 데이터 존재 여부 확인 (최적화된 쿼리)
    let data_exists: i64 = daily_db
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\" WHERE date = ?", table_name),
            rusqlite::params![&date],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query(format!(
                "일봉 데이터 존재 여부 확인 실패: {} (날짜: {})",
                table_name, date
            ))
        })?;

    if data_exists == 0 {
        return Err(StockrsError::database_query(format!(
            "전일 데이터가 없습니다: {} (테이블: {}, 날짜: {})",
            stock_code, table_name, date
        )));
    }

    // 최적화된 쿼리 (한 번에 모든 데이터 조회)
    let query = format!(
        "SELECT close, open, high, low FROM \"{}\" WHERE date = ?",
        table_name
    );

    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([&date], |row| {
        Ok((
            row.get::<_, i32>(0)?, // close
            row.get::<_, i32>(1)?, // open
            row.get::<_, i32>(2)?, // high
            row.get::<_, i32>(3)?, // low
        ))
    })?;

    // 벡터 사전 할당으로 메모리 최적화
    let mut closes = Vec::new();

    for row in rows {
        let (close, _open, _high, _low) = row?;
        closes.push(close as f64);
    }

    Ok(DailyData { closes })
}

/// 거래일 리스트를 조회하는 함수 - 최적화됨
pub fn get_trading_dates_list(db: &Connection) -> StockrsResult<Vec<String>> {
    // 먼저 answer_v3 테이블이 있는지 확인
    let table_exists: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = 'answer_v3'",
            [],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query("answer_v3 테이블 존재 여부 확인 실패".to_string())
        })?;

    if table_exists > 0 {
        // answer_v3 테이블이 있으면 사용
        let query = "SELECT DISTINCT date FROM answer_v3 ORDER BY date";

        let mut stmt = db.prepare(query)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        // 벡터 사전 할당으로 메모리 최적화
        let mut trading_dates = Vec::new();

        for row in rows {
            let date = row?;
            trading_dates.push(date);
        }

        debug!(
            "answer_v3에서 거래일 리스트 로드 완료: {}개",
            trading_dates.len()
        );
        return Ok(trading_dates);
    }

    // answer_v3 테이블이 없으면 사용 가능한 테이블에서 거래일 추출
    let tables_query = "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'answer_%'";
    let mut stmt = db.prepare(tables_query)?;
    let tables: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    if tables.is_empty() {
        return Err(StockrsError::database_query(
            "사용 가능한 테이블이 없습니다.".to_string(),
        ));
    }

    // 첫 번째 테이블에서 거래일 추출 (YYYYMMDD 형식으로 변환)
    let first_table = &tables[0];
    let query = format!(
        "SELECT DISTINCT date/10000 as ymd FROM \"{}\" ORDER BY ymd",
        first_table
    );

    let mut stmt = db.prepare(&query)?;
    let rows = stmt.query_map([], |row| {
        let ymd = row.get::<_, i64>(0)?;
        Ok(ymd.to_string())
    })?;

    // 벡터 사전 할당으로 메모리 최적화
    let mut trading_dates = Vec::new();

    for row in rows {
        let date = row?;
        trading_dates.push(date);
    }

    debug!(
        "테이블 {}에서 거래일 리스트 로드 완료: {}개",
        first_table,
        trading_dates.len()
    );
    Ok(trading_dates)
}

/// 이전 거래일을 찾는 함수 - 최적화됨
pub fn get_previous_trading_day(trading_dates: &[String], date: &str) -> StockrsResult<String> {
    // 이진 탐색으로 최적화 (정렬된 배열에서)
    let mut left = 0;
    let mut right = trading_dates.len();

    while left < right {
        let mid = (left + right) / 2;
        let date_str = date.to_string();
        if trading_dates[mid] < date_str {
            left = mid + 1;
        } else {
            right = mid;
        }
    }

    // 이전 거래일 찾기
    if left > 0 {
        Ok(trading_dates[left - 1].clone())
    } else {
        Err(StockrsError::prediction(format!(
            "이전 거래일을 찾을 수 없습니다: {}",
            date
        )))
    }
}
