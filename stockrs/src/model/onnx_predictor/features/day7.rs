use crate::utility::errors::{StockrsResult,StockrsError};
use super::utils::{get_morning_data, is_first_trading_day};
use rusqlite::Connection;
use tracing::{debug, warn};

/// Day7 연속형 변수 확장 특징들
/// 기존 Day1~Day4의 이산형 변수들을 연속형으로 확장

/// RSI 지표값을 0~1 범위로 정규화
pub fn calculate_rsi_value(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_rsi_value (종목: {}, 날짜: {})", stock_code, date);
    
    // RSI 계산 (기존 day4_rsi_value와 동일한 로직)
    let rsi = super::day4::calculate_rsi_value(db_5min, stock_code, date)?;
    
    // 0~1 범위로 정규화
    let normalized_rsi = (rsi / 100.0).clamp(0.0, 1.0);
    
    debug!("day7_rsi_value 결과: {} → {}", rsi, normalized_rsi);
    Ok(normalized_rsi)
}

/// RSI 강도를 0~1 범위로 정규화 (중립 대비)
pub fn calculate_rsi_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_rsi_strength (종목: {}, 날짜: {})", stock_code, date);
    
    let rsi = super::day4::calculate_rsi_value(db_5min, stock_code, date)?;
    
    // RSI 50을 기준으로 강도 계산
    let strength = (rsi - 50.0).abs() / 50.0;
    let normalized_strength = strength.clamp(0.0, 1.0);
    
    debug!("day7_rsi_strength 결과: RSI={} → 강도={}", rsi, normalized_strength);
    Ok(normalized_strength)
}

/// 캔들 길이를 시가 대비 비율로 계산 (0~1 범위)
pub fn calculate_candle_length_ratio(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_candle_length_ratio (종목: {}, 날짜: {})", stock_code, date);
    
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    match (morning_data.get_max_high(), morning_data.get_min_low(), morning_data.get_last_open()) {
        (Some(high), Some(low), Some(open)) => {
            if open == 0.0 {
                warn!("시가가 0인 경우 (종목: {}, 날짜: {})", stock_code, date);
                return Ok(0.0);
            }
            
            let length_ratio = (high - low) / open;
            let normalized_ratio = (length_ratio * 5.0).clamp(0.0, 1.0); // [0,0.2] → [0,1]
            
            debug!("day7_candle_length_ratio 결과: 고가={}, 저가={}, 시가={} → 비율={}", 
                   high, low, open, normalized_ratio);
            Ok(normalized_ratio)
        }
        _ => {
            warn!("데이터가 없음 (종목: {}, 날짜: {})", stock_code, date);
            Ok(0.0)
        }
    }
}

/// 캔들 몸통을 전체 범위 대비 비율로 계산 (0~1 범위)
pub fn calculate_candle_body_ratio(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_candle_body_ratio (종목: {}, 날짜: {})", stock_code, date);
    
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    match morning_data.get_last_candle() {
        Some((close, open, high, low)) => {
            if high == low {
                debug!("고가와 저가가 동일한 경우 (종목: {}, 날짜: {})", stock_code, date);
                return Ok(0.0);
            }
            
            let body_size = (close - open).abs();
            let total_range = high - low;
            let body_ratio = body_size / total_range;
            
            debug!("day7_candle_body_ratio 결과: 몸통={}, 범위={} → 비율={}", 
                   body_size, total_range, body_ratio);
            Ok(body_ratio)
        }
        _ => {
            warn!("데이터가 없음 (종목: {}, 날짜: {})", stock_code, date);
            Ok(0.0)
        }
    }
}

/// 저항선 돌파 강도를 계산 (0~1 범위)
pub fn calculate_breakout_strength(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_breakout_strength (종목: {}, 날짜: {})", stock_code, date);
    
    // 현재가 조회 (시가 대비 비율이 아닌 절대값)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let current_price = morning_data.get_last_close()
        .ok_or_else(|| StockrsError::prediction(
            format!("현재가를 찾을 수 없음 (종목: {})", stock_code)
        ))?;
    
    // 저항선은 20일 고점으로 가정 (일봉 데이터 사용)
    let query = format!(
        "SELECT MAX(high) FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 20",
        stock_code
    );
    
    let resistance_opt: Option<f64> = daily_db.query_row(
        &query,
        [date],
        |row| row.get::<_, Option<f64>>(0),
    )?;
    
    let resistance = match resistance_opt {
        Some(v) if v > 0.0 => v,
        _ => {
            warn!("저항선을 찾을 수 없음 (종목: {}, 날짜: {})", stock_code, date);
            return Ok(0.0);
        }
    };
    
    if current_price > resistance {
        let breakout_ratio = (current_price - resistance) / resistance;
        let normalized_strength = (breakout_ratio * 10.0).clamp(0.0, 1.0); // [0,0.1] → [0,1]
        
        debug!("day7_breakout_strength 결과: 현재가={}, 저항선={} → 돌파강도={}", 
               current_price, resistance, normalized_strength);
        Ok(normalized_strength)
    } else {
        Ok(0.0) // 저항선 미돌파
    }
}

/// 저항선 대비 현재 위치 비율 계산 (0~1 범위)
pub fn calculate_resistance_break_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_resistance_break_ratio (종목: {}, 날짜: {})", stock_code, date);
    
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let current_price = morning_data.get_last_close()
        .ok_or_else(|| StockrsError::prediction(
            format!("현재가를 찾을 수 없음 (종목: {})", stock_code)
        ))?;
    
    let query = format!(
        "SELECT MAX(high) FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 20",
        stock_code
    );
    
    let resistance_opt: Option<f64> = daily_db.query_row(
        &query,
        [date],
        |row| row.get::<_, Option<f64>>(0),
    )?;
    
    let resistance = match resistance_opt {
        Some(v) if v > 0.0 => v,
        _ => {
            warn!("저항선을 찾을 수 없음 (종목: {}, 날짜: {})", stock_code, date);
            return Ok(1.0);
        }
    };
    
    let ratio = current_price / resistance;
    let normalized_ratio = if ratio < 0.8 {
        (ratio - 0.8) / 0.4 // [0.8, 1.2] → [0, 1]
    } else if ratio > 1.2 {
        1.0
    } else {
        (ratio - 0.8) / 0.4
    };
    
    debug!("day7_resistance_break_ratio 결과: 현재가={}, 저항선={} → 비율={}", 
           current_price, resistance, normalized_ratio);
    Ok(normalized_ratio.clamp(0.0, 1.0))
}

/// 거래량 패턴 일관성 점수 계산 (0~1 범위)
pub fn calculate_volume_pattern_score(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_volume_pattern_score (종목: {}, 날짜: {})", stock_code, date);
    
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let volumes = morning_data.volumes.clone();
    
    if volumes.len() < 2 {
        warn!("거래량 데이터 부족 (종목: {}, 날짜: {})", stock_code, date);
        return Ok(0.5);
    }
    
    // 거래량 트렌드 일관성 계산 (간단한 예시)
    let mut consistent_count = 0;
    for i in 1..volumes.len() {
        if (volumes[i] > volumes[i-1] && i > 1 && volumes[i-1] > volumes[i-2]) ||
           (volumes[i] < volumes[i-1] && i > 1 && volumes[i-1] < volumes[i-2]) {
            consistent_count += 1;
        }
    }
    
    let pattern_score = if volumes.len() > 2 {
        consistent_count as f64 / (volumes.len() - 2) as f64
    } else {
        0.5
    };
    
    debug!("day7_volume_pattern_score 결과: 일관성={}/{} → 점수={}", 
           consistent_count, volumes.len(), pattern_score);
    Ok(pattern_score)
}

/// 거래량 트렌드 강도 계산 (0~1 범위)
pub fn calculate_volume_trend_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_volume_trend_strength (종목: {}, 날짜: {})", stock_code, date);
    
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let volumes = morning_data.volumes.clone();
    
    if volumes.len() < 2 {
        warn!("거래량 데이터 부족 (종목: {}, 날짜: {})", stock_code, date);
        return Ok(0.0);
    }
    
    // 거래량 변화율의 절대값 평균
    let mut total_change = 0.0;
    for i in 1..volumes.len() {
        if volumes[i-1] > 0.0 {
            total_change += ((volumes[i] - volumes[i-1]) / volumes[i-1]).abs();
        }
    }
    
    let avg_change = total_change / (volumes.len() - 1) as f64;
    let normalized_strength = (avg_change * 10.0).clamp(0.0, 1.0); // [0,0.1] → [0,1]
    
    debug!("day7_volume_trend_strength 결과: 평균변화율={} → 강도={}", 
           avg_change, normalized_strength);
    Ok(normalized_strength)
}

/// EMA 정렬 강도 점수 계산 (0~1 범위)
/// 아침 5분봉 6개 기반 단기 EMA 정렬로 기존 일봉 기반과 차별화
pub fn calculate_ema_alignment_score(
    db_5min: &Connection,
    _daily_db: &Connection, // 사용하지 않지만 기존 호출과 호환성 유지
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_ema_alignment_score (종목: {}, 날짜: {})", stock_code, date);
    
    // 아침 5분봉 6개 기반 EMA 정렬 계산 (기존 일봉 기반과 차별화)
    let morning_score = calculate_ema_alignment_morning(db_5min, stock_code, date)?;
    debug!("day7_ema_alignment_score 아침 5분봉 기반 결과: {}", morning_score);
    Ok(morning_score)
}

/// 아침 5분봉 6개 기반 EMA 정렬 점수 계산
fn calculate_ema_alignment_morning(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let closes = morning_data.closes.clone();
    
    if closes.len() < 6 {
        warn!("아침 5분봉 데이터 부족 (종목: {}, 날짜: {})", stock_code, date);
        return Ok(0.5);
    }
    
    // 최근 6개 5분봉 종가로 단기 EMA 계산
    let ema3 = calculate_ema(&closes.iter().rev().take(3).copied().collect::<Vec<f64>>(), 3);
    let ema6 = calculate_ema(&closes.iter().rev().take(6).copied().collect::<Vec<f64>>(), 6);
    
    if ema6 > 0.0 {
        let alignment_ratio = (ema3 - ema6) / ema6 * 100.0;
        // sigmoid 함수로 [0, 1] 범위로 정규화 (5분봉은 변동성이 크므로 계수 조정)
        let sigmoid = 1.0 / (1.0 + (-alignment_ratio / 20.0).exp());
        Ok(sigmoid)
    } else {
        Ok(0.5)
    }
}

/// EMA 계산 함수
fn calculate_ema(prices: &[f64], period: usize) -> f64 {
    if prices.is_empty() || period == 0 {
        return 0.0;
    }
    
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut ema = prices[0];
    
    for &price in prices.iter().skip(1) {
        ema = (price * multiplier) + (ema * (1.0 - multiplier));
    }
    
    ema
}

/// 이동평균 크로스 강도 계산 (-1~1 범위)
pub fn calculate_ma_cross_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_ma_cross_strength (종목: {}, 날짜: {})", stock_code, date);
    
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let closes = morning_data.closes.clone();
    
    if closes.len() < 6 {
        warn!("가격 데이터 부족 (종목: {}, 날짜: {})", stock_code, date);
        return Ok(0.0);
    }
    
    // EMA3과 EMA6 계산 (간단한 이동평균)
    let ema3 = closes.iter().rev().take(3).sum::<f64>() / 3.0;
    let ema6 = closes.iter().rev().take(6).sum::<f64>() / 6.0;
    
    let cross_strength = (ema3 - ema6) / ema6;
    let normalized_strength = (cross_strength * 10.0).clamp(-1.0, 1.0); // [-0.1,0.1] → [-1,1]
    
    debug!("day7_ma_cross_strength 결과: EMA3={}, EMA6={} → 강도={}", 
           ema3, ema6, normalized_strength);
    Ok(normalized_strength)
}

/// 캔들스틱 패턴 완성도 비율 계산 (0~1 범위)
pub fn calculate_pattern_completion_ratio(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_pattern_completion_ratio (종목: {}, 날짜: {})", stock_code, date);
    
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let closes = morning_data.closes.clone();
    let opens = morning_data.opens.clone();
    
    if closes.len() < 2 {
        warn!("패턴 데이터 부족 (종목: {}, 날짜: {})", stock_code, date);
        return Ok(0.0);
    }
    
    // 상승 감싸기 패턴 완성도 계산
    let prev_open = opens[opens.len() - 2];
    let prev_close = closes[closes.len() - 2];
    let curr_open = opens[opens.len() - 1];
    let curr_close = closes[closes.len() - 1];
    
    let is_bullish_engulfing = curr_close > curr_open && // 현재 양봉
                               prev_close < prev_open && // 이전 음봉
                               curr_open < prev_close && // 현재 시가 < 이전 종가
                               curr_close > prev_open;   // 현재 종가 > 이전 시가
    
    let completion_ratio = if is_bullish_engulfing {
        // 패턴 완성도: 몸통 크기 비율
        let curr_body = (curr_close - curr_open).abs();
        let prev_body = (prev_close - prev_open).abs();
        if prev_body > 0.0 {
            (curr_body / prev_body).clamp(0.0, 1.0)
        } else {
            1.0
        }
    } else {
        0.0
    };
    
    debug!("day7_pattern_completion_ratio 결과: 상승감싸기={}, 완성도={}", 
           is_bullish_engulfing, completion_ratio);
    Ok(completion_ratio)
}

/// 패턴 강도 및 신뢰도 점수 계산 (0~1 범위)
pub fn calculate_pattern_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    debug!("계산 중: day7_pattern_strength (종목: {}, 날짜: {})", stock_code, date);
    
    // 패턴 완성도와 거래량을 결합한 신뢰도 점수
    let completion_ratio = calculate_pattern_completion_ratio(db_5min, stock_code, date)?;
    let volume_score = calculate_volume_pattern_score(db_5min, stock_code, date)?;
    
    // 가중 평균으로 최종 점수 계산
    let pattern_strength = (completion_ratio * 0.7 + volume_score * 0.3).clamp(0.0, 1.0);
    
    debug!("day7_pattern_strength 결과: 완성도={}, 거래량점수={} → 최종점수={}", 
           completion_ratio, volume_score, pattern_strength);
    Ok(pattern_strength)
}

/// 시장 상황 분류 점수 계산 (0~1 범위)
pub fn calculate_market_regime_score(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    debug!("계산 중: day7_market_regime_score (종목: {}, 날짜: {})", stock_code, date);
    
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.5);
    }
    
    // 최근 20일 종가를 SQL로 조회 (현재 일자 제외)
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };
    let query = format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 20",
        table_name
    );
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([date], |row| row.get::<_, f64>(0))?;
    let mut daily_closes: Vec<f64> = Vec::new();
    for r in rows { if let Ok(c) = r { if c.is_finite() { daily_closes.push(c); } } }
    
    // 데이터 개수 확인
    if daily_closes.len() < 20 {
        warn!("시장 데이터 부족 (종목: {}, 날짜: {}) - 필요: 20, 실제: {}", 
              stock_code, date, daily_closes.len());
        return Ok(0.5);
    }
    
    // 당일 현재가 조회 (morning data의 last 값)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let current_price = morning_data.get_last_close().ok_or_else(|| {
        StockrsError::prediction(format!(
            "당일 현재가를 찾을 수 없습니다 (종목: {})",
            stock_code
        ))
    })?;
    
    // 20일 이동평균 계산
    let ma20 = daily_closes.iter().sum::<f64>() / daily_closes.len() as f64;
    
    // 시장 상황 점수: 현재가가 MA20 대비 어느 정도인지
    let regime_score = if current_price > ma20 {
        // 상승장: 0.5 ~ 1.0
        0.5 + 0.5 * ((current_price / ma20 - 1.0) * 10.0).clamp(0.0, 1.0)
    } else {
        // 하락장: 0.0 ~ 0.5
        0.5 * (current_price / ma20).clamp(0.0, 1.0)
    };
    
    debug!("day7_market_regime_score 결과: 현재가={}, MA20={} → 점수={}", 
           current_price, ma20, regime_score);
    Ok(regime_score)
}

/// 변동성 상황 분류 계산 (0~1 범위)
pub fn calculate_volatility_regime(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    debug!("계산 중: day7_volatility_regime (종목: {}, 날짜: {})", stock_code, date);
    
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.5);
    }
    
    // 20일 변동성: SQL로 최근 20일 고저종가를 조회 (현재 일자 제외)
    let table_name = if stock_code.starts_with('A') { stock_code.to_string() } else { format!("A{}", stock_code) };
    let query = format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 20",
        table_name
    );
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([date], |row| {
        Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?, row.get::<_, f64>(2)?))
    })?;
    let mut daily_volatilities: Vec<f64> = Vec::new();
    for r in rows {
        if let Ok((high, low, close)) = r {
            if close > 0.0 { daily_volatilities.push(((high - low) / close).max(0.0)); }
        }
    }
    
    // 데이터 개수 확인
    if daily_volatilities.len() < 20 {
        warn!("변동성 데이터 부족 (종목: {}, 날짜: {}) - 필요: 20, 실제: {}", 
              stock_code, date, daily_volatilities.len());
        return Ok(0.5);
    }
    
    // 당일 현재 변동성 계산 (morning data의 high, low, close 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let (today_high, today_low, today_close) = match (morning_data.get_max_high(), morning_data.get_min_low(), morning_data.get_last_close()) {
        (Some(high), Some(low), Some(close)) => (high, low, close),
        _ => {
            warn!("당일 변동성 데이터 부족 (종목: {}, 날짜: {})", stock_code, date);
            return Ok(0.5);
        }
    };
    
    if today_close <= 0.0 {
        warn!("당일 종가가 유효하지 않음 (종목: {}, 날짜: {})", stock_code, date);
        return Ok(0.5);
    }
    
    let current_vol = (today_high - today_low) / today_close;
    let avg_vol = daily_volatilities.iter().sum::<f64>() / daily_volatilities.len() as f64;
    
    let volatility_regime = if avg_vol > 0.0 {
        let vol_ratio = current_vol / avg_vol;
        if vol_ratio < 0.5 {
            0.0 // 낮은 변동성
        } else if vol_ratio > 1.5 {
            1.0 // 높은 변동성
        } else {
            0.5 // 보통 변동성
        }
    } else {
        0.5
    };
    
    debug!("day7_volatility_regime 결과: 현재변동성={}, 평균변동성={} → 상황={}", 
           current_vol, avg_vol, volatility_regime);
    Ok(volatility_regime)
}
