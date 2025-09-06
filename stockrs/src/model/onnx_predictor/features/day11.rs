use super::utils::get_morning_data;
use crate::utility::errors::{StockrsError, StockrsResult};
use super::indicators::ma::{
    clip, compute_ma_bundle, MaBundle, atr, keltner_channel, slope_last_n,
    tema_series, hma_series, kama_series, slope_on_sma_last_n, ema_last,
};
use rusqlite::Connection;
use std::f64::consts::PI;

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

/// Helper: fetch recent daily OHLC data for ATR calculation
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

/// Helper: calculate standard deviation
fn calculate_stdev(prices: &[f64]) -> f64 {
    if prices.len() < 2 {
        return 0.0;
    }
    
    let mean = prices.iter().sum::<f64>() / prices.len() as f64;
    let variance = prices.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>() / (prices.len() - 1) as f64;
    
    variance.sqrt()
}

/// Helper: calculate percentile from historical values
fn calculate_percentile(current: f64, historical: &[f64]) -> f64 {
    if historical.is_empty() {
        return 0.5;
    }
    
    let count_less = historical.iter().filter(|&&x| x < current).count();
    count_less as f64 / historical.len() as f64
}

/// Core builder: returns (bundle, close_today, closes_history)
fn prepare_ma_context(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<(MaBundle, f64, Vec<f64>)> {
    // Morning data for today's close/open
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning.get_last_close().unwrap_or(0.0);

    // Daily closes up to t-1 (need up to 200 + slope window)
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 260)?;
    let bundle = compute_ma_bundle(&closes)
        .unwrap_or_else(|| MaBundle::default());
    Ok((bundle, close_today, closes))
}

// ---------- 곡률·각도 (Curvature & Angle) ----------

/// SMA20 curvature over 5-day period
pub fn calculate_day11_sma20_curvature5(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if closes.len() < (20 + 5 + 5 - 1) { // 20-day SMA + 5-slope + 이전 5일
        return Ok(0.0);
    }
    
    // 최근 5일 상대기울기
    let cur = slope_on_sma_last_n(&closes, 20, 5);
    
    // 직전 5일 상대기울기: 끝을 5일만큼 한 칸 앞에서 끊는다
    let prev = slope_on_sma_last_n(&closes[..closes.len()-5], 20, 5);
    
    let curvature = cur - prev;
    Ok(clip(curvature, -0.05, 0.05) * 20.0)
}

/// SMA60 curvature over 5-day period
pub fn calculate_day11_sma60_curvature5(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if closes.len() < (60 + 5 + 5 - 1) { // 60-day SMA + 5-slope + 이전 5일
        return Ok(0.0);
    }
    
    let cur  = slope_on_sma_last_n(&closes,                 60, 5);
    let prev = slope_on_sma_last_n(&closes[..closes.len()-5], 60, 5);
    
    let curvature = cur - prev;
    Ok(clip(curvature, -0.05, 0.05) * 20.0)
}

/// SMA20 slope angle normalized
pub fn calculate_day11_sma20_angle(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    // need >= ma_window + slope_window - 1 = 20 + 5 - 1 = 24
    if closes.len() < 24 {
        return Ok(0.0);
    }
    
    let slope_rel = slope_on_sma_last_n(&closes, 20, 5); // 상대기울기(= slope / last_SMA)
    let angle = slope_rel.atan() * 180.0 / PI / 90.0;
    Ok(clip(angle, -1.0, 1.0))
}

// ---------- Z-Score 괴리 (Z-Score Divergence) ----------

/// Price Z-score vs SMA20
pub fn calculate_day11_price_zscore_vs_sma20(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (bundle, close_today, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if closes.len() < 20 || bundle.sma20.is_nan() {
        return Ok(0.0);
    }
    
    let sma20 = bundle.sma20;
    // Use recent 20 days for standard deviation calculation
    let stdev = calculate_stdev(&closes[closes.len()-20..]);
    
    if stdev < 1e-10 {
        return Ok(0.0);
    }
    
    let zscore = (close_today - sma20) / stdev;
    let normalized = clip(zscore, -3.0, 3.0) / 3.0;
    
    Ok(normalized)
}

/// Price Z-score vs SMA60
pub fn calculate_day11_price_zscore_vs_sma60(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (bundle, close_today, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if closes.len() < 60 || bundle.sma60.is_nan() {
        return Ok(0.0);
    }
    
    let sma60 = bundle.sma60;
    // Use recent 60 days for standard deviation calculation
    let stdev = calculate_stdev(&closes[closes.len()-60..]);
    
    if stdev < 1e-10 {
        return Ok(0.0);
    }
    
    let zscore = (close_today - sma60) / stdev;
    let normalized = clip(zscore, -3.0, 3.0) / 3.0;
    
    Ok(normalized)
}

// ---------- 리본 압축 (Ribbon Compression) ----------

/// MA ribbon dispersion (log scale)
pub fn calculate_day11_ma_ribbon_dispersion_5_20_60_120(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (bundle, _, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if bundle.sma5.is_nan() || bundle.sma20.is_nan() || bundle.sma60.is_nan() || bundle.sma120.is_nan() {
        return Ok(0.0);
    }
    
    let log_values = vec![
        bundle.sma5.ln(),
        bundle.sma20.ln(),
        bundle.sma60.ln(),
        bundle.sma120.ln(),
    ];
    
    let stdev = calculate_stdev(&log_values);
    let normalized = clip(stdev, 0.0, 0.1) * 10.0;
    
    Ok(normalized)
}

/// MA ribbon tightness percentile
pub fn calculate_day11_ma_ribbon_tightness_lookback60(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (bundle, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if bundle.sma5.is_nan() || bundle.sma20.is_nan() || bundle.sma60.is_nan() || bundle.sma120.is_nan() {
        return Ok(0.5);
    }
    
    // Calculate current dispersion
    let current_dispersion = {
        let log_values = vec![
            bundle.sma5.ln(),
            bundle.sma20.ln(),
            bundle.sma60.ln(),
            bundle.sma120.ln(),
        ];
        calculate_stdev(&log_values)
    };
    
    // Need at least 60 days of historical data for percentile calculation
    if closes.len() < 60 {
        return Ok(0.5);
    }
    
    // Calculate historical dispersions for the past 60 days
    let mut historical_dispersions = Vec::new();
    
    for i in 0..60 {
        if i + 120 >= closes.len() { // Need at least 120 days for SMA120
            break;
        }
        
        // Calculate MA bundle for historical date
        let historical_closes = &closes[i..i+120];
        if historical_closes.len() < 120 {
            break;
        }
        
        // Simple approximation: use the closes directly for historical MAs
        // In a full implementation, you'd want to recalculate the full MA bundle
        let historical_sma5   = historical_closes[historical_closes.len()-5..].iter().copied().sum::<f64>() / 5.0;
        let historical_sma20  = historical_closes[historical_closes.len()-20..].iter().copied().sum::<f64>() / 20.0;
        let historical_sma60  = historical_closes[historical_closes.len()-60..].iter().copied().sum::<f64>() / 60.0;
        let historical_sma120 = historical_closes[historical_closes.len()-120..].iter().copied().sum::<f64>() / 120.0;
        
        let historical_log_values = vec![
            historical_sma5.ln(),
            historical_sma20.ln(),
            historical_sma60.ln(),
            historical_sma120.ln(),
        ];
        
        let historical_dispersion = calculate_stdev(&historical_log_values);
        historical_dispersions.push(historical_dispersion);
    }
    
    if historical_dispersions.is_empty() {
        return Ok(0.5);
    }
    
    // Calculate percentile of current dispersion among historical values
    let percentile = calculate_percentile(current_dispersion, &historical_dispersions);
    
    Ok(percentile)
}

// ---------- Pullback/이격도 (Pullback & Divergence) ----------

/// Pullback depth vs ATR20
pub fn calculate_day11_pullback_depth_atr20(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (bundle, close_today, _closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if bundle.sma20.is_nan() {
        return Ok(0.0);
    }
    
    // Get OHLC data for ATR calculation
    let (highs, lows, ohlc_closes) = get_recent_daily_ohlc(daily_db, stock_code, date, trading_dates, 40)?;
    
    if highs.len() < 20 || lows.len() < 20 || ohlc_closes.len() < 21 {
        return Ok(0.0);
    }
    
    let atr20_val = atr(&highs, &lows, &ohlc_closes, 20);
    if atr20_val.is_nan() {
        return Ok(0.0);
    }
    
    let pullback = (bundle.sma20 - close_today) / atr20_val;
    let normalized = clip(pullback, -2.0, 2.0) / 2.0;
    
    Ok(normalized)
}

/// Pullback depth vs ATR60
pub fn calculate_day11_pullback_depth_atr60(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (bundle, close_today, _closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if bundle.sma60.is_nan() {
        return Ok(0.0);
    }
    
    // Get OHLC data for ATR calculation
    let (highs, lows, ohlc_closes) = get_recent_daily_ohlc(daily_db, stock_code, date, trading_dates, 80)?;
    
    if highs.len() < 60 || lows.len() < 60 || ohlc_closes.len() < 61 {
        return Ok(0.0);
    }
    
    let atr60_val = atr(&highs, &lows, &ohlc_closes, 60);
    if atr60_val.is_nan() {
        return Ok(0.0);
    }
    
    let pullback = (bundle.sma60 - close_today) / atr60_val;
    let normalized = clip(pullback, -2.0, 2.0) / 2.0;
    
    Ok(normalized)
}

/// Distance percentile vs SMA20 (120d)
pub fn calculate_day11_distance_percentile_vs_sma20_120d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (bundle, close_today, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if bundle.sma20.is_nan() {
        return Ok(0.5);
    }
    
    let current_distance = (close_today - bundle.sma20).abs() / bundle.sma20;
    
    // Need at least 120 days of historical data
    if closes.len() < 120 {
        return Ok(0.5);
    }
    
    // Calculate historical distances for the past 120 days
    let mut historical_distances = Vec::new();
    
    for i in 0..120 {
        if i + 20 >= closes.len() {
            break;
        }
        
        let historical_closes = &closes[i..i+20];
        if historical_closes.len() < 20 {
            break;
        }
        
        let historical_sma20 = historical_closes.iter().sum::<f64>() / 20.0;
        let historical_close = historical_closes.last().unwrap_or(&1.0);
        let historical_distance = (historical_close - historical_sma20).abs() / historical_sma20;
        
        historical_distances.push(historical_distance);
    }
    
    if historical_distances.is_empty() {
        return Ok(0.5);
    }
    
    let percentile = calculate_percentile(current_distance, &historical_distances);
    Ok(percentile)
}

// ---------- 채널 지표 (Channel Indicators) ----------

/// Keltner Channel position
pub fn calculate_day11_keltner_position_20(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, close_today, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    let (highs, lows, ohlc_closes) = get_recent_daily_ohlc(daily_db, stock_code, date, trading_dates, 40)?;
    if highs.len() < 20 || lows.len() < 20 || ohlc_closes.len() < 21 { return Ok(0.5); }

    let ema20 = ema_last(&closes, 20);
    let atr20_val = atr(&highs, &lows, &ohlc_closes, 20);
    if !ema20.is_finite() || !atr20_val.is_finite() { return Ok(0.5); }

    let (upper, lower) = keltner_channel(ema20, atr20_val, 2.0);
    let width = upper - lower;
    if !width.is_finite() || width.abs() < 1e-12 { return Ok(0.5); } // 안전장치
    
    let pos = (close_today - lower) / width;
    Ok(clip(pos, 0.0, 1.0))
}

/// Keltner Channel width
pub fn calculate_day11_keltner_width_20(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    let (highs, lows, ohlc_closes) = get_recent_daily_ohlc(daily_db, stock_code, date, trading_dates, 40)?;
    if highs.len() < 20 || lows.len() < 20 || ohlc_closes.len() < 21 { return Ok(0.0); }

    let ema20 = ema_last(&closes, 20);
    let atr20_val = atr(&highs, &lows, &ohlc_closes, 20);
    if !ema20.is_finite() || !atr20_val.is_finite() { return Ok(0.0); }

    let (upper, lower) = keltner_channel(ema20, atr20_val, 2.0);
    let width = (upper - lower) / ema20;
    Ok(clip(width, 0.0, 0.2) * 5.0)
}

// ---------- 어댑티브 MA 계열 (Adaptive MA Series) ----------

/// TEMA20 slope normalized by value
pub fn calculate_day11_tema20_slope5(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if closes.len() < 60 {
        return Ok(0.0);
    }
    
    // TEMA 시계열 생성
    let tema_series = tema_series(&closes, 20);
    if tema_series.len() < 5 {
        return Ok(0.0);
    }
    
    // TEMA 시계열의 최근 5일 slope 계산
    let slope = slope_last_n(&tema_series, 5);
    if slope.is_nan() {
        return Ok(0.0);
    }
    
    let tema20_val = *tema_series.last().unwrap_or(&1.0);
    if tema20_val.abs() < 1e-10 {
        return Ok(0.0);
    }
    
    let normalized_slope = slope / tema20_val;
    let normalized = clip(normalized_slope, -0.05, 0.05) * 20.0;
    
    Ok(normalized)
}

/// KAMA20 slope
pub fn calculate_day11_kama20_slope5(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    if closes.len() < 30 {
        return Ok(0.0);
    }
    
    // KAMA 시계열 생성 (기본값: er_period=20, fast=2, slow=30)
    let kama_series = kama_series(&closes, 20, 2, 30);
    if kama_series.len() < 5 {
        return Ok(0.0);
    }
    
    // KAMA 시계열의 최근 5일 slope 계산
    let slope = slope_last_n(&kama_series, 5);
    if slope.is_nan() {
        return Ok(0.0);
    }
    
    let kama20_val = *kama_series.last().unwrap_or(&1.0);
    if kama20_val.abs() < 1e-10 {
        return Ok(0.0);
    }
    
    let normalized_slope = slope / kama20_val;
    let normalized = clip(normalized_slope, -0.05, 0.05) * 20.0;
    
    Ok(normalized)
}

/// HMA20 slope using the full HMA series
/// HMA(n) = WMA(2 * WMA(n/2) - WMA(n), sqrt(n))
pub fn calculate_day11_hma20_slope5(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    // Need at least 20 days for HMA20 calculation
    if closes.len() < 20 {
        return Ok(0.0);
    }
    
    // Generate full HMA series following the formula
    let hma_series = hma_series(&closes, 20);
    if hma_series.len() < 5 {
        return Ok(0.0);
    }
    
    // Calculate slope over the last 5 HMA values
    let slope = slope_last_n(&hma_series, 5);
    if slope.is_nan() {
        return Ok(0.0);
    }
    
    let hma20_val = *hma_series.last().unwrap_or(&1.0);
    if hma20_val.abs() < 1e-10 {
        return Ok(0.0);
    }
    
    // Normalize slope by HMA value for scale-invariant measure
    let normalized_slope = slope / hma20_val;
    let normalized = clip(normalized_slope, -0.05, 0.05) * 20.0;
    
    Ok(normalized)
}
