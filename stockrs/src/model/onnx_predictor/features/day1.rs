use super::utils::get_morning_data;
use crate::utility::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;

/// day1_current_price_ratio: 현재가 / 시가 비율
pub fn calculate_current_price_ratio(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match (morning_data.get_last_close(), morning_data.get_last_open()) {
        (Some(close), Some(open)) => {
            if open > 0.0 {
                Ok(close / open)
            } else {
                Err(StockrsError::prediction(format!(
                    "종목 {}의 시가가 0이거나 유효하지 않습니다: {}",
                    stock_code, open
                )))
            }
        }
        _ => Err(StockrsError::prediction(format!(
            "종목 {}의 현재가 또는 시가 데이터를 찾을 수 없습니다",
            stock_code
        ))),
    }
}

/// day1_high_price_ratio: 고가 / 시가 비율
pub fn calculate_high_price_ratio(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match (morning_data.get_max_high(), morning_data.get_last_open()) {
        (Some(max_high), Some(open)) => {
            if open > 0.0 {
                Ok(max_high / open)
            } else {
                Err(StockrsError::prediction(format!(
                    "종목 {}의 시가가 0이거나 유효하지 않습니다: {}",
                    stock_code, open
                )))
            }
        }
        _ => Err(StockrsError::prediction(format!(
            "종목 {}의 고가 또는 시가 데이터를 찾을 수 없습니다",
            stock_code
        ))),
    }
}

/// day1_low_price_ratio: 저가 / 시가 비율
pub fn calculate_low_price_ratio(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match (morning_data.get_min_low(), morning_data.get_last_open()) {
        (Some(min_low), Some(open)) => {
            if open > 0.0 {
                Ok(min_low / open)
            } else {
                Err(StockrsError::prediction(format!(
                    "종목 {}의 시가가 0이거나 유효하지 않습니다: {}",
                    stock_code, open
                )))
            }
        }
        _ => Err(StockrsError::prediction(format!(
            "종목 {}의 저가 또는 시가 데이터를 찾을 수 없습니다",
            stock_code
        ))),
    }
}

/// day1_price_position_ratio: 현재가가 당일 고저가 범위에서 차지하는 위치 비율
pub fn calculate_price_position_ratio(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match morning_data.get_last_candle() {
        Some((close, _, high, low)) => {
            if high > low {
                Ok((close - low) / (high - low))
            } else {
                Ok(0.5)
            }
        }
        _ => Ok(0.5),
    }
}

/// day1_fourth_derivative: 4차 도함수 근사 계산
pub fn calculate_fourth_derivative(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 5 {
        return Ok(0.0);
    }

    // 4차 도함수 근사 계산
    let fifth = morning_data.closes.len() / 5;
    let first_fifth_avg = morning_data.closes[..fifth].iter().sum::<f64>() / fifth as f64;
    let last_fifth_avg = morning_data.closes[morning_data.closes.len() - fifth..]
        .iter()
        .sum::<f64>()
        / fifth as f64;
    let derivative = last_fifth_avg - first_fifth_avg;

    // 로그 정규화
    if derivative == 0.0 {
        Ok(0.0)
    } else {
        let sign = if derivative > 0.0 { 1.0 } else { -1.0 };
        Ok(sign * (1.0 + derivative.abs()).ln())
    }
}

/// day1_long_candle_ratio: 긴 양봉 비율
pub fn calculate_long_candle_ratio(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match morning_data.get_last_candle() {
        Some((_, open, high, low)) => {
            let candle_size = high - low;
            let avg_size = open * 0.02; // 시가의 2%

            if candle_size > avg_size {
                Ok(1.0)
            } else {
                Ok(0.0)
            }
        }
        _ => Ok(0.0),
    }
}

/// day1_fifth_derivative: 5차 도함수 근사 계산
pub fn calculate_fifth_derivative(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 6 {
        return Ok(0.0);
    }

    // 5차 도함수 근사 계산
    let sixth = morning_data.closes.len() / 6;
    let first_sixth_avg = morning_data.closes[..sixth].iter().sum::<f64>() / sixth as f64;
    let last_sixth_avg = morning_data.closes[morning_data.closes.len() - sixth..]
        .iter()
        .sum::<f64>()
        / sixth as f64;
    let derivative = last_sixth_avg - first_sixth_avg;

    // 로그 정규화
    if derivative == 0.0 {
        Ok(0.0)
    } else {
        let sign = if derivative > 0.0 { 1.0 } else { -1.0 };
        Ok(sign * (1.0 + derivative.abs()).ln())
    }
}

/// day1_sixth_derivative: 6차 도함수 근사 계산 (연속된 6개 점의 가격 변화 패턴)
pub fn calculate_sixth_derivative(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 6 {
        return Ok(0.0);
    }

    // 6차 도함수 근사 계산 (연속된 6개 점의 가격 변화 패턴)
    let mut direction_changes = 0;
    for i in 1..morning_data.closes.len() {
        if i > 1 {
            let prev_change = morning_data.closes[i - 1] - morning_data.closes[i - 2];
            let curr_change = morning_data.closes[i] - morning_data.closes[i - 1];
            if (prev_change > 0.0 && curr_change < 0.0) || (prev_change < 0.0 && curr_change > 0.0)
            {
                direction_changes += 1;
            }
        }
    }

    let derivative = direction_changes as f64 / (morning_data.closes.len() - 2) as f64;

    // 로그 정규화
    if derivative == 0.0 {
        Ok(0.0)
    } else {
        let sign = if derivative > 0.0 { 1.0 } else { -1.0 };
        Ok(sign * (1.0 + derivative.abs()).ln())
    }
}
