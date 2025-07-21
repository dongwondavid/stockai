use chrono::NaiveDate;
use rusqlite::{Connection, Result};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{error, info, warn};

// NOTE: day3.rs의 calculate_foreign_ratio_feature_thread_optimized 함수 분석용
// 기존 로직을 그대로 가져와서 결과를 분석할 수 있도록 함

#[derive(Default)]
struct Cache {
    foreign_ratio_cache: HashMap<String, Vec<f64>>,
}

fn main() {
    // 로거 초기화
    solomon::init_tracing();
    info!("외국인 현보유비율 특징 계산 분석 프로그램 시작");

    let start_time = Instant::now();

    // 테스트용 종목 정보 (실제로는 DB에서 가져와야 함)
    let test_stocks = vec![
        Day3StockInfo {
            stock_code: "005930".to_string(),                     // 삼성전자
            date: NaiveDate::from_ymd_opt(2024, 12, 20).unwrap(), // 더 최근 날짜로 변경
        },
        Day3StockInfo {
            stock_code: "000660".to_string(),                     // SK하이닉스
            date: NaiveDate::from_ymd_opt(2024, 12, 20).unwrap(), // 더 최근 날짜로 변경
        },
        Day3StockInfo {
            stock_code: "035420".to_string(),                     // NAVER
            date: NaiveDate::from_ymd_opt(2024, 12, 20).unwrap(), // 더 최근 날짜로 변경
        },
    ];

    // DB 연결
    let daily_db_path = std::env::var("DAILY_DB_PATH")
        .unwrap_or_else(|_| "D:\\db\\stock_price(1day)_with_data.db".to_string());
    info!("DB 경로: {}", daily_db_path);

    let mut daily_db = match Connection::open(&daily_db_path) {
        Ok(db) => {
            info!("DB 연결 성공");
            // 성능 최적화 설정
            let _: String = db
                .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
                .unwrap_or_default();
            db.execute("PRAGMA synchronous = NORMAL", [])
                .unwrap_or_default();
            db.execute("PRAGMA cache_size = 10000", [])
                .unwrap_or_default();
            db.execute("PRAGMA temp_store = MEMORY", [])
                .unwrap_or_default();
            db
        }
        Err(e) => {
            error!("DB 연결 실패: {}", e);
            return;
        }
    };

    // DB 테이블 확인
    info!("DB 테이블 목록 확인 중...");
    let tables: Vec<String> = daily_db
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    info!("발견된 테이블: {:?}", tables);

    let mut cache = Cache::default();

    // 각 종목에 대해 기존 로직으로 외국인 현보유비율 특징 계산
    for stock_info in test_stocks {
        info!("=== {} 종목 분석 ===", stock_info.stock_code);

        // 테이블명에 A 접두사 추가
        let table_name = format!("A{}", stock_info.stock_code);
        info!("실제 테이블명: {}", table_name);

        // 테이블 존재 여부 확인
        let table_exists: bool = daily_db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
                rusqlite::params![table_name],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !table_exists {
            error!("테이블 '{}'이 존재하지 않습니다.", table_name);
            continue;
        }

        // 테이블 스키마 확인
        let schema: String = daily_db
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name = ?",
                rusqlite::params![table_name],
                |row| row.get(0),
            )
            .unwrap_or_default();

        info!("테이블 스키마: {}", schema);

        // 컬럼 존재 여부 확인
        let columns: Vec<String> = daily_db
            .prepare(&format!("PRAGMA table_info({})", table_name))
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        info!("컬럼 목록: {:?}", columns);

        // 외국인현보유비율 컬럼이 있는지 확인
        if !columns.iter().any(|col| col == "외국인현보유비율") {
            error!("'외국인현보유비율' 컬럼이 테이블에 존재하지 않습니다.");
            continue;
        }

        // 최근 데이터 확인
        let recent_data: Vec<(String, f64)> = daily_db
            .prepare(&format!(
                "SELECT date, 외국인현보유비율 FROM {} ORDER BY date DESC LIMIT 5",
                table_name
            ))
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        info!("최근 5일 데이터: {:?}", recent_data);

        // 전체 데이터 범위 확인
        let date_range: Vec<(String, String)> = daily_db
            .prepare(&format!("SELECT MIN(date), MAX(date) FROM {}", table_name))
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        if let Some((min_date, max_date)) = date_range.first() {
            info!("데이터 범위: {} ~ {}", min_date, max_date);
        }

        // 전체 데이터 개수 확인
        let total_count: i64 = daily_db
            .query_row(&format!("SELECT COUNT(*) FROM {}", table_name), [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        info!("전체 데이터 개수: {}", total_count);

        // 수정된 Day3StockInfo로 함수 호출
        let modified_stock_info = Day3StockInfo {
            stock_code: table_name.clone(),
            date: stock_info.date,
        };

        match calculate_foreign_ratio_feature_thread_optimized(
            &modified_stock_info,
            &mut daily_db,
            &mut cache,
        ) {
            Ok(result) => {
                info!("결과: {}", result);

                // 상세 분석을 위해 캐시된 데이터 확인
                let cache_key = format!("{}", table_name);
                if let Some(ratios) = cache.foreign_ratio_cache.get(&cache_key) {
                    info!("외국인 현보유비율 데이터 (캐시): {:?}", ratios);

                    if ratios.is_empty() {
                        info!("⚠️ 캐시된 데이터가 비어있습니다.");
                    } else {
                        info!("외국인 현보유비율 데이터:");
                        for (i, &ratio) in ratios.iter().enumerate() {
                            let date = stock_info.date - chrono::Duration::days((i + 1) as i64);
                            info!("  {}: {:.2}%", date.format("%Y-%m-%d"), ratio);
                        }

                        // 패턴 분석
                        if ratios.len() == 3 {
                            let day1 = ratios[0];
                            let day2 = ratios[1];
                            let day3 = ratios[2];

                            info!("패턴 분석:");
                            info!("  최근일: {:.2}%", day1);
                            info!("  2일전: {:.2}%", day2);
                            info!("  3일전: {:.2}%", day3);
                            info!(
                                "  연속 상승 조건: {} > {} && {} > {}",
                                day1, day2, day2, day3
                            );
                            info!("  조건 만족: {}", day1 > day2 && day2 > day3);

                            if day1 > day2 && day2 > day3 {
                                info!("  ✓ 3일 연속 상승 패턴 감지");
                            } else {
                                info!("  ✗ 3일 연속 상승 패턴 아님");
                                if day1 <= day2 {
                                    info!("    - 최근일({:.2}%) <= 2일전({:.2}%)", day1, day2);
                                }
                                if day2 <= day3 {
                                    info!("    - 2일전({:.2}%) <= 3일전({:.2}%)", day2, day3);
                                }
                            }
                        } else {
                            info!("⚠️ 데이터가 3개가 아닙니다: {}개", ratios.len());
                        }
                    }
                } else {
                    info!("⚠️ 캐시에 데이터가 없습니다.");
                }
            }
            Err(e) => {
                error!("{} 종목 처리 중 오류: {}", table_name, e);
            }
        }

        info!("");
    }

    let elapsed = start_time.elapsed();
    info!("외국인 현보유비율 특징 계산 분석 완료: {:.2?}", elapsed);
}

// NOTE: day3.rs의 원본 calculate_foreign_ratio_feature_thread_optimized 함수 (그대로 복사)
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

    // 최근 3개 거래일 조회 (주식 시장 열리는 날짜 기준)
    let mut ratios = Vec::new();

    // 날짜를 INTEGER 형식으로 변환 (YYYYMMDD)
    let target_date_int = stock_info
        .date
        .format("%Y%m%d")
        .to_string()
        .parse::<i32>()
        .unwrap_or(0);

    // 해당 종목의 최근 거래일들을 조회 (최대 10일 전까지 확인)
    let query = format!(
        "SELECT date FROM {} WHERE date < ? ORDER BY date DESC LIMIT 10",
        stock_info.stock_code
    );

    info!("거래일 조회 쿼리: {}", query);
    info!("기준 날짜 (INTEGER): {}", target_date_int);

    let trading_dates: Vec<i32> = match daily_db.prepare(&query) {
        Ok(mut stmt) => {
            match stmt.query_map(rusqlite::params![target_date_int], |row| row.get(0)) {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
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

    info!("발견된 거래일들: {:?}", trading_dates);

    // 최근 3개 거래일 선택
    for i in 0..3 {
        if i < trading_dates.len() {
            let trading_date_int = trading_dates[i];
            info!(
                "조회 중: {} 테이블의 {} 날짜 (거래일)",
                stock_info.stock_code, trading_date_int
            );

            match daily_db.query_row(
                &format!(
                    "SELECT 외국인현보유비율 FROM {} WHERE date = ?",
                    stock_info.stock_code
                ),
                rusqlite::params![trading_date_int],
                |row| row.get::<_, f64>(0),
            ) {
                Ok(ratio) => {
                    info!("  {}: {:.2}%", trading_date_int, ratio);
                    ratios.push(ratio);
                }
                Err(e) => {
                    warn!("  {}: 조회 실패 - {}", trading_date_int, e);
                    // 데이터가 없으면 0으로 채움
                    ratios.push(0.0);
                }
            }
        } else {
            warn!(
                "거래일이 부족합니다. 필요한 거래일: 3개, 실제 거래일: {}개",
                trading_dates.len()
            );
            ratios.push(0.0);
        }
    }

    info!("수집된 데이터: {:?}", ratios);

    // 캐시에 저장
    cache.foreign_ratio_cache.insert(cache_key, ratios.clone());

    if ratios.len() == 3 && ratios[0] > ratios[1] && ratios[1] > ratios[2] {
        Ok(1)
    } else {
        Ok(0)
    }
}

#[allow(dead_code)]
// NOTE: 추가 분석 함수 - 기존 로직의 문제점 파악용
fn analyze_foreign_ratio_logic_issues(ratios: &[f64]) -> Vec<String> {
    let mut issues = Vec::new();

    if ratios.len() != 3 {
        issues.push(format!(
            "데이터 부족: {}일 데이터 (필요: 3일)",
            ratios.len()
        ));
        return issues;
    }

    let day1 = ratios[0];
    let day2 = ratios[1];
    let day3 = ratios[2];

    // 1. 데이터 누락 확인
    if day1 == 0.0 || day2 == 0.0 || day3 == 0.0 {
        issues.push("외국인 현보유비율 데이터 누락 (0.0)".to_string());
    }

    // 2. 비정상적인 값 확인
    if day1 > 100.0 || day2 > 100.0 || day3 > 100.0 {
        issues.push("비정상적으로 높은 외국인 현보유비율 (>100%)".to_string());
    }

    // 3. 급격한 변화 확인
    let change1 = (day1 - day2).abs();
    let change2 = (day2 - day3).abs();
    if change1 > 10.0 || change2 > 10.0 {
        issues.push(format!(
            "급격한 외국인 현보유비율 변화 (1일 변화: {:.2}%, 2일 변화: {:.2}%)",
            change1, change2
        ));
    }

    // 4. 패턴 분석
    if day1 > day2 && day2 > day3 {
        issues.push("✓ 정상적인 3일 연속 상승 패턴".to_string());
    } else {
        if day1 <= day2 {
            issues.push(format!(
                "✗ 최근일({:.2}%) <= 2일전({:.2}%) - 상승 중단",
                day1, day2
            ));
        }
        if day2 <= day3 {
            issues.push(format!(
                "✗ 2일전({:.2}%) <= 3일전({:.2}%) - 연속 상승 아님",
                day2, day3
            ));
        }
    }

    issues
}

#[derive(Debug, Clone)]
struct Day3StockInfo {
    stock_code: String,
    date: NaiveDate,
}

// NOTE: 테스트 함수
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_original_logic() {
        let mut cache = Cache::default();

        // 정상적인 3일 연속 상승
        let ratios = vec![10.5, 10.0, 9.5];
        cache.foreign_ratio_cache.insert("TEST".to_string(), ratios);

        // 이 경우 원본 로직은 1을 반환해야 함
        let cache_key = "TEST".to_string();
        let cache_guard = cache.foreign_ratio_cache.get(&cache_key);
        if let Some(ratios) = cache_guard {
            let result = if ratios.len() == 3 && ratios[0] > ratios[1] && ratios[1] > ratios[2] {
                1
            } else {
                0
            };
            assert_eq!(result, 1);
        }
    }

    #[test]
    fn test_issue_analysis() {
        // 데이터 누락 케이스
        let ratios = vec![10.5, 0.0, 9.5];
        let issues = analyze_foreign_ratio_logic_issues(&ratios);
        assert!(issues.iter().any(|issue| issue.contains("데이터 누락")));

        // 정상 케이스
        let ratios = vec![10.5, 10.0, 9.5];
        let issues = analyze_foreign_ratio_logic_issues(&ratios);
        assert!(issues
            .iter()
            .any(|issue| issue.contains("정상적인 3일 연속 상승 패턴")));
    }
}
