use ndarray::Array2;
use ort::{Environment, SessionBuilder, Value};
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

#[derive(Debug, Serialize, Deserialize)]
struct StockFeatures {
    stock_code: String,
    features: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PredictionResult {
    stock_code: String,
    probability: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ONNXModelInfo {
    onnx_model_path: String,
    features: Vec<String>,
    feature_count: usize,
    input_name: String,
    input_shape: Vec<usize>,
    output_name: String,
    output_shape: Vec<usize>,
}

#[allow(dead_code)]
struct ONNXPredictor {
    session: ort::Session,
    features: Vec<String>,
    input_name: String,
    output_name: String,
}

fn load_extra_stocks() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let file = File::open("extra_stocks.txt")?;
    let reader = BufReader::new(file);
    let mut extra_stocks = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        // 헤더나 빈 줄 건너뛰기
        if line.is_empty() || line.contains("=") || line.contains("총") {
            continue;
        }

        extra_stocks.push(line.to_string());
    }

    debug!("extra_stocks.txt에서 {}개 종목 로드됨", extra_stocks.len());
    Ok(extra_stocks)
}

fn load_features() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let file = File::open("features.txt")?;
    let reader = BufReader::new(file);
    let mut features = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        if !line.is_empty() {
            features.push(line.to_string());
        }
    }

    debug!("features.txt에서 {}개 특징 로드됨", features.len());
    Ok(features)
}

fn get_top_volume_stocks(db: &Connection, date: &str, limit: usize) -> Result<Vec<String>> {
    // 모든 테이블(종목) 목록 가져오기
    let tables_query =
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'";
    let mut stmt = db.prepare(tables_query)?;
    let tables = stmt
        .query_map([], |row| Ok(row.get::<_, String>(0)?))?
        .filter_map(|r| r.ok())
        .collect::<Vec<String>>();

    let mut stock_volumes: Vec<(String, i64)> = Vec::new();

    // 각 종목의 거래대금 계산 (9시~9시반)
    let date_start = format!("{}0900", date);
    let date_end = format!("{}0930", date);

    for table_name in &tables {
        let volume_query = format!(
            "SELECT SUM(volume * close) as total_volume FROM \"{}\" WHERE date >= ? AND date <= ?",
            table_name
        );

        if let Ok(mut volume_stmt) = db.prepare(&volume_query) {
            if let Ok(total_volume) = volume_stmt.query_row([&date_start, &date_end], |row| {
                Ok(row.get::<_, Option<i64>>(0)?.unwrap_or(0))
            }) {
                if total_volume > 0 {
                    // 테이블명이 그대로 종목코드 (A 접두사 포함)
                    let stock_code = table_name.to_string();
                    stock_volumes.push((stock_code, total_volume));
                }
            }
        }
    }

    // 거래대금 기준으로 정렬하고 상위 limit개 선택
    stock_volumes.sort_by(|a, b| b.1.cmp(&a.1));
    let top_stocks: Vec<String> = stock_volumes
        .into_iter()
        .take(limit)
        .map(|(code, _)| code)
        .collect();

    debug!("거래대금 상위 {}개 종목 조회됨", top_stocks.len());
    Ok(top_stocks)
}

fn calculate_features_for_stock_optimized(
    db: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    features: &[String],
    trading_dates: &[String],
) -> Result<Vec<f64>> {
    let mut feature_values = Vec::new();

    for feature in features {
        let value = match feature.as_str() {
            "day4_macd_histogram_increasing" => {
                calculate_macd_histogram_increasing(db, stock_code, date).unwrap_or(0.0)
            }
            "day4_short_macd_cross_signal" => {
                calculate_short_macd_cross_signal(db, stock_code, date).unwrap_or(0.0)
            }
            "day1_current_price_ratio" => {
                calculate_current_price_ratio(db, stock_code, date).unwrap_or(1.0)
            }
            "day1_high_price_ratio" => {
                calculate_high_price_ratio(db, stock_code, date).unwrap_or(1.0)
            }
            "day1_low_price_ratio" => {
                calculate_low_price_ratio(db, stock_code, date).unwrap_or(1.0)
            }
            "day4_open_to_now_return" => {
                calculate_open_to_now_return(db, stock_code, date).unwrap_or(0.0)
            }
            "day4_is_long_bull_candle" => {
                calculate_is_long_bull_candle(db, stock_code, date).unwrap_or(0.0)
            }
            "day1_price_position_ratio" => {
                calculate_price_position_ratio(db, stock_code, date).unwrap_or(0.5)
            }
            "day3_morning_mdd" => calculate_morning_mdd(db, stock_code, date).unwrap_or(0.0),
            "day1_fourth_derivative" => {
                calculate_fourth_derivative(db, stock_code, date).unwrap_or(0.0)
            }
            "day1_long_candle_ratio" => {
                calculate_long_candle_ratio(db, stock_code, date).unwrap_or(0.0)
            }
            "day1_fifth_derivative" => {
                calculate_fifth_derivative(db, stock_code, date).unwrap_or(0.0)
            }
            "day3_breaks_6month_high" => {
                calculate_breaks_6month_high(daily_db, stock_code, date).unwrap_or(0.0)
            }
            "day2_prev_day_range_ratio" => {
                calculate_prev_day_range_ratio(daily_db, stock_code, date, trading_dates)
                    .unwrap_or(0.0)
            }
            "day2_prev_close_to_now_ratio" => {
                calculate_prev_close_to_now_ratio(db, daily_db, stock_code, date, trading_dates)
                    .unwrap_or(1.0)
            }
            "day4_macd_histogram" => calculate_macd_histogram(db, stock_code, date).unwrap_or(0.0),
            "day1_sixth_derivative" => {
                calculate_sixth_derivative(db, stock_code, date).unwrap_or(0.0)
            }
            "day4_pos_vs_high_5d" => {
                calculate_pos_vs_high_5d(daily_db, stock_code, date).unwrap_or(0.0)
            }
            "day4_rsi_value" => calculate_rsi_value(db, stock_code, date).unwrap_or(50.0),
            "day4_pos_vs_high_3d" => {
                calculate_pos_vs_high_3d(daily_db, stock_code, date).unwrap_or(0.0)
            }
            _ => {
                warn!("알 수 없는 특징: {} (종목: {})", feature, stock_code);
                0.0
            }
        };
        feature_values.push(value);
    }

    Ok(feature_values)
}

// 9시반 이전 5분봉 데이터를 조회하는 공통 함수
fn get_morning_data(db: &Connection, stock_code: &str, date: &str) -> Result<MorningData> {
    let table_name = stock_code.to_string();
    let date_start = format!("{}0900", date);
    let date_end = format!("{}0930", date);

    // 먼저 테이블 존재 여부 확인
    let table_exists: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            rusqlite::params![&table_name],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if table_exists == 0 {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "5분봉 테이블이 존재하지 않습니다: {} (종목: {})",
            table_name, stock_code
        )));
    }

    // 해당 날짜의 데이터 존재 여부 확인
    let data_exists: i64 = db
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM \"{}\" WHERE date >= ? AND date <= ?",
                table_name
            ),
            rusqlite::params![&date_start, &date_end],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if data_exists == 0 {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "9시반 이전 데이터가 없습니다: {} (종목: {}, 범위: {} ~ {})",
            stock_code, table_name, date_start, date_end
        )));
    }

    let query = format!(
        "SELECT close, open, high, low FROM \"{}\" WHERE date >= ? AND date <= ? ORDER BY date",
        table_name
    );

    let mut stmt = db.prepare(&query)?;
    let rows = stmt.query_map([&date_start, &date_end], |row| {
        Ok((
            row.get::<_, i32>(0)?, // close
            row.get::<_, i32>(1)?, // open
            row.get::<_, i32>(2)?, // high
            row.get::<_, i32>(3)?, // low
        ))
    })?;

    let mut closes = Vec::new();
    let mut opens = Vec::new();
    let mut highs = Vec::new();
    let mut lows = Vec::new();

    for row in rows {
        let (close, open, high, low) = row?;
        closes.push(close as f64);
        opens.push(open as f64);
        highs.push(high as f64);
        lows.push(low as f64);
    }

    Ok(MorningData {
        closes,
        opens,
        highs,
        lows,
    })
}

// 9시반 이전 데이터 구조체
#[derive(Debug)]
struct MorningData {
    closes: Vec<f64>,
    opens: Vec<f64>,
    highs: Vec<f64>,
    lows: Vec<f64>,
}

impl MorningData {
    fn get_last_close(&self) -> Option<f64> {
        self.closes.last().copied()
    }

    fn get_last_open(&self) -> Option<f64> {
        self.opens.last().copied()
    }

    fn get_max_high(&self) -> Option<f64> {
        self.highs
            .iter()
            .fold(None, |max, &val| Some(max.map_or(val, |m| m.max(val))))
    }

    fn get_min_low(&self) -> Option<f64> {
        self.lows
            .iter()
            .fold(None, |min, &val| Some(min.map_or(val, |m| m.min(val))))
    }

    fn get_last_candle(&self) -> Option<(f64, f64, f64, f64)> {
        if self.closes.is_empty() {
            None
        } else {
            let idx = self.closes.len() - 1;
            Some((
                self.closes[idx],
                self.opens[idx],
                self.highs[idx],
                self.lows[idx],
            ))
        }
    }
}

// 각 특징 계산 함수들 (실제 구현)
fn calculate_macd_histogram_increasing(
    db: &Connection,
    stock_code: &str,
    date: &str,
) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() >= 2 {
        let diff = morning_data.closes[morning_data.closes.len() - 1]
            - morning_data.closes[morning_data.closes.len() - 2];
        Ok(if diff > 0.0 { 1.0 } else { 0.0 })
    } else {
        Ok(0.5)
    }
}

fn calculate_short_macd_cross_signal(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 4 {
        return Ok(0.0);
    }

    // EMA 3, 6 계산
    let ema3 = calculate_ema(&morning_data.closes, 3);
    let ema6 = calculate_ema(&morning_data.closes, 6);
    let macd = ema3 - ema6;

    // 이전 MACD 계산
    if morning_data.closes.len() >= 5 {
        let prev_closes: Vec<f64> = morning_data.closes[..morning_data.closes.len() - 1].to_vec();
        let prev_ema3 = calculate_ema(&prev_closes, 3);
        let prev_ema6 = calculate_ema(&prev_closes, 6);
        let prev_macd = prev_ema3 - prev_ema6;

        if (macd > 0.0 && prev_macd <= 0.0) || (macd > prev_macd && macd > 0.0) {
            return Ok(1.0);
        }
    }

    Ok(0.0)
}

fn calculate_current_price_ratio(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match (morning_data.get_last_close(), morning_data.get_last_open()) {
        (Some(close), Some(open)) => {
            if open > 0.0 {
                Ok(close / open)
            } else {
                Ok(1.0)
            }
        }
        _ => Ok(1.0),
    }
}

fn calculate_high_price_ratio(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match (morning_data.get_max_high(), morning_data.get_last_open()) {
        (Some(max_high), Some(open)) => {
            if open > 0.0 {
                Ok(max_high / open)
            } else {
                Ok(1.0)
            }
        }
        _ => Ok(1.0),
    }
}

fn calculate_low_price_ratio(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match (morning_data.get_min_low(), morning_data.get_last_open()) {
        (Some(min_low), Some(open)) => {
            if open > 0.0 {
                Ok(min_low / open)
            } else {
                Ok(1.0)
            }
        }
        _ => Ok(1.0),
    }
}

fn calculate_open_to_now_return(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match (morning_data.get_last_close(), morning_data.get_last_open()) {
        (Some(close), Some(open)) => {
            if open > 0.0 {
                Ok((close - open) / open)
            } else {
                Ok(0.0)
            }
        }
        _ => Ok(0.0),
    }
}

fn calculate_is_long_bull_candle(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match morning_data.get_last_candle() {
        Some((close, open, high, low)) => {
            let body_size = (close - open).abs();
            let total_size = high - low;

            if total_size > 0.0 && body_size / total_size > 0.6 && close > open {
                Ok(1.0)
            } else {
                Ok(0.0)
            }
        }
        _ => Ok(0.0),
    }
}

fn calculate_price_position_ratio(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match morning_data.get_last_candle() {
        Some((close, _, high, low)) => {
            if high > low {
                Ok((close - low) / (high - low))
            } else {
                Ok(0.5)
            }
        }
        _ => Ok(0.5),
    }
}

fn calculate_morning_mdd(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

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

fn calculate_fourth_derivative(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 5 {
        return Ok(0.0);
    }

    // 4차 도함수 근사 계산
    let fifth = morning_data.closes.len() / 5;
    let first_fifth_avg = morning_data.closes[..fifth].iter().sum::<f64>() / fifth as f64;
    let last_fifth_avg = morning_data.closes[morning_data.closes.len() - fifth..]
        .iter()
        .sum::<f64>()
        / fifth as f64;
    let derivative = last_fifth_avg - first_fifth_avg;

    // 로그 정규화
    if derivative == 0.0 {
        Ok(0.0)
    } else {
        let sign = if derivative > 0.0 { 1.0 } else { -1.0 };
        Ok(sign * (1.0 + derivative.abs()).ln())
    }
}

fn calculate_long_candle_ratio(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    match morning_data.get_last_candle() {
        Some((_, open, high, low)) => {
            let candle_size = high - low;
            let avg_size = open * 0.02;

            if candle_size > avg_size {
                Ok(1.0)
            } else {
                Ok(0.0)
            }
        }
        _ => Ok(0.0),
    }
}

fn calculate_fifth_derivative(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 6 {
        return Ok(0.0);
    }

    // 5차 도함수 근사 계산
    let sixth = morning_data.closes.len() / 6;
    let first_sixth_avg = morning_data.closes[..sixth].iter().sum::<f64>() / sixth as f64;
    let last_sixth_avg = morning_data.closes[morning_data.closes.len() - sixth..]
        .iter()
        .sum::<f64>()
        / sixth as f64;
    let derivative = last_sixth_avg - first_sixth_avg;

    // 로그 정규화
    if derivative == 0.0 {
        Ok(0.0)
    } else {
        let sign = if derivative > 0.0 { 1.0 } else { -1.0 };
        Ok(sign * (1.0 + derivative.abs()).ln())
    }
}

fn calculate_breaks_6month_high(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
) -> Result<f64> {
    use chrono::{Duration, NaiveDate};

    // 날짜 파싱 (YYYYMMDD 형식)
    let year = date[..4].parse::<i32>().unwrap_or(2024);
    let month = date[4..6].parse::<u32>().unwrap_or(1);
    let day = date[6..8].parse::<u32>().unwrap_or(1);

    let target_date = match NaiveDate::from_ymd_opt(year, month, day) {
        Some(date) => date,
        None => {
            warn!("잘못된 날짜 형식: {}", date);
            return Ok(0.0);
        }
    };

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
    let table_name = stock_code;

    // 먼저 테이블에 데이터가 있는지 확인
    let table_count: i64 = daily_db
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\"", table_name),
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if table_count == 0 {
        warn!("테이블 {}에 데이터가 없습니다.", table_name);
        return Ok(0.0);
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
        .unwrap_or(0);

    if six_month_data_count == 0 {
        warn!(
            "6개월 내 데이터가 없습니다. 날짜 범위: {} ~ {} (테이블: {})",
            six_months_ago_str, target_date_str, table_name
        );
        return Ok(0.0);
    }

    // 6개월 내 최고가 조회 (당일 포함하지 않음)
    let six_month_high: f64 = match daily_db.query_row(
        &format!(
            "SELECT MAX(high) FROM \"{}\" WHERE date >= ? AND date < ?",
            table_name
        ),
        rusqlite::params![&six_months_ago_str, &target_date_str],
        |row| row.get(0),
    ) {
        Ok(high) => high,
        Err(e) => {
            warn!(
                "6개월 내 최고가 조회 실패: {} (테이블: {}, 범위: {} ~ {})",
                e, table_name, six_months_ago_str, target_date_str
            );
            return Ok(0.0);
        }
    };

    if six_month_high <= 0.0 {
        warn!("6개월 내 최고가가 유효하지 않습니다: {:.2}", six_month_high);
        return Ok(0.0);
    }

    // 당일 데이터 존재 여부 확인
    let today_data_exists: i64 = daily_db
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\" WHERE date = ?", table_name),
            rusqlite::params![&target_date_str],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if today_data_exists == 0 {
        warn!(
            "당일 데이터가 없습니다. 날짜: {} (테이블: {})",
            target_date_str, table_name
        );
        return Ok(0.0);
    }

    // 당일 일봉 고가 조회
    let today_high: f64 = match daily_db.query_row(
        &format!("SELECT high FROM \"{}\" WHERE date = ?", table_name),
        rusqlite::params![&target_date_str],
        |row| row.get(0),
    ) {
        Ok(high) => high,
        Err(e) => {
            warn!(
                "당일 고가 조회 실패: {} (테이블: {}, 날짜: {})",
                e, table_name, target_date_str
            );
            return Ok(0.0);
        }
    };

    if today_high <= 0.0 {
        warn!("당일 고가가 유효하지 않습니다: {:.2}", today_high);
        return Ok(0.0);
    }

    // 6개월 전고점 돌파 여부 (당일 일봉 고가 기준) - 같을 때도 돌파로 처리
    let breaks_6month_high = if today_high >= six_month_high && six_month_high > 0.0 {
        1.0
    } else {
        0.0
    };

    Ok(breaks_6month_high)
}

// 삼성전자 5분봉 테이블에서 거래일 리스트를 가져오는 함수
fn get_trading_dates_list(db: &Connection) -> Result<Vec<String>> {
    // 먼저 사용 가능한 테이블들을 확인
    let tables_query =
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'";
    let mut stmt = db.prepare(tables_query)?;
    let tables: Vec<String> = stmt
        .query_map([], |row| Ok(row.get::<_, String>(0)?))?
        .filter_map(|r| r.ok())
        .collect();

    // 삼성전자 테이블 찾기 (A005930 또는 005930)
    let table_name = if tables.contains(&"A005930".to_string()) {
        debug!("A005930 테이블 사용");
        "A005930"
    } else if tables.contains(&"005930".to_string()) {
        debug!("005930 테이블 사용");
        "005930"
    } else {
        // 삼성전자가 없으면 첫 번째 테이블 사용
        if let Some(first_table) = tables.first() {
            info!(
                "삼성전자 테이블을 찾을 수 없어 {} 테이블을 사용합니다.",
                first_table
            );
            first_table
        } else {
            return Err(rusqlite::Error::InvalidParameterName(
                "사용 가능한 테이블이 없습니다.".to_string(),
            ));
        }
    };

    // 5분봉 데이터에서 날짜만 추출 (YYYYMMDD 형식으로 변환)
    let query = format!(
        "SELECT DISTINCT date/10000 as ymd FROM \"{}\" ORDER BY ymd",
        table_name
    );
    let mut stmt = db.prepare(&query)?;

    let trading_dates: Vec<String> = stmt
        .query_map([], |row| {
            let ymd = row.get::<_, i64>(0)?;
            Ok(ymd.to_string())
        })?
        .filter_map(|r| r.ok())
        .collect();

    debug!(
        "테이블 {}에서 {}개 거래일 로드됨",
        table_name,
        trading_dates.len()
    );

    if trading_dates.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "테이블 {}에 거래일 데이터가 없습니다.",
            table_name
        )));
    }

    Ok(trading_dates)
}

// 주식 시장이 열린 날 기준으로 전일을 계산하는 함수
fn get_previous_trading_day(trading_dates: &[String], date: &str) -> Result<String> {
    // 빈 배열 체크
    if trading_dates.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "거래일 리스트가 비어있습니다.".to_string(),
        ));
    }

    debug!("거래일 리스트에서 전일 찾기 - 기준 날짜: {}", date);

    // 현재 날짜의 인덱스 찾기
    let current_index = trading_dates.iter().position(|d| d == date);

    match current_index {
        Some(index) => {
            if index > 0 {
                // 이전 거래일 반환
                let prev_date = trading_dates[index - 1].clone();
                debug!("정확한 전일 찾음: {} -> {}", date, prev_date);
                Ok(prev_date)
            } else {
                // 첫 번째 거래일인 경우
                warn!("첫 번째 거래일입니다: {}", date);
                Ok(trading_dates[0].clone())
            }
        }
        None => {
            // 해당 날짜가 거래일 리스트에 없는 경우, 가장 가까운 이전 거래일 찾기
            let target_date = date.parse::<i32>().unwrap_or(0);
            debug!(
                "거래일 리스트에 {}가 없음. 가장 가까운 이전 거래일 찾는 중...",
                date
            );

            for trading_date in trading_dates.iter().rev() {
                if let Ok(td) = trading_date.parse::<i32>() {
                    if td < target_date {
                        debug!(
                            "가장 가까운 이전 거래일 찾음: {} (원래 기준: {})",
                            trading_date, date
                        );
                        return Ok(trading_date.clone());
                    }
                }
            }

            // 모든 거래일이 현재 날짜보다 큰 경우, 첫 번째 거래일 반환
            warn!(
                "모든 거래일이 현재 날짜보다 큽니다. 첫 번째 거래일 사용: {}",
                trading_dates[0]
            );
            Ok(trading_dates[0].clone())
        }
    }
}

fn calculate_prev_day_range_ratio(
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> Result<f64> {
    // 주식 시장이 열린 날 기준으로 전일 계산
    let prev_date_str = match get_previous_trading_day(trading_dates, date) {
        Ok(date) => date,
        Err(e) => {
            warn!(
                "전일 날짜 계산 실패: {} (종목: {}, 날짜: {})",
                e, stock_code, date
            );
            return Ok(0.0);
        }
    };
    debug!("전일 날짜: {}", prev_date_str);

    // 테이블명 (일봉 DB는 A 접두사 포함)
    let table_name = stock_code;
    debug!("테이블명: {}", table_name);

    // 테이블 존재 여부 확인
    let table_exists: i64 = daily_db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            rusqlite::params![&table_name],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if table_exists == 0 {
        warn!(
            "일봉 테이블이 존재하지 않습니다: {} (종목: {})",
            table_name, stock_code
        );
        return Ok(0.0);
    }

    // 전일 데이터 존재 여부 확인
    let prev_data_exists: i64 = daily_db
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\" WHERE date = ?", table_name),
            rusqlite::params![&prev_date_str],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if prev_data_exists == 0 {
        warn!(
            "전일 데이터가 없습니다: {} (테이블: {}, 날짜: {})",
            stock_code, table_name, prev_date_str
        );
        return Ok(0.0);
    }

    // 전일 고가, 저가, 종가 조회
    let (prev_high, prev_low, prev_close): (f64, f64, f64) = match daily_db.query_row(
        &format!(
            "SELECT high, low, close FROM \"{}\" WHERE date = ?",
            table_name
        ),
        rusqlite::params![&prev_date_str],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ) {
        Ok(result) => result,
        Err(e) => {
            warn!(
                "전일 데이터 조회 실패: {} (테이블: {}, 날짜: {})",
                e, table_name, prev_date_str
            );
            return Ok(0.0);
        }
    };

    if prev_close <= 0.0 {
        warn!(
            "전일 종가가 유효하지 않습니다: {:.2} (종목: {})",
            prev_close, stock_code
        );
        return Ok(0.0);
    }

    // 전일 범위 비율 계산: (고가 - 저가) / 종가
    let prev_day_range_ratio = (prev_high - prev_low) / prev_close;

    Ok(prev_day_range_ratio)
}

fn calculate_prev_close_to_now_ratio(
    db: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    trading_dates: &[String],
) -> Result<f64> {
    // 주식 시장이 열린 날 기준으로 전일 계산
    let prev_date_str = match get_previous_trading_day(trading_dates, date) {
        Ok(date) => date,
        Err(e) => {
            warn!(
                "전일 날짜 계산 실패: {} (종목: {}, 날짜: {})",
                e, stock_code, date
            );
            return Ok(1.0);
        }
    };
    debug!("전일 날짜: {}", prev_date_str);

    // 테이블명 (일봉 DB는 A 접두사 포함)
    let table_name = stock_code;
    debug!("테이블명: {}", table_name);

    // 먼저 테이블 존재 여부 확인
    let table_exists: i64 = daily_db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            rusqlite::params![&table_name],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if table_exists == 0 {
        warn!(
            "일봉 테이블이 존재하지 않습니다: {} (종목: {})",
            table_name, stock_code
        );
        return Ok(1.0);
    }

    // 테이블의 실제 데이터 확인 (디버깅용)
    let sample_data: Vec<String> = daily_db
        .prepare(&format!(
            "SELECT date FROM \"{}\" ORDER BY date DESC LIMIT 5",
            table_name
        ))?
        .query_map([], |row| Ok(row.get::<_, String>(0)?))?
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
        .unwrap_or(0);

    if prev_data_exists == 0 {
        warn!(
            "전일 데이터가 없습니다: {} (테이블: {}, 날짜: {})",
            stock_code, table_name, prev_date_str
        );
        return Ok(1.0);
    }

    // 전일 종가 조회
    let prev_close: f64 = match daily_db.query_row(
        &format!("SELECT close FROM \"{}\" WHERE date = ?", table_name),
        rusqlite::params![&prev_date_str],
        |row| row.get(0),
    ) {
        Ok(close) => close,
        Err(e) => {
            warn!(
                "전일 종가 조회 실패: {} (테이블: {}, 날짜: {})",
                e, table_name, prev_date_str
            );
            return Ok(1.0);
        }
    };

    if prev_close <= 0.0 {
        warn!(
            "전일 종가가 유효하지 않습니다: {:.2} (종목: {})",
            prev_close, stock_code
        );
        return Ok(1.0);
    }

    // 당일 현재가 조회 (9시반 이전 5분봉 마지막 종가)
    let morning_data = match get_morning_data(db, stock_code, date) {
        Ok(data) => data,
        Err(e) => {
            warn!(
                "당일 9시반 이전 데이터 조회 실패: {} (종목: {})",
                e, stock_code
            );
            return Ok(1.0);
        }
    };

    let current_close = morning_data.get_last_close().unwrap_or(0.0);

    if current_close <= 0.0 {
        warn!(
            "당일 현재가가 유효하지 않습니다: {:.2} (종목: {})",
            current_close, stock_code
        );
        return Ok(1.0);
    }

    Ok(current_close / prev_close)
}

fn calculate_macd_histogram(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 4 {
        return Ok(0.0);
    }

    let ema3 = calculate_ema(&morning_data.closes, 3);
    let ema6 = calculate_ema(&morning_data.closes, 6);
    let macd = ema3 - ema6;

    Ok(macd)
}

fn calculate_sixth_derivative(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 6 {
        return Ok(0.0);
    }

    // 6차 도함수 근사 계산 (연속된 6개 점의 가격 변화 패턴)
    let mut direction_changes = 0;
    for i in 1..morning_data.closes.len() {
        if i > 1 {
            let prev_change = morning_data.closes[i - 1] - morning_data.closes[i - 2];
            let curr_change = morning_data.closes[i] - morning_data.closes[i - 1];
            if (prev_change > 0.0 && curr_change < 0.0) || (prev_change < 0.0 && curr_change > 0.0)
            {
                direction_changes += 1;
            }
        }
    }

    let derivative = direction_changes as f64 / (morning_data.closes.len() - 2) as f64;

    // 로그 정규화
    if derivative == 0.0 {
        Ok(0.0)
    } else {
        let sign = if derivative > 0.0 { 1.0 } else { -1.0 };
        Ok(sign * (1.0 + derivative.abs()).ln())
    }
}

fn calculate_pos_vs_high_5d(daily_db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    use chrono::{Duration, NaiveDate};

    // 날짜 파싱 (YYYYMMDD 형식)
    let year = date[..4].parse::<i32>().unwrap_or(2024);
    let month = date[4..6].parse::<u32>().unwrap_or(1);
    let day = date[6..8].parse::<u32>().unwrap_or(1);

    let target_date = match NaiveDate::from_ymd_opt(year, month, day) {
        Some(date) => date,
        None => {
            warn!("잘못된 날짜 형식: {}", date);
            return Ok(0.0);
        }
    };

    // 테이블명 (일봉 DB는 A 접두사 포함)
    let table_name = stock_code;
    debug!(
        "5일 고점 조회 - 종목: {}, 테이블: {}",
        stock_code, table_name
    );

    // 당일 현재가 조회 (9시반 이전 5분봉 데이터에서 마지막 종가)
    // 5분봉 데이터는 별도로 조회해야 하므로, 일봉 데이터에서 종가 사용
    let daily_data = get_daily_data(daily_db, stock_code, date)?;
    let current_price = daily_data.get_close().unwrap_or(0.0);

    // 최근 5일 고점 조회 (주식 시장이 열린 날 기준)
    let five_days_ago = target_date - Duration::days(5);
    let five_days_ago_str = five_days_ago.format("%Y%m%d").to_string();
    let target_date_str = target_date.format("%Y%m%d").to_string();
    debug!(
        "5일 고점 조회 범위: {} ~ {}",
        five_days_ago_str, target_date_str
    );

    let five_day_high: f64 = match daily_db.query_row(
        &format!(
            "SELECT MAX(high) FROM \"{}\" WHERE date >= ? AND date <= ?",
            table_name
        ),
        rusqlite::params![&five_days_ago_str, &target_date_str],
        |row| row.get(0),
    ) {
        Ok(high) => high,
        Err(e) => {
            warn!(
                "5일 고점 조회 실패: {} (테이블: {}, 범위: {} ~ {})",
                e, table_name, five_days_ago_str, target_date_str
            );
            return Ok(0.0);
        }
    };

    if five_day_high <= 0.0 {
        warn!("5일 고점이 유효하지 않습니다: {:.2}", five_day_high);
        return Ok(0.0);
    }

    Ok(current_price / five_day_high)
}

fn calculate_rsi_value(db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    let morning_data = get_morning_data(db, stock_code, date)?;

    if morning_data.closes.len() < 6 {
        return Ok(50.0);
    }

    let mut gains = Vec::new();
    let mut losses = Vec::new();

    for i in 1..morning_data.closes.len() {
        let change = morning_data.closes[i] - morning_data.closes[i - 1];
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

    let rsi = if avg_loss > 0.0 {
        100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
    } else {
        100.0
    };

    Ok(rsi)
}

fn calculate_pos_vs_high_3d(daily_db: &Connection, stock_code: &str, date: &str) -> Result<f64> {
    use chrono::{Duration, NaiveDate};

    // 날짜 파싱 (YYYYMMDD 형식)
    let year = date[..4].parse::<i32>().unwrap_or(2024);
    let month = date[4..6].parse::<u32>().unwrap_or(1);
    let day = date[6..8].parse::<u32>().unwrap_or(1);

    let target_date = match NaiveDate::from_ymd_opt(year, month, day) {
        Some(date) => date,
        None => {
            warn!("잘못된 날짜 형식: {}", date);
            return Ok(0.0);
        }
    };

    // 테이블명 (일봉 DB는 A 접두사 포함)
    let table_name = stock_code;
    debug!(
        "3일 고점 조회 - 종목: {}, 테이블: {}",
        stock_code, table_name
    );

    // 당일 현재가 조회 (일봉 데이터에서 종가 사용)
    let daily_data = get_daily_data(daily_db, stock_code, date)?;
    let current_price = daily_data.get_close().unwrap_or(0.0);

    // 최근 3일 고점 조회 (주식 시장이 열린 날 기준)
    let three_days_ago = target_date - Duration::days(3);
    let three_days_ago_str = three_days_ago.format("%Y%m%d").to_string();
    let target_date_str = target_date.format("%Y%m%d").to_string();
    debug!(
        "3일 고점 조회 범위: {} ~ {}",
        three_days_ago_str, target_date_str
    );

    let three_day_high: f64 = match daily_db.query_row(
        &format!(
            "SELECT MAX(high) FROM \"{}\" WHERE date >= ? AND date <= ?",
            table_name
        ),
        rusqlite::params![&three_days_ago_str, &target_date_str],
        |row| row.get(0),
    ) {
        Ok(high) => high,
        Err(e) => {
            warn!(
                "3일 고점 조회 실패: {} (테이블: {}, 범위: {} ~ {})",
                e, table_name, three_days_ago_str, target_date_str
            );
            return Ok(0.0);
        }
    };

    if three_day_high <= 0.0 {
        warn!("3일 고점이 유효하지 않습니다: {:.2}", three_day_high);
        return Ok(0.0);
    }

    Ok(current_price / three_day_high)
}

// EMA 계산 함수
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

// ONNX 모델 로딩 함수
fn load_onnx_model(model_info_path: &str) -> Result<ONNXPredictor, Box<dyn std::error::Error>> {
    // 모델 정보 파일 존재 확인
    if !Path::new(model_info_path).exists() {
        return Err(format!(
            "ONNX 모델 정보 파일을 찾을 수 없습니다: {}",
            model_info_path
        )
        .into());
    }

    // 모델 정보 로드
    let model_info_file = File::open(model_info_path)?;
    let model_info: ONNXModelInfo = serde_json::from_reader(model_info_file)?;

    debug!("ONNX 모델 정보 로드: {}개 특성", model_info.feature_count);

    // ONNX 모델 파일 존재 확인
    if !Path::new(&model_info.onnx_model_path).exists() {
        return Err(format!(
            "ONNX 모델 파일을 찾을 수 없습니다: {}",
            model_info.onnx_model_path
        )
        .into());
    }

    // ONNX Runtime 환경 초기화
    let environment = Arc::new(
        Environment::builder()
            .with_name("solomon_predictor")
            .build()?,
    );

    // 세션 생성
    let session =
        SessionBuilder::new(&environment)?.with_model_from_file(&model_info.onnx_model_path)?;

    debug!("ONNX 모델 로드 완료: {}", model_info.onnx_model_path);
    debug!(
        "입력 이름: {}, 출력 이름: {}",
        model_info.input_name, model_info.output_name
    );

    Ok(ONNXPredictor {
        session,
        features: model_info.features,
        input_name: model_info.input_name,
        output_name: model_info.output_name,
    })
}

// ONNX 모델로 예측 수행
fn predict_with_onnx_model(
    predictor: &ONNXPredictor,
    features_data: &[StockFeatures],
) -> Result<Vec<PredictionResult>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();

    debug!("=== ONNX 모델 예측 디버깅 정보 ===");
    debug!("입력 특징 수: {}", predictor.features.len());
    debug!("입력 이름: {}", predictor.input_name);
    debug!("출력 이름: {}", predictor.output_name);
    debug!("예측할 종목 수: {}", features_data.len());
    debug!("");

    for (idx, stock_data) in features_data.iter().enumerate() {
        debug!(
            "--- 종목 {} 예측 중 ({}/{}) ---",
            stock_data.stock_code,
            idx + 1,
            features_data.len()
        );

        // 1. 특성 벡터를 f32 배열로 변환 (NaN이나 무한대 값 처리)
        let input_vec: Vec<f32> = stock_data
            .features
            .iter()
            .map(|&x| {
                let val = x as f32;
                if val.is_nan() || val.is_infinite() {
                    0.0
                } else {
                    val
                }
            })
            .collect();

        debug!("입력 특징 벡터 (처리 후): {:?}", input_vec);
        debug!("입력 특징 벡터 길이: {}", input_vec.len());

        // 2. ndarray 배열로 변환 (배치 1개, 특성 수만큼)
        let input_array = Array2::from_shape_vec((1, input_vec.len()), input_vec)?;
        debug!("입력 배열 형태: {:?}", input_array.shape());

        // 3. ONNX 텐서 생성 (v1.x API 사용)
        use ndarray::CowArray;
        let input_dyn = input_array.into_dyn();
        let input_cow = CowArray::from(input_dyn);

        // 입력 텐서 생성 시 명시적으로 f32 타입 지정
        let input_tensor = Value::from_array(
            &predictor.session.allocator() as *const _ as *mut _,
            &input_cow,
        )?;
        debug!("입력 텐서 생성 완료");

        // 4. 예측 수행
        debug!("ONNX 모델 실행 중...");
        let outputs = predictor.session.run(vec![input_tensor])?;
        debug!("ONNX 모델 실행 완료, 출력 개수: {}", outputs.len());

        // 5. 모든 출력 텐서 출력
        debug!("=== 모든 출력 텐서 내용 ===");
        for (i, output_value) in outputs.iter().enumerate() {
            debug!("출력 텐서 {}:", i);

            // f32로 시도
            if let Ok(output_tensor) = output_value.try_extract::<f32>() {
                let view = output_tensor.view();
                let slice = view.as_slice().unwrap_or(&[]);
                debug!("  f32 슬라이스: {:?}", slice);
                debug!("  f32 슬라이스 길이: {}", slice.len());
            }

            // i64로 시도
            if let Ok(output_tensor) = output_value.try_extract::<i64>() {
                let view = output_tensor.view();
                let slice = view.as_slice().unwrap_or(&[]);
                debug!("  i64 슬라이스: {:?}", slice);
                debug!("  i64 슬라이스 길이: {}", slice.len());
            }

            // f64로 시도
            if let Ok(output_tensor) = output_value.try_extract::<f64>() {
                let view = output_tensor.view();
                let slice = view.as_slice().unwrap_or(&[]);
                debug!("  f64 슬라이스: {:?}", slice);
                debug!("  f64 슬라이스 길이: {}", slice.len());
            }
            debug!("");
        }

        // 두 번째 출력에서 확률 추출 (실제 확률 값이 있는 텐서)
        let output_value = &outputs[1];
        debug!("=== 확률 추출 (두 번째 출력 사용) ===");

        // 먼저 f32로 시도
        let probability = if let Ok(output_tensor) = output_value.try_extract::<f32>() {
            debug!("  - f32 타입으로 추출 성공");
            let view = output_tensor.view();
            let slice = view.as_slice().unwrap_or(&[]);
            debug!("  - f32 슬라이스: {:?}", slice);
            debug!("  - f32 슬라이스 길이: {}", slice.len());

            if slice.len() >= 2 {
                // 두 번째 값이 양성 클래스(1)의 확률
                let prob = slice[1] as f64;
                debug!("  - 선택된 확률 (인덱스 1): {:.6}", prob);
                prob
            } else if slice.len() == 1 {
                // 단일 값인 경우 그대로 사용
                let prob = slice[0] as f64;
                debug!("  - 선택된 확률 (인덱스 0): {:.6}", prob);
                prob
            } else {
                debug!("  - 슬라이스가 비어있음, 기본값 0.5 사용");
                0.5
            }
        } else if let Ok(output_tensor) = output_value.try_extract::<i64>() {
            debug!("  - i64 타입으로 추출 성공");
            let view = output_tensor.view();
            let slice = view.as_slice().unwrap_or(&[]);
            debug!("  - i64 슬라이스: {:?}", slice);
            debug!("  - i64 슬라이스 길이: {}", slice.len());

            if slice.len() >= 2 {
                // 두 번째 값이 양성 클래스(1)의 확률
                let prob = slice[1] as f64;
                debug!("  - 선택된 확률 (인덱스 1): {:.6}", prob);
                prob
            } else if slice.len() == 1 {
                // 단일 값인 경우 그대로 사용
                let prob = slice[0] as f64;
                debug!("  - 선택된 확률 (인덱스 0): {:.6}", prob);
                prob
            } else {
                debug!("  - 슬라이스가 비어있음, 기본값 0.5 사용");
                0.5
            }
        } else if let Ok(output_tensor) = output_value.try_extract::<f64>() {
            debug!("  - f64 타입으로 추출 성공");
            let view = output_tensor.view();
            let slice = view.as_slice().unwrap_or(&[]);
            debug!("  - f64 슬라이스: {:?}", slice);
            debug!("  - f64 슬라이스 길이: {}", slice.len());

            if slice.len() >= 2 {
                // 두 번째 값이 양성 클래스(1)의 확률
                let prob = slice[1];
                debug!("  - 선택된 확률 (인덱스 1): {:.6}", prob);
                prob
            } else if slice.len() == 1 {
                // 단일 값인 경우 그대로 사용
                let prob = slice[0];
                debug!("  - 선택된 확률 (인덱스 0): {:.6}", prob);
                prob
            } else {
                debug!("  - 슬라이스가 비어있음, 기본값 0.5 사용");
                0.5
            }
        } else {
            info!("  - 모든 데이터 타입으로 텐서 추출 실패");
            warn!(
                "모든 데이터 타입으로 텐서 추출 실패 (종목: {})",
                stock_data.stock_code
            );
            0.5
        };

        let probability = probability.clamp(0.0, 1.0);
        info!("최종 확률 (클램핑 후): {:.6}", probability);

        results.push(PredictionResult {
            stock_code: stock_data.stock_code.clone(),
            probability,
        });

        info!(
            "종목 {} 예측 완료: {:.6}",
            stock_data.stock_code, probability
        );
        debug!("");
    }

    info!("=== ONNX 모델 예측 완료 ===");
    info!("총 예측 종목 수: {}개", results.len());

    // 결과 요약 출력
    if !results.is_empty() {
        let max_prob = results.iter().map(|r| r.probability).fold(0.0, f64::max);
        let min_prob = results.iter().map(|r| r.probability).fold(1.0, f64::min);
        let avg_prob = results.iter().map(|r| r.probability).sum::<f64>() / results.len() as f64;

        info!("확률 통계:");
        info!("  - 최대 확률: {:.6}", max_prob);
        info!("  - 최소 확률: {:.6}", min_prob);
        info!("  - 평균 확률: {:.6}", avg_prob);
    }
    debug!("");

    debug!("ONNX 모델 예측 완료: {}개 종목", results.len());
    Ok(results)
}

fn check_date_exists_in_samsung(db: &Connection, date: &str) -> Result<bool> {
    let table_name = "A005930"; // 삼성전자 종목코드 (5분봉 DB)
    let date_str = format!("{}%", date); // 5분봉 DB는 YYYYMMDDHHMM 형식이므로 LIKE 사용

    let count: i64 = db.query_row(
        &format!("SELECT COUNT(*) FROM \"{}\" WHERE date LIKE ?", table_name),
        rusqlite::params![&date_str],
        |row| row.get(0),
    )?;

    Ok(count > 0)
}

fn get_latest_available_date(db: &Connection) -> Result<String> {
    let table_name = "A005930"; // 삼성전자 종목코드 (5분봉 DB)

    let latest_date: i64 = db.query_row(
        &format!("SELECT MAX(date/10000) FROM \"{}\"", table_name),
        [],
        |row| row.get(0),
    )?;

    Ok(latest_date.to_string())
}

// 일봉 데이터를 조회하는 함수 추가
fn get_daily_data(daily_db: &Connection, stock_code: &str, date: &str) -> Result<DailyData> {
    let table_name = stock_code.to_string();

    // 먼저 테이블 존재 여부 확인
    let table_exists: i64 = daily_db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            rusqlite::params![&table_name],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if table_exists == 0 {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "일봉 테이블이 존재하지 않습니다: {} (종목: {})",
            table_name, stock_code
        )));
    }

    // 해당 날짜의 데이터 존재 여부 확인
    let data_exists: i64 = daily_db
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\" WHERE date = ?", table_name),
            rusqlite::params![&date],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if data_exists == 0 {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "전일 데이터가 없습니다: {} (테이블: {}, 날짜: {})",
            stock_code, table_name, date
        )));
    }

    let query = format!(
        "SELECT close, open, high, low FROM \"{}\" WHERE date = ?",
        table_name
    );

    let mut stmt = daily_db.prepare(&query)?;
    let rows = stmt.query_map([&date], |row| {
        Ok((
            row.get::<_, i32>(0)?, // close
            row.get::<_, i32>(1)?, // open
            row.get::<_, i32>(2)?, // high
            row.get::<_, i32>(3)?, // low
        ))
    })?;

    let mut closes = Vec::new();

    for row in rows {
        let (close, _, _, _) = row?;
        closes.push(close as f64);
    }

    Ok(DailyData { closes })
}

// 일봉 데이터 구조체
#[derive(Debug)]
struct DailyData {
    closes: Vec<f64>,
}

impl DailyData {
    fn get_close(&self) -> Option<f64> {
        self.closes.first().copied()
    }
}

// 주식 예측 시스템의 메인 로직을 캡슐화한 구조체
struct StockPredictor {
    db: Connection,
    daily_db: Connection,
    predictor: ONNXPredictor,
    features: Vec<String>,
    trading_dates: Vec<String>,
    extra_stocks_set: std::collections::HashSet<String>,
}

impl StockPredictor {
    fn new(date: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // 데이터베이스 연결
        let stock_db_path = std::env::var("STOCK_DB_PATH")
            .unwrap_or_else(|_| "D:\\db\\stock_price(5min).db".to_string());
        let daily_db_path = std::env::var("DAILY_DB_PATH")
            .unwrap_or_else(|_| "D:\\db\\stock_price(1day)_with_data.db".to_string());

        let db = Connection::open(&stock_db_path)?;
        let daily_db = Connection::open(&daily_db_path)?;

        // 성능 최적화 설정
        Self::optimize_database(&db)?;
        Self::optimize_database(&daily_db)?;

        debug!("데이터베이스 연결 성공: {}", stock_db_path);

        // 일봉 데이터베이스의 테이블 확인 (디버깅용)
        Self::check_daily_db_tables(&daily_db)?;

        // 거래일 리스트 로드
        let trading_dates = get_trading_dates_list(&db)?;
        debug!("거래일 리스트 로드 완료: {}개", trading_dates.len());

        // 날짜 유효성 검증
        let target_date = Self::validate_and_get_target_date(&db, date)?;
        debug!("분석 대상 날짜: {}", target_date);

        // ONNX 모델 로드
        let model_info_path = std::env::var("ONNX_MODEL_INFO_PATH")
            .unwrap_or_else(|_| "models/rust_model_info.json".to_string());

        debug!("ONNX 모델 로딩 중...");
        let predictor = load_onnx_model(&model_info_path)?;

        // extra_stocks.txt 로드
        let extra_stocks = load_extra_stocks()?;
        let extra_stocks_set: std::collections::HashSet<_> = extra_stocks.into_iter().collect();

        // features.txt 로드
        let features = load_features()?;
        debug!("로드된 특징들: {:?}", features);

        Ok(StockPredictor {
            db,
            daily_db,
            predictor,
            features,
            trading_dates,
            extra_stocks_set,
        })
    }

    fn check_daily_db_tables(db: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        let tables: Vec<String> = db
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )?
            .query_map([], |row| Ok(row.get::<_, String>(0)?))?
            .filter_map(|r| r.ok())
            .collect();

        debug!("일봉 데이터베이스 테이블 수: {}개", tables.len());

        // 처음 10개 테이블만 출력
        let sample_tables: Vec<&String> = tables.iter().take(10).collect();
        debug!("일봉 데이터베이스 샘플 테이블들: {:?}", sample_tables);

        // A005930 테이블이 있는지 확인
        if tables.contains(&"A005930".to_string()) {
            debug!("A005930 테이블이 일봉 데이터베이스에 존재합니다.");

            // A005930 테이블의 최근 데이터 확인
            let recent_dates: Vec<String> = db
                .prepare("SELECT date FROM \"A005930\" ORDER BY date DESC LIMIT 5")?
                .query_map([], |row| Ok(row.get::<_, String>(0)?))?
                .filter_map(|r| r.ok())
                .collect();

            debug!("A005930 테이블의 최근 5개 날짜: {:?}", recent_dates);
        } else {
            warn!("A005930 테이블이 일봉 데이터베이스에 존재하지 않습니다.");
        }

        Ok(())
    }

    fn optimize_database(db: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        let _: String = db.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        db.execute("PRAGMA synchronous = NORMAL", [])?;
        db.execute("PRAGMA cache_size = 10000", [])?;
        db.execute("PRAGMA temp_store = MEMORY", [])?;
        Ok(())
    }

    fn validate_and_get_target_date(
        db: &Connection,
        date: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let date_exists = check_date_exists_in_samsung(db, date)?;
        if !date_exists {
            warn!("삼성전자 주식 5분봉에 날짜 {}가 존재하지 않습니다.", date);
            let latest_date = get_latest_available_date(db)?;
            warn!("최신 사용 가능한 날짜 {}를 사용합니다.", latest_date);
            return Ok(latest_date);
        }
        Ok(date.to_string())
    }

    fn predict_top_stocks(
        &self,
        date: &str,
    ) -> Result<Vec<PredictionResult>, Box<dyn std::error::Error>> {
        // 거래대금 상위 30개 종목 조회
        let top_stocks = get_top_volume_stocks(&self.db, date, 30)?;
        debug!("거래대금 상위 30개 종목: {:?}", top_stocks);

        // extra_stocks.txt에 없는 종목들만 필터링
        let filtered_stocks: Vec<String> = top_stocks
            .into_iter()
            .filter(|stock| !self.extra_stocks_set.contains(stock))
            .collect();

        debug!("필터링된 종목 수: {}개", filtered_stocks.len());
        debug!("필터링된 종목들: {:?}", filtered_stocks);

        if filtered_stocks.is_empty() {
            warn!("분석할 종목이 없습니다.");
            return Ok(Vec::new());
        }

        // 각 종목에 대해 특징 계산
        let features_data = self.calculate_features_for_stocks(&filtered_stocks, date)?;

        if features_data.is_empty() {
            warn!("계산된 특징이 없습니다.");
            return Ok(Vec::new());
        }

        // ONNX 모델로 예측
        debug!("ONNX 모델로 예측 시작...");
        let mut predictions = predict_with_onnx_model(&self.predictor, &features_data)?;

        // 결과 정렬 (확률 높은 순)
        predictions.sort_by(|a, b| b.probability.partial_cmp(&a.probability).unwrap());

        Ok(predictions)
    }

    fn calculate_features_for_stocks(
        &self,
        stocks: &[String],
        date: &str,
    ) -> Result<Vec<StockFeatures>, Box<dyn std::error::Error>> {
        let mut features_data = Vec::new();

        for stock_code in stocks {
            match calculate_features_for_stock_optimized(
                &self.db,
                &self.daily_db,
                stock_code,
                date,
                &self.features,
                &self.trading_dates,
            ) {
                Ok(feature_values) => {
                    features_data.push(StockFeatures {
                        stock_code: stock_code.clone(),
                        features: feature_values,
                    });
                    debug!("종목 {} 특징 계산 완료", stock_code);
                }
                Err(e) => {
                    warn!(
                        "종목 {} 특징 계산 실패: {} - 기본값으로 진행",
                        stock_code, e
                    );
                    // 실패한 경우에도 기본값으로 특징 계산을 시도
                    let default_features = vec![0.0; self.features.len()];
                    features_data.push(StockFeatures {
                        stock_code: stock_code.clone(),
                        features: default_features,
                    });
                }
            }
        }

        Ok(features_data)
    }

    fn print_results(&self, predictions: &[PredictionResult], date: &str) {
        println!("\n=== 예측 결과 ===");
        println!("날짜: {}", date);
        println!("분석 종목 수: {}개", predictions.len());
        println!();

        for (i, prediction) in predictions.iter().enumerate() {
            println!(
                "{}. {}: {:.4} ({:.2}%)",
                i + 1,
                prediction.stock_code,
                prediction.probability,
                prediction.probability * 100.0
            );
        }

        // 최고 확률 종목 출력
        if let Some(best_stock) = predictions.first() {
            println!("\n=== 최고 확률 종목 ===");
            println!("종목코드: {}", best_stock.stock_code);
            println!(
                "확률: {:.4} ({:.2}%)",
                best_stock.probability,
                best_stock.probability * 100.0
            );
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    solomon::init_tracing();

    // 명령행 인수 처리
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("사용법: {} <날짜>", args[0]);
        eprintln!("예시: {} 20241201", args[0]);
        std::process::exit(1);
    }

    let date = &args[1];
    debug!("분석 날짜: {}", date);

    // 주식 예측 시스템 초기화
    let predictor = StockPredictor::new(date)?;

    // 예측 수행
    let predictions = predictor.predict_top_stocks(date)?;

    // 결과 출력
    predictor.print_results(&predictions, date);

    Ok(())
}
