use crate::utility::errors::{StockrsResult,StockrsError};
use super::utils::{
    get_morning_data,
    is_first_trading_day,
    get_prev_daily_data_opt,
};
use rusqlite::Connection;
use chrono::{NaiveDate, Duration};

/// 테이블명 생성 시 A 접두사 중복 방지
fn get_table_name(stock_code: &str) -> String {
    if stock_code.starts_with('A') {
        stock_code.to_string()
    } else {
        format!("A{}", stock_code)
    }
}

/// Day8 특징: 전일 상승률 연속형 변수
/// 기존 day2_prev_gain_over_3 (0/1)을 연속값으로 확장
/// 
/// # Arguments
/// * `daily_db` - 일별 데이터베이스 연결
/// * `stock_code` - 종목 코드
/// * `date` - 날짜
/// * `trading_dates` - 거래일 목록
/// 
/// # Returns
/// * `StockrsResult<f64>` - 전일 상승률 (0~1 범위로 정규화)
///   - 0.0: 전일 하락 10% 이상
///   - 0.5: 전일 변동 없음
///   - 1.0: 전일 상승 15% 이상
pub fn calculate_prev_gain_ratio(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.5); // 첫 거래일이면 중립값 반환
    }
    
    // 전일 데이터 조회 (없으면 중립값)
    let prev_opt = get_prev_daily_data_opt(daily_db, stock_code, date, trading_dates)?;
    let (open, close) = match prev_opt {
        Some(d) => (d.get_open().unwrap_or(0.0), d.get_close().unwrap_or(0.0)),
        None => return Ok(0.5),
    };

    if open == 0.0 {
        return Ok(0.5); // 시가가 0이면 중립값 반환
    }

    // 전일 상승률 계산 (백분율)
    let gain_ratio = (close - open) / open * 100.0;

    // [-10%, +15%] 범위를 [0, 1]로 정규화
    let normalized_value = if gain_ratio <= -10.0 {
        0.0
    } else if gain_ratio >= 15.0 {
        1.0
    } else {
        (gain_ratio + 10.0) / 25.0 // [-10, 15] → [0, 1]
    };

    Ok(normalized_value)
}

/// Day8 특징: 6개월 고점 돌파 강도
/// 기존 day3_breaks_6month_high (0/1)을 연속값으로 확장
/// 
/// # Arguments
/// * `daily_db` - 일별 데이터베이스 연결
/// * `stock_code` - 종목 코드
/// * `date` - 날짜
/// * `trading_dates` - 거래일 목록
/// 
/// # Returns
/// * `StockrsResult<f64>` - 6개월 고점 대비 현재 위치 (0~1 범위)
///   - 0.0: 6개월 고점 대비 0% (고점과 동일)
///   - 0.5: 6개월 고점 대비 60% 위치
///   - 1.0: 6개월 고점 돌파 (120% 이상)
pub fn calculate_breaks_6month_high_ratio(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.5); // 첫 거래일이면 중립값 반환
    }

    // 날짜 파싱 (YYYYMMDD 형식)
    let year = date[..4].parse::<i32>().map_err(|_| {
        StockrsError::prediction(format!("잘못된 연도 형식: {}", date))
    })?;
    let month = date[4..6].parse::<u32>().map_err(|_| {
        StockrsError::prediction(format!("잘못된 월 형식: {}", date))
    })?;
    let day = date[6..8].parse::<u32>().map_err(|_| {
        StockrsError::prediction(format!("잘못된 일 형식: {}", date))
    })?;

    let target_date = NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| {
        StockrsError::prediction(format!("잘못된 날짜 형식: {}", date))
    })?;

    // 6개월 전 날짜 계산 (주식 시장이 열린 날 기준으로 근사)
    let six_months_ago = target_date - Duration::days(180);

    // 날짜 형식을 TEXT 형식으로 변환 (일봉 DB 형식에 맞춤)
    let target_date_str = target_date.format("%Y%m%d").to_string();
    let six_months_ago_str = six_months_ago.format("%Y%m%d").to_string();

    // 테이블명 (일봉 DB는 A 접두사 포함)
    let table_name = get_table_name(stock_code);

    // 6개월 내 최고가 조회
    let six_month_high: f64 = daily_db
        .query_row(
            &format!(
                "SELECT MAX(high) FROM \"{}\" WHERE date >= ? AND date < ?",
                table_name
            ),
            rusqlite::params![six_months_ago_str, target_date_str],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    if six_month_high <= 0.0 {
        return Ok(0.5); // 6개월 고점 데이터가 없으면 중립값 반환
    }

    // 현재가 조회 (get_morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let current_price = morning_data.get_last_close().unwrap_or(0.0);
    
    if current_price == 0.0 {
        return Ok(0.5); // 현재가 데이터가 없으면 중립값 반환
    }

    // 현재가와 6개월 고점 비교
    let ratio = current_price / six_month_high;

    // [0, 1.2] 범위를 [0, 1]로 정규화
    let normalized_value = if ratio <= 0.0 {
        0.0
    } else if ratio >= 1.2 {
        1.0
    } else {
        ratio / 1.2
    };

    Ok(normalized_value)
}

/// Day8 특징: 단기 모멘텀 지표 (5분봉 6개 기반)
/// 5분봉 6개 데이터만으로는 전통적인 MACD 계산이 어려우므로
/// 단기 모멘텀과 가격 변화율을 기반으로 한 지표로 대체
/// 
/// # Arguments
/// * `db_5min` - 5분봉 데이터베이스 연결
/// * `stock_code` - 종목 코드
/// * `date` - 날짜
/// 
/// # Returns
/// * `StockrsResult<f64>` - 단기 모멘텀 지표 (0~1 범위로 정규화)
///   - 0.0: 강한 하락 모멘텀
///   - 0.5: 중립 (변화 없음)
///   - 1.0: 강한 상승 모멘텀
pub fn calculate_macd_histogram_value(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // get_morning_data를 통해 5분봉 데이터 조회
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    if morning_data.closes.len() < 3 {
        return Ok(0.5); // 최소 3개 데이터 필요
    }

    let closes = &morning_data.closes;
    
    // 1. 단기 가격 변화율 계산 (첫봉 vs 마지막봉)
    let first_close = closes[0];
    let last_close = closes[closes.len() - 1];
    let price_change_ratio = (last_close - first_close) / first_close;
    
    // 2. 연속성 점수 계산 (상승/하락 연속성)
    let mut consecutive_up = 0;
    let mut consecutive_down = 0;
    let mut max_consecutive_up = 0;
    let mut max_consecutive_down = 0;
    
    for i in 1..closes.len() {
        if closes[i] > closes[i-1] {
            consecutive_up += 1;
            consecutive_down = 0;
            max_consecutive_up = max_consecutive_up.max(consecutive_up);
        } else if closes[i] < closes[i-1] {
            consecutive_down += 1;
            consecutive_up = 0;
            max_consecutive_down = max_consecutive_down.max(consecutive_down);
        } else {
            consecutive_up = 0;
            consecutive_down = 0;
        }
    }
    
    // 3. 모멘텀 강도 계산 (가격 변화 + 연속성)
    let momentum_score = if price_change_ratio > 0.0 {
        // 상승 모멘텀
        let base_score = (price_change_ratio * 100.0).min(5.0) / 5.0; // 0~5% → 0~1
        let continuity_bonus = (max_consecutive_up as f64) / 6.0; // 연속 상승 보너스
        (base_score + continuity_bonus) / 2.0
    } else {
        // 하락 모멘텀
        let base_score = 1.0 - ((-price_change_ratio * 100.0).min(5.0) / 5.0); // 0~5% → 1~0
        let continuity_penalty = (max_consecutive_down as f64) / 6.0; // 연속 하락 페널티
        (base_score - continuity_penalty).max(0.0)
    };
    
    // 4. 변동성 가중치 적용 (가격 변동이 클수록 신뢰도 높음)
    let volatility = closes.windows(2)
        .map(|w| (w[1] - w[0]).abs() / w[0])
        .sum::<f64>() / (closes.len() - 1) as f64;
    
    let volatility_weight = (volatility * 1000.0).min(1.0); // 변동성 가중치
    
    // 5. 최종 점수 계산
    let final_score = if volatility_weight < 0.1 {
        0.5 // 변동성이 너무 작으면 중립값
    } else {
        momentum_score * volatility_weight + 0.5 * (1.0 - volatility_weight)
    };
    
    Ok(final_score.max(0.0).min(1.0))
}

/// Day8 특징: 외국인 비율 변화 연속형 변수
/// 기존 이진형 외국인 비율 지표를 연속값으로 확장
/// 
/// # Arguments
/// * `daily_db` - 일별 데이터베이스 연결
/// * `stock_code` - 종목 코드
/// * `date` - 날짜
/// * `trading_dates` - 거래일 목록
/// 
/// # Returns
/// * `StockrsResult<f64>` - 외국인 비율 변화 (0~1 범위로 정규화)
///   - 0.0: 외국인 비율 급감 (-5% 이상)
///   - 0.5: 외국인 비율 변화 없음
///   - 1.0: 외국인 비율 급증 (+5% 이상)
pub fn calculate_foreign_ratio_change(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.5); // 첫 거래일이면 중립값 반환
    }
    
    let table_name = get_table_name(stock_code);
    
    // 전일과 당일 외국인 비율 조회 (데이터 누수 방지)
    let ratios: Vec<f64> = daily_db
        .prepare(&format!("SELECT 외국인현보유비율 FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 2", table_name))?
        .query_map(rusqlite::params![date], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    if ratios.len() < 2 {
        return Ok(0.5); // 충분한 데이터가 없으면 중립값 반환
    }

    let current_ratio = ratios[0];
    let prev_ratio = ratios[1];

    if prev_ratio == 0.0 {
        return Ok(0.5); // 전일 외국인 비율이 0이면 중립값 반환
    }

    // 외국인 비율 변화율 계산 (퍼센트 단위이므로 100으로 나눔)
    let change_ratio = (current_ratio - prev_ratio) / 100.0;

    Ok(change_ratio)
}

/// Day8 특징: 연속 양봉 강도 연속형 변수
/// 기존 이진형 연속 양봉 지표를 연속값으로 확장
/// 
/// # Arguments
/// * `db_5min` - 5분 데이터베이스 연결
/// * `stock_code` - 종목 코드
/// * `date` - 날짜
/// 
/// # Returns
/// * `StockrsResult<f64>` - 연속 양봉 강도 (0~1 범위로 정규화)
///   - 0.0: 연속 음봉 (강한 하락)
///   - 0.5: 중립 (상하락 혼재)
///   - 1.0: 연속 양봉 (강한 상승)
pub fn calculate_consecutive_bull_candle_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // get_morning_data를 통해 5분봉 데이터 조회
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    if morning_data.closes.len() < 2 {
        return Ok(0.5); // 충분한 데이터가 없으면 중립값 반환
    }

    // 각 캔들의 상승/하락 여부 판단
    let mut bull_count = 0;
    let mut bear_count = 0;
    let mut total_strength = 0.0;

    for i in 0..morning_data.closes.len() {
        let open = morning_data.opens[i];
        let close = morning_data.closes[i];
        
        if open == 0.0 {
            continue; // 시가가 0이면 건너뛰기
        }

        let change_ratio = (close - open) / open;
        
        if change_ratio > 0.0 {
            bull_count += 1;
            total_strength += change_ratio;
        } else if change_ratio < 0.0 {
            bear_count += 1;
            total_strength += change_ratio.abs();
        }
    }

    if bull_count == 0 && bear_count == 0 {
        return Ok(0.5); // 변화가 없으면 중립값 반환
    }

    // 연속 양봉 강도 계산
    let total_candles = morning_data.closes.len() as f64;
    let bull_ratio = bull_count as f64 / total_candles;
    let average_strength = total_strength / total_candles;

    // [0, 1] 범위로 정규화 (양봉 비율과 평균 강도 조합)
    let normalized_value = (bull_ratio + average_strength) / 2.0;
    
    // 0~1 범위로 클리핑
    Ok(normalized_value.max(0.0).min(1.0))
}

/// Day8 특징: 가격 경계 근접도 연속형 변수
/// 기존 이진형 가격 경계 지표를 연속값으로 확장
/// 
/// # Arguments
/// * `db_5min` - 5분 데이터베이스 연결
/// * `stock_code` - 종목 코드
/// * `date` - 날짜
/// 
/// # Returns
/// * `StockrsResult<f64>` - 가격 경계 근접도 (0~1 범위로 정규화)
///   - 0.0: 가격 경계에서 멀리 떨어짐
///   - 0.5: 가격 경계 근처
///   - 1.0: 가격 경계에 매우 근접
pub fn calculate_price_boundary_proximity(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // get_morning_data를 통해 5분봉 데이터 조회
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    if morning_data.highs.len() < 2 {
        return Ok(0.5); // 충분한 데이터가 없으면 중립값 반환
    }

    // 당일 고가와 저가 계산
    let day_high = morning_data.get_max_high().unwrap_or(0.0);
    let day_low = morning_data.get_min_low().unwrap_or(0.0);
    
    if day_high <= day_low {
        return Ok(0.5); // 고가와 저가가 같으면 중립값 반환
    }

    // 현재가 (최신 종가)
    let current_price = morning_data.get_last_close().unwrap_or(0.0);
    if current_price == 0.0 {
        return Ok(0.5);
    }

    // 가격 범위 계산
    let price_range = day_high - day_low;

    // 현재가가 고가/저가 중 어느 쪽에 가까운지 계산
    let high_proximity = 1.0 - ((day_high - current_price) / price_range).abs();
    let low_proximity = 1.0 - ((current_price - day_low) / price_range).abs();

    // 가장 가까운 경계의 근접도 반환
    let proximity = high_proximity.max(low_proximity);
    
    Ok(proximity)
}

/// Day8 특징: 시가총액 규모 연속형 변수
/// 기존 이진형 시가총액 지표를 연속값으로 확장
/// 
/// # Arguments
/// * `daily_db` - 일별 데이터베이스 연결
/// * `stock_code` - 종목 코드
/// * `date` - 날짜
/// 
/// # Returns
/// * `StockrsResult<f64>` - 시가총액 규모 (0~1 범위로 정규화)
///   - 0.0: 소형주 (1000억 미만)
///   - 0.5: 중형주 (1조~5조)
///   - 1.0: 대형주 (10조 이상)
pub fn calculate_market_cap_scale(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    let table_name = get_table_name(stock_code);
    
    // 시가총액 계산: 상장주식수 * 종가
    let (shares, close_price): (i64, i32) = daily_db
        .query_row(
            &format!(
                "SELECT 상장주식수, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1",
                table_name
            ),
            rusqlite::params![date],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0));
    
    let market_cap = shares as f64 * close_price as f64;

    if market_cap == 0.0 {
        return Ok(0.5); // 시가총액이 0이면 중립값 반환
    }

    // 시가총액을 조 단위로 변환
    let market_cap_trillion = market_cap / 1_000_000_000_000.0;

    // [0.1조, 20조] 범위를 [0, 1]로 정규화
    let normalized_value = if market_cap_trillion <= 0.1 {
        0.0 // 1000억 미만
    } else if market_cap_trillion >= 20.0 {
        1.0 // 20조 이상
    } else {
        // 0.1조 ~ 20조 범위를 0~1로 정규화
        (market_cap_trillion - 0.1) / 19.9
    };

    Ok(normalized_value)
}

/// Day8 특징: VWAP 지지 강도 연속형 변수
/// 기존 이진형 VWAP 지지 지표를 연속값으로 확장
/// 
/// # Arguments
/// * `db_5min` - 5분 데이터베이스 연결
/// * `stock_code` - 종목 코드
/// * `date` - 날짜
/// 
/// # Returns
/// * `StockrsResult<f64>` - VWAP 지지 강도 (0~1 범위로 정규화)
///   - 0.0: VWAP 아래에서 강한 저항
///   - 0.5: VWAP 근처에서 중립
///   - 1.0: VWAP 위에서 강한 지지
pub fn calculate_vwap_support_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // get_morning_data를 통해 5분봉 데이터 조회
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    if morning_data.closes.is_empty() {
        return Ok(0.5); // 데이터가 없으면 중립값 반환
    }

    // VWAP 계산
    let mut total_volume_price = 0.0;
    let mut total_volume = 0.0;

    for i in 0..morning_data.closes.len() {
        let high = morning_data.highs[i];
        let low = morning_data.lows[i];
        let close = morning_data.closes[i];
        let volume = morning_data.volumes[i];
        
        let typical_price = (high + low + close) / 3.0;
        total_volume_price += typical_price * volume;
        total_volume += volume;
    }

    let vwap = if total_volume > 0.0 {
        total_volume_price / total_volume
    } else {
        return Ok(0.5); // 거래량이 0이면 중립값 반환
    };

    // 현재가 조회
    let current_price = morning_data.get_last_close().unwrap_or(0.0);
    if current_price == 0.0 {
        return Ok(0.5);
    }

    // VWAP 대비 현재가 위치 계산
    let price_vs_vwap = (current_price - vwap) / vwap;

    // [-10%, +10%] 범위를 [0, 1]로 정규화
    let normalized_value = if price_vs_vwap <= -0.10 {
        0.0 // VWAP 아래 10% 이상
    } else if price_vs_vwap >= 0.10 {
        1.0 // VWAP 위 10% 이상
    } else {
        // -10% ~ +10% 범위를 0~1로 정규화
        (price_vs_vwap + 0.10) / 0.20
    };

    Ok(normalized_value)
}

/// Day8 특징: 밴드 돌파 강도 연속형 변수
/// 기존 이진형 밴드 돌파 지표를 연속값으로 확장
/// 
/// # Arguments
/// * `db_5min` - 5분 데이터베이스 연결
/// * `stock_code` - 종목 코드
/// * `date` - 날짜
/// 
/// # Returns
/// * `StockrsResult<f64>` - 밴드 돌파 강도 (0~1 범위로 정규화)
///   - 0.0: 하단 밴드 아래에서 강한 하락
///   - 0.5: 밴드 내에서 중립
///   - 1.0: 상단 밴드 위에서 강한 상승
pub fn calculate_band_breakout_strength(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
) -> StockrsResult<f64> {
    // get_morning_data를 통해 5분봉 데이터 조회
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    
    if morning_data.closes.len() < 5 {
        return Ok(0.0); // <5개 데이터면 보수적 접근으로 0.0 반환
    }

    // 이동평균과 표준편차 계산 (가용 구간 전체)
    let prices: Vec<f64> = morning_data.closes.iter().copied().collect();
    let sma = prices.iter().sum::<f64>() / prices.len() as f64;

    let variance = prices.iter().map(|&p| (p - sma).powi(2)).sum::<f64>() / prices.len() as f64;
    let std_dev = variance.sqrt();

    // 볼린저 밴드 계산
    let upper_band = sma + (2.0 * std_dev);
    let lower_band = sma - (2.0 * std_dev);

    // 현재가 조회
    let current_price = morning_data.get_last_close().unwrap_or(0.0);
    if current_price == 0.0 {
        return Ok(0.5);
    }

    // 밴드 대비 현재가 위치를 연속값으로 계산
    let band_width = upper_band - lower_band;
    if band_width == 0.0 {
        return Ok(0.0); // 밴드 폭이 0이면 보수적 접근으로 0.0 반환
    }

    // 현재가가 밴드 내에서 어느 위치에 있는지 계산 (0~1 범위)
    let position_in_band = (current_price - lower_band) / band_width;

    // 밴드 돌파 강도를 연속값으로 계산
    let breakout_strength = if position_in_band < 0.0 {
        // 하단 밴드 아래: 하락 강도 (0~1 범위)
        position_in_band.abs().min(1.0)
    } else if position_in_band > 1.0 {
        // 상단 밴드 위: 상승 돌파 강도 (0~1 범위)
        (position_in_band - 1.0).min(1.0)
    } else {
        // 밴드 내부: 중립 (0.5)
        0.5
    };

    // [0, 1] 범위로 정규화
    let normalized_value = if breakout_strength <= 0.0 {
        0.0 // 강한 하락
    } else if breakout_strength >= 1.0 {
        1.0 // 강한 상승 돌파
    } else {
        breakout_strength // 이미 0~1 범위
    };

    Ok(normalized_value)
}