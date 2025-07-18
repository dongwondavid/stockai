use crate::types::Environment;
use getset::{Getters, Setters};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Default, Getters, Setters)]
pub struct Config {
    #[getset(get = "pub")]
    hts_id: String,
    #[getset(get = "pub")]
    cano: String,
    #[getset(get = "pub")]
    acnt_prdt_cd: String,
    #[getset(get = "pub")]
    app_key: String,
    #[getset(get = "pub")]
    app_secret: String,
    #[getset(get = "pub", set = "pub")]
    approval_key: Option<String>,
    #[getset(get = "pub", set = "pub")]
    token: Option<String>,
    #[getset(get = "pub")]
    environment: Environment,
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
}
