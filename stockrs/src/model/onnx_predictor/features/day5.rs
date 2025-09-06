use crate::utility::errors::{StockrsError, StockrsResult};
use super::utils::{get_morning_data, is_first_trading_day};
use rusqlite::Connection;

/// 테이블명 생성 시 A 접두사 중복 방지
fn get_table_name(stock_code: &str) -> String {
    if stock_code.starts_with('A') {
        stock_code.to_string()
    } else {
        format!("A{}", stock_code)
    }
}

/// day5_prev_day_range_log: 전일 변동성을 로그 스케일로 변환하여 극단값 완화
pub fn calculate_day5_prev_day_range_log(
    db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(db, stock_code, date, trading_dates)? {
        return Ok(0.0); // 첫 거래일이면 전일 데이터 없음
    }

    let table_name = get_table_name(stock_code);
    
    // 전일 데이터 조회
    let (high_prev, low_prev, close_prev): (f64, f64, f64) = db.query_row(
        &format!(
            "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1",
            table_name
        ),
        rusqlite::params![date],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|_| {
        StockrsError::database_query(format!(
            "전일 데이터 조회 실패: {} (종목: {}, 날짜: {})",
            table_name, stock_code, date
        ))
    })?;
    
    if close_prev <= 0.0 {
        return Ok(0.0);
    }
    
    let range_prev = high_prev - low_prev;
    if range_prev < 0.0 {
        return Ok(0.0);
    }
    
    let range_ratio = range_prev / close_prev;
    let result = (1.0 + range_ratio).ln();
    
    // 값 클리핑 [0, 0.2]
    Ok(result.max(0.0).min(0.2))
}

/// day5_prev_day_range_change: 전일 대비 변동성 변화율을 측정하여 급등/급락 시그널 포착
pub fn calculate_day5_prev_day_range_change(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(0.0); // 첫 거래일이면 전일 데이터 없음
    }

    let table_name = get_table_name(stock_code);
    
    // 전일과 전전일 데이터 조회
    let mut stmt = daily_db.prepare(&format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 2",
        table_name
    ))?;
    
    let mut rows = stmt.query(rusqlite::params![date])?;
    let mut data = Vec::new();
    
    while let Some(row) = rows.next()? {
        let high: f64 = row.get(0)?;
        let low: f64 = row.get(1)?;
        let close: f64 = row.get(2)?;
        data.push((high, low, close));
    }
    
    if data.len() < 2 {
        return Ok(0.0);
    }
    
    let (high_prev, low_prev, close_prev) = data[0];
    let (high_prev2, low_prev2, close_prev2) = data[1];
    
    if close_prev <= 0.0 || close_prev2 <= 0.0 {
        return Ok(0.0);
    }
    
    let range_prev = high_prev - low_prev;
    let range_prev2 = high_prev2 - low_prev2;
    
    if range_prev < 0.0 || range_prev2 < 0.0 {
        return Ok(0.0);
    }
    
    let range_ratio_today = range_prev / close_prev;
    let range_ratio_yesterday = range_prev2 / close_prev2;
    
    if range_ratio_yesterday == 0.0 {
        return Ok(0.0);
    }
    
    let result = (range_ratio_today / range_ratio_yesterday) - 1.0;
    
    // 값 클리핑 [-0.5, 2.0]
    Ok(result.max(-0.5).min(2.0))
}

/// day5_prev_day_range_gap_combo: 전일 변동성과 당일 시가 갭을 결합하여 강한 시그널 포착
pub fn calculate_day5_prev_day_range_gap_combo(
    db_5min: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(db_5min, stock_code, date, trading_dates)? {
        return Ok(0.0); // 첫 거래일이면 전일 데이터 없음
    }

    let table_name = get_table_name(stock_code);
    
    // 전일 데이터 조회
    let (high_prev, low_prev, close_prev): (f64, f64, f64) = db_5min.query_row(
        &format!(
            "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1",
            table_name
        ),
        rusqlite::params![date],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|_| {
        StockrsError::database_query(format!(
            "전일 데이터 조회 실패: {} (종목: {}, 날짜: {})",
            table_name, stock_code, date
        ))
    })?;
    
    // 당일 시가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let open_today = morning_data.get_last_open().unwrap_or(0.0);
    
    if close_prev <= 0.0 || open_today <= 0.0 {
        return Ok(0.0);
    }
    
    let range_prev = high_prev - low_prev;
    if range_prev < 0.0 {
        return Ok(0.0);
    }
    
    let range_ratio = range_prev / close_prev;
    let gap_ratio = (open_today - close_prev) / close_prev;
    let result = range_ratio * gap_ratio;
    
    // 값 클리핑 [-0.1, 0.1]
    Ok(result.max(-0.1).min(0.1))
}

/// day5_prev_day_range_body_ratio: 전일 변동성 중 실제 방향성(몸통)의 비중을 측정하여 시그널성 강도 평가
pub fn calculate_day5_prev_day_range_body_ratio(
    db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 전일 데이터 조회
    let (high_prev, low_prev, open_prev, close_prev): (f64, f64, f64, f64) = db.query_row(
        &format!(
            "SELECT high, low, open, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1",
            table_name
        ),
        rusqlite::params![date],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).map_err(|_| {
        StockrsError::database_query(format!(
            "전일 데이터 조회 실패: {} (종목: {}, 날짜: {})",
            table_name, stock_code, date
        ))
    })?;
    
    let range_prev = high_prev - low_prev;
    if range_prev < 0.0 {
        return Ok(1.0);
    }
    
    let body_prev = (close_prev - open_prev).abs();
    if body_prev == 0.0 {
        return Ok(1.0);
    }
    
    let result = range_prev / body_prev;
    
    // 값 클리핑 [1, 10]
    Ok(result.max(1.0).min(10.0))
}

/// day5_prev_day_range_atr_rel: 단일일 변동성을 n일 ATR 대비 정규화하여 상대적 변동성 측정
pub fn calculate_day5_prev_day_range_atr_rel(
    db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 전일 데이터 조회
    let (high_prev, low_prev): (f64, f64) = db.query_row(
        &format!(
            "SELECT high, low FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 1",
            table_name
        ),
        rusqlite::params![date],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|_| {
        StockrsError::database_query(format!(
            "전일 데이터 조회 실패: {} (종목: {}, 날짜: {})",
            table_name, stock_code, date
        ))
    })?;
    
    // ATR(14) 계산 (간단한 구현)
    let mut stmt = db.prepare(&format!(
        "SELECT high, low, close FROM \"{}\" WHERE date < ? ORDER BY date DESC LIMIT 14",
        table_name
    ))?;
    
    let mut rows = stmt.query(rusqlite::params![date])?;
    let mut true_ranges = Vec::new();
    let mut prev_close: Option<f64> = None;
    
    while let Some(row) = rows.next()? {
        let high: f64 = row.get(0)?;
        let low: f64 = row.get(1)?;
        let close: f64 = row.get(2)?;
        
        if let Some(prev) = prev_close {
            let tr1 = high - low;
            let tr2 = (high - prev).abs();
            let tr3 = (low - prev).abs();
            let true_range = tr1.max(tr2).max(tr3);
            true_ranges.push(true_range);
        }
        
        prev_close = Some(close);
    }
    
    if true_ranges.is_empty() {
        return Ok(1.0);
    }
    
    let atr_14 = true_ranges.iter().sum::<f64>() / true_ranges.len() as f64;
    
    if atr_14 <= 0.0 {
        return Ok(1.0);
    }
    
    let range_prev = high_prev - low_prev;
    if range_prev < 0.0 {
        return Ok(0.5);
    }
    
    let result = range_prev / atr_14;
    
    // 값 클리핑 [0.5, 2.0]
    Ok(result.max(0.5).min(2.0))
}

/// day5_pos_vs_high_4d: 4일 고점 대비 현재 가격의 상대적 위치를 측정하여 단기 모멘텀 평가
pub fn calculate_day5_pos_vs_high_4d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 4일 이내의 고점 조회 (trading_dates 사용)
    let high_4d: f64 = if let Some(idx) = trading_dates.iter().position(|d| d == date) {
        if idx >= 4 {
            let start_date = &trading_dates[idx - 4];
            daily_db.query_row(
                &format!("SELECT MAX(high) FROM \"{}\" WHERE date < ? AND date >= ?", table_name),
                rusqlite::params![date, start_date],
                |row| row.get(0)
            ).map_err(|_| {
                StockrsError::database_query(format!(
                    "4일 고점 조회 실패: {} (종목: {}, 날짜: {})",
                    table_name, stock_code, date
                ))
            })?
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    if high_4d <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 종가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_today < 0.0 {
        return Ok(0.0);
    }
    
    let result = close_today / high_4d;
    
    // 값 클리핑 [0, 1]
    Ok(result.max(0.0).min(1.0))
}

/// day5_pos_vs_high_15d: 15일 고점 대비 현재 가격의 상대적 위치를 측정하여 중기 모멘텀 평가
pub fn calculate_day5_pos_vs_high_15d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 15일 이내의 고점 조회 (trading_dates 사용)
    let high_15d: f64 = if let Some(idx) = trading_dates.iter().position(|d| d == date) {
        if idx >= 15 {
            let start_date = &trading_dates[idx - 15];
            daily_db.query_row(
                &format!("SELECT MAX(high) FROM \"{}\" WHERE date < ? AND date >= ?", table_name),
                rusqlite::params![date, start_date],
                |row| row.get(0)
            ).map_err(|_| {
                StockrsError::database_query(format!(
                    "15일 고점 조회 실패: {} (종목: {}, 날짜: {})",
                    table_name, stock_code, date
                ))
            })?
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    if high_15d <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 종가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_today < 0.0 {
        return Ok(0.0);
    }
    
    let result = close_today / high_15d;
    
    // 값 클리핑 [0, 1]
    Ok(result.max(0.0).min(1.0))
}

/// day5_pos_vs_high_20d: 20일 고점 대비 현재 가격의 상대적 위치를 측정하여 중기 모멘텀 평가
pub fn calculate_day5_pos_vs_high_20d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 20일 이내의 고점 조회 (trading_dates 사용)
    let high_20d: f64 = if let Some(idx) = trading_dates.iter().position(|d| d == date) {
        if idx >= 20 {
            let start_date = &trading_dates[idx - 20];
            daily_db.query_row(
                &format!("SELECT MAX(high) FROM \"{}\" WHERE date < ? AND date >= ?", table_name),
                rusqlite::params![date, start_date],
                |row| row.get(0)
            ).map_err(|_| {
                StockrsError::database_query(format!(
                    "20일 고점 조회 실패: {} (종목: {}, 날짜: {})",
                    table_name, stock_code, date
                ))
            })?
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    if high_20d <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 종가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_today < 0.0 {
        return Ok(0.0);
    }
    
    let result = close_today / high_20d;
    
    // 값 클리핑 [0, 1]
    Ok(result.max(0.0).min(1.0))
}

/// day5_pos_vs_high_30d: 30일 고점 대비 현재 가격의 상대적 위치를 측정하여 중장기 모멘텀 평가
pub fn calculate_day5_pos_vs_high_30d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 30일 이내의 고점 조회 (trading_dates 사용)
    let high_30d: f64 = if let Some(idx) = trading_dates.iter().position(|d| d == date) {
        if idx >= 30 {
            let start_date = &trading_dates[idx - 30];
            daily_db.query_row(
                &format!("SELECT MAX(high) FROM \"{}\" WHERE date < ? AND date >= ?", table_name),
                rusqlite::params![date, start_date],
                |row| row.get(0)
            ).map_err(|_| {
                StockrsError::database_query(format!(
                    "30일 고점 조회 실패: {} (종목: {}, 날짜: {})",
                    table_name, stock_code, date
                ))
            })?
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    if high_30d <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 종가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_today < 0.0 {
        return Ok(0.0);
    }
    
    let result = close_today / high_30d;
    
    // 값 클리핑 [0, 1]
    Ok(result.max(0.0).min(1.0))
}

/// day5_pos_vs_high_40d: 40일 고점 대비 현재 가격의 상대적 위치를 측정하여 중장기 모멘텀 평가
pub fn calculate_day5_pos_vs_high_40d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 40일 이내의 고점 조회 (trading_dates 사용)
    let high_40d: f64 = if let Some(idx) = trading_dates.iter().position(|d| d == date) {
        if idx >= 40 {
            let start_date = &trading_dates[idx - 40];
            daily_db.query_row(
                &format!("SELECT MAX(high) FROM \"{}\" WHERE date < ? AND date >= ?", table_name),
                rusqlite::params![date, start_date],
                |row| row.get(0)
            ).map_err(|_| {
                StockrsError::database_query(format!(
                    "40일 고점 조회 실패: {} (종목: {}, 날짜: {})",
                    table_name, stock_code, date
                ))
            })?
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    if high_40d <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 종가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_today < 0.0 {
        return Ok(0.0);
    }
    
    let result = close_today / high_40d;
    
    // 값 클리핑 [0, 1]
    Ok(result.max(0.0).min(1.0))
}

/// day5_pos_vs_high_50d: 50일 고점 대비 현재 가격의 상대적 위치를 측정하여 장기 모멘텀 평가
pub fn calculate_day5_pos_vs_high_50d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 50일 이내의 고점 조회 (trading_dates 사용)
    let high_50d: f64 = if let Some(idx) = trading_dates.iter().position(|d| d == date) {
        if idx >= 50 {
            let start_date = &trading_dates[idx - 50];
            daily_db.query_row(
                &format!("SELECT MAX(high) FROM \"{}\" WHERE date < ? AND date >= ?", table_name),
                rusqlite::params![date, start_date],
                |row| row.get(0)
            ).map_err(|_| {
                StockrsError::database_query(format!(
                    "50일 고점 조회 실패: {} (종목: {}, 날짜: {})",
                    table_name, stock_code, date
                ))
            })?
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    if high_50d <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 종가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_today < 0.0 {
        return Ok(0.0);
    }
    
    let result = close_today / high_50d;
    
    // 값 클리핑 [0, 1]
    Ok(result.max(0.0).min(1.0))
}

/// day5_pos_vs_high_100d: 100일 고점 대비 현재 가격의 상대적 위치를 측정하여 장기 모멘텀 평가
pub fn calculate_day5_pos_vs_high_100d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 100일 이내의 고점 조회 (trading_dates 사용)
    let high_100d: f64 = if let Some(idx) = trading_dates.iter().position(|d| d == date) {
        if idx >= 100 {
            let start_date = &trading_dates[idx - 100];
            daily_db.query_row(
                &format!("SELECT MAX(high) FROM \"{}\" WHERE date < ? AND date >= ?", table_name),
                rusqlite::params![date, start_date],
                |row| row.get(0)
            ).map_err(|_| {
                StockrsError::database_query(format!(
                    "100일 고점 조회 실패: {} (종목: {}, 날짜: {})",
                    table_name, stock_code, date
                ))
            })?
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    if high_100d <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 종가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_today < 0.0 {
        return Ok(0.0);
    }
    
    let result = close_today / high_100d;
    
    // 값 클리핑 [0, 1]
    Ok(result.max(0.0).min(1.0))
}

/// day5_pos_vs_high_150d: 150일 고점 대비 현재 가격의 상대적 위치를 측정하여 장기 모멘텀 평가
pub fn calculate_day5_pos_vs_high_150d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 150일 이내의 고점 조회 (trading_dates 사용)
    let high_150d: f64 = if let Some(idx) = trading_dates.iter().position(|d| d == date) {
        if idx >= 150 {
            let start_date = &trading_dates[idx - 150];
            daily_db.query_row(
                &format!("SELECT MAX(high) FROM \"{}\" WHERE date < ? AND date >= ?", table_name),
                rusqlite::params![date, start_date],
                |row| row.get(0)
            ).map_err(|_| {
                StockrsError::database_query(format!(
                    "150일 고점 조회 실패: {} (종목: {}, 날짜: {})",
                    table_name, stock_code, date
                ))
            })?
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    if high_150d <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 종가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_today < 0.0 {
        return Ok(0.0);
    }
    
    let result = close_today / high_150d;
    
    // 값 클리핑 [0, 1]
    Ok(result.max(0.0).min(1.0))
}

/// day5_pos_vs_high_200d: 200일 고점 대비 현재 가격의 상대적 위치를 측정하여 장기 모멘텀 평가
pub fn calculate_day5_pos_vs_high_200d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 200일 이내의 고점 조회 (trading_dates 사용)
    let high_200d: f64 = if let Some(idx) = trading_dates.iter().position(|d| d == date) {
        if idx >= 200 {
            let start_date = &trading_dates[idx - 200];
            daily_db.query_row(
                &format!("SELECT MAX(high) FROM \"{}\" WHERE date < ? AND date >= ?", table_name),
                rusqlite::params![date, start_date],
                |row| row.get(0)
            ).map_err(|_| {
                StockrsError::database_query(format!(
                    "200일 고점 조회 실패: {} (종목: {}, 날짜: {})",
                    table_name, stock_code, date
                ))
            })?
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    if high_200d <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 종가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_today < 0.0 {
        return Ok(0.0);
    }
    
    let result = close_today / high_200d;
    
    // 값 클리핑 [0, 1]
    Ok(result.max(0.0).min(1.0))
}

/// day5_pos_vs_high_250d: 250일 고점 대비 현재 가격의 상대적 위치를 측정하여 장기 모멘텀 평가
pub fn calculate_day5_pos_vs_high_250d(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0); // 첫 거래일이면 기본값
    }

    let table_name = get_table_name(stock_code);
    
    // 250일 이내의 고점 조회 (trading_dates 사용)
    let high_250d: f64 = if let Some(idx) = trading_dates.iter().position(|d| d == date) {
        if idx >= 250 {
            let start_date = &trading_dates[idx - 250];
            daily_db.query_row(
                &format!("SELECT MAX(high) FROM \"{}\" WHERE date < ? AND date >= ?", table_name),
                rusqlite::params![date, start_date],
                |row| row.get(0)
            ).map_err(|_| {
                StockrsError::database_query(format!(
                    "250일 고점 조회 실패: {} (종목: {}, 날짜: {})",
                    table_name, stock_code, date
                ))
            })?
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    if high_250d <= 0.0 {
        return Ok(0.0);
    }
    
    // 당일 종가 조회 (morning_data 사용)
    let morning_data = get_morning_data(db_5min, stock_code, date)?;
    let close_today = morning_data.get_last_close().unwrap_or(0.0);
    
    if close_today < 0.0 {
        return Ok(0.0);
    }
    
    let result = close_today / high_250d;
    
    // 값 클리핑 [0, 1]
    Ok(result.max(0.0).min(1.0))
}
