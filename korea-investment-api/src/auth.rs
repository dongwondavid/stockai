use crate::types::{request, response, Environment};
use crate::Error;
use reqwest::header::{HeaderMap, HeaderValue};

/// Auth
/// [OAuth 인증 관련](https://apiportal.koreainvestment.com/apiservice/oauth2#L_214b9e22-8f2e-4fba-9688-587279f1061a)
#[derive(Clone)]
pub struct Auth {
    client: reqwest::Client,
    endpoint_url: String,
    appkey: String,
    appsecret: String,
    token: Option<String>,
    approval_key: Option<String>,
    /// 토큰 생성 응답 정보를 저장하기 위한 필드들
    token_response: Option<response::auth::Body::TokenCreation>,
    token_issued_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Auth {
    /// OAuth API 사용을 위한 구조체 생성
    /// hash, token, approval_key는 None으로 시작
    /// create_hash, create_token, create_approval_key 함수를 호출해야 hash, token 값이 저장됨
    pub fn new(
        client: &reqwest::Client,
        environment: Environment,
        appkey: &str,
        appsecret: &str,
    ) -> Self {
        let endpoint_url = match environment {
            Environment::Real => "https://openapi.koreainvestment.com:9443",
            Environment::Virtual => "https://openapivts.koreainvestment.com:29443",
        }
        .to_string();
        Self {
            client: client.clone(),
            endpoint_url,
            appkey: appkey.to_string(),
            appsecret: appsecret.to_string(),
            token: None,
            approval_key: None,
            token_response: None,
            token_issued_at: None,
        }
    }

    /// 구조체에 저장되어 있는 token을 반환
    pub fn get_token(&self) -> Option<String> {
        self.token.clone()
    }

    /// 구조체에 저장되어 있는 approval_key 반환
    pub fn get_approval_key(&self) -> Option<String> {
        self.approval_key.clone()
    }

    /// 구조체에 저장되어 있는 appkey 반환
    pub fn get_appkey(&self) -> String {
        self.appkey.clone()
    }

    /// 구조체에 저장되어 있는 appsecret 반환
    pub fn get_appsecret(&self) -> String {
        self.appsecret.clone()
    }

    /// 토큰 생성 응답 정보를 가져오기
    pub fn get_token_response(&self) -> Option<&response::auth::Body::TokenCreation> {
        self.token_response.as_ref()
    }

    /// 토큰 발급 시간을 가져오기
    pub fn get_token_issued_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.token_issued_at
    }

    /// 실시간 (웹소켓) 접속키 발급[실시간-000]
    /// [Docs](https://apiportal.koreainvestment.com/apiservice/oauth2#L_5c87ba63-740a-4166-93ac-803510bb9c02)
    /// 웹소켓 접속키를 발급받아서 반환함과 동시에 구조체의 approval_key 업데이트
    pub async fn create_approval_key(&mut self) -> Result<String, Error> {
        let approval_key = self
            .client
            .post(format!("{}/oauth2/Approval", self.endpoint_url))
            .header("Content-Type", "application/json")
            .body(
                serde_json::json!(request::auth::ApprovalKeyCreationBody::new(
                    self.appsecret.clone(),
                    self.appkey.clone(),
                ))
                .to_string(),
            )
            .send()
            .await?
            .json::<response::auth::Body::ApprovalKeyCreation>()
            .await?
            .get_approval_key();
        self.approval_key = Some(approval_key.clone());
        Ok(approval_key)
    }

    pub fn set_approval_key(&mut self, approval_key: String) {
        self.approval_key = Some(approval_key);
    }

    /// Hashkey
    /// [Docs](https://apiportal.koreainvestment.com/apiservice/oauth2#L_214b9e22-8f2e-4fba-9688-587279f1061a)
    /// hash값을 얻어와서 반환
    pub async fn get_hash(&self, json: String) -> Result<String, Error> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Content-Type",
            HeaderValue::from_str("application/json").unwrap(),
        );
        headers.insert("appkey", HeaderValue::from_str(&self.appkey).unwrap());
        headers.insert("appsecret", HeaderValue::from_str(&self.appsecret).unwrap());
        let hash = self
            .client
            .post(format!("{}/uapi/hashkey", self.endpoint_url))
            .headers(headers)
            .body(json)
            .send()
            .await?
            .json::<response::auth::Body::HashKey>()
            .await?
            .get_hash();
        Ok(hash)
    }

    /// 접근토큰발급(P)[인증-001]
    /// [Docs](https://apiportal.koreainvestment.com/apiservice/oauth2#L_fa778c98-f68d-451e-8fff-b1c6bfe5cd30)
    /// token값을 얻어와서 반환함과 동시에 구조체의 token을 업데이트
    pub async fn create_token(&mut self) -> Result<String, Error> {
        let request_url = format!("{}/oauth2/tokenP", self.endpoint_url);
        let request_body = serde_json::json!(request::auth::TokenCreationBody::new(
            self.appsecret.clone(),
            self.appkey.clone(),
        )).to_string();
        
        // 디버깅: 요청 정보 출력
        // println!("🔍 [create_token] 요청 URL: {}", request_url);
        // println!("🔍 [create_token] 요청 헤더: Content-Type: application/json");
        // println!("🔍 [create_token] 요청 바디: {}", request_body);
        // println!("🔍 [create_token] 사용된 appkey: {}", self.appkey);
        // println!("🔍 [create_token] 사용된 appsecret: {}", self.appsecret);
        
        let response = self
            .client
            .post(&request_url)
            .header("Content-Type", "application/json")
            .body(request_body)
            .send()
            .await?;

        // 응답 텍스트를 먼저 가져와서 에러인지 확인
        let response_text = response.text().await?;
        
        // 디버깅: 응답 정보 출력
        // println!("🔍 [create_token] 응답 텍스트: {}", response_text);
        
        // JSON으로 파싱해서 에러 응답인지 확인
        if let Ok(error_response) = serde_json::from_str::<response::auth::Body::ApiError>(&response_text) {
            return Err(Error::AuthInitFailed(format!(
                "API 에러: {} - {}",
                error_response.error_code,
                error_response.error_description
            )));
        }

        // 성공 응답으로 파싱
        let token_response: response::auth::Body::TokenCreation = serde_json::from_str(&response_text)?;
        let token = token_response.get_access_token();
        self.token = Some(token.clone());
        self.token_response = Some(token_response);
        self.token_issued_at = Some(chrono::Utc::now());
        Ok(token)
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    /// 접근토큰폐기(P)[인증-002]
    /// [Docs](https://apiportal.koreainvestment.com/apiservice/oauth2#L_dd3cb447-5034-4711-8c88-62c913429c7b)
    /// token을 폐기. 폐기 결과 리스폰스를 그대로 반환
    /// 리스폰스 형식: {
    ///     code: u32,
    ///     message: String,
    /// }
    pub async fn revoke_token(&self) -> Result<response::auth::Body::TokenRevoke, Error> {
        Ok(self
            .client
            .post(format!("{}/oauth2/revokeP", &self.endpoint_url))
            .header("Content-Type", "application/json")
            .body(
                serde_json::json!(request::auth::TokenRevokeBody::new(
                    self.appkey.clone(),
                    self.appsecret.clone(),
                    match self.token.clone() {
                        Some(token) => token,
                        None => {
                            return Err(Error::AuthInitFailed("token".to_string()));
                        }
                    }
                ))
                .to_string(),
            )
            .send()
            .await?
            .json::<response::auth::Body::TokenRevoke>()
            .await?)
    }
}
