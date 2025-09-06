use crate::utility::errors::StockrsResult;
use super::indicators::ma::linreg_slope;
use rusqlite::Connection;

// ---------- Utilities ----------

fn fetch_ohlc(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    limit: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
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

#[allow(dead_code)]
fn stdev(vals: &[f64]) -> f64 {
    if vals.len() < 2 { return 0.0; }
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    let var = vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
    var.sqrt()
}

// +DI/-DI/ADX helpers (Wilder simplified)
fn compute_dm_di(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = highs.len();
    if n < period + 1 { return (vec![], vec![], vec![]); }
    let mut tr_vec = Vec::with_capacity(n - 1);
    let mut plus_dm_vec = Vec::with_capacity(n - 1);
    let mut minus_dm_vec = Vec::with_capacity(n - 1);
    for i in 1..n {
        let high_diff = highs[i] - highs[i - 1];
        let low_diff = lows[i - 1] - lows[i];
        let plus_dm = if high_diff > low_diff && high_diff > 0.0 { high_diff } else { 0.0 };
        let minus_dm = if low_diff > high_diff && low_diff > 0.0 { low_diff } else { 0.0 };
        let tr = (highs[i] - lows[i])
            .max((highs[i] - closes[i - 1]).abs())
            .max((lows[i] - closes[i - 1]).abs());
        plus_dm_vec.push(plus_dm);
        minus_dm_vec.push(minus_dm);
        tr_vec.push(tr);
    }
    // Smooth with simple sums over rolling window
    let mut plus_di = Vec::new();
    let mut minus_di = Vec::new();
    let mut dx = Vec::new();
    for i in period - 1..tr_vec.len() {
        let tr_sum: f64 = tr_vec[i + 1 - period..=i].iter().sum();
        let p_sum: f64 = plus_dm_vec[i + 1 - period..=i].iter().sum();
        let m_sum: f64 = minus_dm_vec[i + 1 - period..=i].iter().sum();
        if tr_sum <= 0.0 { continue; }
        let pdi = 100.0 * p_sum / tr_sum;
        let mdi = 100.0 * m_sum / tr_sum;
        let denom = (pdi + mdi).abs();
        let dx_val = if denom == 0.0 { 0.0 } else { 100.0 * (pdi - mdi).abs() / denom };
        plus_di.push(pdi);
        minus_di.push(mdi);
        dx.push(dx_val);
    }
    (plus_di, minus_di, dx)
}

fn adx_from_dx(dx: &[f64], period: usize) -> Vec<f64> {
    if dx.len() < period { return vec![]; }
    let mut out = Vec::new();
    for i in period - 1..dx.len() {
        let avg = dx[i + 1 - period..=i].iter().sum::<f64>() / period as f64;
        out.push(avg);
    }
    out
}

// ---------- Day16 Features ----------

pub fn day16_adx_7(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, l, c) = fetch_ohlc(daily_db, stock_code, date, 20);
    if h.len() < 8 { return Ok(0.5); }
    let (_p, _m, dx) = compute_dm_di(&h, &l, &c, 7);
    if dx.is_empty() { return Ok(0.5); }
    let adx_vals = adx_from_dx(&dx, 7);
    let last = *adx_vals.last().unwrap_or(&25.0);
    Ok((last.clamp(0.0, 100.0)) / 100.0)
}

pub fn day16_adx_21(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, l, c) = fetch_ohlc(daily_db, stock_code, date, 60);
    if h.len() < 22 { return Ok(0.5); }
    let (_p, _m, dx) = compute_dm_di(&h, &l, &c, 21);
    if dx.is_empty() { return Ok(0.5); }
    let adx_vals = adx_from_dx(&dx, 21);
    let last = *adx_vals.last().unwrap_or(&25.0);
    Ok((last.clamp(0.0, 100.0)) / 100.0)
}

pub fn day16_adx_trend_change(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, l, c) = fetch_ohlc(daily_db, stock_code, date, 60);
    if h.len() < 30 { return Ok(0.0); }
    let (_p, _m, dx) = compute_dm_di(&h, &l, &c, 14);
    if dx.len() < 20 { return Ok(0.0); }
    let adx_vals = adx_from_dx(&dx, 14);
    if adx_vals.len() < 6 { return Ok(0.0); }
    let cur = *adx_vals.last().unwrap();
    let prev = adx_vals[adx_vals.len() - 6];
    let denom = prev.abs().max(1e-9);
    let rate = (cur - prev) / denom;
    Ok(rate.clamp(-1.0, 1.0))
}

pub fn day16_plus_di_14(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, l, c) = fetch_ohlc(daily_db, stock_code, date, 25);
    if h.len() < 15 { return Ok(0.5); }
    let (p, _m, _dx) = compute_dm_di(&h, &l, &c, 14);
    let last = *p.last().unwrap_or(&50.0);
    Ok((last.clamp(0.0, 100.0)) / 100.0)
}

pub fn day16_minus_di_14(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, l, c) = fetch_ohlc(daily_db, stock_code, date, 25);
    if h.len() < 15 { return Ok(0.5); }
    let (_p, m, _dx) = compute_dm_di(&h, &l, &c, 14);
    let last = *m.last().unwrap_or(&50.0);
    Ok((last.clamp(0.0, 100.0)) / 100.0)
}

pub fn day16_di_diff_ratio(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, l, c) = fetch_ohlc(daily_db, stock_code, date, 25);
    if h.len() < 15 { return Ok(0.0); }
    let (p, m, _dx) = compute_dm_di(&h, &l, &c, 14);
    let p_last = *p.last().unwrap_or(&50.0);
    let m_last = *m.last().unwrap_or(&50.0);
    let denom = (p_last + m_last).abs().max(1e-9);
    let val = (p_last - m_last) / denom;
    Ok(val.clamp(-1.0, 1.0))
}

pub fn day16_di_cross_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (h, l, c) = fetch_ohlc(daily_db, stock_code, date, 30);
    if h.len() < 20 { return Ok(0.0); }
    let (p, m, _dx) = compute_dm_di(&h, &l, &c, 14);
    if p.len() < 6 || m.len() < 6 { return Ok(0.0); }
    let n = p.len();
    let mut crossed = false;
    for i in n.saturating_sub(5)..n {
        if i == 0 { continue; }
        let up = p[i - 1] <= m[i - 1] && p[i] > m[i];
        let down = p[i - 1] >= m[i - 1] && p[i] < m[i];
        if up || down { crossed = true; break; }
    }
    Ok(if crossed { 1.0 } else { 0.0 })
}

pub fn day16_aroon_up_25(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, _l, c) = fetch_ohlc(daily_db, stock_code, date, 26);
    if c.len() < 26 { return Ok(0.5); }
    let window = &c[c.len() - 25..];
    let max_idx = window.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(i, _)| i).unwrap_or(0);
    let periods_since_high = 24 - max_idx;
    let aroon_up = 100.0 * (25.0 - periods_since_high as f64) / 25.0;
    Ok((aroon_up.clamp(0.0, 100.0)) / 100.0)
}

pub fn day16_aroon_down_25(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, _l, c) = fetch_ohlc(daily_db, stock_code, date, 26);
    if c.len() < 26 { return Ok(0.5); }
    let window = &c[c.len() - 25..];
    let min_idx = window.iter().enumerate().min_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(i, _)| i).unwrap_or(0);
    let periods_since_low = 24 - min_idx;
    let aroon_down = 100.0 * (25.0 - periods_since_low as f64) / 25.0;
    let clamped = if aroon_down < 0.0 { 0.0 } else if aroon_down > 100.0 { 100.0 } else { aroon_down };
    Ok(clamped / 100.0)
}

pub fn day16_aroon_trend_strength(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let up = day16_aroon_up_25(_db_5min, daily_db, stock_code, date)? * 100.0;
    let down = day16_aroon_down_25(_db_5min, daily_db, stock_code, date)? * 100.0;
    let val = (up - down) / 100.0;
    let clamped = if val < -1.0 { -1.0 } else if val > 1.0 { 1.0 } else { val };
    Ok(clamped)
}

pub fn day16_tii_15(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, _l, c) = fetch_ohlc(daily_db, stock_code, date, 16);
    if c.len() < 16 { return Ok(0.5); }
    let mut up_days = 0.0;
    let mut total = 0.0;
    for i in c.len() - 15..c.len() {
        if i == 0 { continue; }
        total += 1.0;
        if c[i] > c[i - 1] { up_days += 1.0; }
    }
    if total == 0.0 { return Ok(0.5); }
    let v = up_days / total; let clamped = if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v };
    Ok(clamped)
}

pub fn day16_tii_30(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, _l, c) = fetch_ohlc(daily_db, stock_code, date, 31);
    if c.len() < 31 { return Ok(0.5); }
    let mut up_days = 0.0;
    let mut total = 0.0;
    for i in c.len() - 30..c.len() {
        if i == 0 { continue; }
        total += 1.0;
        if c[i] > c[i - 1] { up_days += 1.0; }
    }
    if total == 0.0 { return Ok(0.5); }
    let v = up_days / total; let clamped = if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v };
    Ok(clamped)
}

pub fn day16_regression_slope_20d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, _l, c) = fetch_ohlc(daily_db, stock_code, date, 20);
    if c.len() < 20 { return Ok(0.0); }
    let ln: Vec<f64> = c.iter().map(|&x| if x > 0.0 { x.ln() } else { 0.0 }).collect();
    let slope = linreg_slope(&ln);
    let clamped = if slope < -0.01 { -0.01 } else if slope > 0.01 { 0.01 } else { slope };
    Ok(clamped * 50.0)
}

pub fn day16_regression_r2_20d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_h, _l, c) = fetch_ohlc(daily_db, stock_code, date, 20);
    if c.len() < 20 { return Ok(0.0); }
    let y: Vec<f64> = c.iter().map(|&x| if x > 0.0 { x.ln() } else { 0.0 }).collect();
    let n = y.len();
    if n < 2 { return Ok(0.0); }
    let slope = linreg_slope(&y);
    let mean = y.iter().sum::<f64>() / n as f64;
    let mut ss_tot = 0.0;
    let mut ss_res = 0.0;
    for i in 0..n {
        let yi = y[i];
        let yhat = slope * i as f64 + (mean - slope * ((n - 1) as f64) / 2.0);
        ss_tot += (yi - mean).powi(2);
        ss_res += (yi - yhat).powi(2);
    }
    if ss_tot <= 0.0 { return Ok(0.0); }
    let r2 = 1.0 - ss_res / ss_tot;
    let r2c = if r2 < 0.0 { 0.0 } else if r2 > 1.0 { 1.0 } else { r2 };
    Ok(r2c)
}

 

