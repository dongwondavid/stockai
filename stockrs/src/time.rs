use chrono::{Local, DateTime, Duration, Datelike, Weekday, NaiveDate, TimeZone};
use std::thread;
use std::fs;
use std::path::Path;
use crate::local_time;

/// Signals corresponding to specific time events within the trading day
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TimeSignal {
    /// 08:30 데이터 준비 시간
    DataPrep,
    /// 09:00 장 시작 알림
    MarketOpen,
    /// 09:01 ~ 15:29 1분 단위 업데이트
    Update,
    /// 15:30 장 종료 알림
    MarketClose,
    /// 장 종료 후 다음 영업일 08:30까지 대기
    Overnight,
}

/// `TimeService` 구조체는 내부에 현재 시간(`current`)을 보관하며,
/// 다음 이벤트 시각 계산과 대기를 수행합니다.
pub struct TimeService {
    current: DateTime<Local>,
    current_signal: TimeSignal,
}

impl TimeService {
    /// 새로운 `TimeService` 인스턴스를 생성합니다.
    ///
    /// 먼저 `Local::now()`로 현재 시각을 가져와 `current`를 설정한 뒤,
    /// 다음 거래 이벤트를 계산하여 `current`와 `current_signal`을 갱신합니다.
    pub fn new() -> Self {
        let now = Local::now();
        let mut service = TimeService { 
            current: now,
            current_signal: TimeSignal::DataPrep, // 임시 초기값
        };
        let (next_time, signal) = service.compute_next_time();
        service.current = next_time;
        service.current_signal = signal;
        service
    }

    /// 내부 `current` 시각을 반환합니다.
    pub fn now(&self) -> DateTime<Local> {
        self.current
    }

    pub fn now_signal(&self) -> TimeSignal {
        self.current_signal
    }

    /// 내부 시간(`current`)을 기준으로 다음 이벤트 시각과 시그널을 계산,
    /// 동시에 내부 시간을 그 다음 이벤트 시각으로 업데이트합니다.
    pub fn advance(&mut self) -> (DateTime<Local>, TimeSignal) {
        let (next_time, signal) = self.compute_next_time();
        self.current = next_time;
        self.current_signal = signal;
        (next_time, signal)
    }

    /// 주어진 목표 시각(`target`)까지 블로킹 대기를 수행합니다.
    pub fn wait_until(&self, target: DateTime<Local>) {
        let now = Local::now();
        if target > now {
            if let Ok(dur) = target.signed_duration_since(now).to_std() {
                thread::sleep(dur);
            }
        }
    }

    /// 현재 시각(`current`)을 기준으로 다음 이벤트 시각과 해당 시그널을 계산
    /// 
    /// 시그널 순서:
    /// 1. DataPrep (08:30) - 데이터 준비 시간
    /// 2. MarketOpen (09:00) - 장 시작
    /// 3. Update (09:01~15:29) - 1분 단위 업데이트
    /// 4. MarketClose (15:30) - 장 종료
    /// 5. Overnight - 다음 거래일 08:30 대기
    fn compute_next_time(&self) -> (DateTime<Local>, TimeSignal) {
        let today = self.current.date_naive();

        let prep_time = local_time!(today, 8, 30, 0);
        let open_time  = local_time!(today, 9, 0, 0);
        let last_upd   = local_time!(today, 15, 29, 0);
        let close_time = local_time!(today, 15, 30, 0);

        if self.current < prep_time {
            (prep_time, TimeSignal::DataPrep)
        } else if self.current < open_time {
            (open_time, TimeSignal::MarketOpen)
        } else if self.current < last_upd {
            let next = self.current + Duration::minutes(1);
            (next, TimeSignal::Update)
        } else if self.current < close_time {
            (close_time, TimeSignal::MarketClose)
        } else {
            let next_date = next_trading_day(today);
            let next_datetime = local_time!(next_date, 8, 30, 0);
            (next_datetime, TimeSignal::Overnight)
        }
    }
}

// ------------------------------------------------
// 내부 헬퍼 함수들
// ------------------------------------------------

/// 주어진 날짜가 주말(토/일)인지 확인
fn is_weekend(date: NaiveDate) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

/// 해당 연도의 공휴일 목록을 파일에서 로드
fn load_holidays(year: i32) -> Vec<NaiveDate> {
    let filename = format!("data/market_close_day_{}.txt", year);
    let path = Path::new(&filename);
    
    if !path.exists() {
        return Vec::new();
    }
    
    match fs::read_to_string(path) {
        Ok(content) => {
            content
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").ok()
                    } else {
                        None
                    }
                })
                .collect()
        }
        Err(_) => Vec::new()
    }
}

/// 주어진 날짜가 공휴일인지 확인
fn is_holiday(date: NaiveDate) -> bool {
    let holidays = load_holidays(date.year());
    holidays.contains(&date)
}

/// 다음 영업일(Date 부분) 계산 (주말과 공휴일 건너뛰기)
fn next_trading_day(date: NaiveDate) -> NaiveDate {
    let mut next = date + Duration::days(1);
    while is_weekend(next) || is_holiday(next) {
        next = next + Duration::days(1);
    }
    next
}

// ------------------------------------------------
// 테스트
// ------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};

    #[test]
    fn test_compute_next_time_signals() {
        let c = Local;
        
        // 07:30 -> 데이터 준비
        let now = c.with_ymd_and_hms(2025, 7, 16, 7, 30, 0).unwrap();
        let service = TimeService { current: now, current_signal: TimeSignal::DataPrep };
        let (next, sig) = service.compute_next_time();
        assert_eq!(sig, TimeSignal::DataPrep);
        assert_eq!(next.time().hour(), 8);
        assert_eq!(next.time().minute(), 30);

        // 09:00 이후 -> 업데이트
        let now = c.with_ymd_and_hms(2025, 7, 16, 10, 0, 0).unwrap();
        let service = TimeService { current: now, current_signal: TimeSignal::Update };
        let (next, sig) = service.compute_next_time();
        assert_eq!(sig, TimeSignal::Update);
        assert_eq!(next.time().minute(), 1);

        // 15:30 이후 -> 다음 거래일
        let friday = c.with_ymd_and_hms(2025, 7, 18, 16, 0, 0).unwrap();
        let service = TimeService { current: friday, current_signal: TimeSignal::Overnight };
        let (next, sig) = service.compute_next_time();
        assert_eq!(sig, TimeSignal::Overnight);
        assert_eq!(next.date_naive().weekday(), Weekday::Mon);
    }

    #[test]
    fn test_time_service_flow() {
        let mut svc = TimeService::new();
        // First advance from now to next event
        let (t1, s1) = svc.advance();
        assert!(t1 >= svc.now());
        // Advance again
        let (t2, s2) = svc.advance();
        assert!(t2 >= t1);
        // Signals should be valid enum variants
        assert!(matches!(s1, TimeSignal::DataPrep | TimeSignal::MarketOpen | TimeSignal::Update | TimeSignal::MarketClose | TimeSignal::Overnight));
        assert!(matches!(s2, TimeSignal::DataPrep | TimeSignal::MarketOpen | TimeSignal::Update | TimeSignal::MarketClose | TimeSignal::Overnight));
    }

    #[test]
    fn test_holiday_loading() {
        let holidays = load_holidays(2025);
        assert!(!holidays.is_empty());
        
        // 2025년 1월 1일은 공휴일이어야 함
        let new_year = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        assert!(holidays.contains(&new_year));
        
        // 2025년 1월 27일도 공휴일이어야 함
        let holiday = NaiveDate::from_ymd_opt(2025, 1, 27).unwrap();
        assert!(holidays.contains(&holiday));
    }

    #[test]
    fn test_next_trading_day_with_holidays() {
        // 2025년 1월 1일(수요일, 공휴일) 다음 영업일은 1월 2일(목요일)이어야 함
        let new_year = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let next_day = next_trading_day(new_year);
        assert_eq!(next_day, NaiveDate::from_ymd_opt(2025, 1, 2).unwrap());
        
        // 2025년 1월 27일(월요일, 공휴일) 다음 영업일은 1월 31일(금요일)이어야 함
        // (1월 27일, 28일, 29일, 30일이 모두 공휴일이므로)
        let holiday = NaiveDate::from_ymd_opt(2025, 1, 27).unwrap();
        let next_day = next_trading_day(holiday);
        assert_eq!(next_day, NaiveDate::from_ymd_opt(2025, 1, 31).unwrap());
        
        // 주말 + 공휴일 조합 테스트: 2025년 1월 25일(토요일) 다음 영업일은 1월 31일(금요일)이어야 함
        // (1월 27일, 28일, 29일, 30일이 모두 공휴일이므로)
        let saturday = NaiveDate::from_ymd_opt(2025, 1, 25).unwrap();
        let next_day = next_trading_day(saturday);
        assert_eq!(next_day, NaiveDate::from_ymd_opt(2025, 1, 31).unwrap());
    }
}
