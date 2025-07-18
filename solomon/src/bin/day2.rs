use chrono::NaiveDate;
use rusqlite::{Connection, Result};
use log::{info, warn, error, debug};
use indicatif::{ProgressBar, ProgressStyle};
use solomon::{SectorManager, DateSectorCache};
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
    prev_volume_cache: HashMap<String, f64>,
}

fn main() {
    // 로거 초기화
    env_logger::init();
    info!("Day2 특징 계산 프로그램 시작");

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
    info!("총 {}개 종목에 대해 Day2 특징을 계산합니다.", stock_list.len());

    if stock_list.is_empty() {
        error!("answer 테이블에서 데이터를 가져오지 못했습니다. 테이블이 비어있거나 경로가 잘못되었을 수 있습니다.");
        return;
    }

    // 기존 day2 테이블이 있다면 삭제하고 새로 생성
    db_pool.solomon_db.execute("DROP TABLE IF EXISTS day2", []).unwrap();
    create_day2_table(&db_pool.solomon_db).unwrap();
    debug!("Day2 테이블 준비 완료");

    // 날짜별로 종목들을 그룹화
    let mut date_groups: HashMap<NaiveDate, Vec<Day2StockInfo>> = HashMap::new();
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
            
            match calculate_day2_features_optimized(&stock_info, &sector_manager, &db_pool, &date_cache, &cache) {
                Ok(features) => {
                    batch_features.push((stock_info.clone(), features));
                    
                    // 배치 크기에 도달하면 일괄 저장
                    if batch_features.len() >= BATCH_SIZE {
                        if let Err(e) = save_day2_features_batch(&mut db_pool.solomon_db, &batch_features) {
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
        if let Err(e) = save_day2_features_batch(&mut db_pool.solomon_db, &batch_features) {
            error!("최종 배치 저장 중 오류 발생: {}", e);
        } else {
            processed_count += batch_features.len();
        }
    }

    pb.finish_with_message("Day2 특징 계산 완료!");
    info!("=== Day2 특징 계산 결과 ===");
    info!("처리된 종목 수: {}", processed_count);
    info!("결과가 solomon.db의 day2 테이블에 저장되었습니다.");
    info!("Day2 특징 계산 프로그램 종료");
}

fn create_db_pool() -> Result<DbPool> {
    let solomon_db_path = std::env::var("SOLOMON_DB_PATH").unwrap_or_else(|_| "D:\\db\\solomon.db".to_string());
    let stock_db_path = std::env::var("STOCK_DB_PATH").unwrap_or_else(|_| "D:\\db\\stock_price(5min).db".to_string());

    let solomon_db = Connection::open(&solomon_db_path)?;
    let stock_db = Connection::open(&stock_db_path)?;

    // 성능 최적화 설정 - PRAGMA는 query로 실행
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

fn calculate_day2_features_optimized(
    stock_info: &Day2StockInfo,
    sector_manager: &SectorManager,
    db_pool: &DbPool,
    date_cache: &DateSectorCache,
    cache: &Arc<Mutex<Cache>>,
) -> Result<Day2Features> {
    use chrono::Duration;
    
    // 오늘 9:30까지의 5분봉 데이터
    let today_data = sector_manager.get_five_min_data(&db_pool.stock_db, &stock_info.stock_code, stock_info.date)?;
    // 전일 날짜
    let prev_date = stock_info.date - Duration::days(1);
    // 전일 5분봉 전체 데이터
    let prev_data = sector_manager.get_five_min_data(&db_pool.stock_db, &stock_info.stock_code, prev_date)?;

    // 오늘 9:30 기준 현재가, 누적 거래량
    let now_price = today_data.last().map(|d| d.close as f64).unwrap_or(0.0);
    let _now_volume: i32 = today_data.iter().map(|d| d.volume).sum();
    // 오늘 시가
    let today_open = today_data.first().map(|d| d.open as f64).unwrap_or(0.0);
    // 전일 종가
    let prev_close = prev_data.last().map(|d| d.close as f64).unwrap_or(0.0);

    // 1. 전일 종가 대비 현재가 비율
    let prev_close_to_now_ratio = if prev_close > 0.0 {
        (now_price - prev_close) / prev_close
    } else {
        0.0
    };

    // 2. 전일 고가 대비 위치
    let prev_high = prev_data.iter().map(|d| d.high as f64).fold(f64::NEG_INFINITY, f64::max);
    let position_vs_prev_high = if prev_high > 0.0 {
        now_price / prev_high
    } else {
        0.0
    };

    // 3. 전일 범위 내 위치
    let prev_low = prev_data.iter().map(|d| d.low as f64).fold(f64::INFINITY, f64::min);
    let position_within_prev_range = if prev_high > prev_low {
        (now_price - prev_low) / (prev_high - prev_low)
    } else {
        0.0
    };

    // 4. 섹터 순위 관련 (캐시된 섹터 정보 활용)
    let (sector_rank_ratio_day2, is_sector_first_day2) = {
        let current_sector = sector_manager.get_sector(&stock_info.stock_code);
        let mut sector_stocks = Vec::new();
        
        for (code, (gain_ratio, sector)) in &date_cache.stock_info {
            if *sector == current_sector {
                sector_stocks.push((code.clone(), *gain_ratio));
            }
        }
        
        sector_stocks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        let rank = sector_stocks.iter().position(|(code, _)| code == &stock_info.stock_code)
            .map(|pos| pos + 1)
            .unwrap_or(1);
        
        let sector_rank_ratio = 1.0 / rank as f64;
        let is_sector_first = if rank == 1 { 1 } else { 0 };
        
        (sector_rank_ratio, is_sector_first)
    };

    // 5. 동시 상승 종목 관련 (15위 내)
    let (same_sector_rising_count_15_day2, same_sector_rising_15_ge3_day2) = {
        let current_sector = sector_manager.get_sector(&stock_info.stock_code);
        let mut same_sector_rising_count = 0;
        
        for (_code, (gain_ratio, sector)) in date_cache.stock_info.iter().take(15) {
            if *sector == current_sector && *gain_ratio > 0.0 {
                same_sector_rising_count += 1;
            }
        }
        
        let has_3_or_more = if same_sector_rising_count >= 3 { 1 } else { 0 };
        (same_sector_rising_count, has_3_or_more)
    };

    // 6. 동시 상승 종목 관련 (30위 내)
    let (same_sector_rising_count_30_day2, same_sector_rising_30_ge3_day2) = {
        let current_sector = sector_manager.get_sector(&stock_info.stock_code);
        let mut same_sector_rising_count = 0;
        
        for (_code, (gain_ratio, sector)) in date_cache.stock_info.iter().take(30) {
            if *sector == current_sector && *gain_ratio > 0.0 {
                same_sector_rising_count += 1;
            }
        }
        
        let has_3_or_more = if same_sector_rising_count >= 3 { 1 } else { 0 };
        (same_sector_rising_count, has_3_or_more)
    };

    // 7. 거래량 관련 (캐시 활용)
    let volume_ratio_vs_prevday = calculate_volume_ratio_optimized(&today_data, &stock_info.stock_code, &prev_date, &db_pool.stock_db, sector_manager, cache)?;

    // 8. 전일 장대양봉 여부
    let was_prevday_long_candle = calculate_prevday_long_candle(&prev_data);

    // 9. 갭 오픈 비율
    let gap_open_ratio = if prev_close > 0.0 {
        (today_open - prev_close) / prev_close
    } else {
        0.0
    };

    // 10. 전일 범위 비율
    let prev_day_range_ratio = if prev_close > 0.0 {
        (prev_high - prev_low) / prev_close
    } else {
        0.0
    };

    // 11. 전일 3% 이상 상승 여부
    let prev_gain_over_3 = if prev_close > 0.0 {
        let prev_gain = (prev_close - prev_data.first().map(|d| d.open as f64).unwrap_or(0.0)) / prev_data.first().map(|d| d.open as f64).unwrap_or(1.0);
        if prev_gain >= 0.03 { 1 } else { 0 }
    } else {
        0
    };

    // 12. 섹터 상위 3개 여부
    let is_sector_top3_day2 = {
        let current_sector = sector_manager.get_sector(&stock_info.stock_code);
        let mut sector_stocks = Vec::new();
        
        for (code, (gain_ratio, sector)) in &date_cache.stock_info {
            if *sector == current_sector {
                sector_stocks.push((code.clone(), *gain_ratio));
            }
        }
        
        sector_stocks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        let rank = sector_stocks.iter().position(|(code, _)| code == &stock_info.stock_code)
            .map(|pos| pos + 1)
            .unwrap_or(1);
        
        if rank <= 3 { 1 } else { 0 }
    };

    Ok(Day2Features {
        prev_close_to_now_ratio,
        position_vs_prev_high,
        position_within_prev_range,
        sector_rank_ratio_day2,
        is_sector_first_day2,
        same_sector_rising_count_15_day2,
        same_sector_rising_15_ge3_day2,
        same_sector_rising_count_30_day2,
        same_sector_rising_30_ge3_day2,
        volume_ratio_vs_prevday,
        was_prevday_long_candle,
        gap_open_ratio,
        prev_day_range_ratio,
        prev_gain_over_3,
        is_sector_top3_day2,
    })
}

fn calculate_volume_ratio_optimized(
    today_data: &[solomon::FiveMinData],
    stock_code: &str,
    prev_date: &NaiveDate,
    stock_db: &Connection,
    sector_manager: &SectorManager,
    cache: &Arc<Mutex<Cache>>,
) -> Result<f64> {
    let cache_key = format!("{}", stock_code);
    
    // 캐시 확인
    {
        let cache_guard = cache.lock().unwrap();
        if let Some(&prev_volume) = cache_guard.prev_volume_cache.get(&cache_key) {
            let today_volume = today_data.iter().map(|d| d.volume).sum::<i32>() as f64;
            return Ok(if prev_volume > 0.0 { today_volume / prev_volume } else { 0.0 });
        }
    }
    
    // 전일 거래량 계산
    let prev_data = sector_manager.get_five_min_data(stock_db, stock_code, *prev_date)?;
    let prev_volume = prev_data.iter().map(|d| d.volume).sum::<i32>() as f64;
    
    // 캐시에 저장
    {
        let mut cache_guard = cache.lock().unwrap();
        cache_guard.prev_volume_cache.insert(cache_key, prev_volume);
    }
    
    let today_volume = today_data.iter().map(|d| d.volume).sum::<i32>() as f64;
    Ok(if prev_volume > 0.0 { today_volume / prev_volume } else { 0.0 })
}

fn calculate_prevday_long_candle(prev_data: &[solomon::FiveMinData]) -> i32 {
    if prev_data.is_empty() {
        return 0;
    }
    
    let open = prev_data[0].open as f64;
    let close = prev_data.last().unwrap().close as f64;
    let high = prev_data.iter().map(|d| d.high as f64).fold(f64::NEG_INFINITY, f64::max);
    let low = prev_data.iter().map(|d| d.low as f64).fold(f64::INFINITY, f64::min);
    
    if (high - low) > 0.0 && (close - open) / (high - low) > 0.7 {
        1
    } else {
        0
    }
}

fn save_day2_features_batch(
    db: &mut Connection,
    batch_features: &[(Day2StockInfo, Day2Features)],
) -> Result<()> {
    let transaction = db.transaction()?;
    
    for (stock_info, features) in batch_features {
        let date_str = stock_info.date.format("%Y-%m-%d").to_string();
        transaction.execute(
            "INSERT INTO day2 (
                stock_code, date, prev_close_to_now_ratio, position_vs_prev_high,
                position_within_prev_range, sector_rank_ratio_day2, is_sector_first_day2,
                same_sector_rising_count_15_day2, same_sector_rising_15_ge3_day2,
                same_sector_rising_count_30_day2, same_sector_rising_30_ge3_day2,
                volume_ratio_vs_prevday, was_prevday_long_candle, gap_open_ratio,
                prev_day_range_ratio, prev_gain_over_3, is_sector_top3_day2
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                stock_info.stock_code,
                date_str,
                features.prev_close_to_now_ratio,
                features.position_vs_prev_high,
                features.position_within_prev_range,
                features.sector_rank_ratio_day2,
                features.is_sector_first_day2,
                features.same_sector_rising_count_15_day2,
                features.same_sector_rising_15_ge3_day2,
                features.same_sector_rising_count_30_day2,
                features.same_sector_rising_30_ge3_day2,
                features.volume_ratio_vs_prevday,
                features.was_prevday_long_candle,
                features.gap_open_ratio,
                features.prev_day_range_ratio,
                features.prev_gain_over_3,
                features.is_sector_top3_day2,
            ],
        )?;
    }
    
    transaction.commit()?;
    Ok(())
}

#[derive(Debug, Clone)]
struct Day2StockInfo {
    stock_code: String,
    date: NaiveDate,
}

#[derive(Debug)]
struct Day2Features {
    prev_close_to_now_ratio: f64,
    position_vs_prev_high: f64,
    position_within_prev_range: f64,
    sector_rank_ratio_day2: f64,
    is_sector_first_day2: i32,
    same_sector_rising_count_15_day2: i32,
    same_sector_rising_15_ge3_day2: i32,
    same_sector_rising_count_30_day2: i32,
    same_sector_rising_30_ge3_day2: i32,
    volume_ratio_vs_prevday: f64,
    was_prevday_long_candle: i32,
    gap_open_ratio: f64,
    prev_day_range_ratio: f64,
    prev_gain_over_3: i32,
    is_sector_top3_day2: i32,
}

fn get_stock_list_from_answer(db: &Connection) -> Result<Vec<Day2StockInfo>> {
    let mut stmt = db.prepare(
        "SELECT stock_code, date FROM answer_v3 WHERE date < 20230601"
    )?;
    let mut stock_list = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let stock_code: String = row.get(0)?;
        let date_int: i64 = row.get(1)?;
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
        stock_list.push(Day2StockInfo {
            stock_code,
            date,
        });
    }
    Ok(stock_list)
}

fn create_day2_table(db: &Connection) -> Result<()> {
    db.execute(
        "CREATE TABLE day2 (
            stock_code TEXT,
            date TEXT,
            prev_close_to_now_ratio REAL,
            position_vs_prev_high REAL,
            position_within_prev_range REAL,
            sector_rank_ratio_day2 REAL,
            is_sector_first_day2 INTEGER,
            same_sector_rising_count_15_day2 INTEGER,
            same_sector_rising_15_ge3_day2 INTEGER,
            same_sector_rising_count_30_day2 INTEGER,
            same_sector_rising_30_ge3_day2 INTEGER,
            volume_ratio_vs_prevday REAL,
            was_prevday_long_candle INTEGER,
            gap_open_ratio REAL,
            prev_day_range_ratio REAL,
            prev_gain_over_3 INTEGER,
            is_sector_top3_day2 INTEGER,
            PRIMARY KEY (date, stock_code)
        )",
        [],
    )?;
    Ok(())
} 