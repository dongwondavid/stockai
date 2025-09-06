use crate::utility::errors::StockrsResult;
use super::utils::get_morning_data;
use rusqlite::Connection;

fn stdev(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    var.sqrt()
}

pub fn day14_morning_volume_abs(
    db_5min: &Connection,
    _daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let vol = morning.get_current_volume().unwrap_or(0.0);
    Ok((1.0 + vol.max(0.0)).ln())
}

pub fn day14_morning_turnover_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // Proxy: morning volume vs last 20d average daily volume
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let morning_vol = morning.get_current_volume().unwrap_or(0.0);
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 20",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 5 { return Ok(0.0); }
    let avg_daily = vols.iter().copied().sum::<f64>() / vols.len() as f64;
    if avg_daily <= 0.0 { return Ok(0.0); }
    let ratio = (morning_vol / avg_daily).max(0.0);
    // Normalize like volume_vs_20d_avg: 0.1 -> 0, 5.0 -> 1
    let norm = if ratio <= 0.1 { 0.0 } else if ratio >= 5.0 { 1.0 } else { (ratio - 0.1) / 4.9 };
    Ok(norm)
}

pub fn day14_morning_vs_prevday_volume(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let vol = morning.get_current_volume().unwrap_or(0.0);
    let table = stock_code;
    let prev_vol: f64 = daily_db.query_row(
        &format!("SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1", table),
        [date],
        |row| row.get::<_, i32>(0),
    ).map(|v: i32| v as f64).unwrap_or(0.0);
    if prev_vol <= 0.0 { return Ok(0.0); }
    let r = (vol / prev_vol).max(0.0).min(1.0);
    Ok(r)
}

pub fn day14_volume_vs_20d_avg(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 21",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 21 { return Ok(0.5); }
    let prev = vols[0].max(0.0);
    let avg = vols[1..].iter().copied().sum::<f64>() / 20.0;
    if avg <= 0.0 { return Ok(0.5); }
    let ratio = prev / avg;
    let norm = if ratio <= 0.1 { 0.0 } else if ratio >= 5.0 { 1.0 } else { (ratio - 0.1) / 4.9 };
    Ok(norm)
}

pub fn day14_volume_percentile_60d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 61",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 61 { return Ok(0.5); }
    let prev = vols[0];
    let mut hist = vols[1..].to_vec();
    hist.sort_by(|a,b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = hist.iter().position(|&x| (x - prev).abs() < 1e-12).unwrap_or_else(||{
        match hist.binary_search_by(|x| x.partial_cmp(&prev).unwrap_or(std::cmp::Ordering::Equal)) { Ok(i)|Err(i)=>i }
    });
    let pct = (rank as f64) / hist.len() as f64;
    Ok(pct.max(0.0).min(1.0))
}

pub fn day14_volume_volatility_10d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 10",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 10 { return Ok(0.0); }
    let mean = vols.iter().sum::<f64>() / vols.len() as f64;
    if mean <= 0.0 { return Ok(0.0); }
    let sd = stdev(&vols);
    let ratio = sd / mean;
    Ok(ratio.max(0.0).min(1.0))
}

pub fn day14_volume_volatility_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 20",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 20 { return Ok(0.0); }
    let mean = vols.iter().sum::<f64>() / vols.len() as f64;
    if mean <= 0.0 { return Ok(0.0); }
    let sd = stdev(&vols);
    let ratio = sd / mean;
    Ok(ratio.max(0.0).min(1.0))
}

pub fn day14_up_vs_down_volume_ratio(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 21",
        table
    ))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)))?;
    let mut vols = Vec::new();
    let mut closes = Vec::new();
    for r in rows { let (v,c) = r?; vols.push(v as f64); closes.push(c as f64); }
    if vols.len() < 21 { return Ok(0.5); }
    // Bring to ASC for comparison
    vols.reverse(); closes.reverse();
    let mut upv = 0.0; let mut upn = 0.0; let mut dnv = 0.0; let mut dnn = 0.0;
    for i in 1..closes.len() {
        if closes[i] > closes[i-1] { upv += vols[i]; upn += 1.0; }
        else if closes[i] < closes[i-1] { dnv += vols[i]; dnn += 1.0; }
    }
    if dnn == 0.0 { return Ok(1.0); }
    let avg_up = if upn>0.0 { upv/upn } else { 0.0 };
    let avg_dn = dnv/dnn;
    let ratio = if avg_dn <= 0.0 { 2.0 } else { (avg_up/avg_dn).max(0.0) };
    let norm = if ratio <= 0.5 { 0.0 } else if ratio >= 2.0 { 1.0 } else { (ratio - 0.5) / 1.5 };
    Ok(norm)
}

pub fn day14_buying_pressure_score(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 11",
        table
    ))?;
    let rows = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)))?;
    let mut vols = Vec::new();
    let mut closes = Vec::new();
    for r in rows { let (v,c) = r?; vols.push(v as f64); closes.push(c as f64); }
    if vols.len() < 11 { return Ok(0.0); }
    vols.reverse(); closes.reverse();
    let mut up = 0.0; let mut dn = 0.0; let mut tot = 0.0;
    for i in 1..closes.len() {
        tot += vols[i];
        if closes[i] > closes[i-1] { up += vols[i]; } else if closes[i] < closes[i-1] { dn += vols[i]; }
    }
    if tot <= 0.0 { return Ok(0.0); }
    let score = (up - dn) / tot;
    Ok(score.clamp(-1.0, 1.0))
}

pub fn day14_turnover_rate_5d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // Proxy: recent 5d average volume vs prior 60d average volume
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 65",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 65 { return Ok(0.5); }
    let recent_mean = vols[0..5].iter().copied().sum::<f64>() / 5.0;
    let base_mean = vols[5..65].iter().copied().sum::<f64>() / 60.0;
    if base_mean <= 0.0 { return Ok(0.5); }
    let ratio = (recent_mean / base_mean).max(0.0);
    // Normalize: 0.1 -> 0.0, 5.0 -> 1.0
    let norm = if ratio <= 0.1 { 0.0 } else if ratio >= 5.0 { 1.0 } else { (ratio - 0.1) / 4.9 };
    Ok(norm)
}

pub fn day14_turnover_rate_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // Proxy: recent 20d average volume vs prior 60d average volume
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 80",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 80 { return Ok(0.5); }
    let recent_mean = vols[0..20].iter().copied().sum::<f64>() / 20.0;
    let base_mean = vols[20..80].iter().copied().sum::<f64>() / 60.0;
    if base_mean <= 0.0 { return Ok(0.5); }
    let ratio = (recent_mean / base_mean).max(0.0);
    // Normalize: 0.1 -> 0.0, 5.0 -> 1.0
    let norm = if ratio <= 0.1 { 0.0 } else if ratio >= 5.0 { 1.0 } else { (ratio - 0.1) / 4.9 };
    Ok(norm)
}

pub fn day14_volume_trend_slope20(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 25",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 25 { return Ok(0.0); }
    vols.reverse();
    // MA20 last 5 values
    let mut ma = Vec::new();
    for i in 20..=vols.len() { let w = &vols[i-20..i]; ma.push(w.iter().sum::<f64>()/20.0); }
    if ma.len() < 5 { return Ok(0.0); }
    let last5 = &ma[ma.len()-5..];
    // simple linreg slope
    let n = 5.0; let sum_x = 10.0; let sum_x2 = 30.0; // 0..4
    let sum_y: f64 = last5.iter().sum();
    let sum_xy: f64 = last5.iter().enumerate().map(|(i,v)| *v * i as f64).sum();
    let denom = n * sum_x2 - sum_x * sum_x;
    if denom == 0.0 { return Ok(0.0); }
    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let base = *last5.last().unwrap_or(&1.0);
    if base.abs() < 1e-12 { return Ok(0.0); }
    let rel = slope / base;
    Ok((rel.clamp(-0.01, 0.01)) * 50.0)
}

pub fn day14_volume_acceleration_10d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 20",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 20 { return Ok(0.0); }
    vols.reverse();
    let mut ma10 = Vec::new();
    for i in 10..=vols.len() { ma10.push(vols[i-10..i].iter().sum::<f64>()/10.0); }
    if ma10.len() < 3 { return Ok(0.0); }
    let n = ma10.len();
    let accel = ma10[n-1] - 2.0*ma10[n-2] + ma10[n-3];
    let base = ma10[n-1].abs().max(1e-9);
    let rel = accel / base;
    Ok((rel.clamp(-0.01, 0.01)) * 50.0)
}

pub fn day14_high_volume_spike_flag(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 61",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 61 { return Ok(0.0); }
    let prev = vols[0];
    let hist = &vols[1..];
    let mean = hist.iter().sum::<f64>() / hist.len() as f64;
    let sd = stdev(hist);
    Ok(if prev > mean + 2.0*sd { 1.0 } else { 0.0 })
}

pub fn day14_low_volume_dry_flag(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table = stock_code;
    let mut stmt = daily_db.prepare(&format!(
        "SELECT volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 61",
        table
    ))?;
    let rows = stmt.query_map([date], |row| row.get::<_, i32>(0))?;
    let mut vols: Vec<f64> = Vec::new();
    for r in rows { vols.push(r? as f64); }
    if vols.len() < 61 { return Ok(0.0); }
    let prev = vols[0];
    let hist = &vols[1..];
    let mean = hist.iter().sum::<f64>() / hist.len() as f64;
    let sd = stdev(hist);
    Ok(if prev < mean - 2.0*sd { 1.0 } else { 0.0 })
}


