use super::utils::{get_morning_data, get_previous_trading_day, is_first_trading_day, get_daily_data};
use crate::utility::errors::{StockrsError, StockrsResult};
use rusqlite::Connection;
use tracing::{debug, info, warn};

/// day2_prev_day_range_ratio: 전일 일봉의 (고가-저가)/종가 비율
pub fn calculate_prev_day_range_ratio(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    info!("🔍 [day2_prev_day_range_ratio] 함수 시작 (종목: {}, 날짜: {})", stock_code, date);
    
    // 첫 거래일인지 확인
    info!("🔍 [day2_prev_day_range_ratio] is_first_trading_day 호출 전");
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        info!("✅ [day2_prev_day_range_ratio] 첫 거래일이므로 기본값 1.0 반환");
        return Ok(1.0);
    }

    // 주식 시장이 열린 날 기준으로 전일 계산
    info!("🔍 [day2_prev_day_range_ratio] get_previous_trading_day 호출 전 (종목: {}, 날짜: {}, trading_dates 길이: {})", stock_code, date, trading_dates.len());
    let prev_date_str = get_previous_trading_day(trading_dates, date)?;
    info!("📅 [day2_prev_day_range_ratio] 전일 날짜: {}", prev_date_str);

    // 테이블명 (일봉 DB는 A 접두사 포함)
    let table_name = stock_code;
    info!("📋 [day2_prev_day_range_ratio] 테이블명: {}", table_name);

    // 테이블 존재 여부 확인
    let table_exists: i64 = daily_db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            rusqlite::params![&table_name],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query(format!(
                "일봉 테이블 존재 여부 확인 실패: {}",
                table_name
            ))
        })?;

    if table_exists == 0 {
        return Err(StockrsError::database_query(format!(
            "일봉 테이블이 존재하지 않습니다: {} (종목: {})",
            table_name, stock_code
        )));
    }

    // 전일 데이터 존재 여부 확인
    let prev_data_exists: i64 = daily_db
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\" WHERE date = ?", table_name),
            rusqlite::params![&prev_date_str],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query(format!(
                "전일 데이터 존재 여부 확인 실패: {} (날짜: {})",
                table_name, prev_date_str
            ))
        })?;

    if prev_data_exists == 0 {
        return Err(StockrsError::database_query(format!(
            "전일 데이터가 없습니다: {} (테이블: {}, 날짜: {})",
            stock_code, table_name, prev_date_str
        )));
    }

    // 전일 고가, 저가, 종가 조회
    let (prev_high, prev_low, prev_close): (f64, f64, f64) = daily_db.query_row(
        &format!(
            "SELECT high, low, close FROM \"{}\" WHERE date = ?",
            table_name
        ),
        rusqlite::params![&prev_date_str],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    if prev_close <= 0.0 {
        return Err(StockrsError::prediction(format!(
            "전일 종가가 유효하지 않습니다: {:.2} (종목: {})",
            prev_close, stock_code
        )));
    }

    // 전일 범위 비율 계산: (고가 - 저가) / 종가
    let prev_day_range_ratio = (prev_high - prev_low) / prev_close;

    Ok(prev_day_range_ratio)
}

/// day2_prev_close_to_now_ratio: 전일 종가 대비 현재가 비율
pub fn calculate_prev_close_to_now_ratio(
    db: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    info!("🔍 [day2_prev_close_to_now_ratio] 종목: {}, 날짜: {}", stock_code, date);
    
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        info!("✅ [day2_prev_close_to_now_ratio] 첫 거래일이므로 기본값 1.0 반환");
        return Ok(1.0);
    }

    // 주식 시장이 열린 날 기준으로 전일 계산
    let prev_date_str = get_previous_trading_day(trading_dates, date)?;
    info!("📅 [day2_prev_close_to_now_ratio] 전일 날짜: {}", prev_date_str);

    // 테이블명 (일봉 DB는 A 접두사 포함)
    let table_name = stock_code;
    info!("📋 [day2_prev_close_to_now_ratio] 테이블명: {}", table_name);

    // 먼저 테이블 존재 여부 확인
    let table_exists: i64 = daily_db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            rusqlite::params![&table_name],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query(format!(
                "일봉 테이블 존재 여부 확인 실패: {}",
                table_name
            ))
        })?;

    if table_exists == 0 {
        return Err(StockrsError::database_query(format!(
            "일봉 테이블이 존재하지 않습니다: {} (종목: {})",
            table_name, stock_code
        )));
    }

    // 테이블의 실제 데이터 확인 (디버깅용)
    let sample_data: Vec<String> = daily_db
        .prepare(&format!(
            "SELECT date FROM \"{}\" ORDER BY date DESC LIMIT 5",
            table_name
        ))?
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    debug!("테이블 {}의 최근 5개 날짜: {:?}", table_name, sample_data);

    // 전일 데이터 존재 여부 확인
    let prev_data_exists: i64 = daily_db
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\" WHERE date = ?", table_name),
            rusqlite::params![&prev_date_str],
            |row| row.get(0),
        )
        .map_err(|_| {
            crate::utility::errors::StockrsError::database_query(format!(
                "전일 데이터 존재 여부 확인 실패: {} (날짜: {})",
                table_name, prev_date_str
            ))
        })?;

    if prev_data_exists == 0 {
        return Err(crate::utility::errors::StockrsError::database_query(format!(
            "전일 데이터가 없습니다: {} (테이블: {}, 날짜: {})",
            stock_code, table_name, prev_date_str
        )));
    }

    // 전일 종가 조회
    let prev_close: f64 = daily_db.query_row(
        &format!("SELECT close FROM \"{}\" WHERE date = ?", table_name),
        rusqlite::params![&prev_date_str],
        |row| row.get(0),
    )?;

    if prev_close <= 0.0 {
        return Err(crate::utility::errors::StockrsError::prediction(format!(
            "전일 종가가 유효하지 않습니다: {:.2} (종목: {})",
            prev_close, stock_code
        )));
    }

    // 당일 현재가 조회 (9시반 이전 5분봉 마지막 종가)
    let morning_data = get_morning_data(db, stock_code, date)?;
    let current_close = morning_data.get_last_close().ok_or_else(|| {
        StockrsError::prediction(format!(
            "당일 현재가를 찾을 수 없습니다 (종목: {})",
            stock_code
        ))
    })?;

    if current_close <= 0.0 {
        return Err(StockrsError::prediction(format!(
            "당일 현재가가 유효하지 않습니다: {:.2} (종목: {})",
            current_close, stock_code
        )));
    }

    Ok(current_close / prev_close)
}

/// 전일 대비 거래량 비율 계산
pub fn calculate_volume_ratio_vs_prevday(
    db: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    info!("🔍 [day2_volume_ratio_vs_prevday] 종목: {}, 날짜: {}", stock_code, date);
    info!("📋 [day2_volume_ratio_vs_prevday] trading_dates 길이: {}", trading_dates.len());
    if trading_dates.len() > 0 {
        info!("📋 [day2_volume_ratio_vs_prevday] trading_dates 첫 5개: {:?}", &trading_dates[..trading_dates.len().min(5)]);
    }
    
    // 첫 번째 거래일인 경우 기본값 반환
    info!("🔍 [day2_volume_ratio_vs_prevday] 첫 거래일 확인 중...");
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        info!("✅ [day2_volume_ratio_vs_prevday] 첫 거래일이므로 기본값 1.0 반환");
        return Ok(1.0); // 기본 거래량 비율
    }
    
    // 전일 대비 거래량 비율
    info!("🔍 [day2_volume_ratio_vs_prevday] 전일 날짜 계산 중...");
    let prev_date = get_previous_trading_day(trading_dates, date)?;
    info!("📅 [day2_volume_ratio_vs_prevday] 전일 날짜: {} (현재 날짜: {})", prev_date, date);
    
    // 전일 거래량 조회
    info!("🔍 [day2_volume_ratio_vs_prevday] 전일 거래량 조회 중... (종목: {}, 전일: {})", stock_code, prev_date);
    let prev_data = get_daily_data(daily_db, stock_code, &prev_date)?;
    let prev_volume = prev_data.get_volume()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_volume_ratio_vs_prevday".to_string(),
            "전일 거래량 데이터가 필요합니다".to_string(),
        ))?;
    info!("📊 [day2_volume_ratio_vs_prevday] 전일 거래량: {} (종목: {}, 전일: {})", prev_volume, stock_code, prev_date);
    
    // 당일 오전 거래량 조회 (5분봉 DB에서 조회)
    info!("🔍 [day2_volume_ratio_vs_prevday] 당일 오전 거래량 조회 중...");
    let today_data = get_morning_data(db, stock_code, date)?;
    let today_volume = today_data.get_avg_volume()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_volume_ratio_vs_prevday".to_string(),
            "당일 오전 거래량 데이터가 필요합니다".to_string(),
        ))?;
    info!("📊 [day2_volume_ratio_vs_prevday] 당일 오전 거래량: {}", today_volume);
    
    // 거래량 비율 계산 (당일 오전 거래량 / 전일 거래량)
    if prev_volume > 0.0 {
        let ratio = today_volume / prev_volume;
        info!("📈 [day2_volume_ratio_vs_prevday] 거래량 비율: {:.4}", ratio);
        Ok(ratio)
    } else {
        warn!("⚠️ [day2_volume_ratio_vs_prevday] 전일 거래량이 0이므로 0.0 반환");
        Ok(0.0) // 전일 거래량이 0이면 0.0 반환
    }
}
