use crate::utility::errors::StockrsResult;
use rusqlite::Connection;

fn fetch_ohlcoc(daily_db: &Connection, stock_code: &str, date: &str) -> Option<(f64, f64, f64, f64)> {
    let table = stock_code;
    if let Ok(val) = daily_db.query_row(&format!("SELECT open, close, high, low FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1", table), [date], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?, row.get::<_, i32>(3)?))
    }) {
        let (o,c,h,l): (i32,i32,i32,i32) = val;
        return Some((o as f64, c as f64, h as f64, l as f64));
    }
    None
}

fn ratio(a: f64, b: f64) -> f64 { if b == 0.0 { 0.0 } else { (a / b).clamp(0.0, 1.0) } }

pub fn day18_doji_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    if let Some((o,c,h,l)) = fetch_ohlcoc(daily_db, stock_code, date) {
        let body = (c - o).abs();
        let range = (h - l).abs();
        if range == 0.0 { return Ok(0.0); }
        Ok(if body / range <= 0.1 { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

pub fn day18_inverted_hammer_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    if let Some((o,c,h,l)) = fetch_ohlcoc(daily_db, stock_code, date) {
        let upper = h - o.max(c);
        let lower = o.min(c) - l;
        let body = (c - o).abs();
        Ok(if upper > 2.0 * body && lower < 0.5 * body { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

pub fn day18_shooting_star_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    day18_inverted_hammer_flag(_db_5min, daily_db, stock_code, date)
}

pub fn day18_spinning_top_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    if let Some((o,c,h,l)) = fetch_ohlcoc(daily_db, stock_code, date) {
        let body = (c - o).abs();
        let range = (h - l).abs();
        let upper = h - o.max(c);
        let lower = o.min(c) - l;
        Ok(if body / range <= 0.3 && upper > body && lower > body { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

pub fn day18_three_white_soldiers_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!("SELECT open, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 3", table))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)))?;
    let mut oc = Vec::new(); for r in rows { oc.push(r?); }
    if oc.len() < 3 { return Ok(0.0); }
    oc.reverse();
    let up = oc.iter().all(|&(o,c)| c as f64 > o as f64);
    Ok(if up { 1.0 } else { 0.0 })
}

pub fn day18_three_black_crows_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!("SELECT open, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 3", table))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)))?;
    let mut oc = Vec::new(); for r in rows { oc.push(r?); }
    if oc.len() < 3 { return Ok(0.0); }
    oc.reverse();
    let dn = oc.iter().all(|&(o,c)| (c as f64) < (o as f64));
    Ok(if dn { 1.0 } else { 0.0 })
}

pub fn day18_tweezer_top_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!("SELECT high FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 2", table))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut hs: Vec<f64> = Vec::new(); for r in rows { hs.push(r? as f64); }
    if hs.len() < 2 { return Ok(0.0); }
    hs.reverse();
    Ok(if (hs[1] - hs[0]).abs() / hs[0].max(1.0) <= 0.003 { 1.0 } else { 0.0 })
}

pub fn day18_tweezer_bottom_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!("SELECT low FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 2", table))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut ls: Vec<f64> = Vec::new(); for r in rows { ls.push(r? as f64); }
    if ls.len() < 2 { return Ok(0.0); }
    ls.reverse();
    Ok(if (ls[1] - ls[0]).abs() / ls[0].max(1.0) <= 0.003 { 1.0 } else { 0.0 })
}

pub fn day18_upper_shadow_ratio(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    if let Some((o,c,h,l)) = fetch_ohlcoc(daily_db, stock_code, date) {
        let range = (h - l).abs(); if range == 0.0 { return Ok(0.0); }
        let upper = h - o.max(c);
        Ok(ratio(upper.max(0.0), range))
    } else { Ok(0.0) }
}

pub fn day18_lower_shadow_ratio(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    if let Some((o,c,h,l)) = fetch_ohlcoc(daily_db, stock_code, date) {
        let range = (h - l).abs(); if range == 0.0 { return Ok(0.0); }
        let lower = o.min(c) - l;
        Ok(ratio(lower.max(0.0), range))
    } else { Ok(0.0) }
}

pub fn day18_shadow_to_body_ratio(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    if let Some((o,c,h,l)) = fetch_ohlcoc(daily_db, stock_code, date) {
        let body = (c - o).abs();
        let upper = h - o.max(c);
        let lower = o.min(c) - l;
        if body == 0.0 { return Ok(1.0); }
        let raw = (upper.max(0.0) + lower.max(0.0)) / body;
        Ok((raw.clamp(0.0, 5.0)) / 5.0)
    } else { Ok(0.0) }
}

pub fn day18_candle_body_percentile_60d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!("SELECT open, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 61", table))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)))?;
    let mut bodies = Vec::new();
    for r in rows { let (o,c) = r?; bodies.push(((c - o).abs()) as f64); }
    if bodies.len() < 61 { return Ok(0.5); }
    let cur = bodies[0]; let mut hist = bodies[1..].to_vec();
    hist.sort_by(|a,b| a.partial_cmp(b).unwrap());
    let rank = hist.iter().filter(|&&x| x < cur).count();
    Ok((rank as f64 / hist.len() as f64).clamp(0.0, 1.0))
}

pub fn day18_upper_shadow_percentile_60d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!("SELECT open, close, high FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 61", table))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?)))?;
    let mut vals = Vec::new();
    for r in rows { let (o,c,h) = r?; vals.push((h - (o.max(c))) as f64); }
    if vals.len() < 61 { return Ok(0.5); }
    let cur = vals[0]; let mut hist = vals[1..].to_vec();
    hist.sort_by(|a,b| a.partial_cmp(b).unwrap());
    let rank = hist.iter().filter(|&&x| x < cur).count();
    Ok((rank as f64 / hist.len() as f64).clamp(0.0, 1.0))
}

pub fn day18_lower_shadow_percentile_60d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!("SELECT open, close, low FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 61", table))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?)))?;
    let mut vals = Vec::new();
    for r in rows { let (o,c,l) = r?; vals.push(((o.min(c)) - l) as f64); }
    if vals.len() < 61 { return Ok(0.5); }
    let cur = vals[0]; let mut hist = vals[1..].to_vec();
    hist.sort_by(|a,b| a.partial_cmp(b).unwrap());
    let rank = hist.iter().filter(|&&x| x < cur).count();
    Ok((rank as f64 / hist.len() as f64).clamp(0.0, 1.0))
}

pub fn day18_marubozu_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    if let Some((o,c,h,l)) = fetch_ohlcoc(daily_db, stock_code, date) {
        let maru_bull = (o - l).abs() < 1e-9 && (h - c).abs() < 1e-9;
        let maru_bear = (c - l).abs() < 1e-9 && (h - o).abs() < 1e-9;
        Ok(if maru_bull || maru_bear { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}


