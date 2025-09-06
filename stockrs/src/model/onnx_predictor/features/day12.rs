use crate::utility::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;

/// Helper: fetch recent daily closes up to the previous trading day (t-1).
fn get_recent_daily_closes(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
    max_lookback: usize,
) -> StockrsResult<Vec<f64>> {
    // Identify index of current date in trading_dates
    let idx = trading_dates
        .iter()
        .position(|d| d == date)
        .ok_or_else(|| StockrsError::prediction(format!("거래일 인덱스 확인 실패: {}", date)))?;
    if idx == 0 {
        return Ok(Vec::new());
    }
    let start = if idx > max_lookback { idx - max_lookback } else { 0 };
    let start_date = &trading_dates[start];
    let table = stock_code; // already proper format for daily tables
    let mut stmt = daily_db.prepare(&format!(
        "SELECT close FROM \"{}\" WHERE date < ? AND date >= ? ORDER BY date",
        table
    ))?;
    let mut rows = stmt.query(rusqlite::params![date, start_date])?;
    let mut closes: Vec<f64> = Vec::new();
    while let Some(row) = rows.next()? {
        let c: i32 = row.get(0)?;
        closes.push(c as f64);
    }
    Ok(closes)
}

/// Helper: fetch recent daily OHLC data for technical indicators
fn get_recent_daily_ohlc(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
    max_lookback: usize,
) -> StockrsResult<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let idx = trading_dates
        .iter()
        .position(|d| d == date)
        .ok_or_else(|| StockrsError::prediction(format!("거래일 인덱스 확인 실패: {}", date)))?;
    if idx == 0 {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }
    let start = if idx > max_lookback { idx - max_lookback } else { 0 };
    let start_date = &trading_dates[start];
    let table = stock_code;
    
    let mut stmt = daily_db.prepare(&format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? AND date >= ? ORDER BY date",
        table
    ))?;
    let mut rows = stmt.query(rusqlite::params![date, start_date])?;
    
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut closes = Vec::new();
    
    while let Some(row) = rows.next()? {
        let h: i32 = row.get(0)?;
        let l: i32 = row.get(1)?;
        let c: i32 = row.get(2)?;
        highs.push(h as f64);
        lows.push(l as f64);
        closes.push(c as f64);
    }
    
    Ok((highs, lows, closes))
}

/// Helper: calculate RSI
fn calculate_rsi(closes: &[f64], period: usize) -> f64 {
    if closes.len() < period + 1 {
        return 50.0; // 중립값
    }
    
    let mut gains = Vec::new();
    let mut losses = Vec::new();
    
    for i in 1..closes.len() {
        let change = closes[i] - closes[i-1];
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
    
    if avg_loss.abs() < f64::EPSILON {
        return 100.0;
    }
    
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

/// Helper: calculate Williams %R
fn calculate_williams_r(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> f64 {
    if highs.len() < period || lows.len() < period || closes.len() < period {
        return -50.0; // 중립값
    }

    // 최근 period 윈도우로 제한
    let hh = highs[highs.len() - period..]
        .iter()
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let ll = lows[lows.len() - period..]
        .iter()
        .fold(f64::INFINITY, |a, &b| a.min(b));
    let c = *closes.last().unwrap_or(&0.0);

    if (hh - ll).abs() < f64::EPSILON {
        return -50.0;
    }

    -100.0 * (hh - c) / (hh - ll)
}

/// Helper: calculate CCI (Commodity Channel Index)
fn calculate_cci(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> f64 {
    if highs.len() < period || lows.len() < period || closes.len() < period {
        return 0.0; // 중립값
    }
    
    let typical_prices: Vec<f64> = highs.iter()
        .zip(lows.iter())
        .zip(closes.iter())
        .map(|((&h, &l), &c)| (h + l + c) / 3.0)
        .collect();
    
    let mean_tp = typical_prices.iter().sum::<f64>() / typical_prices.len() as f64;
    let mean_deviation = typical_prices.iter()
        .map(|&tp| (tp - mean_tp).abs())
        .sum::<f64>() / typical_prices.len() as f64;
    
    let current_tp = typical_prices.last().unwrap_or(&mean_tp);
    
    if mean_deviation.abs() < f64::EPSILON {
        return 0.0;
    }
    
    (current_tp - mean_tp) / (0.015 * mean_deviation)
}

/// 공통 롤링 유틸: 모멘텀(%) 시계열 [(C_t - C_{t-p})/C_{t-p}*100]
fn rolling_momentum_percent(closes: &[f64], period: usize) -> Vec<f64> {
    if closes.len() < period + 1 {
        return vec![];
    }
    (period..closes.len())
        .map(|i| {
            let cur = closes[i];
            let past = closes[i - period];
            if past.abs() < f64::EPSILON {
                0.0
            } else {
                (cur - past) / past * 100.0
            }
        })
        .collect()
}

/// RSI 시계열 생성 (각 시점마다 길이 period+1 윈도우 사용)
fn rsi_series(closes: &[f64], period: usize) -> Vec<f64> {
    if closes.len() < period + 1 {
        return vec![];
    }
    let mut out = Vec::with_capacity(closes.len() - period);
    for end in period..closes.len() {
        out.push(calculate_rsi(&closes[end - period..=end], period));
    }
    out
}

/// StochRSI 시계열 (RSI 계산 후 해당 RSI에 대해 lookback으로 스토캐스틱)
fn stoch_rsi_series(closes: &[f64], rsi_period: usize, stoch_lookback: usize) -> Vec<f64> {
    let rsi_vals = rsi_series(closes, rsi_period);
    if rsi_vals.len() < stoch_lookback {
        return vec![];
    }
    let mut out = Vec::with_capacity(rsi_vals.len() - stoch_lookback + 1);
    for end in (stoch_lookback - 1)..rsi_vals.len() {
        let window = &rsi_vals[end + 1 - stoch_lookback..=end];
        let min_rsi = window.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_rsi = window.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let cur = rsi_vals[end];
        let v = if (max_rsi - min_rsi).abs() < f64::EPSILON {
            0.5
        } else {
            (cur - min_rsi) / (max_rsi - min_rsi)
        };
        out.push(v.clamp(0.0, 1.0));
    }
    out
}


/// Helper: normalize value to [0, 1] range
fn normalize_to_0_1(value: f64, min_val: f64, max_val: f64) -> f64 {
    if (max_val - min_val).abs() < f64::EPSILON {
        return 0.5;
    }
    ((value - min_val) / (max_val - min_val)).clamp(0.0, 1.0)
}

/// Helper: normalize value to [-1, 1] range
fn normalize_to_minus1_1(value: f64, min_val: f64, max_val: f64) -> f64 {
    if (max_val - min_val).abs() < f64::EPSILON {
        return 0.0;
    }
    let normalized = ((value - min_val) / (max_val - min_val)).clamp(0.0, 1.0);
    2.0 * normalized - 1.0
}

// ===== Day12 특징 함수들 =====

/// RSI (7일) [0, 100] → [0, 1]
pub fn day12_rsi_7(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 30)?;
    
    if closes.len() < 8 {
        return Ok(0.5); // 기본값
    }
    
    let rsi = calculate_rsi(&closes, 7);
    Ok(normalize_to_0_1(rsi, 0.0, 100.0))
}

/// RSI (21일) [0, 100] → [0, 1]
pub fn day12_rsi_21(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 50)?;
    
    if closes.len() < 22 {
        return Ok(0.5); // 기본값
    }
    
    let rsi = calculate_rsi(&closes, 21);
    Ok(normalize_to_0_1(rsi, 0.0, 100.0))
}

/// RSI 다이버전스 플래그: 가격 고점 갱신 시 RSI가 하락 → 다이버전스 여부 [0, 1]
pub fn day12_rsi_divergence_flag(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 60)?;
    if closes.len() < 7 + 7 + 1 {
        return Ok(0.0);
    }

    // 가격: 최근 14일(7+7) 평균 비교
    let recent = &closes[closes.len() - 14..];
    let p1 = recent[..7].iter().copied().sum::<f64>() / 7.0;
    let p2 = recent[7..].iter().copied().sum::<f64>() / 7.0;
    let price_up = p2 > p1;

    // RSI 시계열 생성 후 동일 윈도우 비교
    let rsi_vals = rsi_series(&closes, 7);
    if rsi_vals.len() < 14 {
        return Ok(0.0);
    }
    let rsi_recent = &rsi_vals[rsi_vals.len() - 14..];
    let r1 = rsi_recent[..7].iter().copied().sum::<f64>() / 7.0;
    let r2 = rsi_recent[7..].iter().copied().sum::<f64>() / 7.0;
    let rsi_down = r2 < r1;

    Ok(if price_up && rsi_down { 1.0 } else { 0.0 })
}

/// Stochastic RSI (14일) [0, 1]
pub fn day12_stoch_rsi_14(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 80)?;
    let srs = stoch_rsi_series(&closes, 14, 14);
    if let Some(&last) = srs.last() { Ok(last) } else { Ok(0.5) }
}

/// StochRSI %K vs %D 교차 여부 [0, 1]
pub fn day12_stoch_rsi_signal_cross(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 90)?;
    let k = stoch_rsi_series(&closes, 14, 14);
    if k.len() < 4 { return Ok(0.0); }

    // %D = %K의 3SMA
    let mut d = Vec::with_capacity(k.len()-2);
    for i in 2..k.len() { d.push((k[i-2] + k[i-1] + k[i]) / 3.0); }
    if d.len() < 2 { return Ok(0.0); }

    let k_prev = k[k.len()-2];
    let k_cur  = k[k.len()-1];
    let d_prev = d[d.len()-2];
    let d_cur  = d[d.len()-1];
    Ok(if (k_prev <= d_prev && k_cur > d_cur) || (k_prev >= d_prev && k_cur < d_cur) { 1.0 } else { 0.0 })
}

/// Stoch %K ≥ 80 지속 일수 [0, 1]
pub fn day12_stoch_overbought_persistence(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 120)?;
    let k = stoch_rsi_series(&closes, 14, 14);
    if k.is_empty() { return Ok(0.0); }
    let mut cnt = 0;
    for &v in k.iter().rev() { if v >= 0.8 { cnt += 1; } else { break; } }
    Ok((cnt as f64 / 10.0).clamp(0.0, 1.0))
}

/// Stoch %K ≤ 20 지속 일수 [0, 1]
pub fn day12_stoch_oversold_persistence(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 120)?;
    let k = stoch_rsi_series(&closes, 14, 14);
    if k.is_empty() { return Ok(0.0); }
    let mut cnt = 0;
    for &v in k.iter().rev() { if v <= 0.2 { cnt += 1; } else { break; } }
    Ok((cnt as f64 / 10.0).clamp(0.0, 1.0))
}

/// Williams %R (14일) [-100, 0] → [0, 1]
pub fn day12_williams_r14(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (highs, lows, closes) = get_recent_daily_ohlc(daily_db, stock_code, date, trading_dates, 30)?;
    
    if highs.len() < 14 || lows.len() < 14 || closes.len() < 14 {
        return Ok(0.5); // 기본값
    }
    
    let williams_r = calculate_williams_r(&highs, &lows, &closes, 14);
    // [-100, 0] → [0, 1] 변환
    Ok(normalize_to_0_1(williams_r, -100.0, 0.0))
}

/// CCI (20일) [-200, +200] → [-1, +1]
pub fn day12_cci_20(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (highs, lows, closes) = get_recent_daily_ohlc(daily_db, stock_code, date, trading_dates, 50)?;
    
    if highs.len() < 20 || lows.len() < 20 || closes.len() < 20 {
        return Ok(0.0); // 기본값
    }
    
    let cci = calculate_cci(&highs, &lows, &closes, 20);
    // [-200, +200] → [-1, +1] 변환
    Ok(normalize_to_minus1_1(cci, -200.0, 200.0))
}

/// CCI가 ±100 이상 유지된 일수 비율 [0, 1]
pub fn day12_cci_trend_score(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (highs, lows, closes) = get_recent_daily_ohlc(daily_db, stock_code, date, trading_dates, 50)?;
    
    if highs.len() < 20 || lows.len() < 20 || closes.len() < 20 {
        return Ok(0.0); // 기본값
    }
    
    let cci_values: Vec<f64> = (0..=closes.len()-20)
        .map(|i| calculate_cci(&highs[i..i+20], &lows[i..i+20], &closes[i..i+20], 20))
        .collect();
    
    if cci_values.is_empty() {
        return Ok(0.0);
    }
    
    // ±100 이상인 일수 계산
    let strong_trend_days = cci_values.iter()
        .filter(|&&cci| cci.abs() >= 100.0)
        .count();
    
    Ok(strong_trend_days as f64 / cci_values.len() as f64)
}

/// ROC (10일) [% 변화율 → 정규화]
pub fn day12_roc_10(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 40)?;
    let m = rolling_momentum_percent(&closes, 10);
    if m.is_empty() { return Ok(0.5); }
    let roc = *m.last().unwrap();
    Ok(normalize_to_0_1(roc.clamp(-20.0, 20.0), -20.0, 20.0))
}

/// ROC (20일) [% 변화율 → 정규화]
pub fn day12_roc_20(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 60)?;
    let m = rolling_momentum_percent(&closes, 20);
    if m.is_empty() { return Ok(0.5); }
    let roc = *m.last().unwrap();
    Ok(normalize_to_0_1(roc.clamp(-30.0, 30.0), -30.0, 30.0))
}

/// 단기 vs 중기 ROC 차이 (ROC10 − ROC20)
pub fn day12_roc_momentum_diff(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 80)?;
    let m10 = rolling_momentum_percent(&closes, 10);
    let m20 = rolling_momentum_percent(&closes, 20);
    if m10.is_empty() || m20.is_empty() { return Ok(0.0); }
    let diff = m10.last().unwrap() - m20.last().unwrap();
    Ok(normalize_to_minus1_1(diff.clamp(-15.0, 15.0), -15.0, 15.0))
}

/// (오늘 종가 − 10일 전 종가) / 10일 전 종가 [%]
pub fn day12_momentum_10(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 40)?;
    let m = rolling_momentum_percent(&closes, 10);
    if m.is_empty() { return Ok(0.5); }
    let cur = *m.last().unwrap();
    Ok(normalize_to_0_1(cur.clamp(-25.0, 25.0), -25.0, 25.0))
}

/// Momentum 값을 표준편차로 정규화한 z-score
pub fn day12_momentum_norm(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 120)?;
    let m = rolling_momentum_percent(&closes, 10);
    if m.len() < 50 { return Ok(0.0); }
    let mean = m.iter().sum::<f64>() / m.len() as f64;
    let variance = m.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / m.len() as f64;
    let std = variance.sqrt();
    if std.abs() < f64::EPSILON { return Ok(0.0); }
    let z = (m.last().unwrap() - mean) / std;
    Ok((z / 3.0).clamp(-1.0, 1.0))
}
