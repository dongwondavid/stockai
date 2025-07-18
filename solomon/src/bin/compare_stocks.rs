use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use log::{info, error};
use rusqlite::Connection;
use solomon::SectorManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 로깅 설정
    env_logger::init();

    // 명령행 인수 파싱
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        println!("사용법: {} <DB_경로> <CSV_경로>", args[0]);
        println!("예시: {} ./stock_data.db ./sector_utf8.csv", args[0]);
        return Ok(());
    }

    let db_path = &args[1];
    let csv_path = &args[2];

    info!("종목 비교 분석 시작");
    info!("DB 경로: {}", db_path);
    info!("CSV 경로: {}", csv_path);

    // DB 연결 확인
    if !Path::new(db_path).exists() {
        error!("DB 파일이 존재하지 않습니다: {}", db_path);
        return Err("DB 파일이 존재하지 않습니다".into());
    }

    // CSV 파일 존재 확인
    if !Path::new(csv_path).exists() {
        error!("CSV 파일이 존재하지 않습니다: {}", csv_path);
        return Err("CSV 파일이 존재하지 않습니다".into());
    }

    // DB 연결
    let db = Connection::open(db_path)?;
    info!("DB 연결 성공");

    // SectorManager 생성 및 CSV 로드
    let mut sector_manager = SectorManager::new();
    sector_manager.load_from_csv(csv_path)?;
    info!("CSV 파일 로드 완료");

    // DB에서 테이블 목록 가져오기
    let db_tables = sector_manager.get_db_table_list(&db)?;
    info!("DB에서 {}개의 종목 테이블을 찾았습니다", db_tables.len());

    // CSV에는 있지만 DB에는 없는 종목들 찾기
    let missing_stocks = sector_manager.find_missing_stocks(&db)?;
    info!("CSV에 있지만 DB에 없는 종목: {}개", missing_stocks.len());

    // DB에만 있고 CSV에는 없는 종목들 찾기
    let extra_stocks = sector_manager.find_extra_stocks(&db)?;
    info!("DB에만 있고 CSV에는 없는 종목: {}개", extra_stocks.len());

    // 결과를 파일로 저장
    save_missing_stocks(&missing_stocks)?;
    save_extra_stocks(&extra_stocks)?;
    save_comparison_summary(&db_tables, &sector_manager, &missing_stocks, &extra_stocks)?;

    info!("분석 완료! 결과 파일들이 생성되었습니다.");
    println!("=== 분석 결과 ===");
    println!("DB 종목 테이블 수: {}", db_tables.len());
    println!("CSV 종목 수: {}", sector_manager.get_csv_stock_count());
    println!("CSV에만 있는 종목: {}개", missing_stocks.len());
    println!("DB에만 있는 종목: {}개", extra_stocks.len());
    println!("=== 생성된 파일 ===");
    println!("- missing_stocks.txt: CSV에만 있는 종목들");
    println!("- extra_stocks.txt: DB에만 있는 종목들");
    println!("- comparison_summary.txt: 전체 비교 요약");

    Ok(())
}

fn save_missing_stocks(missing_stocks: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create("missing_stocks.txt")?;
    writeln!(file, "CSV에 있지만 DB에 없는 종목들 (총 {}개)", missing_stocks.len())?;
    writeln!(file, "=")?;
    
    for stock_code in missing_stocks {
        writeln!(file, "{}", stock_code)?;
    }
    
    info!("missing_stocks.txt 파일 생성 완료");
    Ok(())
}

fn save_extra_stocks(extra_stocks: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create("extra_stocks.txt")?;
    writeln!(file, "DB에만 있고 CSV에는 없는 종목들 (총 {}개)", extra_stocks.len())?;
    writeln!(file, "=")?;
    
    for stock_code in extra_stocks {
        writeln!(file, "{}", stock_code)?;
    }
    
    info!("extra_stocks.txt 파일 생성 완료");
    Ok(())
}

fn save_comparison_summary(
    db_tables: &[String], 
    sector_manager: &SectorManager,
    missing_stocks: &[String],
    extra_stocks: &[String]
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create("comparison_summary.txt")?;
    
    writeln!(file, "종목 비교 분석 요약")?;
    writeln!(file, "=")?;
    writeln!(file, "DB 종목 테이블 수: {}", db_tables.len())?;
    writeln!(file, "CSV 종목 수: {}", sector_manager.get_csv_stock_count())?;
    writeln!(file, "CSV에만 있는 종목: {}개", missing_stocks.len())?;
    writeln!(file, "DB에만 있는 종목: {}개", extra_stocks.len())?;
    writeln!(file)?;
    
    writeln!(file, "DB 테이블 목록 (처음 20개):")?;
    for (i, table) in db_tables.iter().take(20).enumerate() {
        writeln!(file, "{:2}. {}", i + 1, table)?;
    }
    if db_tables.len() > 20 {
        writeln!(file, "... (총 {}개)", db_tables.len())?;
    }
    writeln!(file)?;
    
    writeln!(file, "CSV 종목 목록 (처음 20개):")?;
    let csv_codes = sector_manager.get_csv_stock_codes_limit(20);
    for (i, code) in csv_codes.iter().enumerate() {
        writeln!(file, "{:2}. {}", i + 1, code)?;
    }
    if sector_manager.get_csv_stock_count() > 20 {
        writeln!(file, "... (총 {}개)", sector_manager.get_csv_stock_count())?;
    }
    
    info!("comparison_summary.txt 파일 생성 완료");
    Ok(())
} 