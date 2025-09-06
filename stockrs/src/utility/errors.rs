use thiserror::Error;

/// StockAI 시스템의 모든 오류 타입을 정의하는 enum
/// 각 오류는 구체적인 컨텍스트 정보를 포함하여 디버깅과 사용자 경험을 개선
#[derive(Error, Debug)]
pub enum StockrsError {
    /// API 관련 오류
    #[error("API 오류: {message}")]
    Api { message: String },

    /// 한국투자증권 API 특화 오류
    #[error("한국투자증권 API 오류: {operation} 실패 - {reason}")]
    KoreaInvestmentApi { operation: String, reason: String },

    /// 종목 데이터 관련 오류
    #[error("종목 데이터 없음: {stockcode} (기간: {days}일)")]
    NoStockData { stockcode: String, days: i32 },

    /// 주문 실행 관련 오류
    #[error("주문 실행 실패: {order_type} {stockcode} {quantity}주 - {reason}")]
    OrderExecution {
        order_type: String,
        stockcode: String,
        quantity: u32,
        reason: String,
    },

    /// 주문 체결 확인 오류
    #[error("주문 체결 확인 실패: 주문번호 {order_id} - {reason}")]
    OrderFillCheck { order_id: String, reason: String },

    /// 잔고 조회 오류
    #[error("잔고 조회 실패: {reason}")]
    BalanceInquiry { reason: String },

    /// 가격 조회 오류
    #[error("가격 조회 실패: {stockcode} ({price_type}) - {reason}")]
    PriceInquiry {
        stockcode: String,
        price_type: String, // "현재가", "평균가" 등
        reason: String,
    },

    /// 데이터베이스 관련 오류
    #[error("데이터베이스 오류: {operation} - {reason}")]
    Database { operation: String, reason: String },

    /// 설정 관련 오류 (config.rs의 ConfigError와 연동)
    #[error("설정 오류: {0}")]
    Config(#[from] crate::utility::config::ConfigError),

    /// 네트워크 관련 오류
    #[error("네트워크 오류: {operation} - {reason}")]
    Network { operation: String, reason: String },

    /// 데이터 파싱 오류
    #[error("파싱 오류: {data_type} 파싱 실패 - {reason}")]
    Parsing { data_type: String, reason: String },

    /// 권한 관련 오류
    #[error("권한 오류: {operation} - {reason}")]
    Permission { operation: String, reason: String },

    /// 지원하지 않는 기능
    #[error("지원하지 않는 기능: {feature} (현재 Phase: {phase})")]
    UnsupportedFeature { feature: String, phase: String },

    /// ONNX 모델 관련 오류
    #[error("ONNX 모델 오류: {operation} - {reason}")]
    OnnxModel { operation: String, reason: String },

    /// 시간 관련 오류 (시장 시간, 거래 시간 등)
    #[error("시간 오류: {operation} - {reason}")]
    Time { operation: String, reason: String },

    /// 일반적인 I/O 오류
    #[error("I/O 오류: {operation} - {source}")]
    Io {
        operation: String,
        #[source]
        source: std::io::Error,
    },

    /// 타입 변환 오류
    #[error("타입 변환 오류: {from} -> {to} - {reason}")]
    TypeConversion {
        from: String,
        to: String,
        reason: String,
    },

    /// 유효성 검증 오류
    #[error("유효성 검증 실패: {field} - {reason}")]
    Validation { field: String, reason: String },

    /// 토큰 관련 오류 (인증, API 토큰 등)
    #[error("토큰 오류: {operation} - {reason}")]
    Token { operation: String, reason: String },

    /// 일반적인 오류 (기타)
    #[error("오류: {message}")]
    General { message: String },
}

/// StockAI 시스템에서 사용하는 Result 타입 별칭
/// 모든 함수가 이 타입을 반환하여 일관된 오류 처리를 제공
pub type StockrsResult<T> = Result<T, StockrsError>;

impl StockrsError {
    /// API 오류를 간편하게 생성하는 헬퍼 함수
    pub fn api(message: impl Into<String>) -> Self {
        Self::Api {
            message: message.into(),
        }
    }

    /// 한국투자증권 API 오류를 간편하게 생성하는 헬퍼 함수
    pub fn korea_api(operation: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::KoreaInvestmentApi {
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    /// 종목 데이터 없음 오류를 간편하게 생성하는 헬퍼 함수
    pub fn no_stock_data(stockcode: impl Into<String>, days: i32) -> Self {
        Self::NoStockData {
            stockcode: stockcode.into(),
            days,
        }
    }

    /// 주문 실행 오류를 간편하게 생성하는 헬퍼 함수
    pub fn order_execution(
        order_type: impl Into<String>,
        stockcode: impl Into<String>,
        quantity: u32,
        reason: impl Into<String>,
    ) -> Self {
        Self::OrderExecution {
            order_type: order_type.into(),
            stockcode: stockcode.into(),
            quantity,
            reason: reason.into(),
        }
    }

    /// 가격 조회 오류를 간편하게 생성하는 헬퍼 함수
    pub fn price_inquiry(
        stockcode: impl Into<String>,
        price_type: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::PriceInquiry {
            stockcode: stockcode.into(),
            price_type: price_type.into(),
            reason: reason.into(),
        }
    }

    /// 데이터베이스 오류를 간편하게 생성하는 헬퍼 함수
    pub fn database(operation: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Database {
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    /// 지원하지 않는 기능 오류를 간편하게 생성하는 헬퍼 함수
    pub fn unsupported_feature(feature: impl Into<String>, phase: impl Into<String>) -> Self {
        Self::UnsupportedFeature {
            feature: feature.into(),
            phase: phase.into(),
        }
    }

    /// 일반적인 오류를 생성하는 헬퍼 함수
    pub fn general(message: impl Into<String>) -> Self {
        StockrsError::General {
            message: message.into(),
        }
    }

    /// 파일 시스템 관련 오류를 생성하는 헬퍼 함수
    pub fn file_not_found(message: impl Into<String>) -> Self {
        StockrsError::General {
            message: format!("파일을 찾을 수 없습니다: {}", message.into()),
        }
    }

    /// 파일 I/O 오류를 생성하는 헬퍼 함수
    pub fn file_io(message: impl Into<String>) -> Self {
        StockrsError::General {
            message: format!("파일 I/O 오류: {}", message.into()),
        }
    }

    /// 파일 파싱 오류를 생성하는 헬퍼 함수
    pub fn file_parse(message: impl Into<String>) -> Self {
        StockrsError::General {
            message: format!("파일 파싱 오류: {}", message.into()),
        }
    }

    /// 모델 로딩 오류를 생성하는 헬퍼 함수
    pub fn model_loading(message: impl Into<String>) -> Self {
        StockrsError::General {
            message: format!("모델 로딩 오류: {}", message.into()),
        }
    }

    /// 예측 관련 오류를 생성하는 헬퍼 함수
    pub fn prediction(message: impl Into<String>) -> Self {
        StockrsError::General {
            message: format!("예측 오류: {}", message.into()),
        }
    }

    /// 데이터베이스 쿼리 관련 오류를 생성하는 헬퍼 함수
    pub fn database_query(message: impl Into<String>) -> Self {
        StockrsError::Database {
            operation: "쿼리 실행".to_string(),
            reason: message.into(),
        }
    }

    /// 데이터 파싱 오류를 간편하게 생성하는 헬퍼 함수
    pub fn parsing(data_type: impl Into<String>, reason: impl Into<String>) -> Self {
        StockrsError::Parsing {
            data_type: data_type.into(),
            reason: reason.into(),
        }
    }
}

/// Korea Investment API 라이브러리의 오류를 StockrsError로 변환
impl From<korea_investment_api::Error> for StockrsError {
    fn from(error: korea_investment_api::Error) -> Self {
        StockrsError::korea_api("Korea Investment API", error.to_string())
    }
}

/// Rusqlite 데이터베이스 오류를 StockrsError로 변환
impl From<rusqlite::Error> for StockrsError {
    fn from(error: rusqlite::Error) -> Self {
        let operation = match &error {
            rusqlite::Error::SqliteFailure(_, _) => "SQL 실행",
            rusqlite::Error::InvalidParameterName(_) => "매개변수 검증",
            rusqlite::Error::InvalidPath(_) => "경로 확인",
            rusqlite::Error::InvalidColumnIndex(_) => "컬럼 인덱스",
            rusqlite::Error::InvalidColumnName(_) => "컬럼 이름",
            rusqlite::Error::InvalidColumnType(_, _, _) => "컬럼 타입",
            _ => "데이터베이스 작업",
        };

        StockrsError::Database {
            operation: operation.to_string(),
            reason: error.to_string(),
        }
    }
}

/// std::io::Error를 StockrsError로 변환
impl From<std::io::Error> for StockrsError {
    fn from(error: std::io::Error) -> Self {
        StockrsError::Io {
            operation: "파일 I/O".to_string(),
            source: error,
        }
    }
}

/// &str을 StockrsError로 변환
impl From<&str> for StockrsError {
    fn from(message: &str) -> Self {
        StockrsError::General {
            message: message.to_string(),
        }
    }
}

/// String을 StockrsError로 변환
impl From<String> for StockrsError {
    fn from(message: String) -> Self {
        StockrsError::General { message }
    }
}

/// Tokio runtime 오류를 StockrsError로 변환
impl From<tokio::task::JoinError> for StockrsError {
    fn from(error: tokio::task::JoinError) -> Self {
        StockrsError::General {
            message: format!("비동기 작업 실패: {}", error),
        }
    }
}

/// Box<dyn std::error::Error>를 StockrsError로 변환
/// 다른 컴포넌트에서 반환하는 일반적인 에러를 StockrsError로 변환
impl From<Box<dyn std::error::Error>> for StockrsError {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        // 이미 StockrsError인 경우 중복 변환 방지
        if let Some(stockrs_error) = error.downcast_ref::<StockrsError>() {
            return StockrsError::General {
                message: stockrs_error.to_string(),
            };
        }

        let error_msg = error.to_string();

        // 중복된 "오류:" 접두사 제거
        let clean_msg = if error_msg.starts_with("오류: 오류: ") {
            error_msg[6..].to_string() // "오류: 오류: " 제거
        } else if error_msg.starts_with("오류: ") {
            error_msg[3..].to_string() // "오류: " 제거
        } else {
            error_msg
        };

        StockrsError::General { message: clean_msg }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = StockrsError::no_stock_data("005930", 20);
        assert_eq!(error.to_string(), "종목 데이터 없음: 005930 (기간: 20일)");
    }

    #[test]
    fn test_error_helpers() {
        let error = StockrsError::order_execution("매수", "005930", 100, "잔고 부족");
        match error {
            StockrsError::OrderExecution { stockcode, .. } => {
                assert_eq!(stockcode, "005930");
            }
            _ => {
                eprintln!("잘못된 오류 타입");
                assert!(false, "잘못된 오류 타입");
            }
        }
    }

    #[test]
    fn test_result_type() {
        fn test_function() -> StockrsResult<i32> {
            Ok(42)
        }

        assert_eq!(test_function().expect("Test function should succeed"), 42);
    }
}
