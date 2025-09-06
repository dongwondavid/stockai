//! Moving Average indicator utilities for feature engineering (Day10 theme)
//!
//! This module centralizes MA/EMA computation, slopes, cross detection,
//! and alignment scoring. Designed to avoid look-ahead: callers must pass
//! daily closes **up to and including the day before** the prediction day.
//!
//! # Conventions
//! - Inputs are plain slices ordered **oldest → newest**.
//! - Functions are defensive against empty/short inputs.
//! - `compute_ma_bundle` caches a useful subset for 5/20/60/120/200 + 12/26.
//! - Downstream feature functions should handle `NaN` (returned when insufficient data).
//!
//! # Example
//! ```ignore
//! use crate::indicators::ma::{compute_ma_bundle, MaBundle, ema_last, sma_last};
//! let closes = vec![/* ... historical closes up to T-1 ... */];
//! if let Some(bundle) = compute_ma_bundle(&closes) {
//!     let s = bundle.sma5; // may be NaN if not enough data
//! }
//! ```

use core::f64;

#[derive(Debug, Clone, Copy, Default)]
pub struct MaBundle {
    pub sma5: f64,
    pub sma20: f64,
    pub sma60: f64,
    pub sma120: f64,
    pub sma200: f64,
    pub ema12: f64,
    pub ema26: f64,
    // short slope-of-SMA over 5 bars (relative per-step slope)
    pub sma5_slope5: f64,
    pub sma20_slope5: f64,
    pub sma60_slope5: f64,
}

/// Compute and return a bundle of common MA/EMA values and short slopes.
/// Returns `None` if `closes` is empty; otherwise fills fields with `NaN` when
/// the window is not satisfiable.
pub fn compute_ma_bundle(closes: &[f64]) -> Option<MaBundle> {
    if closes.is_empty() { return None; }
    
    // Calculate SMAs with bounds checking
    let sma5 = if closes.len() >= 5 { sma_last(closes, 5) } else { f64::NAN };
    let sma20 = if closes.len() >= 20 { sma_last(closes, 20) } else { f64::NAN };
    let sma60 = if closes.len() >= 60 { sma_last(closes, 60) } else { f64::NAN };
    let sma120 = if closes.len() >= 120 { sma_last(closes, 120) } else { f64::NAN };
    let sma200 = if closes.len() >= 200 { sma_last(closes, 200) } else { f64::NAN };

    // Calculate EMAs with bounds checking
    let ema12 = if closes.len() >= 12 { ema_last(closes, 12) } else { f64::NAN };
    let ema26 = if closes.len() >= 26 { ema_last(closes, 26) } else { f64::NAN };

    // Calculate slopes only when we have enough data
    let sma5_slope5 = if closes.len() >= 9 { slope_on_sma(closes, 5, 5) } else { f64::NAN };
    let sma20_slope5 = if closes.len() >= 24 { slope_on_sma(closes, 20, 5) } else { f64::NAN };
    let sma60_slope5 = if closes.len() >= 64 { slope_on_sma(closes, 60, 5) } else { f64::NAN };

    Some(MaBundle { 
        sma5, sma20, sma60, sma120, sma200, 
        ema12, ema26, 
        sma5_slope5, sma20_slope5, sma60_slope5 
    })
}

/// Simple moving average last value for a given window. `NaN` if insufficient.
pub fn sma_last(x: &[f64], window: usize) -> f64 {
    if window == 0 || x.len() < window { return f64::NAN; }
    let start = x.len() - window;
    let sum: f64 = x[start..].iter().copied().sum();
    sum / window as f64
}

/// Exponential moving average last value using standard K=2/(n+1). `NaN` if empty.
pub fn ema_last(x: &[f64], window: usize) -> f64 {
    if window == 0 || x.is_empty() { return f64::NAN; }
    let k = 2.0 / (window as f64 + 1.0);
    // Initialize with SMA of first `window` elements when possible; else first element
    let mut ema = if x.len() >= window {
        sma_last(&x[..window], window)
    } else {
        x[0]
    };
    for &v in &x[if x.len() >= window { window } else { 1 }..] {
        ema = v * k + ema * (1.0 - k);
    }
    ema
}

/// Linear regression slope (per-step) of y over indices 0..n-1. `NaN` if <2 points.
pub fn linreg_slope(y: &[f64]) -> f64 {
    let n = y.len();
    if n < 2 { return f64::NAN; }
    // x are 0..n-1; compute in O(n)
    let n_f = n as f64;
    let sum_x = (n as f64 - 1.0) * n_f / 2.0; // sum 0..n-1
    let sum_x2 = (n as f64 - 1.0) * n_f * (2.0 * n_f - 1.0) / 6.0; // sum i^2
    let sum_y: f64 = y.iter().copied().sum();
    let sum_xy: f64 = y.iter().enumerate().map(|(i, v)| v * i as f64).sum();

    let denom = n_f * sum_x2 - sum_x * sum_x;
    if denom == 0.0 { return f64::NAN; }
    (n_f * sum_xy - sum_x * sum_y) / denom
}

/// Compute slope-on-SMA: build SMA series with `ma_window`, take last `slope_window` values and
/// compute linear regression slope normalized by the last SMA value to make it scale-free.
pub fn slope_on_sma(closes: &[f64], ma_window: usize, slope_window: usize) -> f64 {
    if ma_window == 0 || slope_window == 0 { return f64::NAN; }
    
    // Calculate total required data points
    let total_needed = ma_window + slope_window - 1;
    if closes.len() < total_needed { return f64::NAN; }
    
    // Ensure we don't go out of bounds
    let start_idx = closes.len().saturating_sub(total_needed);
    let end_idx = closes.len();
    
    // Build SMA series safely
    let mut sma_series: Vec<f64> = Vec::with_capacity(slope_window);
    
    // Calculate first SMA
    let first_sma_start = start_idx;
    let first_sma_end = (first_sma_start + ma_window).min(end_idx);
    if first_sma_end <= first_sma_start { return f64::NAN; }
    
    let sum: f64 = closes[first_sma_start..first_sma_end].iter().copied().sum();
    let first_sma = sum / ma_window as f64;
    sma_series.push(first_sma);
    
    // Calculate subsequent SMAs using rolling window
    for i in 1..slope_window {
        let window_start = start_idx + i;
        let window_end = (window_start + ma_window).min(end_idx);
        
        if window_end <= window_start { break; }
        
        // Recalculate SMA for this window (safer than rolling sum for edge cases)
        let window_sum: f64 = closes[window_start..window_end].iter().copied().sum();
        let window_sma = window_sum / ma_window as f64;
        sma_series.push(window_sma);
    }
    
    // Need at least 2 points for slope calculation
    if sma_series.len() < 2 { return f64::NAN; }
    
    let slope = linreg_slope(&sma_series);
    let last = *sma_series.last().unwrap_or(&f64::NAN);
    
    if last == 0.0 || !last.is_finite() || !slope.is_finite() { return f64::NAN; }
    slope / last
}

/// Return true if a crossed above b on the last step: a_{t-1} <= b_{t-1} && a_t > b_t.
pub fn cross_up(prev_a: f64, prev_b: f64, a: f64, b: f64) -> bool {
    prev_a.is_finite() && prev_b.is_finite() && a.is_finite() && b.is_finite() && prev_a <= prev_b && a > b
}

/// Return true if a crossed below b on the last step: a_{t-1} >= b_{t-1} && a_t < b_t.
pub fn cross_down(prev_a: f64, prev_b: f64, a: f64, b: f64) -> bool {
    prev_a.is_finite() && prev_b.is_finite() && a.is_finite() && b.is_finite() && prev_a >= prev_b && a < b
}

/// Find days since last cross between two series. If `prefer_up` is true, only count upward crosses;
/// if false, only downward; if none found, returns `None`.
/// `a` and `b` must be the same length, ordered oldest→newest.
pub fn last_cross_days(a: &[f64], b: &[f64], prefer_up: Option<bool>) -> Option<usize> {
    let n = a.len();
    if n < 2 || n != b.len() { return None; }
    
    // Walk backward and test last transition.
    for i in (1..n).rev() {
        let pa = a[i - 1];
        let pb = b[i - 1];
        let ca = a[i];
        let cb = b[i];
        
        let up = cross_up(pa, pb, ca, cb);
        let down = cross_down(pa, pb, ca, cb);
        
        match prefer_up {
            Some(true) if up => return Some(n - 1 - i),
            Some(false) if down => return Some(n - 1 - i),
            None if up || down => return Some(n - 1 - i),
            _ => {}
        }
    }
    
    None
}

/// Alignment score for 5-level MA stack (SMA5,20,60,120,200). Returns 0..1.
/// It adds six pairwise order checks and divides by 6 for a soft, robust score.
pub fn alignment_score5(s5: f64, s20: f64, s60: f64, s120: f64, s200: f64) -> f64 {
    if ![s5,s20,s60,s120,s200].iter().all(|v| v.is_finite()) { return f64::NAN; }
    let mut score = 0.0;
    score += if s5 > s20 { 1.0 } else { 0.0 };
    score += if s20 > s60 { 1.0 } else { 0.0 };
    score += if s60 > s120 { 1.0 } else { 0.0 };
    score += if s120 > s200 { 1.0 } else { 0.0 };
    score += if s5 > s60 { 1.0 } else { 0.0 };
    score += if s5 > s200 { 1.0 } else { 0.0 };
    score / 6.0
}

/// Safe division helper. Returns `default` when denominator is zero or non-finite.
pub fn safe_div(numer: f64, denom: f64, default: f64) -> f64 {
    if denom == 0.0 || !numer.is_finite() || !denom.is_finite() { default } else { numer / denom }
}

/// Clip a value to a range [min, max]
pub fn clip(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

/// Calculate Average True Range (ATR) for a given period
pub fn atr(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> f64 {
    if highs.len() < period + 1 || lows.len() < period + 1 || closes.len() < period + 1 {
        return f64::NAN;
    }
    
    let mut true_ranges = Vec::new();
    
    for i in 1..closes.len() {
        let high_low = highs[i] - lows[i];
        let high_close_prev = (highs[i] - closes[i-1]).abs();
        let low_close_prev = (lows[i] - closes[i-1]).abs();
        
        let true_range = high_low.max(high_close_prev).max(low_close_prev);
        true_ranges.push(true_range);
    }
    
    if true_ranges.len() < period {
        return f64::NAN;
    }
    
    // Use simple moving average for ATR
    let start = true_ranges.len() - period;
    let sum: f64 = true_ranges[start..].iter().sum();
    sum / period as f64
}

/// Calculate Triple Exponential Moving Average (TEMA)
pub fn tema(series: &[f64], period: usize) -> f64 {
    if series.len() < period * 3 {
        return f64::NAN;
    }
    
    let ema1 = ema_last(series, period);
    let ema2 = ema_last(&ema_series(series, period), period);
    let ema3 = ema_last(&ema_series(&ema_series(series, period), period), period);
    
    3.0 * ema1 - 3.0 * ema2 + ema3
}

/// Generate TEMA series for a given period
pub fn tema_series(series: &[f64], period: usize) -> Vec<f64> {
    if series.len() < period * 3 {
        return Vec::new();
    }
    
    let mut tema_series = Vec::with_capacity(series.len());
    
    // Calculate TEMA for each point where we have enough data
    for i in period * 3 - 1..series.len() {
        let window = &series[..=i];
        let tema_val = tema(window, period);
        tema_series.push(tema_val);
    }
    
    tema_series
}

/// Weighted Moving Average (WMA) of the *last `period`* elements in `series`
fn wma(series: &[f64], period: usize) -> f64 {
    if series.len() < period { return f64::NAN; }
    let start = series.len() - period;
    let mut sum = 0.0;
    let mut weight_sum = 0.0;
    for (i, &v) in series[start..].iter().enumerate() {
        let w = (i + 1) as f64;
        sum += v * w;
        weight_sum += w;
    }
    sum / weight_sum
}

/// **정의 그대로**의 HMA 시계열
pub fn hma_series(series: &[f64], period: usize) -> Vec<f64> {
    let h = period / 2;
    let s = (period as f64).sqrt().floor() as usize;
    if period == 0 || h == 0 || s == 0 || series.len() < period { return vec![]; }

    // 1) WMA(n/2), WMA(n) 시리즈 생성 (각 시점 i에서 끝나는 윈도우)
    let mut raw: Vec<f64> = Vec::new();
    for i in period-1..series.len() {
        // 각 i에서의 WMA는 "해당 시점의 마지막 period/half 구간"에 대해 계산
        let w_half = wma(&series[..=i], h);
        let w_full = wma(&series[..=i], period);
        raw.push(2.0 * w_half - w_full);
    }

    // 2) raw 시리즈에 WMA(√n) 적용
    let mut hma_vals = Vec::new();
    for i in s-1..raw.len() {
        let val = wma(&raw[..=i], s);
        hma_vals.push(val);
    }
    hma_vals
}

/// Calculate Kaufman Adaptive Moving Average (KAMA)
pub fn kama(series: &[f64], er_period: usize, fast: usize, slow: usize) -> f64 {
    if er_period == 0 || series.len() < er_period + 1 { return f64::NAN; }

    let fast_sc = 2.0 / (fast as f64 + 1.0);
    let slow_sc = 2.0 / (slow as f64 + 1.0);

    let mut kama = series[0];
    let mut prev = kama;

    for i in er_period..series.len() {
        let change = (series[i] - series[i - er_period]).abs();
        let mut volatility = 0.0;
        for j in (i - er_period + 1)..=i {
            volatility += (series[j] - series[j - 1]).abs();
        }
        let er = if volatility > 0.0 { change / volatility } else { 0.0 };
        let sc = (er * (fast_sc - slow_sc) + slow_sc).powi(2);

        kama = series[i] * sc + prev * (1.0 - sc);
        prev = kama;
    }
    kama
}

/// Generate KAMA series for a given period
pub fn kama_series(series: &[f64], er_period: usize, fast: usize, slow: usize) -> Vec<f64> {
    if er_period == 0 || series.len() < er_period + 1 { return vec![]; }
    let mut out = Vec::with_capacity(series.len() - er_period);
    for i in er_period..series.len() {
        let val = kama(&series[..=i], er_period, fast, slow);
        out.push(val);
    }
    out
}



/// Generate EMA series for a given period
fn ema_series(series: &[f64], period: usize) -> Vec<f64> {
    if series.is_empty() || period == 0 {
        return Vec::new();
    }
    
    let k = 2.0 / (period as f64 + 1.0);
    let mut ema_series = Vec::with_capacity(series.len());
    
    // Initialize with first value
    let mut ema = series[0];
    ema_series.push(ema);
    
    // Calculate EMA for each subsequent value
    for &value in &series[1..] {
        ema = value * k + ema * (1.0 - k);
        ema_series.push(ema);
    }
    
    ema_series
}

/// Calculate Keltner Channel bounds
pub fn keltner_channel(ema: f64, atr: f64, multiplier: f64) -> (f64, f64) {
    let upper = ema + multiplier * atr;
    let lower = ema - multiplier * atr;
    (upper, lower)
}

/// PUBLIC: 마지막 `slope_window` 구간에서 SMA(ma_window)의 상대 기울기
pub fn slope_on_sma_last_n(closes: &[f64], ma_window: usize, slope_window: usize) -> f64 {
    if ma_window == 0 || slope_window == 0 { return f64::NAN; }
    slope_on_sma(closes, ma_window, slope_window)
}

/// Calculate slope over the last N periods using linear regression
pub fn slope_last_n(series: &[f64], periods: usize) -> f64 {
    if series.len() < periods {
        return f64::NAN;
    }
    
    let start = series.len() - periods;
    let window = &series[start..];
    linreg_slope(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sma_last() {
        let x = [1.0,2.0,3.0,4.0,5.0];
        assert!((sma_last(&x, 5) - 3.0).abs() < 1e-9);
        assert!(sma_last(&x, 6).is_nan());
    }

    #[test]
    fn test_ema_last_basic() {
        let x = [1.0,1.0,1.0,1.0];
        let e = ema_last(&x, 2);
        assert!(e.is_finite());
    }

    #[test]
    fn test_linreg_slope() {
        let y = [1.0, 2.0, 3.0, 4.0];
        let s = linreg_slope(&y);
        assert!(s > 0.0);
    }

    #[test]
    fn test_cross_funcs() {
        // cross_up: prev_a(1.0) <= prev_b(2.0) && a(3.0) > b(2.5) -> true
        assert!(cross_up(1.0, 2.0, 3.0, 2.5));
        // cross_up: prev_a(1.0) <= prev_b(2.0) && a(2.5) > b(2.0) -> true
        assert!(cross_up(1.0, 2.0, 2.5, 2.0));
        // cross_down: prev_a(2.0) >= prev_b(1.0) && a(1.5) < b(2.0) -> true (crossed down)
        assert!(cross_down(2.0, 1.0, 1.5, 2.0));
        // cross_down: prev_a(2.0) >= prev_b(1.0) && a(0.5) < b(1.0) -> true
        assert!(cross_down(2.0, 1.0, 0.5, 1.0));
    }

    #[test]
    fn test_alignment_score5() {
        let s = alignment_score5(10.0,9.0,8.0,7.0,6.0);
        assert!( (s - 1.0).abs() < 1e-9 );
        let s2 = alignment_score5(6.0,7.0,8.0,9.0,10.0);
        assert!( (s2 - 0.0).abs() < 1e-9 );
    }

    #[test]
    fn test_tema_series() {
        let series = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let tema_result = tema_series(&series, 3);
        assert!(!tema_result.is_empty());
        assert!(tema_result.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_hma_series() {
        let series = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let hma_result = hma_series(&series, 4);
        assert!(!hma_result.is_empty());
        assert!(hma_result.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_kama_series() {
        let series = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0];
        let kama_result = kama_series(&series, 3, 2, 30);
        assert!(!kama_result.is_empty());
        assert!(kama_result.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_atr() {
        let highs = vec![110.0, 112.0, 115.0, 113.0, 116.0];
        let lows = vec![100.0, 101.0, 102.0, 103.0, 104.0];
        let closes = vec![105.0, 106.0, 107.0, 108.0, 109.0];
        let atr_val = atr(&highs, &lows, &closes, 3);
        assert!(atr_val.is_finite());
        assert!(atr_val > 0.0);
    }

    #[test]
    fn test_keltner_channel() {
        let (upper, lower) = keltner_channel(100.0, 5.0, 2.0);
        assert!(upper > lower);
        assert!((upper - 110.0).abs() < 1e-9); // 100 + 2*5 = 110
        assert!((lower - 90.0).abs() < 1e-9);  // 100 - 2*5 = 90
    }
}
