use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use chrono::{DateTime, Utc, NaiveDateTime};
use crate::utility::config::get_config;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::types::api::ApiType;
use tracing::{info, warn, debug};

/// 개별 API 토큰 정보
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiToken {
    /// 접근 토큰
    pub access_token: String,
    /// 토큰 타입 (항상 "Bearer")
    pub token_type: String,
    /// 유효기간 (초)
    pub expires_in: u32,
    /// 토큰 만료 시간 (일시표시) - "2024-01-01 00:00:00" 형식
    pub access_token_token_expired: String,
    /// 발급 시간
    pub issued_at: DateTime<Utc>,
    /// approval_key (웹소켓용)
    pub approval_key: Option<String>,
}

impl ApiToken {
    /// 토큰이 유효한지 확인
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        let expiry = self.issued_at + chrono::Duration::seconds(self.expires_in as i64);
        now < expiry
    }
    
    /// 토큰 만료까지 남은 시간 (초)
    pub fn seconds_until_expiry(&self) -> i64 {
        let now = Utc::now();
        let expiry = self.issued_at + chrono::Duration::seconds(self.expires_in as i64);
        (expiry - now).num_seconds()
    }
    
    /// 토큰이 곧 만료되는지 확인 (기본값: 6시간 전)
    pub fn is_expiring_soon(&self, buffer_minutes: u32) -> bool {
        let buffer_seconds = buffer_minutes * 60;
        self.seconds_until_expiry() <= buffer_seconds as i64
    }
    
    /// 토큰 만료 시간을 DateTime으로 변환
    pub fn expiry_datetime(&self) -> DateTime<Utc> {
        self.issued_at + chrono::Duration::seconds(self.expires_in as i64)
    }
    
    /// access_token_token_expired 문자열을 DateTime으로 파싱
    pub fn parse_expiry_datetime(&self) -> Result<DateTime<Utc>, StockrsError> {
        let naive = NaiveDateTime::parse_from_str(&self.access_token_token_expired, "%Y-%m-%d %H:%M:%S")
            .map_err(|e| StockrsError::Token {
                operation: "만료 시간 파싱".to_string(),
                reason: format!("{} - {}", self.access_token_token_expired, e),
            })?;
        
        Ok(DateTime::from_naive_utc_and_offset(naive, Utc))
    }
}

/// 토큰 데이터 구조체
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenData {
    /// 실전 거래 토큰
    pub real_token: Option<ApiToken>,
    /// 모의투자 토큰
    pub paper_token: Option<ApiToken>,
    /// 정보 조회 토큰
    pub info_token: Option<ApiToken>,
    /// 마지막 업데이트 시간
    pub last_updated: DateTime<Utc>,
}

impl Default for TokenData {
    fn default() -> Self {
        Self {
            real_token: None,
            paper_token: None,
            info_token: None,
            last_updated: Utc::now(),
        }
    }
}

/// 토큰 관리자
pub struct TokenManager {
    token_file_path: String,
    backup_file_path: Option<String>,
    auto_refresh: bool,
    refresh_buffer_minutes: u32,
    auto_cleanup: bool,
}

impl TokenManager {
    /// 새로운 토큰 관리자 생성
    pub fn new() -> StockrsResult<Self> {
        let config = get_config()?;
        
        Ok(Self {
            token_file_path: config.token_management.token_file_path.clone(),
            backup_file_path: if config.token_management.backup_token_file {
                Some(config.token_management.backup_token_file_path.clone())
            } else {
                None
            },
            auto_refresh: config.token_management.auto_refresh_tokens,
            refresh_buffer_minutes: config.token_management.refresh_before_expiry_minutes,
            auto_cleanup: config.token_management.auto_cleanup_expired_tokens,
        })
    }
    
    /// 토큰 데이터 로드
    pub fn load_tokens(&self) -> StockrsResult<Option<TokenData>> {
        if !Path::new(&self.token_file_path).exists() {
            debug!("토큰 파일이 존재하지 않습니다: {}", self.token_file_path);
            return Ok(None);
        }
        
        let content = fs::read_to_string(&self.token_file_path)
            .map_err(|e| StockrsError::Token {
                operation: "토큰 파일 읽기".to_string(),
                reason: e.to_string(),
            })?;
            
        let mut tokens: TokenData = serde_json::from_str(&content)
            .map_err(|e| StockrsError::Token {
                operation: "토큰 파싱".to_string(),
                reason: e.to_string(),
            })?;
        
        // 만료된 토큰 정리
        if self.auto_cleanup {
            self.cleanup_expired_tokens(&mut tokens)?;
        }
        
        // 유효한 토큰이 있는지 확인
        if tokens.real_token.is_none() && tokens.paper_token.is_none() && tokens.info_token.is_none() {
            debug!("모든 토큰이 만료되었거나 존재하지 않습니다");
            return Ok(None);
        }
        
        Ok(Some(tokens))
    }
    
    /// 토큰 데이터 저장
    pub fn save_tokens(&self, tokens: &TokenData) -> StockrsResult<()> {
        let json = serde_json::to_string_pretty(tokens)
            .map_err(|e| StockrsError::Token {
                operation: "토큰 직렬화".to_string(),
                reason: e.to_string(),
            })?;
        
        // 백업 파일 생성
        if let Some(backup_path) = &self.backup_file_path {
            if Path::new(&self.token_file_path).exists() {
                fs::copy(&self.token_file_path, backup_path)
                    .map_err(|e| StockrsError::Token {
                        operation: "토큰 파일 백업".to_string(),
                        reason: e.to_string(),
                    })?;
            }
        }
        
        // 토큰 파일 저장
        fs::write(&self.token_file_path, json)
            .map_err(|e| StockrsError::Token {
                operation: "토큰 파일 저장".to_string(),
                reason: e.to_string(),
            })?;
        
        info!("토큰이 저장되었습니다: {}", self.token_file_path);
        Ok(())
    }
    
    /// 특정 API 타입의 토큰 가져오기
    pub fn get_token(&self, api_type: ApiType) -> StockrsResult<Option<ApiToken>> {
        let tokens = self.load_tokens()?;
        
        let token = match api_type {
            ApiType::Real => tokens.and_then(|t| t.real_token),
            ApiType::Paper => tokens.and_then(|t| t.paper_token),
            ApiType::Backtest => None,
        };
        
        if let Some(token) = &token {
            if !token.is_valid() {
                warn!("토큰이 만료되었습니다: {:?}", api_type);
                return Ok(None);
            }
            
            if self.auto_refresh && token.is_expiring_soon(self.refresh_buffer_minutes) {
                info!("토큰이 곧 만료됩니다 ({}분 후). 갱신이 필요합니다.", 
                      self.refresh_buffer_minutes);
            }
        }
        
        Ok(token)
    }
    
    /// 토큰 업데이트
    pub fn update_token(&self, api_type: ApiType, token: ApiToken) -> StockrsResult<()> {
        let tokens_option = self.load_tokens()?;
        let mut tokens = tokens_option.unwrap_or_default();
        
        match api_type {
            ApiType::Real => tokens.real_token = Some(token),
            ApiType::Paper => tokens.paper_token = Some(token),
            ApiType::Backtest => return Ok(()), // 백테스트는 토큰 불필요
        }
        
        tokens.last_updated = Utc::now();
        self.save_tokens(&tokens)
    }
    
    /// 만료된 토큰 정리
    fn cleanup_expired_tokens(&self, tokens: &mut TokenData) -> StockrsResult<()> {
        let mut cleaned = false;
        
        if let Some(token) = &tokens.real_token {
            if !token.is_valid() {
                tokens.real_token = None;
                cleaned = true;
                info!("만료된 실전 토큰을 정리했습니다");
            }
        }
        
        if let Some(token) = &tokens.paper_token {
            if !token.is_valid() {
                tokens.paper_token = None;
                cleaned = true;
                info!("만료된 모의투자 토큰을 정리했습니다");
            }
        }
        
        if let Some(token) = &tokens.info_token {
            if !token.is_valid() {
                tokens.info_token = None;
                cleaned = true;
                info!("만료된 정보 조회 토큰을 정리했습니다");
            }
        }
        
        if cleaned {
            tokens.last_updated = Utc::now();
            self.save_tokens(tokens)?;
        }
        
        Ok(())
    }
    
    /// 모든 토큰 삭제
    pub fn clear_all_tokens(&self) -> StockrsResult<()> {
        if Path::new(&self.token_file_path).exists() {
            fs::remove_file(&self.token_file_path)
                .map_err(|e| StockrsError::Token {
                    operation: "토큰 파일 삭제".to_string(),
                    reason: e.to_string(),
                })?;
            info!("모든 토큰이 삭제되었습니다");
        }
        
        if let Some(backup_path) = &self.backup_file_path {
            if Path::new(backup_path).exists() {
                fs::remove_file(backup_path)
                    .map_err(|e| StockrsError::Token {
                        operation: "토큰 백업 파일 삭제".to_string(),
                        reason: e.to_string(),
                    })?;
            }
        }
        
        Ok(())
    }
    
    /// 토큰 상태 정보 출력
    pub fn print_token_status(&self) -> StockrsResult<()> {
        let tokens = self.load_tokens()?;
        
        if let Some(tokens) = tokens {
            info!("=== 토큰 상태 ===");
            
            if let Some(token) = &tokens.real_token {
                let remaining = token.seconds_until_expiry();
                let hours = remaining / 3600;
                let minutes = (remaining % 3600) / 60;
                info!("실전 토큰: 유효 (남은 시간: {}시간 {}분)", hours, minutes);
            } else {
                info!("실전 토큰: 없음");
            }
            
            if let Some(token) = &tokens.paper_token {
                let remaining = token.seconds_until_expiry();
                let hours = remaining / 3600;
                let minutes = (remaining % 3600) / 60;
                info!("모의투자 토큰: 유효 (남은 시간: {}시간 {}분)", hours, minutes);
            } else {
                info!("모의투자 토큰: 없음");
            }
            
            if let Some(token) = &tokens.info_token {
                let remaining = token.seconds_until_expiry();
                let hours = remaining / 3600;
                let minutes = (remaining % 3600) / 60;
                info!("정보 조회 토큰: 유효 (남은 시간: {}시간 {}분)", hours, minutes);
            } else {
                info!("정보 조회 토큰: 없음");
            }
            
            info!("마지막 업데이트: {}", tokens.last_updated.format("%Y-%m-%d %H:%M:%S"));
        } else {
            info!("저장된 토큰이 없습니다");
        }
        
        Ok(())
    }
} 