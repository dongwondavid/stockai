use crate::utility::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;
use super::utils::{get_morning_data, calculate_rsi};

// 새로운 특징들 (example/day4.rs에서 가져온 것들)

pub fn calculate_rsi_above_50(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 오전 5분봉만 사용하여 가변 기간 RSI 계산 (오전 캔들 수에 맞춤)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let closes = &morning_data.closes;

    if closes.len() < 2 {
        return Ok(0.0);
    }

    let period = (closes.len() - 1).min(14);
    let rsi = calculate_rsi(closes, period);

    Ok(if rsi >= 50.0 { 1.0 } else { 0.0 })
}

pub fn calculate_rsi_overbought(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // RSI 과매수 구간 (70 이상) 여부 확인 - 오전 5분봉 데이터 기반, 가변 기간
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let closes = &morning_data.closes;

    if closes.len() < 2 {
        return Ok(0.0);
    }

    let period = (closes.len() - 1).min(14);
    let rsi = calculate_rsi(closes, period);

    Ok(if rsi >= 70.0 { 1.0 } else { 0.0 })
}

pub fn calculate_rsi_oversold(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // RSI 과매도 구간 (30 이하) 여부 확인 - 오전 5분봉 데이터 기반, 가변 기간
    let morning_data = get_morning_data(db, stock_code, date)?;
    let closes = &morning_data.closes;

    if closes.len() < 2 {
        return Ok(0.0);
    }

    let period = (closes.len() - 1).min(14);
    let rsi = calculate_rsi(closes, period);

    Ok(if rsi <= 30.0 { 1.0 } else { 0.0 })
}

pub fn calculate_pos_vs_high_10d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 최근 10일 고점 대비 현재가 위치 비율
    // 현재가는 오전 5분봉 데이터를 사용
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let current_price = morning_data.get_last_close().ok_or_else(|| StockrsError::unsupported_feature(
        "calculate_pos_vs_high_10d".to_string(),
        "현재가 데이터가 필요합니다".to_string(),
    ))?;

    // 전일까지의 최근 10일 종가 사용 (현재일 제외)
    let table_name = stock_code;
    let query = format!(
        "SELECT close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 10",
        table_name
    );
    
    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([&date.parse::<i32>().unwrap_or(0)], |row| {
        Ok(row.get::<_, i32>(0)?)
    })?;
    
    let mut prices = Vec::new();
    for row in rows {
        prices.push(row? as f64);
    }
    
    // 가격을 시간순으로 정렬 (최신이 마지막)
    prices.reverse();
    
    if prices.len() < 10 {
        return Ok(0.0); // 데이터가 부족하면 0.0 반환
    }
    
    let high_10d = prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)); // 10일 고점
    
    if high_10d > 0.0 {
        // 현재가 / 10일 고점 비율
        Ok(current_price / high_10d)
    } else {
        Ok(0.0)
    }
}

pub fn calculate_consecutive_bull_count(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 연속 양봉 개수
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    if morning_data.closes.len() < 2 {
        return Err(StockrsError::unsupported_feature(
            "calculate_consecutive_bull_count".to_string(),
            "최소 2개의 가격 데이터가 필요합니다".to_string(),
        ));
    }
    
    let mut max_consecutive = 0;
    let mut current_consecutive = 0;
    
    for i in 1..morning_data.closes.len() {
        if morning_data.closes[i] > morning_data.closes[i - 1] {
            current_consecutive += 1;
            if current_consecutive > max_consecutive {
                max_consecutive = current_consecutive;
            }
        } else {
            current_consecutive = 0;
        }
    }
    
    Ok(max_consecutive as f64)
}

pub fn calculate_is_highest_volume_bull_candle(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 최고 거래량 양봉 여부 확인
    let morning_data = get_morning_data(db, stock_code, date)?;
    
    if morning_data.closes.len() < 2 || morning_data.volumes.len() < 2 {
        return Ok(0.0); // 데이터가 부족하면 최고 거래량이 아님
    }
    
    // 현재 캔들이 양봉인지 확인
    let current_close = morning_data.get_last_close().unwrap_or(0.0);
    let current_open = morning_data.get_last_open().unwrap_or(0.0);
    let is_bull_candle = current_close > current_open;
    
    if !is_bull_candle {
        return Ok(0.0); // 양봉이 아니면 0.0
    }
    
    // 현재 거래량이 최고인지 확인
    let current_volume = morning_data.get_current_volume().unwrap_or(0.0);
    let max_volume = morning_data.volumes.iter().fold(0.0_f64, |a, &b| a.max(b));
    
    // 현재 거래량이 최고 거래량이면 1.0, 아니면 0.0
    Ok(if current_volume >= max_volume { 1.0 } else { 0.0 })
}

pub fn calculate_high_volume_early_count(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 오전 고거래량 발생 횟수
    let morning_data = get_morning_data(db, stock_code, date)?;
    
    if morning_data.volumes.is_empty() {
        return Ok(0.0);
    }
    
    // 평균 거래량 계산
    let avg_volume = morning_data.get_avg_volume().unwrap_or(0.0);
    let high_volume_threshold = avg_volume * 1.5; // 평균의 1.5배를 고거래량으로 정의
    
    // 고거래량 발생 횟수 계산
    let high_volume_count = morning_data.volumes.iter()
        .filter(|&&volume| volume > high_volume_threshold)
        .count();
    
    Ok(high_volume_count as f64)
}

pub fn calculate_is_bullish_engulfing(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 상승 감싸기 패턴 확인
    let morning_data = get_morning_data(db, stock_code, date)?;
    
    if morning_data.closes.len() < 2 || morning_data.opens.len() < 2 {
        return Ok(0.0); // 데이터가 부족하면 패턴이 아님
    }
    
    // 현재 캔들 (양봉)
    let current_close = morning_data.get_last_close().unwrap_or(0.0);
    let current_open = morning_data.get_last_open().unwrap_or(0.0);
    let is_current_bull = current_close > current_open;
    
    if !is_current_bull {
        return Ok(0.0); // 현재 캔들이 양봉이 아니면 감싸기가 아님
    }
    
    // 이전 캔들 (음봉)
    let prev_close = morning_data.closes[morning_data.closes.len() - 2];
    let prev_open = morning_data.opens[morning_data.opens.len() - 2];
    let is_prev_bear = prev_close < prev_open;
    
    if !is_prev_bear {
        return Ok(0.0); // 이전 캔들이 음봉이 아니면 감싸기가 아님
    }
    
    // 감싸기 조건 확인: 현재 캔들이 이전 캔들을 완전히 감싸는지
    let is_engulfing = current_open < prev_close && current_close > prev_open;
    
    Ok(if is_engulfing { 1.0 } else { 0.0 })
}

pub fn calculate_is_bearish_engulfing(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 하락 감싸기 패턴 확인
    let morning_data = get_morning_data(db, stock_code, date)?;
    
    if morning_data.closes.len() < 2 || morning_data.opens.len() < 2 {
        return Ok(0.0); // 데이터가 부족하면 패턴이 아님
    }
    
    // 현재 캔들 (음봉)
    let current_close = morning_data.get_last_close().unwrap_or(0.0);
    let current_open = morning_data.get_last_open().unwrap_or(0.0);
    let is_current_bear = current_close < current_open;
    
    if !is_current_bear {
        return Ok(0.0); // 현재 캔들이 음봉이 아니면 감싸기가 아님
    }
    
    // 이전 캔들 (양봉)
    let prev_close = morning_data.closes[morning_data.closes.len() - 2];
    let prev_open = morning_data.opens[morning_data.opens.len() - 2];
    let is_prev_bull = prev_close > prev_open;
    
    if !is_prev_bull {
        return Ok(0.0); // 이전 캔들이 양봉이 아니면 감싸기가 아님
    }
    
    // 감싸기 조건 확인: 현재 캔들이 이전 캔들을 완전히 감싸는지
    let is_engulfing = current_open > prev_close && current_close < prev_open;
    
    Ok(if is_engulfing { 1.0 } else { 0.0 })
}

pub fn calculate_is_morning_star(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 샛별형 반등 패턴: 첫 번째 봉이 큰 음봉, 두 번째 봉이 작은 봉, 세 번째 봉이 큰 양봉
    let morning_data = get_morning_data(db, stock_code, date)?;
    
    if morning_data.closes.len() < 3 {
        return Ok(0.0);
    }
    
    // 최근 3개 캔들 사용
    let len = morning_data.closes.len();
    let first_open = morning_data.opens[len - 3];
    let first_close = morning_data.closes[len - 3];
    let second_open = morning_data.opens[len - 2];
    let second_close = morning_data.closes[len - 2];
    let third_open = morning_data.opens[len - 1];
    let third_close = morning_data.closes[len - 1];
    
    // 첫 번째 봉이 큰 음봉, 두 번째 봉이 작은 봉, 세 번째 봉이 큰 양봉
    if first_close < first_open
        && (second_close - second_open).abs() < (first_open - first_close) * 0.3
        && third_close > third_open
    {
        Ok(1.0)
    } else {
        Ok(0.0)
    }
}

pub fn calculate_is_evening_star(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 석별형 반락 패턴: 첫 번째 봉이 큰 양봉, 두 번째 봉이 작은 봉, 세 번째 봉이 큰 음봉
    let morning_data = get_morning_data(db, stock_code, date)?;
    
    if morning_data.closes.len() < 3 {
        return Ok(0.0);
    }
    
    // 최근 3개 캔들 사용
    let len = morning_data.closes.len();
    let first_open = morning_data.opens[len - 3];
    let first_close = morning_data.closes[len - 3];
    let second_open = morning_data.opens[len - 2];
    let second_close = morning_data.closes[len - 2];
    let third_open = morning_data.opens[len - 1];
    let third_close = morning_data.closes[len - 1];
    
    // 첫 번째 봉이 큰 양봉, 두 번째 봉이 작은 봉, 세 번째 봉이 큰 음봉
    if first_close > first_open
        && (second_close - second_open).abs() < (first_close - first_open) * 0.3
        && third_close < third_open
    {
        Ok(1.0)
    } else {
        Ok(0.0)
    }
}

pub fn calculate_is_hammer(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 망치형 반등 신호 패턴
    let morning_data = get_morning_data(db, stock_code, date)?;
    
    if morning_data.closes.is_empty() {
        return Ok(0.0);
    }
    
    let close = morning_data.get_last_close().unwrap_or(0.0);
    let open = morning_data.get_last_open().unwrap_or(0.0);
    let high = morning_data.get_max_high().unwrap_or(0.0);
    let low = morning_data.get_min_low().unwrap_or(0.0);
    
    let body_size = (close - open).abs();
    let lower_shadow = if close > open {
        open - low
    } else {
        close - low
    };
    let upper_shadow = if close > open {
        high - close
    } else {
        high - open
    };
    
    // 몸통이 작고, 아래 그림자가 길며, 위 그림자가 짧은 경우
    if lower_shadow > body_size * 2.0 && upper_shadow < body_size * 0.5 {
        Ok(1.0)
    } else {
        Ok(0.0)
    }
}

pub fn calculate_vwap_support_check(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // D+1 종가가 D+1 VWAP 이상인지 여부
    let morning_data = get_morning_data(db, stock_code, date)?;
    
    if morning_data.closes.is_empty() {
        return Ok(0.0);
    }
    
    let vwap = morning_data.get_vwap().unwrap_or(0.0);
    let close_price = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_price >= vwap {
        Ok(1.0)
    } else {
        Ok(0.0)
    }
}

pub fn calculate_vwap_vs_high(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 고점과 VWAP 간 괴리율
    let morning_data = get_morning_data(db, stock_code, date)?;
    
    if morning_data.closes.is_empty() {
        return Ok(0.0);
    }
    
    let vwap = morning_data.get_vwap().unwrap_or(0.0);
    let high = morning_data.get_max_high().unwrap_or(0.0);
    
    let vwap_vs_high = if vwap > 0.0 {
        (high - vwap) / vwap
    } else {
        0.0
    };
    
    Ok(vwap_vs_high)
}

pub fn calculate_vwap_vs_low(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 저점과 VWAP 간 괴리율
    let morning_data = get_morning_data(db, stock_code, date)?;
    
    if morning_data.closes.is_empty() {
        return Ok(0.0);
    }
    
    let vwap = morning_data.get_vwap().unwrap_or(0.0);
    let low = morning_data.get_min_low().unwrap_or(0.0);
    
    let vwap_vs_low = if vwap > 0.0 {
        (vwap - low) / vwap
    } else {
        0.0
    };
    
    Ok(vwap_vs_low)
}

pub fn calculate_bollinger_band_width(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 볼린저 밴드 폭 (상단-하단/중심선)
    let morning_data = get_morning_data(db, stock_code, date)?;
    // 오전 데이터는 보통 6개 이므로 가변 윈도우(최소 5)
    if morning_data.closes.len() < 5 {
        return Ok(0.0);
    }

    // 이동평균과 표준편차 계산 (가용 구간 전체)
    let prices: Vec<f64> = morning_data.closes.iter().copied().collect();
    let sma = prices.iter().sum::<f64>() / prices.len() as f64;

    let variance = prices.iter().map(|&p| (p - sma).powi(2)).sum::<f64>() / prices.len() as f64;
    let std_dev = variance.sqrt();

    let upper_band = sma + (2.0 * std_dev);
    let lower_band = sma - (2.0 * std_dev);

    let band_width = if sma > 0.0 {
        (upper_band - lower_band) / sma
    } else {
        0.0
    };

    Ok(band_width)
}

pub fn calculate_bollinger_position(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 현재가의 밴드 내 상대 위치 (0~1)
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 5 {
        return Ok(0.5);
    }

    let current_price = morning_data.get_last_close().unwrap_or(0.0);

    // 이동평균과 표준편차 계산 (가용 구간 전체)
    let prices: Vec<f64> = morning_data.closes.iter().copied().collect();
    let sma = prices.iter().sum::<f64>() / prices.len() as f64;

    let variance = prices.iter().map(|&p| (p - sma).powi(2)).sum::<f64>() / prices.len() as f64;
    let std_dev = variance.sqrt();

    let upper_band = sma + (2.0 * std_dev);
    let lower_band = sma - (2.0 * std_dev);

    let position = if upper_band > lower_band {
        (current_price - lower_band) / (upper_band - lower_band)
    } else {
        0.5
    };

    Ok(position)
}

pub fn calculate_is_breaking_upper_band(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // 상단 밴드 돌파 여부
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 5 {
        return Ok(0.0);
    }

    let current_price = morning_data.get_last_close().unwrap_or(0.0);

    // 이동평균과 표준편차 계산 (가용 구간 전체)
    let prices: Vec<f64> = morning_data.closes.iter().copied().collect();
    let sma = prices.iter().sum::<f64>() / prices.len() as f64;

    let variance = prices.iter().map(|&p| (p - sma).powi(2)).sum::<f64>() / prices.len() as f64;
    let std_dev = variance.sqrt();

    let upper_band = sma + (2.0 * std_dev);

    let is_breaking_upper = if current_price > upper_band { 1.0 } else { 0.0 };

    Ok(is_breaking_upper)
} 