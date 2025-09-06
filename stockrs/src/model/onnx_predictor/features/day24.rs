use crate::utility::errors::StockrsResult;
use super::utils::{
    get_daily_data,
    get_morning_data,
    get_prev_daily_data_opt,
    is_first_trading_day,
};
use rusqlite::Connection;

// Helpers
fn safe_div(numer: f64, denom: f64) -> f64 {
    if denom == 0.0 { 0.0 } else { numer / denom }
}

fn sign(x: f64) -> i32 {
    if x > 0.0 { 1 } else if x < 0.0 { -1 } else { 0 }
}

fn get_prev_ohlc(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<Option<(f64, f64, f64, f64)>> {
    match get_prev_daily_data_opt(daily_db, stock_code, date, trading_dates)? {
        Some(d) => Ok(Some((
            d.get_open().unwrap_or(0.0),
            d.get_high().unwrap_or(0.0),
            d.get_low().unwrap_or(0.0),
            d.get_close().unwrap_or(0.0),
        ))),
        None => Ok(None),
    }
}

fn collect_prev_n_daily_ohlc(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
    n: usize,
) -> Vec<(f64, f64, f64, f64)> {
    if n == 0 { return Vec::new(); }
    // find index of date
    let idx_opt = trading_dates.iter().position(|d| d == date);
    if idx_opt.is_none() { return Vec::new(); }
    let idx = idx_opt.unwrap();
    if idx == 0 { return Vec::new(); }
    let start = idx.saturating_sub(n);
    let mut out = Vec::new();
    for i in start..idx {
        let d = &trading_dates[i];
        if let Ok(dd) = get_daily_data(daily_db, stock_code, d) {
            let open = dd.get_open().unwrap_or(0.0);
            let high = dd.get_high().unwrap_or(0.0);
            let low = dd.get_low().unwrap_or(0.0);
            let close = dd.get_close().unwrap_or(0.0);
            out.push((open, high, low, close));
        }
    }
    out
}

fn collect_prev_n_morning_ranges(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
    n: usize,
) -> Vec<f64> {
    if n == 0 { return Vec::new(); }
    let idx_opt = trading_dates.iter().position(|d| d == date);
    if idx_opt.is_none() { return Vec::new(); }
    let idx = idx_opt.unwrap();
    if idx == 0 { return Vec::new(); }
    let start = idx.saturating_sub(n);
    let mut out = Vec::new();
    for i in start..idx {
        let d = &trading_dates[i];
        if let Ok(m) = get_morning_data(db_5min, stock_code, d) {
            let hi = m.get_max_high().unwrap_or(0.0);
            let lo = m.get_min_low().unwrap_or(0.0);
            if hi > 0.0 && lo > 0.0 { out.push((hi - lo).max(0.0)); }
        }
    }
    out
}

fn percentile_rank(values: &[f64], x: f64) -> f64 {
    if values.is_empty() { return 0.0; }
    let mut cnt = 0usize;
    for &v in values { if v <= x { cnt += 1; } }
    (cnt as f64) / (values.len() as f64)
}

fn compute_atr(daily: &[(f64, f64, f64, f64)]) -> f64 {
    // daily: vec of (open, high, low, close) in chronological order
    if daily.len() < 2 { return 0.0; }
    let mut trs = Vec::new();
    for i in 1..daily.len() {
        let (_, h, l, _c) = daily[i];
        let (_po, _ph, _pl, pc) = daily[i - 1];
        let tr = (h - l)
            .max((h - pc).abs())
            .max((l - pc).abs())
            .max(0.0);
        trs.push(tr);
    }
    if trs.is_empty() { 0.0 } else { trs.iter().sum::<f64>() / (trs.len() as f64) }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / (values.len() as f64) }
}

// std_dev not needed currently

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

// Features

// day24_morning_vs_prev_range_ratio: 오전 30분 변동폭 / 전일 고저폭
pub fn calculate_day24_morning_vs_prev_range_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0);
    }
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let hi = morning.get_max_high().unwrap_or(0.0);
    let lo = morning.get_min_low().unwrap_or(0.0);
    let morning_range = (hi - lo).max(0.0);
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((_po, ph, pl, _pc)) = prev {
        let prev_range = (ph - pl).max(0.0);
        Ok(safe_div(morning_range, prev_range))
    } else {
        Ok(0.0)
    }
}

// day24_gap_and_morning_trend_flag: 갭 방향과 오전 추세 일치 여부 [0,1]
pub fn calculate_day24_gap_and_morning_trend_flag(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? { return Ok(0.0); }
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let open = morning.get_last_open().unwrap_or(0.0);
    let last = morning.get_last_close().unwrap_or(0.0);
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((_po, _ph, _pl, pc)) = prev {
        let gap = open - pc;
        let trend = last - open;
        Ok(if sign(gap) != 0 && sign(gap) == sign(trend) { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

// day24_prev_day_gain_and_morning_follow: 전일 상승 + 오전 상승 여부 [0,1]
pub fn calculate_day24_prev_day_gain_and_morning_follow(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? { return Ok(0.0); }
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let o = morning.get_last_open().unwrap_or(0.0);
    let c = morning.get_last_close().unwrap_or(0.0);
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((po, _ph, _pl, pc)) = prev {
        Ok(if pc > po && c > o { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

// day24_prev_day_loss_and_morning_follow: 전일 하락 + 오전 하락 여부 [0,1]
pub fn calculate_day24_prev_day_loss_and_morning_follow(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? { return Ok(0.0); }
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let o = morning.get_last_open().unwrap_or(0.0);
    let c = morning.get_last_close().unwrap_or(0.0);
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((po, _ph, _pl, pc)) = prev {
        Ok(if pc < po && c < o { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

// day24_morning_high_vs_prev_close: 오전 고가 / 전일 종가
pub fn calculate_day24_morning_high_vs_prev_close(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let hi = m.get_max_high().unwrap_or(0.0);
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((_po, _ph, _pl, pc)) = prev { Ok(safe_div(hi, pc)) } else { Ok(1.0) }
}

// day24_morning_low_vs_prev_close: 오전 저가 / 전일 종가
pub fn calculate_day24_morning_low_vs_prev_close(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let lo = m.get_min_low().unwrap_or(0.0);
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((_po, _ph, _pl, pc)) = prev { Ok(safe_div(lo, pc)) } else { Ok(1.0) }
}

// day24_morning_vwap_vs_prev_close: 오전 VWAP / 전일 종가
pub fn calculate_day24_morning_vwap_vs_prev_close(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let vwap = m.get_vwap().unwrap_or(m.get_last_close().unwrap_or(0.0));
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((_po, _ph, _pl, pc)) = prev { Ok(safe_div(vwap, pc)) } else { Ok(1.0) }
}

// day24_opening_volatility_ratio: 오전 변동성 / 전일 ATR14
pub fn calculate_day24_opening_volatility_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? { return Ok(0.0); }
    let m = get_morning_data(db_5min, stock_code, date)?;
    let hi = m.get_max_high().unwrap_or(0.0);
    let lo = m.get_min_low().unwrap_or(0.0);
    let morning_vol = (hi - lo).max(0.0);
    let daily = collect_prev_n_daily_ohlc(daily_db, stock_code, date, trading_dates, 15);
    let atr14 = compute_atr(&daily);
    Ok(safe_div(morning_vol, atr14))
}

// day24_prev_trend_continuation_score: 전일 캔들 방향과 오전 캔들 방향 일치 여부 [0,1]
pub fn calculate_day24_prev_trend_continuation_score(
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
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((po, _ph, _pl, pc)) = prev {
        Ok(if sign(pc - po) != 0 && sign(pc - po) == sign(c - o) { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

// day24_morning_vs_prev_volatility_percentile: 오전 변동폭의 60일 백분위
pub fn calculate_day24_morning_vs_prev_volatility_percentile(
    db_5min: &Connection,
    _daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let hi = m.get_max_high().unwrap_or(0.0);
    let lo = m.get_min_low().unwrap_or(0.0);
    let cur_range = (hi - lo).max(0.0);
    // daily_db is unused but kept for signature consistency
    let hist = collect_prev_n_morning_ranges(db_5min, stock_code, date, trading_dates, 60);
    Ok(percentile_rank(&hist, cur_range))
}

// day24_opening_strength_ratio: (오전 고가−시가)/(전일 고가−저가)
pub fn calculate_day24_opening_strength_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let o = m.get_last_open().unwrap_or(0.0);
    let hi = m.get_max_high().unwrap_or(0.0);
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((_po, ph, pl, _pc)) = prev {
        Ok(safe_div((hi - o).max(0.0), (ph - pl).max(0.0)))
    } else { Ok(0.0) }
}

// day24_morning_candle_strength_ratio: 오전 캔들 크기 / 전일 캔들 크기
pub fn calculate_day24_morning_candle_strength_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let o = m.get_last_open().unwrap_or(0.0);
    let c = m.get_last_close().unwrap_or(0.0);
    let morning_body = (c - o).abs();
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((po, _ph, _pl, pc)) = prev {
        let prev_body = (pc - po).abs();
        Ok(safe_div(morning_body, prev_body))
    } else { Ok(0.0) }
}

// day24_opening_gap_and_range_alignment: 갭 크기 vs 오전 변동폭 방향 일치 여부 [0,1]
pub fn calculate_day24_opening_gap_and_range_alignment(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? { return Ok(0.0); }
    let m = get_morning_data(db_5min, stock_code, date)?;
    let o = m.get_last_open().unwrap_or(0.0);
    let hi = m.get_max_high().unwrap_or(0.0);
    let lo = m.get_min_low().unwrap_or(0.0);
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((_po, _ph, _pl, pc)) = prev {
        let gap = o - pc;
        let up_ext = (hi - o).max(0.0);
        let down_ext = (o - lo).max(0.0);
        if gap >= 0.0 { Ok(if up_ext >= down_ext { 1.0 } else { 0.0 }) }
        else { Ok(if down_ext >= up_ext { 1.0 } else { 0.0 }) }
    } else { Ok(0.0) }
}

// day24_morning_vwap_vs_prev_vwap: 오전 VWAP / 전일 VWAP(typical price proxy)
pub fn calculate_day24_morning_vwap_vs_prev_vwap(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let m = get_morning_data(db_5min, stock_code, date)?;
    let vwap = m.get_vwap().unwrap_or(m.get_last_close().unwrap_or(0.0));
    let prev = get_prev_ohlc(daily_db, stock_code, date, trading_dates)?;
    if let Some((_po, ph, pl, pc)) = prev {
        let prev_typical = (ph + pl + pc) / 3.0;
        Ok(safe_div(vwap, prev_typical))
    } else { Ok(1.0) }
}

// day24_opening_momentum_vs_prev5d: 오전 수익률 vs 최근 5일 평균 일일 수익률
pub fn calculate_day24_opening_momentum_vs_prev5d(
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
    let mean_ret = mean(&rets);
    Ok(safe_div(morning_ret, mean_ret))
}


