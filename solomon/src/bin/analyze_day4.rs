use chrono::NaiveDate;
use rusqlite::{Connection, Result};
use log::{info, warn, error, debug};
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
    rsi_cache: HashMap<String, f64>,
}

#[derive(Debug, Clone)]
struct Day4StockInfo {
    stock_code: String,
    date: NaiveDate,
}

fn main() {
    // 로거 초기화
    env_logger::init();
    info!("Day4 특징 분석 프로그램 시작");

    // 섹터 정보 로드
    let mut sector_manager = SectorManager::new();
    if let Err(e) = sector_manager.load_from_csv("sector_utf8.csv") {
        warn!("섹터 정보 로드 실패: {}. 기본값으로 진행합니다.", e);
    } else {
        debug!("섹터 정보 로드 완료");
    }

    // 데이터베이스 연결 풀 생성
    let db_pool = match create_db_pool() {
        Ok(pool) => pool,
        Err(e) => {
            error!("데이터베이스 연결 풀 생성 실패: {}", e);
            return;
        }
    };

    // 분석할 종목 선택 (예시: 최근 5개 종목)
    let stock_list = match get_sample_stocks(&db_pool.solomon_db) {
        Ok(list) => list,
        Err(e) => {
            error!("샘플 종목 조회 실패: {}", e);
            return;
        }
    };

    info!("총 {}개 종목에 대해 Day4 특징을 분석합니다.", stock_list.len());

    // 캐시 초기화
    let cache = Arc::new(Mutex::new(Cache::default()));

    // 각 종목별 상세 분석
    for (i, stock_info) in stock_list.iter().enumerate() {
        info!("=== 종목 {} 분석 ({}/{}) ===", stock_info.stock_code, i + 1, stock_list.len());
        
        match analyze_single_stock(&stock_info, &sector_manager, &db_pool, &cache) {
            Ok(_) => info!("종목 {} 분석 완료", stock_info.stock_code),
            Err(e) => error!("종목 {} 분석 실패: {}", stock_info.stock_code, e),
        }
        
        println!(); // 구분선
    }

    info!("Day4 특징 분석 프로그램 종료");
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

fn get_sample_stocks(db: &Connection) -> Result<Vec<Day4StockInfo>> {
    let mut stmt = db.prepare(
        "SELECT stock_code, date FROM answer ORDER BY date DESC LIMIT 5"
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

fn analyze_single_stock(
    stock_info: &Day4StockInfo,
    sector_manager: &SectorManager,
    db_pool: &DbPool,
    cache: &Arc<Mutex<Cache>>,
) -> Result<()> {
    
    info!("종목코드: {}, 날짜: {}", stock_info.stock_code, stock_info.date.format("%Y-%m-%d"));
    
    // 5분봉 데이터 가져오기
    let today_data = match sector_manager.get_five_min_data(&db_pool.stock_db, &stock_info.stock_code, stock_info.date) {
        Ok(data) => {
            info!("오늘 5분봉 데이터: {}개", data.len());
            data
        },
        Err(e) => {
            error!("오늘 5분봉 데이터 조회 실패: {}", e);
            return Err(e);
        }
    };
    
    if today_data.is_empty() {
        error!("오늘 5분봉 데이터가 없습니다.");
        return Ok(());
    }
    
    // 데이터 출력
    println!("=== 5분봉 데이터 (오늘) ===");
    for (i, data) in today_data.iter().enumerate() {
        println!("{}: O:{}, H:{}, L:{}, C:{}, V:{}", 
            i, data.open, data.high, data.low, data.close, data.volume);
    }
    
    // RSI 분석
    println!("\n=== RSI 분석 ===");
    analyze_rsi(&today_data, &stock_info.stock_code, cache)?;
    
    // MACD 분석
    println!("\n=== MACD 분석 ===");
    analyze_macd(&today_data, &stock_info.stock_code)?;
    
    // 기타 특징들 분석
    println!("\n=== 기타 특징 분석 ===");
    analyze_other_features(&today_data);
    
    Ok(())
}

fn analyze_rsi(data: &[solomon::FiveMinData], stock_code: &str, cache: &Arc<Mutex<Cache>>) -> Result<()> {
    let cache_key = format!("{}_rsi", stock_code);
    
    // 캐시 확인
    {
        let cache_guard = cache.lock().unwrap();
        if let Some(&rsi) = cache_guard.rsi_cache.get(&cache_key) {
            println!("캐시된 RSI: {:.2}", rsi);
            println!("RSI >= 50: {}", if rsi >= 50.0 { "예" } else { "아니오" });
            return Ok(());
        }
    }
    
    if data.len() < 6 {
        println!("데이터 부족: {}개 (최소 6개 필요)", data.len());
        return Ok(());
    }
    
    // 첫 번째 5분봉의 open을 포함하여 7개 데이터 포인트 생성
    let mut prices = Vec::new();
    if !data.is_empty() {
        prices.push(data[0].open as f64); // 첫 번째 5분봉의 open
        println!("첫 번째 5분봉 open: {}", data[0].open);
    }
    for d in data {
        prices.push(d.close as f64); // 각 5분봉의 close
    }
    
    println!("가격 데이터 (open + closes): {:?}", prices);
    
    if prices.len() < 7 {
        println!("가격 데이터 부족: {}개 (최소 7개 필요)", prices.len());
        return Ok(());
    }
    
    let mut gains = Vec::new();
    let mut losses = Vec::new();
    
    println!("가격 변화 계산:");
    for i in 1..prices.len() {
        let change = prices[i] - prices[i-1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
            println!("  {:.2} -> {:.2}: +{:.2} (gain)", prices[i-1], prices[i], change);
        } else {
            gains.push(0.0);
            losses.push(-change);
            println!("  {:.2} -> {:.2}: {:.2} (loss)", prices[i-1], prices[i], change);
        }
    }
    
    // 6기간 평균 계산
    let avg_gain = gains.iter().sum::<f64>() / gains.len() as f64;
    let avg_loss = losses.iter().sum::<f64>() / losses.len() as f64;
    
    println!("평균 gain: {:.4}", avg_gain);
    println!("평균 loss: {:.4}", avg_loss);
    
    let rsi = if avg_loss > 0.0 {
        100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
    } else {
        100.0
    };
    
    println!("계산된 RSI: {:.2}", rsi);
    println!("RSI >= 50: {}", if rsi >= 50.0 { "예" } else { "아니오" });
    
    // 캐시에 저장
    {
        let mut cache_guard = cache.lock().unwrap();
        cache_guard.rsi_cache.insert(cache_key, rsi);
    }
    
    Ok(())
}

fn analyze_macd(data: &[solomon::FiveMinData], _stock_code: &str) -> Result<()> {
    if data.len() < 5 {
        println!("데이터 부족: {}개 (최소 5개 필요)", data.len());
        return Ok(());
    }
    
    // 첫 번째 5분봉의 open을 포함하여 더 많은 데이터 포인트 생성
    let mut prices = Vec::new();
    if !data.is_empty() {
        prices.push(data[0].open as f64); // 첫 번째 5분봉의 open
        println!("첫 번째 5분봉 open: {}", data[0].open);
    }
    for d in data {
        prices.push(d.close as f64); // 각 5분봉의 close
    }
    
    println!("가격 데이터 (open + closes): {:?}", prices);
    
    if prices.len() < 6 {
        println!("가격 데이터 부족: {}개 (최소 6개 필요)", prices.len());
        return Ok(());
    }
    
    // 단축 MACD (3,6) 분석
    println!("\n--- 단축 MACD (3,6) 분석 ---");
    let ema3 = calculate_ema(&prices, 3);
    let ema6 = calculate_ema(&prices, 6);
    let macd = ema3 - ema6;
    let signal = calculate_ema(&vec![macd], 3);
    
    println!("EMA(3): {:.4}", ema3);
    println!("EMA(6): {:.4}", ema6);
    println!("MACD: {:.4}", macd);
    println!("Signal: {:.4}", signal);
    println!("MACD > Signal: {}", if macd > signal { "예" } else { "아니오" });
    
    // 이전 값과 비교하여 상향 돌파 확인
    if prices.len() >= 7 {
        let prev_prices: Vec<f64> = prices[..prices.len()-1].to_vec();
        let prev_ema3 = calculate_ema(&prev_prices, 3);
        let prev_ema6 = calculate_ema(&prev_prices, 6);
        let prev_macd = prev_ema3 - prev_ema6;
        let prev_signal = calculate_ema(&vec![prev_macd], 3);
        
        println!("이전 MACD: {:.4}", prev_macd);
        println!("이전 Signal: {:.4}", prev_signal);
        println!("이전 MACD > Signal: {}", if prev_macd > prev_signal { "예" } else { "아니오" });
        println!("상향 돌파: {}", if macd > signal && prev_macd <= prev_signal { "예" } else { "아니오" });
    }
    
    // 정규 MACD (12,26) 분석
    println!("\n--- 정규 MACD (12,26) 분석 ---");
    if prices.len() < 26 {
        println!("가격 데이터 부족: {}개 (최소 26개 필요)", prices.len());
        return Ok(());
    }
    
    let ema12 = calculate_ema(&prices, 12);
    let ema26 = calculate_ema(&prices, 26);
    let macd_regular = ema12 - ema26;
    let signal_regular = calculate_ema(&vec![macd_regular], 9);
    
    println!("EMA(12): {:.4}", ema12);
    println!("EMA(26): {:.4}", ema26);
    println!("MACD: {:.4}", macd_regular);
    println!("Signal: {:.4}", signal_regular);
    println!("MACD > Signal: {}", if macd_regular > signal_regular { "예" } else { "아니오" });
    
    // 이전 값과 비교하여 돌파 확인
    if prices.len() >= 27 {
        let prev_prices: Vec<f64> = prices[..prices.len()-1].to_vec();
        let prev_ema12 = calculate_ema(&prev_prices, 12);
        let prev_ema26 = calculate_ema(&prev_prices, 26);
        let prev_macd_regular = prev_ema12 - prev_ema26;
        let prev_signal_regular = calculate_ema(&vec![prev_macd_regular], 9);
        
        println!("이전 MACD: {:.4}", prev_macd_regular);
        println!("이전 Signal: {:.4}", prev_signal_regular);
        println!("돌파 발생: {}", 
            if (macd_regular > signal_regular && prev_macd_regular <= prev_signal_regular) ||
               (macd_regular < signal_regular && prev_macd_regular >= prev_signal_regular) 
            { "예" } else { "아니오" });
    }
    
    Ok(())
}

fn analyze_other_features(data: &[solomon::FiveMinData]) {
    if data.is_empty() {
        println!("데이터가 없습니다.");
        return;
    }
    
    // 시가 대비 현재가 수익률
    let open_price = data[0].open as f64;
    let current_price = data.last().unwrap().close as f64;
    let return_rate = if open_price > 0.0 {
        (current_price - open_price) / open_price
    } else {
        0.0
    };
    println!("시가 대비 수익률: {:.4} ({:.2}%)", return_rate, return_rate * 100.0);
    
    // 연속 양봉 개수
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
    println!("최대 연속 양봉 개수: {}", max_consecutive);
    
    // 장대양봉 여부
    let is_long_bull = if open_price > 0.0 && return_rate >= 0.01 { "예" } else { "아니오" };
    println!("장대양봉 (1% 이상): {}", is_long_bull);
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