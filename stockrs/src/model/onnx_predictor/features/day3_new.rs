use crate::utility::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;
use super::utils::{get_morning_data, is_first_trading_day};
use chrono::Duration;

/// 테이블명 생성 시 A 접두사 중복 방지
fn get_table_name(stock_code: &str) -> String {
    if stock_code.starts_with('A') {
        stock_code.to_string()
    } else {
        format!("A{}", stock_code)
    }
}

// 새로운 특징들 (farmer.db 의존성 없는 것들만)
pub fn calculate_breaks_6month_high_with_long_candle(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0); // 첫 거래일이면 6개월 전고점 돌파 불가능
    }

    // 6개월 전 데이터 조회
    let six_months_ago = chrono::NaiveDate::parse_from_str(date, "%Y%m%d")
        .map_err(|e| StockrsError::parsing("날짜 파싱".to_string(), e.to_string()))?
        - Duration::days(180);
    
    let six_months_ago_str = six_months_ago.format("%Y%m%d").to_string();
    let table_name = get_table_name(stock_code);
    
    // 6개월 내 최고가 조회
    let six_month_high: f64 = daily_db
        .query_row(
            &format!(
                "SELECT MAX(high) FROM {} WHERE date >= ? AND date < ?",
                table_name
            ),
            rusqlite::params![six_months_ago_str, date],
            |row| row.get(0),
        )
        .map_err(|e| StockrsError::database("6개월 내 최고가 조회".to_string(), e.to_string()))?;
    
    if six_month_high <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 데이터 조회
    let today_data = get_morning_data(db_5min, stock_code, date)?;
    let today_open = today_data.get_last_open()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_breaks_6month_high_with_long_candle".to_string(),
            "당일 시가 데이터가 필요합니다".to_string(),
        ))?;
    let today_close = today_data.get_last_close()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_breaks_6month_high_with_long_candle".to_string(),
            "당일 종가 데이터가 필요합니다".to_string(),
        ))?;
    let today_high = today_data.get_max_high()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_breaks_6month_high_with_long_candle".to_string(),
            "당일 고가 데이터가 필요합니다".to_string(),
        ))?;
    let today_low = today_data.get_min_low()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_breaks_6month_high_with_long_candle".to_string(),
            "당일 저가 데이터가 필요합니다".to_string(),
        ))?;
    
    // 6개월 전고점 돌파 여부
    let breaks_6month_high = if today_high >= six_month_high { 1.0 } else { 0.0 };
    
    // 장대양봉 여부
    let body_size = (today_close - today_open).abs();
    let total_range = today_high - today_low;
    let is_long_candle = if total_range > 0.0 && body_size / total_range > 0.6 { 1.0 } else { 0.0 };
    
    // 두 조건 모두 만족하면 1.0, 아니면 0.0
    Ok(if breaks_6month_high > 0.0 && is_long_candle > 0.0 { 1.0 } else { 0.0 })
}

pub fn calculate_had_high_volume_gain_experience(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0); // 첫 거래일이면 과거 경험 없음
    }

    // 6개월 전 데이터 조회
    let six_months_ago = chrono::NaiveDate::parse_from_str(date, "%Y%m%d")
        .map_err(|e| StockrsError::parsing("날짜 파싱".to_string(), e.to_string()))?
        - Duration::days(180);
    
    let six_months_ago_str = six_months_ago.format("%Y%m%d").to_string();
    let table_name = get_table_name(stock_code);
    
    // 6개월 내 1000억 이상 거래대금 + 10% 이상 상승 경험 조회
    let count: i32 = daily_db
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM {} WHERE date >= ? AND date < ? 
                 AND volume * close >= 1000_0000_0000 AND (close - open) / open >= 0.1",
                table_name
            ),
            rusqlite::params![six_months_ago_str, date],
            |row| row.get(0),
        )
        .map_err(|e| StockrsError::database("고거래량 상승 경험 조회".to_string(), e.to_string()))?;
    
    Ok(if count > 0 { 1.0 } else { 0.0 })
}

pub fn calculate_is_ema_aligned(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0); // 첫 거래일이면 EMA 데이터 부족
    }

    let table_name = get_table_name(stock_code);
    
    // 최근 25일 종가 데이터 조회
    let prices: Vec<f64> = daily_db
        .prepare(&format!(
            "SELECT close FROM {} WHERE date < ? ORDER BY date DESC LIMIT 25",
            table_name
        ))?
        .query_map(rusqlite::params![date], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    
    if prices.len() < 20 {
        return Ok(0.0);
    }
    
    let mut sorted_prices = prices.clone();
    sorted_prices.reverse();
    
    let ema5 = calculate_ema(&sorted_prices, 5);
    let ema20 = calculate_ema(&sorted_prices, 20);
    
    Ok(if ema5 > ema20 { 1.0 } else { 0.0 })
}

fn calculate_ema(prices: &[f64], period: usize) -> f64 {
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

pub fn calculate_market_cap_over_3000b(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0); // 첫 거래일이면 시가총액 데이터 부족
    }

    let table_name = get_table_name(stock_code);
    
    // 시가총액 계산: 상장주식수 * 종가
    let (shares, close_price): (i64, i32) = daily_db
        .query_row(
            &format!(
                "SELECT 상장주식수, close FROM {} WHERE date < ? ORDER BY date DESC LIMIT 1",
                table_name
            ),
            rusqlite::params![date],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| StockrsError::database("시가총액 계산".to_string(), e.to_string()))?;
    
    let market_cap = shares as f64 * close_price as f64;
    
    Ok(if market_cap >= 3000000000000.0 { 1.0 } else { 0.0 })
}

pub fn calculate_near_price_boundary(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let today_data = get_morning_data(db_5min, stock_code, date)?;
    let current_price = today_data.get_last_close()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_near_price_boundary".to_string(),
            "현재가 데이터가 필요합니다".to_string(),
        ))?;
    
    // 호가 단위 경계값들 (2천, 5천, 2만, 5만, 20만, 50만, 200만, 500만)
    let boundaries = vec![
        2000.0, 5000.0, 20000.0, 50000.0, 200000.0, 500000.0, 2000000.0, 5000000.0,
    ];
    
    for boundary in boundaries {
        let lower_bound = boundary * 0.95;
        let upper_bound = boundary * 1.05;
        if current_price >= lower_bound && current_price <= upper_bound {
            return Ok(1.0);
        }
    }
    
    Ok(0.0)
}

pub fn calculate_foreign_ratio_3day_rising(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0); // 첫 거래일이면 외국인 비율 데이터 부족
    }

    let table_name = get_table_name(stock_code);
    
    // 최근 3개 거래일 조회
    let mut ratios = Vec::new();
    
    // 날짜를 INTEGER 형식으로 변환 (YYYYMMDD)
    let target_date_int = date.parse::<i32>()
        .map_err(|e| StockrsError::parsing("날짜 파싱".to_string(), e.to_string()))?;
    
    // 해당 종목의 최근 거래일들을 조회 (최대 10일 전까지 확인)
    let query = format!(
        "SELECT date FROM {} WHERE date < ? ORDER BY date DESC LIMIT 10",
        table_name
    );
    
    let trading_dates: Vec<i32> = daily_db
        .prepare(&query)?
        .query_map(rusqlite::params![target_date_int], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    
    // 최근 3개 거래일 선택
    for i in 0..3 {
        if i < trading_dates.len() {
            let trading_date_int = trading_dates[i];
            
            match daily_db.query_row(
                &format!("SELECT 외국인현보유비율 FROM {} WHERE date = ?", table_name),
                rusqlite::params![trading_date_int],
                |row| row.get::<_, f64>(0),
            ) {
                Ok(ratio) => {
                    ratios.push(ratio);
                }
                Err(_) => {
                    // 데이터가 없으면 0으로 채움
                    ratios.push(0.0);
                }
            }
        } else {
            ratios.push(0.0);
        }
    }
    
    // 3일 연속 상승 여부 확인
    if ratios.len() == 3 && ratios[0] > ratios[1] && ratios[1] > ratios[2] {
        Ok(1.0)
    } else {
        Ok(0.0)
    }
}

pub fn calculate_morning_volume_ratio(
    db_5min: &Connection,
    _daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 최근 5일 평균 거래량 계산
    let mut total_volume = 0.0;
    let mut day_count = 0;
    
    for i in 1..=5 {
        let prev_date = chrono::NaiveDate::parse_from_str(date, "%Y%m%d")
            .map_err(|e| StockrsError::parsing("날짜 파싱".to_string(), e.to_string()))?
            - Duration::days(i);
        
        let prev_date_str = prev_date.format("%Y%m%d").to_string();
        
        // 거래일인지 확인
        if trading_dates.contains(&prev_date_str) {
            let data = get_morning_data(db_5min, stock_code, &prev_date_str);
            if let Ok(data) = data {
                if let Some(volume) = data.get_current_volume() {
                    total_volume += volume;
                    day_count += 1;
                }
            }
        }
    }
    
    let avg_volume = if day_count > 0 {
        total_volume / day_count as f64
    } else {
        0.0
    };
    
    // 당일 오전 거래량 계산
    let today_data = get_morning_data(db_5min, stock_code, date)?;
    let morning_volume = today_data.get_current_volume()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_morning_volume_ratio".to_string(),
            "당일 거래량 데이터가 필요합니다".to_string(),
        ))?;
    
    if avg_volume > 0.0 {
        let ratio = morning_volume / avg_volume;
        // log1p 변환으로 스케일 안정화: ln(1 + x)
        // 원본 1 → log1p=0.69, 원본 10 → log1p=2.4, 원본 1000 → log1p=6.9
        Ok((1.0 + ratio).ln())
    } else {
        Ok(0.0)
    }
}

pub fn calculate_consecutive_3_positive_candles(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let today_data = get_morning_data(db_5min, stock_code, date)?;
    
    if today_data.closes.len() < 3 {
        return Ok(0.0);
    }
    
    let mut consecutive_count = 0;
    for i in 1..today_data.closes.len() {
        let prev_close = today_data.closes[i - 1];
        let curr_close = today_data.closes[i];
        
        if curr_close > prev_close {
            consecutive_count += 1;
            if consecutive_count >= 3 {
                return Ok(1.0);
            }
        } else {
            consecutive_count = 0;
        }
    }
    
    Ok(0.0)
} 