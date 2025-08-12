use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("설정 파일을 찾을 수 없습니다: {0}")]
    FileNotFound(String),
    #[error("설정 파일 읽기 오류: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("설정 파일 파싱 오류: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("설정 유효성 검증 실패: {0}")]
    ValidationError(String),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub database: DatabaseConfig,
    pub onnx_model: OnnxModelConfig,
    pub korea_investment_api: KoreaInvestmentApiConfig,
    pub trading: TradingConfig,
    pub backtest: BacktestConfig,
    pub joonwoo: JoonwooConfig,
    #[serde(default)]
    pub dongwon: DongwonConfig,
    pub time_management: TimeManagementConfig,
    pub market_hours: MarketHoursConfig,
    pub logging: LoggingConfig,
    pub token_management: TokenManagementConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseConfig {
    pub stock_db_path: String,
    pub daily_db_path: String,
    pub minute_db_path: String,
    pub trading_db_path: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OnnxModelConfig {
    pub model_file_path: String,
    pub features_file_path: String,
    pub included_stocks_file_path: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KoreaInvestmentApiConfig {
    // 실제 거래 API 설정
    pub real_app_key: String,
    pub real_app_secret: String,
    pub real_base_url: String,
    pub real_account_number: String,
    pub real_account_product_code: String,

    // 모의투자 API 설정
    pub paper_app_key: String,
    pub paper_app_secret: String,
    pub paper_base_url: String,
    pub paper_account_number: String,
    pub paper_account_product_code: String,

    // 정보용 실전 API 설정 (시세 조회 등 정보 취득용)
    pub info_app_key: String,
    pub info_app_secret: String,
    pub info_base_url: String,
    pub info_account_number: String,
    pub info_account_product_code: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TradingConfig {
    pub default_mode: String,
    pub max_positions: u32,
    pub max_position_amount: u64,
    pub stop_loss_ratio: f64,
    pub initial_capital: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BacktestConfig {
    pub buy_fee_rate: f64,
    pub sell_fee_rate: f64,
    pub buy_slippage_rate: f64,
    pub sell_slippage_rate: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JoonwooConfig {
    pub stop_loss_pct: f64,
    pub take_profit_pct: f64,
    pub trailing_stop_pct: f64,
    pub entry_time: String,
    pub force_close_time: String,
    pub entry_asset_ratio: f64,
    pub fixed_entry_amount: f64,  // 고정 매수 금액 (원)
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct DongwonConfig {
    pub stockcode: String,
    pub entry_time: String, // HH:MM:SS
    pub exit_time: String,  // HH:MM:SS
    pub quantity: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TimeManagementConfig {
    pub trading_dates_file_path: String,

    pub event_check_interval: u64,
    // trading_dates_file_path에서 자동으로 시작/종료 날짜 설정
    pub auto_set_dates_from_file: bool,
    pub start_date: String,
    pub end_date: String,
    // 특별한 시작 시간이 적용되는 날짜 파일 경로
    pub special_start_dates_file_path: String,
    // 특별한 날짜들의 시간 오프셋 (분 단위, 양수는 늦춤, 음수는 앞당김)
    pub special_start_time_offset_minutes: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MarketHoursConfig {
    pub data_prep_time: String,
    pub trading_start_time: String,
    pub trading_end_time: String,
    pub last_update_time: String,
    pub market_close_time: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TokenManagementConfig {
    /// 토큰 저장 파일 경로 (JSON 형식)
    pub token_file_path: String,
    /// 토큰 자동 갱신 여부
    pub auto_refresh_tokens: bool,
    /// 토큰 만료 전 갱신 시간 (분)
    pub refresh_before_expiry_minutes: u32,
    /// 토큰 만료 시 자동 삭제 여부
    pub auto_cleanup_expired_tokens: bool,
    /// 토큰 파일 백업 여부
    pub backup_token_file: bool,
    /// 토큰 파일 백업 경로
    pub backup_token_file_path: String,
}

impl Default for TokenManagementConfig {
    fn default() -> Self {
        Self {
            token_file_path: "tokens.json".to_string(),
            auto_refresh_tokens: true,
            refresh_before_expiry_minutes: 360, // 6시간
            auto_cleanup_expired_tokens: true,
            backup_token_file: true,
            backup_token_file_path: "tokens_backup.json".to_string(),
        }
    }
}

impl Config {
    /// config.toml 파일에서 설정을 로드
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from_file("config.toml")
    }

    /// 지정된 파일에서 설정을 로드
    pub fn load_from_file(path: &str) -> Result<Self, ConfigError> {
        if !Path::new(path).exists() {
            return Err(ConfigError::FileNotFound(format!(
                "{}가 없습니다. config.example.toml을 복사해서 config.toml을 만들고 설정을 채워주세요.", 
                path
            )));
        }

        let content = fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;

        // 환경 변수로 오버라이드
        config.apply_env_overrides();

        // ===== 자동 날짜 설정 기능 구현 =====
        if config.time_management.auto_set_dates_from_file {
            let file_path = &config.time_management.trading_dates_file_path;
            let path = Path::new(file_path);
            if !path.exists() {
                return Err(ConfigError::ValidationError(format!(
                    "trading_dates_file_path 파일이 존재하지 않습니다: {}", file_path
                )));
            }
            let content = fs::read_to_string(path).map_err(|e| {
                ConfigError::ValidationError(format!(
                    "trading_dates_file_path 파일 읽기 실패: {}", e
                ))
            })?;
            let mut dates: Vec<chrono::NaiveDate> = content
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        chrono::NaiveDate::parse_from_str(trimmed, "%Y%m%d").ok()
                    } else {
                        None
                    }
                })
                .collect();
            if dates.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "trading_dates_file_path 파일이 비어있거나 파싱할 수 없습니다: {}", file_path
                )));
            }
            dates.sort_unstable();
            config.time_management.start_date = dates.first().unwrap().format("%Y%m%d").to_string();
            config.time_management.end_date = dates.last().unwrap().format("%Y%m%d").to_string();
        }
        // ===== 자동 날짜 설정 기능 끝 =====

        // 설정 유효성 검증
        config.validate()?;

        Ok(config)
    }

    /// 환경 변수로 설정을 오버라이드
    fn apply_env_overrides(&mut self) {
        // 데이터베이스 경로들
        if let Ok(path) = std::env::var("STOCK_DB_PATH") {
            self.database.stock_db_path = path;
        }
        if let Ok(path) = std::env::var("DAILY_DB_PATH") {
            self.database.daily_db_path = path;
        }
        if let Ok(path) = std::env::var("MINUTE_DB_PATH") {
            self.database.minute_db_path = path;
        }

        // ONNX 모델 경로
        if let Ok(path) = std::env::var("ONNX_MODEL_FILE_PATH") {
            self.onnx_model.model_file_path = path;
        }

        // 실제 거래 API 키들 (보안상 환경변수 우선)
        if let Ok(key) = std::env::var("KOREA_INVESTMENT_REAL_APP_KEY") {
            self.korea_investment_api.real_app_key = key;
        }
        if let Ok(secret) = std::env::var("KOREA_INVESTMENT_REAL_APP_SECRET") {
            self.korea_investment_api.real_app_secret = secret;
        }
        if let Ok(account) = std::env::var("KOREA_INVESTMENT_REAL_ACCOUNT_NUMBER") {
            self.korea_investment_api.real_account_number = account;
        }

        // 모의투자 API 키들
        if let Ok(key) = std::env::var("KOREA_INVESTMENT_PAPER_APP_KEY") {
            self.korea_investment_api.paper_app_key = key;
        }
        if let Ok(secret) = std::env::var("KOREA_INVESTMENT_PAPER_APP_SECRET") {
            self.korea_investment_api.paper_app_secret = secret;
        }
        if let Ok(account) = std::env::var("KOREA_INVESTMENT_PAPER_ACCOUNT_NUMBER") {
            self.korea_investment_api.paper_account_number = account;
        }

        // 정보용 실전 API 키들
        if let Ok(key) = std::env::var("KOREA_INVESTMENT_INFO_APP_KEY") {
            self.korea_investment_api.info_app_key = key;
        }
        if let Ok(secret) = std::env::var("KOREA_INVESTMENT_INFO_APP_SECRET") {
            self.korea_investment_api.info_app_secret = secret;
        }
        if let Ok(account) = std::env::var("KOREA_INVESTMENT_INFO_ACCOUNT_NUMBER") {
            self.korea_investment_api.info_account_number = account;
        }

        // 로그 레벨
        if let Ok(level) = std::env::var("RUST_LOG") {
            self.logging.level = level;
        }

        // 특별한 시작 시간 설정
        if let Ok(path) = std::env::var("SPECIAL_START_DATES_FILE_PATH") {
            self.time_management.special_start_dates_file_path = path;
        }
        if let Ok(offset) = std::env::var("SPECIAL_START_TIME_OFFSET_MINUTES") {
            if let Ok(offset_value) = offset.parse::<i32>() {
                self.time_management.special_start_time_offset_minutes = offset_value;
            }
        }
    }

    /// 설정 유효성 검증
    fn validate(&self) -> Result<(), ConfigError> {
        // 거래 모드 검증
        match self.trading.default_mode.as_str() {
            "real" | "paper" | "backtest" => {}
            _ => {
                return Err(ConfigError::ValidationError(
                    "default_mode는 'real', 'paper', 'backtest' 중 하나여야 합니다".to_string(),
                ))
            }
        }

        // 비율 검증
        if self.trading.stop_loss_ratio <= 0.0 || self.trading.stop_loss_ratio > 100.0 {
            return Err(ConfigError::ValidationError(
                "stop_loss_ratio는 0~100 사이여야 합니다".to_string(),
            ));
        }

        // 백테스팅 설정 검증
        if self.backtest.buy_fee_rate < 0.0 || self.backtest.buy_fee_rate > 10.0 {
            return Err(ConfigError::ValidationError(
                "buy_fee_rate는 0~10 사이여야 합니다".to_string(),
            ));
        }
        if self.backtest.sell_fee_rate < 0.0 || self.backtest.sell_fee_rate > 10.0 {
            return Err(ConfigError::ValidationError(
                "sell_fee_rate는 0~10 사이여야 합니다".to_string(),
            ));
        }
        if self.backtest.buy_slippage_rate < 0.0 || self.backtest.buy_slippage_rate > 10.0 {
            return Err(ConfigError::ValidationError(
                "buy_slippage_rate는 0~10 사이여야 합니다".to_string(),
            ));
        }
        if self.backtest.sell_slippage_rate < 0.0 || self.backtest.sell_slippage_rate > 10.0 {
            return Err(ConfigError::ValidationError(
                "sell_slippage_rate는 0~10 사이여야 합니다".to_string(),
            ));
        }

        // Joonwoo 모델 설정 검증
        if self.joonwoo.stop_loss_pct <= 0.0 || self.joonwoo.stop_loss_pct > 100.0 {
            return Err(ConfigError::ValidationError(
                "joonwoo.stop_loss_pct는 0~100 사이여야 합니다".to_string(),
            ));
        }
        if self.joonwoo.take_profit_pct <= 0.0 || self.joonwoo.take_profit_pct > 100.0 {
            return Err(ConfigError::ValidationError(
                "joonwoo.take_profit_pct는 0~100 사이여야 합니다".to_string(),
            ));
        }
        if self.joonwoo.trailing_stop_pct <= 0.0 || self.joonwoo.trailing_stop_pct > 100.0 {
            return Err(ConfigError::ValidationError(
                "joonwoo.trailing_stop_pct는 0~100 사이여야 합니다".to_string(),
            ));
        }
        if self.joonwoo.entry_asset_ratio <= 0.0 || self.joonwoo.entry_asset_ratio > 100.0 {
            return Err(ConfigError::ValidationError(
                "joonwoo.entry_asset_ratio는 0~100 사이여야 합니다".to_string(),
            ));
        }
        if self.joonwoo.fixed_entry_amount < 0.0 {
            return Err(ConfigError::ValidationError(
                "joonwoo.fixed_entry_amount는 0 이상이어야 합니다".to_string(),
            ));
        }

        // 시간 형식 검증 (HH:MM:SS)
        if !self.is_valid_time_format(&self.joonwoo.entry_time) {
            return Err(ConfigError::ValidationError(
                "joonwoo.entry_time는 HH:MM:SS 형식이어야 합니다".to_string(),
            ));
        }
        if !self.is_valid_time_format(&self.joonwoo.force_close_time) {
            return Err(ConfigError::ValidationError(
                "joonwoo.force_close_time는 HH:MM:SS 형식이어야 합니다".to_string(),
            ));
        }

        // 로그 레벨 검증
        match self.logging.level.to_lowercase().as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => {}
            _ => {
                return Err(ConfigError::ValidationError(
                    "log level은 'error', 'warn', 'info', 'debug', 'trace' 중 하나여야 합니다"
                        .to_string(),
                ))
            }
        }

        // 특별한 시작 시간 설정 검증
        if self.time_management.special_start_time_offset_minutes < -1440 || self.time_management.special_start_time_offset_minutes > 1440 {
            return Err(ConfigError::ValidationError(
                "special_start_time_offset_minutes는 -1440~1440 사이여야 합니다 (24시간 범위)".to_string(),
            ));
        }

        // 자동 날짜 설정 검증
        if !self.time_management.auto_set_dates_from_file {
            // auto_set_dates_from_file이 false일 때만 start_date, end_date 형식 검증
            if !self.is_valid_date_format(&self.time_management.start_date) {
                return Err(ConfigError::ValidationError(
                    "start_date는 YYYYMMDD 형식이어야 합니다".to_string(),
                ));
            }
            if !self.is_valid_date_format(&self.time_management.end_date) {
                return Err(ConfigError::ValidationError(
                    "end_date는 YYYYMMDD 형식이어야 합니다".to_string(),
                ));
            }
        }

        // API 키 검증 (실제 거래 모드일 때)
        if self.trading.default_mode == "real"
            && (self.korea_investment_api.real_app_key.contains("YOUR_")
                || self.korea_investment_api.real_app_secret.contains("YOUR_")
                || self
                    .korea_investment_api
                    .real_account_number
                    .contains("YOUR_"))
        {
            return Err(ConfigError::ValidationError(
                "실제 거래 모드에서는 유효한 실제 거래 API 키가 필요합니다".to_string(),
            ));
        }

        // API 키 검증 (모의투자 모드일 때)
        if self.trading.default_mode == "paper"
            && (self.korea_investment_api.paper_app_key.contains("YOUR_")
                || self.korea_investment_api.paper_app_secret.contains("YOUR_")
                || self
                    .korea_investment_api
                    .paper_account_number
                    .contains("YOUR_"))
        {
            return Err(ConfigError::ValidationError(
                "모의투자 모드에서는 유효한 모의투자 API 키가 필요합니다".to_string(),
            ));
        }

        Ok(())
    }

    /// 시간 형식 검증 헬퍼 함수
    fn is_valid_time_format(&self, time_str: &str) -> bool {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 3 {
            return false;
        }

        if let (Ok(hour), Ok(minute), Ok(second)) = (
            parts[0].parse::<u32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        ) {
            hour < 24 && minute < 60 && second < 60
        } else {
            false
        }
    }

    /// 날짜 형식 검증 헬퍼 함수 (YYYYMMDD)
    fn is_valid_date_format(&self, date_str: &str) -> bool {
        if date_str.len() != 8 {
            return false;
        }

        if let (Ok(year), Ok(month), Ok(day)) = (
            date_str[0..4].parse::<u32>(),
            date_str[4..6].parse::<u32>(),
            date_str[6..8].parse::<u32>(),
        ) {
            // 기본적인 날짜 범위 검증
            year >= 1900 && year <= 2100 && month >= 1 && month <= 12 && day >= 1 && day <= 31
        } else {
            false
        }
    }

    /// 설정을 파일로 저장 (주로 디버깅용)
    pub fn save_to_file(&self, path: &str) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::ValidationError(format!("직렬화 오류: {}", e)))?;
        fs::write(path, content)?;
        Ok(())
    }
}

/// 글로벌 설정 인스턴스 (한 번만 로드)
static GLOBAL_CONFIG: std::sync::OnceLock<Option<Config>> = std::sync::OnceLock::new();

/// 글로벌 설정 인스턴스를 가져오기
pub fn get_config() -> Result<&'static Config, ConfigError> {
    let config_option = GLOBAL_CONFIG.get_or_init(|| match Config::load() {
        Ok(config) => Some(config),
        Err(e) => {
            eprintln!("설정 로드 실패: {}", e);
            eprintln!("config.example.toml을 config.toml로 복사하고 설정을 채워주세요.");
            None
        }
    });

    config_option
        .as_ref()
        .ok_or_else(|| ConfigError::FileNotFound("설정을 로드할 수 없습니다".to_string()))
}

/// 전역 설정을 설정 (main.rs에서 사용)
pub fn set_global_config(config: Config) -> Result<(), ConfigError> {
    GLOBAL_CONFIG
        .set(Some(config))
        .map_err(|_| ConfigError::ValidationError("전역 설정이 이미 초기화되어 있습니다".to_string()))
}

/// 설정을 강제로 다시 로드 (주로 테스트용)
/// 주의: 이 함수는 이미 초기화된 글로벌 설정을 변경할 수 없습니다.
/// 테스트 환경에서는 새로운 프로세스를 시작하는 것이 권장됩니다.
pub fn reload_config() -> Result<(), ConfigError> {
    // OnceLock은 한 번만 설정할 수 있으므로, 이미 설정된 경우 경고만 출력
    if GLOBAL_CONFIG.get().is_some() {
        eprintln!("경고: 글로벌 설정이 이미 초기화되어 다시 로드할 수 없습니다.");
        eprintln!("새로운 설정을 적용하려면 프로그램을 재시작하세요.");
        return Ok(());
    }

    // 아직 초기화되지 않은 경우에만 로드 시도
    let _ = get_config()?;
    Ok(())
}
