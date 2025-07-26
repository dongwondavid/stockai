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
    pub time_management: TimeManagementConfig,
    pub market_hours: MarketHoursConfig,
    pub logging: LoggingConfig,
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
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TimeManagementConfig {
    pub trading_dates_file_path: String,
    pub market_close_file_path: String,
    pub event_check_interval: u64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = Config {
            database: DatabaseConfig {
                stock_db_path: "test.db".to_string(),
                daily_db_path: "test.db".to_string(),
                minute_db_path: "test.db".to_string(),
                trading_db_path: "test.db".to_string(),
            },
            trading: TradingConfig {
                default_mode: "backtest".to_string(),
                max_positions: 5,
                max_position_amount: 1000000,
                stop_loss_ratio: 3.0,
                initial_capital: 1000000.0,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
            },
            // ... 다른 필드들은 테스트용으로 기본값 설정
            onnx_model: OnnxModelConfig {
                model_file_path: "test".to_string(),
                features_file_path: "test".to_string(),
                included_stocks_file_path: "test".to_string(),
            },
            korea_investment_api: KoreaInvestmentApiConfig {
                real_app_key: "test".to_string(),
                real_app_secret: "test".to_string(),
                real_base_url: "test".to_string(),
                real_account_number: "test".to_string(),
                real_account_product_code: "01".to_string(),
                paper_app_key: "test".to_string(),
                paper_app_secret: "test".to_string(),
                paper_base_url: "test".to_string(),
                paper_account_number: "test".to_string(),
                paper_account_product_code: "01".to_string(),
                info_app_key: "test".to_string(),
                info_app_secret: "test".to_string(),
                info_base_url: "test".to_string(),
                info_account_number: "test".to_string(),
                info_account_product_code: "01".to_string(),
            },
            time_management: TimeManagementConfig {
                trading_dates_file_path: "test".to_string(),
                market_close_file_path: "test".to_string(),
                event_check_interval: 30,
                start_date: "20241201".to_string(),
                end_date: "20241231".to_string(),
                special_start_dates_file_path: "data/start1000.txt".to_string(),
                special_start_time_offset_minutes: 60,
            },
            market_hours: MarketHoursConfig {
                data_prep_time: "08:30:00".to_string(),
                trading_start_time: "09:00:00".to_string(),
                trading_end_time: "15:20:00".to_string(),
                last_update_time: "15:29:00".to_string(),
                market_close_time: "15:30:00".to_string(),
            },
            joonwoo: JoonwooConfig {
                stop_loss_pct: 1.0,
                take_profit_pct: 2.0,
                trailing_stop_pct: 0.7,
                entry_time: "09:30:00".to_string(),
                force_close_time: "12:00:00".to_string(),
                entry_asset_ratio: 90.0,
            },
            backtest: BacktestConfig {
                buy_fee_rate: 0.015,
                sell_fee_rate: 0.015,
                buy_slippage_rate: 0.05,
                sell_slippage_rate: 0.05,
            },
        };

        // 정상적인 설정
        assert!(config.validate().is_ok());

        // 잘못된 거래 모드
        config.trading.default_mode = "invalid".to_string();
        assert!(config.validate().is_err());
    }
}
