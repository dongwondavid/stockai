use chrono::NaiveDate;
use indicatif::{ProgressBar, ProgressStyle};
use rusqlite::{Connection, Result};
use tracing::{debug, error, info, warn};

fn main() {
    // 로거 초기화
    solomon::init_tracing();

    info!("주식 조건 분석 프로그램 시작");

    let db = Connection::open("D:\\db\\stock_price(5min).db").unwrap();
    let date_list = get_date_list_from_db(&db).unwrap();
    info!("총 {}개 날짜에 대해 분석을 수행합니다.", date_list.len());

    // 진행 상황 표시를 위한 ProgressBar 설정
    let pb = ProgressBar::new(date_list.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut total_matching_stocks = 0;
    let mut date_results = Vec::new();
    let mut total_analyzed_stocks = 0;

    for (_index, &date_num) in date_list.iter().enumerate() {
        // 날짜 형식 변환 (YYYYMMDD -> NaiveDate)
        let year = (date_num / 10000) as i32;
        let month = ((date_num % 10000) / 100) as u32;
        let day = (date_num % 100) as u32;

        let date = match NaiveDate::from_ymd_opt(year, month, day) {
            Some(d) => d,
            None => {
                warn!("잘못된 날짜 형식: {}", date_num);
                pb.inc(1);
                continue;
            }
        };

        pb.set_message(format!("분석 중: {}", date.format("%Y-%m-%d")));

        match get_top_volume_stocks(date) {
            Ok(stock_infos) => {
                if !stock_infos.is_empty() {
                    // 조건에 맞는 종목 수 계산
                    let matching_count = stock_infos
                        .iter()
                        .filter(|info| info.is_answer == 1)
                        .count();
                    total_matching_stocks += matching_count;
                    total_analyzed_stocks += stock_infos.len();
                    date_results.push((date, stock_infos.clone()));

                    debug!(
                        "{}: {}개 종목이 조건에 맞음 (전체 {}개)",
                        date.format("%Y-%m-%d"),
                        matching_count,
                        stock_infos.len()
                    );
                }
            }
            Err(e) => {
                error!("{} 날짜 처리 중 오류 발생: {}", date.format("%Y-%m-%d"), e);
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message("분석 완료!");

    // 전체 결과 요약
    info!("=== 전체 분석 결과 ===");
    info!("분석한 총 날짜 수: {}", date_list.len());
    info!("조건에 맞는 종목이 있는 날짜 수: {}", date_results.len());
    info!("총 조건에 맞는 종목 수: {}", total_matching_stocks);
    info!("총 분석한 종목 수: {}", total_analyzed_stocks);

    // 정답 비율 계산
    let answer_ratio = if total_analyzed_stocks > 0 {
        (total_matching_stocks as f64 / total_analyzed_stocks as f64) * 100.0
    } else {
        0.0
    };

    info!("정답으로 선정된 종목 비율: {:.2}%", answer_ratio);
    info!("조건: 고점상승률 ≥ 2% AND 저점하락률 ≤ 1%");

    // 각 날짜별 상세 결과 출력
    for (date, stock_infos) in date_results {
        debug!("=== {} ===", date.format("%Y-%m-%d"));
        for info in stock_infos {
            debug!("순위: {}, 종목: {}, 총상승률: {:.2}%, 고점상승률: {:.2}%, 저점하락률: {:.2}%, MDD: {:.2}%, 정답: {}", 
                info.rank, info.stock_code, info.total_gain, info.high_gain, info.low_drop, info.mdd, info.is_answer);
        }
    }

    info!("주식 조건 분석 프로그램 종료");
}

#[derive(Debug, Clone)]
struct StockInfo {
    stock_code: String,
    rank: i32,
    total_gain: f64, // 9시반~12시 총상승률
    high_gain: f64,  // 9시반~고점 상승률
    low_drop: f64,   // 9시반~저점 하락률
    mdd: f64,        // 최고 고가 대비 최저 저가 (Maximum Drawdown)
    is_answer: i32,  // 정답 여부 (1: 정답, 0: 오답)
}

#[derive(Debug)]
struct StockData {
    #[allow(dead_code)]
    date: i64,
    open: i32,
    high: i32,
    low: i32,
    close: i32,
    #[allow(dead_code)]
    volume: i32,
}

fn get_top_volume_stocks(date: NaiveDate) -> Result<Vec<StockInfo>> {
    debug!("데이터베이스 연결 시도");
    let db = Connection::open("D:\\db\\stock_price(5min).db")?;
    debug!("데이터베이스 연결 성공");

    // 1. 모든 테이블(종목) 목록 가져오기
    debug!("모든 종목 테이블 목록 조회 중...");
    let tables = get_all_stock_tables(&db)?;
    debug!("총 {}개 종목 테이블 발견", tables.len());

    // 2. 각 종목의 거래대금 계산 (9시반~12시)
    debug!("각 종목의 거래대금 계산 중...");
    let mut stock_volumes: Vec<(String, i64)> = Vec::new();

    for table_name in &tables {
        if let Ok(volume) = calculate_total_volume(&db, table_name, date) {
            if volume > 0 {
                stock_volumes.push((table_name.clone(), volume));
            }
        }
    }
    debug!("거래대금이 있는 종목 {}개 발견", stock_volumes.len());

    // 3. 거래대금 기준으로 정렬하고 상위 30개 선택
    debug!("거래대금 기준으로 정렬 중...");
    stock_volumes.sort_by(|a, b| b.1.cmp(&a.1));
    let top_30_stocks: Vec<String> = stock_volumes
        .into_iter()
        .take(30)
        .map(|(code, _)| code)
        .collect();
    debug!("거래대금 상위 30개 종목 선정 완료");

    // 4. 각 상위 30개 종목에 대해 상승률과 MDD 계산
    debug!("상위 30개 종목의 상승률 및 MDD 계산 중...");
    let mut stock_infos: Vec<StockInfo> = Vec::new();

    for (rank, stock_code) in top_30_stocks.iter().enumerate() {
        debug!("종목 {} 처리 중 ({}/30)", stock_code, rank + 1);
        if let Ok(stock_data) = get_stock_data_for_date(&db, stock_code, date) {
            if let Some(info) = calculate_stock_metrics(&stock_data, rank as i32 + 1, stock_code) {
                stock_infos.push(info);
            }
        }
    }
    debug!("상위 30개 종목 분석 완료");

    // 5. 모든 30개 종목 반환 (필터링하지 않음)
    debug!("전체 30개 종목 분석 완료");
    Ok(stock_infos)
}

fn get_all_stock_tables(db: &Connection) -> Result<Vec<String>> {
    let mut stmt = db.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )?;

    let tables = stmt
        .query_map([], |row| Ok(row.get::<_, String>(0)?))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(tables)
}

fn calculate_total_volume(db: &Connection, table_name: &str, date: NaiveDate) -> Result<i64> {
    let date_start = date.format("%Y%m%d").to_string() + "0900"; // 9시 00분
    let date_end = date.format("%Y%m%d").to_string() + "0930"; // 9시 30분

    let query = format!(
        "SELECT SUM(volume * close) as total_volume 
         FROM \"{}\" 
         WHERE date >= ? AND date <= ?",
        table_name
    );

    let mut stmt = db.prepare(&query)?;
    let total_volume: Option<i64> = stmt
        .query_row([&date_start, &date_end], |row| Ok(row.get(0)?))
        .ok();

    Ok(total_volume.unwrap_or(0))
}

fn get_stock_data_for_date(
    db: &Connection,
    table_name: &str,
    date: NaiveDate,
) -> Result<Vec<StockData>> {
    let date_start = date.format("%Y%m%d").to_string() + "0930"; // 9시 30분
    let date_end = date.format("%Y%m%d").to_string() + "1200"; // 12시 00분

    let query = format!(
        "SELECT date, open, high, low, close, volume 
         FROM \"{}\" 
         WHERE date >= ? AND date <= ? 
         ORDER BY date",
        table_name
    );

    let mut stmt = db.prepare(&query)?;
    let stock_data = stmt
        .query_map([&date_start, &date_end], |row| {
            Ok(StockData {
                date: row.get(0)?,
                open: row.get(1)?,
                high: row.get(2)?,
                low: row.get(3)?,
                close: row.get(4)?,
                volume: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(stock_data)
}

fn calculate_stock_metrics(
    stock_data: &[StockData],
    rank: i32,
    table_name: &str,
) -> Option<StockInfo> {
    if stock_data.is_empty() {
        return None;
    }

    // 9시 30분 시가 (첫 번째 데이터의 시가)
    let open_price = stock_data[0].open as f64;

    // 12시 종가 (마지막 데이터의 종가)
    let close_price = stock_data.last().unwrap().close as f64;

    // 총상승률 계산 (9시반~12시)
    let total_gain = if open_price > 0.0 {
        ((close_price - open_price) / open_price) * 100.0
    } else {
        0.0
    };

    // 고점 찾기 (9시반~12시)
    let mut max_high = stock_data[0].high as f64;
    // 저점도
    let mut min_low = stock_data[0].low as f64;

    for data in stock_data.iter() {
        if (data.high as f64) > max_high {
            max_high = data.high as f64;
        }
        if (data.low as f64) < min_low {
            min_low = data.low as f64;
        }
    }

    // 고점상승률 계산 (9시반~고점)
    let high_gain = if open_price > 0.0 {
        ((max_high - open_price) / open_price) * 100.0
    } else {
        0.0
    };

    // 저점하락률 계산 (9시반~저점)
    let low_drop = if open_price > 0.0 {
        ((open_price - min_low) / open_price) * 100.0
    } else {
        0.0
    };

    // MDD (Maximum Drawdown) 계산
    let mdd = (max_high - min_low) / max_high * 100.0;

    // 종목 코드는 테이블 이름 그대로 사용 (예: "A000020")
    let stock_code = table_name.to_string();

    // 정답 여부 판단 (고점상승률 ≥ 2% AND 저점하락률 ≤ 1%)
    let is_answer = if high_gain >= 2.0 && low_drop <= 1.0 {
        1
    } else {
        0
    };

    Some(StockInfo {
        stock_code,
        rank,
        total_gain,
        high_gain,
        low_drop,
        mdd,
        is_answer,
    })
}

/// 삼성전자 테이블(A005930)에서 20230601 이전 날짜 목록을 불러오는 함수
fn get_date_list_from_db(db: &Connection) -> Result<Vec<i64>> {
    let mut stmt = db.prepare(
        "SELECT DISTINCT date/10000 as ymd FROM \"A005930\" WHERE date/10000 < 20230601 ORDER BY ymd"
    )?;
    let dates = stmt
        .query_map([], |row| Ok(row.get::<_, i64>(0)?))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(dates)
}
