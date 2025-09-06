use super::utils::{get_morning_data, is_first_trading_day};
use crate::utility::errors::{StockrsError, StockrsResult};
use super::indicators::ma::{
    alignment_score5, clip, compute_ma_bundle, cross_down, cross_up, safe_div, sma_last,
};
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

fn map_nan(v: f64, default_when_nan: f64) -> f64 { if v.is_finite() { v } else { default_when_nan } }

/// Core builder: returns (bundle, close_today, closes_history)
fn prepare_ma_context(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<(super::indicators::ma::MaBundle, f64, Vec<f64>)> {
    // Morning data for today's close/open
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning.get_last_close().unwrap_or(0.0);

    // Daily closes up to t-1 (need up to 200 + slope window)
    let closes = get_recent_daily_closes(daily_db, stock_code, date, trading_dates, 260)?;
    let bundle = compute_ma_bundle(&closes)
        .unwrap_or_else(|| super::indicators::ma::MaBundle::default());
    Ok((bundle, close_today, closes))
}

// ---------- Value level (로그차로 변경) ----------
pub fn calculate_day10_sma5_value(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { 
    let (bundle, close_today, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    let sma5 = map_nan(bundle.sma5, 0.0);
    if sma5 <= 0.0 || close_today <= 0.0 {
        return Ok(0.0);
    }
    Ok(close_today.ln() - sma5.ln())
}

pub fn calculate_day10_sma20_value(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { 
    let (bundle, close_today, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    let sma20 = map_nan(bundle.sma20, 0.0);
    if sma20 <= 0.0 || close_today <= 0.0 {
        return Ok(0.0);
    }
    Ok(close_today.ln() - sma20.ln())
}

pub fn calculate_day10_sma60_value(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { 
    let (bundle, close_today, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    let sma60 = map_nan(bundle.sma60, 0.0);
    if sma60 <= 0.0 || close_today <= 0.0 {
        return Ok(0.0);
    }
    Ok(close_today.ln() - sma60.ln())
}

pub fn calculate_day10_sma120_value(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { 
    let (bundle, close_today, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    let sma120 = map_nan(bundle.sma120, 0.0);
    if sma120 <= 0.0 || close_today <= 0.0 {
        return Ok(0.0);
    }
    Ok(close_today.ln() - sma120.ln())
}

pub fn calculate_day10_sma200_value(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { 
    let (bundle, close_today, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    let sma200 = map_nan(bundle.sma200, 0.0);
    if sma200 <= 0.0 || close_today <= 0.0 {
        return Ok(0.0);
    }
    Ok(close_today.ln() - sma200.ln())
}

// ---------- Price vs MA ratios ----------
fn price_vs(v: f64, price: f64, lo: f64, hi: f64) -> f64 {
    let r = safe_div(price, v, 1.0);
    let r = clip(r, lo, hi);
    // Map to [0,1]
    if hi > lo { (r - lo) / (hi - lo) } else { 0.5 }
}

pub fn calculate_day10_price_vs_sma5_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { let (b, p, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; Ok(price_vs(b.sma5, p, 0.8, 1.2)) }

pub fn calculate_day10_price_vs_sma20_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { let (b, p, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; Ok(price_vs(b.sma20, p, 0.8, 1.2)) }

pub fn calculate_day10_price_vs_sma60_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { let (b, p, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; Ok(price_vs(b.sma60, p, 0.8, 1.2)) }

pub fn calculate_day10_price_vs_sma120_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { let (b, p, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; Ok(price_vs(b.sma120, p, 0.8, 1.2)) }

pub fn calculate_day10_price_vs_sma200_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { let (b, p, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; Ok(price_vs(b.sma200, p, 0.8, 1.2)) }

pub fn calculate_day10_price_vs_ema12_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { let (b, p, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; Ok(price_vs(b.ema12, p, 0.85, 1.15)) }

pub fn calculate_day10_price_vs_ema26_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { let (b, p, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; Ok(price_vs(b.ema26, p, 0.85, 1.15)) }

// ---------- Slopes ----------
fn slope_clip(v: f64) -> f64 { clip(v, -0.01, 0.01) * 50.0 }

pub fn calculate_day10_sma5_slope5(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { Ok(slope_clip(prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?.0.sma5_slope5)) }

pub fn calculate_day10_sma20_slope5(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { Ok(slope_clip(prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?.0.sma20_slope5)) }

pub fn calculate_day10_sma60_slope5(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> { Ok(slope_clip(prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?.0.sma60_slope5)) }

pub fn calculate_day10_sma_slope_change_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    use super::indicators::ma::linreg_slope;
    // Build last two SMA values of SMA5 over a window of 5 SMA points
    let m = 5usize; let w = 5usize;
    if closes.len() < m + w { return Ok(0.0); }
    // Build SMA series ending at t-1
    let series_t: Vec<f64> = (0..w)
        .map(|i| sma_last(&closes[..closes.len() - (w - 1 - i)], m))
        .collect();
    let series_tm1: Vec<f64> = (0..w)
        .map(|i| sma_last(&closes[..closes.len() - (w - i)], m))
        .collect();
    let s_t = linreg_slope(&series_t);
    let s_tm1 = linreg_slope(&series_tm1);
    let raw = safe_div(s_t - s_tm1, s_tm1.abs().max(1e-6), 0.0);
    Ok(clip(raw, -1.0, 1.0))
}

// ---------- Differences and divisions ----------
pub fn calculate_day10_sma_difference_ratio_5_20(
    db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str, trading_dates: &[String],
) -> StockrsResult<f64> { let (b, _, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; Ok(clip(safe_div(b.sma5 - b.sma20, b.sma20.abs().max(1e-6), 0.0), -0.1, 0.1) * 5.0) }

pub fn calculate_day10_sma_difference_ratio_20_60(
    db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str, trading_dates: &[String],
) -> StockrsResult<f64> { let (b, _, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; Ok(clip(safe_div(b.sma20 - b.sma60, b.sma60.abs().max(1e-6), 0.0), -0.1, 0.1) * 5.0) }

pub fn calculate_day10_sma_difference_ratio_60_120(
    db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str, trading_dates: &[String],
) -> StockrsResult<f64> { let (b, _, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; Ok(clip(safe_div(b.sma60 - b.sma120, b.sma120.abs().max(1e-6), 0.0), -0.1, 0.1) * 5.0) }

pub fn calculate_day10_sma_division_ratio_5_20(
    db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str, trading_dates: &[String],
) -> StockrsResult<f64> { let (b, _, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?; let r = safe_div(b.sma5, b.sma20.abs().max(1e-6), 1.0); Ok((clip(r, 0.9, 1.1) - 0.9) / 0.2) }

// ---------- Cross detection ----------
fn get_prev_curr_sma(closes: &[f64], window: usize) -> Option<(f64, f64)> {
    if closes.len() < window + 1 { return None; }
    let prev = sma_last(&closes[..closes.len() - 1], window);
    let curr = sma_last(&closes, window);
    Some((prev, curr))
}

pub fn calculate_day10_cross_up_5_20(
    db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str, trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    if let (Some((p5, p20)), Some((c5, c20))) = (get_prev_curr_sma(&closes, 5), get_prev_curr_sma(&closes, 20)) {
        let up = cross_up(p5, p20, c5, c20);
        Ok(if up { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

pub fn calculate_day10_cross_down_5_20(
    db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str, trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    if let (Some((p5, p20)), Some((c5, c20))) = (get_prev_curr_sma(&closes, 5), get_prev_curr_sma(&closes, 20)) {
        let down = cross_down(p5, p20, c5, c20);
        Ok(if down { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

pub fn calculate_day10_cross_up_20_60(
    db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str, trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    if let (Some((p20, p60)), Some((c20, c60))) = (get_prev_curr_sma(&closes, 20), get_prev_curr_sma(&closes, 60)) {
        let up = cross_up(p20, p60, c20, c60);
        Ok(if up { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

pub fn calculate_day10_cross_down_20_60(
    db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str, trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    if let (Some((p20, p60)), Some((c20, c60))) = (get_prev_curr_sma(&closes, 20), get_prev_curr_sma(&closes, 60)) {
        let down = cross_down(p20, p60, c20, c60);
        Ok(if down { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

// ---------- Days since last cross ----------
fn build_sma_series(closes: &[f64], window: usize) -> Vec<f64> {
    if closes.len() < window { return Vec::new(); }
    (window..=closes.len()).map(|i| sma_last(&closes[..i], window)).collect()
}

pub fn calculate_day10_days_since_cross_5_20(
    db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str, trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    // 더 엄격한 데이터 검증
    if closes.len() < 50 { 
        return Ok(0.5); // 기본값을 0.5로 변경 (중간값)
    }
    
    let s5 = build_sma_series(&closes, 5);
    let s20 = build_sma_series(&closes, 20);
    
    if s5.is_empty() || s20.is_empty() || s5.len() < 10 || s20.len() < 10 { 
        return Ok(0.5); // 기본값을 0.5로 변경
    }
    
    use super::indicators::ma::last_cross_days;
    let days_opt = last_cross_days(&s5, &s20, None);
    
    match days_opt {
        Some(days) => {
            let normalized_days = (days as f64).min(250.0) / 250.0;
            Ok(normalized_days)
        }
        None => {
            // 교차가 없으면 최대값 반환 (250일 경과)
            Ok(1.0)
        }
    }
}

pub fn calculate_day10_days_since_cross_up_20_60(
    db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str, trading_dates: &[String],
) -> StockrsResult<f64> {
    let (_, _, closes) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    
    // 더 엄격한 데이터 검증
    if closes.len() < 120 { 
        return Ok(0.5); // 기본값을 0.5로 변경 (중간값)
    }
    
    let s20 = build_sma_series(&closes, 20);
    let s60 = build_sma_series(&closes, 60);
    
    if s20.is_empty() || s60.is_empty() || s20.len() < 20 || s60.len() < 20 { 
        return Ok(0.5); // 기본값을 0.5로 변경
    }
    
    use super::indicators::ma::last_cross_days;
    let days_opt = last_cross_days(&s20, &s60, Some(true));
    
    match days_opt {
        Some(days) => {
            let normalized_days = (days as f64).min(250.0) / 250.0;
            Ok(normalized_days)
        }
        None => {
            // 상향 교차가 없으면 최대값 반환 (250일 경과)
            Ok(1.0)
        }
    }
}

// ---------- Alignment score ----------
pub fn calculate_day10_ma_alignment_score(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일 보호
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.5);
    }
    let (b, _, _) = prepare_ma_context(db_5min, daily_db, stock_code, date, trading_dates)?;
    let s = alignment_score5(b.sma5, b.sma20, b.sma60, b.sma120, b.sma200);
    Ok(map_nan(s, 0.5))
}