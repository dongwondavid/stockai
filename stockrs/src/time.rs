use crate::utility::holiday_checker::HolidayChecker;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::config;
use crate::local_time;
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone};
use std::thread;

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
#[derive(Clone)]
pub struct TimeService {
    current: DateTime<Local>,
    current_signal: TimeSignal,
    // 시간 캐싱을 위한 필드들
    cached_time: Option<DateTime<Local>>,
    cache_timestamp: Option<std::time::Instant>,
    cache_duration: std::time::Duration,
    // 공휴일 체커
    holiday_checker: HolidayChecker,
}

impl TimeService {
    /// 새로운 `TimeService` 인스턴스를 생성합니다.
    ///
    /// config.toml의 start_date를 읽어서 시작 시간을 설정하고,
    /// 다음 거래 이벤트를 계산하여 `current`와 `current_signal`을 갱신합니다.
    pub fn new() -> StockrsResult<Self> {
        // config에서 start_date 읽기
        let start_time = if let Ok(config) = config::get_config() {
            let start_date_str = &config.time_management.start_date;

            // YYYYMMDD 형식을 NaiveDate로 파싱
            let year = start_date_str[0..4].parse::<i32>().map_err(|_| {
                StockrsError::Time {
                    operation: "start_date 파싱".to_string(),
                    reason: format!("연도 파싱 실패: {}", start_date_str),
                }
            })?;
            let month = start_date_str[4..6].parse::<u32>().map_err(|_| {
                StockrsError::Time {
                    operation: "start_date 파싱".to_string(),
                    reason: format!("월 파싱 실패: {}", start_date_str),
                }
            })?;
            let day = start_date_str[6..8].parse::<u32>().map_err(|_| {
                StockrsError::Time {
                    operation: "start_date 파싱".to_string(),
                    reason: format!("일 파싱 실패: {}", start_date_str),
                }
            })?;

            if let Some(naive_date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
                // 시작일의 8:00부터 시작
                let time = naive_date
                    .and_hms_opt(8, 0, 0)
                    .ok_or_else(|| {
                        StockrsError::Time {
                            operation: "시간 생성".to_string(),
                            reason: format!("잘못된 시간 생성: {}일 8:00:00", naive_date),
                        }
                    })?;
                Local
                    .from_local_datetime(&time)
                    .single()
                    .ok_or_else(|| {
                        StockrsError::Time {
                            operation: "로컬 시간 변환".to_string(),
                            reason: format!("로컬 시간 변환 실패: {}", naive_date),
                        }
                    })
            } else {
                Err(StockrsError::Time {
                    operation: "날짜 파싱".to_string(),
                    reason: format!(
                        "잘못된 날짜 형식: {} (연도: {}, 월: {}, 일: {})",
                        start_date_str, year, month, day
                    ),
                })
            }
        } else {
            Err(StockrsError::Time {
                operation: "설정 로드".to_string(),
                reason: "설정 파일을 읽을 수 없습니다. config.toml을 확인하세요.".to_string(),
            })
        };

        let mut service = TimeService {
            current: start_time?,
            current_signal: TimeSignal::DataPrep,
            cached_time: None,
            cache_timestamp: None,
            cache_duration: std::time::Duration::from_secs(1), // 1초 캐시
            holiday_checker: HolidayChecker::new().unwrap_or_default(),
        };
        let (next_time, signal) = service.compute_next_time();
        service.current = next_time;
        service.current_signal = signal;
        Ok(service)
    }

    /// 내부 `current` 시각을 반환합니다.
    /// 캐싱 메커니즘을 통해 일관된 시간 정보를 제공합니다.
    pub fn now(&self) -> DateTime<Local> {
        // 캐시된 시간이 있고 유효한 경우 사용
        if let (Some(cached_time), Some(cache_timestamp)) = (self.cached_time, self.cache_timestamp) {
            if cache_timestamp.elapsed() < self.cache_duration {
                return cached_time;
            }
        }
        
        // 캐시가 없거나 만료된 경우 현재 시간 반환
        self.current
    }

    pub fn now_signal(&self) -> TimeSignal {
        self.current_signal
    }

    /// 현재 시간 캐시를 업데이트합니다.
    /// 백테스팅 모드에서는 시간 단위 일관성을 보장하고,
    /// 실시간 모드에서는 적절한 시간 갱신 주기를 설정합니다.
    pub fn update_cache(&mut self) {
        // 설정에서 캐시 지속 시간 읽기
        self.cache_duration = if let Ok(config) = config::get_config() {
            std::time::Duration::from_secs(config.time_management.event_check_interval / 2) // 이벤트 체크 간격의 절반
        } else {
            std::time::Duration::from_secs(1) // 기본값 1초
        };
        
        self.cached_time = Some(self.current);
        self.cache_timestamp = Some(std::time::Instant::now());
    }

    /// 캐시를 무효화합니다.
    pub fn invalidate_cache(&mut self) {
        self.cached_time = None;
        self.cache_timestamp = None;
    }

    // ------------------------------------------------
    // Duration 연산 헬퍼 함수들
    // ------------------------------------------------

    /// 현재 시간에 1분을 더합니다.
    pub fn add_minute(&self) -> DateTime<Local> {
        self.current + Duration::minutes(1)
    }

    /// 현재 시간에 지정된 분을 더합니다.
    pub fn add_minutes(&self, minutes: i64) -> DateTime<Local> {
        self.current + Duration::minutes(minutes)
    }

    /// 현재 시간에 지정된 시간을 더합니다.
    pub fn add_hours(&self, hours: i64) -> DateTime<Local> {
        self.current + Duration::hours(hours)
    }

    /// 현재 시간에 지정된 일을 더합니다.
    pub fn add_days(&self, days: i64) -> DateTime<Local> {
        self.current + Duration::days(days)
    }

    /// 현재 시간에서 1분을 뺍니다.
    pub fn subtract_minute(&self) -> DateTime<Local> {
        self.current - Duration::minutes(1)
    }

    /// 현재 시간에서 지정된 분을 뺍니다.
    pub fn subtract_minutes(&self, minutes: i64) -> DateTime<Local> {
        self.current - Duration::minutes(minutes)
    }

    /// 현재 시간에서 지정된 시간을 뺍니다.
    pub fn subtract_hours(&self, hours: i64) -> DateTime<Local> {
        self.current - Duration::hours(hours)
    }

    /// 현재 시간에서 지정된 일을 뺍니다.
    pub fn subtract_days(&self, days: i64) -> DateTime<Local> {
        self.current - Duration::days(days)
    }

    /// 두 시간 간의 차이를 분 단위로 계산합니다.
    pub fn diff_minutes(&self, other: DateTime<Local>) -> i64 {
        self.current.signed_duration_since(other).num_minutes()
    }

    /// 두 시간 간의 차이를 시간 단위로 계산합니다.
    pub fn diff_hours(&self, other: DateTime<Local>) -> i64 {
        self.current.signed_duration_since(other).num_hours()
    }

    /// 두 시간 간의 차이를 일 단위로 계산합니다.
    pub fn diff_days(&self, other: DateTime<Local>) -> i64 {
        self.current.signed_duration_since(other).num_days()
    }

    /// 정적 함수: 두 시간 간의 차이를 분 단위로 계산합니다.
    pub fn diff_minutes_static(time1: DateTime<Local>, time2: DateTime<Local>) -> i64 {
        time1.signed_duration_since(time2).num_minutes()
    }

    /// 정적 함수: 두 시간 간의 차이를 시간 단위로 계산합니다.
    pub fn diff_hours_static(time1: DateTime<Local>, time2: DateTime<Local>) -> i64 {
        time1.signed_duration_since(time2).num_hours()
    }

    /// 정적 함수: 두 시간 간의 차이를 일 단위로 계산합니다.
    pub fn diff_days_static(time1: DateTime<Local>, time2: DateTime<Local>) -> i64 {
        time1.signed_duration_since(time2).num_days()
    }

    /// 내부 시간(`current`)을 기준으로 다음 이벤트 시각과 시그널을 계산,
    /// 동시에 내부 시간을 그 다음 이벤트 시각으로 업데이트합니다.
    pub fn advance(&mut self) -> (DateTime<Local>, TimeSignal) {
        let (next_time, signal) = self.compute_next_time();
        self.current = next_time;
        self.current_signal = signal;
        
        // 시간이 변경되었으므로 캐시 업데이트
        self.update_cache();
        
        (next_time, signal)
    }

    /// 주어진 목표 시각(`target`)까지 블로킹 대기를 수행합니다.
    pub fn wait_until(&self, target: DateTime<Local>) {
        if target > Local::now() {
            if let Ok(dur) = target.signed_duration_since(Local::now()).to_std() {
                thread::sleep(dur);
            }
        }
    }

    /// 현재 시각(`current`)을 기준으로 다음 이벤트 시각과 해당 시그널을 계산
    ///
    /// 시그널 순서:
    /// 1. DataPrep (설정된 시간) - 데이터 준비 시간
    /// 2. MarketOpen (설정된 시간) - 장 시작
    /// 3. Update (설정된 시간 범위) - 1분 단위 업데이트
    /// 4. MarketClose (설정된 시간) - 장 종료
    /// 5. Overnight - 다음 거래일 데이터 준비 시간 대기
    fn compute_next_time(&self) -> (DateTime<Local>, TimeSignal) {
        let today = self.current.date_naive();
        
        // 설정에서 시장 시간 정보 읽기
        let market_hours = if let Ok(config) = config::get_config() {
            &config.market_hours
        } else {
            // 설정을 읽을 수 없는 경우 기본값 사용
            return self.compute_next_time_fallback(today);
        };

        // 시간 문자열을 파싱하여 NaiveTime으로 변환
        let prep_time = self.parse_time_string(&market_hours.data_prep_time, today)
            .unwrap_or_else(|_| local_time!(today, 8, 30, 0));
        let open_time = self.parse_time_string(&market_hours.trading_start_time, today)
            .unwrap_or_else(|_| local_time!(today, 9, 0, 0));
        let last_upd = self.parse_time_string(&market_hours.last_update_time, today)
            .unwrap_or_else(|_| local_time!(today, 15, 29, 0));
        let close_time = self.parse_time_string(&market_hours.market_close_time, today)
            .unwrap_or_else(|_| local_time!(today, 15, 30, 0));

        if self.current < prep_time {
            (prep_time, TimeSignal::DataPrep)
        } else if self.current < open_time {
            (open_time, TimeSignal::MarketOpen)
        } else if self.current < last_upd {
            (self.add_minute(), TimeSignal::Update)
        } else if self.current < close_time {
            (close_time, TimeSignal::MarketClose)
        } else {
            // self가 &self이므로 임시로 HolidayChecker를 생성하여 사용
            let next_date = HolidayChecker::default().next_trading_day(today);
            (local_time!(next_date, 8, 30, 0), TimeSignal::Overnight)
        }
    }

    /// 설정을 읽을 수 없는 경우 사용하는 기본값 기반 계산
    fn compute_next_time_fallback(&self, today: NaiveDate) -> (DateTime<Local>, TimeSignal) {
        let prep_time = local_time!(today, 8, 30, 0);
        let open_time = local_time!(today, 9, 0, 0);
        let last_upd = local_time!(today, 15, 29, 0);
        let close_time = local_time!(today, 15, 30, 0);

        if self.current < prep_time {
            (prep_time, TimeSignal::DataPrep)
        } else if self.current < open_time {
            (open_time, TimeSignal::MarketOpen)
        } else if self.current < last_upd {
            (self.add_minute(), TimeSignal::Update)
        } else if self.current < close_time {
            (close_time, TimeSignal::MarketClose)
        } else {
            // self가 &self이므로 임시로 HolidayChecker를 생성하여 사용
            let next_date = HolidayChecker::default().next_trading_day(today);
            (local_time!(next_date, 8, 30, 0), TimeSignal::Overnight)
        }
    }

    /// HH:MM:SS 형식의 시간 문자열을 파싱하여 NaiveDateTime으로 변환
    fn parse_time_string(&self, time_str: &str, date: NaiveDate) -> StockrsResult<DateTime<Local>> {
        let time = chrono::NaiveTime::parse_from_str(time_str, "%H:%M:%S")
            .map_err(|e| {
                StockrsError::Time {
                    operation: "시간 문자열 파싱".to_string(),
                    reason: format!("시간 파싱 실패: {} - {}", time_str, e),
                }
            })?;
        
        let naive_datetime = date.and_time(time);
        Local.from_local_datetime(&naive_datetime)
            .single()
            .ok_or_else(|| {
                StockrsError::Time {
                    operation: "로컬 시간 변환".to_string(),
                    reason: format!("로컬 시간 변환 실패: {}", naive_datetime),
                }
            })
    }

    // ------------------------------------------------
    // 시간 포맷 변환 헬퍼 함수들
    // ------------------------------------------------

    /// YYYYMMDDHHMM 형식으로 변환 (분봉 DB 조회용)
    pub fn format_ymdhm(&self) -> String {
        self.current.format("%Y%m%d%H%M").to_string()
    }

    /// YYYYMMDD 형식으로 변환 (일봉 DB 조회용)
    pub fn format_ymd(&self) -> String {
        self.current.format("%Y%m%d").to_string()
    }

    /// HH:MM:SS 형식으로 변환 (로그 출력용)
    pub fn format_hms(&self) -> String {
        self.current.format("%H:%M:%S").to_string()
    }

    /// YYYY-MM-DD 형식으로 변환 (일반적인 날짜 표시용)
    pub fn format_iso_date(&self) -> String {
        self.current.format("%Y-%m-%d").to_string()
    }

    /// YYYY-MM-DD HH:MM:SS 형식으로 변환 (상세 로그용)
    pub fn format_iso_datetime(&self) -> String {
        self.current.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    /// NaiveDateTime을 YYYYMMDDHHMM 형식으로 변환 (정적 함수)
    pub fn format_naive_ymdhm(dt: &NaiveDateTime) -> String {
        dt.format("%Y%m%d%H%M").to_string()
    }

    /// NaiveDateTime을 YYYYMMDD 형식으로 변환 (정적 함수)
    pub fn format_naive_ymd(dt: &NaiveDateTime) -> String {
        dt.format("%Y%m%d").to_string()
    }

    /// NaiveDateTime을 HH:MM:SS 형식으로 변환 (정적 함수)
    pub fn format_naive_hms(dt: &NaiveDateTime) -> String {
        dt.format("%H:%M:%S").to_string()
    }

    /// DateTime<Local>을 YYYYMMDDHHMM 형식으로 변환 (정적 함수)
    pub fn format_local_ymdhm(dt: &DateTime<Local>) -> String {
        dt.format("%Y%m%d%H%M").to_string()
    }

    /// DateTime<Local>을 YYYYMMDD 형식으로 변환 (정적 함수)
    pub fn format_local_ymd(dt: &DateTime<Local>) -> String {
        dt.format("%Y%m%d").to_string()
    }

    /// DateTime<Local>을 HH:MM:SS 형식으로 변환 (정적 함수)
    pub fn format_local_hms(dt: &DateTime<Local>) -> String {
        dt.format("%H:%M:%S").to_string()
    }
}

// ------------------------------------------------
// 내부 헬퍼 함수들
// ------------------------------------------------

/// 다음 영업일(Date 부분) 계산 (주말과 공휴일 건너뛰기)
impl TimeService {
    /// 다음 거래일을 계산합니다 (공휴일과 주말을 건너뛰고)
    pub fn next_trading_day(&mut self, date: NaiveDate) -> NaiveDate {
        self.holiday_checker.next_trading_day(date)
    }

    /// 이전 거래일을 계산합니다 (공휴일과 주말을 건너뛰고)
    pub fn previous_trading_day(&mut self, date: NaiveDate) -> NaiveDate {
        self.holiday_checker.previous_trading_day(date)
    }

    /// 주어진 날짜가 거래일이 아닌지 확인합니다
    pub fn is_non_trading_day(&mut self, date: NaiveDate) -> bool {
        self.holiday_checker.is_non_trading_day(date)
    }
}

/// 생명주기 패턴 추가 - prototype.py와 동일
impl TimeService {
    /// time 시작 시 호출 - prototype.py의 self.time.on_start()
    pub fn on_start(&mut self) -> StockrsResult<()> {
        Ok(())
    }

    /// time 업데이트 - prototype.py의 self.time.update()
    pub fn update(&mut self) -> StockrsResult<()> {
        let (next_time, signal) = self.compute_next_time();
        self.current = next_time;
        self.current_signal = signal;
        
        // 시간이 변경되었으므로 캐시 업데이트
        self.update_cache();
        
        Ok(())
    }

    /// time 종료 시 호출
    pub fn on_end(&mut self) -> StockrsResult<()> {
        Ok(())
    }

    /// 공휴일/주말을 건너뛰고 다음 거래일로 이동
    pub fn skip_to_next_trading_day(&mut self) -> StockrsResult<()> {
        let current_date = self.current.date_naive();
        let next_date = self.next_trading_day(current_date);

        // 설정에서 거래 시작 시간 읽기
        let trading_start_time = if let Ok(config) = config::get_config() {
            &config.market_hours.trading_start_time
        } else {
            "09:00:00" // 기본값
        };

        // 다음 거래일의 거래 시작 시간으로 설정
        let next_datetime = self.parse_time_string(trading_start_time, next_date)?;

        self.current = next_datetime;
        self.current_signal = TimeSignal::MarketOpen;
        
        // 시간이 변경되었으므로 캐시 업데이트
        self.update_cache();

        Ok(())
    }
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
        let now = c.with_ymd_and_hms(2025, 7, 16, 7, 30, 0)
            .single()
            .expect("Invalid test datetime");
        let service = TimeService {
            current: now,
            current_signal: TimeSignal::DataPrep,
            cached_time: None,
            cache_timestamp: None,
            cache_duration: std::time::Duration::from_secs(1),
            holiday_checker: HolidayChecker::default(),
        };
        let (next, sig) = service.compute_next_time();
        assert_eq!(sig, TimeSignal::DataPrep);
        assert_eq!(next.time().hour(), 8);
        assert_eq!(next.time().minute(), 30);

        // 09:00 이후 -> 업데이트
        let now = c.with_ymd_and_hms(2025, 7, 16, 10, 0, 0)
            .single()
            .expect("Invalid test datetime");
        let service = TimeService {
            current: now,
            current_signal: TimeSignal::Update,
            cached_time: None,
            cache_timestamp: None,
            cache_duration: std::time::Duration::from_secs(1),
            holiday_checker: HolidayChecker::default(),
        };
        let (next, sig) = service.compute_next_time();
        assert_eq!(sig, TimeSignal::Update);
        assert_eq!(next.time().minute(), 1);

        // 15:30 이후 -> 다음 거래일
        let friday = c.with_ymd_and_hms(2025, 7, 18, 16, 0, 0)
            .single()
            .expect("Invalid test datetime");
        let service = TimeService {
            current: friday,
            current_signal: TimeSignal::Overnight,
            cached_time: None,
            cache_timestamp: None,
            cache_duration: std::time::Duration::from_secs(1),
            holiday_checker: HolidayChecker::default(),
        };
        let (next, sig) = service.compute_next_time();
        assert_eq!(sig, TimeSignal::Overnight);
        // 다음 거래일이 월요일인지 확인 (2025년 7월 21일은 월요일)
        assert_eq!(next.date_naive(), NaiveDate::from_ymd_opt(2025, 7, 21)
            .expect("Invalid expected date"));
    }

    #[test]
    fn test_holiday_loading() {
        let mut holiday_checker = HolidayChecker::default();
        
        // 2025년 1월 1일은 공휴일이어야 함
        let new_year = NaiveDate::from_ymd_opt(2025, 1, 1)
            .expect("Invalid test date");
        assert!(holiday_checker.is_holiday(new_year));

        // 2025년 1월 27일도 공휴일이어야 함
        let holiday = NaiveDate::from_ymd_opt(2025, 1, 27)
            .expect("Invalid test date");
        assert!(holiday_checker.is_holiday(holiday));
    }

    #[test]
    fn test_next_trading_day_with_holidays() {
        let mut holiday_checker = HolidayChecker::default();
        
        // 2025년 1월 1일(수요일, 공휴일) 다음 영업일은 1월 2일(목요일)이어야 함
        let new_year = NaiveDate::from_ymd_opt(2025, 1, 1)
            .expect("Invalid test date");
        let next_day = holiday_checker.next_trading_day(new_year);
        assert_eq!(next_day, NaiveDate::from_ymd_opt(2025, 1, 2)
            .expect("Invalid expected date"));

        // 2025년 1월 27일(월요일, 공휴일) 다음 영업일은 1월 31일(금요일)이어야 함
        // (1월 27일, 28일, 29일, 30일이 모두 공휴일이므로)
        let holiday = NaiveDate::from_ymd_opt(2025, 1, 27)
            .expect("Invalid test date");
        let next_day = holiday_checker.next_trading_day(holiday);
        assert_eq!(next_day, NaiveDate::from_ymd_opt(2025, 1, 31)
            .expect("Invalid expected date"));

        // 주말 + 공휴일 조합 테스트: 2025년 1월 25일(토요일) 다음 영업일은 1월 31일(금요일)이어야 함
        // (1월 27일, 28일, 29일, 30일이 모두 공휴일이므로)
        let saturday = NaiveDate::from_ymd_opt(2025, 1, 25)
            .expect("Invalid test date");
        let next_day = holiday_checker.next_trading_day(saturday);
        assert_eq!(next_day, NaiveDate::from_ymd_opt(2025, 1, 31)
            .expect("Invalid expected date"));
    }
}
