use crate::utility::errors::StockrsResult;
use super::utils::get_morning_data;
use rusqlite::Connection;

fn get_prev_ohlc(daily_db: &Connection, stock_code: &str, date: &str) -> Option<(f64, f64, f64, f64)> {
    let table = stock_code;
    if let Ok(val) = daily_db.query_row(&format!("SELECT close, open, high, low FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1", table), [date], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?, row.get::<_, i32>(3)?))
    }) {
        let (pc, po, ph, pl): (i32,i32,i32,i32) = val;
        return Some((pc as f64, po as f64, ph as f64, pl as f64));
    }
    None
}

pub fn day19_gap_percent(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let open = morning.get_last_open().unwrap_or(0.0);
    let (close_prev, _op, _hp, _lp) = get_prev_ohlc(daily_db, stock_code, date).unwrap_or((0.0,0.0,0.0,0.0));
    if close_prev == 0.0 { return Ok(0.0); }
    let gp = (open - close_prev) / close_prev;
    Ok((gp.clamp(-0.1, 0.1)) * 10.0)
}

pub fn day19_gap_up_flag(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let open = morning.get_last_open().unwrap_or(0.0);
    let (close_prev, _op, _hp, _lp) = get_prev_ohlc(daily_db, stock_code, date).unwrap_or((0.0,0.0,0.0,0.0));
    Ok(if open > close_prev { 1.0 } else { 0.0 })
}

pub fn day19_gap_down_flag(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let open = morning.get_last_open().unwrap_or(0.0);
    let (close_prev, _op, _hp, _lp) = get_prev_ohlc(daily_db, stock_code, date).unwrap_or((0.0,0.0,0.0,0.0));
    Ok(if open < close_prev { 1.0 } else { 0.0 })
}

pub fn day19_gap_above_prev_high_flag(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let open = morning.get_last_open().unwrap_or(0.0);
    let (_cp, _op, high_prev, _lp) = get_prev_ohlc(daily_db, stock_code, date).unwrap_or((0.0,0.0,0.0,0.0));
    Ok(if open > high_prev { 1.0 } else { 0.0 })
}

pub fn day19_gap_below_prev_low_flag(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let open = morning.get_last_open().unwrap_or(0.0);
    let (_cp, _op, _hp, low_prev) = get_prev_ohlc(daily_db, stock_code, date).unwrap_or((0.0,0.0,0.0,0.0));
    Ok(if open < low_prev { 1.0 } else { 0.0 })
}

pub fn day19_gap_follow_through_return(db_5min: &Connection, _daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let open = morning.get_last_open().unwrap_or(0.0);
    let last_close = morning.get_last_close().unwrap_or(0.0);
    if open == 0.0 { return Ok(0.0); }
    let r = (last_close - open) / open;
    let cl = if r < -0.05 { -0.05 } else if r > 0.05 { 0.05 } else { r };
    Ok(cl * 10.0)
}

pub fn day19_opening_range_pct(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let high = morning.get_max_high().unwrap_or(0.0);
    let low = morning.get_min_low().unwrap_or(0.0);
    let (close_prev, _op, _hp, _lp) = get_prev_ohlc(daily_db, stock_code, date).unwrap_or((0.0,0.0,0.0,0.0));
    if close_prev == 0.0 { return Ok(0.0); }
    let pct = (high - low).max(0.0) / close_prev;
    let cl = if pct < 0.0 { 0.0 } else if pct > 0.1 { 0.1 } else { pct };
    Ok(cl * 10.0)
}

pub fn day19_gap_fade_flag(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let hi = morning.get_max_high().unwrap_or(0.0);
    let lo = morning.get_min_low().unwrap_or(0.0);
    let (close_prev, _op, _hp, _lp) = get_prev_ohlc(daily_db, stock_code, date).unwrap_or((0.0,0.0,0.0,0.0));
    Ok(if close_prev >= lo && close_prev <= hi { 1.0 } else { 0.0 })
}

pub fn day19_gap_fill_intraday_flag(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    day19_gap_fade_flag(db_5min, daily_db, stock_code, date)
}

pub fn day19_gap_unfilled_flag(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    // End-of-morning proxy: if prev close not reached by 30m range → 1
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let hi = morning.get_max_high().unwrap_or(0.0);
    let lo = morning.get_min_low().unwrap_or(0.0);
    let (close_prev, _op, _hp, _lp) = get_prev_ohlc(daily_db, stock_code, date).unwrap_or((0.0,0.0,0.0,0.0));
    Ok(if close_prev < lo || close_prev > hi { 1.0 } else { 0.0 })
}

pub fn day19_gap_up_freq_20d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    // Use last 21 days (including t-1 opens) to compute frequency
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!("SELECT open, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 21", table))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)))?;
    let mut data = Vec::new(); for r in rows { data.push(r?); }
    if data.len() < 21 { return Ok(0.0); }
    let mut cnt = 0.0; let mut tot = 0.0;
    for i in 0..20 { // compare open_i (older) vs close_{i+1} (previous day)
        let (o, _c_today) = data[i];
        let (_o_prev, c_prev) = data[i + 1];
        tot += 1.0; if (o as f64) > (c_prev as f64) { cnt += 1.0; }
    }
    if tot == 0.0 { return Ok(0.0); }
    let v = cnt / tot; let cl = if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v };
    Ok(cl)
}

pub fn day19_gap_down_freq_20d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!("SELECT open, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 21", table))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)))?;
    let mut data = Vec::new(); for r in rows { data.push(r?); }
    if data.len() < 21 { return Ok(0.0); }
    let mut cnt = 0.0; let mut tot = 0.0;
    for i in 0..20 {
        let (o, _c_today) = data[i];
        let (_o_prev, c_prev) = data[i + 1];
        tot += 1.0; if (o as f64) < (c_prev as f64) { cnt += 1.0; }
    }
    if tot == 0.0 { return Ok(0.0); }
    let v = cnt / tot; let cl = if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v };
    Ok(cl)
}

pub fn day19_gap_fill_rate_20d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    // Approximate gap fill using daily OHLC: if day t had a gap up (open_t > close_{t-1}),
    // consider it filled if low_t <= close_{t-1}. Similarly for gap down.
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT open, high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 21",
        table
    ))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?, row.get::<_, i32>(3)?)))?;
    let mut data: Vec<(f64, f64, f64, f64)> = Vec::new();
    for r in rows { let (o,h,l,c) = r?; data.push((o as f64, h as f64, l as f64, c as f64)); }
    if data.len() < 21 { return Ok(0.0); }
    // DESC → ASC
    data.reverse();
    let mut gaps = 0.0; let mut filled = 0.0;
    // iterate t from 1..20 relative to this window end (exclude latest day which is t=20 prior to 'date')
    for i in 1..data.len() { // compare day i vs day i-1
        let (o, _h, l, _c) = data[i].clone();
        let (_o_prev, _h_prev, _l_prev, c_prev) = data[i-1].clone();
        if o > c_prev { // gap up
            gaps += 1.0;
            if l <= c_prev { filled += 1.0; }
        } else if o < c_prev { // gap down
            gaps += 1.0;
            if _h >= c_prev { filled += 1.0; }
        }
        if gaps >= 20.0 { break; }
    }
    if gaps == 0.0 { return Ok(0.0); }
    let rate = {
        let r = filled / gaps;
        if r < 0.0 { 0.0 } else if r > 1.0 { 1.0 } else { r }
    };
    Ok(rate)
}

pub fn day19_extreme_gap_flag(db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let v = day19_gap_percent(db_5min, daily_db, stock_code, date)?;
    Ok(if v.abs() >= 0.5 { 1.0 } else { 0.0 })
}


