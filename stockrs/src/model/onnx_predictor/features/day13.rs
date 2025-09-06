use crate::utility::errors::StockrsResult;
use super::indicators::ma::{atr, clip, linreg_slope};
use super::utils::{get_morning_data};
use rusqlite::Connection;

// Helper: compute stdev of ln returns
fn stdev_log_returns(closes: &[f64]) -> f64 {
    if closes.len() < 2 { return 0.0; }
    let mut rets = Vec::with_capacity(closes.len() - 1);
    for i in 1..closes.len() {
        if closes[i-1] <= 0.0 || closes[i] <= 0.0 { continue; }
        let r = (closes[i-1] / closes[i]).ln();
        rets.push(r);
    }
    if rets.len() < 2 { return 0.0; }
    let mean = rets.iter().sum::<f64>() / rets.len() as f64;
    let var = rets.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (rets.len() - 1) as f64;
    var.sqrt()
}

// Helper: simple stdev of values
fn stdev(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    var.sqrt()
}

pub fn day13_atr_10(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // need 11 OHLC rows (t-1 inclusive)
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 11",
        table
    ))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?)))?;
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut closes = Vec::new();
    for r in rows { let (h,l,c) = r?; highs.push(h as f64); lows.push(l as f64); closes.push(c as f64); }
    if highs.len() < 11 { return Ok(0.0); }
    highs.reverse(); lows.reverse(); closes.reverse(); // oldest→newest
    let atr10 = atr(&highs, &lows, &closes, 10);
    if !atr10.is_finite() { return Ok(0.0); }
    let close_today = *closes.last().unwrap_or(&0.0);
    if close_today <= 0.0 { return Ok(0.0); }
    let val = (atr10 / close_today) as f64;
    Ok(clip(val, 0.0, 0.2) / 0.2)
}

pub fn day13_atr_20(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 21",
        table
    ))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?)))?;
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut closes = Vec::new();
    for r in rows { let (h,l,c) = r?; highs.push(h as f64); lows.push(l as f64); closes.push(c as f64); }
    if highs.len() < 21 { return Ok(0.0); }
    highs.reverse(); lows.reverse(); closes.reverse();
    let atr20 = atr(&highs, &lows, &closes, 20);
    if !atr20.is_finite() { return Ok(0.0); }
    let close_today = *closes.last().unwrap_or(&0.0);
    if close_today <= 0.0 { return Ok(0.0); }
    let val = atr20 / close_today;
    Ok(clip(val, 0.0, 0.3) / 0.3)
}

pub fn day13_realized_volatility_10d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 11",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut closes: Vec<f64> = Vec::new();
    for r in rows { closes.push(r? as f64); }
    if closes.len() < 11 { return Ok(0.0); }
    closes.reverse();
    let rv = stdev_log_returns(&closes);
    Ok(clip(rv, 0.0, 0.05) / 0.05)
}

pub fn day13_realized_volatility_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 21",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut closes: Vec<f64> = Vec::new();
    for r in rows { closes.push(r? as f64); }
    if closes.len() < 21 { return Ok(0.0); }
    closes.reverse();
    let rv = stdev_log_returns(&closes);
    Ok(clip(rv, 0.0, 0.08) / 0.08)
}

pub fn day13_intraday_range_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let high_t = morning.get_max_high().unwrap_or(0.0);
    let low_t = morning.get_min_low().unwrap_or(0.0);
    // Use previous day's close (doc: 전일 종가)
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1",
        table
    ))?;
    let mut close_prev: f64 = 0.0;
    {
        let mut rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
        if let Some(r) = rows.next() {
            close_prev = r? as f64;
        }
    }
    if close_prev <= 0.0 { return Ok(0.0); }
    let rng = (high_t - low_t).max(0.0);
    let ratio = rng / close_prev;
    Ok(clip(ratio, 0.0, 0.15) / 0.15)
}

pub fn day13_intraday_range_vs_atr(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let high_t = morning.get_max_high().unwrap_or(0.0);
    let low_t = morning.get_min_low().unwrap_or(0.0);

    // Need ATR14 from last 15 daily bars
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 15",
        table
    ))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?)))?;
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut closes = Vec::new();
    for r in rows { let (h,l,c) = r?; highs.push(h as f64); lows.push(l as f64); closes.push(c as f64); }
    if highs.len() < 15 { return Ok(0.5); }
    highs.reverse(); lows.reverse(); closes.reverse();
    let a = atr(&highs, &lows, &closes, 14);
    if !a.is_finite() || a <= 0.0 { return Ok(0.5); }

    let rng = (high_t - low_t).max(0.0);
    let ratio = rng / a;
    // map [0.5,2.0] → [0,1]
    let norm = if ratio <= 0.5 { 0.0 } else if ratio >= 2.0 { 1.0 } else { (ratio - 0.5) / 1.5 };
    Ok(norm)
}

pub fn day13_atr_slope5(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // Need enough OHLC to build ATR14 series of at least 5 points
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 40",
        table
    ))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?)))?;
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut closes = Vec::new();
    for r in rows { let (h,l,c) = r?; highs.push(h as f64); lows.push(l as f64); closes.push(c as f64); }
    if highs.len() < 20 { return Ok(0.0); }
    highs.reverse(); lows.reverse(); closes.reverse();

    // Build ATR14 series (for each end index)
    let mut atr_series = Vec::new();
    for end in (15)..=highs.len() { // need at least 15 points (14 TRs + prev close)
        let a = atr(&highs[..end], &lows[..end], &closes[..end], 14);
        if a.is_finite() { atr_series.push(a); }
    }
    if atr_series.len() < 5 { return Ok(0.0); }

    // Take last 5 values and compute slope normalized by last value
    let last5 = &atr_series[atr_series.len()-5..];
    let slope = linreg_slope(last5);
    let last = *last5.last().unwrap_or(&1.0);
    if !slope.is_finite() || last.abs() < 1e-12 { return Ok(0.0); }
    let rel = slope / last;
    Ok(clip(rel, -0.01, 0.01) * 50.0)
}

pub fn day13_bollinger_band_width_20(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 20",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut closes: Vec<f64> = Vec::new();
    for r in rows { closes.push(r? as f64); }
    if closes.len() < 20 { return Ok(0.0); }
    closes.reverse();
    let sma = closes.iter().sum::<f64>() / 20.0;
    if sma.abs() < 1e-12 { return Ok(0.0); }
    // standard deviation of closes over 20
    let sd = stdev(&closes);
    let upper = sma + 2.0 * sd;
    let lower = sma - 2.0 * sd;
    let width = (upper - lower) / sma.max(1e-12);
    Ok(clip(width, 0.0, 0.3) / 0.3)
}

pub fn day13_bollinger_band_squeeze_flag(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // Need 140 closes to compute 120 widths with 20 window
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 140",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut closes: Vec<f64> = Vec::new();
    for r in rows { closes.push(r? as f64); }
    if closes.len() < 40 { return Ok(0.0); }
    closes.reverse();

    // Build rolling BB width series for window 20
    let mut widths = Vec::new();
    for i in 20..=closes.len() {
        let window = &closes[i-20..i];
        let sma = window.iter().sum::<f64>() / 20.0;
        if !sma.is_finite() || sma.abs() < 1e-12 { continue; }
        let sd = stdev(window);
        let width = (2.0 * 2.0 * sd) / sma; // (upper-lower)/sma = 4*sd/sma
        widths.push(width);
    }
    if widths.len() < 10 { return Ok(0.0); }
    let current = *widths.last().unwrap_or(&0.0);
    let mut sorted = widths.clone();
    sorted.sort_by(|a,b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = sorted.iter().position(|&x| (x - current).abs() < 1e-12).unwrap_or(0);
    let pct = rank as f64 / sorted.len() as f64;
    Ok(if pct <= 0.05 { 1.0 } else { 0.0 })
}

pub fn day13_volatility_change_5d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // Build ATR14 series and compare last vs 5 days earlier
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 40",
        table
    ))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?)))?;
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut closes = Vec::new();
    for r in rows { let (h,l,c) = r?; highs.push(h as f64); lows.push(l as f64); closes.push(c as f64); }
    if highs.len() < 25 { return Ok(0.0); }
    highs.reverse(); lows.reverse(); closes.reverse();
    let mut atrs = Vec::new();
    for end in (15)..=highs.len() { let a = atr(&highs[..end], &lows[..end], &closes[..end], 14); if a.is_finite() { atrs.push(a); } }
    if atrs.len() < 6 { return Ok(0.0); }
    let cur = *atrs.last().unwrap();
    let prev = atrs[atrs.len()-6];
    let denom = prev.abs().max(1e-9);
    let delta = (cur - prev) / denom;
    Ok(clip(delta, -1.0, 1.0))
}

pub fn day13_volatility_change_10d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 40",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut closes: Vec<f64> = Vec::new();
    for r in rows { closes.push(r? as f64); }
    if closes.len() < 30 { return Ok(0.0); }
    closes.reverse();
    // Build stdev10 series of ln returns
    let mut stdev10_series = Vec::new();
    for end in 11..=closes.len() {
        let window = &closes[end-11..end];
        let v = stdev_log_returns(window);
        stdev10_series.push(v);
    }
    if stdev10_series.len() < 11 { return Ok(0.0); }
    let cur = *stdev10_series.last().unwrap();
    let prev = stdev10_series[stdev10_series.len()-11];
    let denom = prev.abs().max(1e-9);
    let delta = (cur - prev) / denom;
    Ok(clip(delta, -1.0, 1.0))
}

pub fn day13_volatility_ratio_10_60(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 61",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut closes: Vec<f64> = Vec::new();
    for r in rows { closes.push(r? as f64); }
    if closes.len() < 61 { return Ok(0.5); }
    closes.reverse();
    let stdev10 = stdev_log_returns(&closes[closes.len()-11..]);
    let stdev60 = stdev_log_returns(&closes);
    if stdev60 <= 0.0 { return Ok(0.5); }
    let ratio = stdev10 / stdev60;
    let norm = if ratio <= 0.5 { 0.0 } else if ratio >= 2.0 { 1.0 } else { (ratio - 0.5) / 1.5 };
    Ok(norm)
}

pub fn day13_volatility_ratio_20_120(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 121",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut closes: Vec<f64> = Vec::new();
    for r in rows { closes.push(r? as f64); }
    if closes.len() < 121 { return Ok(0.5); }
    closes.reverse();
    let stdev20 = stdev_log_returns(&closes[closes.len()-21..]);
    let stdev120 = stdev_log_returns(&closes);
    if stdev120 <= 0.0 { return Ok(0.5); }
    let ratio = stdev20 / stdev120;
    let norm = if ratio <= 0.5 { 0.0 } else if ratio >= 2.0 { 1.0 } else { (ratio - 0.5) / 1.5 };
    Ok(norm)
}

pub fn day13_volatility_spike_flag(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // Use (high-low)/prev_close as intraday volatility proxy over last 5 days
    let table = stock_code;
    // Need 6 rows to compute 5 ratios
    let mut stmt = daily_db.prepare(&format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 6",
        table
    ))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?)))?;
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut closes = Vec::new();
    for r in rows { let (h,l,c) = r?; highs.push(h as f64); lows.push(l as f64); closes.push(c as f64); }
    if highs.len() < 6 { return Ok(0.0); }
    highs.reverse(); lows.reverse(); closes.reverse();
    let mut ratios = Vec::new();
    for i in 1..highs.len() { // day i uses prev close at i-1
        let prev_close = closes[i-1];
        if prev_close <= 0.0 { ratios.push(0.0); continue; }
        let r = (highs[i] - lows[i]).max(0.0) / prev_close;
        ratios.push(r);
    }
    if ratios.len() < 5 { return Ok(0.0); }
    // Use last day's ratio vs previous 4 days baseline (from 5 total ratios)
    let r_last = *ratios.last().unwrap_or(&0.0);
    let baseline = &ratios[..ratios.len()-1];
    if baseline.len() < 2 { return Ok(0.0); }
    let mean = baseline.iter().sum::<f64>() / baseline.len() as f64;
    let sd = stdev(baseline);
    let threshold = mean + 1.5 * sd; // slightly less strict to capture genuine spikes
    Ok(if r_last > threshold { 1.0 } else { 0.0 })
}


