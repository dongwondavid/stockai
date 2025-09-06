use crate::utility::errors::StockrsResult;
use super::indicators::ma::{clip, ema_last};
use rusqlite::Connection;

fn fetch_daily_series(daily_db: &Connection, stock_code: &str, date: &str, limit: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut closes = Vec::new();
    let mut highs = Vec::new();
    let mut lows = Vec::new();
    let mut vols = Vec::new();
    if limit == 0 { return (closes, highs, lows, vols); }
    let table = stock_code;
    if let Ok(mut stmt) = daily_db.prepare(&format!(
        "SELECT close, high, low, volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT {}",
        table, limit
    )) {
        if let Ok(rows) = stmt.query_map([date], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?, row.get::<_, i32>(3)?))) {
            for r in rows.flatten() {
                let (c,h,l,v) = r;
                closes.push(c as f64);
                highs.push(h as f64);
                lows.push(l as f64);
                vols.push(v as f64);
            }
        }
    }
    closes.reverse(); highs.reverse(); lows.reverse(); vols.reverse();
    (closes, highs, lows, vols)
}

fn stdev(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    var.sqrt()
}

fn rsi_from_series(values: &[f64], period: usize) -> f64 {
    if values.len() < period + 1 { return 50.0; }
    let mut gains = 0.0; let mut losses = 0.0;
    for i in 1..values.len() {
        let ch = values[i] - values[i-1];
        if ch > 0.0 { gains += ch; } else { losses += -ch; }
    }
    if losses == 0.0 { return 100.0; }
    let rs = (gains / period as f64) / (losses / period as f64);
    100.0 - (100.0 / (1.0 + rs))
}

fn obv_series(closes: &[f64], vols: &[f64]) -> Vec<f64> {
    if closes.len() < 2 || vols.len() != closes.len() { return vec![]; }
    let mut out = Vec::with_capacity(closes.len());
    let mut obv = 0.0;
    out.push(obv);
    for i in 1..closes.len() {
        if closes[i] > closes[i-1] { obv += vols[i]; }
        else if closes[i] < closes[i-1] { obv -= vols[i]; }
        out.push(obv);
    }
    out
}

fn adl_series(highs: &[f64], lows: &[f64], closes: &[f64], vols: &[f64]) -> Vec<f64> {
    if highs.len() < 1 || lows.len() != highs.len() || closes.len() != highs.len() || vols.len() != highs.len() { return vec![]; }
    let mut out = Vec::with_capacity(closes.len());
    let mut adl = 0.0;
    for i in 0..closes.len() {
        let range = (highs[i] - lows[i]).abs();
        let mfm = if range == 0.0 { 0.0 } else { ((closes[i] - lows[i]) - (highs[i] - closes[i])) / range };
        adl += mfm * vols[i];
        out.push(adl);
    }
    out
}

fn pvt_series(closes: &[f64], vols: &[f64]) -> Vec<f64> {
    if closes.len() < 2 || vols.len() != closes.len() { return vec![]; }
    let mut out = Vec::with_capacity(closes.len());
    let mut pvt = 0.0;
    out.push(pvt);
    for i in 1..closes.len() {
        if closes[i-1] != 0.0 {
            let pct = (closes[i] - closes[i-1]) / closes[i-1];
            pvt += pct * vols[i];
        }
        out.push(pvt);
    }
    out
}

pub fn day15_obv_change_5d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, _h, _l, vols) = fetch_daily_series(daily_db, stock_code, date, 40);
    if closes.len() < 6 { return Ok(0.0); }
    let obv = obv_series(&closes, &vols);
    if obv.len() < 6 { return Ok(0.0); }
    let cur = *obv.last().unwrap();
    let prev = obv[obv.len()-6];
    let denom = prev.abs().max(1e-9);
    let rate = (cur - prev) / denom;
    Ok(clip(rate, -1.0, 1.0))
}

pub fn day15_obv_divergence_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, _h, _l, vols) = fetch_daily_series(daily_db, stock_code, date, 70);
    if closes.len() < 61 { return Ok(0.0); }
    let obv = obv_series(&closes, &vols);
    if obv.len() < 61 { return Ok(0.0); }
    // Compare last value against previous 60 excluding itself
    let n = closes.len();
    let price_hh = *closes.last().unwrap() > closes[n-61..n-1].iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let m = obv.len();
    let obv_hh = *obv.last().unwrap() > obv[m-61..m-1].iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok(if price_hh && !obv_hh { 1.0 } else { 0.0 })
}

pub fn day15_ad_line_value(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, highs, lows, vols) = fetch_daily_series(daily_db, stock_code, date, 60);
    if closes.is_empty() { return Ok(0.0); }
    let adl = adl_series(&highs, &lows, &closes, &vols);
    let last = *adl.last().unwrap_or(&0.0);
    Ok((1.0 + last.abs()).ln())
}

pub fn day15_ad_line_change_5d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, highs, lows, vols) = fetch_daily_series(daily_db, stock_code, date, 40);
    if closes.len() < 6 { return Ok(0.0); }
    let adl = adl_series(&highs, &lows, &closes, &vols);
    if adl.len() < 6 { return Ok(0.0); }
    let cur = *adl.last().unwrap();
    let prev = adl[adl.len()-6];
    let denom = prev.abs().max(1e-9);
    let rate = (cur - prev) / denom;
    Ok(clip(rate, -1.0, 1.0))
}

pub fn day15_mfi_7(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, highs, lows, vols) = fetch_daily_series(daily_db, stock_code, date, 8);
    if closes.len() < 8 { return Ok(0.5); }
    let mut pos = 0.0; let mut neg = 0.0;
    for i in 1..closes.len() {
        let tp = (highs[i] + lows[i] + closes[i]) / 3.0;
        let prev_tp = (highs[i-1] + lows[i-1] + closes[i-1]) / 3.0;
        let mf = tp * vols[i];
        if tp > prev_tp { pos += mf; } else if tp < prev_tp { neg += mf; }
    }
    if neg == 0.0 { return Ok(1.0); }
    let money_ratio = pos / neg;
    let mfi = 100.0 - (100.0 / (1.0 + money_ratio));
    Ok(clip(mfi, 0.0, 100.0) / 100.0)
}

pub fn day15_mfi_trend_slope5(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, highs, lows, vols) = fetch_daily_series(daily_db, stock_code, date, 12);
    if closes.len() < 12 { return Ok(0.0); }
    // Build MFI series (7) for last 5 points
    let mut mfi_vals = Vec::new();
    for end in 8..=closes.len() {
        let mut pos = 0.0; let mut neg = 0.0;
        for i in end-7..end {
            let tp = (highs[i] + lows[i] + closes[i]) / 3.0;
            let prev_tp = (highs[i-1] + lows[i-1] + closes[i-1]) / 3.0;
            let mf = tp * vols[i];
            if tp > prev_tp { pos += mf; } else if tp < prev_tp { neg += mf; }
        }
        let mfi = if neg == 0.0 { 100.0 } else { 100.0 - (100.0 / (1.0 + pos/neg)) };
        mfi_vals.push(mfi);
    }
    if mfi_vals.len() < 5 { return Ok(0.0); }
    let last5 = &mfi_vals[mfi_vals.len()-5..];
    // linear regression slope over 0..4
    let n = 5.0; let sum_x = 10.0; let sum_x2 = 30.0;
    let sum_y: f64 = last5.iter().sum();
    let sum_xy: f64 = last5.iter().enumerate().map(|(i,v)| *v * i as f64).sum();
    let denom = n * sum_x2 - sum_x * sum_x;
    if denom == 0.0 { return Ok(0.0); }
    let slope = (n * sum_xy - sum_x * sum_y) / denom; // per step in MFI units
    Ok(clip(slope / 100.0, -0.01, 0.01) * 50.0)
}

pub fn day15_pvt_value(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, _h, _l, vols) = fetch_daily_series(daily_db, stock_code, date, 60);
    if closes.len() < 2 { return Ok(0.0); }
    let pvt = pvt_series(&closes, &vols);
    let last = *pvt.last().unwrap_or(&0.0);
    Ok((1.0 + last.abs()).ln())
}

pub fn day15_pvt_change_5d(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, _h, _l, vols) = fetch_daily_series(daily_db, stock_code, date, 40);
    if closes.len() < 6 { return Ok(0.0); }
    let pvt = pvt_series(&closes, &vols);
    if pvt.len() < 6 { return Ok(0.0); }
    let cur = *pvt.last().unwrap();
    let prev = pvt[pvt.len()-6];
    let denom = prev.abs().max(1e-9);
    let rate = (cur - prev) / denom;
    Ok(clip(rate, -1.0, 1.0))
}

pub fn day15_chaikin_oscillator(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, highs, lows, vols) = fetch_daily_series(daily_db, stock_code, date, 40);
    if closes.len() < 20 { return Ok(0.0); }
    let adl = adl_series(&highs, &lows, &closes, &vols);
    if adl.len() < 10 { return Ok(0.0); }
    // EMA3 and EMA10 of ADL up to last point
    let ema3 = ema_last(&adl, 3);
    let ema10 = ema_last(&adl, 10);
    if !ema3.is_finite() || !ema10.is_finite() { return Ok(0.0); }
    let osc = ema3 - ema10;
    let st = stdev(&adl[adl.len().saturating_sub(20)..]);
    if st <= 0.0 { return Ok(0.0); }
    Ok(clip(osc / st, -1.0, 1.0))
}

pub fn day15_chaikin_money_flow_20(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, highs, lows, vols) = fetch_daily_series(daily_db, stock_code, date, 21);
    if closes.len() < 21 { return Ok(0.5); }
    let start = closes.len() - 20;
    let mut mfv_sum = 0.0; let mut vol_sum = 0.0;
    for i in start..closes.len() {
        let range = (highs[i] - lows[i]).abs();
        let mfm = if range == 0.0 { 0.0 } else { ((closes[i] - lows[i]) - (highs[i] - closes[i])) / range };
        mfv_sum += mfm * vols[i];
        vol_sum += vols[i];
    }
    if vol_sum == 0.0 { return Ok(0.5); }
    let cmf = mfv_sum / vol_sum; // [-1,1]
    Ok((cmf + 1.0) / 2.0)
}

pub fn day15_chaikin_mf_trend(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, highs, lows, vols) = fetch_daily_series(daily_db, stock_code, date, 31);
    if closes.len() < 31 { return Ok(0.0); }
    // CMF now and 10 periods earlier
    let cmf_now = {
        let mut mfv = 0.0; let mut v = 0.0;
        for i in closes.len()-20..closes.len() {
            let range = (highs[i] - lows[i]).abs();
            let mfm = if range == 0.0 { 0.0 } else { ((closes[i] - lows[i]) - (highs[i] - closes[i])) / range };
            mfv += mfm * vols[i]; v += vols[i];
        }
        if v == 0.0 { 0.0 } else { mfv / v }
    };
    let cmf_prev = {
        let mut mfv = 0.0; let mut v = 0.0;
        for i in closes.len()-30..closes.len()-10 {
            let range = (highs[i] - lows[i]).abs();
            let mfm = if range == 0.0 { 0.0 } else { ((closes[i] - lows[i]) - (highs[i] - closes[i])) / range };
            mfv += mfm * vols[i]; v += vols[i];
        }
        if v == 0.0 { 0.0 } else { mfv / v }
    };
    let diff = cmf_now - cmf_prev;
    Ok(clip(diff, -1.0, 1.0))
}

pub fn day15_volume_oscillator_5_20(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_c, _h, _l, vols) = fetch_daily_series(daily_db, stock_code, date, 25);
    if vols.len() < 20 { return Ok(0.0); }
    let sma5 = vols[vols.len()-5..].iter().sum::<f64>() / 5.0;
    let sma20 = vols[vols.len()-20..].iter().sum::<f64>() / 20.0;
    if sma20.abs() < 1e-12 { return Ok(0.0); }
    let val = (sma5 - sma20) / sma20;
    Ok(clip(val, -1.0, 1.0))
}

pub fn day15_volume_rsi_14(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (_c, _h, _l, vols) = fetch_daily_series(daily_db, stock_code, date, 15);
    if vols.len() < 15 { return Ok(0.5); }
    let r = rsi_from_series(&vols, 14);
    Ok(clip(r, 0.0, 100.0) / 100.0)
}

pub fn day15_price_volume_divergence_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, _h, _l, vols) = fetch_daily_series(daily_db, stock_code, date, 65);
    if closes.len() < 61 { return Ok(0.0); }
    let obv = obv_series(&closes, &vols);
    let pvt = pvt_series(&closes, &vols);
    if obv.len() < 61 || pvt.len() < 61 { return Ok(0.0); }
    let n = closes.len();
    let price_break = *closes.last().unwrap() > closes[n-61..n-1].iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let m = obv.len();
    let obv_break = *obv.last().unwrap() > obv[m-61..m-1].iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let k = pvt.len();
    let pvt_break = *pvt.last().unwrap() > pvt[k-61..k-1].iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok(if price_break && !(obv_break || pvt_break) { 1.0 } else { 0.0 })
}

pub fn day15_extreme_money_inflow_flag(_db_5min: &Connection, daily_db: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let (closes, highs, lows, vols) = fetch_daily_series(daily_db, stock_code, date, 12);
    if closes.len() < 12 { return Ok(0.0); }
    let mut mfi_vals = Vec::new();
    for end in 8..=closes.len() { // MFI(7)
        let mut pos = 0.0; let mut neg = 0.0;
        for i in end-7..end {
            let tp = (highs[i] + lows[i] + closes[i]) / 3.0;
            let prev_tp = (highs[i-1] + lows[i-1] + closes[i-1]) / 3.0;
            let mf = tp * vols[i];
            if tp > prev_tp { pos += mf; } else if tp < prev_tp { neg += mf; }
        }
        let mfi = if neg == 0.0 { 100.0 } else { 100.0 - (100.0 / (1.0 + pos/neg)) };
        mfi_vals.push(mfi);
    }
    let any_over_90 = mfi_vals.iter().rev().take(5).any(|&x| x > 90.0);
    Ok(if any_over_90 { 1.0 } else { 0.0 })
}


