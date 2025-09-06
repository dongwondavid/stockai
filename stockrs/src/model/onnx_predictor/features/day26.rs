use crate::utility::errors::StockrsResult;
use rusqlite::Connection;

// 로컬 데이터 구조체: 필요한 일봉 확장 컬럼 포함
#[derive(Clone, Copy, Default)]
#[allow(dead_code)]
struct DayRow {
    close: f64,
    open: f64,
    high: f64,
    low: f64,
    volume: f64,
    shares_outstanding: f64,          // 상장주식수
    foreign_limit_shares: f64,        // 외국인주문한도수량
    foreign_shares: f64,              // 외국인현보유수량
    foreign_ratio_pct: f64,           // 외국인현보유비율 [%]
    inst_net_buy: f64,                // 기관순매수 (단위 불명: 금액/수량)
    inst_net_buy_cum: f64,            // 기관누적순매수
}

fn read_row(daily_db: &Connection, table: &str, date: &str) -> StockrsResult<Option<DayRow>> {
    let sql = format!(
        "SELECT close, open, high, low, volume, \"상장주식수\", \"외국인주문한도수량\", \"외국인현보유수량\", \"외국인현보유비율\", \"기관순매수\", \"기관누적순매수\" FROM \"{}\" WHERE date = ?",
        table
    );
    let mut stmt = daily_db.prepare(&sql)?;
    let mut rows = stmt.query([date])?;
    if let Some(row) = rows.next()? {
        let r = DayRow {
            close: row.get::<_, i32>(0)? as f64,
            open: row.get::<_, i32>(1)? as f64,
            high: row.get::<_, i32>(2)? as f64,
            low: row.get::<_, i32>(3)? as f64,
            volume: row.get::<_, i32>(4)? as f64,
            shares_outstanding: row.get::<_, i64>(5)? as f64,
            foreign_limit_shares: row.get::<_, i64>(6)? as f64,
            foreign_shares: row.get::<_, i64>(7)? as f64,
            foreign_ratio_pct: row.get::<_, f64>(8)? as f64,
            inst_net_buy: row.get::<_, i64>(9)? as f64,
            inst_net_buy_cum: row.get::<_, i64>(10)? as f64,
        };
        Ok(Some(r))
    } else {
        Ok(None)
    }
}

fn find_index(dates: &[String], date: &str) -> Option<usize> {
    dates.iter().position(|d| d == date)
}

fn collect_last_n(daily_db: &Connection, table: &str, dates: &[String], idx: usize, n: usize) -> Vec<(String, DayRow)> {
    if n == 0 { return Vec::new(); }
    let start = idx.saturating_sub(n - 1);
    let mut out = Vec::new();
    for i in start..=idx {
        if let Ok(Some(r)) = read_row(daily_db, table, &dates[i]) {
            out.push((dates[i].clone(), r));
        }
    }
    out
}

fn safe_div(numer: f64, denom: f64) -> f64 { if denom == 0.0 { 0.0 } else { numer / denom } }

fn percentile_rank(values: &[f64], x: f64) -> f64 {
    if values.is_empty() { return 0.0; }
    let count = values.iter().filter(|&&v| v <= x).count();
    (count as f64) / (values.len() as f64)
}

fn mean(values: &[f64]) -> f64 { if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / (values.len() as f64) } }
fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let m = mean(values);
    let var = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / ((values.len() - 1) as f64);
    var.sqrt()
}

// Foreign: 전일 외국인 순매수 금액/거래대금 비율 (근사)
// = (Δ외국인현보유수량 × Close) / (Close×Volume) = Δ외국인현보유수량 / Volume
pub fn day26_foreign_net_buy_1d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        if idx == 0 { return Ok(0.0); }
        let cur = read_row(daily_db, table, date)?.unwrap_or_default();
        let prev = read_row(daily_db, table, &trading_dates[idx - 1])?.unwrap_or_default();
        let delta_shares = cur.foreign_shares - prev.foreign_shares;
        Ok(safe_div(delta_shares, cur.volume))
    } else { Ok(0.0) }
}

// 최근 5일 외국인 누적 순매수/시가총액 (근사)
pub fn day26_foreign_net_buy_5d_sum(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        if idx == 0 { return Ok(0.0); }
        let series = collect_last_n(daily_db, table, trading_dates, idx, 6);
        if series.len() < 2 { return Ok(0.0); }
        let mut sum_delta_value = 0.0;
        for w in series.windows(2) {
            let r0 = w[0].1; // &DayRow -> Copy
            let r1 = w[1].1;
            let delta_shares = r1.foreign_shares - r0.foreign_shares;
            sum_delta_value += delta_shares * r1.close; // 금액 근사
        }
        let cur = series.last().unwrap().1;
        let mcap = cur.shares_outstanding.max(1.0) * cur.close;
        Ok(safe_div(sum_delta_value, mcap))
    } else { Ok(0.0) }
}

// 외국인 보유비율 [%] -> 0~1
pub fn day26_foreign_holding_ratio(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    let cur = read_row(daily_db, table, date)?.unwrap_or_default();
    Ok((cur.foreign_ratio_pct / 100.0).clamp(0.0, 1.0))
}

// 외국인 보유비율 5일 변화율 (현재 - 5영업일 전)
pub fn day26_foreign_holding_change_5d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        if idx < 5 { return Ok(0.0); }
        let cur = read_row(daily_db, table, date)?.unwrap_or_default();
        let past = read_row(daily_db, table, &trading_dates[idx - 5])?.unwrap_or_default();
        Ok((cur.foreign_ratio_pct - past.foreign_ratio_pct) / 100.0)
    } else { Ok(0.0) }
}

// 당일 외국인 매수 체결 비중: 5분봉 외국인 체결 데이터 부재 → 0.0 반환
pub fn day26_foreign_buy_pressure_intraday(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 5분봉 데이터 부재 → 일봉 외국인 순매수 비율로 근사
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        if idx == 0 { return Ok(0.5); } // 첫 거래일은 중립값
        
        let cur = read_row(daily_db, table, date)?.unwrap_or_default();
        let prev = read_row(daily_db, table, &trading_dates[idx - 1])?.unwrap_or_default();
        
        // 외국인 보유 변화율을 기반으로 매수 압력 근사
        let foreign_change = cur.foreign_shares - prev.foreign_shares;
        let total_volume = cur.volume.max(1.0);
        
        // 변화율을 [0, 1] 범위로 정규화
        let pressure = if foreign_change > 0.0 {
            // 매수: 변화량/거래량 비율을 0.5~1.0으로 매핑
            0.5 + 0.5 * (foreign_change / total_volume).min(1.0)
        } else if foreign_change < 0.0 {
            // 매도: 변화량/거래량 비율을 0.0~0.5로 매핑
            0.5 * (1.0 - (foreign_change.abs() / total_volume).min(1.0))
        } else {
            0.5 // 변화 없음
        };
        
        Ok(pressure)
    } else {
        Ok(0.5) // 데이터 부족 시 중립값
    }
}

// 전일 기관 순매수 금액/거래대금 비율 (근사)
pub fn day26_institution_net_buy_1d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        if idx == 0 { return Ok(0.0); }
        let cur = read_row(daily_db, table, date)?.unwrap_or_default();
        // 금액/거래대금 근사: inst_net_buy / (close*volume)
        Ok(safe_div(cur.inst_net_buy, cur.close * cur.volume).clamp(-1.0, 1.0))
    } else { Ok(0.0) }
}

// 최근 20일 기관 누적 순매수/시가총액 (근사)
pub fn day26_institution_net_buy_20d_sum(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        let series = collect_last_n(daily_db, table, trading_dates, idx, 20);
        if series.is_empty() { return Ok(0.0); }
        let sum_inst: f64 = series.iter().map(|(_, r)| r.inst_net_buy).sum();
        let cur = series.last().unwrap().1;
        let mcap = cur.shares_outstanding.max(1.0) * cur.close;
        Ok(safe_div(sum_inst, mcap).clamp(-1.0, 1.0))
    } else { Ok(0.0) }
}

// 기관 보유비율 [%] 대체: 기관누적순매수 / 상장주식수 (근사) → 0~1 클리핑
pub fn day26_institution_holding_ratio(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    _trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    let cur = read_row(daily_db, table, date)?.unwrap_or_default();
    Ok(safe_div(cur.inst_net_buy_cum, cur.shares_outstanding).clamp(0.0, 1.0))
}

// 당일 기관 매수 체결 비중: 일봉 기관 순매수 변화율 기반 근사
pub fn day26_institution_buy_pressure_intraday(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 5분봉 데이터 부재 → 일봉 기관 순매수 변화율로 근사
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        if idx == 0 { return Ok(0.5); } // 첫 거래일은 중립값
        
        let cur = read_row(daily_db, table, date)?.unwrap_or_default();
        let prev = read_row(daily_db, table, &trading_dates[idx - 1])?.unwrap_or_default();
        
        // 기관 순매수 변화율을 기반으로 매수 압력 근사
        let inst_change = cur.inst_net_buy - prev.inst_net_buy;
        let total_volume = cur.volume.max(1.0);
        
        // 변화율을 [0, 1] 범위로 정규화
        let pressure = if inst_change > 0.0 {
            // 매수: 변화량/거래량 비율을 0.5~1.0으로 매핑
            0.5 + 0.5 * (inst_change / total_volume).min(1.0)
        } else if inst_change < 0.0 {
            // 매도: 변화량/거래량 비율을 0.0~0.5로 매핑
            0.5 * (1.0 - (inst_change.abs() / total_volume).min(1.0))
        } else {
            0.5 // 변화 없음
        };
        
        Ok(pressure)
    } else {
        Ok(0.5) // 데이터 부족 시 중립값
    }
}

// 외국인 순매수 − 기관 순매수 (비율 차이)
pub fn day26_foreign_vs_institution_balance(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let f = day26_foreign_net_buy_1d(db_5min, daily_db, stock_code, date, trading_dates)?;
    let i = day26_institution_net_buy_1d(db_5min, daily_db, stock_code, date, trading_dates)?;
    Ok((f - i).clamp(-1.0, 1.0))
}

// 최근 20일 외국인 순매수 변동성 (비율 표준편차)
pub fn day26_foreign_flow_volatility(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        if idx == 0 { return Ok(0.0); }
        let series = collect_last_n(daily_db, table, trading_dates, idx, 21);
        if series.len() < 2 { return Ok(0.0); }
        let mut ratios = Vec::new();
        for w in series.windows(2) {
            let r0 = w[0].1;
            let r1 = w[1].1;
            let delta_shares = r1.foreign_shares - r0.foreign_shares;
            ratios.push(safe_div(delta_shares, r1.volume));
        }
        Ok(std_dev(&ratios).min(1.0))
    } else { Ok(0.0) }
}

// 최근 20일 기관 순매수 변동성 (비율 표준편차)
pub fn day26_institution_flow_volatility(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        let series = collect_last_n(daily_db, table, trading_dates, idx, 20);
        if series.is_empty() { return Ok(0.0); }
        let ratios: Vec<f64> = series.iter().map(|(_, r)| safe_div(r.inst_net_buy, r.close * r.volume)).collect();
        Ok(std_dev(&ratios).min(1.0))
    } else { Ok(0.0) }
}

// 외국인·기관 순매수 상관계수 (20일)
pub fn day26_foreign_institution_correlation(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        if idx == 0 { return Ok(0.0); }
        let series = collect_last_n(daily_db, table, trading_dates, idx, 21);
        if series.len() < 2 { return Ok(0.0); }
        let mut f_vec = Vec::new();
        let mut i_vec = Vec::new();
        for w in series.windows(2) {
            let r0 = w[0].1;
            let r1 = w[1].1;
            let delta_shares = r1.foreign_shares - r0.foreign_shares;
            f_vec.push(safe_div(delta_shares, r1.volume));
            i_vec.push(safe_div(r1.inst_net_buy, r1.close * r1.volume));
        }
        let mf = mean(&f_vec);
        let mi = mean(&i_vec);
        let mut num = 0.0; let mut df = 0.0; let mut di = 0.0;
        for k in 0..f_vec.len() {
            let a = f_vec[k] - mf; let b = i_vec[k] - mi;
            num += a * b; df += a * a; di += b * b;
        }
        let denom = df.sqrt() * di.sqrt();
        Ok(if denom == 0.0 { 0.0 } else { (num / denom).clamp(-1.0, 1.0) })
    } else { Ok(0.0) }
}

// 외국인+기관 순매수 60일 백분위 (합산 비율 기준)
pub fn day26_net_buy_percentile_60d(
    _db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        if idx == 0 { return Ok(0.0); }
        let series = collect_last_n(daily_db, table, trading_dates, idx, 60);
        if series.is_empty() { return Ok(0.0); }
        let vals: Vec<f64> = series.iter().map(|(_, r)| {
            let f_ratio = 0.0; // 단일 시점에서는 delta 정의 어려움 → 0
            let i_ratio = safe_div(r.inst_net_buy, r.close * r.volume);
            f_ratio + i_ratio
        }).collect();
        let cur = *vals.last().unwrap_or(&0.0);
        Ok(percentile_rank(&vals, cur))
    } else { Ok(0.0) }
}

// 최근 20일 순매수 흐름이 지속적 (+)이면 1, 아니면 0
pub fn day26_flow_regime_flag(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    let table = stock_code;
    if let Some(idx) = find_index(trading_dates, date) {
        if idx == 0 { return Ok(0.0); }
        let series = collect_last_n(daily_db, table, trading_dates, idx, 21);
        if series.len() < 2 { return Ok(0.0); }
        let mut sum = 0.0; let mut cnt = 0.0;
        for w in series.windows(2) {
            let r0 = w[0].1;
            let r1 = w[1].1;
            let delta_shares = r1.foreign_shares - r0.foreign_shares;
            let f_ratio = safe_div(delta_shares, r1.volume);
            let i_ratio = safe_div(r1.inst_net_buy, r1.close * r1.volume);
            sum += f_ratio + i_ratio; cnt += 1.0;
        }
        let avg = if cnt == 0.0 { 0.0 } else { sum / cnt };
        Ok(if avg > 0.0 { 1.0 } else { 0.0 })
    } else { Ok(0.0) }
}


