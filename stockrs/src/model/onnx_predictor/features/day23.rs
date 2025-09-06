use crate::utility::errors::{StockrsResult, StockrsError};
use rusqlite::Connection;

/// 1일 시차 자기상관계수
pub fn calculate_day23_autocorr_1d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 30)?;
    if returns.len() < 3 {
        return Ok(0.0);
    }
    
    let autocorr = calculate_autocorrelation(&returns, 1);
    // [-0.5, 0.5] 범위로 클리핑 후 정규화
    Ok((autocorr.max(-0.5).min(0.5) + 0.5) * 2.0)
}

/// 2일 시차 자기상관계수
pub fn calculate_day23_autocorr_2d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 30)?;
    if returns.len() < 4 {
        return Ok(0.0);
    }
    
    let autocorr = calculate_autocorrelation(&returns, 2);
    Ok((autocorr.max(-0.5).min(0.5) + 0.5) * 2.0)
}

/// 5일 시차 자기상관계수
pub fn calculate_day23_autocorr_5d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 40)?;
    if returns.len() < 7 {
        return Ok(0.0);
    }
    
    let autocorr = calculate_autocorrelation(&returns, 5);
    Ok((autocorr.max(-0.5).min(0.5) + 0.5) * 2.0)
}

/// 1일 시차 편자기상관
pub fn calculate_day23_partial_autocorr_1d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 30)?;
    if returns.len() < 3 {
        return Ok(0.0);
    }
    
    let partial_autocorr = calculate_partial_autocorrelation(&returns, 1);
    Ok((partial_autocorr.max(-0.5).min(0.5) + 0.5) * 2.0)
}

/// 20일 변동성의 표준편차 (volatility of volatility)
pub fn calculate_day23_return_vol_of_vol_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 40)?;
    if returns.len() < 20 {
        return Ok(0.0);
    }
    
    // 20일 윈도우로 변동성 계산
    let volatilities: Vec<f64> = returns.windows(20)
        .map(|window| {
            let mean = window.iter().sum::<f64>() / window.len() as f64;
            let variance = window.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (window.len() - 1) as f64;
            variance.sqrt()
        })
        .collect();
    
    if volatilities.len() < 2 {
        return Ok(0.0);
    }
    
    let vol_of_vol = calculate_std_dev(&volatilities);
    // [0, 0.02] 범위로 클리핑 후 정규화
    Ok((vol_of_vol.max(0.0).min(0.02) * 50.0).min(1.0))
}

/// 최근 20일 수익률 분포 엔트로피
pub fn calculate_day23_return_entropy_20d(
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
    
    let entropy = calculate_entropy(&returns);
    // [0, 5] 범위로 클리핑 후 정규화
    Ok((entropy.max(0.0).min(5.0)) / 5.0)
}

/// 95% 상위 수익률
pub fn calculate_day23_return_percentile_95_20d(
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
    
    let percentile_95 = calculate_percentile(&returns, 0.95);
    // [0, 0.1] 범위로 클리핑 후 정규화
    Ok((percentile_95.max(0.0).min(0.1) * 10.0).min(1.0))
}

/// 5% 하위 수익률
pub fn calculate_day23_return_percentile_5_20d(
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
    
    let percentile_5 = calculate_percentile(&returns, 0.05);
    // [-0.1, 0] 범위로 클리핑 후 정규화
    Ok((percentile_5.max(-0.1).min(0.0) + 0.1) * 10.0)
}

/// 5% ES (조건부 VaR)
pub fn calculate_day23_expected_shortfall_5p(
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
    
    let es_5p = calculate_expected_shortfall(&returns, 0.05);
    // [-0.1, 0] 범위로 클리핑 후 정규화
    Ok((es_5p.max(-0.1).min(0.0) + 0.1) * 10.0)
}

/// 5% Value-at-Risk (VaR)
pub fn calculate_day23_var_5p_20d(
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
    
    let var_5p = calculate_var(&returns, 0.05);
    // [-0.1, 0] 범위로 클리핑 후 정규화
    Ok((var_5p.max(-0.1).min(0.0) + 0.1) * 10.0)
}

/// 100일 허스트 지수 (0.5 기준 추세/평균회귀 성향)
pub fn calculate_day23_hurst_exponent_100d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 100)?;
    if returns.len() < 20 {
        return Ok(0.5);
    }
    
    let hurst = calculate_hurst_exponent(&returns);
    // [0, 1] 범위로 클리핑 (이미 정규화됨)
    Ok(hurst.max(0.0).min(1.0))
}

/// 장기 자기상관 기반 장기기억 점수 (0~1 정규화)
pub fn calculate_day23_long_memory_score(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 100)?;
    if returns.len() < 20 {
        return Ok(0.5);
    }
    
    let long_memory_score = calculate_long_memory_score(&returns);
    // [0, 1] 범위로 클리핑 (이미 정규화됨)
    Ok(long_memory_score.max(0.0).min(1.0))
}

/// 분포가 2모드 이상이면 1 (혼합 분포 신호)
pub fn calculate_day23_regime_switching_flag(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 60)?;
    if returns.len() < 20 {
        return Ok(0.0);
    }
    
    let regime_flag = detect_regime_switching(&returns);
    // [0, 1] 범위 (이진형)
    Ok(if regime_flag { 1.0 } else { 0.0 })
}

/// Hill 추정치 기반 꼬리 두께 지표
pub fn calculate_day23_tail_index_hill(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let returns = get_daily_returns(daily_db, stock_code, date, trading_dates, 100)?;
    if returns.len() < 20 {
        return Ok(0.5);
    }
    
    let tail_index = calculate_hill_tail_index(&returns);
    // [0.5, 5.0] 범위로 클리핑 후 정규화
    Ok(((tail_index.max(0.5).min(5.0) - 0.5) / 4.5).min(1.0))
}

/// 최근 20일 변동성 군집도 점수 (연속 큰 변동률 발생률)
pub fn calculate_day23_volatility_clustering_score(
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
    
    let clustering_score = calculate_volatility_clustering(&returns);
    // [0, 1] 범위로 클리핑 (이미 정규화됨)
    Ok(clustering_score.max(0.0).min(1.0))
}

// 헬퍼 함수들
fn get_daily_returns(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
    lookback_days: usize,
) -> StockrsResult<Vec<f64>> {
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };
    let query = format!(
        "SELECT open, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT {}",
        table_name,
        lookback_days
    );
    let mut stmt = daily_db
        .prepare(&query)
        .map_err(|e| StockrsError::database_query(format!("[get_daily_returns] prepare failed: {}", e)))?;
    let mut returns: Vec<f64> = Vec::new();
    let rows = stmt
        .query_map([&date], |row| {
            let open: f64 = row.get(0)?;
            let close: f64 = row.get(1)?;
            Ok((open, close))
        })
        .map_err(|e| StockrsError::database_query(format!("[get_daily_returns] query_map failed: {}", e)))?;
    for row in rows {
        let (open, close) = row.map_err(|e| StockrsError::database_query(format!("[get_daily_returns] row read failed: {}", e)))?;
        if open > 0.0 { returns.push((close - open) / open); }
    }
    Ok(returns)
}

fn calculate_autocorrelation(returns: &[f64], lag: usize) -> f64 {
    if returns.len() < lag + 1 {
        return 0.0;
    }
    
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
    
    if variance == 0.0 {
        return 0.0;
    }
    
    let mut autocorr = 0.0;
    for i in 0..returns.len() - lag {
        autocorr += (returns[i] - mean) * (returns[i + lag] - mean);
    }
    
    autocorr / ((returns.len() - lag) as f64 * variance)
}

fn calculate_partial_autocorrelation(returns: &[f64], lag: usize) -> f64 {
    // 간단한 구현: 일반 자기상관계수 반환
    calculate_autocorrelation(returns, lag)
}

fn calculate_std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

fn calculate_entropy(returns: &[f64]) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }
    
    // 간단한 엔트로피 계산: 표준편차 기반
    let std_dev = calculate_std_dev(returns);
    if std_dev == 0.0 {
        return 0.0;
    }
    
    // 로그 정규화된 표준편차
    (1.0 + std_dev).ln()
}

fn calculate_percentile(returns: &[f64], percentile: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    
    let mut sorted_returns = returns.to_vec();
    sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let index = (percentile * (sorted_returns.len() - 1) as f64).round() as usize;
    sorted_returns[index.min(sorted_returns.len() - 1)]
}

fn calculate_expected_shortfall(returns: &[f64], alpha: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    
    let var = calculate_var(returns, alpha);
    let tail_returns: Vec<f64> = returns.iter()
        .filter(|&&r| r <= var)
        .cloned()
        .collect();
    
    if tail_returns.is_empty() {
        return var;
    }
    
    tail_returns.iter().sum::<f64>() / tail_returns.len() as f64
}

fn calculate_var(returns: &[f64], alpha: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    
    let mut sorted_returns = returns.to_vec();
    sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let index = (alpha * (sorted_returns.len() - 1) as f64).round() as usize;
    sorted_returns[index.min(sorted_returns.len() - 1)]
}

fn calculate_hurst_exponent(returns: &[f64]) -> f64 {
    if returns.len() < 20 {
        return 0.5;
    }
    
    // 간단한 Hurst 지수 계산: R/S 분석 기반
    let rs_values: Vec<f64> = (10..=returns.len()/2).map(|n| {
        calculate_rs_statistic(returns, n)
    }).collect();
    
    if rs_values.is_empty() {
        return 0.5;
    }
    
    let log_n: Vec<f64> = (10..=returns.len()/2).map(|n| (n as f64).ln()).collect();
    let log_rs: Vec<f64> = rs_values.iter().map(|&rs| rs.ln()).collect();
    
    // 선형 회귀로 기울기 계산 (간단화)
    let slope = calculate_slope(&log_n, &log_rs);
    
    // Hurst 지수 = 기울기
    (slope + 1.0) / 2.0
}

fn calculate_rs_statistic(returns: &[f64], n: usize) -> f64 {
    if returns.len() < n {
        return 1.0;
    }
    
    // 간단한 R/S 통계 계산
    let window = &returns[..n];
    let mean = window.iter().sum::<f64>() / n as f64;
    let cumulative = window.iter().scan(0.0, |acc, &r| {
        *acc += r - mean;
        Some(*acc)
    }).collect::<Vec<f64>>();
    
        let range = cumulative.iter().fold(0.0_f64, |acc, &x| acc.max(x)) -
                cumulative.iter().fold(0.0_f64, |acc, &x| acc.min(x));
    let std_dev = calculate_std_dev(window);
    
    if std_dev == 0.0 {
        return 1.0;
    }
    
    range / std_dev
}

fn calculate_slope(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.len() < 2 {
        return 0.0;
    }
    
    let n = x.len() as f64;
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let sum_xy: f64 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
    let sum_x2: f64 = x.iter().map(|a| a * a).sum();
    
    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = n * sum_x2 - sum_x * sum_x;
    
    if denominator == 0.0 {
        return 0.0;
    }
    
    numerator / denominator
}

fn calculate_long_memory_score(returns: &[f64]) -> f64 {
    if returns.len() < 20 {
        return 0.5;
    }
    
    // 장기기억 점수: 자기상관계수의 지속성 기반
    let autocorr_1 = calculate_autocorrelation(returns, 1);
    let autocorr_5 = calculate_autocorrelation(returns, 5);
    let autocorr_10 = calculate_autocorrelation(returns, 10);
    
    // 자기상관계수가 천천히 감소하면 장기기억
    let memory_score = (autocorr_1.abs() + autocorr_5.abs() + autocorr_10.abs()) / 3.0;
    
    memory_score.max(0.0).min(1.0)
}

fn detect_regime_switching(returns: &[f64]) -> bool {
    if returns.len() < 20 {
        return false;
    }
    
    // 간단한 체제 전환 감지: 변동성의 급격한 변화
    let mid_point = returns.len() / 2;
    let first_half = &returns[..mid_point];
    let second_half = &returns[mid_point..];
    
    let vol_first = calculate_std_dev(first_half);
    let vol_second = calculate_std_dev(second_half);
    
    if vol_first == 0.0 || vol_second == 0.0 {
        return false;
    }
    
    let vol_ratio = vol_first.max(vol_second) / vol_first.min(vol_second);
    
    // 변동성 비율이 2배 이상이면 체제 전환으로 간주
    vol_ratio > 2.0
}

fn calculate_hill_tail_index(returns: &[f64]) -> f64 {
    if returns.len() < 20 {
        return 2.0;
    }
    
    // 간단한 Hill 추정치: 극값 기반
    let threshold = calculate_percentile(returns, 0.9);
    let tail_returns: Vec<f64> = returns.iter()
        .filter(|&&r| r > threshold)
        .cloned()
        .collect();
    
    if tail_returns.len() < 2 {
        return 2.0;
    }
    
    let tail_index = tail_returns.iter()
        .map(|&r| (r / threshold).ln())
        .sum::<f64>() / tail_returns.len() as f64;
    
    if tail_index <= 0.0 {
        return 2.0;
    }
    
    1.0 / tail_index
}

fn calculate_volatility_clustering(returns: &[f64]) -> f64 {
    if returns.len() < 5 {
        return 0.0;
    }
    
    // 변동성 군집도: 연속된 큰 변동률의 발생 빈도
    let threshold = calculate_percentile(returns, 0.8);
    let mut cluster_count = 0;
    let mut consecutive_count = 0;
    
    for &ret in returns {
        if ret.abs() > threshold {
            consecutive_count += 1;
        } else {
            if consecutive_count > 1 {
                cluster_count += 1;
            }
            consecutive_count = 0;
        }
    }
    
    // 마지막 연속 구간 처리
    if consecutive_count > 1 {
        cluster_count += 1;
    }
    
    let clustering_score = cluster_count as f64 / returns.len() as f64;
    clustering_score.max(0.0).min(1.0)
}
