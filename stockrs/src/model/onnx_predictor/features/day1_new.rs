use crate::utility::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;
use super::utils::get_morning_data;

// 새로운 특징들 (farmer.db 의존성 없는 것들만)
pub fn calculate_vwap_position_ratio(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    let vwap = morning_data.get_vwap()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_vwap_position_ratio".to_string(),
            "VWAP 계산이 필요합니다".to_string(),
        ))?;
    
    let max_high = morning_data.get_max_high()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_vwap_position_ratio".to_string(),
            "고가 데이터가 필요합니다".to_string(),
        ))?;
    
    let min_low = morning_data.get_min_low()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_vwap_position_ratio".to_string(),
            "저가 데이터가 필요합니다".to_string(),
        ))?;
    
    if max_high > min_low {
        Ok((vwap - min_low) / (max_high - min_low))
    } else {
        Ok(0.5)
    }
}

pub fn calculate_volume_ratio(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    let current_volume = morning_data.get_current_volume()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_volume_ratio".to_string(),
            "현재 거래량 데이터가 필요합니다".to_string(),
        ))?;
    
    let avg_volume = morning_data.get_avg_volume()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_volume_ratio".to_string(),
            "평균 거래량 데이터가 필요합니다".to_string(),
        ))?;
    
    if avg_volume > 0.0 {
        Ok(current_volume / avg_volume)
    } else {
        Err(StockrsError::unsupported_feature(
            "calculate_volume_ratio".to_string(),
            "평균 거래량이 0입니다".to_string(),
        ))
    }
}

pub fn calculate_first_derivative(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    if morning_data.closes.len() < 2 {
        return Err(StockrsError::unsupported_feature(
            "calculate_first_derivative".to_string(),
            "최소 2개의 가격 데이터가 필요합니다".to_string(),
        ));
    }
    
    let first_half = morning_data.closes.len() / 2;
    let first_half_avg = morning_data.closes[..first_half].iter().sum::<f64>() / first_half as f64;
    let second_half_avg = morning_data.closes[first_half..].iter().sum::<f64>() / (morning_data.closes.len() - first_half) as f64;
    
    let derivative = second_half_avg - first_half_avg;
    
    // 로그 정규화 적용
    if derivative == 0.0 {
        Ok(0.0)
    } else {
        let sign = if derivative > 0.0 { 1.0 } else { -1.0 };
        Ok(sign * (1.0_f64 + derivative.abs()).ln())
    }
}

pub fn calculate_second_derivative(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    if morning_data.closes.len() < 3 {
        return Err(StockrsError::unsupported_feature(
            "calculate_second_derivative".to_string(),
            "최소 3개의 가격 데이터가 필요합니다".to_string(),
        ));
    }
    
    let third = morning_data.closes.len() / 3;
    let first_third_avg = morning_data.closes[..third].iter().sum::<f64>() / third as f64;
    let middle_third_avg = morning_data.closes[third..2*third].iter().sum::<f64>() / third as f64;
    let last_third_avg = morning_data.closes[2*third..].iter().sum::<f64>() / (morning_data.closes.len() - 2*third) as f64;
    
    let derivative = last_third_avg - 2.0 * middle_third_avg + first_third_avg;
    
    // 로그 정규화 적용
    if derivative == 0.0 {
        Ok(0.0)
    } else {
        let sign = if derivative > 0.0 { 1.0 } else { -1.0 };
        Ok(sign * (1.0_f64 + derivative.abs()).ln())
    }
}

pub fn calculate_third_derivative(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    if morning_data.closes.len() < 4 {
        return Err(StockrsError::unsupported_feature(
            "calculate_third_derivative".to_string(),
            "최소 4개의 가격 데이터가 필요합니다".to_string(),
        ));
    }
    
    let quarter = morning_data.closes.len() / 4;
    let first_quarter_avg = morning_data.closes[..quarter].iter().sum::<f64>() / quarter as f64;
    let second_quarter_avg = morning_data.closes[quarter..2*quarter].iter().sum::<f64>() / quarter as f64;
    let third_quarter_avg = morning_data.closes[2*quarter..3*quarter].iter().sum::<f64>() / quarter as f64;
    let last_quarter_avg = morning_data.closes[3*quarter..].iter().sum::<f64>() / (morning_data.closes.len() - 3*quarter) as f64;
    
    let derivative = last_quarter_avg - 3.0 * third_quarter_avg + 3.0 * second_quarter_avg - first_quarter_avg;
    
    // 로그 정규화 적용
    if derivative == 0.0 {
        Ok(0.0)
    } else {
        let sign = if derivative > 0.0 { 1.0 } else { -1.0 };
        Ok(sign * (1.0_f64 + derivative.abs()).ln())
    }
}

pub fn calculate_is_long_candle(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    let open = morning_data.get_last_open()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_is_long_candle".to_string(),
            "시가 데이터가 필요합니다".to_string(),
        ))?;
    
    let close = morning_data.get_last_close()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_is_long_candle".to_string(),
            "종가 데이터가 필요합니다".to_string(),
        ))?;
    
    let max_high = morning_data.get_max_high()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_is_long_candle".to_string(),
            "고가 데이터가 필요합니다".to_string(),
        ))?;
    
    let min_low = morning_data.get_min_low()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_is_long_candle".to_string(),
            "저가 데이터가 필요합니다".to_string(),
        ))?;
    
    let body_size = (close - open).abs();
    let total_range = max_high - min_low;
    
    if total_range > 0.0 {
        Ok(if body_size / total_range > 0.6 { 1.0 } else { 0.0 })
    } else {
        // 고가와 저가가 동일하면 장대양봉으로 보지 않음(0.0)으로 안전하게 처리
        Ok(0.0)
    }
} 