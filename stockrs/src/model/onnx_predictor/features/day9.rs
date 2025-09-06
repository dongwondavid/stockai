use super::utils::{get_morning_data, get_previous_trading_day, is_first_trading_day};
use crate::utility::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;
use chrono::{Duration, NaiveDate};

/// 테이블명 생성 시 A 접두사 중복 방지
fn get_table_name(stock_code: &str) -> String {
    if stock_code.starts_with('A') {
        stock_code.to_string()
    } else {
        format!("A{}", stock_code)
    }
}

/// 일봉 데이터 구조체
struct DailyData {
    open: f64,
    close: f64,
    high: f64,
    low: f64,
    #[allow(dead_code)]
    volume: f64,
}

impl DailyData {
    fn new(open: f64, close: f64, high: f64, low: f64, volume: f64) -> Self {
        Self { open, close, high, low, volume }
    }

    fn get_open(&self) -> Option<f64> { Some(self.open) }
    fn get_close(&self) -> Option<f64> { Some(self.close) }
    fn get_high(&self) -> Option<f64> { Some(self.high) }
    fn get_low(&self) -> Option<f64> { Some(self.low) }
    #[allow(dead_code)]
    fn get_volume(&self) -> Option<f64> { Some(self.volume) }
}

/// 일봉 데이터 조회 함수
fn get_daily_data(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<DailyData> {
    let table_name = get_table_name(stock_code);
    
    let (open, close, high, low, volume): (f64, f64, f64, f64, f64) = daily_db.query_row(
        &format!(
            "SELECT open, close, high, low, volume FROM \"{}\" WHERE date = ?",
            table_name
        ),
        rusqlite::params![date],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )?;

    Ok(DailyData::new(open, close, high, low, volume))
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

/// RSI 계산 함수
fn calculate_rsi(prices: &[f64], period: usize) -> f64 {
    if prices.len() < period + 1 {
        return 50.0; // 기본값
    }
    
    let mut gains = Vec::new();
    let mut losses = Vec::new();
    
    for i in 1..prices.len() {
        let change = prices[i] - prices[i-1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }
    
    let avg_gain = gains.iter().sum::<f64>() / gains.len() as f64;
    let avg_loss = losses.iter().sum::<f64>() / losses.len() as f64;
    
    if avg_loss == 0.0 {
        return 100.0;
    }
    
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

/// day9_prevday_long_candle_strength: 전일 장대 캔들 강도 (연속형)
pub fn calculate_prevday_long_candle_strength(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.5); // 중립값
    }

    // 전일 거래일 가져오기
    let prev_date = get_previous_trading_day(trading_dates, date)?;
    
    // 전일 데이터 조회
    let prev_data = get_daily_data(daily_db, stock_code, &prev_date)?;
    let prev_open = prev_data.get_open()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_prevday_long_candle_strength".to_string(),
            "전일 시가 데이터가 필요합니다".to_string(),
        ))?;
    let prev_close = prev_data.get_close()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_prevday_long_candle_strength".to_string(),
            "전일 종가 데이터가 필요합니다".to_string(),
        ))?;
    let prev_high = prev_data.get_high()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_prevday_long_candle_strength".to_string(),
            "전일 고가 데이터가 필요합니다".to_string(),
        ))?;
    let prev_low = prev_data.get_low()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_prevday_long_candle_strength".to_string(),
            "전일 저가 데이터가 필요합니다".to_string(),
        ))?;
    
    let body_size = (prev_close - prev_open).abs();
    let total_range = prev_high - prev_low;
    
    if total_range > 0.0 {
        Ok(body_size / total_range)
    } else {
        Ok(0.5) // 중립값
    }
}

/// day9_high_volume_gain_experience_score: 고거래량 상승 경험 점수 (연속형)
pub fn calculate_high_volume_gain_experience_score(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0); // 첫 거래일이면 과거 경험 없음
    }

    // 6개월 전 데이터 조회
    let six_months_ago = NaiveDate::parse_from_str(date, "%Y%m%d")
        .map_err(|e| StockrsError::parsing("날짜 파싱".to_string(), e.to_string()))?
        - Duration::days(180);
    
    let six_months_ago_str = six_months_ago.format("%Y%m%d").to_string();
    let table_name = get_table_name(stock_code);
    
    // 6개월 내 1000억 이상 거래대금 + 10% 이상 상승 경험 조회
    let count: i32 = daily_db
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM {} WHERE date >= ? AND date < ? 
                 AND volume * close >= 1000_0000_0000 AND (close - open) / open >= 0.1",
                table_name
            ),
            rusqlite::params![six_months_ago_str, date],
            |row| row.get(0),
        )
        .map_err(|e| StockrsError::database("고거래량 상승 경험 조회".to_string(), e.to_string()))?;
    
    // 180일로 정규화
    Ok(count as f64 / 180.0)
}

/// day9_short_macd_cross_strength: 단기 MACD 크로스 강도 (연속형)
pub fn calculate_short_macd_cross_strength(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 4 {
        return Ok(0.5); // 중립값
    }

    // EMA 3, 6 계산
    let ema3 = calculate_ema(&morning_data.closes, 3);
    let ema6 = calculate_ema(&morning_data.closes, 6);
    let macd = ema3 - ema6;

    // 과거 MACD 최대값을 저장하여 정규화 (실제로는 전역 상태나 DB에 저장 필요)
    let max_historical_macd = 2.0; // 예시 값, 실제로는 동적으로 계산 필요
    
    if max_historical_macd > 0.0 {
        Ok((macd.abs() / max_historical_macd).min(1.0))
    } else {
        Ok(0.5) // 중립값
    }
}

/// day9_rsi_overbought_score: RSI 과매수 점수 (연속형)
pub fn calculate_rsi_overbought_score(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;
    let closes = &morning_data.closes;

    if closes.len() < 2 {
        return Ok(0.0);
    }

    let period = (closes.len() - 1).min(14);
    let rsi = calculate_rsi(closes, period);
    
    if rsi > 70.0 {
        Ok(((rsi - 70.0) / 30.0).min(1.0))
    } else {
        Ok(0.0)
    }
}

/// day9_rsi_oversold_score: RSI 과매도 점수 (연속형)
pub fn calculate_rsi_oversold_score(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;
    let closes = &morning_data.closes;

    if closes.len() < 2 {
        return Ok(0.0);
    }

    let period = (closes.len() - 1).min(14);
    let rsi = calculate_rsi(closes, period);
    
    if rsi < 30.0 {
        Ok(((30.0 - rsi) / 30.0).min(1.0))
    } else {
        Ok(0.0)
    }
}

/// day9_long_bull_candle_strength: 긴 양봉 강도 (연속형)
pub fn calculate_long_bull_candle_strength(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match morning_data.get_last_candle() {
        Some((close, open, high, low)) => {
            if close <= open {
                return Ok(0.0); // 양봉이 아님
            }
            
            let body_size = close - open;
            let total_range = high - low;
            
            if total_range > 0.0 {
                Ok((body_size / total_range).min(1.0))
            } else {
                Ok(0.0)
            }
        }
        _ => Ok(0.0),
    }
}

/// day9_bullish_engulfing_strength: 상승 감싸기 패턴 강도 (연속형)
pub fn calculate_bullish_engulfing_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;

    if morning_data.opens.len() < 2 || morning_data.closes.len() < 2 {
        return Ok(0.0); // 데이터 부족 시 0.0
    }

    let current_open = morning_data.opens[morning_data.opens.len() - 1];
    let current_close = morning_data.closes[morning_data.closes.len() - 1];
    let prev_open = morning_data.opens[morning_data.opens.len() - 2];
    let prev_close = morning_data.closes[morning_data.closes.len() - 2];

    // 상승 감싸기 패턴 확인
    if current_close > current_open && // 현재 양봉
       prev_close < prev_open && // 이전 음봉
       current_open < prev_close && // 현재 시가 < 이전 종가
       current_close > prev_open { // 현재 종가 > 이전 시가
        
        // 감싸기 강도 계산: 현재 캔들 크기 / 이전 캔들 크기
        let current_body = (current_close - current_open).abs();
        let prev_body = (prev_close - prev_open).abs();
        
        if prev_body > 0.0 {
            Ok((current_body / prev_body).min(1.0))
        } else {
            Ok(0.8) // 기본 강도
        }
    } else {
        Ok(0.0)
    }
}

/// day9_bearish_engulfing_strength: 하락 감싸기 패턴 강도 (연속형)
pub fn calculate_bearish_engulfing_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;

    if morning_data.opens.len() < 2 || morning_data.closes.len() < 2 {
        return Ok(0.0); // 데이터 부족 시 0.0
    }

    let current_open = morning_data.opens[morning_data.opens.len() - 1];
    let current_close = morning_data.closes[morning_data.closes.len() - 1];
    let prev_open = morning_data.opens[morning_data.opens.len() - 2];
    let prev_close = morning_data.closes[morning_data.closes.len() - 2];

    // 하락 감싸기 패턴 확인
    if current_close < current_open && // 현재 음봉
       prev_close > prev_open && // 이전 양봉
       current_open > prev_close && // 현재 시가 > 이전 종가
       current_close < prev_open { // 현재 종가 < 이전 시가
        
        // 감싸기 강도 계산: 현재 캔들 크기 / 이전 캔들 크기
        let current_body = (current_close - current_open).abs();
        let prev_body = (prev_close - prev_open).abs();
        
        if prev_body > 0.0 {
            Ok((current_body / prev_body).min(1.0))
        } else {
            Ok(0.8) // 기본 강도
        }
    } else {
        Ok(0.0)
    }
}

/// day9_morning_star_strength: 모닝스타 패턴 강도 (연속형)
pub fn calculate_morning_star_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;

    if morning_data.opens.len() < 3 || morning_data.closes.len() < 3 {
        return Ok(0.0); // 데이터 부족 시 0.0
    }

    let first_open = morning_data.opens[morning_data.opens.len() - 3];
    let first_close = morning_data.closes[morning_data.closes.len() - 3];
    let second_open = morning_data.opens[morning_data.opens.len() - 2];
    let second_close = morning_data.closes[morning_data.closes.len() - 2];
    let third_open = morning_data.opens[morning_data.opens.len() - 1];
    let third_close = morning_data.closes[morning_data.closes.len() - 1];

    // 모닝스타 패턴 확인: 음봉 → 작은 캔들 → 양봉
    if first_close < first_open && // 첫 번째 음봉
       (second_close - second_open).abs() < (first_close - first_open).abs() * 0.3 && // 두 번째 작은 캔들
       third_close > third_open { // 세 번째 양봉
        
        // 패턴 완성도 계산
        let first_body = (first_close - first_open).abs();
        let second_body = (second_close - second_open).abs();
        let third_body = (third_close - third_open).abs();
        
        let total_range = first_body + second_body + third_body;
        if total_range > 0.0 {
            // 세 번째 양봉의 강도가 클수록 높은 점수
            Ok((third_body / total_range).min(1.0))
        } else {
            Ok(0.8) // 기본 강도
        }
    } else {
        Ok(0.0)
    }
}

/// day9_evening_star_strength: 이브닝스타 패턴 강도 (연속형)
pub fn calculate_evening_star_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;

    if morning_data.opens.len() < 3 || morning_data.closes.len() < 3 {
        return Ok(0.0); // 데이터 부족 시 0.0
    }

    let first_open = morning_data.opens[morning_data.opens.len() - 3];
    let first_close = morning_data.closes[morning_data.closes.len() - 3];
    let second_open = morning_data.opens[morning_data.opens.len() - 2];
    let second_close = morning_data.closes[morning_data.closes.len() - 2];
    let third_open = morning_data.opens[morning_data.opens.len() - 1];
    let third_close = morning_data.closes[morning_data.closes.len() - 1];

    // 이브닝스타 패턴 확인: 양봉 → 작은 캔들 → 음봉
    if first_close > first_open && // 첫 번째 양봉
       (second_close - second_open).abs() < (first_close - first_open).abs() * 0.3 && // 두 번째 작은 캔들
       third_close < third_open { // 세 번째 음봉
        
        // 패턴 완성도 계산
        let first_body = (first_close - first_open).abs();
        let second_body = (second_close - second_open).abs();
        let third_body = (third_close - third_open).abs();
        
        let total_range = first_body + second_body + third_body;
        if total_range > 0.0 {
            // 세 번째 음봉의 강도가 클수록 높은 점수
            Ok((third_body / total_range).min(1.0))
        } else {
            Ok(0.8) // 기본 강도
        }
    } else {
        Ok(0.0)
    }
}

/// day9_hammer_strength: 망치 패턴 강도 (연속형)
pub fn calculate_hammer_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;

    match morning_data.get_last_candle() {
        Some((close, open, high, low)) => {
            if high == low {
                return Ok(0.0);
            }
            
            let body_size = (close - open).abs();
            let total_range = high - low;
            let upper_shadow = high - open.max(close);
            let lower_shadow = open.min(close) - low;
            
            // 망치 패턴 조건: 아래 그림자가 길고, 몸통이 작고, 위 그림자가 짧음
            if lower_shadow > body_size * 2.0 && // 아래 그림자가 몸통의 2배 이상
               body_size < total_range * 0.3 && // 몸통이 전체 범위의 30% 미만
               upper_shadow < body_size * 0.5 { // 위 그림자가 몸통의 50% 미만
                
                // 망치 강도 계산: 아래 그림자 비율
                Ok((lower_shadow / total_range).min(1.0))
            } else {
                Ok(0.0)
            }
        }
        _ => Ok(0.0),
    }
}

/// day9_consecutive_positive_candles_score: 연속 양봉 점수 (연속형)
pub fn calculate_consecutive_positive_candles_score(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 2 {
        return Ok(0.0);
    }

    let mut max_consecutive = 0;
    let mut current_consecutive = 0;
    
    for i in 1..morning_data.closes.len() {
        if morning_data.closes[i] > morning_data.closes[i-1] {
            current_consecutive += 1;
            max_consecutive = max_consecutive.max(current_consecutive);
        } else {
            current_consecutive = 0;
        }
    }
    
    let max_possible = morning_data.closes.len() - 1;
    if max_possible > 0 {
        Ok((max_consecutive as f64 / max_possible as f64).min(1.0))
    } else {
        Ok(0.0)
    }
}

/// day9_highest_volume_bull_strength: 최고 거래량 양봉 강도 (연속형)
pub fn calculate_highest_volume_bull_strength(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 2 || morning_data.volumes.len() < 2 {
        return Ok(0.0);
    }

    let (close, open, high, low) = morning_data.get_last_candle()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_highest_volume_bull_strength".to_string(),
            "캔들 데이터가 필요합니다".to_string(),
        ))?;
    
    let volume = morning_data.volumes.last()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_highest_volume_bull_strength".to_string(),
            "거래량 데이터가 필요합니다".to_string(),
        ))?;
    
    // 양봉이 아니면 0.0 반환
    if close <= open {
        return Ok(0.0);
    }
    
    // 거래량 비율 계산
    let max_volume = morning_data.volumes.iter().fold(0.0_f64, |a, &b| a.max(b));
    let volume_ratio = if max_volume > 0.0 { volume / max_volume } else { 0.0 };
    
    // 양봉 강도 계산
    let body_size = close - open;
    let total_range = high - low;
    let bull_strength = if total_range > 0.0 { body_size / total_range } else { 0.0 };
    
    Ok((volume_ratio * bull_strength).min(1.0))
}
