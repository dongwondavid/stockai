use chrono::NaiveDate;
use rusqlite::{Connection, Result};
use log::{info, warn, error, debug};
use indicatif::{ProgressBar, ProgressStyle};
use solomon::{SectorManager, DateSectorCache, FiveMinData};
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
    volume_cache: HashMap<String, f64>,
}

fn main() {
    // 로거 초기화
    env_logger::init();
    
    info!("Day1 특징 계산 프로그램 시작");
    
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
    info!("총 {}개 종목에 대해 Day1 특징을 계산합니다.", stock_list.len());
    
    if stock_list.is_empty() {
        error!("answer 테이블에서 데이터를 가져오지 못했습니다. 테이블이 비어있거나 경로가 잘못되었을 수 있습니다.");
        return;
    }
    
    // 기존 day1 테이블이 있다면 삭제하고 새로 생성
    db_pool.solomon_db.execute("DROP TABLE IF EXISTS day1", []).unwrap();
    create_day1_table(&db_pool.solomon_db).unwrap();
    debug!("Day1 테이블 준비 완료");
    
    // 날짜별로 종목들을 그룹화
    let mut date_groups: HashMap<NaiveDate, Vec<Day1StockInfo>> = HashMap::new();
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
    
    // 날짜별로 처리 (날짜별 섹터 정보 캐시 사용)
    for (date, stocks) in date_groups {
        pb.set_message(format!("날짜 {} 처리 중: {}개 종목", date.format("%Y-%m-%d"), stocks.len()));
        
        // 해당 날짜의 섹터 정보를 한 번만 계산
        let date_cache = match sector_manager.calculate_date_sector_cache(&date, &db_pool.solomon_db) {
            Ok(cache) => cache,
            Err(e) => {
                error!("날짜 {} 섹터 정보 계산 실패: {}", date.format("%Y-%m-%d"), e);
                continue;
            }
        };
        
        for stock_info in stocks {
            pb.set_message(format!("처리 중: {} ({})", stock_info.stock_code, stock_info.date));
            
            match calculate_day1_features_optimized(&stock_info, &sector_manager, &db_pool, &date_cache, &cache) {
                Ok(features) => {
                    batch_features.push((stock_info.clone(), features));
                    
                    // 배치 크기에 도달하면 일괄 저장
                    if batch_features.len() >= BATCH_SIZE {
                        if let Err(e) = save_day1_features_batch(&mut db_pool.solomon_db, &batch_features) {
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
        if let Err(e) = save_day1_features_batch(&mut db_pool.solomon_db, &batch_features) {
            error!("최종 배치 저장 중 오류 발생: {}", e);
        } else {
            processed_count += batch_features.len();
        }
    }
    
    pb.finish_with_message("Day1 특징 계산 완료!");
    
    info!("=== Day1 특징 계산 결과 ===");
    info!("처리된 종목 수: {}", processed_count);
    info!("결과가 solomon.db의 day1 테이블에 저장되었습니다.");
    
    info!("Day1 특징 계산 프로그램 종료");
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

fn calculate_day1_features_optimized(
    stock_info: &Day1StockInfo, 
    sector_manager: &SectorManager,
    db_pool: &DbPool,
    date_cache: &DateSectorCache,
    cache: &Arc<Mutex<Cache>>,
) -> Result<Day1Features> {
    // 오늘 9:30까지의 5분봉 데이터
    let today_data = sector_manager.get_five_min_data(&db_pool.stock_db, &stock_info.stock_code, stock_info.date)?;
    
    // 1. 오늘의 시가 기준
    let (current_price_ratio, high_price_ratio, low_price_ratio) = calculate_price_ratios(&today_data);
    
    // 2. 당일 가격 위치 관련
    let (price_position_ratio, vwap_position_ratio) = calculate_position_ratios_optimized(&today_data, &stock_info.stock_code, cache)?;
    
    // 3. 동시 상승 종목 관련 (15위 내)
    let (same_sector_rising_count_15, has_3_or_more_rising_15) = calculate_sector_rising_features(date_cache, &stock_info.stock_code, 15);
    
    // 4. 동시 상승 종목 관련 (30위 내)
    let (same_sector_rising_count_30, has_3_or_more_rising_30) = calculate_sector_rising_features(date_cache, &stock_info.stock_code, 30);
    
    // 5. 업종 내 상대적 강도
    let (sector_rank_ratio, is_sector_first) = calculate_sector_rank_features(date_cache, &stock_info.stock_code);
    
    // 6. 거래량 관련
    let volume_ratio = calculate_volume_ratio_optimized(&today_data, &stock_info.stock_code, cache)?;
    
    // 7. 캔들 형태 반복 여부
    let (first_derivative, second_derivative, third_derivative, fourth_derivative, fifth_derivative, sixth_derivative) = calculate_derivatives(&today_data);
    
    // 8. 장대양봉
    let (long_candle_ratio, is_long_candle) = calculate_long_candle_features(&today_data);

    Ok(Day1Features {
        current_price_ratio,
        high_price_ratio,
        low_price_ratio,
        price_position_ratio,
        vwap_position_ratio,
        same_sector_rising_count_15,
        has_3_or_more_rising_15,
        same_sector_rising_count_30,
        has_3_or_more_rising_30,
        sector_rank_ratio,
        is_sector_first,
        volume_ratio,
        first_derivative,
        second_derivative,
        third_derivative,
        fourth_derivative,
        fifth_derivative,
        sixth_derivative,
        long_candle_ratio,
        is_long_candle,
    })
}

fn calculate_sector_rising_features(date_cache: &DateSectorCache, stock_code: &str, limit: usize) -> (i32, i32) {
    let current_sector = solomon::SectorManager::new().get_sector(stock_code);
    let mut same_sector_rising_count = 0;
    
    // 상위 limit개 종목 중 같은 섹터의 상승 종목 수 계산
    for (_code, (gain_ratio, sector)) in date_cache.stock_info.iter().take(limit) {
        if *sector == current_sector && *gain_ratio > 0.0 {
            same_sector_rising_count += 1;
        }
    }
    
    let has_3_or_more = if same_sector_rising_count >= 3 { 1 } else { 0 };
    
    (same_sector_rising_count, has_3_or_more)
}

fn calculate_sector_rank_features(date_cache: &DateSectorCache, stock_code: &str) -> (f64, i32) {
    let current_sector = solomon::SectorManager::new().get_sector(stock_code);
    let mut sector_stocks = Vec::new();
    
    // 같은 섹터의 모든 종목 수집
    for (code, (gain_ratio, sector)) in &date_cache.stock_info {
        if *sector == current_sector {
            sector_stocks.push((code.clone(), *gain_ratio));
        }
    }
    
    // 상승률 순으로 정렬
    sector_stocks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    // 현재 종목의 순위 찾기
    let rank = sector_stocks.iter().position(|(code, _)| code == stock_code)
        .map(|pos| pos + 1)
        .unwrap_or(1);
    
    let sector_rank_ratio = 1.0 / rank as f64;
    let is_sector_first = if rank == 1 { 1 } else { 0 };
    
    (sector_rank_ratio, is_sector_first)
}

fn calculate_position_ratios_optimized(data: &[FiveMinData], stock_code: &str, cache: &Arc<Mutex<Cache>>) -> Result<(f64, f64)> {
    if data.is_empty() {
        return Ok((0.0, 0.0));
    }
    
    let open_price = data[0].open as f64;
    let current_price = data.last().unwrap().close as f64;
    let high_price = data.iter().map(|d| d.high as f64).fold(f64::NEG_INFINITY, f64::max);
    
    let price_position_ratio = if high_price > open_price {
        (current_price - open_price) / (high_price - open_price)
    } else {
        0.0
    };
    
    // VWAP 계산 - 캐시 활용
    let vwap = {
        let cache_key = format!("{}", stock_code);
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
    
    let vwap_position_ratio = if vwap > open_price {
        (current_price - open_price) / (vwap - open_price)
    } else {
        0.0
    };
    
    Ok((price_position_ratio, vwap_position_ratio))
}

fn calculate_vwap(data: &[FiveMinData]) -> f64 {
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

fn calculate_volume_ratio_optimized(data: &[FiveMinData], stock_code: &str, cache: &Arc<Mutex<Cache>>) -> Result<f64> {
    if data.is_empty() {
        return Ok(0.0);
    }
    
    let cache_key = format!("{}", stock_code);
    let cache_guard = cache.lock().unwrap();
    if let Some(&avg_volume) = cache_guard.volume_cache.get(&cache_key) {
        return Ok(if avg_volume > 0.0 { data.iter().map(|d| d.volume as f64).sum::<f64>() / avg_volume } else { 0.0 });
    }
    drop(cache_guard);
    
    // 상승 분봉과 하락 분봉의 거래량 평균 계산
    let mut rising_volumes = Vec::new();
    let mut falling_volumes = Vec::new();
    
    for i in 1..data.len() {
        let prev_close = data[i-1].close as f64;
        let curr_close = data[i].close as f64;
        let volume = data[i].volume as f64;
        
        if curr_close > prev_close {
            rising_volumes.push(volume);
        } else if curr_close < prev_close {
            falling_volumes.push(volume);
        }
    }
    
    let rising_avg = if !rising_volumes.is_empty() {
        rising_volumes.iter().sum::<f64>() / rising_volumes.len() as f64
    } else {
        0.0
    };
    
    let falling_avg = if !falling_volumes.is_empty() {
        falling_volumes.iter().sum::<f64>() / falling_volumes.len() as f64
    } else {
        0.0
    };
    
    let ratio = if falling_avg > 0.0 { rising_avg / falling_avg } else { 0.0 };
    
    // 캐시에 저장
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.volume_cache.insert(cache_key, falling_avg);
    
    Ok(ratio)
}

fn save_day1_features_batch(
    db: &mut Connection,
    batch_features: &[(Day1StockInfo, Day1Features)],
) -> Result<()> {
    let transaction = db.transaction()?;
    
    for (stock_info, features) in batch_features {
        let date_str = stock_info.date.format("%Y-%m-%d").to_string();
        transaction.execute(
            "INSERT INTO day1 (
                stock_code, date, current_price_ratio, high_price_ratio, low_price_ratio,
                price_position_ratio, vwap_position_ratio, same_sector_rising_count_15,
                has_3_or_more_rising_15, same_sector_rising_count_30, has_3_or_more_rising_30,
                sector_rank_ratio, is_sector_first, volume_ratio, first_derivative,
                second_derivative, third_derivative, fourth_derivative, fifth_derivative, sixth_derivative,
                long_candle_ratio, is_long_candle
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                stock_info.stock_code,
                date_str,
                features.current_price_ratio,
                features.high_price_ratio,
                features.low_price_ratio,
                features.price_position_ratio,
                features.vwap_position_ratio,
                features.same_sector_rising_count_15,
                features.has_3_or_more_rising_15,
                features.same_sector_rising_count_30,
                features.has_3_or_more_rising_30,
                features.sector_rank_ratio,
                features.is_sector_first,
                features.volume_ratio,
                features.first_derivative,
                features.second_derivative,
                features.third_derivative,
                features.fourth_derivative,
                features.fifth_derivative,
                features.sixth_derivative,
                features.long_candle_ratio,
                features.is_long_candle,
            ],
        )?;
    }
    
    transaction.commit()?;
    Ok(())
}

#[derive(Debug, Clone)]
struct Day1StockInfo {
    stock_code: String,
    date: NaiveDate,
}

#[derive(Debug)]
struct Day1Features {
    // 1. 오늘의 시가 기준
    current_price_ratio: f64,  // 시가 대비 현재가
    high_price_ratio: f64,     // 시가 대비 고가
    low_price_ratio: f64,      // 시가 대비 저가
    
    // 2. 당일 가격 위치 관련
    price_position_ratio: f64, // 당일 고가 대비 현재가 비율
    vwap_position_ratio: f64,  // VWAP 대비 위치 비율
    
    // 3. 동시 상승 종목 관련 (15위 내)
    same_sector_rising_count_15: i32, // 15위 내 같은 업종 내 동시 상승 종목 수
    has_3_or_more_rising_15: i32,     // 15위 내 3개 이상 여부 (1 or 0)
    
    // 4. 동시 상승 종목 관련 (30위 내)
    same_sector_rising_count_30: i32, // 30위 내 같은 업종 내 동시 상승 종목 수
    has_3_or_more_rising_30: i32,     // 30위 내 3개 이상 여부 (1 or 0)
    
    // 5. 업종 내 상대적 강도
    sector_rank_ratio: f64,    // 업종 내 상승률 순위 (1/순위)
    is_sector_first: i32,      // 업종 내 1위 여부 (1 or 0)
    
    // 6. 거래량 관련
    volume_ratio: f64,         // 상승 분봉 거래량 평균 / 하락 분봉 거래량 평균
    
    // 7. 캔들 형태 반복 여부
    first_derivative: f64,     // 1차 기울기
    second_derivative: f64,    // 2차 기울기
    third_derivative: f64,     // 3차 기울기
    fourth_derivative: f64,    // 4차 기울기
    fifth_derivative: f64,     // 5차 기울기
    sixth_derivative: f64,     // 6차 기울기
    
    // 8. 장대양봉
    long_candle_ratio: f64,    // 장대양봉 비율
    is_long_candle: i32,       // 0.7 이상 여부 (1 or 0)
}

fn get_stock_list_from_answer(db: &Connection) -> Result<Vec<Day1StockInfo>> {
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
        
        stock_list.push(Day1StockInfo {
            stock_code,
            date,
        });
    }
    Ok(stock_list)
}

fn create_day1_table(db: &Connection) -> Result<()> {
    db.execute(
        "CREATE TABLE day1 (
            stock_code TEXT,
            date TEXT,
            current_price_ratio REAL,
            high_price_ratio REAL,
            low_price_ratio REAL,
            price_position_ratio REAL,
            vwap_position_ratio REAL,
            same_sector_rising_count_15 INTEGER,
            has_3_or_more_rising_15 INTEGER,
            same_sector_rising_count_30 INTEGER,
            has_3_or_more_rising_30 INTEGER,
            sector_rank_ratio REAL,
            is_sector_first INTEGER,
            volume_ratio REAL,
            first_derivative REAL,
            second_derivative REAL,
            third_derivative REAL,
            fourth_derivative REAL,
            fifth_derivative REAL,
            sixth_derivative REAL,
            long_candle_ratio REAL,
            is_long_candle INTEGER,
            PRIMARY KEY (date, stock_code)
        )",
        [],
    )?;
    
    Ok(())
}

fn calculate_price_ratios(data: &[FiveMinData]) -> (f64, f64, f64) {
    if data.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    
    let first_open = data[0].open as f64;
    let last_close = data.last().unwrap().close as f64;
    
    let high = data.iter().map(|d| d.high as f64).fold(f64::NEG_INFINITY, f64::max);
    let low = data.iter().map(|d| d.low as f64).fold(f64::INFINITY, f64::min);
    
    let current_price_ratio = last_close / first_open;
    let high_price_ratio = high / first_open;
    let low_price_ratio = low / first_open;
    
    (current_price_ratio, high_price_ratio, low_price_ratio)
}

#[allow(dead_code)]
fn calculate_position_ratios(data: &[FiveMinData]) -> (f64, f64) {
    if data.is_empty() {
        return (0.0, 0.0);
    }
    
    let current_price = data.last().unwrap().close as f64;
    let high = data.iter().map(|d| d.high as f64).fold(f64::NEG_INFINITY, f64::max);
    let low = data.iter().map(|d| d.low as f64).fold(f64::INFINITY, f64::min);
    
    // 당일 고가 대비 현재가 비율
    let price_position_ratio = if high != low {
        (current_price - low) / (high - low)
    } else {
        0.0
    };
    
    // VWAP 계산
    let total_volume_price: f64 = data.iter().map(|d| (d.close as f64) * (d.volume as f64)).sum();
    let total_volume: f64 = data.iter().map(|d| d.volume as f64).sum();
    let vwap = if total_volume > 0.0 { total_volume_price / total_volume } else { 0.0 };
    
    // VWAP 대비 위치 비율
    let vwap_position_ratio = if vwap > 0.0 { (current_price - vwap) / vwap } else { 0.0 };
    
    (price_position_ratio, vwap_position_ratio)
}

#[allow(dead_code)]
fn calculate_volume_ratio(data: &[FiveMinData]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    
    let mut rising_volumes = Vec::new();
    let mut falling_volumes = Vec::new();
    
    for i in 1..data.len() {
        let prev_close = data[i-1].close as f64;
        let curr_close = data[i].close as f64;
        let volume = data[i].volume as f64;
        
        if curr_close > prev_close {
            rising_volumes.push(volume);
        } else if curr_close < prev_close {
            falling_volumes.push(volume);
        }
    }
    
    let rising_avg = if !rising_volumes.is_empty() {
        rising_volumes.iter().sum::<f64>() / rising_volumes.len() as f64
    } else { 0.0 };
    
    let falling_avg = if !falling_volumes.is_empty() {
        falling_volumes.iter().sum::<f64>() / falling_volumes.len() as f64
    } else { 0.0 };
    
    if falling_avg > 0.0 { rising_avg / falling_avg } else { 0.0 }
}

fn calculate_derivatives(data: &[FiveMinData]) -> (f64, f64, f64, f64, f64, f64) {
    if data.len() < 6 {
        return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
    
    let prices: Vec<f64> = data.iter().map(|d| d.close as f64).collect();
    
    // 1차 기울기 (선형 회귀)
    let n = prices.len() as f64;
    let x_sum: f64 = (0..prices.len()).map(|i| i as f64).sum();
    let y_sum: f64 = prices.iter().sum();
    let xy_sum: f64 = prices.iter().enumerate().map(|(i, &y)| i as f64 * y).sum();
    let x2_sum: f64 = (0..prices.len()).map(|i| (i as f64).powi(2)).sum();
    
    let first_derivative_raw = if n * x2_sum - x_sum * x_sum != 0.0 {
        (n * xy_sum - x_sum * y_sum) / (n * x2_sum - x_sum * x_sum)
    } else { 0.0 };
    
    // 2차 기울기 (2차 미분 근사)
    let second_derivative_raw = if prices.len() >= 3 {
        let mid = prices.len() / 2;
        let first_half_avg = prices[..mid].iter().sum::<f64>() / mid as f64;
        let second_half_avg = prices[mid..].iter().sum::<f64>() / (prices.len() - mid) as f64;
        second_half_avg - first_half_avg
    } else { 0.0 };
    
    // 3차 기울기 (3차 미분 근사)
    let third_derivative_raw = if prices.len() >= 4 {
        let quarter = prices.len() / 4;
        let q1_avg = prices[..quarter].iter().sum::<f64>() / quarter as f64;
        let q3_avg = prices[quarter*3..].iter().sum::<f64>() / quarter as f64;
        q3_avg - q1_avg
    } else { 0.0 };
    
    // 4차 기울기 (4차 미분 근사)
    let fourth_derivative_raw = if prices.len() >= 5 {
        let fifth = prices.len() / 5;
        let first_fifth_avg = prices[..fifth].iter().sum::<f64>() / fifth as f64;
        let last_fifth_avg = prices[prices.len()-fifth..].iter().sum::<f64>() / fifth as f64;
        last_fifth_avg - first_fifth_avg
    } else { 0.0 };
    
    // 5차 기울기 (5차 미분 근사)
    let fifth_derivative_raw = if prices.len() >= 6 {
        let sixth = prices.len() / 6;
        let first_sixth_avg = prices[..sixth].iter().sum::<f64>() / sixth as f64;
        let last_sixth_avg = prices[prices.len()-sixth..].iter().sum::<f64>() / sixth as f64;
        last_sixth_avg - first_sixth_avg
    } else { 0.0 };
    
    // 6차 기울기 (연속된 6개 점의 가격 변화 패턴)
    let sixth_derivative_raw = if prices.len() >= 6 {
        // 6개 점에서의 가격 변화 방향 패턴을 분석
        let mut direction_changes = 0;
        for i in 1..prices.len() {
            if i > 1 {
                let prev_change = prices[i-1] - prices[i-2];
                let curr_change = prices[i] - prices[i-1];
                if (prev_change > 0.0 && curr_change < 0.0) || (prev_change < 0.0 && curr_change > 0.0) {
                    direction_changes += 1;
                }
            }
        }
        direction_changes as f64 / (prices.len() - 2) as f64
    } else { 0.0 };
    
    // 로그 정규화 적용: log(1 + |x|) * sign(x)
    let log_normalize = |x: f64| -> f64 {
        if x == 0.0 {
            0.0
        } else {
            let sign = if x > 0.0 { 1.0 } else { -1.0 };
            sign * (1.0 + x.abs()).ln()
        }
    };
    
    let first_derivative = log_normalize(first_derivative_raw);
    let second_derivative = log_normalize(second_derivative_raw);
    let third_derivative = log_normalize(third_derivative_raw);
    let fourth_derivative = log_normalize(fourth_derivative_raw);
    let fifth_derivative = log_normalize(fifth_derivative_raw);
    let sixth_derivative = log_normalize(sixth_derivative_raw);
    
    // debug!("기울기 계산 결과: 1차={:.6}->{:.6}, 2차={:.6}->{:.6}, 3차={:.6}->{:.6}, 4차={:.6}->{:.6}, 5차={:.6}->{:.6}, 6차={:.6}->{:.6}", 
    //        first_derivative_raw, first_derivative,
    //        second_derivative_raw, second_derivative,
    //        third_derivative_raw, third_derivative,
    //        fourth_derivative_raw, fourth_derivative,
    //        fifth_derivative_raw, fifth_derivative,
    //        sixth_derivative_raw, sixth_derivative);
    
    (first_derivative, second_derivative, third_derivative, fourth_derivative, fifth_derivative, sixth_derivative)
}

fn calculate_long_candle_features(data: &[FiveMinData]) -> (f64, i32) {
    if data.is_empty() {
        return (0.0, 0);
    }
    
    let first_open = data[0].open as f64;
    let last_close = data.last().unwrap().close as f64;
    let high = data.iter().map(|d| d.high as f64).fold(f64::NEG_INFINITY, f64::max);
    let low = data.iter().map(|d| d.low as f64).fold(f64::INFINITY, f64::min);
    
    let long_candle_ratio = if high != low {
        (last_close - first_open) / (high - low)
    } else { 0.0 };
    
    let is_long_candle = if long_candle_ratio >= 0.7 { 1 } else { 0 };
    
    (long_candle_ratio, is_long_candle)
}