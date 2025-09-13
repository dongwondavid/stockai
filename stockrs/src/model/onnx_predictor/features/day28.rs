use crate::utility::errors::StockrsResult;
use super::utils::{
    get_morning_data,
    get_prev_daily_data_opt,
};
use rusqlite::Connection;

fn safe_divide(numer: f64, denom: f64, default_when_zero: f64) -> f64 {
    if denom == 0.0 { default_when_zero } else { numer / denom }
}

fn compute_pivot_from_prev(oh: (f64, f64, f64, f64)) -> (f64, f64, f64, f64, f64, f64, f64) {
    // Input: (open, high, low, close)
    let (_po, ph, pl, pc) = oh;
    let pivot = (ph + pl + pc) / 3.0;
    let r1 = 2.0 * pivot - pl;
    let s1 = 2.0 * pivot - ph;
    let r2 = pivot + (ph - pl);
    let s2 = pivot - (ph - pl);
    let r3 = ph + 2.0 * (pivot - pl);
    let s3 = pl - 2.0 * (ph - pivot);
    (pivot, r1, s1, r2, s2, r3, s3)
}

// Helpers to get prev day OHLC
fn get_prev_ohlc(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> Option<(f64, f64, f64, f64)> {
    if let Ok(Some(d)) = get_prev_daily_data_opt(daily_db, stock_code, date, trading_dates) {
        Some((
            d.get_open().unwrap_or(0.0),
            d.get_high().unwrap_or(0.0),
            d.get_low().unwrap_or(0.0),
            d.get_close().unwrap_or(0.0),
        ))
    } else {
        None
    }
}

// day28_daily_pivot_point: (전일 고+저+종)/3
pub fn day28_daily_pivot_point(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(prev) = get_prev_ohlc(daily_db, stock_code, date, trading_dates) {
        let (pivot, _r1, _s1, _r2, _s2, _r3, _s3) = compute_pivot_from_prev(prev);
        Ok(pivot)
    } else {
        // 첫 거래일 등: 누수 방지를 위해 기본값 반환
        Ok(0.0)
    }
}

pub fn day28_pivot_resistance1(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(prev) = get_prev_ohlc(daily_db, stock_code, date, trading_dates) {
        let (_p, r1, _s1, _r2, _s2, _r3, _s3) = compute_pivot_from_prev(prev);
        Ok(r1)
    } else {
        // 첫 거래일 등: 누수 방지를 위해 기본값 반환
        Ok(0.0)
    }
}

pub fn day28_pivot_support1(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(prev) = get_prev_ohlc(daily_db, stock_code, date, trading_dates) {
        let (_p, _r1, s1, _r2, _s2, _r3, _s3) = compute_pivot_from_prev(prev);
        Ok(s1)
    } else {
        // 첫 거래일 등: 누수 방지를 위해 기본값 반환
        Ok(0.0)
    }
}

pub fn day28_pivot_resistance2(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(prev) = get_prev_ohlc(daily_db, stock_code, date, trading_dates) {
        let (_p, _r1, _s1, r2, _s2, _r3, _s3) = compute_pivot_from_prev(prev);
        Ok(r2)
    } else {
        // 첫 거래일 등: 누수 방지를 위해 기본값 반환
        Ok(0.0)
    }
}

pub fn day28_pivot_support2(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(prev) = get_prev_ohlc(daily_db, stock_code, date, trading_dates) {
        let (_p, _r1, _s1, _r2, s2, _r3, _s3) = compute_pivot_from_prev(prev);
        Ok(s2)
    } else {
        // 첫 거래일 등: 누수 방지를 위해 기본값 반환
        Ok(0.0)
    }
}

pub fn day28_pivot_resistance3(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(prev) = get_prev_ohlc(daily_db, stock_code, date, trading_dates) {
        let (_p, _r1, _s1, _r2, _s2, r3, _s3) = compute_pivot_from_prev(prev);
        Ok(r3)
    } else {
        // 첫 거래일 등: 누수 방지를 위해 기본값 반환
        Ok(0.0)
    }
}

pub fn day28_pivot_support3(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(prev) = get_prev_ohlc(daily_db, stock_code, date, trading_dates) {
        let (_p, _r1, _s1, _r2, _s2, _r3, s3) = compute_pivot_from_prev(prev);
        Ok(s3)
    } else {
        // 첫 거래일 등: 누수 방지를 위해 기본값 반환
        Ok(0.0)
    }
}

pub fn day28_price_vs_pivot_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let pivot = day28_daily_pivot_point(db_5min, daily_db, stock_code, date, trading_dates)?;
    // 현재가: 오전 마지막 체결가 사용
    let m = get_morning_data(db_5min, stock_code, date)?;
    let price = m.get_last_close().unwrap_or(0.0);
    Ok(safe_divide(price, pivot, 1.0))
}

pub fn day28_price_vs_r1_gap(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let r1 = day28_pivot_resistance1(db_5min, daily_db, stock_code, date, trading_dates)?;
    let m = get_morning_data(db_5min, stock_code, date)?;
    let price = m.get_last_close().unwrap_or(0.0);
    let denom = if r1.abs() > 0.0 { r1.abs() } else { 1.0 };
    Ok((price - r1) / denom)
}

pub fn day28_price_vs_s1_gap(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let s1 = day28_pivot_support1(db_5min, daily_db, stock_code, date, trading_dates)?;
    let m = get_morning_data(db_5min, stock_code, date)?;
    let price = m.get_last_close().unwrap_or(0.0);
    let denom = if s1.abs() > 0.0 { s1.abs() } else { 1.0 };
    Ok((price - s1) / denom)
}

pub fn day28_intraday_pivot_break_flag(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let pivot = day28_daily_pivot_point(db_5min, daily_db, stock_code, date, trading_dates)?;
    let m = get_morning_data(db_5min, stock_code, date)?;
    let hi = m.get_max_high().unwrap_or(0.0);
    let lo = m.get_min_low().unwrap_or(0.0);
    Ok(if hi >= pivot && lo <= pivot { 1.0 } else { 0.0 })
}

pub fn day28_intraday_r1_break_flag(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let r1 = day28_pivot_resistance1(db_5min, daily_db, stock_code, date, trading_dates)?;
    if r1 == 0.0 { return Ok(0.0); }
    let m = get_morning_data(db_5min, stock_code, date)?;
    let hi = m.get_max_high().unwrap_or(0.0);
    Ok(if hi >= r1 { 1.0 } else { 0.0 })
}

pub fn day28_intraday_s1_break_flag(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let s1 = day28_pivot_support1(db_5min, daily_db, stock_code, date, trading_dates)?;
    let m = get_morning_data(db_5min, stock_code, date)?;
    let lo = m.get_min_low().unwrap_or(0.0);
    Ok(if lo <= s1 { 1.0 } else { 0.0 })
}

pub fn day28_pivot_bandwidth(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let pivot = day28_daily_pivot_point(db_5min, daily_db, stock_code, date, trading_dates)?;
    let r1 = day28_pivot_resistance1(db_5min, daily_db, stock_code, date, trading_dates)?;
    let s1 = day28_pivot_support1(db_5min, daily_db, stock_code, date, trading_dates)?;
    Ok(safe_divide(r1 - s1, pivot, 0.0))
}

pub fn day28_pivot_regime_score(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 가격이 Pivot/R/S 레벨 어디에 있는지: S1 이하 0, Pivot 0.5, R1 이상 1 근사 스코어
    let pivot = day28_daily_pivot_point(db_5min, daily_db, stock_code, date, trading_dates)?;
    let r1 = day28_pivot_resistance1(db_5min, daily_db, stock_code, date, trading_dates)?;
    let s1 = day28_pivot_support1(db_5min, daily_db, stock_code, date, trading_dates)?;
    let m = get_morning_data(db_5min, stock_code, date)?;
    let price = m.get_last_close().unwrap_or(0.0);
    let score = if price <= s1 {
        0.0
    } else if price >= r1 {
        1.0
    } else {
        // 선형 보간: [s1, pivot] -> [0.0, 0.5], [pivot, r1] -> [0.5, 1.0]
        if price <= pivot {
            0.5 * safe_divide(price - s1, pivot - s1, 0.0).max(0.0).min(1.0)
        } else {
            0.5 + 0.5 * safe_divide(price - pivot, r1 - pivot, 0.0).max(0.0).min(1.0)
        }
    };
    Ok(score)
}


