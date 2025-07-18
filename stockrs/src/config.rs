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
    pub time_management: TimeManagementConfig,
    pub logging: LoggingConfig,
    pub risk_management: RiskManagementConfig,
    pub model_prediction: ModelPredictionConfig,
    pub performance: PerformanceConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseConfig {
    pub stock_db_path: String,
    pub daily_db_path: String,
    pub trading_db_path: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OnnxModelConfig {
    pub model_info_path: String,
    pub model_file_path: String,
    pub features_file_path: String,
    pub extra_stocks_file_path: String,
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
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TradingConfig {
    pub default_mode: String,
    pub max_positions: u32,
    pub max_position_amount: u64,
    pub min_order_amount: u64,
    pub stop_loss_ratio: f64,
    pub take_profit_ratio: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TimeManagementConfig {
    pub market_close_file_path: String,
    pub trading_start_time: String,
    pub trading_end_time: String,
    pub event_check_interval: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: Option<String>,
    pub max_file_size: u64,
    pub max_files: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RiskManagementConfig {
    pub daily_max_loss: u64,
    pub max_investment_ratio: f64,
    pub max_single_stock_ratio: f64,
    pub var_confidence_level: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelPredictionConfig {
    pub buy_threshold: f64,
    pub sell_threshold: f64,
    pub top_volume_stocks: u32,
    pub normalize_features: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PerformanceConfig {
    pub db_pool_size: u32,
    pub api_rate_limit: u32,
    pub worker_threads: u32,
    pub cache_size_mb: u64,
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
        
        // ONNX 모델 경로
        if let Ok(path) = std::env::var("ONNX_MODEL_INFO_PATH") {
            self.onnx_model.model_info_path = path;
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
        
        // 로그 레벨
        if let Ok(level) = std::env::var("RUST_LOG") {
            self.logging.level = level;
        }
    }

    /// 설정 유효성 검증
    fn validate(&self) -> Result<(), ConfigError> {
        // 거래 모드 검증
        match self.trading.default_mode.as_str() {
            "real" | "paper" | "backtest" => {},
            _ => return Err(ConfigError::ValidationError(
                "default_mode는 'real', 'paper', 'backtest' 중 하나여야 합니다".to_string()
            )),
        }

        // 비율 검증
        if self.trading.stop_loss_ratio <= 0.0 || self.trading.stop_loss_ratio > 100.0 {
            return Err(ConfigError::ValidationError(
                "stop_loss_ratio는 0~100 사이여야 합니다".to_string()
            ));
        }

        if self.risk_management.max_investment_ratio <= 0.0 || self.risk_management.max_investment_ratio > 100.0 {
            return Err(ConfigError::ValidationError(
                "max_investment_ratio는 0~100 사이여야 합니다".to_string()
            ));
        }

        // 로그 레벨 검증
        match self.logging.level.to_lowercase().as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => {},
            _ => return Err(ConfigError::ValidationError(
                "log level은 'error', 'warn', 'info', 'debug', 'trace' 중 하나여야 합니다".to_string()
            )),
        }

        // API 키 검증 (실제 거래 모드일 때)
        if self.trading.default_mode == "real" {
            if self.korea_investment_api.real_app_key.contains("YOUR_") || 
               self.korea_investment_api.real_app_secret.contains("YOUR_") ||
               self.korea_investment_api.real_account_number.contains("YOUR_") {
                return Err(ConfigError::ValidationError(
                    "실제 거래 모드에서는 유효한 실제 거래 API 키가 필요합니다".to_string()
                ));
            }
        }
        
        // API 키 검증 (모의투자 모드일 때)
        if self.trading.default_mode == "paper" {
            if self.korea_investment_api.paper_app_key.contains("YOUR_") || 
               self.korea_investment_api.paper_app_secret.contains("YOUR_") ||
               self.korea_investment_api.paper_account_number.contains("YOUR_") {
                return Err(ConfigError::ValidationError(
                    "모의투자 모드에서는 유효한 모의투자 API 키가 필요합니다".to_string()
                ));
            }
        }

        Ok(())
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
    let config_option = GLOBAL_CONFIG.get_or_init(|| {
        match Config::load() {
            Ok(config) => Some(config),
            Err(e) => {
                eprintln!("설정 로드 실패: {}", e);
                eprintln!("config.example.toml을 config.toml로 복사하고 설정을 채워주세요.");
                None
            }
        }
    });
    
    config_option.as_ref().ok_or_else(|| ConfigError::FileNotFound(
        "설정을 로드할 수 없습니다".to_string()
    ))
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
                trading_db_path: "test.db".to_string(),
            },
            trading: TradingConfig {
                default_mode: "backtest".to_string(),
                max_positions: 5,
                max_position_amount: 1000000,
                min_order_amount: 10000,
                stop_loss_ratio: 3.0,
                take_profit_ratio: 5.0,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file_path: None,
                max_file_size: 10,
                max_files: 5,
            },
            // ... 다른 필드들은 테스트용으로 기본값 설정
            onnx_model: OnnxModelConfig {
                model_info_path: "test".to_string(),
                model_file_path: "test".to_string(),
                features_file_path: "test".to_string(),
                extra_stocks_file_path: "test".to_string(),
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
            },
            time_management: TimeManagementConfig {
                market_close_file_path: "test".to_string(),
                trading_start_time: "09:00:00".to_string(),
                trading_end_time: "15:20:00".to_string(),
                event_check_interval: 30,
            },
            risk_management: RiskManagementConfig {
                daily_max_loss: 100000,
                max_investment_ratio: 80.0,
                max_single_stock_ratio: 20.0,
                var_confidence_level: 95.0,
            },
            model_prediction: ModelPredictionConfig {
                buy_threshold: 0.6,
                sell_threshold: 0.4,
                top_volume_stocks: 30,
                normalize_features: true,
            },
            performance: PerformanceConfig {
                db_pool_size: 10,
                api_rate_limit: 20,
                worker_threads: 0,
                cache_size_mb: 100,
            },
        };

        // 정상적인 설정
        assert!(config.validate().is_ok());

        // 잘못된 거래 모드
        config.trading.default_mode = "invalid".to_string();
        assert!(config.validate().is_err());
    }
} 