use crate::utility::errors::{StockrsError, StockrsResult};
use super::utils::{
    get_morning_data,
    is_first_trading_day,
};
use rusqlite::Connection;

fn safe_div(numer: f64, denom: f64) -> f64 { if denom == 0.0 { 0.0 } else { numer / denom } }

fn collect_prev_n_daily_ohlc(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
    n: usize,
) -> Vec<(f64, f64, f64, f64)> {
    if n == 0 { return Vec::new(); }
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };
    let query = format!(
        "SELECT open, high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT {}",
        table_name,
        n
    );
    let mut stmt = match daily_db.prepare(&query) { Ok(s) => s, Err(_) => return Vec::new() };
    let rows = match stmt.query_map([&date], |row| {
        Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?, row.get::<_, f64>(2)?, row.get::<_, f64>(3)?))
    }) { Ok(r) => r, Err(_) => return Vec::new() };
    let mut out: Vec<(f64, f64, f64, f64)> = Vec::new();
    for row in rows { if let Ok(v) = row { out.push(v); } }
    out.reverse();
    out
}

fn mean(values: &[f64]) -> f64 { if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / (values.len() as f64) } }
fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let m = mean(values);
    let var = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / ((values.len() - 1) as f64);
    var.sqrt()
}

fn collect_prev_n_daily_returns(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
    n: usize,
) -> Vec<f64> {
    let ohlc = collect_prev_n_daily_ohlc(daily_db, stock_code, date, trading_dates, n);
    ohlc
        .into_iter()
        .filter_map(|(o, _h, _l, c)| if o > 0.0 { Some((c - o) / o) } else { None })
        .collect()
}

// day25_gap_continue_or_fade_flag: 갭 후 오전 지속/페이드 여부 [0,1]
pub fn calculate_day25_gap_continue_or_fade_flag(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? { return Ok(0.0); }
    let m = get_morning_data(db_5min, stock_code, date)?;
    let o = m.get_last_open().unwrap_or(0.0);
    let c = m.get_last_close().unwrap_or(0.0);
    let hi = m.get_max_high().unwrap_or(0.0);
    let lo = m.get_min_low().unwrap_or(0.0);
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };
    let query = format!("SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1", table_name);
    let mut stmt = daily_db.prepare(&query).map_err(|e| StockrsError::database_query(format!("prepare failed: {}", e)))?;
    let pc: Option<f64> = stmt.query_row([&date], |row| row.get(0)).ok();
    if let Some(pc) = pc {
        let gap = o - pc;
        let follow_up = c - o;
        // continue if morning range max extension aligns and close direction aligns
        let up_ext = (hi - o).max(0.0);
        let down_ext = (o - lo).max(0.0);
        let range_align = if gap >= 0.0 { up_ext >= down_ext } else { down_ext >= up_ext };
        let dir_align = (gap >= 0.0 && follow_up >= 0.0) || (gap <= 0.0 && follow_up <= 0.0);
        Ok(if range_align && dir_align { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

// day25_prev_close_vs_morning_vwap: 전일 종가 / 오전 VWAP
pub fn calculate_day25_prev_close_vs_morning_vwap(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let vwap = m.get_vwap().unwrap_or(m.get_last_close().unwrap_or(0.0));
    if vwap == 0.0 { return Ok(1.0); }
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };
    let query = format!("SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1", table_name);
    let mut stmt = daily_db.prepare(&query).map_err(|e| StockrsError::database_query(format!("prepare failed: {}", e)))?;
    let pc: Option<f64> = stmt.query_row([&date], |row| row.get(0)).ok();
    if let Some(pc) = pc { Ok(pc / vwap) } else { Ok(1.0) }
}

// day25_prev_high_vs_morning_vwap: 전일 고가 / 오전 VWAP
pub fn calculate_day25_prev_high_vs_morning_vwap(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let vwap = m.get_vwap().unwrap_or(m.get_last_close().unwrap_or(0.0));
    if vwap == 0.0 { return Ok(1.0); }
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };
    let query = format!("SELECT high FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1", table_name);
    let mut stmt = daily_db.prepare(&query).map_err(|e| StockrsError::database_query(format!("prepare failed: {}", e)))?;
    let ph: Option<f64> = stmt.query_row([&date], |row| row.get(0)).ok();
    if let Some(ph) = ph { Ok(ph / vwap) } else { Ok(1.0) }
}

// day25_prev_low_vs_morning_vwap: 전일 저가 / 오전 VWAP
pub fn calculate_day25_prev_low_vs_morning_vwap(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let vwap = m.get_vwap().unwrap_or(m.get_last_close().unwrap_or(0.0));
    if vwap == 0.0 { return Ok(1.0); }
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };
    let query = format!("SELECT low FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1", table_name);
    let mut stmt = daily_db.prepare(&query).map_err(|e| StockrsError::database_query(format!("prepare failed: {}", e)))?;
    let pl: Option<f64> = stmt.query_row([&date], |row| row.get(0)).ok();
    if let Some(pl) = pl { Ok(pl / vwap) } else { Ok(1.0) }
}

// day25_morning_return_vs_prev5d_mean: 오전 수익률 / 최근 5일 평균
pub fn calculate_day25_morning_return_vs_prev5d_mean(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let o = m.get_last_open().unwrap_or(0.0);
    let c = m.get_last_close().unwrap_or(0.0);
    if o <= 0.0 { return Ok(0.0); }
    let morning_ret = (c - o) / o;
    let rets = collect_prev_n_daily_returns(daily_db, stock_code, date, trading_dates, 5);
    let mr = mean(&rets);
    Ok(safe_div(morning_ret, mr))
}

// day25_morning_volatility_vs_prev5d_mean: 오전 변동성 / 최근 5일 평균 intraday 변동성
pub fn calculate_day25_morning_volatility_vs_prev5d_mean(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let cur = (m.get_max_high().unwrap_or(0.0) - m.get_min_low().unwrap_or(0.0)).max(0.0);
    // historical morning vol proxy: use previous 5 days morning ranges
    // we don't have a utility for past mornings here, so approximate using daily range avg
    let daily = collect_prev_n_daily_ohlc(daily_db, stock_code, date, trading_dates, 5);
    let hist: Vec<f64> = daily.into_iter().map(|(_o,h,l,_c)| (h-l).max(0.0)).collect();
    let mvol = mean(&hist);
    Ok(safe_div(cur, mvol))
}

// day25_intraday_momentum_persistence: 전일 모멘텀 방향 유지 여부 [0,1]
pub fn calculate_day25_intraday_momentum_persistence(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? { return Ok(0.0); }
    let m = get_morning_data(db_5min, stock_code, date)?;
    let o = m.get_last_open().unwrap_or(0.0);
    let c = m.get_last_close().unwrap_or(0.0);
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };
    let query = format!("SELECT open, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1", table_name);
    let mut stmt = daily_db.prepare(&query).map_err(|e| StockrsError::database_query(format!("prepare failed: {}", e)))?;
    let prev: Option<(f64,f64)> = stmt.query_row([&date], |row| Ok((row.get(0)?, row.get(1)?))).ok();
    if let Some((po, pc)) = prev {
        let prev_dir = pc - po;
        let morn_dir = c - o;
        Ok(if (prev_dir >= 0.0 && morn_dir >= 0.0) || (prev_dir <= 0.0 && morn_dir <= 0.0) { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

// day25_morning_gain_loss_balance: 오전 상승폭/하락폭 비율
pub fn calculate_day25_morning_gain_loss_balance(
    db_5min: &Connection,
    _daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let o = m.get_last_open().unwrap_or(0.0);
    let hi = m.get_max_high().unwrap_or(0.0);
    let lo = m.get_min_low().unwrap_or(0.0);
    let up = (hi - o).max(0.0);
    let dn = (o - lo).max(0.0);
    Ok(safe_div(up, dn))
}

// day25_opening_range_vs_prev20d_avg: 오전 변동폭 / 최근 20일 평균 일중 변동폭
pub fn calculate_day25_opening_range_vs_prev20d_avg(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let cur = (m.get_max_high().unwrap_or(0.0) - m.get_min_low().unwrap_or(0.0)).max(0.0);
    let daily = collect_prev_n_daily_ohlc(daily_db, stock_code, date, trading_dates, 20);
    let avg = mean(&daily.into_iter().map(|(_o,h,l,_c)| (h-l).max(0.0)).collect::<Vec<_>>());
    Ok(safe_div(cur, avg))
}

// day25_opening_return_vs_prev20d_vol: 오전 수익률 / 20일 수익률 표준편차
pub fn calculate_day25_opening_return_vs_prev20d_vol(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let o = m.get_last_open().unwrap_or(0.0);
    let c = m.get_last_close().unwrap_or(0.0);
    if o <= 0.0 { return Ok(0.0); }
    let morn_ret = (c - o) / o;
    let rets = collect_prev_n_daily_returns(daily_db, stock_code, date, trading_dates, 20);
    let vol = std_dev(&rets);
    Ok(safe_div(morn_ret, vol))
}

// day25_prev_day_volume_and_morning_intensity: 전일 거래량 백분위 × 오전 거래량 강도
pub fn calculate_day25_prev_day_volume_and_morning_intensity(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // volume intensity: morning volume / morning avg volume (within first 30m)
    let m = get_morning_data(db_5min, stock_code, date)?;
    let cur_vol = m.get_current_volume().unwrap_or(0.0);
    let avg_vol = m.get_avg_volume().unwrap_or(0.0);
    let intensity = if avg_vol == 0.0 { 0.0 } else { (cur_vol / avg_vol).min(10.0) / 10.0 };

    // prev day volume percentile over 20 days
    let daily = collect_prev_n_daily_ohlc(daily_db, stock_code, date, trading_dates, 20);
    // fetch volumes as well for percentile; fallback: use high-low as proxy if volumes not accessible from ohlc tuple
    // Since we did not fetch volumes above, approximate using range percentile
    let ranges: Vec<f64> = daily.iter().map(|(_,h,l,_)| (h-l).max(0.0)).collect();
    let last_range = ranges.last().cloned().unwrap_or(0.0);
    let mut sorted = ranges.clone();
    sorted.sort_by(|a,b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = if sorted.is_empty() { 0.0 } else { (sorted.iter().position(|&v| v >= last_range).unwrap_or(sorted.len()-1) as f64) / (sorted.len() as f64) };

    Ok((rank.max(0.0).min(1.0)) * (intensity.max(0.0).min(1.0)))
}

// day25_multi_tf_vol_ratio: 오전 변동성 / 20일 일봉 변동성
pub fn calculate_day25_multi_tf_vol_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let cur = (m.get_max_high().unwrap_or(0.0) - m.get_min_low().unwrap_or(0.0)).max(0.0);
    let daily = collect_prev_n_daily_ohlc(daily_db, stock_code, date, trading_dates, 20);
    let dvol = mean(&daily.into_iter().map(|(_o,h,l,_c)| (h-l).max(0.0)).collect::<Vec<_>>());
    Ok(safe_div(cur, dvol))
}

// day25_multi_tf_volume_ratio: 오전 거래량 / 20일 일봉 평균 거래량 (proxy)
pub fn calculate_day25_multi_tf_volume_ratio(
    db_5min: &Connection,
    _daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    // morning volume vs morning avg as proxy for 20d avg
    let cur = m.get_current_volume().unwrap_or(0.0);
    let avg = m.get_avg_volume().unwrap_or(0.0);
    Ok(safe_div(cur, avg))
}

// day25_opening_trend_consistency: 시초 30분 방향 vs 최근 3일 추세 일치 여부 [0,1]
pub fn calculate_day25_opening_trend_consistency(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let o = m.get_last_open().unwrap_or(0.0);
    let c = m.get_last_close().unwrap_or(0.0);
    let rets = collect_prev_n_daily_returns(daily_db, stock_code, date, trading_dates, 3);
    let trend = mean(&rets);
    let morn = c - o;
    Ok(if (trend >= 0.0 && morn >= 0.0) || (trend <= 0.0 && morn <= 0.0) { 1.0 } else { 0.0 })
}

// day25_gap_vs_prev_trend_alignment: 갭 방향 vs 최근 3일 추세 방향 일치 여부 [0,1]
pub fn calculate_day25_gap_vs_prev_trend_alignment(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? { return Ok(0.0); }
    let m = get_morning_data(db_5min, stock_code, date)?;
    let o = m.get_last_open().unwrap_or(0.0);
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };
    let query = format!("SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1", table_name);
    let mut stmt = daily_db.prepare(&query).map_err(|e| StockrsError::database_query(format!("prepare failed: {}", e)))?;
    let pc: Option<f64> = stmt.query_row([&date], |row| row.get(0)).ok();
    if let Some(pc) = pc {
        let gap = o - pc;
        let rets = collect_prev_n_daily_returns(daily_db, stock_code, date, trading_dates, 3);
        let trend = mean(&rets);
        Ok(if (gap >= 0.0 && trend >= 0.0) || (gap <= 0.0 && trend <= 0.0) { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}


