use chrono::NaiveDate;
use rusqlite::{Connection, Result};
use log::{info, warn, error, debug};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() {    
    // 로거 초기화
    env_logger::init();
    
    info!("주식 분석 프로그램 시작");
    
    // extra_stocks.txt 파일에서 제외할 종목 목록 읽기
    let excluded_stocks = match load_excluded_stocks() {
        Ok(stocks) => {
            info!("제외할 종목 {}개 로드 완료", stocks.len());
            stocks
        }
        Err(e) => {
            warn!("extra_stocks.txt 파일을 읽을 수 없습니다: {}. 제외 목록 없이 진행합니다.", e);
            Vec::new()
        }
    };
    
    // 저장할 테이블 이름 설정
    let table_name: &'static str = "answer_v3"; // 고점상승률 ≥ 2% AND 저점하락률 ≤ 1% 조건
    
    let db = Connection::open("D:\\db\\stock_price(5min).db").unwrap();
    let date_list = get_date_list_from_db(&db).unwrap();
    info!("총 {}개 날짜에 대해 분석을 수행합니다.", date_list.len());
    
    // 결과 저장용 데이터베이스 연결
    let result_db = Connection::open("D:\\db\\solomon.db").unwrap();
    
    // 기존 테이블이 있다면 삭제하고 새로 생성
    let drop_query = format!("DROP TABLE IF EXISTS {}", table_name);
    result_db.execute(&drop_query, []).unwrap();
    create_answer_table(&result_db, table_name).unwrap();
    debug!("결과 저장 테이블 '{}' 준비 완료", table_name);
    
    // 진행 상황 표시를 위한 ProgressBar 설정
    let pb = ProgressBar::new(date_list.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars("#>-"));
    
    let mut total_matching_stocks = 0;
    let mut date_results = Vec::new();
    
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
        
        match get_top_volume_stocks(date, &excluded_stocks) {
            Ok(stock_infos) => {
                if !stock_infos.is_empty() {
                    // 조건에 맞는 종목 수 계산
                    let matching_count = stock_infos.iter().filter(|info| info.is_answer == 1).count();
                    total_matching_stocks += matching_count;
                    date_results.push((date, stock_infos.clone()));
                    
                    // 결과를 데이터베이스에 저장
                    if let Err(e) = save_stock_results(&result_db, &date, &stock_infos, table_name) {
                        error!("결과 저장 중 오류 발생: {}", e);
                    }
                    
                    debug!("{}: {}개 종목이 조건에 맞음 (전체 30개)", date.format("%Y-%m-%d"), matching_count);
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
    
    // 정답 비율 계산
    let total_analyzed_stocks = date_results.len() * 30; // 각 날짜마다 30개 종목
    let answer_ratio = if total_analyzed_stocks > 0 {
        (total_matching_stocks as f64 / total_analyzed_stocks as f64) * 100.0
    } else {
        0.0
    };
    
    info!("총 분석한 종목 수: {}", total_analyzed_stocks);
    info!("정답으로 선정된 종목 비율: {:.2}%", answer_ratio);
    info!("조건: 현재가에서 1% 이상 하락하기 전까지 고점이 2% 이상 상승한 적이 있는 경우");
    info!("결과가 D:\\db\\solomon.db의 {} 테이블에 저장되었습니다.", table_name);
    
    // 각 날짜별 상세 결과 출력
    for (date, stock_infos) in date_results {
        debug!("=== {} ===", date.format("%Y-%m-%d"));
        for info in stock_infos {
            debug!("순위: {}, 종목: {}, 총상승률: {:.2}%, 고점상승률: {:.2}%, 저점하락률: {:.2}%, MDD: {:.2}%, 정답: {}", 
                info.rank, info.stock_code, info.total_gain, info.high_gain, info.low_drop, info.mdd, info.is_answer);
        }
    }
    
    info!("주식 분석 프로그램 종료");
}

#[derive(Debug, Clone)]
struct StockInfo {
    stock_code: String,
    rank: i32,
    total_gain: f64,    // 9시반~12시 총상승률
    high_gain: f64,     // 9시반~고점 상승률
    low_drop: f64,      // 9시반~저점 하락률
    mdd: f64,           // 최고 고가 대비 최저 저가 (Maximum Drawdown)
    is_answer: i32,     // 정답 여부 (1: 정답, 0: 오답)
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

fn get_top_volume_stocks(date: NaiveDate, excluded_stocks: &[String]) -> Result<Vec<StockInfo>> {
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
    
    // 4. 상위 30개 중에서 제외 목록에 있는 종목은 건너뛰기
    let filtered_stocks: Vec<String> = top_30_stocks
        .into_iter()
        .filter(|stock_code| {
            if excluded_stocks.contains(stock_code) {
                debug!("종목 {} 제외됨 (extra_stocks.txt에 포함)", stock_code);
                false
            } else {
                true
            }
        })
        .collect();
    debug!("제외 목록 필터링 후 {}개 종목 남음", filtered_stocks.len());
    
    // 5. 필터링된 종목들에 대해 상승률과 MDD 계산
    debug!("필터링된 종목들의 상승률 및 MDD 계산 중...");
    let mut stock_infos: Vec<StockInfo> = Vec::new();
    
    for (rank, stock_code) in filtered_stocks.iter().enumerate() {
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
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
    )?;
    
    let tables = stmt.query_map([], |row| {
        Ok(row.get::<_, String>(0)?)
    })?
    .filter_map(|r| r.ok())
    .collect();
    
    Ok(tables)
}

fn calculate_total_volume(db: &Connection, table_name: &str, date: NaiveDate) -> Result<i64> {
    let date_start = date.format("%Y%m%d").to_string() + "0900"; // 9시 00분``
    let date_end = date.format("%Y%m%d").to_string() + "0930";   // 9시 30분
    
    let query = format!(
        "SELECT SUM(volume * close) as total_volume 
         FROM \"{}\" 
         WHERE date >= ? AND date <= ?",
        table_name
    );
    
    let mut stmt = db.prepare(&query)?;
    let total_volume: Option<i64> = stmt.query_row([&date_start, &date_end], |row| {
        Ok(row.get(0)?)
    }).ok();
    
    Ok(total_volume.unwrap_or(0))
}

fn get_stock_data_for_date(db: &Connection, table_name: &str, date: NaiveDate) -> Result<Vec<StockData>> {
    let date_start = date.format("%Y%m%d").to_string() + "0930"; // 9시 30분
    let date_end = date.format("%Y%m%d").to_string() + "1200";   // 12시 00분
    
    let query = format!(
        "SELECT date, open, high, low, close, volume 
         FROM \"{}\" 
         WHERE date >= ? AND date <= ? 
         ORDER BY date",
        table_name
    );
    
    let mut stmt = db.prepare(&query)?;
    let stock_data = stmt.query_map([&date_start, &date_end], |row| {
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

fn calculate_stock_metrics(stock_data: &[StockData], rank: i32, table_name: &str) -> Option<StockInfo> {
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
    
    // 새로운 조건: 현재가에서 1% 이상 하락하기 전까지 고점이 2% 이상 상승한 적이 있는 경우
    let is_answer = check_new_condition(stock_data, open_price);
    
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

/// 새로운 조건을 확인하는 함수
/// 현재가에서 1% 이상 하락하기 전까지 고점이 2% 이상 상승한 적이 있는 경우를 정답으로 간주
fn check_new_condition(stock_data: &[StockData], open_price: f64) -> i32 {
    if stock_data.is_empty() || open_price <= 0.0 {
        return 0;
    }
    
    let mut max_gain_before_drop = 0.0;  // 1% 하락 전까지의 최대 상승률
    let mut current_max_gain = 0.0;      // 현재까지의 최대 상승률
    
    for data in stock_data.iter() {
        let high_price = data.high as f64;
        let low_price = data.low as f64;
        
        // 현재 구간의 상승률 계산
        let high_gain = ((high_price - open_price) / open_price) * 100.0;
        let low_gain = ((low_price - open_price) / open_price) * 100.0;
        
        // 현재까지의 최대 상승률 업데이트
        if high_gain > current_max_gain {
            current_max_gain = high_gain;
        }
        
        // 1% 이상 하락했는지 확인
        if low_gain <= -1.0 {
            // 1% 이상 하락하기 전까지의 최대 상승률이 2% 이상이었는지 확인
            if max_gain_before_drop >= 2.0 {
                return 1;  // 정답
            }
            break;  // 1% 하락이 발생했으므로 더 이상 확인할 필요 없음
        }
        
        // 아직 1% 하락이 없었다면, 현재까지의 최대 상승률을 기록
        if current_max_gain > max_gain_before_drop {
            max_gain_before_drop = current_max_gain;
        }
    }
    
    // 마지막까지 1% 하락이 없었다면, 전체 기간 동안 2% 이상 상승했는지 확인
    if current_max_gain >= 2.0 {
        return 1;  // 정답
    }
    
    0  // 오답
}

/// 삼성전자 테이블(A005930)에서 20230601 이전 날짜 목록을 불러오는 함수
fn get_date_list_from_db(db: &Connection) -> Result<Vec<i64>> {
    let mut stmt = db.prepare(
        "SELECT DISTINCT date/10000 as ymd FROM \"A005930\" WHERE date/10000 < 20230601 ORDER BY ymd"
    )?;
    let dates = stmt.query_map([], |row| {
        Ok(row.get::<_, i64>(0)?)
    })?
    .filter_map(|r| r.ok())
    .collect();
    Ok(dates)
}

fn create_answer_table(db: &Connection, table_name: &str) -> Result<()> {
    let create_query = format!(
        "CREATE TABLE IF NOT EXISTS {} (
            date INTEGER,
            stock_code TEXT,
            rank INTEGER,
            total_gain REAL,
            high_gain REAL,
            low_drop REAL,
            mdd REAL,
            is_answer INTEGER,
            PRIMARY KEY (date, stock_code)
        )",
        table_name
    );
    db.execute(&create_query, [])?;
    Ok(())
}

fn save_stock_results(db: &Connection, date: &NaiveDate, stock_infos: &[StockInfo], table_name: &str) -> Result<()> {
    let insert_query = format!(
        "INSERT INTO {} (date, stock_code, rank, total_gain, high_gain, low_drop, mdd, is_answer) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        table_name
    );
    let mut stmt = db.prepare(&insert_query)?;
    
    let date_int = date.format("%Y%m%d").to_string().parse::<i64>().unwrap_or(0);
    
    for info in stock_infos {
        stmt.execute(rusqlite::params![
            date_int,
            info.stock_code,
            info.rank,
            info.total_gain,
            info.high_gain,
            info.low_drop,
            info.mdd,
            info.is_answer,
        ])?;
    }
    Ok(())
}

/// extra_stocks.txt 파일에서 제외할 종목 목록을 읽어오는 함수
fn load_excluded_stocks() -> std::io::Result<Vec<String>> {
    let file = File::open("extra_stocks.txt")?;
    let reader = BufReader::new(file);
    let mut excluded_stocks = Vec::new();
    
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("DB에만") && !trimmed.starts_with("=") {
            // "A" 접두사를 추가하여 테이블 이름 형식으로 변환
            excluded_stocks.push(format!("A{}", trimmed));
        }
    }
    
    Ok(excluded_stocks)
}