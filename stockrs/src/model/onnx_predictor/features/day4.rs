use super::utils::{calculate_ema, get_daily_data, get_morning_data};
use crate::utility::errors::{StockrsError, StockrsResult};
use chrono::{Duration, NaiveDate};
use rusqlite::Connection;
use tracing::debug;

/// day4_macd_histogram_increasing: MACD 히스토그램 증가 여부
pub fn calculate_macd_histogram_increasing(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() >= 2 {
        let diff = morning_data.closes[morning_data.closes.len() - 1]
            - morning_data.closes[morning_data.closes.len() - 2];
        Ok(if diff > 0.0 { 1.0 } else { 0.0 })
    } else {
        Ok(0.5)
    }
}

/// day4_short_macd_cross_signal: 단기 MACD 골든크로스 신호
pub fn calculate_short_macd_cross_signal(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 4 {
        return Ok(0.0);
    }

    // EMA 3, 6 계산
    let ema3 = calculate_ema(&morning_data.closes, 3);
    let ema6 = calculate_ema(&morning_data.closes, 6);
    let macd = ema3 - ema6;

    // 이전 MACD 계산
    if morning_data.closes.len() >= 5 {
        let prev_closes: Vec<f64> = morning_data.closes[..morning_data.closes.len() - 1].to_vec();
        let prev_ema3 = calculate_ema(&prev_closes, 3);
        let prev_ema6 = calculate_ema(&prev_closes, 6);
        let prev_macd = prev_ema3 - prev_ema6;

        if (macd > prev_macd || prev_macd <= 0.0) && macd > 0.0 {
            return Ok(1.0);
        }
    }

    Ok(0.0)
}

/// day4_open_to_now_return: 시가 대비 현재가 수익률
pub fn calculate_open_to_now_return(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match (morning_data.get_last_close(), morning_data.get_last_open()) {
        (Some(close), Some(open)) => {
            if open > 0.0 {
                Ok((close - open) / open)
            } else {
                Ok(0.0)
            }
        }
        _ => Ok(0.0),
    }
}

/// day4_is_long_bull_candle: 긴 양봉 여부
pub fn calculate_is_long_bull_candle(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match morning_data.get_last_candle() {
        Some((close, open, high, low)) => {
            let body_size = (close - open).abs();
            let total_size = high - low;

            if total_size > 0.0 && body_size / total_size > 0.6 && close > open {
                Ok(1.0)
            } else {
                Ok(0.0)
            }
        }
        _ => Ok(0.0),
    }
}

/// day4_macd_histogram: MACD 히스토그램 값
pub fn calculate_macd_histogram(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 4 {
        return Ok(0.0);
    }

    let ema3 = calculate_ema(&morning_data.closes, 3);
    let ema6 = calculate_ema(&morning_data.closes, 6);
    let macd = ema3 - ema6;

    Ok(macd)
}

/// day4_pos_vs_high_5d: 5일 고점 대비 현재가 위치
pub fn calculate_pos_vs_high_5d(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 날짜 파싱 (YYYYMMDD 형식)
    let year = date[..4].parse::<i32>().map_err(|_| {
        StockrsError::prediction(format!("잘못된 연도 형식: {}", date))
    })?;
    let month = date[4..6].parse::<u32>().map_err(|_| {
        StockrsError::prediction(format!("잘못된 월 형식: {}", date))
    })?;
    let day = date[6..8].parse::<u32>().map_err(|_| {
        StockrsError::prediction(format!("잘못된 일 형식: {}", date))
    })?;

    let target_date = NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| {
        StockrsError::prediction(format!("잘못된 날짜 형식: {}", date))
    })?;

    // 테이블명 (일봉 DB는 A 접두사 포함)
    let table_name = stock_code;
    debug!(
        "5일 고점 조회 - 종목: {}, 테이블: {}",
        stock_code, table_name
    );

    // 당일 현재가 조회 (일봉 데이터에서 종가 사용)
    let daily_data = get_daily_data(daily_db, stock_code, date)?;
    let current_price = daily_data.get_close().ok_or_else(|| {
        StockrsError::prediction(format!(
            "당일 종가를 찾을 수 없습니다 (종목: {})",
            stock_code
        ))
    })?;

    // 최근 5일 고점 조회 (주식 시장이 열린 날 기준)
    let five_days_ago = target_date - Duration::days(5);
    let five_days_ago_str = five_days_ago.format("%Y%m%d").to_string();
    let target_date_str = target_date.format("%Y%m%d").to_string();
    debug!(
        "5일 고점 조회 범위: {} ~ {}",
        five_days_ago_str, target_date_str
    );

    let five_day_high: f64 = daily_db.query_row(
        &format!(
            "SELECT MAX(high) FROM \"{}\" WHERE date >= ? AND date <= ?",
            table_name
        ),
        rusqlite::params![&five_days_ago_str, &target_date_str],
        |row| row.get(0),
    )?;

    if five_day_high <= 0.0 {
        return Err(StockrsError::prediction(format!(
            "5일 고점이 유효하지 않습니다: {:.2}",
            five_day_high
        )));
    }

    Ok(current_price / five_day_high)
}

/// day4_rsi_value: RSI 지표 값
pub fn calculate_rsi_value(db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 6 {
        return Ok(50.0);
    }

    let mut gains = Vec::new();
    let mut losses = Vec::new();

    for i in 1..morning_data.closes.len() {
        let change = morning_data.closes[i] - morning_data.closes[i - 1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }

    let avg_gain = gains.iter().sum::<f64>() / gains.len() as f64;
    let avg_loss = losses.iter().sum::<f64>() / losses.len() as f64;

    let rsi = if avg_loss > 0.0 {
        100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
    } else {
        100.0
    };

    Ok(rsi)
}

/// day4_pos_vs_high_3d: 3일 고점 대비 현재가 위치
pub fn calculate_pos_vs_high_3d(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 날짜 파싱 (YYYYMMDD 형식)
    let year = date[..4].parse::<i32>().map_err(|_| {
        StockrsError::prediction(format!("잘못된 연도 형식: {}", date))
    })?;
    let month = date[4..6].parse::<u32>().map_err(|_| {
        StockrsError::prediction(format!("잘못된 월 형식: {}", date))
    })?;
    let day = date[6..8].parse::<u32>().map_err(|_| {
        StockrsError::prediction(format!("잘못된 일 형식: {}", date))
    })?;

    let target_date = NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| {
        StockrsError::prediction(format!("잘못된 날짜 형식: {}", date))
    })?;

    // 테이블명 (일봉 DB는 A 접두사 포함)
    let table_name = stock_code;
    debug!(
        "3일 고점 조회 - 종목: {}, 테이블: {}",
        stock_code, table_name
    );

    // 당일 현재가 조회 (일봉 데이터에서 종가 사용)
    let daily_data = get_daily_data(daily_db, stock_code, date)?;
    let current_price = daily_data.get_close().ok_or_else(|| {
        StockrsError::prediction(format!(
            "당일 종가를 찾을 수 없습니다 (종목: {})",
            stock_code
        ))
    })?;

    // 최근 3일 고점 조회 (주식 시장이 열린 날 기준)
    let three_days_ago = target_date - Duration::days(3);
    let three_days_ago_str = three_days_ago.format("%Y%m%d").to_string();
    let target_date_str = target_date.format("%Y%m%d").to_string();
    debug!(
        "3일 고점 조회 범위: {} ~ {}",
        three_days_ago_str, target_date_str
    );

    let three_day_high: f64 = daily_db.query_row(
        &format!(
            "SELECT MAX(high) FROM \"{}\" WHERE date >= ? AND date <= ?",
            table_name
        ),
        rusqlite::params![&three_days_ago_str, &target_date_str],
        |row| row.get(0),
    )?;

    if three_day_high <= 0.0 {
        return Err(StockrsError::prediction(format!(
            "3일 고점이 유효하지 않습니다: {:.2}",
            three_day_high
        )));
    }

    Ok(current_price / three_day_high)
}

/// 10일 고점 대비 현재 위치 계산
pub fn calculate_pos_vs_high_10d(daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    // 최근 10일 고점 대비 현재가 위치 비율
    let table_name = stock_code;
    let query = format!(
        "SELECT close FROM \"{}\" WHERE date <= ? ORDER BY date DESC LIMIT 11",
        table_name
    );
    
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([&date.parse::<i32>().unwrap_or(0)], |row| {
        Ok(row.get::<_, i32>(0)?)
    })?;
    
    let mut prices = Vec::new();
    for row in rows {
        prices.push(row? as f64);
    }
    
    // 가격을 시간순으로 정렬 (최신이 마지막)
    prices.reverse();
    
    if prices.len() < 11 {
        return Ok(0.0); // 데이터가 부족하면 0.0 반환
    }
    
    let current_price = prices[prices.len() - 1]; // 현재가
    let high_10d = prices.iter().take(10).fold(f64::NEG_INFINITY, |a, &b| a.max(b)); // 10일 고점
    
    if high_10d > 0.0 {
        // 현재가 / 10일 고점 비율
        Ok(current_price / high_10d)
    } else {
        Ok(0.0)
    }
}
