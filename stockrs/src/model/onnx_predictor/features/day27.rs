use crate::utility::errors::StockrsResult;
use super::utils::get_daily_data;
use rusqlite::Connection;

fn safe_div(numer: f64, denom: f64) -> f64 { if denom == 0.0 { 0.0 } else { numer / denom } }

fn find_index(dates: &[String], date: &str) -> Option<usize> { dates.iter().position(|d| d == date) }

fn collect_prev_n_daily_ohlc(
    daily_db: &Connection,
    code: &str,
    date: &str,
    n: usize,
) -> Vec<(f64, f64, f64, f64)> {
    if n == 0 { return Vec::new(); }
    let table_name = if code.starts_with('A') { code.to_string() } else { format!("A{}", code) };
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


fn collect_prev_n_ohlc(
    daily_db: &Connection,
    code: &str,
    dates: &[String],
    idx: usize,
    n: usize,
) -> Vec<(f64, f64, f64, f64)> {
    if n == 0 { return Vec::new(); }
    let start = idx.saturating_sub(n - 1);
    let mut out = Vec::new();
    for i in start..=idx {
        if let Ok(dd) = get_daily_data(daily_db, code, &dates[i]) {
            out.push((
                dd.get_open().unwrap_or(0.0),
                dd.get_high().unwrap_or(0.0),
                dd.get_low().unwrap_or(0.0),
                dd.get_close().unwrap_or(0.0),
            ));
        }
    }
    out
}

fn close_series(ohlc: &[(f64, f64, f64, f64)]) -> Vec<f64> { ohlc.iter().map(|&(_o,_h,_l,c)| c).collect() }

fn daily_returns(ohlc: &[(f64, f64, f64, f64)]) -> Vec<f64> {
    let mut rets = Vec::new();
    for &(o,_h,_l,c) in ohlc.iter() { if o > 0.0 { rets.push((c - o) / o); } }
    rets
}

fn mean(values: &[f64]) -> f64 { if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / (values.len() as f64) } }
fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let m = mean(values);
    let var = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / ((values.len() - 1) as f64);
    var.sqrt()
}

fn max_drawdown_from_closes(closes: &[f64]) -> f64 {
    if closes.len() < 2 { return 0.0; }
    let mut peak = closes[0];
    let mut mdd: f64 = 0.0;
    for &c in closes.iter() {
        if c > peak { peak = c; }
        if peak > 0.0 { 
            let drawdown = (c - peak) / peak;
            mdd = mdd.min(drawdown); // 최소값(가장 큰 음수)을 찾음
        }
    }
    // 드로우다운은 음수이므로 절댓값을 반환 (0~1 범위)
    mdd.abs().min(1.0)
}

fn ulcer_index_from_closes(closes: &[f64]) -> f64 {
    if closes.is_empty() { return 0.0; }
    let mut peak = closes[0];
    let mut sum_sq = 0.0; let mut n = 0.0;
    for &c in closes {
        if c > peak { peak = c; }
        if peak > 0.0 { let dd_pct = (c - peak) / peak; sum_sq += dd_pct.powi(2); n += 1.0; }
    }
    (safe_div(sum_sq, n)).sqrt().abs()
}

fn drawdown_series(closes: &[f64]) -> Vec<f64> {
    let mut peak = if closes.is_empty() { 0.0 } else { closes[0] };
    let mut out = Vec::new();
    for &c in closes { if c > peak { peak = c; } out.push(if peak > 0.0 { (c - peak) / peak } else { 0.0 }); }
    out
}

fn skewness(values: &[f64]) -> f64 {
    if values.len() < 3 { return 0.0; }
    let m = mean(values);
    let sd = std_dev(values);
    if sd == 0.0 { return 0.0; }
    values.iter().map(|v| ((v - m) / sd).powi(3)).sum::<f64>() / (values.len() as f64)
}

fn kurtosis_excess(values: &[f64]) -> f64 {
    if values.len() < 3 { return 0.0; }
    let m = mean(values);
    let sd = std_dev(values);
    if sd == 0.0 { return 0.0; }
    values.iter().map(|v| ((v - m) / sd).powi(4)).sum::<f64>() / (values.len() as f64) - 3.0
}

// 최근 20일 최대 낙폭
pub fn day27_max_drawdown_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 20);
        let closes = close_series(&ohlc);
        Ok(max_drawdown_from_closes(&closes).min(1.0))
    } else { Ok(0.0) }
}

// 최근 60일 최대 낙폭
pub fn day27_max_drawdown_60d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 60);
        let closes = close_series(&ohlc);
        Ok(max_drawdown_from_closes(&closes).min(1.0))
    } else { Ok(0.0) }
}

// 최근 고점 대비 회복 소요일수 (마지막 피크 이후 경과 일수)
pub fn day27_recovery_days_from_last_peak(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let ohlc = collect_prev_n_daily_ohlc(daily_db, stock_code, date, 120);
    let closes = close_series(&ohlc);
    let mut peak = f64::MIN; let mut last_peak_idx: i32 = -1; let mut cur_idx: i32 = -1; let mut i = 0i32;
    for &c in &closes { if c >= peak { peak = c; last_peak_idx = i; } i += 1; cur_idx = i - 1; }
    let days = if last_peak_idx >= 0 && cur_idx >= last_peak_idx { (cur_idx - last_peak_idx) as f64 } else { 0.0 };
    Ok(days)
}

// 95% VaR (정규 가정): -(μ - 1.645σ)
pub fn day27_var_95_norm(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 60);
        let rets = daily_returns(&ohlc);
        let mu = mean(&rets); let sd = std_dev(&rets);
        let var = -(mu - 1.645 * sd);
        Ok(var.max(0.0).min(0.2) * 5.0)
    } else { Ok(0.0) }
}

// 95% CVaR: 하위 5% 평균 손실
pub fn day27_cvar_95(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 60);
        let mut rets = daily_returns(&ohlc);
        if rets.is_empty() { return Ok(0.0); }
        rets.sort_by(|a,b| a.partial_cmp(b).unwrap());
        let k = ((rets.len() as f64) * 0.05).ceil() as usize;
        let k = k.max(1).min(rets.len());
        let tail = &rets[..k];
        let es = -mean(tail);
        Ok(es.max(0.0).min(0.2) * 5.0)
    } else { Ok(0.0) }
}

// 드로우다운 표준편차 / 전체 수익률 표준편차
pub fn day27_drawdown_volatility_ratio(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 60);
        let closes = close_series(&ohlc);
        let dds = drawdown_series(&closes);
        let rets = daily_returns(&ohlc);
        let ratio = safe_div(std_dev(&dds).abs(), std_dev(&rets));
        Ok(ratio.min(2.0) / 2.0)
    } else { Ok(0.0) }
}

// Ulcer Index (20일)
pub fn day27_ulcer_index_20d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 20);
        let closes = close_series(&ohlc);
        Ok(ulcer_index_from_closes(&closes).min(0.2) * 5.0)
    } else { Ok(0.0) }
}

// Pain Index (60일): 평균 드로우다운 크기
pub fn day27_pain_index_60d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 60);
        let closes = close_series(&ohlc);
        let dds = drawdown_series(&closes);
        let avg_dd = -mean(&dds).min(0.0);
        Ok(avg_dd.min(0.2) * 5.0)
    } else { Ok(0.0) }
}

// 극단 이벤트(±5σ) 발생 여부 [0,1] (당일 수익률 기준)
pub fn day27_stress_var_flag(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 60);
        let rets = daily_returns(&ohlc);
        if rets.len() < 10 { return Ok(0.0); }
        let mu = mean(&rets); let sd = std_dev(&rets);
        let today = *rets.last().unwrap_or(&0.0);
        Ok(if sd > 0.0 && (today - mu).abs() >= 5.0 * sd { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}

// 평균 회복 기간 (20일 이동) 근사: 마지막 피크 이후 경과 일수 / 20
pub fn day27_time_to_recover_index(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let ohlc = collect_prev_n_daily_ohlc(daily_db, stock_code, date, 20);
    let closes = close_series(&ohlc);
    let mut peak = f64::MIN; let mut last_peak_idx: i32 = -1; let mut i = 0i32;
    for &c in &closes { if c >= peak { peak = c; last_peak_idx = i; } i += 1; }
    let cur = i - 1; let days = (cur - last_peak_idx).max(0) as f64;
    Ok((days / 20.0).min(1.0))
}

// Kelly 기준 위험도 근사치: mean/var 기반 음수면 위험 높음 → [0,1]
pub fn day27_risk_of_ruin_proxy(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let rets = daily_returns(&collect_prev_n_daily_ohlc(daily_db, stock_code, date, 60));
    let mu = mean(&rets); let sd = std_dev(&rets); let var = sd*sd;
    if var == 0.0 { return Ok(0.0); }
    let kelly = mu / var;
    Ok(((-kelly).max(0.0)).min(1.0))
}

// 드로우다운 분포 왜도
pub fn day27_drawdown_skewness(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 60);
        let closes = close_series(&ohlc); let dds = drawdown_series(&closes);
        let s = skewness(&dds);
        Ok(((s + 2.0) / 4.0).clamp(0.0, 1.0))
    } else { Ok(0.0) }
}

// 드로우다운 분포 첨도
pub fn day27_drawdown_kurtosis(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 60);
        let closes = close_series(&ohlc); let dds = drawdown_series(&closes);
        let k = kurtosis_excess(&dds).max(0.0).min(10.0);
        Ok(k / 10.0)
    } else { Ok(0.0) }
}

// 꼬리 상관계수 (시장 지수와 동조성) → 외부 시장 데이터 부재로 0.0
pub fn day27_tail_dependence_index(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 외부 시장 데이터 부재 → 내부 수익률 분포의 꼬리 특성으로 근사
    if let Some(idx) = find_index(trading_dates, date) {
        let ohlc = collect_prev_n_ohlc(daily_db, stock_code, trading_dates, idx, 60);
        let rets = daily_returns(&ohlc);
        
        if rets.len() < 10 { return Ok(0.5); } // 데이터 부족 시 중립값
        
        // 수익률의 꼬리 특성을 기반으로 의존성 근사
        let mu = mean(&rets);
        let sd = std_dev(&rets);
        
        if sd == 0.0 { return Ok(0.5); }
        
        // 표준화된 수익률의 극값 비율로 꼬리 의존성 근사
        let extreme_threshold = 2.0; // 2 표준편차 이상을 극값으로 정의
        let extreme_count = rets.iter()
            .filter(|&&r| ((r - mu) / sd).abs() >= extreme_threshold)
            .count();
        
        let extreme_ratio = extreme_count as f64 / rets.len() as f64;
        
        // 극값 비율을 [0, 1] 범위로 정규화 (높을수록 꼬리 의존성 강함)
        // 정상 분포 대비 극값 비율: 2σ 이상 = 약 4.6%, 3σ 이상 = 약 0.3%
        let normalized_dependence = (extreme_ratio / 0.05).min(1.0); // 5% 기준으로 정규화
        
        Ok(normalized_dependence)
    } else {
        Ok(0.5) // 데이터 부족 시 중립값
    }
}

// 변동성+MDD 기준 위험 국면 여부 [0,1]
pub fn day27_risk_regime_flag(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let mdd20 = day27_max_drawdown_20d(_db_5min, daily_db, stock_code, date, _trading_dates)?;
    let ohlc = collect_prev_n_daily_ohlc(daily_db, stock_code, date, 20);
    let rets = daily_returns(&ohlc);
    let vol = std_dev(&rets);
    let high_risk = (mdd20 > 0.1) || (vol > 0.03);
    Ok(if high_risk { 1.0 } else { 0.0 })
}


