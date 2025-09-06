use crate::utility::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;

/// 최근 5일 평균 일간 수익률
pub fn calculate_day22_mean_return_5d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 5일간의 일간 수익률 계산
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 5)?;
    if returns.len() < 2 {
        return Ok(0.0);
    }
    
    let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
    // [-0.1, 0.1] 범위로 클리핑 후 정규화
    Ok(mean_return.max(-0.1).min(0.1) * 10.0)
}

/// 최근 20일 평균 일간 수익률
pub fn calculate_day22_mean_return_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 20)?;
    if returns.len() < 5 {
        return Ok(0.0);
    }
    
    let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
    Ok(mean_return.max(-0.1).min(0.1) * 10.0)
}

/// 최근 5일 수익률 표준편차
pub fn calculate_day22_volatility_5d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 5)?;
    if returns.len() < 2 {
        return Ok(0.0);
    }
    
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
    let std_dev = variance.sqrt();
    
    // [0, 0.05] 범위로 클리핑 후 정규화
    Ok((std_dev.max(0.0).min(0.05) * 20.0).min(1.0))
}

/// 최근 20일 수익률 표준편차
pub fn calculate_day22_volatility_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 20)?;
    if returns.len() < 5 {
        return Ok(0.0);
    }
    
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
    let std_dev = variance.sqrt();
    
    // [0, 0.08] 범위로 클리핑 후 정규화
    Ok((std_dev.max(0.0).min(0.08) * 12.5).min(1.0))
}

/// 최근 10일 수익률 분포 왜도
pub fn calculate_day22_skewness_10d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 10)?;
    if returns.len() < 3 {
        return Ok(0.0);
    }
    
    let skewness = calculate_skewness(&returns);
    // [-2, 2] 범위로 클리핑 후 정규화
    Ok((skewness.max(-2.0).min(2.0) + 2.0) / 4.0)
}

/// 최근 60일 수익률 분포 왜도
pub fn calculate_day22_skewness_60d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 60)?;
    if returns.len() < 10 {
        return Ok(0.0);
    }
    
    let skewness = calculate_skewness(&returns);
    Ok((skewness.max(-2.0).min(2.0) + 2.0) / 4.0)
}

/// 최근 10일 수익률 분포 첨도
pub fn calculate_day22_kurtosis_10d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 10)?;
    if returns.len() < 3 {
        return Ok(0.0);
    }
    
    let kurtosis = calculate_kurtosis(&returns);
    // [0, 10] 범위로 클리핑 후 정규화
    Ok((kurtosis.max(0.0).min(10.0)) / 10.0)
}

/// 최근 60일 수익률 분포 첨도
pub fn calculate_day22_kurtosis_60d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 60)?;
    if returns.len() < 10 {
        return Ok(0.0);
    }
    
    let kurtosis = calculate_kurtosis(&returns);
    Ok((kurtosis.max(0.0).min(10.0)) / 10.0)
}

/// 하락일만 표준편차 (Sortino denominator)
pub fn calculate_day22_downside_volatility_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 20)?;
    if returns.len() < 5 {
        return Ok(0.0);
    }
    
    let downside_returns: Vec<f64> = returns.iter()
        .filter(|&&r| r < 0.0)
        .cloned()
        .collect();
    
    if downside_returns.len() < 2 {
        return Ok(0.0);
    }
    
    let mean = downside_returns.iter().sum::<f64>() / downside_returns.len() as f64;
    let variance = downside_returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (downside_returns.len() - 1) as f64;
    let std_dev = variance.sqrt();
    
    // [0, 0.08] 범위로 클리핑 후 정규화
    Ok((std_dev.max(0.0).min(0.08) * 12.5).min(1.0))
}

/// 상승일만 표준편차
pub fn calculate_day22_upside_volatility_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 20)?;
    if returns.len() < 5 {
        return Ok(0.0);
    }
    
    let upside_returns: Vec<f64> = returns.iter()
        .filter(|&&r| r > 0.0)
        .cloned()
        .collect();
    
    if upside_returns.len() < 2 {
        return Ok(0.0);
    }
    
    let mean = upside_returns.iter().sum::<f64>() / upside_returns.len() as f64;
    let variance = upside_returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (upside_returns.len() - 1) as f64;
    let std_dev = variance.sqrt();
    
    // [0, 0.08] 범위로 클리핑 후 정규화
    Ok((std_dev.max(0.0).min(0.08) * 12.5).min(1.0))
}

/// 20일 수익률 사분위 범위 (IQR)
pub fn calculate_day22_return_iqr_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 20)?;
    if returns.len() < 5 {
        return Ok(0.0);
    }
    
    let mut sorted_returns = returns.clone();
    sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let q1_idx = (sorted_returns.len() as f64 * 0.25) as usize;
    let q3_idx = (sorted_returns.len() as f64 * 0.75) as usize;
    
    let iqr = sorted_returns[q3_idx] - sorted_returns[q1_idx];
    
    // [0, 0.1] 범위로 클리핑 후 정규화
    Ok((iqr.max(0.0).min(0.1) * 10.0).min(1.0))
}

/// 20일 최고 수익률 – 최저 수익률
pub fn calculate_day22_return_range_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 20)?;
    if returns.len() < 5 {
        return Ok(0.0);
    }
    
    let min_return = returns.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_return = returns.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let range = max_return - min_return;
    
    // [0, 0.2] 범위로 클리핑 후 정규화
    Ok((range.max(0.0).min(0.2) * 5.0).min(1.0))
}

/// 20일 샤프 비율 유사 지표
pub fn calculate_day22_sharpe_like_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 20)?;
    if returns.len() < 5 {
        return Ok(0.0);
    }
    
    let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|r| (r - mean_return).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
    let std_dev = variance.sqrt();
    
    if std_dev == 0.0 {
        return Ok(0.0);
    }
    
    let sharpe_like = mean_return / std_dev;
    // [-2, 2] 범위로 클리핑 후 정규화
    Ok((sharpe_like.max(-2.0).min(2.0) + 2.0) / 4.0)
}

/// 20일 소르티노 비율 유사 지표
pub fn calculate_day22_sortino_like_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 20)?;
    if returns.len() < 5 {
        return Ok(0.0);
    }
    
    let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
    let downside_returns: Vec<f64> = returns.iter()
        .filter(|&&r| r < 0.0)
        .cloned()
        .collect();
    
    if downside_returns.len() < 2 {
        return Ok(0.0);
    }
    
    let downside_mean = downside_returns.iter().sum::<f64>() / downside_returns.len() as f64;
    let downside_variance = downside_returns.iter().map(|r| (r - downside_mean).powi(2)).sum::<f64>() / (downside_returns.len() - 1) as f64;
    let downside_std_dev = downside_variance.sqrt();
    
    if downside_std_dev == 0.0 {
        return Ok(0.0);
    }
    
    let sortino_like = mean_return / downside_std_dev;
    // [-2, 2] 범위로 클리핑 후 정규화
    Ok((sortino_like.max(-2.0).min(2.0) + 2.0) / 4.0)
}

/// 상승일 std / 하락일 std
pub fn calculate_day22_gain_loss_std_ratio_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 20)?;
    if returns.len() < 5 {
        return Ok(0.5);
    }
    
    let upside_returns: Vec<f64> = returns.iter()
        .filter(|&&r| r > 0.0)
        .cloned()
        .collect();
    
    let downside_returns: Vec<f64> = returns.iter()
        .filter(|&&r| r < 0.0)
        .cloned()
        .collect();
    
    if upside_returns.len() < 2 || downside_returns.len() < 2 {
        return Ok(0.5);
    }
    
    let upside_std = calculate_std_dev(&upside_returns);
    let downside_std = calculate_std_dev(&downside_returns);
    
    if downside_std == 0.0 {
        return Ok(1.0);
    }
    
    let ratio = upside_std / downside_std;
    // [0.5, 2.0] 범위로 클리핑 후 정규화
    Ok(((ratio.max(0.5).min(2.0) - 0.5) / 1.5).min(1.0))
}

// 헬퍼 함수들
fn get_daily_returns(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
    lookback_days: usize,
) -> StockrsResult<Vec<f64>> {
    // trading_dates에 존재하지 않아도, DB에서 직접 직전 거래일 기준으로 조회
    // WHERE date < ? ORDER BY date DESC LIMIT lookback_days
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };

    let query = format!(
        "SELECT open, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT {}",
        table_name,
        lookback_days
    );

    let mut stmt = daily_db
        .prepare(&query)
        .map_err(|e| StockrsError::database_query(format!("prepare failed: {}", e)))?;

    let mut returns: Vec<f64> = Vec::new();
    let rows = stmt
        .query_map([&date], |row| {
            let open: f64 = row.get(0)?;
            let close: f64 = row.get(1)?;
            Ok((open, close))
        })
        .map_err(|e| StockrsError::database_query(format!("query_map failed: {}", e)))?;

    for row in rows {
        let (open, close) = row.map_err(|e| StockrsError::database_query(format!("row read failed: {}", e)))?;
        if open > 0.0 {
            returns.push((close - open) / open);
        }
    }

    // 부족해도 그대로 반환 (호출부에서 길이 체크함)
    Ok(returns)
}

fn calculate_skewness(returns: &[f64]) -> f64 {
    if returns.len() < 3 {
        return 0.0;
    }
    
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
    let std_dev = variance.sqrt();
    
    if std_dev == 0.0 {
        return 0.0;
    }
    
    let skewness = returns.iter()
        .map(|r| ((r - mean) / std_dev).powi(3))
        .sum::<f64>() / returns.len() as f64;
    
    skewness
}

fn calculate_kurtosis(returns: &[f64]) -> f64 {
    if returns.len() < 3 {
        return 0.0;
    }
    
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
    let std_dev = variance.sqrt();
    
    if std_dev == 0.0 {
        return 0.0;
    }
    
    let kurtosis = returns.iter()
        .map(|r| ((r - mean) / std_dev).powi(4))
        .sum::<f64>() / returns.len() as f64;
    
    kurtosis - 3.0 // excess kurtosis
}

fn calculate_std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}
