use chrono::{NaiveDate, Weekday, Datelike};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::utility::errors::{StockrsError, StockrsResult};



/// 공휴일 체크를 담당하는 구조체
#[derive(Clone)]
pub struct HolidayChecker {
    // 연도별 공휴일 캐시
    holiday_cache: HashMap<i32, Vec<NaiveDate>>,
    // 설정 파일 경로 템플릿
    file_path_template: String,
}

impl HolidayChecker {
    /// 새로운 HolidayChecker 인스턴스를 생성합니다.
    pub fn new() -> StockrsResult<Self> {
        let file_path_template = if let Ok(config) = crate::utility::config::get_config() {
            config.time_management.market_close_file_path.clone()
        } else {
            "data/market_close_day_{}.txt".to_string()
        };

        Ok(HolidayChecker {
            holiday_cache: HashMap::new(),
            file_path_template,
        })
    }

    /// 주어진 날짜가 주말(토/일)인지 확인합니다.
    pub fn is_weekend(&self, date: NaiveDate) -> bool {
        matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
    }

    /// 주어진 날짜가 공휴일인지 확인합니다.
    pub fn is_holiday(&mut self, date: NaiveDate) -> bool {
        let year = date.year();
        
        // 캐시에서 확인
        if let Some(holidays) = self.holiday_cache.get(&year) {
            return holidays.contains(&date);
        }

        // 캐시에 없으면 파일에서 로드
        match self.load_holidays_for_year(year) {
            Ok(holidays) => {
                // 캐시에 저장
                self.holiday_cache.insert(year, holidays.clone());
                holidays.contains(&date)
            }
            Err(e) => {
                eprintln!("공휴일 파일 로드 실패: {}", e);
                false // 에러 시 공휴일이 아닌 것으로 처리
            }
        }
    }

    /// 주어진 날짜가 거래일이 아닌지 확인합니다 (주말 또는 공휴일).
    pub fn is_non_trading_day(&mut self, date: NaiveDate) -> bool {
        self.is_weekend(date) || self.is_holiday(date)
    }

    /// 다음 거래일을 계산합니다 (주말과 공휴일을 건너뛰고).
    pub fn next_trading_day(&mut self, date: NaiveDate) -> NaiveDate {
        let mut next = date + chrono::Duration::days(1);
        while self.is_non_trading_day(next) {
            next += chrono::Duration::days(1);
        }
        next
    }

    /// 이전 거래일을 계산합니다 (주말과 공휴일을 건너뛰고).
    pub fn previous_trading_day(&mut self, date: NaiveDate) -> NaiveDate {
        let mut prev = date - chrono::Duration::days(1);
        while self.is_non_trading_day(prev) {
            prev -= chrono::Duration::days(1);
        }
        prev
    }

    /// 특정 연도의 공휴일 목록을 파일에서 로드합니다.
    pub fn load_holidays_for_year(&self, year: i32) -> StockrsResult<Vec<NaiveDate>> {
        let filename = self.file_path_template.replace("{}", &year.to_string());
        let path = Path::new(&filename);

        if !path.exists() {
            return Err(StockrsError::Time {
                operation: "공휴일 파일 로드".to_string(),
                reason: format!("공휴일 파일이 존재하지 않습니다: {}", filename),
            });
        }

        let content = fs::read_to_string(path).map_err(|e| {
            StockrsError::Time {
                operation: "공휴일 파일 읽기".to_string(),
                reason: format!("공휴일 파일 읽기 실패: {}", e),
            }
        })?;

        let holidays: Vec<NaiveDate> = content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").ok()
                } else {
                    None
                }
            })
            .collect();

        if holidays.is_empty() {
            return Err(StockrsError::Time {
                operation: "공휴일 파일 파싱".to_string(),
                reason: format!("공휴일 파일이 비어있거나 파싱할 수 없습니다: {}", filename),
            });
        }

        Ok(holidays)
    }

    /// 캐시를 무효화합니다.
    pub fn clear_cache(&mut self) {
        self.holiday_cache.clear();
    }

    /// 특정 연도의 캐시를 무효화합니다.
    pub fn clear_cache_for_year(&mut self, year: i32) {
        self.holiday_cache.remove(&year);
    }

    /// 캐시된 연도 목록을 반환합니다.
    pub fn cached_years(&self) -> Vec<i32> {
        self.holiday_cache.keys().cloned().collect()
    }

    /// 특정 연도의 공휴일 수를 반환합니다.
    pub fn holiday_count_for_year(&mut self, year: i32) -> StockrsResult<usize> {
        if let Some(holidays) = self.holiday_cache.get(&year) {
            Ok(holidays.len())
        } else {
            let holidays = self.load_holidays_for_year(year)?;
            self.holiday_cache.insert(year, holidays.clone());
            Ok(holidays.len())
        }
    }
}

impl Default for HolidayChecker {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| HolidayChecker {
            holiday_cache: HashMap::new(),
            file_path_template: "data/market_close_day_{}.txt".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weekend_detection() {
        let checker = HolidayChecker::default();
        
        // 2025년 1월 25일은 토요일
        let saturday = NaiveDate::from_ymd_opt(2025, 1, 25)
            .expect("Invalid test date");
        assert!(checker.is_weekend(saturday));
        
        // 2025년 1월 26일은 일요일
        let sunday = NaiveDate::from_ymd_opt(2025, 1, 26)
            .expect("Invalid test date");
        assert!(checker.is_weekend(sunday));
        
        // 2025년 1월 27일은 월요일 (공휴일이지만 주말은 아님)
        let monday = NaiveDate::from_ymd_opt(2025, 1, 27)
            .expect("Invalid test date");
        assert!(!checker.is_weekend(monday));
    }

    #[test]
    fn test_next_trading_day() {
        let mut checker = HolidayChecker::default();
        
        // 2025년 1월 25일(토요일) 다음 거래일은 1월 31일(금요일)이어야 함
        // (1월 27일, 28일, 29일, 30일이 모두 공휴일이므로)
        let saturday = NaiveDate::from_ymd_opt(2025, 1, 25)
            .expect("Invalid test date");
        let next_day = checker.next_trading_day(saturday);
        assert_eq!(next_day, NaiveDate::from_ymd_opt(2025, 1, 31)
            .expect("Invalid expected date"));
    }

    #[test]
    fn test_previous_trading_day() {
        let mut checker = HolidayChecker::default();
        
        // 2025년 1월 31일(금요일) 이전 거래일은 1월 24일(금요일)이어야 함
        let friday = NaiveDate::from_ymd_opt(2025, 1, 31)
            .expect("Invalid test date");
        let prev_day = checker.previous_trading_day(friday);
        assert_eq!(prev_day, NaiveDate::from_ymd_opt(2025, 1, 24)
            .expect("Invalid expected date"));
    }
} 