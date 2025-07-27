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

/// RSI(상대강도지수) 계산 함수
pub fn calculate_rsi(prices: &[f64], period: usize) -> f64 {
    if prices.len() < period + 1 {
        return 50.0; // 데이터가 부족하면 중립값 반환
    }

    let mut gains = Vec::new();
    let mut losses = Vec::new();

    // 가격 변화 계산
    for i in 1..prices.len() {
        let change = prices[i] - prices[i - 1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }

    // 평균 이익과 손실 계산
    let avg_gain = gains.iter().sum::<f64>() / gains.len() as f64;
    let avg_loss = losses.iter().sum::<f64>() / losses.len() as f64;

    if avg_loss == 0.0 {
        return 100.0; // 손실이 없으면 RSI 100
    }

    let rs = avg_gain / avg_loss;
    let rsi = 100.0 - (100.0 / (1.0 + rs));

    rsi
}

/// 9시반 이전 5분봉 데이터 구조체
#[derive(Debug)]
pub struct MorningData {
    pub closes: Vec<f64>,
    pub opens: Vec<f64>,
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
    pub volumes: Vec<f64>,
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

    pub fn get_current_volume(&self) -> Option<f64> {
        self.volumes.last().copied()
    }

    pub fn get_avg_volume(&self) -> Option<f64> {
        if self.volumes.is_empty() {
            None
        } else {
            Some(self.volumes.iter().sum::<f64>() / self.volumes.len() as f64)
        }
    }

    pub fn get_vwap(&self) -> Option<f64> {
        if self.volumes.is_empty() || self.closes.is_empty() {
            None
        } else {
            let total_volume_price: f64 = self.closes.iter()
                .zip(self.volumes.iter())
                .map(|(close, volume)| close * volume)
                .sum();
            let total_volume: f64 = self.volumes.iter().sum();
            
            if total_volume > 0.0 {
                Some(total_volume_price / total_volume)
            } else {
                None
            }
        }
    }
}

/// 일봉 데이터 구조체
#[derive(Debug)]
pub struct DailyData {
    pub closes: Vec<f64>,
    pub opens: Vec<f64>,
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
    pub volumes: Vec<f64>,
}

impl DailyData {
    pub fn get_close(&self) -> Option<f64> {
        self.closes.first().copied()
    }

    pub fn get_open(&self) -> Option<f64> {
        self.opens.first().copied()
    }

    pub fn get_high(&self) -> Option<f64> {
        self.highs.first().copied()
    }

    pub fn get_low(&self) -> Option<f64> {
        self.lows.first().copied()
    }

    pub fn get_volume(&self) -> Option<f64> {
        self.volumes.first().copied()
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
        "SELECT close, open, high, low, volume FROM \"{}\" WHERE date >= ? AND date <= ? ORDER BY date",
        table_name
    );

    let mut stmt = db.prepare(&query)?;
    let rows = stmt.query_map([&date_start, &date_end], |row| {
        Ok((
            row.get::<_, i32>(0)?, // close
            row.get::<_, i32>(1)?, // open
            row.get::<_, i32>(2)?, // high
            row.get::<_, i32>(3)?, // low
            row.get::<_, i32>(4)?, // volume
        ))
    })?;

    // 벡터 사전 할당으로 메모리 최적화
    let mut closes = Vec::new();
    let mut opens = Vec::new();
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut volumes = Vec::new();

    for row in rows {
        let (close, open, high, low, volume) = row?;
        closes.push(close as f64);
        opens.push(open as f64);
        highs.push(high as f64);
        lows.push(low as f64);
        volumes.push(volume as f64);
    }

    Ok(MorningData {
        closes,
        opens,
        highs,
        lows,
        volumes,
    })
}

/// 일봉 데이터를 조회하는 함수 - 최적화됨
pub fn get_daily_data(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<DailyData> {
    use tracing::{info, warn};
    info!("🔍 [get_daily_data] 일봉 데이터 조회 시작 (종목: {}, 날짜: {})", stock_code, date);
    let table_name = stock_code.to_string();
    info!("📋 [get_daily_data] 테이블명: '{}'", table_name);

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

    info!("📊 [get_daily_data] 테이블 존재 여부: {} (종목: {})", table_exists, stock_code);

    if table_exists == 0 {
        warn!("❌ [get_daily_data] 테이블이 존재하지 않습니다 (종목: {}, 테이블: {})", stock_code, table_name);
        return Err(StockrsError::database_query(format!(
            "일봉 테이블이 존재하지 않습니다: {} (종목: {})",
            table_name, stock_code
        )));
    }

    // 해당 날짜의 데이터 존재 여부 확인 (최적화된 쿼리)
    let count_query = format!("SELECT COUNT(*) FROM \"{}\" WHERE date = ?", table_name);
    info!("🔍 [get_daily_data] 데이터 존재 확인 쿼리: '{}' (파라미터: date='{}')", count_query, date);
    
    let data_exists: i64 = daily_db
        .query_row(
            &count_query,
            rusqlite::params![&date],
            |row| row.get(0),
        )
        .map_err(|e| {
            warn!("❌ [get_daily_data] 데이터 존재 확인 쿼리 실패: {} (종목: {}, 테이블: {}, 날짜: {})", e, stock_code, table_name, date);
            StockrsError::database_query(format!(
                "일봉 데이터 존재 여부 확인 실패: {} (날짜: {})",
                table_name, date
            ))
        })?;

    info!("📊 [get_daily_data] 데이터 존재 개수: {} (종목: {}, 테이블: {}, 날짜: {})", data_exists, stock_code, table_name, date);

    if data_exists == 0 {
        warn!("❌ [get_daily_data] 데이터가 없습니다 (종목: {}, 테이블: {}, 날짜: {})", stock_code, table_name, date);
        return Err(StockrsError::database_query(format!(
            "전일 데이터가 없습니다: {} (테이블: {}, 날짜: {})",
            stock_code, table_name, date
        )));
    }

    // 최적화된 쿼리 (한 번에 모든 데이터 조회)
    let query = format!(
        "SELECT close, open, high, low, volume FROM \"{}\" WHERE date = ?",
        table_name
    );
    info!("🔍 [get_daily_data] 데이터 조회 쿼리: '{}' (파라미터: date='{}')", query, date);

    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([&date], |row| {
        Ok((
            row.get::<_, i32>(0)?, // close
            row.get::<_, i32>(1)?, // open
            row.get::<_, i32>(2)?, // high
            row.get::<_, i32>(3)?, // low
            row.get::<_, i32>(4)?, // volume
        ))
    })?;

    // 벡터 사전 할당으로 메모리 최적화
    let mut closes = Vec::new();
    let mut opens = Vec::new();
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut volumes = Vec::new();

    let mut row_count = 0;
    for row in rows {
        let (close, open, high, low, volume) = row?;
        closes.push(close as f64);
        opens.push(open as f64);
        highs.push(high as f64);
        lows.push(low as f64);
        volumes.push(volume as f64);
        row_count += 1;
    }

    info!("✅ [get_daily_data] 데이터 조회 완료 (종목: {}, 날짜: {}, 행 개수: {})", stock_code, date, row_count);

    Ok(DailyData {
        closes,
        opens,
        highs,
        lows,
        volumes,
    })
}

/// 이전 거래일을 찾는 함수 - 1일봉 날짜 목록 사용
pub fn get_previous_trading_day(day_dates: &[String], date: &str) -> StockrsResult<String> {
    use tracing::{info, warn};
    
    info!("🔍 [get_previous_trading_day] 전일 찾기 시작 (날짜: {}, 전체 날짜 수: {})", date, day_dates.len());
    
    // 빈 배열 체크
    if day_dates.is_empty() {
        warn!("❌ [get_previous_trading_day] 거래일 배열이 비어있습니다");
        return Err(StockrsError::prediction(format!(
            "거래일 배열이 비어있습니다"
        )));
    }
    
    // 첫 번째 날짜인지 확인
    if day_dates[0] == date {
        warn!("❌ [get_previous_trading_day] 첫 번째 거래일이므로 전일이 없습니다: {}", date);
        return Err(StockrsError::prediction(format!(
            "첫 번째 거래일이므로 전일이 없습니다: {}",
            date
        )));
    }
    
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
        let prev_date = day_dates[left - 1].clone();
        info!("✅ [get_previous_trading_day] 전일 찾기 완료: {} -> {}", date, prev_date);
        Ok(prev_date)
    } else {
        warn!("❌ [get_previous_trading_day] 전일을 찾을 수 없습니다: {}", date);
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
        dates
    });
    
    special_dates.contains(date)
}

/// 첫 거래일인지 확인하는 함수
pub fn is_first_trading_day(daily_db: &Connection, stock_code: &str, date: &str, day_dates: &[String]) -> StockrsResult<bool> {
    use tracing::{info, warn};
    info!("🔍 [is_first_trading_day] 첫 거래일 확인 중 (종목: {}, 날짜: {})", stock_code, date);
    
    // 빈 배열 체크
    if day_dates.is_empty() {
        warn!("❌ [is_first_trading_day] 거래일 배열이 비어있습니다");
        return Err(StockrsError::prediction(format!(
            "거래일 배열이 비어있습니다"
        )));
    }
    
    // 첫 번째 날짜인지 확인
    if day_dates[0] == date {
        info!("✅ [is_first_trading_day] 첫 번째 거래일이므로 첫 거래일로 판단: {}", date);
        return Ok(true);
    }
    
    // 전 거래일 가져오기
    let previous_date = get_previous_trading_day(day_dates, date)?;
    info!("📅 [is_first_trading_day] 전 거래일: {} (종목: {})", previous_date, stock_code);
    
    // 전 거래일에 해당 종목 데이터가 있는지 확인
    let table_name = stock_code;
    let count_query = format!("SELECT COUNT(*) FROM \"{}\" WHERE date = ?", table_name);
    info!("🔍 [is_first_trading_day] 데이터 확인 쿼리: '{}' (파라미터: date='{}')", count_query, previous_date);
    
    let count: i64 = daily_db
        .query_row(
            &count_query,
            [&previous_date],
            |row| row.get(0),
        )
        .map_err(|e| {
            warn!("❌ [is_first_trading_day] 데이터 확인 쿼리 실패: {} (종목: {}, 테이블: {}, 전일: {})", e, stock_code, table_name, previous_date);
            StockrsError::database_query(format!(
                "종목 {}의 전 거래일 데이터 확인 실패",
                table_name
            ))
        })?;
    
    info!("📊 [is_first_trading_day] 전 거래일 데이터 개수: {} (종목: {}, 전일: {})", count, stock_code, previous_date);
    
    // 전 거래일에 데이터가 없으면 첫 거래일
    let is_first = count == 0;
    info!("✅ [is_first_trading_day] 첫 거래일 여부: {} (종목: {}, 날짜: {})", is_first, stock_code, date);
    Ok(is_first)
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
