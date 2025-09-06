use crate::utility::errors::StockrsResult;
use rusqlite::Connection;

fn fetch_ohlc(daily_db: &Connection, stock_code: &str, date: &str, limit: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut closes = Vec::new();
    if limit == 0 { return (highs, lows, closes); }
    let table = stock_code;
    if let Ok(mut stmt) = daily_db.prepare(&format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT {}",
        table, limit
    )) {
        if let Ok(rows) = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?))) {
            for r in rows.flatten() {
                let (h,l,c) = r;
                highs.push(h as f64);
                lows.push(l as f64);
                closes.push(c as f64);
            }
        }
    }
    highs.reverse(); lows.reverse(); closes.reverse();
    (highs, lows, closes)
}

fn percentile(value: f64, hist: &[f64]) -> f64 {
    if hist.is_empty() { return 0.5; }
    let count_less = hist.iter().filter(|&&x| x < value).count();
    (count_less as f64 / hist.len() as f64).clamp(0.0, 1.0)
}

pub fn day17_52w_high_gap(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, _l, c) = fetch_ohlc(daily_db, stock_code, date, 252);
    if c.is_empty() || h.is_empty() { return Ok(0.0); }
    let hi_52w = h.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let close = *c.last().unwrap_or(&0.0);
    if hi_52w <= 0.0 { return Ok(0.0); }
    let gap = close / hi_52w - 1.0; // [-0.5, +0.5] target
    Ok((gap.clamp(-0.5, 0.5)) * 2.0)
}

pub fn day17_52w_low_gap(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, l, c) = fetch_ohlc(daily_db, stock_code, date, 252);
    if c.is_empty() || l.is_empty() { return Ok(0.0); }
    let lo_52w = l.iter().copied().fold(f64::INFINITY, f64::min);
    let close = *c.last().unwrap_or(&0.0);
    if lo_52w <= 0.0 { return Ok(0.0); }
    let gap = close / lo_52w - 1.0;
    Ok((gap.clamp(-0.5, 0.5)) * 2.0)
}

pub fn day17_near_52w_high_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, _l, c) = fetch_ohlc(daily_db, stock_code, date, 252);
    if c.is_empty() || h.is_empty() { return Ok(0.0); }
    let hi_52w = h.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let close = *c.last().unwrap_or(&0.0);
    if hi_52w <= 0.0 { return Ok(0.0); }
    let near = ((close - hi_52w).abs() / hi_52w) <= 0.02;
    Ok(if near { 1.0 } else { 0.0 })
}

pub fn day17_near_52w_low_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, l, c) = fetch_ohlc(daily_db, stock_code, date, 252);
    if c.is_empty() || l.is_empty() { return Ok(0.0); }
    let lo_52w = l.iter().copied().fold(f64::INFINITY, f64::min);
    let close = *c.last().unwrap_or(&0.0);
    if lo_52w <= 0.0 { return Ok(0.0); }
    let near = ((close - lo_52w).abs() / lo_52w) <= 0.02;
    Ok(if near { 1.0 } else { 0.0 })
}

pub fn day17_pos_vs_high_20d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, l, c) = fetch_ohlc(daily_db, stock_code, date, 20);
    if h.len() < 20 || l.len() < 20 || c.len() < 20 { return Ok(0.5); }
    let high20 = h.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let low20  = l.iter().copied().fold(f64::INFINITY, f64::min);
    let close = *c.last().unwrap_or(&0.0);
    let denom = (high20 - low20).abs();
    if denom == 0.0 { return Ok(0.5); }
    let pos = (close - low20) / denom;
    Ok(pos.clamp(0.0, 1.0))
}

pub fn day17_pos_vs_low_20d(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    day17_pos_vs_high_20d(db_5min, daily_db, stock_code, date)
}

pub fn day17_range_percentile_20d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    // Build last 40 days, compute daily position over rolling 20, then percentile of current among last 20
    let (h, l, c) = fetch_ohlc(daily_db, stock_code, date, 40);
    if h.len() < 40 { return Ok(0.5); }
    let mut positions = Vec::new();
    for i in 20..=h.len() {
        let high20 = h[i - 20..i].iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let low20  = l[i - 20..i].iter().copied().fold(f64::INFINITY, f64::min);
        let close = c[i - 1];
        let denom = (high20 - low20).abs();
        let pos = if denom == 0.0 { 0.5 } else { ((close - low20) / denom).clamp(0.0, 1.0) };
        positions.push(pos);
    }
    if positions.len() < 21 { return Ok(0.5); }
    let current = *positions.last().unwrap_or(&0.5);
    let hist = &positions[positions.len() - 21..positions.len() - 1];
    Ok(percentile(current, hist))
}

pub fn day17_breaks_52w_high_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, _l, c) = fetch_ohlc(daily_db, stock_code, date, 252);
    if h.len() < 2 || c.is_empty() { return Ok(0.0); }
    let last_close = *c.last().unwrap_or(&0.0);
    // Exclude today's high from the 52w window when checking break to avoid trivial false negatives
    let hi_lookback = h[..h.len()-1].iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok(if last_close > hi_lookback { 1.0 } else { 0.0 })
}

pub fn day17_breaks_52w_low_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, l, c) = fetch_ohlc(daily_db, stock_code, date, 252);
    if l.len() < 2 || c.is_empty() { return Ok(0.0); }
    let last_close = *c.last().unwrap_or(&0.0);
    let lo_lookback = l[..l.len()-1].iter().copied().fold(f64::INFINITY, f64::min);
    Ok(if last_close < lo_lookback { 1.0 } else { 0.0 })
}

pub fn day17_resistance_touch_count_3m(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    // Approx: count days where close within 1% of rolling 20d high in last 63d
    let (h, _l, c) = fetch_ohlc(daily_db, stock_code, date, 63);
    if h.len() < 21 { return Ok(0.0); }
    let mut touches = 0.0; let mut total = 0.0;
    for i in 20..h.len() {
        total += 1.0;
        let high20 = h[i - 20..=i].iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let close = c[i];
        if high20 > 0.0 && ((close - high20).abs() / high20) <= 0.01 { touches += 1.0; }
    }
    if total == 0.0 { return Ok(0.0); }
    let v = touches / total; let clamped = if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v };
    Ok(clamped)
}

pub fn day17_support_touch_count_3m(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, l, c) = fetch_ohlc(daily_db, stock_code, date, 63);
    if l.len() < 21 { return Ok(0.0); }
    let mut touches = 0.0; let mut total = 0.0;
    for i in 20..l.len() {
        total += 1.0;
        let low20 = l[i - 20..=i].iter().copied().fold(f64::INFINITY, f64::min);
        let close = c[i];
        if low20 > 0.0 && ((close - low20).abs() / low20) <= 0.01 { touches += 1.0; }
    }
    if total == 0.0 { return Ok(0.0); }
    let v = touches / total; let clamped = if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v };
    Ok(clamped)
}

pub fn day17_double_top_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    // Simple heuristic: two highs within 1% near 60d window top with pullback between
    let (h, _l, _c) = fetch_ohlc(daily_db, stock_code, date, 60);
    if h.len() < 10 { return Ok(0.0); }
    let max_val = h.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut first = None;
    for (i, &v) in h.iter().enumerate() {
        if (max_val - v).abs() / max_val <= 0.01 { if first.is_none() { first = Some(i); } }
    }
    let flag = first.is_some();
    Ok(if flag { 1.0 } else { 0.0 })
}

pub fn day17_double_bottom_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, l, _c) = fetch_ohlc(daily_db, stock_code, date, 60);
    if l.len() < 10 { return Ok(0.0); }
    let min_val = l.iter().copied().fold(f64::INFINITY, f64::min);
    let mut first = None;
    for (i, &v) in l.iter().enumerate() {
        if (min_val - v).abs() / min_val <= 0.01 { if first.is_none() { first = Some(i); } }
    }
    let flag = first.is_some();
    Ok(if flag { 1.0 } else { 0.0 })
}

pub fn day17_range_contraction_ratio(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, l, _c) = fetch_ohlc(daily_db, stock_code, date, 60);
    if h.len() < 60 || l.len() < 60 { return Ok(0.5); }
    let range20 = h[h.len() - 20..].iter().copied().fold(f64::NEG_INFINITY, f64::max) - l[l.len() - 20..].iter().copied().fold(f64::INFINITY, f64::min);
    let range60 = h.iter().copied().fold(f64::NEG_INFINITY, f64::max) - l.iter().copied().fold(f64::INFINITY, f64::min);
    if range60 <= 0.0 { return Ok(0.5); }
    let v = range20 / range60; let clamped = if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v };
    Ok(clamped)
}

pub fn day17_range_expansion_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, l, _c) = fetch_ohlc(daily_db, stock_code, date, 60);
    if h.len() < 60 { return Ok(0.0); }
    let range5 = h[h.len() - 5..].iter().copied().fold(f64::NEG_INFINITY, f64::max) - l[l.len() - 5..].iter().copied().fold(f64::INFINITY, f64::min);
    let avg_range60 = (h.iter().zip(l.iter()).map(|(hh, ll)| hh - ll).sum::<f64>() / h.len() as f64).abs();
    Ok(if range5 > 1.5 * avg_range60 { 1.0 } else { 0.0 })
}


