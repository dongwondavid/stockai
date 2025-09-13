use chrono::NaiveDate;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::config::get_config;

/// 거래 캘린더를 담당하는 구조체
/// samsung_1min_dates.txt 파일에서 거래일 목록을 로드하여 관리
#[derive(Clone)]
pub struct TradingCalender {
    // 전체 거래일 캐시 (YYYYMMDD 형식 파일에서 로드)
    all_trading_days_set: Option<HashSet<NaiveDate>>,
    // 정렬된 전체 거래일 목록 (next/previous 계산용)
    all_trading_days_list: Option<Vec<NaiveDate>>,
    // 설정 파일 경로 (samsung_1min_dates.txt)
    trading_dates_file_path: String,
}

impl TradingCalender {
    /// 새로운 TradingCalender 인스턴스를 생성합니다.
    pub fn new() -> StockrsResult<Self> {
        // 시간 관리용 거래일 파일 경로를 설정에서 읽음
        let cfg = get_config().map_err(|e| StockrsError::Time { operation: "설정 로드".to_string(), reason: e.to_string() })?;
        let trading_dates_file_path = cfg.time_management.trading_dates_file_path.clone();

        let mut calender = TradingCalender {
            all_trading_days_set: None,
            all_trading_days_list: None,
            trading_dates_file_path,
        };

        // 거래일 목록 로드
        calender.load_all_trading_days_internal()?;

        Ok(calender)
    }

    /// 다음 거래일을 계산합니다 (samsung_1min_dates.txt 기준)
    pub fn next_trading_day(&mut self, date: NaiveDate) -> NaiveDate {
        // 거래일 목록이 로드되지 않은 경우 로드
        if self.all_trading_days_list.is_none() {
            if let Err(e) = self.load_all_trading_days_internal() {
                eprintln!("거래일 파일 로드 실패: {}", e);
                return date; // 로드 실패 시 현재 날짜 반환
            }
        }

        let trading_days = self.all_trading_days_list.as_ref().unwrap();
        match trading_days.binary_search(&date) {
            Ok(index) => {
                // 현재 날짜가 거래일인 경우, 다음 거래일 반환
                if index + 1 < trading_days.len() {
                    trading_days[index + 1]
                } else {
                    // 마지막 거래일인 경우 현재 날짜 반환
                    date
                }
            },
            Err(insert_point) => {
                // 현재 날짜가 거래일이 아닌 경우, 그 이후 첫 번째 거래일 반환
                if insert_point < trading_days.len() {
                    trading_days[insert_point]
                } else {
                    // 모든 거래일보다 이후인 경우 현재 날짜 반환
                    date
                }
            }
        }
    }

    /// 이전 거래일을 계산합니다 (samsung_1min_dates.txt 기준)
    pub fn previous_trading_day(&mut self, date: NaiveDate) -> NaiveDate {
        // 거래일 목록이 로드되지 않은 경우 로드
        if self.all_trading_days_list.is_none() {
            if let Err(e) = self.load_all_trading_days_internal() {
                eprintln!("거래일 파일 로드 실패: {}", e);
                return date; // 로드 실패 시 현재 날짜 반환
            }
        }

        let trading_days = self.all_trading_days_list.as_ref().unwrap();
        match trading_days.binary_search(&date) {
            Ok(index) => {
                // 현재 날짜가 거래일인 경우, 이전 거래일 반환
                if index > 0 {
                    trading_days[index - 1]
                } else {
                    // 첫 번째 거래일인 경우 현재 날짜 반환
                    date
                }
            },
            Err(insert_point) => {
                // 현재 날짜가 거래일이 아닌 경우, 그 이전 첫 번째 거래일 반환
                if insert_point > 0 {
                    trading_days[insert_point - 1]
                } else {
                    // 모든 거래일보다 이전인 경우 현재 날짜 반환
                    date
                }
            }
        }
    }

    /// 주어진 날짜가 거래일이 아닌지 확인합니다 (samsung_1min_dates.txt 기준)
    pub fn is_non_trading_day(&mut self, date: NaiveDate) -> bool {
        // 거래일 목록이 로드되지 않은 경우 로드
        if self.all_trading_days_set.is_none() {
            if let Err(e) = self.load_all_trading_days_internal() {
                eprintln!("거래일 파일 로드 실패: {}", e);
                return true; // 로드 실패 시 거래일이 아닌 것으로 간주
            }
        }

        // 거래일 목록에 없는 날짜는 거래일이 아님
        !self.all_trading_days_set.as_ref().unwrap().contains(&date)
    }

    /// 내부적으로 모든 거래일 목록을 파일에서 로드합니다.
    fn load_all_trading_days_internal(&mut self) -> StockrsResult<()> {
        if self.all_trading_days_set.is_some() {
            return Ok(()); // 이미 로드됨
        }

        let path = Path::new(&self.trading_dates_file_path);

        if !path.exists() {
            return Err(StockrsError::Time {
                operation: "거래일 파일 로드".to_string(),
                reason: format!("거래일 파일이 존재하지 않습니다: {}", self.trading_dates_file_path),
            });
        }

        let content = fs::read_to_string(path).map_err(|e| {
            StockrsError::Time {
                operation: "거래일 파일 읽기".to_string(),
                reason: format!("거래일 파일 읽기 실패: {}", e),
            }
        })?;

        let mut trading_days: Vec<NaiveDate> = content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    NaiveDate::parse_from_str(trimmed, "%Y%m%d").ok() // YYYYMMDD 형식 파싱
                } else {
                    None
                }
            })
            .collect();

        if trading_days.is_empty() {
            return Err(StockrsError::Time {
                operation: "거래일 파일 파싱".to_string(),
                reason: format!("거래일 파일이 비어있거나 파싱할 수 없습니다: {}", self.trading_dates_file_path),
            });
        }

        trading_days.sort_unstable(); // 이진 검색을 위해 정렬
        self.all_trading_days_set = Some(trading_days.iter().cloned().collect());
        self.all_trading_days_list = Some(trading_days);

        Ok(())
    }
}

impl Default for TradingCalender {
    fn default() -> Self {
        // 전역 설정을 우선 시도
        if let Ok(cfg) = crate::utility::config::get_config() {
            return TradingCalender {
                all_trading_days_set: None,
                all_trading_days_list: None,
                trading_dates_file_path: cfg.time_management.trading_dates_file_path.clone(),
            };
        }

        // config.toml이 없는 경우, 예시 설정으로 폴백
        if let Ok(example_cfg) = crate::utility::config::Config::load_from_file("config.example.toml") {
            return TradingCalender {
                all_trading_days_set: None,
                all_trading_days_list: None,
                trading_dates_file_path: example_cfg.time_management.trading_dates_file_path,
            };
        }

        // 최종 폴백: 안전한 기본 경로
        TradingCalender {
            all_trading_days_set: None,
            all_trading_days_list: None,
            trading_dates_file_path: "data/samsung_1min_dates.txt".to_string(),
        }
    }
}