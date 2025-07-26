use crate::utility::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::OnceLock;

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
    let (date_start, date_end) = get_time_range_for_date(date);

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
        let time_range = if is_special_trading_date(date) { "10시~10시반" } else { "9시~9시반" };
        return Err(StockrsError::database_query(format!(
            "{} 이전 데이터가 없습니다: {} (종목: {}, 범위: {} ~ {})",
            time_range, stock_code, table_name, date_start, date_end
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

/// 이전 거래일을 찾는 함수 - 1일봉 날짜 목록 사용
pub fn get_previous_trading_day(day_dates: &[String], date: &str) -> StockrsResult<String> {
    
    // 이진 탐색으로 최적화 (정렬된 배열에서)
    let mut left = 0;
    let mut right = day_dates.len();

    while left < right {
        let mid = (left + right) / 2;
        let date_str = date.to_string();
        if day_dates[mid] < date_str {
            left = mid + 1;
        } else {
            right = mid;
        }
    }

    // 이전 거래일 찾기
    if left > 0 {
        Ok(day_dates[left - 1].clone())
    } else {
        Err(StockrsError::prediction(format!(
            "이전 거래일을 찾을 수 없습니다: {}",
            date
        )))
    }
}

/// 특이한 거래일인지 판별하는 함수
pub fn is_special_trading_date(date: &str) -> bool {
    static SPECIAL_DATES: OnceLock<HashSet<String>> = OnceLock::new();
    
    let special_dates = SPECIAL_DATES.get_or_init(|| {
        let mut dates = HashSet::new();
        if let Ok(file) = File::open("data/start1000.txt") {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                dates.insert(line.trim().to_string());
            }
        }
        println!("[DEBUG] 특이한 날짜 목록 로드됨: {:?}", dates);
        dates
    });
    
    special_dates.contains(date)
}

/// 첫 거래일인지 확인하는 함수
pub fn is_first_trading_day(daily_db: &Connection, stock_code: &str, date: &str, day_dates: &[String]) -> StockrsResult<bool> {
    // 전 거래일 가져오기
    let previous_date = get_previous_trading_day(day_dates, date)?;
    
    // 전 거래일에 해당 종목 데이터가 있는지 확인
    let table_name = stock_code;
    let count: i64 = daily_db
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\" WHERE date = ?", table_name),
            [&previous_date],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query(format!(
                "종목 {}의 전 거래일 데이터 확인 실패",
                table_name
            ))
        })?;
    
    // 전 거래일에 데이터가 없으면 첫 거래일
    Ok(count == 0)
}

/// 날짜에 따른 시간 범위를 반환하는 함수
pub fn get_time_range_for_date(date: &str) -> (String, String) {
    if is_special_trading_date(date) {
        // 특이한 날짜: 10:00~10:30
        (format!("{}1000", date), format!("{}1030", date))
    } else {
        // 일반 날짜: 09:00~09:30
        (format!("{}0900", date), format!("{}0930", date))
    }
}
