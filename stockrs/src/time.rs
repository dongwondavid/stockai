use crate::utility::trading_calender::TradingCalender;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::config;
use crate::local_time;
use crate::utility::types::trading::TradingMode;
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike, Datelike};
use std::thread;
use std::collections::HashSet;
use std::fs;
use once_cell::sync::Lazy;
use std::sync::Mutex;

/// 전역 TimeService 인스턴스 (싱글톤)
static TIME_SERVICE: Lazy<Mutex<Option<TimeService>>> = Lazy::new(|| Mutex::new(None));

/// Signals corresponding to specific time events within the trading day
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TimeSignal {
    /// 08:30 데이터 준비 시간
    DataPrep,
    /// 09:00 장 시작 알림
    MarketOpen,
    /// 09:01 ~ 15:29 1분 단위 업데이트
    Update,
    /// 15:20 장 종료 알림
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
    // 거래 캘린더
    trading_calender: TradingCalender,
    // 특별한 시작 날짜 집합
    special_start_dates: HashSet<String>,
    pub special_start_time_offset_minutes: i32,
}

impl TimeService {
    /// 전역 TimeService 인스턴스 초기화
    pub fn init() -> StockrsResult<()> {
        let time_service = Self::new()?;
        let mut global = TIME_SERVICE.lock().map_err(|e| {
            StockrsError::general(format!("TimeService 전역 뮤텍스 락 실패: {}", e))
        })?;
        *global = Some(time_service);
        Ok(())
    }

    /// 전역 TimeService 인스턴스 가져오기
    pub fn get() -> StockrsResult<&'static Mutex<Option<TimeService>>> {
        Ok(&TIME_SERVICE)
    }

    /// 전역 TimeService 인스턴스의 현재 시간 가져오기
    pub fn global_now() -> StockrsResult<DateTime<Local>> {
        let global = TIME_SERVICE.lock().map_err(|e| {
            StockrsError::general(format!("TimeService 전역 뮤텍스 락 실패: {}", e))
        })?;
        
        if let Some(time_service) = global.as_ref() {
            Ok(time_service.now())
        } else {
            Err(StockrsError::general("TimeService가 초기화되지 않았습니다".to_string()))
        }
    }

    /// 전역 TimeService 인스턴스의 현재 신호 가져오기
    pub fn global_now_signal() -> StockrsResult<TimeSignal> {
        let global = TIME_SERVICE.lock().map_err(|e| {
            StockrsError::general(format!("TimeService 전역 뮤텍스 락 실패: {}", e))
        })?;
        
        if let Some(time_service) = global.as_ref() {
            Ok(time_service.now_signal())
        } else {
            Err(StockrsError::general("TimeService가 초기화되지 않았습니다".to_string()))
        }
    }

    /// 전역 TimeService 인스턴스 업데이트
    pub fn global_update() -> StockrsResult<()> {
        let mut global = TIME_SERVICE.lock().map_err(|e| {
            StockrsError::general(format!("TimeService 전역 뮤텍스 락 실패: {}", e))
        })?;
        
        if let Some(time_service) = global.as_mut() {
            time_service.update()
        } else {
            Err(StockrsError::general("TimeService가 초기화되지 않았습니다".to_string()))
        }
    }

    /// 전역 TimeService 인스턴스 on_start
    pub fn global_on_start() -> StockrsResult<()> {
        let mut global = TIME_SERVICE.lock().map_err(|e| {
            StockrsError::general(format!("TimeService 전역 뮤텍스 락 실패: {}", e))
        })?;
        
        if let Some(time_service) = global.as_mut() {
            time_service.on_start()
        } else {
            Err(StockrsError::general("TimeService가 초기화되지 않았습니다".to_string()))
        }
    }

    /// 전역 TimeService 인스턴스 handle_mid_session_entry
    pub fn global_handle_mid_session_entry(trading_mode: TradingMode) -> StockrsResult<()> {
        let mut global = TIME_SERVICE.lock().map_err(|e| {
            StockrsError::general(format!("TimeService 전역 뮤텍스 락 실패: {}", e))
        })?;
        
        if let Some(time_service) = global.as_mut() {
            time_service.handle_mid_session_entry(trading_mode)
        } else {
            Err(StockrsError::general("TimeService가 초기화되지 않았습니다".to_string()))
        }
    }

    /// 전역 TimeService 인스턴스 wait_until_next_event
    pub fn global_wait_until_next_event(trading_mode: TradingMode) -> StockrsResult<()> {
        let mut global = TIME_SERVICE.lock().map_err(|e| {
            StockrsError::general(format!("TimeService 전역 뮤텍스 락 실패: {}", e))
        })?;
        
        if let Some(time_service) = global.as_mut() {
            time_service.wait_until_next_event(trading_mode)
        } else {
            Err(StockrsError::general("TimeService가 초기화되지 않았습니다".to_string()))
        }
    }

    /// 전역 TimeService 인스턴스의 시간 포맷 메서드들
    pub fn global_format_ymdhm() -> StockrsResult<String> {
        let global = TIME_SERVICE.lock().map_err(|e| {
            StockrsError::general(format!("TimeService 전역 뮤텍스 락 실패: {}", e))
        })?;
        
        if let Some(time_service) = global.as_ref() {
            Ok(time_service.format_ymdhm())
        } else {
            Err(StockrsError::general("TimeService가 초기화되지 않았습니다".to_string()))
        }
    }

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

        // 특별한 날짜 파일 로드
        let (special_start_dates, special_start_time_offset_minutes) = if let Ok(config) = config::get_config() {
            let path = &config.time_management.special_start_dates_file_path;
            let offset = config.time_management.special_start_time_offset_minutes;
            let mut set = HashSet::new();
            if let Ok(content) = fs::read_to_string(path) {
                for line in content.lines() {
                    let date = line.trim();
                    if !date.is_empty() {
                        set.insert(date.to_string());
                    }
                }
            }
            (set, offset)
        } else {
            (HashSet::new(), 0)
        };

        let mut service = TimeService {
            current: start_time?,
            current_signal: TimeSignal::DataPrep,
            cached_time: None,
            cache_timestamp: None,
            cache_duration: std::time::Duration::from_secs(1), // 1초 캐시
            trading_calender: TradingCalender::new().unwrap_or_default(),
            special_start_dates,
            special_start_time_offset_minutes,
        };
        let (next_time, signal) = service.compute_next_time()?;
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
    pub fn update_cache(&mut self) -> StockrsResult<()> {
        // 설정에서 캐시 지속 시간 읽기
        let config = config::get_config()?;
        self.cache_duration = std::time::Duration::from_secs(config.time_management.event_check_interval / 2); // 이벤트 체크 간격의 절반
        
        self.cached_time = Some(self.current);
        self.cache_timestamp = Some(std::time::Instant::now());
        Ok(())
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
    pub fn advance(&mut self) -> StockrsResult<(DateTime<Local>, TimeSignal)> {
        let (next_time, signal) = self.compute_next_time()?;
        self.current = next_time;
        self.current_signal = signal;
        
        // 시간이 변경되었으므로 캐시 업데이트
        self.update_cache()?;
        
        Ok((next_time, signal))
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
    fn compute_next_time(&self) -> StockrsResult<(DateTime<Local>, TimeSignal)> {
        let today = self.current.date_naive();
        
        // 설정에서 시장 시간 정보 읽기
        let config = config::get_config()?;
        let market_hours = &config.market_hours;

        // 시간 문자열을 파싱하여 NaiveTime으로 변환
        let prep_time = self.parse_time_string(&market_hours.data_prep_time, today)?;
        let open_time = self.parse_time_string(&market_hours.trading_start_time, today)?;
        let last_upd = self.parse_time_string(&market_hours.last_update_time, today)?;
        let close_time = self.parse_time_string(&market_hours.market_close_time, today)?;

        let result = if self.current < prep_time {
            (prep_time, TimeSignal::DataPrep)
        } else if self.current < open_time {
            (open_time, TimeSignal::MarketOpen)
        } else if self.current < last_upd {
            // 현재 시간에서 1분 후로 설정 (Update 신호)
            (self.current + Duration::minutes(1), TimeSignal::Update)
        } else if self.current < close_time {
            (close_time, TimeSignal::MarketClose)
        } else {
            // self가 &self이므로 임시로 TradingCalender를 생성하여 사용
            let next_date = TradingCalender::default().next_trading_day(today);
            (local_time!(next_date, 8, 30, 0), TimeSignal::Overnight)
        };
        
        Ok(result)
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
        let adjusted_datetime = if self.is_special_start_date(date) {
            naive_datetime + chrono::Duration::minutes(self.special_start_time_offset_minutes as i64)
        } else {
            naive_datetime
        };
        
        Local.from_local_datetime(&adjusted_datetime)
            .single()
            .ok_or_else(|| {
                StockrsError::Time {
                    operation: "로컬 시간 변환".to_string(),
                    reason: format!("로컬 시간 변환 실패: {}", adjusted_datetime),
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
    /// 다음 거래일을 계산합니다 (samsung_1min_dates.txt 기준)
    pub fn next_trading_day(&mut self, date: NaiveDate) -> NaiveDate {
        self.trading_calender.next_trading_day(date)
    }

    /// 이전 거래일을 계산합니다 (samsung_1min_dates.txt 기준)
    pub fn previous_trading_day(&mut self, date: NaiveDate) -> NaiveDate {
        self.trading_calender.previous_trading_day(date)
    }

    /// 주어진 날짜가 거래일이 아닌지 확인합니다 (samsung_1min_dates.txt 기준)
    pub fn is_non_trading_day(&mut self, date: NaiveDate) -> bool {
        self.trading_calender.is_non_trading_day(date)
    }

    /// 현재 시간이 다음 거래일로 건너뛰어야 하는지 확인합니다
    /// Overnight 신호는 이미 TimeService에서 다음 거래일로 이동했으므로 제외
    pub fn should_skip_to_next_trading_day(&mut self) -> bool {
        let current_date = self.current.date_naive();
        let is_non_trading = self.is_non_trading_day(current_date);
        let is_overnight = self.current_signal == TimeSignal::Overnight;
        
        is_non_trading && !is_overnight
    }

    pub fn is_special_start_date(&self, date: NaiveDate) -> bool {
        let ymd = date.format("%Y%m%d").to_string();
        self.special_start_dates.contains(&ymd)
    }
}

/// 생명주기 패턴 추가 - prototype.py와 동일
impl TimeService {
    /// time 시작 시 호출 - 백테스팅 초기화
    pub fn on_start(&mut self) -> StockrsResult<()> {
        // 백테스팅 시작 시 첫 번째 이벤트(08:30 DataPrep)로 설정
        let config = config::get_config()?;
        let market_hours = &config.market_hours;
        
        let today = self.current.date_naive();
        let prep_time = self.parse_time_string(&market_hours.data_prep_time, today)?;
        
        // 현재 시간을 08:30으로 설정하고 DataPrep 신호로 시작
        self.current = prep_time;
        self.current_signal = TimeSignal::DataPrep;
        
        // 시간이 변경되었으므로 캐시 업데이트
        self.update_cache()?;
        
        println!(
            "🕐 [Time] 시작 초기화 - 초기 시간: {}, 신호: {:?}",
            self.current.format("%Y-%m-%d %H:%M:%S"),
            self.current_signal
        );
        
        Ok(())
    }

    /// time 업데이트 - prototype.py의 self.time.update()
    pub fn update(&mut self) -> StockrsResult<()> {
        let (next_time, signal) = self.compute_next_time()?;
        self.current = next_time;
        self.current_signal = signal;
        
        // 시간이 변경되었으므로 캐시 업데이트
        self.update_cache()?;
        
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
        let config = config::get_config()?;
        let trading_start_time = &config.market_hours.trading_start_time;

        // 다음 거래일의 거래 시작 시간으로 설정
        let next_datetime = self.parse_time_string(trading_start_time, next_date)?;

        self.current = next_datetime;
        self.current_signal = TimeSignal::MarketOpen;
        
        // 시간이 변경되었으므로 캐시 업데이트
        self.update_cache()?;

        Ok(())
    }

    /// 모드별 대기 로직 - 다음 이벤트까지 대기
    pub fn wait_until_next_event(&mut self, trading_mode: TradingMode) -> StockrsResult<()> {
        match trading_mode {
            TradingMode::Backtest => {
                // 백테스팅: 현재 시간을 다음 이벤트로 업데이트
                self.update()?;
                
                println!(
                    "⏰ [Time] 백테스팅 다음 이벤트 - 시간: {}, 신호: {:?}",
                    self.current.format("%Y-%m-%d %H:%M:%S"),
                    self.current_signal
                );
                
                Ok(())
            }
            TradingMode::Real | TradingMode::Paper => {
                // 실거래/모의투자: 현 시각 기준으로 다음 이벤트 시각을 계산하고 해당 시각까지 대기
                let now = Local::now();

                // 오늘의 경계 시각 계산
                let config = config::get_config()?;
                let market_hours = &config.market_hours;
                let today = now.date_naive();
                let prep_time = self.parse_time_string(&market_hours.data_prep_time, today)?;
                let open_time = self.parse_time_string(&market_hours.trading_start_time, today)?;
                let last_upd = self.parse_time_string(&market_hours.last_update_time, today)?;
                let close_time = self.parse_time_string(&market_hours.market_close_time, today)?;

                // 다음 이벤트 목표 시각과 신호 결정
                let (target, signal) = if now < prep_time {
                    (prep_time, TimeSignal::DataPrep)
                } else if now < open_time {
                    (open_time, TimeSignal::MarketOpen)
                } else if now < last_upd {
                    // 분 정렬: 다음 분의 00초로 정렬하되, last_upd를 넘지 않도록 제한
                    let next_minute_base = now + Duration::minutes(1);
                    let rounded = Local
                        .with_ymd_and_hms(
                            next_minute_base.year(),
                            next_minute_base.month(),
                            next_minute_base.day(),
                            next_minute_base.hour(),
                            next_minute_base.minute(),
                            0,
                        )
                        .single()
                        .ok_or_else(|| {
                            StockrsError::Time {
                                operation: "분 정렬".to_string(),
                                reason: "로컬 시간 변환 실패".to_string(),
                            }
                        })?;
                    (std::cmp::min(rounded, last_upd), TimeSignal::Update)
                } else if now < close_time {
                    (close_time, TimeSignal::MarketClose)
                } else {
                    // 다음 거래일 08:30 (DataPrep)까지 대기
                    let next_date = TradingCalender::default().next_trading_day(today);
                    (local_time!(next_date, 8, 30, 0), TimeSignal::Overnight)
                };

                // 내부 상태 업데이트 및 대기
                self.current = target;
                self.current_signal = signal;
                self.update_cache()?;

                self.wait_until(target);
                println!(
                    "⏰ [Time][실시간] 다음 이벤트: {:?}, 타겟 시각: {}",
                    self.current_signal,
                    target.format("%Y-%m-%d %H:%M:%S")
                );
                Ok(())
            }
        }
    }

    /// 모드별 대기 로직 - 다음 거래일로 이동해야 하는 상황 처리
    pub fn handle_next_trading_day(&mut self, trading_mode: TradingMode) -> StockrsResult<()> {
        // 다음 거래일로 이동해야 하는지 확인
        if self.should_skip_to_next_trading_day() {
            match trading_mode {
                TradingMode::Backtest => {
                    // 백테스팅: 즉시 진행
                    self.skip_to_next_trading_day()
                }
                TradingMode::Real | TradingMode::Paper => {
                    // 실거래/모의투자: 실제 대기
                    self.wait_until(self.now());
                    Ok(())
                }
            }
        } else {
            Ok(())
        }
    }

    /// 모드별 대기 로직 - Overnight 신호 처리
    pub fn handle_overnight_signal(&mut self, trading_mode: TradingMode) -> StockrsResult<()> {
        if self.current_signal == TimeSignal::Overnight {
            match trading_mode {
                TradingMode::Backtest => {
                    // 백테스팅: 즉시 진행 (다음 거래일로 이동)
                    self.skip_to_next_trading_day()
                }
                TradingMode::Real | TradingMode::Paper => {
                    // 실거래/모의투자: 실제 대기
                    self.wait_until(self.now());
                    Ok(())
                }
            }
        } else {
            Ok(())
        }
    }

    /// 장 중간 진입 처리 - 모의투자/실거래에서 장 중간에 시작할 때
    pub fn handle_mid_session_entry(&mut self, trading_mode: TradingMode) -> StockrsResult<()> {
        match trading_mode {
            TradingMode::Backtest => {
                // 백테스팅에서는 장 중간 진입이 의미없음 (항상 08:30부터 시작)
                Ok(())
            }
            TradingMode::Real | TradingMode::Paper => {
                // 장 중간 진입 시점은 실제 현재 시각 기준으로 판정
                let current_time = Local::now();
                let config = config::get_config()?;
                let market_hours = &config.market_hours;

                let today = current_time.date_naive();
                let prep_time = self.parse_time_string(&market_hours.data_prep_time, today)?;
                let open_time = self.parse_time_string(&market_hours.trading_start_time, today)?;
                let last_upd = self.parse_time_string(&market_hours.last_update_time, today)?;
                let close_time = self.parse_time_string(&market_hours.market_close_time, today)?;

                // 현재 시간에 맞는 신호 설정 (데이터 준비 시간 포함)
                if current_time < prep_time {
                    self.current_signal = TimeSignal::Overnight;
                } else if current_time < open_time {
                    self.current_signal = TimeSignal::DataPrep;
                } else if current_time < last_upd {
                    self.current_signal = TimeSignal::Update;
                } else if current_time < close_time {
                    self.current_signal = TimeSignal::MarketClose;
                } else {
                    self.current_signal = TimeSignal::Overnight;
                }

                // 내부 현재 시각도 실제 현재 시각으로 맞춰 캐시 업데이트
                self.current = current_time;
                self.update_cache()?;

                println!(
                    "🟢 [Time][실시간] 장 중간 진입 - 현재 시각: {}, 신호: {:?}",
                    current_time.format("%H:%M:%S"),
                    self.current_signal
                );

                Ok(())
            }
        }
    }
}
