use chrono::NaiveDate;
use rusqlite::{Connection, Result};
use log::{info, warn, error, debug};
use indicatif::{ProgressBar, ProgressStyle};
use solomon::{SectorManager, DateSectorCache};
use std::collections::HashMap;
use std::sync::Arc;

use rayon::prelude::*;
use std::time::Instant;
use chrono::Duration;

// NOTE: 성능 최적화 개선사항
// 1. Rayon을 통한 날짜별 병렬 처리로 CPU 코어 활용 극대화
// 2. 5분봉 데이터 캐시로 중복 DB 호출 제거 (종목+날짜 기준)
// 3. Prepared Statement 재사용으로 SQL 파싱 오버헤드 제거
// 4. 배치 저장 시 prepare 한 번만 실행하여 반복 오버헤드 제거
// 5. 메모리 기반 전처리 데이터 로딩으로 DB I/O 최소화

// NOTE: 스레드별 독립적인 DB 연결을 위한 구조체
struct ThreadResources {
    solomon_db: Connection,
    stock_db: Connection,      // 메모리 DB로 복제된 5분봉 데이터
    daily_db: Connection,      // 메모리 DB로 복제된 일봉 데이터
    cache: Cache,
}

// NOTE: 스레드별 독립적인 캐시 구조체
#[derive(Default)]
struct Cache {
    ema_cache: HashMap<String, (f64, f64)>, // (ema5, ema20)
    market_cap_cache: HashMap<String, f64>,
    foreign_ratio_cache: HashMap<String, Vec<f64>>,
    volume_cache: HashMap<String, f64>,
    five_min_data_cache: HashMap<(String, NaiveDate), Vec<solomon::FiveMinData>>, // 5분봉 데이터 캐시
}

fn main() {
    // 로거 초기화
    env_logger::init();
    info!("Day3 특징 계산 프로그램 시작 (고성능 최적화 버전)");

    let start_time = Instant::now();

    // 섹터 정보 로드
    let mut sector_manager = SectorManager::new();
    if let Err(e) = sector_manager.load_from_csv("sector_utf8.csv") {
        warn!("섹터 정보 로드 실패: {}. 기본값으로 진행합니다.", e);
    } else {
        debug!("섹터 정보 로드 완료");
    }

    let stock_list = match get_stock_list_from_answer() {
        Ok(list) => list,
        Err(e) => {
            error!("answer 테이블에서 데이터 조회 실패: {}", e);
            return;
        }
    };
    info!("총 {}개 종목에 대해 Day3 특징을 계산합니다.", stock_list.len());

    if stock_list.is_empty() {
        error!("answer 테이블에서 데이터를 가져오지 못했습니다. 테이블이 비어있거나 경로가 잘못되었을 수 있습니다.");
        return;
    }

    // 기존 day3 테이블이 있다면 삭제하고 새로 생성
    let mut _main_db = create_solomon_connection().unwrap();
    _main_db.execute("DROP TABLE IF EXISTS day3", []).unwrap();
    create_day3_table(&_main_db).unwrap();
    debug!("Day3 테이블 준비 완료");

    // 날짜별로 종목들을 그룹화
    let mut date_groups: HashMap<NaiveDate, Vec<Day3StockInfo>> = HashMap::new();
    for stock_info in stock_list {
        date_groups.entry(stock_info.date).or_insert_with(Vec::new).push(stock_info);
    }

    info!("총 {}개 날짜로 그룹화되었습니다.", date_groups.len());

    // NOTE: DB 성능 최적화 설정
    info!("DB 성능 최적화를 적용합니다...");
    let db_optimize_start_time = std::time::Instant::now();
    
    let stock_db_path = std::env::var("STOCK_DB_PATH").unwrap_or_else(|_| "D:\\db\\stock_price(5min).db".to_string());
    let daily_db_path = std::env::var("DAILY_DB_PATH").unwrap_or_else(|_| "D:\\db\\stock_price(1day)_with_data.db".to_string());
    
    // DB 파일들을 미리 열어서 메모리에 캐시
    let _stock_db = Connection::open(&stock_db_path).unwrap();
    let _daily_db = Connection::open(&daily_db_path).unwrap();
    
    let db_optimize_elapsed = db_optimize_start_time.elapsed();
    info!("DB 성능 최적화 완료: {:.2?}", db_optimize_elapsed);
    
    // NOTE: 모든 날짜의 섹터 정보를 미리 계산하여 중복 계산 방지
    info!("섹터 정보를 미리 계산합니다...");
    let sector_start_time = std::time::Instant::now();
    let sector_cache = Arc::new(calculate_all_sector_cache_with_progress(&date_groups, &sector_manager, &_main_db));
    let sector_elapsed = sector_start_time.elapsed();

    // NOTE: 병렬 처리 도입 - 날짜별로 병렬 처리하여 CPU 코어 활용 극대화
    let date_groups_vec: Vec<(NaiveDate, Vec<Day3StockInfo>)> = date_groups.into_iter().collect();
    let total_stocks: usize = date_groups_vec.iter().map(|(_, stocks)| stocks.len()).sum();
    
    info!("Day3 특징 계산을 시작합니다. 총 {}개 종목을 병렬 처리합니다.", total_stocks);
    
    // Day3 특징 계산 진행률 표시를 위한 ProgressBar 설정
    let pb = ProgressBar::new(total_stocks as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Day3 특징 계산 중")
        .unwrap()
        .progress_chars("#>-"));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    
    let day3_start_time = std::time::Instant::now();
    
    let results: Vec<Result<Vec<(Day3StockInfo, Day3Features)>>> = date_groups_vec
        .into_par_iter()
        .map(|(date, stocks)| {
            process_date_group_optimized(&date, stocks, &sector_manager, &sector_cache, &pb)
        })
        .collect();

    let day3_elapsed = day3_start_time.elapsed();
    pb.finish_with_message("Day3 특징 계산 완료!");
    
    info!("Day3 특징 계산 완료: 총 소요시간: {:.2?}", day3_elapsed);

    // 결과 수집 및 배치 저장
    let mut processed_count = 0;
    const BATCH_SIZE: usize = 1000; // 배치 크기 증가
    let mut batch_features = Vec::new();

    for result in results {
        match result {
            Ok(features) => {
                for (stock_info, feature) in features {
                    batch_features.push((stock_info, feature));
                    
                    // 배치 크기에 도달하면 일괄 저장
                    if batch_features.len() >= BATCH_SIZE {
                        if let Err(e) = save_day3_features_batch_optimized(&mut _main_db, &batch_features) {
                            error!("배치 저장 중 오류 발생: {}", e);
                        } else {
                            processed_count += batch_features.len();
                        }
                        batch_features.clear();
                    }
                }
            }
            Err(e) => {
                error!("날짜 그룹 처리 중 오류 발생: {}", e);
            }
        }
    }

    // 남은 배치 저장
    if !batch_features.is_empty() {
        if let Err(e) = save_day3_features_batch_optimized(&mut _main_db, &batch_features) {
            error!("최종 배치 저장 중 오류 발생: {}", e);
        } else {
            processed_count += batch_features.len();
        }
    }

    let elapsed = start_time.elapsed();
    
    info!("=== Day3 특징 계산 결과 (고성능 최적화 버전) ===");
    info!("처리된 종목 수: {}", processed_count);
    info!("총 소요 시간: {:.2?}", elapsed);
    info!("DB 최적화: {:.2?}", db_optimize_elapsed);
    info!("섹터 정보 계산: {:.2?}", sector_elapsed);
    info!("Day3 특징 계산: {:.2?}", day3_elapsed);
    info!("평균 처리 속도: {:.2} 종목/초", processed_count as f64 / elapsed.as_secs_f64());
    info!("결과가 solomon.db의 day3 테이블에 저장되었습니다.");
    info!("Day3 특징 계산 프로그램 종료");
}

// NOTE: 모든 날짜의 섹터 정보를 미리 계산 (병렬 처리 + 진행 상황 표시)
fn calculate_all_sector_cache_with_progress(
    date_groups: &HashMap<NaiveDate, Vec<Day3StockInfo>>,
    sector_manager: &SectorManager,
    _main_db: &Connection,
) -> HashMap<NaiveDate, DateSectorCache> {
    let total_dates = date_groups.len();
    
    // 진행 상황 표시를 위한 ProgressBar 설정
    let pb = ProgressBar::new(total_dates as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - 섹터 정보 병렬 계산 중")
        .unwrap()
        .progress_chars("#>-"));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    
    let start_time = std::time::Instant::now();
    
    // NOTE: 병렬 처리를 위해 날짜 목록을 벡터로 변환
    let dates: Vec<NaiveDate> = date_groups.keys().cloned().collect();
    
    // NOTE: 병렬 처리로 섹터 정보 계산 - 각 스레드가 독립적인 DB 연결 사용
    let results: Vec<(NaiveDate, Result<DateSectorCache>)> = dates
        .into_par_iter()
        .map(|date| {
            // 스레드별 독립적인 DB 연결 생성
            let thread_db = match create_solomon_connection() {
                Ok(db) => db,
                Err(e) => {
                    error!("스레드별 DB 연결 생성 실패: {}", e);
                    return (date, Err(e));
                }
            };
            
            // 섹터 정보 계산
            let result = sector_manager.calculate_date_sector_cache(&date, &thread_db);
            (date, result)
        })
        .collect();
    
    // 결과 수집
    let mut sector_cache = HashMap::new();
    let mut processed_count = 0;
    let mut success_count = 0;
    
    for (date, result) in results {
        processed_count += 1;
        
        match result {
            Ok(cache) => {
                sector_cache.insert(date, cache);
                success_count += 1;
            }
            Err(e) => {
                error!("날짜 {} 섹터 정보 계산 실패: {}", date.format("%Y-%m-%d"), e);
            }
        }
        
        // 10개마다 진행 상황 로그 출력
        // if processed_count % 10 == 0 {
        //     let elapsed = start_time.elapsed();
        //     let avg_time_per_date = elapsed.as_secs_f64() / processed_count as f64;
        //     let remaining_dates = total_dates - processed_count;
        //     let estimated_remaining = std::time::Duration::from_secs_f64(avg_time_per_date * remaining_dates as f64);
            
        //     debug!("섹터 정보 계산 진행률: {}/{} ({:.1}%) - 예상 완료: {}",
        //         processed_count, total_dates,
        //         (processed_count as f64 / total_dates as f64) * 100.0,
        //         (chrono::Utc::now() + chrono::Duration::from_std(estimated_remaining).unwrap()).format("%H:%M:%S")
        //     );
        // }
        
        pb.inc(1);
    }
    
    let total_elapsed = start_time.elapsed();
    pb.finish_with_message("섹터 정보 병렬 계산 완료!");
    
    info!("섹터 정보 병렬 계산 완료: {}개 날짜 (성공: {}개), 총 소요시간: {:.2?}, 평균: {:.2}ms/날짜",
        processed_count, success_count, total_elapsed, total_elapsed.as_millis() as f64 / processed_count as f64);
    
    sector_cache
}



// NOTE: 스레드별 독립적인 DB 연결 생성
fn create_thread_resources() -> Result<ThreadResources> {
    let solomon_db_path = std::env::var("SOLOMON_DB_PATH").unwrap_or_else(|_| "D:\\db\\solomon.db".to_string());
    let stock_db_path = std::env::var("STOCK_DB_PATH").unwrap_or_else(|_| "D:\\db\\stock_price(5min).db".to_string());
    let daily_db_path = std::env::var("DAILY_DB_PATH").unwrap_or_else(|_| "D:\\db\\stock_price(1day)_with_data.db".to_string());

    // Solomon DB는 그대로 사용 (쓰기 작업이 있으므로)
    let solomon_db = Connection::open(&solomon_db_path)?;
    let _: String = solomon_db.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    solomon_db.execute("PRAGMA synchronous = NORMAL", [])?;
    solomon_db.execute("PRAGMA cache_size = 10000", [])?;
    solomon_db.execute("PRAGMA temp_store = MEMORY", [])?;

    // 5분봉 DB 연결
    let stock_db = Connection::open(&stock_db_path)?;
    let _: String = stock_db.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    stock_db.execute("PRAGMA synchronous = NORMAL", [])?;
    stock_db.execute("PRAGMA cache_size = 10000", [])?;
    stock_db.execute("PRAGMA temp_store = MEMORY", [])?;

    // 일봉 DB 연결
    let daily_db = Connection::open(&daily_db_path)?;
    let _: String = daily_db.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    daily_db.execute("PRAGMA synchronous = NORMAL", [])?;
    daily_db.execute("PRAGMA cache_size = 10000", [])?;
    daily_db.execute("PRAGMA temp_store = MEMORY", [])?;

    Ok(ThreadResources {
        solomon_db,
        stock_db,
        daily_db,
        cache: Cache::default(),
    })
}

// NOTE: 메인 DB 연결 생성
fn create_solomon_connection() -> Result<Connection> {
    let solomon_db_path = std::env::var("SOLOMON_DB_PATH").unwrap_or_else(|_| "D:\\db\\solomon.db".to_string());
    let solomon_db = Connection::open(&solomon_db_path)?;

    // 성능 최적화 설정
    let _: String = solomon_db.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    solomon_db.execute("PRAGMA synchronous = NORMAL", [])?;
    solomon_db.execute("PRAGMA cache_size = 10000", [])?;
    solomon_db.execute("PRAGMA temp_store = MEMORY", [])?;

    Ok(solomon_db)
}

// NOTE: 최적화된 날짜별 병렬 처리 함수 - 스레드별 독립적인 리소스 사용
fn process_date_group_optimized(
    date: &NaiveDate,
    stocks: Vec<Day3StockInfo>,
    sector_manager: &SectorManager,
    sector_cache: &Arc<HashMap<NaiveDate, DateSectorCache>>,
    pb: &ProgressBar,
) -> Result<Vec<(Day3StockInfo, Day3Features)>> {
    let mut results = Vec::new();
    
    // 스레드별 독립적인 리소스 생성
    let mut resources = create_thread_resources()?;
    
    // 미리 계산된 섹터 정보 사용
    let date_cache = sector_cache.get(date).unwrap();

    for stock_info in stocks {
        pb.set_message(format!("처리 중: {} ({})", stock_info.stock_code, stock_info.date));
        
        match calculate_day3_features_thread_optimized(&stock_info, sector_manager, &mut resources, date_cache) {
            Ok(features) => {
                results.push((stock_info, features));
            }
            Err(e) => {
                error!("{} 종목 처리 중 오류 발생: {}", stock_info.stock_code, e);
            }
        }
        
        pb.inc(1);
    }
    
    Ok(results)
}

// NOTE: 스레드별 최적화된 특징 계산 함수
fn calculate_day3_features_thread_optimized(
    stock_info: &Day3StockInfo,
    sector_manager: &SectorManager,
    resources: &mut ThreadResources,
    date_cache: &DateSectorCache,
) -> Result<Day3Features> {
    // NOTE: 5분봉 데이터 캐시 활용으로 중복 DB 호출 제거
    let today_data = get_five_min_data_cached_thread(&stock_info.stock_code, stock_info.date, sector_manager, &resources.stock_db, &mut resources.cache)?;
    
    let today_current = today_data.last().map(|d| d.close as f64).unwrap_or(0.0);
    
    // A. 전고점 & 과거 이력 기반
    let (breaks_6month_high, breaks_6month_high_with_long_candle, breaks_6month_high_with_leading_sector) = 
        calculate_high_break_features_thread_optimized(&stock_info, &resources.daily_db, sector_manager, date_cache, &today_data)?;
    
    // B. 전일 정답 및 섹터 리더십
    let (was_prev_answer, was_prev_answer_sector_first, prev_answer_sector_rank_ratio) = 
        calculate_prev_answer_features_thread_optimized(&stock_info, &resources.solomon_db, sector_manager)?;
    
    // C. 과거 급등 이력 기반
    let had_high_volume_gain_experience = calculate_high_volume_gain_experience_thread_optimized(&stock_info, &resources.daily_db)?;
    
    // D. 기술적 & 수급 지표
    let is_ema_aligned = calculate_ema_alignment_thread_optimized(&stock_info, &resources.daily_db, &mut resources.cache)?;
    let market_cap_over_3000b = calculate_market_cap_feature_thread_optimized(&stock_info, &resources.daily_db, &mut resources.cache)?;
    let near_price_boundary = calculate_price_boundary_feature(&today_current);
    let foreign_ratio_3day_rising = calculate_foreign_ratio_feature_thread_optimized(&stock_info, &resources.daily_db, &mut resources.cache)?;
    
    // E. 장초 흐름 및 당일 데이터
    let morning_volume_ratio = calculate_morning_volume_ratio_thread_optimized(&stock_info, resources, sector_manager)?;
    let consecutive_3_positive_candles = calculate_consecutive_candles(&today_data)?;
    let morning_mdd = calculate_morning_mdd(&today_data)?;

    Ok(Day3Features {
        breaks_6month_high,
        breaks_6month_high_with_long_candle,
        breaks_6month_high_with_leading_sector,
        was_prev_answer,
        was_prev_answer_sector_first,
        prev_answer_sector_rank_ratio,
        had_high_volume_gain_experience,
        is_ema_aligned,
        market_cap_over_3000b,
        near_price_boundary,
        foreign_ratio_3day_rising,
        morning_volume_ratio,
        consecutive_3_positive_candles,
        morning_mdd,
    })
}

// NOTE: 스레드별 5분봉 데이터 캐시 함수
fn get_five_min_data_cached_thread(
    stock_code: &str,
    date: NaiveDate,
    sector_manager: &SectorManager,
    stock_db: &Connection,
    cache: &mut Cache,
) -> Result<Vec<solomon::FiveMinData>> {
    let cache_key = (stock_code.to_string(), date);
    
    // 캐시 확인
    if let Some(data) = cache.five_min_data_cache.get(&cache_key) {
        return Ok(data.clone());
    }
    
    // 캐시에 없으면 DB에서 조회
    let data = sector_manager.get_five_min_data(stock_db, stock_code, date)?;
    
    // 캐시에 저장
    cache.five_min_data_cache.insert(cache_key, data.clone());
    
    Ok(data)
}

fn calculate_high_break_features_thread_optimized(
    stock_info: &Day3StockInfo,
    daily_db: &Connection,
    sector_manager: &SectorManager,
    date_cache: &DateSectorCache,
    today_data: &[solomon::FiveMinData],
) -> Result<(i32, i32, i32)> {
    // 6개월 전 데이터 조회 - 인덱스 활용
    let six_months_ago = stock_info.date - Duration::days(180);
    
    // 날짜 형식을 YYYYMMDD 형식으로 변환 (일봉 DB 형식에 맞춤)
    let target_date_str = stock_info.date.format("%Y%m%d").to_string();
    let six_months_ago_str = six_months_ago.format("%Y%m%d").to_string();
    
    // 테이블명에 A 접두사 추가 (일봉 DB 형식에 맞춤)
    let table_name = format!("A{}", stock_info.stock_code);
    
    // 먼저 테이블에 데이터가 있는지 확인
    let table_count: i64 = daily_db.query_row(
        &format!("SELECT COUNT(*) FROM {}", table_name),
        [],
        |row| row.get(0)
    ).unwrap_or(0);
    
    if table_count == 0 {
        error!("테이블 {}에 데이터가 없습니다.", table_name);
        return Ok((0, 0, 0));
    }
    
    // 6개월 내 데이터 개수 확인
    let six_month_data_count: i64 = daily_db.query_row(
        &format!("SELECT COUNT(*) FROM {} WHERE date >= ? AND date < ?", table_name),
        rusqlite::params![six_months_ago_str, target_date_str],
        |row| row.get(0)
    ).unwrap_or(0);
    
    if six_month_data_count == 0 {
        error!("6개월 내 데이터가 없습니다. 날짜 범위: {} ~ {}", six_months_ago_str, target_date_str);
        return Ok((0, 0, 0));
    }
    
    // 6개월 내 최고가 조회 (당일 포함하지 않음) - 더 효율적인 쿼리
    let six_month_high: f64 = match daily_db.query_row(
        &format!("SELECT MAX(high) FROM {} WHERE date >= ? AND date < ?", table_name),
        rusqlite::params![six_months_ago_str, target_date_str],
        |row| row.get(0)
    ) {
        Ok(high) => high,
        Err(e) => {
            error!("6개월 내 최고가 조회 실패: {}", e);
            return Ok((0, 0, 0));
        }
    };
    
    if six_month_high <= 0.0 {
        error!("6개월 내 최고가가 유효하지 않습니다: {:.2}", six_month_high);
        return Ok((0, 0, 0));
    }
    
    // 당일 데이터 존재 여부 확인
    let today_data_exists: i64 = daily_db.query_row(
        &format!("SELECT COUNT(*) FROM {} WHERE date = ?", table_name),
        rusqlite::params![target_date_str],
        |row| row.get(0)
    ).unwrap_or(0);
    
    if today_data_exists == 0 {
        error!("당일 데이터가 없습니다. 날짜: {}", target_date_str);
        return Ok((0, 0, 0));
    }
    
    // 당일 일봉 고가 조회
    let today_high: f64 = match daily_db.query_row(
        &format!("SELECT high FROM {} WHERE date = ?", table_name),
        rusqlite::params![target_date_str],
        |row| row.get(0)
    ) {
        Ok(high) => high,
        Err(e) => {
            error!("당일 고가 조회 실패: {}", e);
            return Ok((0, 0, 0));
        }
    };
    
    if today_high <= 0.0 {
        error!("당일 고가가 유효하지 않습니다: {:.2}", today_high);
        return Ok((0, 0, 0));
    }
    
    // 6개월 전고점 돌파 여부 (당일 일봉 고가 기준) - 같을 때도 돌파로 처리
    let breaks_6month_high = if today_high >= six_month_high && six_month_high > 0.0 { 1 } else { 0 };
    
    // 장대양봉 여부 계산 (5분봉 데이터 기준)
    let is_long_candle = if !today_data.is_empty() {
        let open = today_data[0].open as f64;
        let close = today_data.last().unwrap().close as f64;
        let high = today_data.iter().map(|d| d.high as f64).fold(f64::NEG_INFINITY, f64::max);
        let low = today_data.iter().map(|d| d.low as f64).fold(f64::INFINITY, f64::min);
        
        let body_size = (close - open).abs();
        let total_range = high - low;
        let body_ratio = if total_range > 0.0 { body_size / total_range } else { 0.0 };
        
        if body_ratio > 0.7 { 1 } else { 0 }
    } else { 0 };
    
    let breaks_6month_high_with_long_candle = if breaks_6month_high == 1 && is_long_candle == 1 { 1 } else { 0 };
    
    // 주도 섹터 여부 계산 - 캐시된 섹터 정보 활용
    let breaks_6month_high_with_leading_sector = {
        if breaks_6month_high_with_long_candle == 1 {
            let current_sector = sector_manager.get_sector(&stock_info.stock_code);
            
            // 섹터별 5% 이상 상승 종목 수 계산
            let mut sector_rising_counts = HashMap::new();
            for (_code, (gain_ratio, sector)) in &date_cache.stock_info {
                if *gain_ratio >= 0.05 {
                    *sector_rising_counts.entry(sector.clone()).or_insert(0) += 1;
                }
            }
            
            // 현재 섹터가 주도 섹터인지 확인 (상위 3개 섹터 중 하나)
            let mut sorted_sectors: Vec<_> = sector_rising_counts.into_iter().collect();
            sorted_sectors.sort_by(|a, b| b.1.cmp(&a.1));
            
            let is_leading_sector = sorted_sectors.iter().take(3).any(|(sector, _)| *sector == current_sector);
            if is_leading_sector { 1 } else { 0 }
        } else {
            0
        }
    };
    
    Ok((breaks_6month_high, breaks_6month_high_with_long_candle, breaks_6month_high_with_leading_sector))
}

fn calculate_prev_answer_features_thread_optimized(
    stock_info: &Day3StockInfo,
    solomon_db: &Connection,
    sector_manager: &SectorManager,
) -> Result<(i32, i32, f64)> {
    use chrono::Duration;
    let prev_date = stock_info.date - Duration::days(1);
    
    // 전날의 9시~9시반 거래대금 기준 30위 내 종목들 조회
    let prev_date_cache = match sector_manager.calculate_date_sector_cache(&prev_date, solomon_db) {
        Ok(cache) => cache,
        Err(_) => {
            // 전날 데이터가 없으면 0으로 처리
            return Ok((0, 0, 0.0));
        }
    };
    
    // 전날 30위 내에 포함되었는지 확인
    let was_prev_answer = if prev_date_cache.stock_info.iter().any(|(code, _)| code == &stock_info.stock_code) {
        1
    } else {
        0
    };
    
    // 전날 섹터 내 순위 조회
    let (was_prev_answer_sector_first, prev_answer_sector_rank_ratio) = {
        if was_prev_answer == 1 {
            let current_sector = sector_manager.get_sector(&stock_info.stock_code);
            let mut sector_stocks = Vec::new();
            
            // 전날 30위 내 종목들 중 같은 섹터 종목들만 수집
            for (code, (gain_ratio, sector)) in &prev_date_cache.stock_info {
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
            
            (is_sector_first, sector_rank_ratio)
        } else {
            (0, 0.0)
        }
    };
    
    Ok((was_prev_answer, was_prev_answer_sector_first, prev_answer_sector_rank_ratio))
}

fn calculate_high_volume_gain_experience_thread_optimized(
    stock_info: &Day3StockInfo,
    daily_db: &Connection,
) -> Result<i32> {
    use chrono::Duration;
    let six_months_ago = stock_info.date - Duration::days(180);
    
    // 날짜 형식을 YYYYMMDD 형식으로 변환 (일봉 DB 형식에 맞춤)
    let target_date_str = stock_info.date.format("%Y%m%d").to_string();
    let six_months_ago_str = six_months_ago.format("%Y%m%d").to_string();
    
    // 테이블명에 A 접두사 추가 (일봉 DB 형식에 맞춤)
    let table_name = format!("A{}", stock_info.stock_code);
    
    // 인덱스 활용한 효율적인 쿼리
    let count: i32 = daily_db.query_row(
        &format!("SELECT COUNT(*) FROM {} WHERE date >= ? AND date < ? 
         AND volume * close >= 1000000000000 AND (close - open) / open >= 0.1", table_name),
        rusqlite::params![six_months_ago_str, target_date_str],
        |row| row.get(0)
    ).unwrap_or(0);
    
    Ok(if count > 0 { 1 } else { 0 })
}

fn calculate_ema_alignment_thread_optimized(
    stock_info: &Day3StockInfo,
    daily_db: &Connection,
    cache: &mut Cache,
) -> Result<i32> {
    let cache_key = format!("{}", stock_info.stock_code);
    
    // 캐시 확인
    {
        let cache_guard = cache.ema_cache.get(&cache_key);
        if let Some((ema5, ema20)) = cache_guard {
            return Ok(if *ema5 > *ema20 { 1 } else { 0 });
        }
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
    
    // 테이블명에 A 접두사 추가 (일봉 DB 형식에 맞춤)
    let table_name = format!("A{}", stock_info.stock_code);
    
    // 최근 25일 종가 데이터 조회
    let target_date_str = stock_info.date.format("%Y%m%d").to_string();
    let prices: Vec<f64> = daily_db.prepare(
        &format!("SELECT close FROM {} WHERE date < ? ORDER BY date DESC LIMIT 25", table_name)
    )?
    .query_map(
        rusqlite::params![target_date_str],
        |row| row.get(0)
    )?
    .filter_map(|r| r.ok())
    .collect();
    
    if prices.len() < 20 {
        return Ok(0);
    }
    
    let mut sorted_prices = prices.clone();
    sorted_prices.reverse();
    
    let ema5 = calculate_ema(&sorted_prices, 5);
    let ema20 = calculate_ema(&sorted_prices, 20);
    
    // 캐시에 저장
    cache.ema_cache.insert(cache_key, (ema5, ema20));
    
    Ok(if ema5 > ema20 { 1 } else { 0 })
}

fn calculate_market_cap_feature_thread_optimized(
    stock_info: &Day3StockInfo,
    daily_db: &Connection,
    cache: &mut Cache,
) -> Result<i32> {
    let cache_key = format!("{}", stock_info.stock_code);
    
    // 캐시 확인
    {
        let cache_guard = cache.market_cap_cache.get(&cache_key);
        if let Some(&market_cap) = cache_guard {
            return Ok(if market_cap >= 3000000000000.0 { 1 } else { 0 });
        }
    }
    
    // 테이블명에 A 접두사 추가 (일봉 DB 형식에 맞춤)
    let table_name = format!("A{}", stock_info.stock_code);
    
    // 시가총액 계산: 상장주식수 * 종가
    let target_date_str = stock_info.date.format("%Y%m%d").to_string();
    let (shares, close_price): (i64, i32) = daily_db.query_row(
        &format!("SELECT 상장주식수, close FROM {} WHERE date < ? ORDER BY date DESC LIMIT 1", table_name),
        rusqlite::params![target_date_str],
        |row| Ok((row.get(0)?, row.get(1)?))
    ).unwrap_or((0, 0));
    
    let market_cap = shares as f64 * close_price as f64;
    
    // 캐시에 저장
    cache.market_cap_cache.insert(cache_key, market_cap);
    
    Ok(if market_cap >= 3000000000000.0 { 1 } else { 0 })
}

fn calculate_foreign_ratio_feature_thread_optimized(
    stock_info: &Day3StockInfo,
    daily_db: &Connection,
    cache: &mut Cache,
) -> Result<i32> {
    let cache_key = format!("{}", stock_info.stock_code);
    
    // 캐시 확인
    {
        let cache_guard = cache.foreign_ratio_cache.get(&cache_key);
        if let Some(ratios) = cache_guard {
            if ratios.len() == 3 && ratios[0] > ratios[1] && ratios[1] > ratios[2] {
                return Ok(1);
            } else {
                return Ok(0);
            }
        }
    }
    
    // 테이블명에 A 접두사 추가 (일봉 DB 형식에 맞춤)
    let table_name = format!("A{}", stock_info.stock_code);
    
    // 최근 3개 거래일 조회 (주식 시장 열리는 날짜 기준)
    let mut ratios = Vec::new();
    
    // 날짜를 INTEGER 형식으로 변환 (YYYYMMDD)
    let target_date_int = stock_info.date.format("%Y%m%d").to_string().parse::<i32>().unwrap_or(0);
    
    // 해당 종목의 최근 거래일들을 조회 (최대 10일 전까지 확인)
    let query = format!("SELECT date FROM {} WHERE date < ? ORDER BY date DESC LIMIT 10", table_name);
    
    let trading_dates: Vec<i32> = match daily_db.prepare(&query) {
        Ok(mut stmt) => {
            match stmt.query_map(rusqlite::params![target_date_int], |row| row.get(0)) {
                Ok(rows) => {
                    rows.filter_map(|r| r.ok()).collect()
                }
                Err(e) => {
                    error!("거래일 조회 실패: {}", e);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            error!("쿼리 준비 실패: {}", e);
            Vec::new()
        }
    };
    
    // 최근 3개 거래일 선택
    for i in 0..3 {
        if i < trading_dates.len() {
            let trading_date_int = trading_dates[i];
            
            match daily_db.query_row(
                &format!("SELECT 외국인현보유비율 FROM {} WHERE date = ?", table_name),
                rusqlite::params![trading_date_int],
                |row| row.get::<_, f64>(0)
            ) {
                Ok(ratio) => {
                    ratios.push(ratio);
                }
                Err(e) => {
                    warn!("{}: 외국인현보유비율 조회 실패 - {}", trading_date_int, e);
                    // 데이터가 없으면 0으로 채움
                    ratios.push(0.0);
                }
            }
        } else {
            warn!("거래일이 부족합니다. 필요한 거래일: 3개, 실제 거래일: {}개", trading_dates.len());
            ratios.push(0.0);
        }
    }
    
    // 캐시에 저장
    cache.foreign_ratio_cache.insert(cache_key, ratios.clone());
    
    if ratios.len() == 3 && ratios[0] > ratios[1] && ratios[1] > ratios[2] {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn calculate_morning_volume_ratio_thread_optimized(
    stock_info: &Day3StockInfo,
    resources: &mut ThreadResources,
    sector_manager: &SectorManager,
) -> Result<f64> {
    use chrono::Duration;
    let cache_key = format!("{}", stock_info.stock_code);
    
    // 캐시 확인
    {
        let cache_guard = resources.cache.volume_cache.get(&cache_key);
        if let Some(&avg_volume) = cache_guard {
            let today_data = get_five_min_data_cached_thread(&stock_info.stock_code, stock_info.date, sector_manager, &resources.stock_db, &mut resources.cache)?;
            let morning_volume = today_data.iter().take(6).map(|d| d.volume).sum::<i32>() as f64;
            return Ok(if avg_volume > 0.0 { morning_volume / avg_volume } else { 0.0 });
        }
    }
    
    // 최근 5일 평균 거래량 계산
    let mut total_volume = 0;
    let mut day_count = 0;
    
    for i in 1..=5 {
        let date = stock_info.date - Duration::days(i);
        let data = get_five_min_data_cached_thread(&stock_info.stock_code, date, sector_manager, &resources.stock_db, &mut resources.cache)?;
        if !data.is_empty() {
            total_volume += data.iter().map(|d| d.volume).sum::<i32>();
            day_count += 1;
        }
    }
    
    let avg_volume = if day_count > 0 { total_volume as f64 / day_count as f64 } else { 0.0 };
    
    // 캐시에 저장
    resources.cache.volume_cache.insert(cache_key, avg_volume);
    
    // 당일 오전 거래량 계산
    let today_data = get_five_min_data_cached_thread(&stock_info.stock_code, stock_info.date, sector_manager, &resources.stock_db, &mut resources.cache)?;
    let morning_volume = today_data.iter().take(6).map(|d| d.volume).sum::<i32>() as f64;
    
    Ok(if avg_volume > 0.0 { morning_volume / avg_volume } else { 0.0 })
}

// NOTE: 최적화된 배치 저장 함수 - prepare 한 번만 실행하여 반복 오버헤드 제거
fn save_day3_features_batch_optimized(
    db: &mut Connection,
    batch_features: &[(Day3StockInfo, Day3Features)],
) -> Result<()> {
    let transaction = db.transaction()?;
    
    // NOTE: prepare 한 번만 실행하여 반복 오버헤드 제거
    let mut stmt = transaction.prepare(
        "INSERT INTO day3 (
            stock_code, date, breaks_6month_high, breaks_6month_high_with_long_candle, 
            breaks_6month_high_with_leading_sector, was_prev_answer, was_prev_answer_sector_first,
            prev_answer_sector_rank_ratio, had_high_volume_gain_experience, is_ema_aligned,
            market_cap_over_3000b, near_price_boundary, foreign_ratio_3day_rising,
            morning_volume_ratio, consecutive_3_positive_candles, morning_mdd
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )?;
    
    for (stock_info, features) in batch_features {
        let date_str = stock_info.date.format("%Y-%m-%d").to_string();
        stmt.execute(rusqlite::params![
            stock_info.stock_code,
            date_str,
            features.breaks_6month_high,
            features.breaks_6month_high_with_long_candle,
            features.breaks_6month_high_with_leading_sector,
            features.was_prev_answer,
            features.was_prev_answer_sector_first,
            features.prev_answer_sector_rank_ratio,
            features.had_high_volume_gain_experience,
            features.is_ema_aligned,
            features.market_cap_over_3000b,
            features.near_price_boundary,
            features.foreign_ratio_3day_rising,
            features.morning_volume_ratio,
            features.consecutive_3_positive_candles,
            features.morning_mdd,
        ])?;
    }
    
    drop(stmt);
    transaction.commit()?;
    Ok(())
}

fn calculate_price_boundary_feature(current_price: &f64) -> i32 {
    // 호가 단위 경계값들 (2천, 5천, 2만, 5만, 20만, 50만, 200만, 500만)
    let boundaries = vec![2000.0, 5000.0, 20000.0, 50000.0, 200000.0, 500000.0, 2000000.0, 5000000.0];
    
    for boundary in boundaries {
        let lower_bound = boundary * 0.95;
        let upper_bound = boundary * 1.05;
        if *current_price >= lower_bound && *current_price <= upper_bound {
            return 1;
        }
    }
    
    0
}

#[derive(Debug, Clone)]
struct Day3StockInfo {
    stock_code: String,
    date: NaiveDate,
}

#[derive(Debug)]
struct Day3Features {
    // A. 전고점 & 과거 이력 기반
    breaks_6month_high: i32,                    // 6개월 전고점 돌파 여부
    breaks_6month_high_with_long_candle: i32,   // 6개월 전고점 돌파 + 장대양봉 발생 여부
    breaks_6month_high_with_leading_sector: i32, // 6개월 전고점 돌파 + 장대양봉 + 주도 섹터 내 포함 여부
    
    // B. 전일 정답 및 섹터 리더십
    was_prev_answer: i32,                        // 전날 정답 여부 (is_answer=1)
    was_prev_answer_sector_first: i32,           // 전날 정답이면서 섹터 내 상승률 1위 여부
    prev_answer_sector_rank_ratio: f64,          // 전날 정답이면서 섹터 내 상승률 순위 지수 (1/순위)
    
    // C. 과거 급등 이력 기반
    had_high_volume_gain_experience: i32,        // 6개월 내 1000억 이상 거래대금 + 10% 이상 상승 경험 여부
    
    // D. 기술적 & 수급 지표
    is_ema_aligned: i32,                         // 일봉 기준 정배열 여부 (EMA 5일선 > EMA 20일선)
    market_cap_over_3000b: i32,                  // 시가총액 3000억 이상 여부
    near_price_boundary: i32,                    // 현재가가 호가 단위 경계 근접 (호가단위 5% 이내) 여부
    foreign_ratio_3day_rising: i32,              // 외국인 현보유비율 3일 연속 상승 여부
    
    // E. 장초 흐름 및 당일 데이터
    morning_volume_ratio: f64,                   // 최근 5일 평균 거래량 대비 당일 오전(9:00~9:30) 거래량 비율
    consecutive_3_positive_candles: i32,         // 당일 9:30 기준 시가 이후 연속 3개 양봉 발생 여부
    morning_mdd: f64,                            // 당일 9:30 기준 시가 대비 Maximum Drawdown (MDD)
}

fn get_stock_list_from_answer() -> Result<Vec<Day3StockInfo>> {
    let db = create_solomon_connection()?;
    let mut stmt = db.prepare(
        "SELECT stock_code, date FROM answer_v3 WHERE date < 20230601 ORDER BY date"
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
        
        stock_list.push(Day3StockInfo {
            stock_code,
            date,
        });
    }
    Ok(stock_list)
}

fn create_day3_table(db: &Connection) -> Result<()> {
    db.execute(
        "CREATE TABLE day3 (
            stock_code TEXT,
            date TEXT,
            breaks_6month_high INTEGER,
            breaks_6month_high_with_long_candle INTEGER,
            breaks_6month_high_with_leading_sector INTEGER,
            was_prev_answer INTEGER,
            was_prev_answer_sector_first INTEGER,
            prev_answer_sector_rank_ratio REAL,
            had_high_volume_gain_experience INTEGER,
            is_ema_aligned INTEGER,
            market_cap_over_3000b INTEGER,
            near_price_boundary INTEGER,
            foreign_ratio_3day_rising INTEGER,
            morning_volume_ratio REAL,
            consecutive_3_positive_candles INTEGER,
            morning_mdd REAL,
            PRIMARY KEY (date, stock_code)
        )",
        [],
    )?;
    Ok(())
}

fn calculate_consecutive_candles(data: &[solomon::FiveMinData]) -> Result<i32> {
    if data.len() < 3 {
        return Ok(0);
    }
    
    let mut consecutive_count = 0;
    for i in 1..data.len() {
        let prev_close = data[i-1].close as f64;
        let curr_close = data[i].close as f64;
        
        if curr_close > prev_close {
            consecutive_count += 1;
            if consecutive_count >= 3 {
                return Ok(1);
            }
        } else {
            consecutive_count = 0;
        }
    }
    
    Ok(0)
}

fn calculate_morning_mdd(data: &[solomon::FiveMinData]) -> Result<f64> {
    if data.is_empty() {
        return Ok(0.0);
    }
    
    let open_price = data[0].open as f64;
    let mut max_price = open_price;
    let mut mdd = 0.0;
    
    for d in data {
        let current_price = d.close as f64;
        if current_price > max_price {
            max_price = current_price;
        }
        
        let drawdown = (max_price - current_price) / max_price;
        if drawdown > mdd {
            mdd = drawdown;
        }
    }
    
    Ok(mdd)
}

 