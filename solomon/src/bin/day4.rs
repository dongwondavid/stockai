use chrono::NaiveDate;
use rusqlite::{Connection, Result};
use log::{info, warn, error, debug};
use indicatif::{ProgressBar, ProgressStyle};
use solomon::SectorManager;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

// 데이터베이스 연결 풀 구조체
struct DbPool {
    solomon_db: Connection,
    stock_db: Connection,
}

// 캐시 구조체들
#[derive(Default)]
struct Cache {
    vwap_cache: HashMap<String, f64>,
    rsi_cache: HashMap<String, f64>,
    bollinger_cache: HashMap<String, (f64, f64, f64)>, // (upper, middle, lower)
}

fn main() {
    // 로거 초기화
    env_logger::init();
    info!("Day4 특징 계산 프로그램 시작");

    // 섹터 정보 로드
    let mut sector_manager = SectorManager::new();
    if let Err(e) = sector_manager.load_from_csv("sector_utf8.csv") {
        warn!("섹터 정보 로드 실패: {}. 기본값으로 진행합니다.", e);
    } else {
        debug!("섹터 정보 로드 완료");
    }

    // 데이터베이스 연결 풀 생성
    let mut db_pool = match create_db_pool() {
        Ok(pool) => pool,
        Err(e) => {
            error!("데이터베이스 연결 풀 생성 실패: {}", e);
            return;
        }
    };

    let stock_list = match get_stock_list_from_answer(&db_pool.solomon_db) {
        Ok(list) => list,
        Err(e) => {
            error!("answer 테이블에서 데이터 조회 실패: {}", e);
            return;
        }
    };
    info!("총 {}개 종목에 대해 Day4 특징을 계산합니다.", stock_list.len());

    if stock_list.is_empty() {
        error!("answer 테이블에서 데이터를 가져오지 못했습니다. 테이블이 비어있거나 경로가 잘못되었을 수 있습니다.");
        return;
    }

    // 기존 day4 테이블이 있다면 삭제하고 새로 생성
    db_pool.solomon_db.execute("DROP TABLE IF EXISTS day4", []).unwrap();
    create_day4_table(&db_pool.solomon_db).unwrap();
    debug!("Day4 테이블 준비 완료");

    // 날짜별로 종목들을 그룹화
    let mut date_groups: HashMap<NaiveDate, Vec<Day4StockInfo>> = HashMap::new();
    for stock_info in stock_list {
        date_groups.entry(stock_info.date).or_insert_with(Vec::new).push(stock_info);
    }

    info!("총 {}개 날짜로 그룹화되었습니다.", date_groups.len());

    // 캐시 초기화
    let cache = Arc::new(Mutex::new(Cache::default()));

    // 진행 상황 표시를 위한 ProgressBar 설정
    let total_stocks: usize = date_groups.values().map(|stocks| stocks.len()).sum();
    let pb = ProgressBar::new(total_stocks as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars("#>-"));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let mut processed_count = 0;
    let mut batch_features = Vec::new();

    // 배치 크기 설정
    const BATCH_SIZE: usize = 100;

    // 날짜별로 처리
    for (date, stocks) in date_groups {
        pb.set_message(format!("날짜 {} 처리 중: {}개 종목", date.format("%Y-%m-%d"), stocks.len()));
        
        for stock_info in stocks {
            pb.set_message(format!("처리 중: {} ({})", stock_info.stock_code, stock_info.date));
            
            match calculate_day4_features_optimized(&stock_info, &sector_manager, &db_pool, &cache) {
                Ok(features) => {
                    batch_features.push((stock_info.clone(), features));
                    
                    // 배치 크기에 도달하면 일괄 저장
                    if batch_features.len() >= BATCH_SIZE {
                        if let Err(e) = save_day4_features_batch(&mut db_pool.solomon_db, &batch_features) {
                            error!("배치 저장 중 오류 발생: {}", e);
                        } else {
                            processed_count += batch_features.len();
                        }
                        batch_features.clear();
                    }
                }
                Err(e) => {
                    error!("{} 종목 처리 중 오류 발생: {}", stock_info.stock_code, e);
                }
            }
            
            pb.inc(1);
        }
    }

    // 남은 배치 저장
    if !batch_features.is_empty() {
        if let Err(e) = save_day4_features_batch(&mut db_pool.solomon_db, &batch_features) {
            error!("최종 배치 저장 중 오류 발생: {}", e);
        } else {
            processed_count += batch_features.len();
        }
    }

    pb.finish_with_message("Day4 특징 계산 완료!");
    info!("=== Day4 특징 계산 결과 ===");
    info!("처리된 종목 수: {}", processed_count);
    info!("결과가 solomon.db의 day4 테이블에 저장되었습니다.");
    info!("Day4 특징 계산 프로그램 종료");
}

fn create_db_pool() -> Result<DbPool> {
    let solomon_db_path = std::env::var("SOLOMON_DB_PATH").unwrap_or_else(|_| "D:\\db\\solomon.db".to_string());
    let stock_db_path = std::env::var("STOCK_DB_PATH").unwrap_or_else(|_| "D:\\db\\stock_price(5min).db".to_string());

    let solomon_db = Connection::open(&solomon_db_path)?;
    let stock_db = Connection::open(&stock_db_path)?;

    // 성능 최적화 설정
    let _: String = solomon_db.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    solomon_db.execute("PRAGMA synchronous = NORMAL", [])?;
    solomon_db.execute("PRAGMA cache_size = 10000", [])?;
    solomon_db.execute("PRAGMA temp_store = MEMORY", [])?;

    let _: String = stock_db.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    stock_db.execute("PRAGMA synchronous = NORMAL", [])?;
    stock_db.execute("PRAGMA cache_size = 10000", [])?;
    stock_db.execute("PRAGMA temp_store = MEMORY", [])?;

    Ok(DbPool {
        solomon_db,
        stock_db,
    })
}

fn calculate_day4_features_optimized(
    stock_info: &Day4StockInfo,
    sector_manager: &SectorManager,
    db_pool: &DbPool,
    cache: &Arc<Mutex<Cache>>,
) -> Result<Day4Features> {
    use chrono::Duration;
    
    // 오늘 9:30까지의 5분봉 데이터
    let today_data = sector_manager.get_five_min_data(&db_pool.stock_db, &stock_info.stock_code, stock_info.date)?;
    
    // 전일 데이터 (D+1 종가 계산용)
    let prev_date = stock_info.date - Duration::days(1);
    let prev_data = sector_manager.get_five_min_data(&db_pool.stock_db, &stock_info.stock_code, prev_date)?;
    
    // 전전일 데이터 (D+2 종가 계산용)
    let prev_prev_date = stock_info.date - Duration::days(2);
    let prev_prev_data = sector_manager.get_five_min_data(&db_pool.stock_db, &stock_info.stock_code, prev_prev_date)?;

    // 1. 모멘텀 & 추세
    let rsi_above_50 = calculate_rsi_above_50(&today_data, &stock_info.stock_code, cache)?;
    let short_macd_cross_signal = calculate_short_macd_cross_signal(&today_data, &stock_info.stock_code, cache)?;
    
    // RSI 파생 특징들
    let rsi_value = calculate_rsi_value(&today_data, &stock_info.stock_code, cache)?;
    let (rsi_overbought, rsi_oversold) = calculate_rsi_extremes(&today_data, &stock_info.stock_code, cache)?;
    
    // MACD 파생 특징들
    let macd_histogram = calculate_macd_histogram(&today_data, &stock_info.stock_code, cache)?;
    let macd_histogram_increasing = calculate_macd_histogram_increasing(&today_data, &stock_info.stock_code, cache)?;

    // 2. 가격 위치 & 추세 유지
    let open_to_now_return = calculate_open_to_now_return(&today_data);
    let pos_vs_high_3d = calculate_pos_vs_high_3d(&today_data, &prev_data, &prev_prev_data);
    let pos_vs_high_5d = calculate_pos_vs_high_5d(&today_data, &prev_data, &prev_prev_data);
    let pos_vs_high_10d = calculate_pos_vs_high_10d(&today_data, &prev_data, &prev_prev_data);

    // 3. 변동성 & 양봉 흐름
    let consecutive_bull_count = calculate_consecutive_bull_count(&today_data);
    let is_highest_volume_bull_candle = calculate_is_highest_volume_bull_candle(&today_data);

    // 4. 거래량 관련
    let high_volume_early_count = calculate_high_volume_early_count(&today_data, &stock_info.stock_code, cache)?;

    // 5. 캔들 패턴
    let is_long_bull_candle = calculate_is_long_bull_candle(&today_data);
    let is_bullish_engulfing = calculate_is_bullish_engulfing(&today_data);
    let is_bearish_engulfing = calculate_is_bearish_engulfing(&today_data);
    let is_morning_star = calculate_is_morning_star(&today_data);
    let is_evening_star = calculate_is_evening_star(&today_data);
    let is_hammer = calculate_is_hammer(&today_data);

    // 6. VWAP 기반
    let vwap_support_check = calculate_vwap_support_check(&prev_data);
    let (vwap_vs_high, vwap_vs_low) = calculate_vwap_vs_extremes(&today_data, &stock_info.stock_code, cache)?;

    // 7. Bollinger Band 기반
    let (bollinger_band_width, bollinger_position, is_breaking_upper_band) = 
        calculate_bollinger_features(&today_data, &stock_info.stock_code, cache)?;

    Ok(Day4Features {
        rsi_above_50,
        short_macd_cross_signal,
        rsi_value,
        rsi_overbought,
        rsi_oversold,
        macd_histogram,
        macd_histogram_increasing,
        open_to_now_return,
        pos_vs_high_3d,
        pos_vs_high_5d,
        pos_vs_high_10d,
        consecutive_bull_count,
        is_highest_volume_bull_candle,
        high_volume_early_count,
        is_long_bull_candle,
        is_bullish_engulfing,
        is_bearish_engulfing,
        is_morning_star,
        is_evening_star,
        is_hammer,
        vwap_support_check,
        vwap_vs_high,
        vwap_vs_low,
        bollinger_band_width,
        bollinger_position,
        is_breaking_upper_band,
    })
}

// RSI 계산 (6기간) - 첫 번째 5분봉의 open을 포함하여 7개 데이터 포인트 사용
fn calculate_rsi_above_50(data: &[solomon::FiveMinData], stock_code: &str, cache: &Arc<Mutex<Cache>>) -> Result<i32> {
    let cache_key = format!("{}_rsi", stock_code);
    
    // 캐시 확인
    {
        let cache_guard = cache.lock().unwrap();
        if let Some(&rsi) = cache_guard.rsi_cache.get(&cache_key) {
            return Ok(if rsi >= 50.0 { 1 } else { 0 });
        }
    }
    
    if data.len() < 6 {
        return Ok(0);
    }
    
    // 첫 번째 5분봉의 open을 포함하여 7개 데이터 포인트 생성
    let mut prices = Vec::new();
    if !data.is_empty() {
        prices.push(data[0].open as f64); // 첫 번째 5분봉의 open
    }
    for d in data {
        prices.push(d.close as f64); // 각 5분봉의 close
    }
    
    if prices.len() < 7 {
        return Ok(0);
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
    
    // 6기간 평균 계산
    let avg_gain = gains.iter().sum::<f64>() / gains.len() as f64;
    let avg_loss = losses.iter().sum::<f64>() / losses.len() as f64;
    
    let rsi = if avg_loss > 0.0 {
        100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
    } else {
        100.0
    };
    
    // 캐시에 저장
    {
        let mut cache_guard = cache.lock().unwrap();
        cache_guard.rsi_cache.insert(cache_key, rsi);
    }
    
    Ok(if rsi >= 50.0 { 1 } else { 0 })
}

// 단축 MACD (3,6) 시그널선 상향 돌파 - 더 간단하고 실용적인 접근
fn calculate_short_macd_cross_signal(data: &[solomon::FiveMinData], _stock_code: &str, _cache: &Arc<Mutex<Cache>>) -> Result<i32> {
    if data.len() < 4 {
        return Ok(0);
    }
    
    // 가격 데이터 준비 (시가 + 종가들)
    let mut prices = Vec::new();
    if !data.is_empty() {
        prices.push(data[0].open as f64); // 첫 번째 5분봉의 open
    }
    for d in data {
        prices.push(d.close as f64); // 각 5분봉의 close
    }
    
    if prices.len() < 5 {
        return Ok(0);
    }
    
    // 현재 MACD 계산
    let ema3 = calculate_ema(&prices, 3);
    let ema6 = calculate_ema(&prices, 6);
    let macd = ema3 - ema6;
    
    // 이전 MACD 계산 (마지막 가격 제외)
    if prices.len() >= 6 {
        let prev_prices: Vec<f64> = prices[..prices.len()-1].to_vec();
        let prev_ema3 = calculate_ema(&prev_prices, 3);
        let prev_ema6 = calculate_ema(&prev_prices, 6);
        let prev_macd = prev_ema3 - prev_ema6;
        
        // MACD가 음수에서 양수로 전환되거나, 증가 추세에서 상향 돌파하는 경우
        if (macd > 0.0 && prev_macd <= 0.0) || (macd > prev_macd && macd > 0.0) {
            return Ok(1);
        }
    }
    
    Ok(0)
}

// RSI 값 계산 (0-100 범위)
fn calculate_rsi_value(data: &[solomon::FiveMinData], stock_code: &str, cache: &Arc<Mutex<Cache>>) -> Result<f64> {
    let cache_key = format!("{}_rsi", stock_code);
    
    // 캐시 확인
    {
        let cache_guard = cache.lock().unwrap();
        if let Some(&rsi) = cache_guard.rsi_cache.get(&cache_key) {
            return Ok(rsi);
        }
    }
    
    if data.len() < 6 {
        return Ok(50.0); // 기본값
    }
    
    // 첫 번째 5분봉의 open을 포함하여 7개 데이터 포인트 생성
    let mut prices = Vec::new();
    if !data.is_empty() {
        prices.push(data[0].open as f64); // 첫 번째 5분봉의 open
    }
    for d in data {
        prices.push(d.close as f64); // 각 5분봉의 close
    }
    
    if prices.len() < 7 {
        return Ok(50.0); // 기본값
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
    
    // 6기간 평균 계산
    let avg_gain = gains.iter().sum::<f64>() / gains.len() as f64;
    let avg_loss = losses.iter().sum::<f64>() / losses.len() as f64;
    
    let rsi = if avg_loss > 0.0 {
        100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
    } else {
        100.0
    };
    
    // 캐시에 저장
    {
        let mut cache_guard = cache.lock().unwrap();
        cache_guard.rsi_cache.insert(cache_key, rsi);
    }
    
    Ok(rsi)
}

// RSI 과매수/과매도 구간 확인
fn calculate_rsi_extremes(data: &[solomon::FiveMinData], stock_code: &str, cache: &Arc<Mutex<Cache>>) -> Result<(i32, i32)> {
    let rsi = calculate_rsi_value(data, stock_code, cache)?;
    
    let is_overbought = if rsi >= 70.0 { 1 } else { 0 };
    let is_oversold = if rsi <= 30.0 { 1 } else { 0 };
    
    Ok((is_overbought, is_oversold))
}

// MACD 히스토그램 값 계산
fn calculate_macd_histogram(data: &[solomon::FiveMinData], _stock_code: &str, _cache: &Arc<Mutex<Cache>>) -> Result<f64> {
    if data.len() < 4 {
        return Ok(0.0);
    }
    
    // 가격 데이터 준비 (시가 + 종가들)
    let mut prices = Vec::new();
    if !data.is_empty() {
        prices.push(data[0].open as f64); // 첫 번째 5분봉의 open
    }
    for d in data {
        prices.push(d.close as f64); // 각 5분봉의 close
    }
    
    if prices.len() < 5 {
        return Ok(0.0);
    }
    
    // EMA 3, 6 계산
    let ema3 = calculate_ema(&prices, 3);
    let ema6 = calculate_ema(&prices, 6);
    
    let macd = ema3 - ema6;
    
    // 단순한 시그널 계산 (MACD의 이동평균 대신 0선 기준)
    let signal = 0.0; // 0선을 시그널로 사용
    
    let histogram = macd - signal;
    Ok(histogram)
}

// MACD 히스토그램 증가 여부
fn calculate_macd_histogram_increasing(data: &[solomon::FiveMinData], _stock_code: &str, _cache: &Arc<Mutex<Cache>>) -> Result<i32> {
    if data.len() < 5 {
        return Ok(0);
    }
    
    // 가격 데이터 준비 (시가 + 종가들)
    let mut prices = Vec::new();
    if !data.is_empty() {
        prices.push(data[0].open as f64); // 첫 번째 5분봉의 open
    }
    for d in data {
        prices.push(d.close as f64); // 각 5분봉의 close
    }
    
    if prices.len() < 6 {
        return Ok(0);
    }
    
    // 현재 MACD 히스토그램
    let ema3 = calculate_ema(&prices, 3);
    let ema6 = calculate_ema(&prices, 6);
    let macd = ema3 - ema6;
    let current_histogram = macd; // 0선 기준
    
    // 이전 MACD 히스토그램
    let prev_prices: Vec<f64> = prices[..prices.len()-1].to_vec();
    let prev_ema3 = calculate_ema(&prev_prices, 3);
    let prev_ema6 = calculate_ema(&prev_prices, 6);
    let prev_macd = prev_ema3 - prev_ema6;
    let prev_histogram = prev_macd; // 0선 기준
    
    Ok(if current_histogram > prev_histogram { 1 } else { 0 })
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

// 시가 대비 현재가 수익률
fn calculate_open_to_now_return(data: &[solomon::FiveMinData]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    
    let open_price = data[0].open as f64;
    let current_price = data.last().unwrap().close as f64;
    
    if open_price > 0.0 {
        (current_price - open_price) / open_price
    } else {
        0.0
    }
}

// 최근 3일 고점 대비 현재가 위치 비율
fn calculate_pos_vs_high_3d(today_data: &[solomon::FiveMinData], prev_data: &[solomon::FiveMinData], prev_prev_data: &[solomon::FiveMinData]) -> f64 {
    if today_data.is_empty() {
        return 0.0;
    }
    
    let current_price = today_data.last().unwrap().close as f64;
    let mut high_3d = f64::NEG_INFINITY;
    
    // 3일간의 고점 찾기
    for data in [today_data, prev_data, prev_prev_data] {
        if !data.is_empty() {
            let day_high = data.iter().map(|d| d.high as f64).fold(f64::NEG_INFINITY, f64::max);
            high_3d = high_3d.max(day_high);
        }
    }
    
    if high_3d > 0.0 {
        current_price / high_3d
    } else {
        0.0
    }
}

// 최근 5일 고점 대비 현재가 위치 비율
fn calculate_pos_vs_high_5d(today_data: &[solomon::FiveMinData], prev_data: &[solomon::FiveMinData], prev_prev_data: &[solomon::FiveMinData]) -> f64 {
    // 5일 데이터가 부족하므로 3일 데이터로 근사
    calculate_pos_vs_high_3d(today_data, prev_data, prev_prev_data)
}

// 최근 10일 고점 대비 현재가 위치 비율
fn calculate_pos_vs_high_10d(today_data: &[solomon::FiveMinData], prev_data: &[solomon::FiveMinData], prev_prev_data: &[solomon::FiveMinData]) -> f64 {
    // 10일 데이터가 부족하므로 3일 데이터로 근사
    calculate_pos_vs_high_3d(today_data, prev_data, prev_prev_data)
}

// 연속 양봉 최대 개수
fn calculate_consecutive_bull_count(data: &[solomon::FiveMinData]) -> i32 {
    if data.len() < 2 {
        return 0;
    }
    
    let mut max_consecutive = 0;
    let mut current_consecutive = 0;
    
    for i in 1..data.len() {
        let prev_close = data[i-1].close as f64;
        let curr_close = data[i].close as f64;
        
        if curr_close > prev_close {
            current_consecutive += 1;
            max_consecutive = max_consecutive.max(current_consecutive);
        } else {
            current_consecutive = 0;
        }
    }
    
    max_consecutive
}

// 가장 거래량 많은 5분봉이 양봉인지 여부
fn calculate_is_highest_volume_bull_candle(data: &[solomon::FiveMinData]) -> i32 {
    if data.is_empty() {
        return 0;
    }
    
    let mut max_volume = 0;
    let mut max_volume_index = 0;
    
    for (i, d) in data.iter().enumerate() {
        if d.volume > max_volume {
            max_volume = d.volume;
            max_volume_index = i;
        }
    }
    
    if max_volume_index > 0 {
        let prev_close = data[max_volume_index - 1].close as f64;
        let curr_close = data[max_volume_index].close as f64;
        if curr_close > prev_close { 1 } else { 0 }
    } else {
        0
    }
}

// 9:00~9:30 중 평균 거래량 초과한 5분봉 개수
fn calculate_high_volume_early_count(data: &[solomon::FiveMinData], _stock_code: &str, _cache: &Arc<Mutex<Cache>>) -> Result<i32> {
    if data.len() < 6 {
        return Ok(0);
    }
    
    // 오전 6개 5분봉 (9:00~9:30)
    let morning_data = &data[..6];
    let morning_avg_volume = morning_data.iter().map(|d| d.volume).sum::<i32>() as f64 / morning_data.len() as f64;
    
    let mut high_volume_count = 0;
    for d in morning_data {
        if d.volume as f64 > morning_avg_volume {
            high_volume_count += 1;
        }
    }
    
    Ok(high_volume_count)
}

// 장대양봉 여부 (시가 대비 1% 이상 상승)
fn calculate_is_long_bull_candle(data: &[solomon::FiveMinData]) -> i32 {
    if data.is_empty() {
        return 0;
    }
    
    let open_price = data[0].open as f64;
    let close_price = data.last().unwrap().close as f64;
    
    if open_price > 0.0 {
        let return_rate = (close_price - open_price) / open_price;
        if return_rate >= 0.01 { 1 } else { 0 }
    } else {
        0
    }
}

// 상승 장악형 캔들 패턴
fn calculate_is_bullish_engulfing(data: &[solomon::FiveMinData]) -> i32 {
    if data.len() < 2 {
        return 0;
    }
    
    let prev = &data[data.len() - 2];
    let curr = &data[data.len() - 1];
    
    let prev_open = prev.open as f64;
    let prev_close = prev.close as f64;
    let curr_open = curr.open as f64;
    let curr_close = curr.close as f64;
    
    // 이전 봉이 음봉이고, 현재 봉이 양봉이며, 현재 봉이 이전 봉을 완전히 감싸는 경우
    if prev_close < prev_open && curr_close > curr_open && 
       curr_open < prev_close && curr_close > prev_open {
        1
    } else {
        0
    }
}

// 하락 장악형 캔들 패턴
fn calculate_is_bearish_engulfing(data: &[solomon::FiveMinData]) -> i32 {
    if data.len() < 2 {
        return 0;
    }
    
    let prev = &data[data.len() - 2];
    let curr = &data[data.len() - 1];
    
    let prev_open = prev.open as f64;
    let prev_close = prev.close as f64;
    let curr_open = curr.open as f64;
    let curr_close = curr.close as f64;
    
    // 이전 봉이 양봉이고, 현재 봉이 음봉이며, 현재 봉이 이전 봉을 완전히 감싸는 경우
    if prev_close > prev_open && curr_close < curr_open && 
       curr_open > prev_close && curr_close < prev_open {
        1
    } else {
        0
    }
}

// 샛별형 반등 패턴
fn calculate_is_morning_star(data: &[solomon::FiveMinData]) -> i32 {
    if data.len() < 3 {
        return 0;
    }
    
    let first = &data[data.len() - 3];
    let second = &data[data.len() - 2];
    let third = &data[data.len() - 1];
    
    let first_open = first.open as f64;
    let first_close = first.close as f64;
    let second_open = second.open as f64;
    let second_close = second.close as f64;
    let third_open = third.open as f64;
    let third_close = third.close as f64;
    
    // 첫 번째 봉이 큰 음봉, 두 번째 봉이 작은 봉, 세 번째 봉이 큰 양봉
    if first_close < first_open && 
       (second_close - second_open).abs() < (first_open - first_close) * 0.3 &&
       third_close > third_open {
        1
    } else {
        0
    }
}

// 석별형 반락 패턴
fn calculate_is_evening_star(data: &[solomon::FiveMinData]) -> i32 {
    if data.len() < 3 {
        return 0;
    }
    
    let first = &data[data.len() - 3];
    let second = &data[data.len() - 2];
    let third = &data[data.len() - 1];
    
    let first_open = first.open as f64;
    let first_close = first.close as f64;
    let second_open = second.open as f64;
    let second_close = second.close as f64;
    let third_open = third.open as f64;
    let third_close = third.close as f64;
    
    // 첫 번째 봉이 큰 양봉, 두 번째 봉이 작은 봉, 세 번째 봉이 큰 음봉
    if first_close > first_open && 
       (second_close - second_open).abs() < (first_close - first_open) * 0.3 &&
       third_close < third_open {
        1
    } else {
        0
    }
}

// 망치형 반등 신호 패턴
fn calculate_is_hammer(data: &[solomon::FiveMinData]) -> i32 {
    if data.is_empty() {
        return 0;
    }
    
    let last = data.last().unwrap();
    let open = last.open as f64;
    let high = last.high as f64;
    let low = last.low as f64;
    let close = last.close as f64;
    
    let body_size = (close - open).abs();
    let lower_shadow = if close > open { open - low } else { close - low };
    let upper_shadow = if close > open { high - close } else { high - open };
    
    // 몸통이 작고, 아래 그림자가 길며, 위 그림자가 짧은 경우
    if lower_shadow > body_size * 2.0 && upper_shadow < body_size * 0.5 {
        1
    } else {
        0
    }
}

// D+1 종가가 D+1 VWAP 이상인지 여부
fn calculate_vwap_support_check(prev_data: &[solomon::FiveMinData]) -> i32 {
    if prev_data.is_empty() {
        return 0;
    }
    
    let vwap = calculate_vwap(prev_data);
    let close_price = prev_data.last().unwrap().close as f64;
    
    if close_price >= vwap { 1 } else { 0 }
}

// VWAP 계산
fn calculate_vwap(data: &[solomon::FiveMinData]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    
    let mut total_volume_price = 0.0;
    let mut total_volume = 0.0;
    
    for d in data {
        let typical_price = (d.high as f64 + d.low as f64 + d.close as f64) / 3.0;
        total_volume_price += typical_price * d.volume as f64;
        total_volume += d.volume as f64;
    }
    
    if total_volume > 0.0 {
        total_volume_price / total_volume
    } else {
        0.0
    }
}

// 고점과 VWAP 간 괴리율, 저점과 VWAP 간 괴리율
fn calculate_vwap_vs_extremes(data: &[solomon::FiveMinData], stock_code: &str, cache: &Arc<Mutex<Cache>>) -> Result<(f64, f64)> {
    if data.is_empty() {
        return Ok((0.0, 0.0));
    }
    
    let vwap = {
        let cache_key = format!("{}_vwap", stock_code);
        let cache_guard = cache.lock().unwrap();
        if let Some(&cached_vwap) = cache_guard.vwap_cache.get(&cache_key) {
            cached_vwap
        } else {
            drop(cache_guard);
            let vwap_value = calculate_vwap(data);
            let mut cache_guard = cache.lock().unwrap();
            cache_guard.vwap_cache.insert(cache_key, vwap_value);
            vwap_value
        }
    };
    
    let high = data.iter().map(|d| d.high as f64).fold(f64::NEG_INFINITY, f64::max);
    let low = data.iter().map(|d| d.low as f64).fold(f64::INFINITY, f64::min);
    
    let vwap_vs_high = if vwap > 0.0 { (high - vwap) / vwap } else { 0.0 };
    let vwap_vs_low = if vwap > 0.0 { (vwap - low) / vwap } else { 0.0 };
    
    Ok((vwap_vs_high, vwap_vs_low))
}

// Bollinger Band 관련 특징들
fn calculate_bollinger_features(data: &[solomon::FiveMinData], stock_code: &str, cache: &Arc<Mutex<Cache>>) -> Result<(f64, f64, i32)> {
    if data.len() < 20 {
        return Ok((0.0, 0.0, 0));
    }
    
    let cache_key = format!("{}_bollinger", stock_code);
    
    // 캐시 확인
    {
        let cache_guard = cache.lock().unwrap();
        if let Some(&(upper, middle, lower)) = cache_guard.bollinger_cache.get(&cache_key) {
            let current_price = data.last().unwrap().close as f64;
            let band_width = if middle > 0.0 { (upper - lower) / middle } else { 0.0 };
            let position = if upper > lower { (current_price - lower) / (upper - lower) } else { 0.5 };
            let is_breaking_upper = if current_price > upper { 1 } else { 0 };
            return Ok((band_width, position, is_breaking_upper));
        }
    }
    
    // 20기간 이동평균과 표준편차 계산
    let prices: Vec<f64> = data.iter().map(|d| d.close as f64).collect();
    let sma = prices.iter().sum::<f64>() / prices.len() as f64;
    
    let variance = prices.iter().map(|&p| (p - sma).powi(2)).sum::<f64>() / prices.len() as f64;
    let std_dev = variance.sqrt();
    
    let upper_band = sma + (2.0 * std_dev);
    let lower_band = sma - (2.0 * std_dev);
    
    // 캐시에 저장
    {
        let mut cache_guard = cache.lock().unwrap();
        cache_guard.bollinger_cache.insert(cache_key, (upper_band, sma, lower_band));
    }
    
    let current_price = data.last().unwrap().close as f64;
    let band_width = if sma > 0.0 { (upper_band - lower_band) / sma } else { 0.0 };
    let position = if upper_band > lower_band { (current_price - lower_band) / (upper_band - lower_band) } else { 0.5 };
    let is_breaking_upper = if current_price > upper_band { 1 } else { 0 };
    
    Ok((band_width, position, is_breaking_upper))
}

fn save_day4_features_batch(
    db: &mut Connection,
    batch_features: &[(Day4StockInfo, Day4Features)],
) -> Result<()> {
    let transaction = db.transaction()?;
    
    for (stock_info, features) in batch_features {
        let date_str = stock_info.date.format("%Y-%m-%d").to_string();
        transaction.execute(
            "INSERT INTO day4 (
                stock_code, date, rsi_above_50, short_macd_cross_signal, rsi_value, rsi_overbought, rsi_oversold,
                macd_histogram, macd_histogram_increasing, open_to_now_return, pos_vs_high_3d, pos_vs_high_5d, pos_vs_high_10d,
                consecutive_bull_count, is_highest_volume_bull_candle, high_volume_early_count,
                is_long_bull_candle, is_bullish_engulfing, is_bearish_engulfing,
                is_morning_star, is_evening_star, is_hammer, vwap_support_check,
                vwap_vs_high, vwap_vs_low, bollinger_band_width, bollinger_position, is_breaking_upper_band
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                stock_info.stock_code,
                date_str,
                features.rsi_above_50,
                features.short_macd_cross_signal,
                features.rsi_value,
                features.rsi_overbought,
                features.rsi_oversold,
                features.macd_histogram,
                features.macd_histogram_increasing,
                features.open_to_now_return,
                features.pos_vs_high_3d,
                features.pos_vs_high_5d,
                features.pos_vs_high_10d,
                features.consecutive_bull_count,
                features.is_highest_volume_bull_candle,
                features.high_volume_early_count,
                features.is_long_bull_candle,
                features.is_bullish_engulfing,
                features.is_bearish_engulfing,
                features.is_morning_star,
                features.is_evening_star,
                features.is_hammer,
                features.vwap_support_check,
                features.vwap_vs_high,
                features.vwap_vs_low,
                features.bollinger_band_width,
                features.bollinger_position,
                features.is_breaking_upper_band,
            ],
        )?;
    }
    
    transaction.commit()?;
    Ok(())
}

#[derive(Debug, Clone)]
struct Day4StockInfo {
    stock_code: String,
    date: NaiveDate,
}

#[derive(Debug)]
struct Day4Features {
    // 1. 모멘텀 & 추세
    rsi_above_50: i32,                    // RSI(6)가 50 이상인지 여부
    short_macd_cross_signal: i32,         // 단축 MACD(3,6) 시그널선 상향 돌파 여부
    
    // 1-1. RSI 파생 특징들
    rsi_value: f64,                       // RSI 값 (0-100)
    rsi_overbought: i32,                  // RSI 과매수 구간 (70 이상)
    rsi_oversold: i32,                    // RSI 과매도 구간 (30 이하)
    
    // 1-2. MACD 파생 특징들
    macd_histogram: f64,                  // MACD 히스토그램 값
    macd_histogram_increasing: i32,       // MACD 히스토그램 증가 여부
    
    // 2. 가격 위치 & 추세 유지
    open_to_now_return: f64,              // 시가 대비 현재가 수익률
    pos_vs_high_3d: f64,                  // 최근 3일 고점 대비 현재가 위치 비율
    pos_vs_high_5d: f64,                  // 최근 5일 고점 대비 현재가 위치 비율
    pos_vs_high_10d: f64,                 // 최근 10일 고점 대비 현재가 위치 비율
    
    // 3. 변동성 & 양봉 흐름
    consecutive_bull_count: i32,          // 연속 양봉 최대 개수
    is_highest_volume_bull_candle: i32,   // 가장 거래량 많은 5분봉이 양봉인지 여부
    
    // 4. 거래량 관련
    high_volume_early_count: i32,         // 9:00~9:30 중 평균 거래량 초과한 5분봉 개수
    
    // 5. 캔들 패턴
    is_long_bull_candle: i32,             // 장대양봉 여부 (시가 대비 1% 이상 상승)
    is_bullish_engulfing: i32,            // 상승 장악형 캔들 패턴
    is_bearish_engulfing: i32,            // 하락 장악형 캔들 패턴
    is_morning_star: i32,                 // 샛별형 반등 패턴
    is_evening_star: i32,                 // 석별형 반락 패턴
    is_hammer: i32,                       // 망치형 반등 신호 패턴
    
    // 6. VWAP 기반
    vwap_support_check: i32,              // D+1 종가가 D+1 VWAP 이상인지 여부
    vwap_vs_high: f64,                    // 고점과 VWAP 간 괴리율
    vwap_vs_low: f64,                     // 저점과 VWAP 간 괴리율
    
    // 7. Bollinger Band 기반
    bollinger_band_width: f64,            // 볼린저 밴드 폭 (상단-하단/중심선)
    bollinger_position: f64,              // 현재가의 밴드 내 상대 위치 (0~1)
    is_breaking_upper_band: i32,          // 상단 밴드 돌파 여부
}

fn get_stock_list_from_answer(db: &Connection) -> Result<Vec<Day4StockInfo>> {
    let mut stmt = db.prepare(
        "SELECT stock_code, date FROM answer_v3 WHERE date < 20230601"
    )?;
    let mut stock_list = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let stock_code: String = row.get(0)?;
        let date_int: i64 = row.get(1)?;
        
        // 날짜 형식 처리 (YYYYMMDD)
        let year = (date_int / 10000) as i32;
        let month = ((date_int % 10000) / 100) as u32;
        let day = (date_int % 100) as u32;
        let date = match NaiveDate::from_ymd_opt(year, month, day) {
            Some(date) => date,
            None => {
                warn!("잘못된 날짜 형식: {}", date_int);
                continue;
            }
        };
        
        stock_list.push(Day4StockInfo {
            stock_code,
            date,
        });
    }
    Ok(stock_list)
}

fn create_day4_table(db: &Connection) -> Result<()> {
    db.execute(
        "CREATE TABLE day4 (
            stock_code TEXT,
            date TEXT,
            rsi_above_50 INTEGER,
            short_macd_cross_signal INTEGER,
            rsi_value REAL,
            rsi_overbought INTEGER,
            rsi_oversold INTEGER,
            macd_histogram REAL,
            macd_histogram_increasing INTEGER,
            open_to_now_return REAL,
            pos_vs_high_3d REAL,
            pos_vs_high_5d REAL,
            pos_vs_high_10d REAL,
            consecutive_bull_count INTEGER,
            is_highest_volume_bull_candle INTEGER,
            high_volume_early_count INTEGER,
            is_long_bull_candle INTEGER,
            is_bullish_engulfing INTEGER,
            is_bearish_engulfing INTEGER,
            is_morning_star INTEGER,
            is_evening_star INTEGER,
            is_hammer INTEGER,
            vwap_support_check INTEGER,
            vwap_vs_high REAL,
            vwap_vs_low REAL,
            bollinger_band_width REAL,
            bollinger_position REAL,
            is_breaking_upper_band INTEGER,
            PRIMARY KEY (date, stock_code)
        )",
        [],
    )?;
    
    Ok(())
}