use chrono::Duration;
use chrono::NaiveDate;
use log::{error, info, warn};
use rusqlite::{Connection, Result};
use solomon::{DateSectorCache, FiveMinData, SectorManager};
use std::collections::HashMap;
use std::time::Instant;

// NOTE: day3.rs의 calculate_high_break_features_thread_optimized 함수 분석용
// 기존 로직을 그대로 가져와서 결과를 분석할 수 있도록 함

// 데이터베이스 연결 풀 구조체 (day1, day2와 동일한 방식)
struct DbPool {
    solomon_db: Connection,
    stock_db: Connection,
    daily_db: Connection,
}

struct Cache {
    high_break_analysis: HashMap<String, String>, // 분석 결과 저장용
}

impl Cache {
    fn new() -> Self {
        Cache {
            high_break_analysis: HashMap::new(),
        }
    }
}

fn main() {
    // 로거 초기화
    env_logger::init();
    info!("6개월 전고점 돌파 특징 계산 분석 프로그램 시작");

    let start_time = Instant::now();

    // 섹터 정보 로드
    let mut sector_manager = SectorManager::new();
    if let Err(e) = sector_manager.load_from_csv("sector_utf8.csv") {
        warn!("섹터 정보 로드 실패: {}. 기본값으로 진행합니다.", e);
    } else {
        info!("섹터 정보 로드 완료");
    }

    // 테스트용 종목 정보 (실제로는 DB에서 가져와야 함)
    let test_stocks = vec![
        Day3StockInfo {
            stock_code: "005930".to_string(), // 삼성전자
            date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        },
        Day3StockInfo {
            stock_code: "000660".to_string(), // SK하이닉스
            date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        },
        Day3StockInfo {
            stock_code: "035420".to_string(), // NAVER
            date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        },
    ];

    // 데이터베이스 연결 풀 생성 (day1, day2와 동일한 방식)
    let mut db_pool = match create_db_pool() {
        Ok(pool) => pool,
        Err(e) => {
            error!("데이터베이스 연결 풀 생성 실패: {}", e);
            return;
        }
    };

    // DB 테이블 확인
    info!("일봉 DB 테이블 목록 확인 중...");
    let daily_tables: Vec<String> = db_pool
        .daily_db
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    info!("일봉 DB 발견된 테이블: {:?}", daily_tables);

    info!("5분봉 DB 테이블 목록 확인 중...");
    let stock_tables: Vec<String> = db_pool
        .stock_db
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    info!("5분봉 DB 발견된 테이블: {:?}", stock_tables);

    let cache = Cache::new();

    // 각 종목에 대해 기존 로직으로 6개월 전고점 돌파 특징 계산
    for stock_info in test_stocks {
        info!("=== {} 종목 분석 ===", stock_info.stock_code);

        // 테이블명에 A 접두사 추가
        let table_name = format!("A{}", stock_info.stock_code);
        info!("실제 테이블명: {}", table_name);

        // 일봉 테이블 존재 여부 확인
        let daily_table_exists: bool = db_pool
            .daily_db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
                rusqlite::params![table_name],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !daily_table_exists {
            error!("일봉 테이블 '{}'이 존재하지 않습니다.", table_name);
            continue;
        }

        // 5분봉 테이블 존재 여부 확인
        let stock_table_exists: bool = db_pool
            .stock_db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
                rusqlite::params![table_name],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !stock_table_exists {
            error!("5분봉 테이블 '{}'이 존재하지 않습니다.", table_name);
            continue;
        }

        // 일봉 테이블 스키마 확인
        let daily_schema: String = db_pool
            .daily_db
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name = ?",
                rusqlite::params![table_name],
                |row| row.get(0),
            )
            .unwrap_or_default();

        info!("일봉 테이블 스키마: {}", daily_schema);

        // 일봉 컬럼 존재 여부 확인
        let daily_columns: Vec<String> = db_pool
            .daily_db
            .prepare(&format!("PRAGMA table_info({})", table_name))
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        info!("일봉 컬럼 목록: {:?}", daily_columns);

        // 필요한 컬럼들이 있는지 확인
        let required_columns = vec!["high", "open", "close", "low"];
        for col in &required_columns {
            if !daily_columns.iter().any(|c| c == col) {
                error!("필요한 컬럼 '{}'이 일봉 테이블에 존재하지 않습니다.", col);
            }
        }

        // 5분봉 컬럼 존재 여부 확인
        let stock_columns: Vec<String> = db_pool
            .stock_db
            .prepare(&format!("PRAGMA table_info({})", table_name))
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        info!("5분봉 컬럼 목록: {:?}", stock_columns);

        // 필요한 5분봉 컬럼들이 있는지 확인
        let required_stock_columns = vec!["open", "high", "low", "close", "volume"];
        for col in &required_stock_columns {
            if !stock_columns.iter().any(|c| c == col) {
                error!("필요한 컬럼 '{}'이 5분봉 테이블에 존재하지 않습니다.", col);
            }
        }

        // 일봉 데이터 범위 확인
        let daily_date_range: Vec<(String, String)> = db_pool
            .daily_db
            .prepare(&format!("SELECT MIN(date), MAX(date) FROM {}", table_name))
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        if let Some((min_date, max_date)) = daily_date_range.first() {
            info!("일봉 데이터 범위: {} ~ {}", min_date, max_date);
        }

        // 일봉 데이터 샘플 확인 (최근 5개)
        let daily_sample: Vec<(String, f64, f64, f64, f64)> = db_pool
            .daily_db
            .prepare(&format!(
                "SELECT date, open, high, low, close FROM {} ORDER BY date DESC LIMIT 5",
                table_name
            ))
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        info!("일봉 데이터 샘플 (최근 5개):");
        for (date, open, high, low, close) in &daily_sample {
            info!(
                "  {}: O={:.2}, H={:.2}, L={:.2}, C={:.2}",
                date, open, high, low, close
            );
        }

        // 특정 날짜 데이터 확인 (테스트용)
        let test_date = "20240115";
        let specific_date_data: Vec<(String, f64, f64, f64, f64)> = db_pool
            .daily_db
            .prepare(&format!(
                "SELECT date, open, high, low, close FROM {} WHERE date = ?",
                table_name
            ))
            .unwrap()
            .query_map([test_date], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        if let Some((date, open, high, low, close)) = specific_date_data.first() {
            info!(
                "특정 날짜 {} 데이터: O={:.2}, H={:.2}, L={:.2}, C={:.2}",
                date, open, high, low, close
            );
        } else {
            info!("특정 날짜 {} 데이터가 없습니다.", test_date);
        }

        // 5분봉 데이터 범위 확인
        let stock_date_range: Vec<(String, String)> = db_pool
            .stock_db
            .prepare(&format!("SELECT MIN(date), MAX(date) FROM {}", table_name))
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        if let Some((min_date, max_date)) = stock_date_range.first() {
            info!("5분봉 데이터 범위: {} ~ {}", min_date, max_date);
        }

        // 실제 섹터 캐시 계산 (day3.rs와 동일한 방식)
        info!("섹터 캐시 계산 중...");
        let date_cache = match sector_manager
            .calculate_date_sector_cache(&stock_info.date, &db_pool.solomon_db)
        {
            Ok(cache) => {
                info!(
                    "섹터 캐시 계산 완료: {}개 종목 정보",
                    cache.stock_info.len()
                );
                cache
            }
            Err(e) => {
                error!("섹터 캐시 계산 실패: {}", e);
                continue;
            }
        };

        // 5분봉 데이터 가져오기 (day1, day2와 동일한 방식)
        let today_data = match sector_manager.get_five_min_data(
            &db_pool.stock_db,
            &stock_info.stock_code,
            stock_info.date,
        ) {
            Ok(data) => {
                info!("5분봉 데이터 가져오기 완료: {}개 데이터", data.len());
                data
            }
            Err(e) => {
                error!("5분봉 데이터 가져오기 실패: {}", e);
                continue;
            }
        };

        // 수정된 Day3StockInfo로 함수 호출
        let modified_stock_info = Day3StockInfo {
            stock_code: table_name.clone(),
            date: stock_info.date,
        };

        match calculate_high_break_features_thread_optimized(
            &modified_stock_info,
            &mut db_pool.daily_db,
            &sector_manager,
            &date_cache,
            &today_data,
        ) {
            Ok((
                breaks_6month_high,
                breaks_6month_high_with_long_candle,
                breaks_6month_high_with_leading_sector,
            )) => {
                info!("=== 6개월 전고점 돌파 특징 계산 결과 ===");
                info!("1. 6개월 전고점 돌파 여부: {}", breaks_6month_high);
                info!(
                    "2. 6개월 전고점 돌파 + 장대양봉: {}",
                    breaks_6month_high_with_long_candle
                );
                info!(
                    "3. 6개월 전고점 돌파 + 장대양봉 + 주도 섹터: {}",
                    breaks_6month_high_with_leading_sector
                );

                // 상세 분석 결과 출력
                if let Some(analysis) = cache.high_break_analysis.get(&table_name) {
                    info!("=== 상세 분석 결과 ===");
                    info!("{}", analysis);
                }
            }
            Err(e) => {
                error!("{} 종목 처리 중 오류: {}", table_name, e);
            }
        }

        info!("");
    }

    let elapsed = start_time.elapsed();
    info!("6개월 전고점 돌파 특징 계산 분석 완료: {:.2?}", elapsed);
}

fn create_db_pool() -> Result<DbPool> {
    let solomon_db_path =
        std::env::var("SOLOMON_DB_PATH").unwrap_or_else(|_| "D:\\db\\solomon.db".to_string());
    let stock_db_path = std::env::var("STOCK_DB_PATH")
        .unwrap_or_else(|_| "D:\\db\\stock_price(5min).db".to_string());
    let daily_db_path = std::env::var("DAILY_DB_PATH")
        .unwrap_or_else(|_| "D:\\db\\stock_price(1day)_with_data.db".to_string());

    info!("일봉 DB 경로: {}", daily_db_path);
    info!("5분봉 DB 경로: {}", stock_db_path);
    info!("Solomon DB 경로: {}", solomon_db_path);

    let solomon_db = Connection::open(&solomon_db_path)?;
    let stock_db = Connection::open(&stock_db_path)?;
    let daily_db = Connection::open(&daily_db_path)?;

    // 성능 최적화 설정
    let _: String = solomon_db.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    solomon_db.execute("PRAGMA synchronous = NORMAL", [])?;
    solomon_db.execute("PRAGMA cache_size = 10000", [])?;
    solomon_db.execute("PRAGMA temp_store = MEMORY", [])?;

    let _: String = stock_db.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    stock_db.execute("PRAGMA synchronous = NORMAL", [])?;
    stock_db.execute("PRAGMA cache_size = 10000", [])?;
    stock_db.execute("PRAGMA temp_store = MEMORY", [])?;

    let _: String = daily_db.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    daily_db.execute("PRAGMA synchronous = NORMAL", [])?;
    daily_db.execute("PRAGMA cache_size = 10000", [])?;
    daily_db.execute("PRAGMA temp_store = MEMORY", [])?;

    Ok(DbPool {
        solomon_db,
        stock_db,
        daily_db,
    })
}

// NOTE: day3.rs의 원본 calculate_high_break_features_thread_optimized 함수 (수정된 버전)
fn calculate_high_break_features_thread_optimized(
    stock_info: &Day3StockInfo,
    daily_db: &Connection,
    sector_manager: &SectorManager,
    date_cache: &DateSectorCache,
    today_data: &Vec<FiveMinData>,
) -> Result<(i32, i32, i32)> {
    info!("=== 6개월 전고점 돌파 특징 계산 시작 ===");

    // 6개월 전 데이터 조회 - 인덱스 활용
    let six_months_ago = stock_info.date - Duration::days(180);
    info!("기준 날짜: {}", stock_info.date.format("%Y-%m-%d"));
    info!("6개월 전 날짜: {}", six_months_ago.format("%Y-%m-%d"));

    // 날짜 형식을 YYYYMMDD 형식으로 변환 (일봉 DB 형식에 맞춤)
    let target_date_str = stock_info.date.format("%Y%m%d").to_string();
    let six_months_ago_str = six_months_ago.format("%Y%m%d").to_string();

    info!("변환된 날짜 형식:");
    info!("  기준 날짜 (YYYYMMDD): {}", target_date_str);
    info!("  6개월 전 날짜 (YYYYMMDD): {}", six_months_ago_str);

    // 먼저 테이블에 데이터가 있는지 확인
    let table_count: i64 = daily_db
        .query_row(
            &format!("SELECT COUNT(*) FROM {}", stock_info.stock_code),
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    info!(
        "테이블 {} 전체 데이터 개수: {}",
        stock_info.stock_code, table_count
    );

    if table_count == 0 {
        error!("테이블 {}에 데이터가 없습니다.", stock_info.stock_code);
        return Ok((0, 0, 0));
    }

    // 6개월 내 최고가 조회 (당일 포함하지 않음) - 더 효율적인 쿼리
    let six_month_high_query = format!(
        "SELECT MAX(high) FROM {} WHERE date >= ? AND date < ?",
        stock_info.stock_code
    );
    info!("6개월 최고가 조회 쿼리: {}", six_month_high_query);
    info!(
        "쿼리 파라미터: {} ~ {}",
        six_months_ago_str, target_date_str
    );

    // 6개월 내 데이터 개수 확인
    let six_month_data_count: i64 = daily_db
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM {} WHERE date >= ? AND date < ?",
                stock_info.stock_code
            ),
            rusqlite::params![six_months_ago_str, target_date_str],
            |row| row.get(0),
        )
        .unwrap_or(0);

    info!("6개월 내 데이터 개수: {}", six_month_data_count);

    if six_month_data_count == 0 {
        error!(
            "6개월 내 데이터가 없습니다. 날짜 범위: {} ~ {}",
            six_months_ago_str, target_date_str
        );
        return Ok((0, 0, 0));
    }

    // 6개월 내 최고가 조회
    let six_month_high: f64 = match daily_db.query_row(
        &six_month_high_query,
        rusqlite::params![six_months_ago_str, target_date_str],
        |row| row.get(0),
    ) {
        Ok(high) => {
            info!("6개월 내 최고가 조회 성공: {:.2}", high);
            high
        }
        Err(e) => {
            error!("6개월 내 최고가 조회 실패: {}", e);
            return Ok((0, 0, 0));
        }
    };

    if six_month_high <= 0.0 {
        error!("6개월 내 최고가가 유효하지 않습니다: {:.2}", six_month_high);
        return Ok((0, 0, 0));
    }

    // 당일 일봉 고가 조회
    let today_high_query = format!("SELECT high FROM {} WHERE date = ?", stock_info.stock_code);
    info!("당일 고가 조회 쿼리: {}", today_high_query);
    info!("쿼리 파라미터: {}", target_date_str);

    // 당일 데이터 존재 여부 확인
    let today_data_exists: i64 = daily_db
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM {} WHERE date = ?",
                stock_info.stock_code
            ),
            rusqlite::params![target_date_str],
            |row| row.get(0),
        )
        .unwrap_or(0);

    info!(
        "당일 데이터 존재 여부: {} (1: 존재, 0: 없음)",
        today_data_exists
    );

    if today_data_exists == 0 {
        error!("당일 데이터가 없습니다. 날짜: {}", target_date_str);
        return Ok((0, 0, 0));
    }

    // 당일 고가 조회
    let today_high: f64 = match daily_db.query_row(
        &today_high_query,
        rusqlite::params![target_date_str],
        |row| row.get(0),
    ) {
        Ok(high) => {
            info!("당일 고가 조회 성공: {:.2}", high);
            high
        }
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
    let breaks_6month_high = if today_high >= six_month_high && six_month_high > 0.0 {
        1
    } else {
        0
    };
    info!(
        "6개월 전고점 돌파 여부: {} (조건: {} >= {} && {} > 0.0)",
        breaks_6month_high, today_high, six_month_high, six_month_high
    );

    // 장대양봉 여부 계산 (5분봉 데이터 기준)
    let is_long_candle = if !today_data.is_empty() {
        let open = today_data[0].open as f64;
        let close = today_data.last().unwrap().close as f64;
        let high = today_data
            .iter()
            .map(|d| d.high as f64)
            .fold(f64::NEG_INFINITY, f64::max);
        let low = today_data
            .iter()
            .map(|d| d.low as f64)
            .fold(f64::INFINITY, f64::min);

        let body_size = (close - open).abs();
        let total_range = high - low;
        let body_ratio = if total_range > 0.0 {
            body_size / total_range
        } else {
            0.0
        };

        info!("장대양봉 계산:");
        info!("  시가: {:.2}", open);
        info!("  종가: {:.2}", close);
        info!("  고가: {:.2}", high);
        info!("  저가: {:.2}", low);
        info!("  몸통 크기: {:.2}", body_size);
        info!("  전체 범위: {:.2}", total_range);
        info!("  몸통 비율: {:.2} (기준: > 0.7)", body_ratio);

        if body_ratio > 0.7 {
            1
        } else {
            0
        }
    } else {
        info!("5분봉 데이터가 없어서 장대양봉 계산 불가");
        0
    };

    info!("장대양봉 여부: {}", is_long_candle);

    let breaks_6month_high_with_long_candle = if breaks_6month_high == 1 && is_long_candle == 1 {
        1
    } else {
        0
    };
    info!(
        "6개월 전고점 돌파 + 장대양봉: {} (조건: {} == 1 && {} == 1)",
        breaks_6month_high_with_long_candle, breaks_6month_high, is_long_candle
    );

    // 주도 섹터 여부 계산 - 캐시된 섹터 정보 활용
    let breaks_6month_high_with_leading_sector = {
        if breaks_6month_high_with_long_candle == 1 {
            let current_sector = sector_manager.get_sector(&stock_info.stock_code);
            info!("현재 섹터: {}", current_sector);

            // 섹터별 5% 이상 상승 종목 수 계산
            let mut sector_rising_counts = HashMap::new();
            for (_code, (gain_ratio, sector)) in &date_cache.stock_info {
                if *gain_ratio >= 0.05 {
                    *sector_rising_counts.entry(sector.clone()).or_insert(0) += 1;
                }
            }

            info!("섹터별 5% 이상 상승 종목 수: {:?}", sector_rising_counts);

            // 현재 섹터가 주도 섹터인지 확인 (상위 3개 섹터 중 하나)
            let mut sorted_sectors: Vec<_> = sector_rising_counts.into_iter().collect();
            sorted_sectors.sort_by(|a, b| b.1.cmp(&a.1));

            info!("섹터 순위 (상승 종목 수 기준): {:?}", sorted_sectors);

            let is_leading_sector = sorted_sectors
                .iter()
                .take(3)
                .any(|(sector, _)| *sector == current_sector);
            info!(
                "주도 섹터 여부: {} (상위 3개 섹터: {:?})",
                is_leading_sector,
                sorted_sectors
                    .iter()
                    .take(3)
                    .map(|(s, _)| s)
                    .collect::<Vec<_>>()
            );

            if is_leading_sector {
                1
            } else {
                0
            }
        } else {
            info!("6개월 전고점 돌파 + 장대양봉 조건을 만족하지 않아 주도 섹터 계산 생략");
            0
        }
    };

    info!(
        "6개월 전고점 돌파 + 장대양봉 + 주도 섹터: {}",
        breaks_6month_high_with_leading_sector
    );

    // 분석 결과를 캐시에 저장
    let _analysis = format!(
        "=== 6개월 전고점 돌파 상세 분석 ===\n\
        기준 날짜: {} (YYYYMMDD: {})\n\
        6개월 전 날짜: {} (YYYYMMDD: {})\n\
        6개월 내 데이터 개수: {}\n\
        6개월 내 최고가: {:.2}\n\
        당일 데이터 존재: {}\n\
        당일 고가: {:.2}\n\
        전고점 돌파: {} (조건: {} >= {} && {} > 0.0)\n\
        장대양봉: {} (몸통 비율 > 0.7)\n\
        주도 섹터: {} (상위 3개 섹터 내 포함)\n\
        최종 결과: ({}, {}, {})",
        stock_info.date.format("%Y-%m-%d"),
        target_date_str,
        six_months_ago.format("%Y-%m-%d"),
        six_months_ago_str,
        six_month_data_count,
        six_month_high,
        today_data_exists,
        today_high,
        breaks_6month_high,
        today_high,
        six_month_high,
        six_month_high,
        is_long_candle,
        breaks_6month_high_with_leading_sector,
        breaks_6month_high,
        breaks_6month_high_with_long_candle,
        breaks_6month_high_with_leading_sector
    );

    Ok((
        breaks_6month_high,
        breaks_6month_high_with_long_candle,
        breaks_6month_high_with_leading_sector,
    ))
}

// NOTE: 5분봉 데이터 캐시 함수 (간단한 버전)

#[derive(Debug, Clone)]
struct Day3StockInfo {
    stock_code: String,
    date: NaiveDate,
}

// NOTE: 테스트 함수
#[cfg(test)]
mod tests {
    #[test]
    fn test_high_break_logic() {
        // 6개월 전고점 돌파 테스트
        let six_month_high = 50000.0;
        let today_high = 51000.0;
        let breaks_high = if today_high >= six_month_high && six_month_high > 0.0 {
            1
        } else {
            0
        };
        assert_eq!(breaks_high, 1);

        // 장대양봉 테스트
        let body_ratio = 0.8;
        let is_long_candle = if body_ratio > 0.7 { 1 } else { 0 };
        assert_eq!(is_long_candle, 1);

        // 복합 조건 테스트
        let combined = if breaks_high == 1 && is_long_candle == 1 {
            1
        } else {
            0
        };
        assert_eq!(combined, 1);
    }
}
