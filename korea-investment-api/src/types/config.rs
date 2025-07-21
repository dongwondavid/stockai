use crate::types::Environment;
use getset::{Getters, Setters};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Default, Getters, Setters)]
pub struct Config {
    #[getset(get = "pub")]
    korea_investment_api: KoreaInvestmentApiConfig,

    // 런타임 설정 (설정 파일에는 없지만 프로그램 실행 중 설정)
    #[serde(skip)]
    #[getset(get = "pub", set = "pub")]
    environment: Environment,

    #[serde(skip)]
    #[getset(get = "pub", set = "pub")]
    approval_key: Option<String>,

    #[serde(skip)]
    #[getset(get = "pub", set = "pub")]
    token: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default, Getters)]
pub struct KoreaInvestmentApiConfig {
    // 실제 거래 설정
    #[getset(get = "pub")]
    real_app_key: String,
    #[getset(get = "pub")]
    real_app_secret: String,
    #[getset(get = "pub")]
    real_base_url: String,
    #[getset(get = "pub")]
    real_account_number: String,
    #[getset(get = "pub")]
    real_account_product_code: String,

    // 모의투자 설정
    #[getset(get = "pub")]
    paper_app_key: String,
    #[getset(get = "pub")]
    paper_app_secret: String,
    #[getset(get = "pub")]
    paper_base_url: String,
    #[getset(get = "pub")]
    paper_account_number: String,
    #[getset(get = "pub")]
    paper_account_product_code: String,
}

impl Config {
    /// Returns token as Option<String>, treating empty string as None
    pub fn token_as_option(&self) -> Option<String> {
        match &self.token {
            Some(s) if !s.trim().is_empty() => Some(s.clone()),
            _ => None,
        }
    }

    /// Returns approval_key as Option<String>, treating empty string as None
    pub fn approval_key_as_option(&self) -> Option<String> {
        match &self.approval_key {
            Some(s) if !s.trim().is_empty() => Some(s.clone()),
            _ => None,
        }
    }

    /// 기본 HTS ID 반환 (빈 문자열)
    pub fn hts_id(&self) -> &str {
        "" // HTS ID는 일반적으로 필요하지 않으므로 빈 문자열 반환
    }

    /// 현재 환경에 맞는 API 키 반환
    pub fn app_key(&self) -> &str {
        match self.environment {
            Environment::Real => &self.korea_investment_api.real_app_key,
            Environment::Virtual => &self.korea_investment_api.paper_app_key,
        }
    }

    /// 현재 환경에 맞는 API 시크릿 반환
    pub fn app_secret(&self) -> &str {
        match self.environment {
            Environment::Real => &self.korea_investment_api.real_app_secret,
            Environment::Virtual => &self.korea_investment_api.paper_app_secret,
        }
    }

    /// 현재 환경에 맞는 계좌번호에서 CANO (앞 8자리) 추출
    pub fn cano(&self) -> String {
        let account_number = match self.environment {
            Environment::Real => &self.korea_investment_api.real_account_number,
            Environment::Virtual => &self.korea_investment_api.paper_account_number,
        };

        if account_number.len() >= 8 {
            account_number[..8].to_string()
        } else {
            account_number.clone()
        }
    }

    /// 현재 환경에 맞는 계좌상품코드 반환
    pub fn acnt_prdt_cd(&self) -> &str {
        match self.environment {
            Environment::Real => &self.korea_investment_api.real_account_product_code,
            Environment::Virtual => &self.korea_investment_api.paper_account_product_code,
        }
    }
}
