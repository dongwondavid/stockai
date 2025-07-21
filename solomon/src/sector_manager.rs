use chrono::NaiveDate;
use rusqlite::{Connection, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct StockInfo {
    pub code: String,
    pub name: String,
    pub sector: String,
}

// 날짜별 섹터 정보 캐시
#[derive(Debug, Clone)]
pub struct DateSectorCache {
    // 종목코드 -> (gain_ratio, sector) 매핑
    pub stock_info: HashMap<String, (f64, String)>,
    // 15위 내 섹터별 상승 종목 수
    pub sector_rising_count_15: HashMap<String, i32>,
    // 30위 내 섹터별 상승 종목 수
    pub sector_rising_count_30: HashMap<String, i32>,
    // 섹터별 종목들을 상승률 순으로 정렬한 리스트
    pub sector_ranked_stocks: HashMap<String, Vec<String>>,
}

// 5분봉 데이터 구조
#[derive(Debug, Clone)]
pub struct FiveMinData {
    pub date: i64,
    pub open: i32,
    pub high: i32,
    pub low: i32,
    pub close: i32,
    pub volume: i32,
}

pub struct SectorManager {
    stock_map: HashMap<String, StockInfo>,
}

impl SectorManager {
    pub fn new() -> Self {
        debug!("SectorManager 새로 생성됨");
        Self {
            stock_map: HashMap::new(),
        }
    }

    pub fn load_from_csv(&mut self, csv_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new(csv_path);
        if !path.exists() {
            warn!("CSV 파일이 존재하지 않습니다: {}", csv_path);
            return Err(format!("CSV 파일이 존재하지 않습니다: {}", csv_path).into());
        }

        info!("섹터 정보 CSV 파일 로드 시작: {}", csv_path);
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut line_count = 0;
        let mut loaded_count = 0;

        for line in reader.lines() {
            let line = line?;
            line_count += 1;

            // 첫 번째 줄은 헤더로 건너뛰기
            if line_count == 1 {
                debug!("헤더 라인 건너뛰기: {}", line);
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 3 {
                let code = parts[0].trim().to_string();
                let name = parts[1].trim().to_string();
                let sector = parts[2].trim().to_string();

                debug!(
                    "종목 정보 로드: 코드={}, 이름={}, 섹터={}",
                    code, name, sector
                );

                let stock_info = StockInfo {
                    code: code.clone(),
                    name,
                    sector,
                };

                self.stock_map.insert(code, stock_info);
                loaded_count += 1;
            } else {
                warn!(
                    "잘못된 CSV 라인 {}: {} (파트 수: {})",
                    line_count,
                    line,
                    parts.len()
                );
            }
        }

        info!("섹터 정보 로드 완료: {}개 종목", loaded_count);
        debug!(
            "로드된 종목 코드들: {:?}",
            self.stock_map.keys().take(10).collect::<Vec<_>>()
        );
        Ok(())
    }

    pub fn get_stock_info(&self, code: &str) -> StockInfo {
        // 종목코드에서 'A' 접두사 제거 (DB 테이블명에서 종목코드 추출)
        let clean_code = code.trim_start_matches('A');

        debug!(
            "종목 정보 조회: 원본코드={}, 정리된코드={}",
            code, clean_code
        );
        debug!("현재 stock_map 크기: {}", self.stock_map.len());

        if let Some(stock_info) = self.stock_map.get(clean_code) {
            debug!("종목 정보 찾음: {:?}", stock_info);
            stock_info.clone()
        } else {
            debug!(
                "섹터 정보를 찾을 수 없는 종목코드: {} (기타로 설정)",
                clean_code
            );
            debug!(
                "사용 가능한 종목 코드들 (처음 10개): {:?}",
                self.stock_map.keys().take(10).collect::<Vec<_>>()
            );
            StockInfo {
                code: clean_code.to_string(),
                name: "종목명 모름".to_string(),
                sector: "기타".to_string(),
            }
        }
    }

    pub fn get_sector(&self, code: &str) -> String {
        let sector = self.get_stock_info(code).sector;
        debug!("섹터 조회: 종목코드={}, 섹터={}", code, sector);
        sector
    }

    pub fn count_stocks_by_sector(&self, stock_codes: &[String]) -> HashMap<String, usize> {
        debug!("섹터별 종목 수 계산 시작: {}개 종목", stock_codes.len());
        let mut sector_count: HashMap<String, usize> = HashMap::new();

        for code in stock_codes {
            let sector = self.get_sector(code);
            *sector_count.entry(sector).or_insert(0) += 1;
        }

        debug!("섹터별 종목 수 결과: {:?}", sector_count);
        sector_count
    }

    pub fn find_sectors_with_3_or_more(&self, stock_codes: &[String]) -> Vec<String> {
        let sector_count = self.count_stocks_by_sector(stock_codes);

        let result = sector_count
            .into_iter()
            .filter(|(_, count)| *count >= 3)
            .map(|(sector, _)| sector)
            .collect();

        debug!("3개 이상 종목이 있는 섹터들: {:?}", result);
        result
    }

    pub fn get_stocks_in_sector(&self, sector: &str) -> Vec<String> {
        let result = self
            .stock_map
            .values()
            .filter(|stock| stock.sector == sector)
            .map(|stock| stock.code.clone())
            .collect();

        debug!("섹터 '{}'의 종목들: {:?}", sector, result);
        result
    }

    pub fn get_sector_rank(&self, target_code: &str, stock_codes: &[String]) -> Option<usize> {
        let target_sector = self.get_sector(target_code);
        debug!(
            "섹터 순위 계산: 타겟종목={}, 타겟섹터={}",
            target_code, target_sector
        );

        // 같은 섹터의 종목들만 필터링
        let same_sector_stocks: Vec<String> = stock_codes
            .iter()
            .filter(|code| self.get_sector(code) == target_sector)
            .cloned()
            .collect();

        debug!("같은 섹터 종목들: {:?}", same_sector_stocks);

        if same_sector_stocks.is_empty() {
            debug!("같은 섹터 종목이 없음");
            return None;
        }

        // 종목코드 순으로 정렬 (간단한 순위 계산)
        let mut sorted_stocks = same_sector_stocks.clone();
        sorted_stocks.sort();

        // 타겟 종목의 순위 찾기
        let rank = sorted_stocks
            .iter()
            .position(|code| code == target_code)
            .map(|pos| pos + 1);

        debug!("섹터 순위 결과: {:?}", rank);
        rank
    }

    // 새로운 기능들: 날짜별 섹터 캐시 관련

    /// 특정 날짜의 섹터 정보를 계산하여 캐시로 반환
    pub fn calculate_date_sector_cache(
        &self,
        date: &NaiveDate,
        db: &Connection,
    ) -> Result<DateSectorCache> {
        let date_int = date
            .format("%Y%m%d")
            .to_string()
            .parse::<i64>()
            .unwrap_or(0);
        debug!("날짜 {}의 섹터 정보 계산 중...", date.format("%Y-%m-%d"));

        // 해당 날짜의 모든 종목 정보 가져오기 (거래대금 순으로 정렬됨)
        let mut stmt = db.prepare("SELECT stock_code FROM answer WHERE date = ? ORDER BY rank")?;

        let all_stocks: Vec<String> = stmt
            .query_map([&date_int], |row| Ok(row.get(0)?))?
            .filter_map(|r| r.ok())
            .collect();

        debug!(
            "날짜 {}의 총 종목 수: {}",
            date.format("%Y-%m-%d"),
            all_stocks.len()
        );

        // 종목별 섹터 정보 생성
        let mut stock_info = HashMap::new();
        let mut sector_rising_count_15: HashMap<String, i32> = HashMap::new();
        let mut sector_rising_count_30: HashMap<String, i32> = HashMap::new();
        let mut sector_stocks: HashMap<String, Vec<(String, f64)>> = HashMap::new();

        // 주가 데이터베이스 연결
        let stock_db_path = std::env::var("STOCK_DB_PATH")
            .unwrap_or_else(|_| "D:\\db\\stock_price(5min).db".to_string());
        let stock_db = match Connection::open(&stock_db_path) {
            Ok(conn) => conn,
            Err(e) => {
                return Err(rusqlite::Error::InvalidPath(
                    format!(
                        "주가 데이터베이스 연결 실패: {} (경로: {})",
                        e, stock_db_path
                    )
                    .into(),
                ));
            }
        };

        for (rank, code) in all_stocks.iter().enumerate() {
            let sector = self.get_sector(code);

            // 9시~9시 30분 상승률 계산
            let five_min_data = match self.get_five_min_data(&stock_db, code, *date) {
                Ok(data) => data,
                Err(_) => continue, // 데이터 없으면 건너뛰기
            };

            if five_min_data.is_empty() {
                continue;
            }

            // 9시~9시 30분 상승률 계산
            let first_open = five_min_data[0].open as f64;
            let last_close = five_min_data.last().unwrap().close as f64;
            let gain_ratio = if first_open > 0.0 {
                (last_close - first_open) / first_open
            } else {
                0.0
            };

            stock_info.insert(code.clone(), (gain_ratio, sector.clone()));

            // 섹터별 상승 종목 수 계산 (15위 내, 30위 내 분리)
            if gain_ratio >= 0.05 {
                // 5% 이상 상승 종목
                let rank_num = rank + 1;
                if rank_num <= 15 {
                    *sector_rising_count_15.entry(sector.clone()).or_insert(0) += 1;
                }
                if rank_num <= 30 {
                    *sector_rising_count_30.entry(sector.clone()).or_insert(0) += 1;
                }
            }

            // 섹터별 종목 목록 생성 (상승률과 함께)
            sector_stocks
                .entry(sector.clone())
                .or_insert_with(Vec::new)
                .push((code.clone(), gain_ratio));
        }

        // 섹터별로 상승률 순으로 정렬
        let mut sector_ranked_stocks = HashMap::new();
        for (sector, stocks) in sector_stocks {
            let mut sorted_stocks = stocks.clone();
            sorted_stocks
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)); // 내림차순 정렬 (상승률 높은 순)
            let ranked_codes: Vec<String> =
                sorted_stocks.into_iter().map(|(code, _)| code).collect();
            sector_ranked_stocks.insert(sector, ranked_codes);
        }

        Ok(DateSectorCache {
            stock_info,
            sector_rising_count_15,
            sector_rising_count_30,
            sector_ranked_stocks,
        })
    }

    /// 5분봉 데이터 가져오기
    pub fn get_five_min_data(
        &self,
        db: &Connection,
        stock_code: &str,
        date: NaiveDate,
    ) -> Result<Vec<FiveMinData>> {
        let date_start = date.format("%Y%m%d").to_string() + "0900"; // 9시 00분
        let date_end = date.format("%Y%m%d").to_string() + "0930"; // 9시 30분

        // 종목코드에서 특수문자 제거 및 안전한 테이블명 생성
        // A + 6자리 숫자 또는 Q + 6자리 숫자 패턴으로 처리
        let safe_table_name = if stock_code.starts_with('A') || stock_code.starts_with('Q') {
            // 이미 A 또는 Q로 시작하는 경우 그대로 사용
            stock_code.replace(['"', '\'', ';', '-', ' '], "_")
        } else if stock_code.len() == 6 && stock_code.chars().all(|c| c.is_ascii_digit()) {
            // 6자리 숫자인 경우 A 접두사 추가
            format!("A{}", stock_code)
        } else {
            // 기타 경우 A 접두사 추가
            format!("A{}", stock_code.replace(['"', '\'', ';', '-', ' '], "_"))
        };

        let query = format!(
            "SELECT date, open, high, low, close, volume 
             FROM \"{}\" 
             WHERE date >= ? AND date <= ?
             ORDER BY date",
            safe_table_name
        );

        let mut stmt = db.prepare(&query)?;
        let data = stmt
            .query_map([&date_start, &date_end], |row| {
                Ok(FiveMinData {
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

        Ok(data)
    }

    /// 캐시를 사용하여 15위 내 섹터 상승 특징 계산
    pub fn calculate_sector_rising_features_15(
        &self,
        stock_code: &str,
        date_cache: &DateSectorCache,
    ) -> (i32, i32) {
        let current_sector = date_cache
            .stock_info
            .get(stock_code)
            .map(|(_, sector)| sector.clone())
            .unwrap_or_else(|| "기타".to_string());

        let same_sector_rising_count_15 = date_cache
            .sector_rising_count_15
            .get(&current_sector)
            .copied()
            .unwrap_or(0);
        let has_3_or_more_rising_15 = if same_sector_rising_count_15 >= 3 {
            1
        } else {
            0
        };

        (same_sector_rising_count_15, has_3_or_more_rising_15)
    }

    /// 캐시를 사용하여 30위 내 섹터 상승 특징 계산
    pub fn calculate_sector_rising_features_30(
        &self,
        stock_code: &str,
        date_cache: &DateSectorCache,
    ) -> (i32, i32) {
        let current_sector = date_cache
            .stock_info
            .get(stock_code)
            .map(|(_, sector)| sector.clone())
            .unwrap_or_else(|| "기타".to_string());

        let same_sector_rising_count_30 = date_cache
            .sector_rising_count_30
            .get(&current_sector)
            .copied()
            .unwrap_or(0);
        let has_3_or_more_rising_30 = if same_sector_rising_count_30 >= 3 {
            1
        } else {
            0
        };

        (same_sector_rising_count_30, has_3_or_more_rising_30)
    }

    /// 캐시를 사용하여 섹터 강도 계산
    pub fn calculate_sector_strength(
        &self,
        stock_code: &str,
        date_cache: &DateSectorCache,
    ) -> (f64, i32) {
        let current_sector = date_cache
            .stock_info
            .get(stock_code)
            .map(|(_, sector)| sector.clone())
            .unwrap_or_else(|| "기타".to_string());

        // 섹터 내 순위 계산
        let sector_ranked_stocks = date_cache.sector_ranked_stocks.get(&current_sector);

        if let Some(stocks) = sector_ranked_stocks {
            let rank = stocks
                .iter()
                .position(|code| code == stock_code)
                .map(|pos| pos + 1)
                .unwrap_or(0) as f64;

            let sector_rank_ratio = if rank > 0.0 { 1.0 / rank } else { 0.0 };
            let is_sector_first = if rank == 1.0 { 1 } else { 0 };

            (sector_rank_ratio, is_sector_first)
        } else {
            (0.0, 0)
        }
    }

    /// 실시간으로 15위 내 섹터 상승 특징 계산
    pub fn calculate_sector_rising_features_realtime_15(
        &self,
        stock_code: &str,
        date: NaiveDate,
        db: &Connection,
    ) -> Result<(i32, i32)> {
        let date_int = date
            .format("%Y%m%d")
            .to_string()
            .parse::<i64>()
            .unwrap_or(0);
        let current_sector = self.get_sector(stock_code);

        // 해당 날짜의 상위 15개 종목 가져오기
        let mut stmt =
            db.prepare("SELECT stock_code FROM answer WHERE date = ? ORDER BY rank LIMIT 15")?;

        let top_15_stocks: Vec<String> = stmt
            .query_map([&date_int], |row| Ok(row.get(0)?))?
            .filter_map(|r| r.ok())
            .collect();

        // 같은 섹터의 상승 종목 수 계산
        let stock_db_path = std::env::var("STOCK_DB_PATH")
            .unwrap_or_else(|_| "D:\\db\\stock_price(5min).db".to_string());
        let stock_db = match Connection::open(&stock_db_path) {
            Ok(conn) => conn,
            Err(_) => return Ok((0, 0)), // DB 연결 실패 시 0 반환
        };

        let mut same_sector_rising_count = 0;

        for code in &top_15_stocks {
            let sector = self.get_sector(code);
            if sector == current_sector {
                // 9시~9시 30분 상승률 계산
                if let Ok(five_min_data) = self.get_five_min_data(&stock_db, code, date) {
                    if !five_min_data.is_empty() {
                        let first_open = five_min_data[0].open as f64;
                        let last_close = five_min_data.last().unwrap().close as f64;
                        let gain_ratio = if first_open > 0.0 {
                            (last_close - first_open) / first_open
                        } else {
                            0.0
                        };

                        if gain_ratio >= 0.05 {
                            // 5% 이상 상승 종목
                            same_sector_rising_count += 1;
                        }
                    }
                }
            }
        }

        let has_3_or_more_rising_15 = if same_sector_rising_count >= 3 { 1 } else { 0 };

        Ok((same_sector_rising_count, has_3_or_more_rising_15))
    }

    /// 실시간으로 30위 내 섹터 상승 특징 계산
    pub fn calculate_sector_rising_features_realtime_30(
        &self,
        stock_code: &str,
        date: NaiveDate,
        db: &Connection,
    ) -> Result<(i32, i32)> {
        let date_int = date
            .format("%Y%m%d")
            .to_string()
            .parse::<i64>()
            .unwrap_or(0);
        let current_sector = self.get_sector(stock_code);

        // 해당 날짜의 상위 30개 종목 가져오기
        let mut stmt =
            db.prepare("SELECT stock_code FROM answer WHERE date = ? ORDER BY rank LIMIT 30")?;

        let top_30_stocks: Vec<String> = stmt
            .query_map([&date_int], |row| Ok(row.get(0)?))?
            .filter_map(|r| r.ok())
            .collect();

        // 같은 섹터의 상승 종목 수 계산
        let stock_db_path = std::env::var("STOCK_DB_PATH")
            .unwrap_or_else(|_| "D:\\db\\stock_price(5min).db".to_string());
        let stock_db = match Connection::open(&stock_db_path) {
            Ok(conn) => conn,
            Err(_) => return Ok((0, 0)), // DB 연결 실패 시 0 반환
        };

        let mut same_sector_rising_count = 0;

        for code in &top_30_stocks {
            let sector = self.get_sector(code);
            if sector == current_sector {
                // 9시~9시 30분 상승률 계산
                if let Ok(five_min_data) = self.get_five_min_data(&stock_db, code, date) {
                    if !five_min_data.is_empty() {
                        let first_open = five_min_data[0].open as f64;
                        let last_close = five_min_data.last().unwrap().close as f64;
                        let gain_ratio = if first_open > 0.0 {
                            (last_close - first_open) / first_open
                        } else {
                            0.0
                        };

                        if gain_ratio >= 0.05 {
                            // 5% 이상 상승 종목
                            same_sector_rising_count += 1;
                        }
                    }
                }
            }
        }

        let has_3_or_more_rising_30 = if same_sector_rising_count >= 3 { 1 } else { 0 };

        Ok((same_sector_rising_count, has_3_or_more_rising_30))
    }

    /// 실시간으로 섹터 강도 계산
    pub fn calculate_sector_strength_realtime(
        &self,
        stock_code: &str,
        date: NaiveDate,
        db: &Connection,
    ) -> Result<(f64, i32)> {
        let date_int = date
            .format("%Y%m%d")
            .to_string()
            .parse::<i64>()
            .unwrap_or(0);
        let current_sector = self.get_sector(stock_code);

        // 해당 날짜의 모든 종목 가져오기
        let mut stmt = db.prepare("SELECT stock_code FROM answer WHERE date = ? ORDER BY rank")?;

        let all_stocks: Vec<String> = stmt
            .query_map([&date_int], |row| Ok(row.get(0)?))?
            .filter_map(|r| r.ok())
            .collect();

        // 같은 섹터의 종목들만 필터링하고 상승률 계산
        let stock_db_path = std::env::var("STOCK_DB_PATH")
            .unwrap_or_else(|_| "D:\\db\\stock_price(5min).db".to_string());
        let stock_db = match Connection::open(&stock_db_path) {
            Ok(conn) => conn,
            Err(_) => return Ok((1.0, 1)), // DB 연결 실패 시 기본값 반환
        };

        let mut sector_stocks = Vec::new();

        for code in &all_stocks {
            let sector = self.get_sector(code);
            if sector == current_sector {
                // 9시~9시 30분 상승률 계산
                if let Ok(five_min_data) = self.get_five_min_data(&stock_db, code, date) {
                    if !five_min_data.is_empty() {
                        let first_open = five_min_data[0].open as f64;
                        let last_close = five_min_data.last().unwrap().close as f64;
                        let gain_ratio = if first_open > 0.0 {
                            (last_close - first_open) / first_open
                        } else {
                            0.0
                        };

                        sector_stocks.push((code.clone(), gain_ratio));
                    }
                }
            }
        }

        // 상승률 순으로 정렬 (내림차순)
        sector_stocks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 현재 종목의 순위 찾기
        let rank = sector_stocks
            .iter()
            .position(|(code, _)| code == stock_code)
            .map(|pos| pos + 1)
            .unwrap_or(1);

        let sector_rank_ratio = 1.0 / rank as f64;
        let is_sector_first = if rank == 1 { 1 } else { 0 };

        Ok((sector_rank_ratio, is_sector_first))
    }

    /// DB에서 모든 테이블 목록을 가져옴 (5분봉 데이터 테이블들)
    pub fn get_db_table_list(&self, db: &Connection) -> Result<Vec<String>> {
        let mut stmt = db.prepare("SELECT name FROM sqlite_master WHERE type='table'")?;
        let table_iter = stmt.query_map([], |row| Ok(row.get(0)?))?;

        let mut tables = Vec::new();
        for table in table_iter {
            let table_name: String = table?;
            // A로 시작하는 테이블만 필터링 (종목 테이블)
            if table_name.starts_with('A') {
                tables.push(table_name);
            }
        }

        info!("DB에서 {}개의 종목 테이블을 찾았습니다", tables.len());
        debug!(
            "테이블 목록 (처음 10개): {:?}",
            tables.iter().take(10).collect::<Vec<_>>()
        );

        Ok(tables)
    }

    /// CSV에 있는 종목과 DB에 있는 종목을 비교하여 없는 종목들을 찾음
    pub fn find_missing_stocks(&self, db: &Connection) -> Result<Vec<String>> {
        // DB에서 테이블 목록 가져오기
        let db_tables = self.get_db_table_list(db)?;

        // DB 테이블명에서 종목코드 추출 (A 접두사 제거)
        let db_stock_codes: Vec<String> = db_tables
            .iter()
            .map(|table_name| table_name.trim_start_matches('A').to_string())
            .collect();

        // CSV에 있는 종목코드들
        let csv_stock_codes: Vec<String> = self.stock_map.keys().cloned().collect();

        // CSV에는 있지만 DB에는 없는 종목들 찾기
        let missing_stocks: Vec<String> = csv_stock_codes
            .iter()
            .filter(|csv_code| !db_stock_codes.contains(csv_code))
            .cloned()
            .collect();

        info!("CSV에 있지만 DB에 없는 종목: {}개", missing_stocks.len());
        debug!("없는 종목들: {:?}", missing_stocks);

        Ok(missing_stocks)
    }

    /// DB에만 있고 CSV에는 없는 종목들 찾기
    pub fn find_extra_stocks(&self, db: &Connection) -> Result<Vec<String>> {
        // DB에서 테이블 목록 가져오기
        let db_tables = self.get_db_table_list(db)?;

        // DB 테이블명에서 종목코드 추출 (A 접두사 제거)
        let db_stock_codes: Vec<String> = db_tables
            .iter()
            .map(|table_name| table_name.trim_start_matches('A').to_string())
            .collect();

        // CSV에 있는 종목코드들
        let csv_stock_codes: Vec<String> = self.stock_map.keys().cloned().collect();

        // DB에는 있지만 CSV에는 없는 종목들 찾기
        let extra_stocks: Vec<String> = db_stock_codes
            .iter()
            .filter(|db_code| !csv_stock_codes.contains(db_code))
            .cloned()
            .collect();

        info!("DB에만 있고 CSV에는 없는 종목: {}개", extra_stocks.len());
        debug!("추가 종목들: {:?}", extra_stocks);

        Ok(extra_stocks)
    }

    /// CSV에 있는 종목 수 반환
    pub fn get_csv_stock_count(&self) -> usize {
        self.stock_map.len()
    }

    /// CSV에 있는 종목코드 목록 반환
    pub fn get_csv_stock_codes(&self) -> Vec<String> {
        self.stock_map.keys().cloned().collect()
    }

    /// CSV에 있는 종목코드 목록 (처음 n개) 반환
    pub fn get_csv_stock_codes_limit(&self, limit: usize) -> Vec<String> {
        self.stock_map.keys().take(limit).cloned().collect()
    }
}
