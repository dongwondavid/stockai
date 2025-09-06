use crate::utility::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;
use super::utils::{get_morning_data, get_daily_data, get_previous_trading_day, is_first_trading_day};

// 새로운 특징들 (farmer.db 의존성 없는 것들만)
pub fn calculate_position_vs_prev_high(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0);
    }

    // 전일 거래일 가져오기
    let prev_date = get_previous_trading_day(trading_dates, date)?;
    
    // 전일 고가 조회
    let prev_data = get_daily_data(daily_db, stock_code, &prev_date)?;
    let prev_high = prev_data.get_high()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_position_vs_prev_high".to_string(),
            "전일 고가 데이터가 필요합니다".to_string(),
        ))?;
    
    // 당일 현재가 조회
    let today_data = get_morning_data(db_5min, stock_code, date)?;
    let current_price = today_data.get_last_close()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_position_vs_prev_high".to_string(),
            "당일 현재가 데이터가 필요합니다".to_string(),
        ))?;
    
    if prev_high > 0.0 {
        Ok(current_price / prev_high)
    } else {
        Ok(1.0)
    }
}

pub fn calculate_position_within_prev_range(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.5);
    }

    // 전일 거래일 가져오기
    let prev_date = get_previous_trading_day(trading_dates, date)?;
    
    // 전일 고가, 저가 조회
    let prev_data = get_daily_data(daily_db, stock_code, &prev_date)?;
    let prev_high = prev_data.get_high()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_position_within_prev_range".to_string(),
            "전일 고가 데이터가 필요합니다".to_string(),
        ))?;
    let prev_low = prev_data.get_low()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_position_within_prev_range".to_string(),
            "전일 저가 데이터가 필요합니다".to_string(),
        ))?;
    
    // 당일 현재가 조회
    let today_data = get_morning_data(db_5min, stock_code, date)?;
    let current_price = today_data.get_last_close()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_position_within_prev_range".to_string(),
            "당일 현재가 데이터가 필요합니다".to_string(),
        ))?;
    
    if prev_high > prev_low {
        Ok((current_price - prev_low) / (prev_high - prev_low))
    } else {
        Ok(0.5)
    }
}

pub fn calculate_volume_ratio_vs_prevday(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0);
    }

    // 전일 거래일 가져오기
    let prev_date = get_previous_trading_day(trading_dates, date)?;
    
    // 전일 거래량 조회
    let prev_data = get_daily_data(daily_db, stock_code, &prev_date)?;
    let prev_volume = prev_data.get_volume()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_volume_ratio_vs_prevday".to_string(),
            "전일 거래량 데이터가 필요합니다".to_string(),
        ))?;
    
    // 당일 오전 거래량 조회
    let today_data = get_morning_data(db_5min, stock_code, date)?;
    let today_volume = today_data.get_current_volume()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_volume_ratio_vs_prevday".to_string(),
            "당일 거래량 데이터가 필요합니다".to_string(),
        ))?;
    
    if prev_volume > 0.0 {
        Ok(today_volume / prev_volume)
    } else {
        Ok(1.0)
    }
}

pub fn calculate_was_prevday_long_candle(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0);
    }

    // 전일 거래일 가져오기
    let prev_date = get_previous_trading_day(trading_dates, date)?;
    
    // 전일 데이터 조회
    let prev_data = get_daily_data(daily_db, stock_code, &prev_date)?;
    let prev_open = prev_data.get_open()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_was_prevday_long_candle".to_string(),
            "전일 시가 데이터가 필요합니다".to_string(),
        ))?;
    let prev_close = prev_data.get_close()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_was_prevday_long_candle".to_string(),
            "전일 종가 데이터가 필요합니다".to_string(),
        ))?;
    let prev_high = prev_data.get_high()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_was_prevday_long_candle".to_string(),
            "전일 고가 데이터가 필요합니다".to_string(),
        ))?;
    let prev_low = prev_data.get_low()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_was_prevday_long_candle".to_string(),
            "전일 저가 데이터가 필요합니다".to_string(),
        ))?;
    
    let body_size = (prev_close - prev_open).abs();
    let total_range = prev_high - prev_low;
    
    if total_range > 0.0 {
        Ok(if body_size / total_range > 0.6 { 1.0 } else { 0.0 })
    } else {
        Ok(0.0)
    }
}

pub fn calculate_gap_open_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0);
    }

    // 전일 거래일 가져오기
    let prev_date = get_previous_trading_day(trading_dates, date)?;
    
    // 전일 종가 조회
    let prev_data = get_daily_data(daily_db, stock_code, &prev_date)?;
    let prev_close = prev_data.get_close()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_gap_open_ratio".to_string(),
            "전일 종가 데이터가 필요합니다".to_string(),
        ))?;
    
    // 당일 시가 조회
    let today_data = get_morning_data(db_5min, stock_code, date)?;
    let today_open = today_data.get_last_open()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_gap_open_ratio".to_string(),
            "당일 시가 데이터가 필요합니다".to_string(),
        ))?;
    
    if prev_close > 0.0 {
        Ok((today_open - prev_close) / prev_close)
    } else {
        Ok(0.0)
    }
}

pub fn calculate_prev_gain_over_3(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0);
    }

    // 전일 거래일 가져오기
    let prev_date = get_previous_trading_day(trading_dates, date)?;
    
    // 전일 데이터 조회
    let prev_data = get_daily_data(daily_db, stock_code, &prev_date)?;
    let prev_open = prev_data.get_open()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_prev_gain_over_3".to_string(),
            "전일 시가 데이터가 필요합니다".to_string(),
        ))?;
    let prev_close = prev_data.get_close()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_prev_gain_over_3".to_string(),
            "전일 종가 데이터가 필요합니다".to_string(),
        ))?;
    
    if prev_open > 0.0 {
        let gain_ratio = (prev_close - prev_open) / prev_open;
        Ok(if gain_ratio >= 0.03 { 1.0 } else { 0.0 })
    } else {
        Ok(0.0)
    }
} 