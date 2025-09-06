use crate::utility::errors::{StockrsResult,StockrsError};
use rusqlite::Connection;
use tracing::warn;

/// Day6: RSI 14기간 계산
pub fn calculate_rsi_14(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 테이블명 생성 (종목 코드를 테이블명으로 사용)
    let table_name = stock_code.to_string();
    
    // 14일 종가 데이터 조회
    let query = format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 14",
        table_name
    );
    
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([date], |row| {
        row.get::<_, i32>(0)
    })?;
    
    let mut closes: Vec<f64> = Vec::new();
    for row in rows {
        closes.push(row? as f64);
    }
    
    // 14일 데이터가 부족하면 중립값 반환
    if closes.len() < 14 {
        warn!("RSI 14 계산을 위한 데이터 부족: {} (필요: 14, 실제: {})", stock_code, closes.len());
        return Ok(50.0);
    }
    
    // RSI 계산
    let mut gains = 0.0;
    let mut losses = 0.0;
    
    for i in 1..closes.len() {
        let change = closes[i-1] - closes[i];
        if change > 0.0 {
            gains += change;
        } else {
            losses += change.abs();
        }
    }
    
    let avg_gain = gains / 13.0;
    let avg_loss = losses / 13.0;
    
    if avg_loss == 0.0 {
        return Ok(1.0);
    }
    
    let rs = avg_gain / avg_loss;
    let rsi = 1.0 - (1.0 / (1.0 + rs));
    
    // 정규화: clip [0,1]
    let normalized_rsi = rsi.max(0.0).min(1.0);
    
    Ok(normalized_rsi)
}

/// Day6: Stochastic %K 14;3 계산
pub fn calculate_stoch_k_14_3(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 테이블명 생성 (종목 코드를 테이블명으로 사용)
    let table_name = stock_code.to_string();
    
    // 14일 고가/저가/종가 데이터 조회
    let query = format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 14",
        table_name
    );
    
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([date], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, i32>(2)?
        ))
    })?;
    
    let mut data: Vec<(f64, f64, f64)> = Vec::new();
    for row in rows {
        let (h, l, c) = row?;
        data.push((h as f64, l as f64, c as f64));
    }
    
    // 14일 데이터가 부족하면 중립값 반환
    if data.len() < 14 {
        warn!("Stochastic %K 14;3 계산을 위한 데이터 부족: {} (필요: 14, 실제: {})", stock_code, data.len());
        return Ok(0.5);
    }
    
    let current_close = data[0].2;
    let high_14 = data.iter().map(|&(h, _, _)| h).fold(f64::NEG_INFINITY, f64::max);
    let low_14 = data.iter().map(|&(_, l, _)| l).fold(f64::INFINITY, f64::min);
    
    // 14일간 고가=저가인 경우 중립값 반환
    if (high_14 - low_14).abs() < f64::EPSILON {
        return Ok(0.5);
    }
    
    let stoch_k = (current_close - low_14) / (high_14 - low_14) * 100.0;
    
    // 정규화: clip [0,100] 후 /100 to [0,1]
    let normalized_stoch_k = (stoch_k.max(0.0).min(100.0)) / 100.0;
    
    Ok(normalized_stoch_k)
}

/// Day6: ATR 14 상대값 계산
pub fn calculate_atr_14_rel(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 테이블명 생성 (종목 코드를 테이블명으로 사용)
    let table_name = stock_code.to_string();
    
    // 14일 고가/저가/종가 데이터 조회
    let query = format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 14",
        table_name
    );
    
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([date], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, i32>(2)?
        ))
    })?;
    
    let mut data: Vec<(f64, f64, f64)> = Vec::new();
    for row in rows {
        let (h, l, c) = row?;
        data.push((h as f64, l as f64, c as f64));
    }
    
    // 14일 데이터가 부족하면 중간값 반환
    if data.len() < 14 {
        warn!("ATR 14 상대값 계산을 위한 데이터 부족: {} (필요: 14, 실제: {})", stock_code, data.len());
        return Ok(0.5);
    }
    
    // True Range 계산
    let mut true_ranges = Vec::new();
    for i in 1..data.len() {
        let high = data[i-1].0;
        let low = data[i-1].1;
        let prev_close = data[i].2;
        
        let tr1 = high - low;
        let tr2 = (high - prev_close).abs();
        let tr3 = (low - prev_close).abs();
        
        let true_range = tr1.max(tr2).max(tr3);
        true_ranges.push(true_range);
    }
    
    // ATR 계산 (14일 평균)
    let atr = true_ranges.iter().sum::<f64>() / true_ranges.len() as f64;
    let current_close = data[0].2;
    
    if current_close == 0.0 {
        return Err(StockrsError::General {
            message: "종가가 0입니다".to_string()
        });
    }
    
    let atr_rel = atr / current_close * 100.0;
    
    // 정규화: clip [0.5,5.0] 후 normalize to [0,1]
    let normalized_atr_rel = if atr_rel < 0.5 {
        0.0
    } else if atr_rel > 5.0 {
        1.0
    } else {
        (atr_rel - 0.5) / 4.5
    };
    
    Ok(normalized_atr_rel)
}

/// Day6: ADX 14 계산
pub fn calculate_adx_14(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 테이블명 생성 (종목 코드를 테이블명으로 사용)
    let table_name = stock_code.to_string();
    
    // 14일 고가/저가/종가 데이터 조회
    let query = format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 14",
        table_name
    );
    
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([date], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, i32>(2)?
        ))
    })?;
    
    let mut data: Vec<(f64, f64, f64)> = Vec::new();
    for row in rows {
        let (h, l, c) = row?;
        data.push((h as f64, l as f64, c as f64));
    }
    
    // 14일 데이터가 부족하면 중간값 반환
    if data.len() < 14 {
        warn!("ADX 14 계산을 위한 데이터 부족: {} (필요: 14, 실제: {})", stock_code, data.len());
        return Ok(0.25);
    }
    
    // +DM, -DM 계산
    let mut plus_dm = Vec::new();
    let mut minus_dm = Vec::new();
    
    for i in 1..data.len() {
        let high_diff = data[i-1].0 - data[i].0;
        let low_diff = data[i].1 - data[i-1].1;
        
        let plus_dm_val = if high_diff > low_diff && high_diff > 0.0 {
            high_diff
        } else {
            0.0
        };
        
        let minus_dm_val = if low_diff > high_diff && low_diff > 0.0 {
            low_diff
        } else {
            0.0
        };
        
        plus_dm.push(plus_dm_val);
        minus_dm.push(minus_dm_val);
    }
    
    // +DI, -DI 계산
    let plus_di_sum: f64 = plus_dm.iter().sum();
    let minus_di_sum: f64 = minus_dm.iter().sum();
    
    if (plus_di_sum + minus_di_sum).abs() < f64::EPSILON {
        return Ok(0.0);
    }
    
    let dx = 100.0 * (plus_di_sum - minus_di_sum).abs() / (plus_di_sum + minus_di_sum);
    
    // ADX는 DX의 14일 평균 (간단화를 위해 현재 DX 값 사용)
    let adx = dx;
    
    // 정규화: clip [0,100] 후 /100 to [0,1]
    let normalized_adx = (adx.max(0.0).min(100.0)) / 100.0;
    
    Ok(normalized_adx)
}

/// Day6: OBV Z-score 20 계산
pub fn calculate_obv_zscore_20(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 테이블명 생성 (종목 코드를 테이블명으로 사용)
    let table_name = stock_code.to_string();
    
    // 20일 거래량/종가 데이터 조회
    let query = format!(
        "SELECT volume, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 20",
        table_name
    );
    
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([date], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, i32>(1)?
        ))
    })?;
    
    let mut data: Vec<(f64, f64)> = Vec::new();
    for row in rows {
        let (v, c) = row?;
        data.push((v as f64, c as f64));
    }
    
    // 20일 데이터가 부족하면 중립값 반환
    if data.len() < 20 {
        warn!("OBV Z-score 20 계산을 위한 데이터 부족: {} (필요: 20, 실제: {})", stock_code, data.len());
        return Ok(0.0);
    }
    
    // OBV 계산
    let mut obv_values = Vec::new();
    let mut cumulative_obv = 0.0;
    
    for i in 1..data.len() {
        let current_close = data[i-1].1;
        let prev_close = data[i].1;
        let volume = data[i-1].0;
        
        if current_close > prev_close {
            cumulative_obv += volume;
        } else if current_close < prev_close {
            cumulative_obv -= volume;
        }
        // current_close == prev_close인 경우 변화 없음
        
        obv_values.push(cumulative_obv);
    }
    
    // Z-score 계산
    let mean = obv_values.iter().sum::<f64>() / obv_values.len() as f64;
    let variance = obv_values.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>() / obv_values.len() as f64;
    let std_dev = variance.sqrt();
    
    if std_dev == 0.0 {
        return Ok(0.0);
    }
    
    let current_obv = obv_values[0];
    let z_score = (current_obv - mean) / std_dev;
    
    // 정규화: clip [-3,3] 후 /3 to [-1,1]
    let normalized_z_score = (z_score.max(-3.0).min(3.0)) / 3.0;
    
    Ok(normalized_z_score)
}

/// Day6: Donchian 20 돌파 강도 계산 (최근 종가가 '이전 20일' 채널을 돌파했는지)
pub fn calculate_donchian20_break_strength(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 테이블명 생성 (종목 코드를 테이블명으로 사용)
    let table_name = stock_code.to_string();
    
    // 최근 21개(비교할 최근 1개 + 채널 계산용 이전 20개) 가져오기
    let query = format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 21",
        table_name
    );
    
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([date], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, i32>(2)?
        ))
    })?;
    
    // DESC로 담김: [0]=가장 최근(t-1), [1..]=그 이전
    let mut data: Vec<(f64, f64, f64)> = Vec::new();
    for row in rows {
        let (h, l, c) = row?;
        data.push((h as f64, l as f64, c as f64));
    }
    
    // 최근 1 + 이전 20 = 21개 필요
    if data.len() < 21 {
        warn!("Donchian 20 돌파 강도 계산을 위한 데이터 부족: {} (필요: 21, 실제: {})", stock_code, data.len());
        return Ok(0.0);
    }
    
    let (_, _, cur_close) = data[0];
    // 채널은 "이전 20개"로 계산 (현재 bar 제외)
    let channel_slice = &data[1..21];
    
    let prior_high_20 = channel_slice.iter().map(|&(h, _, _)| h).fold(f64::NEG_INFINITY, f64::max);
    let prior_low_20 = channel_slice.iter().map(|&(_, l, _)| l).fold(f64::INFINITY, f64::min);
    
    // 분모 방어
    let range = prior_high_20 - prior_low_20;
    if !range.is_finite() || range.abs() < f64::EPSILON {
        return Ok(0.0);
    }
    
    // 돌파 강도 (동률 포함 여부는 취향: >= / <= 로 하면 접촉도 돌파로 간주)
    let strength = if cur_close > prior_high_20 {
        (cur_close - prior_high_20) / range
    } else if cur_close < prior_low_20 {
        (cur_close - prior_low_20) / range
    } else {
        0.0
    };
    
    Ok(strength.clamp(-1.0, 1.0))
}

/// Day6: MFI 14 계산
pub fn calculate_mfi_14(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 테이블명 생성 (종목 코드를 테이블명으로 사용)
    let table_name = stock_code.to_string();
    
    // 14일 고가/저가/종가/거래량 데이터 조회
    let query = format!(
        "SELECT high, low, close, volume FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 14",
        table_name
    );
    
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([date], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, i32>(1)?,
            row.get::<_, i32>(2)?,
            row.get::<_, i32>(3)?
        ))
    })?;
    
    let mut data: Vec<(f64, f64, f64, f64)> = Vec::new();
    for row in rows {
        let (h, l, c, v) = row?;
        data.push((h as f64, l as f64, c as f64, v as f64));
    }
    
    // 14일 데이터가 부족하면 중립값 반환
    if data.len() < 14 {
        warn!("MFI 14 계산을 위한 데이터 부족: {} (필요: 14, 실제: {})", stock_code, data.len());
        return Ok(0.5);
    }
    
    // Money Flow 계산
    let mut positive_money_flow = 0.0;
    let mut negative_money_flow = 0.0;
    
    for i in 1..data.len() {
        let typical_price = (data[i-1].0 + data[i-1].1 + data[i-1].2) / 3.0;
        let prev_typical_price = (data[i].0 + data[i].1 + data[i].2) / 3.0;
        let volume = data[i-1].3;
        
        let money_flow = typical_price * volume;
        
        if typical_price > prev_typical_price {
            positive_money_flow += money_flow;
        } else if typical_price < prev_typical_price {
            negative_money_flow += money_flow;
        }
        // typical_price == prev_typical_price인 경우 변화 없음
    }
    
    if negative_money_flow == 0.0 {
        return Ok(1.0);
    }
    
    let money_ratio = positive_money_flow / negative_money_flow;
    let mfi = 100.0 - (100.0 / (1.0 + money_ratio));
    
    // 정규화: clip [0,100] 후 /100 to [0,1]
    let normalized_mfi = (mfi.max(0.0).min(100.0)) / 100.0;
    
    Ok(normalized_mfi)
}
