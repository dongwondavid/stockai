use super::utils::{
    get_morning_data,
    is_first_trading_day,
    get_prev_daily_data_opt,
};
use crate::utility::errors::{StockrsError, StockrsResult};
use chrono::{Duration, NaiveDate};
use rusqlite::Connection;
use tracing::debug;

/// day3_morning_mdd: 장 시작 후 최대 낙폭(Maximum Drawdown)
pub fn calculate_morning_mdd(db_5min: &Connection, stock_code: &str, date: &str) -> StockrsResult<f64> {
    let morning_data = get_morning_data(db_5min, stock_code, date)?;

    if morning_data.closes.is_empty() {
        return Ok(0.0);
    }

    let open_price = morning_data.closes[0];
    let mut max_price = open_price;
    let mut mdd = 0.0;

    for &price in &morning_data.closes {
        if price > max_price {
            max_price = price;
        }

        let drawdown = (max_price - price) / max_price;
        if drawdown > mdd {
            mdd = drawdown;
        }
    }

    Ok(-mdd) // 음수로 반환 (낙폭)
}


/// 테이블명 생성 시 A 접두사 중복 방지
fn get_table_name(stock_code: &str) -> String {
    if stock_code.starts_with('A') {
        stock_code.to_string()
    } else {
        format!("A{}", stock_code)
    }
}

/// day3_breaks_6month_high: 6개월 내 최고가 돌파 여부
pub fn calculate_breaks_6month_high(
    db_5min: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 첫 거래일인지 확인
    if is_first_trading_day(daily_db, stock_code, date, trading_dates)? {
        return Ok(1.0);
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

    debug!(
        "6개월 전고점 조회 - 종목: {}, 날짜 범위: {} ~ {}",
        stock_code, six_months_ago_str, target_date_str
    );

    // 테이블명 (일봉 DB는 A 접두사 포함)
    let table_name = get_table_name(stock_code);

    // 먼저 테이블에 데이터가 있는지 확인
    let table_count: i64 = daily_db
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\"", table_name),
            [],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query(format!(
                "테이블 데이터 개수 확인 실패: {}",
                table_name
            ))
        })?;

    if table_count == 0 {
        return Err(StockrsError::database_query(format!(
            "테이블 {}에 데이터가 없습니다.",
            table_name
        )));
    }

    // 6개월 내 데이터 개수 확인
    let six_month_data_count: i64 = daily_db
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM \"{}\" WHERE date >= ? AND date < ?",
                table_name
            ),
            rusqlite::params![&six_months_ago_str, &target_date_str],
            |row| row.get(0),
        )
        .map_err(|_| {
            StockrsError::database_query(format!(
                "6개월 내 데이터 개수 확인 실패: {} (범위: {} ~ {})",
                table_name, six_months_ago_str, target_date_str
            ))
        })?;

    if six_month_data_count == 0 {
        return Err(StockrsError::database_query(format!(
            "6개월 내 데이터가 없습니다. 날짜 범위: {} ~ {} (테이블: {})",
            six_months_ago_str, target_date_str, table_name
        )));
    }

    // 6개월 내 최고가 조회 (당일 포함하지 않음)
    let six_month_high: f64 = daily_db.query_row(
        &format!(
            "SELECT MAX(high) FROM \"{}\" WHERE date >= ? AND date < ?",
            table_name
        ),
        rusqlite::params![&six_months_ago_str, &target_date_str],
        |row| row.get(0),
    )?;

    if six_month_high <= 0.0 {
        return Err(StockrsError::prediction(format!(
            "6개월 내 최고가가 유효하지 않습니다: {:.2}",
            six_month_high
        )));
    }

    // 당일 고가는 오전 5분봉의 최고가로 판단
    let morning = get_morning_data(db_5min, stock_code, date)?;
    let today_high = morning
        .highs
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let breaks_6month_high = if today_high.is_finite() && six_month_high > 0.0 && today_high >= six_month_high {
        1.0
    } else {
        0.0
    };

    Ok(breaks_6month_high)
}

/// 오전 거래량 비율 계산
/// - 시간 구간: 일반일 09:00~09:30, 특이일(예: start1000) 10:00~10:30
/// - 분자: 위 시간 구간의 5분봉 거래량의 평균값
/// - 분모: 전일 일봉 거래량 (동일 일자 사용 시 데이터 누수 발생)
pub fn calculate_morning_volume_ratio(
    db: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> StockrsResult<f64> {
    // 오전 거래량 비율 계산
    let morning_data = get_morning_data(db, stock_code, date)?;
    let morning_volume = morning_data.get_avg_volume()
        .ok_or_else(|| StockrsError::unsupported_feature(
            "calculate_morning_volume_ratio".to_string(),
            "오전 거래량 데이터가 필요합니다".to_string(),
        ))?;
    // 전일(이전 거래일) 일봉 거래량 조회 - 당일 사용 시 데이터 누수
    let prev_opt = get_prev_daily_data_opt(daily_db, stock_code, date, trading_dates)?;
    let prev_daily_volume = match prev_opt.and_then(|d| d.get_volume()) {
        Some(v) if v > 0.0 => v,
        _ => return Ok(0.5), // 중립값
    };
    
    // 오전 거래량 비율 계산 (오전 거래량 / 전일 일봉 거래량)
    if prev_daily_volume > 0.0 {
        Ok(morning_volume / prev_daily_volume)
    } else {
        Ok(0.0) // 당일 일봉 거래량이 0이면 0.0 반환
    }
}
