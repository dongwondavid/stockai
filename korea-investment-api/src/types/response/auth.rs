#![allow(non_snake_case)]

pub mod Body {
    use serde::{Deserialize, Serialize};

    /// API 오류 응답
    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ApiError {
        pub error_code: String,
        pub error_description: String,
    }

    /// 실시간 (웹소켓) 접속키 발급
    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ApprovalKeyCreation {
        approval_key: String,
    }
    impl ApprovalKeyCreation {
        pub fn get_approval_key(&self) -> String {
            self.approval_key.clone()
        }
    }

    /// Hashkey
    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct HashKey {
        HASH: String,
    }
    impl HashKey {
        pub fn get_hash(&self) -> String {
            self.HASH.clone()
        }
    }

    /// 접근토큰발급(P)
    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct TokenCreation {
        access_token: String,
        token_type: String,
        expires_in: u32,
        access_token_token_expired: String,
    }
    impl TokenCreation {
        pub fn get_access_token(&self) -> String {
            self.access_token.clone()
        }
        
        pub fn get_token_type(&self) -> String {
            self.token_type.clone()
        }
        
        pub fn get_expires_in(&self) -> u32 {
            self.expires_in
        }
        
        pub fn get_access_token_token_expired(&self) -> String {
            self.access_token_token_expired.clone()
        }
    }

    /// 접근토큰폐기(P)
    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct TokenRevoke {
        pub code: u32,
        pub message: String,
    }
}
