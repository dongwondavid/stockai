use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::types::api::StockApi;
use crate::utility::types::broker::Order;
use crate::utility::types::trading::AssetInfo;
use crate::utility::config;
use crate::utility::token_manager::{TokenManager, ApiToken};

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use chrono::Utc;
use tracing::{info, warn};
use std::time::Duration;
use tokio::time::timeout;

#[derive(Debug, Clone, Copy)]
pub enum ApiMode {
    Real,  // 실제 거래
    Paper, // 모의투자
    Info,  // 정보용 실전 API (시세 조회 등)
}

/// 한국투자증권 API 구현
pub struct KoreaApi {
    mode: ApiMode,
    api: RefCell<Rc<korea_investment_api::KoreaInvestmentApi>>,
    token_manager: TokenManager,
    // 주문번호별 주문채번지점번호(영업점코드) 저장: order_id -> ord_gno_brno / krx_fwdg_ord_orgno
    order_branch_map: RefCell<HashMap<String, String>>, 
}

/// 한국투자 주식일별주문체결조회 결과 요약
pub struct OrderFillInfo {
    pub ord_dt: String,
    pub ord_tmd: String,
    pub pdno: String,
    pub ord_qty: u32,
    pub tot_ccld_qty: u32,
    pub rmn_qty: u32,
    pub ord_unpr: f64,
    pub avg_prvs: f64,
}

#[derive(Debug, Clone, Copy)]
struct TimeoutRetryPolicy {
    max_retries: usize,
    base_delay_ms: u64,
    #[allow(dead_code)]
    max_delay_ms: u64,
    timeout_ms: u64,
    retry_on_error: bool,
}

impl Default for TimeoutRetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay_ms: 1_100,
            max_delay_ms: 6_000,
            timeout_ms: 6_000,
            retry_on_error: true,
        }
    }
}

fn is_rate_limit_error_message(message: &str) -> bool {
    let m = message;
    // KIS rate limit only
    m.contains("EGW00201")
        || m.contains("초당 거래건수")
        || m.contains("Too Many Requests")
        || m.contains("429")
}

async fn with_rate_limit_retry<T, Fut, F>(mode_name: &str, op_name: &str, mut make_future: F, policy: TimeoutRetryPolicy) -> StockrsResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = StockrsResult<T>>,
{
    let mut attempt_index: usize = 0;
    let mut current_delay_ms: u64 = policy.base_delay_ms;

    loop {
        attempt_index += 1;
        let op_timeout = Duration::from_millis(policy.timeout_ms);

        match timeout(op_timeout, make_future()).await {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(err)) => {
                let attempt_allowed = attempt_index <= policy.max_retries;
                let msg = err.to_string();
                if policy.retry_on_error && attempt_allowed && is_rate_limit_error_message(&msg) {
                    let delay_ms = current_delay_ms;
                    println!(
                        "⚠️ [KoreaApi:{}] {} 레이트리미트 재시도({}/{}): {} (대기 {}ms)",
                        mode_name,
                        op_name,
                        attempt_index,
                        policy.max_retries,
                        msg,
                        delay_ms
                    );
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    // Exponential backoff up to max_delay_ms
                    current_delay_ms = (current_delay_ms.saturating_mul(2)).min(policy.max_delay_ms);
                    continue;
                }
                // 비-재시도 오류는 그대로 전파
                return Err(err);
            }
            Err(_elapsed) => {
                // 타임아웃은 재시도하지 않고 즉시 에러 반환
                return Err(StockrsError::Network {
                    operation: format!("{} ({})", op_name, mode_name),
                    reason: format!("요청 타임아웃(>{}ms)", policy.timeout_ms),
                });
            }
        }
    }
}

impl KoreaApi {
    /// 주식 당일 1분 데이터를 조회하여 5분봉(09:01~09:30, 특이일은 10:01~10:30)으로 집계
    /// 실전/모의/정보 모드 모두 사용 가능 (읽기 전용)
    pub fn get_morning_5min_ohlcv(&self, stockcode: &str, date: &str) -> StockrsResult<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>)> {
        use std::collections::{BTreeMap, HashSet};

        // HHMMSS 범위 결정 (특이일 지원)
        let end_hhmmss = {
            let cfg = config::get_config()?;
            let special_path = &cfg.time_management.special_start_dates_file_path;
            let mut is_special = false;
            if let Ok(content) = std::fs::read_to_string(special_path) {
                for line in content.lines() {
                    if line.trim() == date { is_special = true; break; }
                }
            }
            if is_special { "103000".to_string() } else { "093000".to_string() }
        };

        // 종목코드 정규화 (API는 'A' 접두사 없이 6자리 단축코드 사용)
        let clean_code = if stockcode.starts_with('A') { &stockcode[1..] } else { stockcode };

        let rt = tokio::runtime::Runtime::new()?;
        let (rows, meta_ok) = rt.block_on(async {
            self.call_with_token_refresh(
                "당일분봉조회",
                || async {
                    let api = { self.api.borrow().clone() };
                    let params = korea_investment_api::types::request::stock::quote::MinutePriceChartParameter::new(
                        "J",
                        clean_code,
                        &end_hhmmss,
                        false,
                        "",
                    );
                    let out = api
                        .quote
                        .minute_price_chart(params)
                        .await
                        .map_err(StockrsError::from)?;

                    let has_output2 = out.output2().is_some();
                    if out.rt_cd() != "0" || !has_output2 {
                        return Err(StockrsError::api(format!(
                            "KIS 당일분봉 조회 오류: rt_cd={}, msg_cd={}, msg1={}",
                            out.rt_cd(),
                            out.msg_cd(),
                            out.msg1()
                        )));
                    }

                    let meta_ok = out.output1().is_some();
                    let rows = out
                        .output2()
                        .as_ref()
                        .cloned()
                        .unwrap_or_default();
                    Ok((rows, meta_ok))
                },
            ).await
        })?;

        if !meta_ok { println!("⚠️ [KoreaApi:{}] 분봉 메타데이터(output1) 누락", self.mode_name()); }

        // 필요한 시작/종료 구간 계산 (특이일 지원)
        // 요구사항: 항상 09:01~09:30 또는 10:01~10:30 구간만 사용
        let (start_hhmmss, end_hhmmss) = if end_hhmmss.starts_with("10") { ("100100", "103000") } else { ("090100", "093000") };

        #[derive(Default, Clone)]
        struct Bucket { open: Option<f64>, high: f64, low: f64, close: Option<f64>, volume: f64 }

        let mut buckets: BTreeMap<String, Bucket> = BTreeMap::new();
        let mut seen_minute: HashSet<(String, String)> = HashSet::new();
        let mut bucket_times: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();

        for row in rows.iter() {
            if row.stck_bsop_date() != date { continue; }
            let t = row.stck_cntg_hour(); // &String (HHMMSS)
            let ts = t.as_str();
            if ts < start_hhmmss || ts > end_hhmmss { continue; }

            // 동일 분(YYYYMMDD+HHMMSS) 중복 방지
            let minute_key = (row.stck_bsop_date().to_string(), ts.to_string());
            if !seen_minute.insert(minute_key) { continue; }

            // 버킷 키 계산: 고정 구간 (01~05, 06~10, 11~15, 16~20, 21~25, 26~30)
            if ts.len() < 6 { continue; }
            let hh: &str = &ts[0..2];
            let mm: usize = ts[2..4].parse().unwrap_or(0);
            let mm_start = match mm {
                1..=5 => 1,
                6..=10 => 6,
                11..=15 => 11,
                16..=20 => 16,
                21..=25 => 21,
                26..=30 => 26,
                _ => { continue; } // 00 또는 31 이상은 제외
            };
            let key = format!("{}{:02}", hh, mm_start);

            let mut b = buckets.get(&key).cloned().unwrap_or_default();

            let op = row.stck_oprc().trim().parse::<f64>().map_err(|e| StockrsError::Parsing { data_type: format!("시가({})", key), reason: e.to_string() })?;
            let hg = row.stck_hgpr().trim().parse::<f64>().map_err(|e| StockrsError::Parsing { data_type: format!("고가({})", key), reason: e.to_string() })?;
            let lw = row.stck_lwpr().trim().parse::<f64>().map_err(|e| StockrsError::Parsing { data_type: format!("저가({})", key), reason: e.to_string() })?;
            let cp = row.stck_prpr().trim().parse::<f64>().map_err(|e| StockrsError::Parsing { data_type: format!("종가({})", key), reason: e.to_string() })?;
            let vol = row.cntg_vol().trim().replace(",", "").parse::<f64>().map_err(|e| StockrsError::Parsing { data_type: format!("거래량({})", key), reason: e.to_string() })?;

            if b.open.is_none() { b.open = Some(op); }
            if b.close.is_none() { b.close = Some(cp); } else { b.close = Some(cp); }
            if b.high == 0.0 { b.high = hg; } else { b.high = b.high.max(hg); }
            if b.low == 0.0 { b.low = lw; } else { b.low = b.low.min(lw); }
            b.volume += vol;
            bucket_times.entry(key.clone()).or_default().insert(ts.to_string());
            buckets.insert(key, b);
        }

        for (k, time_set) in bucket_times.iter() {
            println!("[KoreaApi] 5분 bucket {} 포함된 시간: {:?}", k, time_set);
        }

        // 결과 벡터 구성: 키 오름차순 (시간순)
        let mut opens = Vec::new();
        let mut highs = Vec::new();
        let mut lows = Vec::new();
        let mut closes = Vec::new();
        let mut volumes = Vec::new();

        for (_k, b) in buckets.iter() {
            let open = b.open.ok_or_else(|| StockrsError::general("5분 버킷 시가 누락".to_string()))?;
            let close = b.close.ok_or_else(|| StockrsError::general("5분 버킷 종가 누락".to_string()))?;
            opens.push(open);
            highs.push(b.high);
            lows.push(b.low);
            closes.push(close);
            volumes.push(b.volume);
        }

        if opens.is_empty() {
            return Err(StockrsError::general(format!(
                "분봉 데이터가 비어있습니다: {} {} ({}~{})",
                clean_code, date, start_hhmmss, end_hhmmss
            )));
        }

        Ok((closes, opens, highs, lows, volumes))
    }
    /// KIS 오류 메시지에서 토큰 만료 여부를 판단
    fn is_token_expired_message(message: &str) -> bool {
        let m = message;
        // 대표 오류 코드 및 메시지
        m.contains("EGW00123") || m.contains("기간이 만료된 token") || m.to_ascii_lowercase().contains("token expired")
    }

    /// API 토큰을 재발급하고 내부 API 인스턴스를 교체
    async fn refresh_api_token(&self) -> StockrsResult<()> {
        let mode = self.mode;
            let config = config::get_config()?;

            // Info는 Real과 동일 토큰 사용
            let api_type = match mode {
                ApiMode::Real => crate::utility::types::api::ApiType::Real,
                ApiMode::Paper => crate::utility::types::api::ApiType::Paper,
                ApiMode::Info => crate::utility::types::api::ApiType::Real,
            };

            let account = korea_investment_api::types::Account {
                cano: match mode {
                    ApiMode::Real => config.korea_investment_api.real_account_number.clone(),
                    ApiMode::Paper => config.korea_investment_api.paper_account_number.clone(),
                    ApiMode::Info => config.korea_investment_api.info_account_number.clone(),
                },
                acnt_prdt_cd: match mode {
                    ApiMode::Real => config.korea_investment_api.real_account_product_code.clone(),
                    ApiMode::Paper => config.korea_investment_api.paper_account_product_code.clone(),
                    ApiMode::Info => config.korea_investment_api.info_account_product_code.clone(),
                },
            };

            let api = with_rate_limit_retry(
                match mode {
                    ApiMode::Real => "실거래",
                    ApiMode::Paper => "모의투자",
                    ApiMode::Info => "정보용 실전 API",
                },
                "토큰 재발급 및 API 재초기화",
                || async {
                    let api = korea_investment_api::KoreaInvestmentApi::new(
                        match mode {
                            ApiMode::Real => korea_investment_api::types::Environment::Real,
                            ApiMode::Paper => korea_investment_api::types::Environment::Virtual,
                            ApiMode::Info => korea_investment_api::types::Environment::Real,
                        },
                        match mode {
                            ApiMode::Real => &config.korea_investment_api.real_app_key,
                            ApiMode::Paper => &config.korea_investment_api.paper_app_key,
                            ApiMode::Info => &config.korea_investment_api.info_app_key,
                        },
                        match mode {
                            ApiMode::Real => &config.korea_investment_api.real_app_secret,
                            ApiMode::Paper => &config.korea_investment_api.paper_app_secret,
                            ApiMode::Info => &config.korea_investment_api.info_app_secret,
                        },
                        account.clone(),
                        "HTS_ID",
                        None,        // force new token issuance
                        None,
                    )
                    .await
                    .map_err(StockrsError::from)?;
                    Ok(api)
                },
                TimeoutRetryPolicy { timeout_ms: 2_500, ..Default::default() },
            )
            .await?;

            // 토큰 저장
            if let (Some(token), Some(approval_key)) = (api.auth.get_token(), api.auth.get_approval_key()) {
                if let Some(token_response) = api.auth.get_token_response() {
                    let api_token = ApiToken {
                        access_token: token,
                        token_type: token_response.get_token_type(),
                        expires_in: token_response.get_expires_in(),
                        access_token_token_expired: token_response.get_access_token_token_expired(),
                        issued_at: api.auth.get_token_issued_at().unwrap_or_else(|| Utc::now()),
                        approval_key: Some(approval_key),
                    };
                    self.token_manager.update_token(api_type, api_token)?;
                } else {
                    warn!("토큰 응답 정보 누락 - 기본값 사용(재발급)");
                    let api_token = ApiToken {
                        access_token: token,
                        token_type: "Bearer".to_string(),
                        expires_in: 86400,
                        access_token_token_expired: "2024-12-31 23:59:59".to_string(),
                        issued_at: Utc::now(),
                        approval_key: Some(approval_key),
                    };
                    self.token_manager.update_token(api_type, api_token)?;
                }
            }

            // 내부 API 교체
            {
                let mut slot = self.api.borrow_mut();
                *slot = Rc::new(api);
            }

            info!("🔑 [KoreaApi] 토큰 재발급 및 API 재초기화 완료: {:?}", mode);
            Ok(())
    }

    /// 일반 호출을 감싸서 토큰 만료 시 재발급 후 1회 재시도
    async fn call_with_token_refresh<T, Fut, F>(&self, op_name: &str, mut make_future: F) -> StockrsResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = StockrsResult<T>>,
    {
        match with_rate_limit_retry(self.mode_name(), op_name, || make_future(), TimeoutRetryPolicy::default()).await {
            Ok(v) => Ok(v),
            Err(e) => {
                let msg = e.to_string();
                if Self::is_token_expired_message(&msg) {
                    println!(
                        "🔐 [KoreaApi:{}] {}: 토큰 만료 감지 -> 재발급 후 1회 재시도",
                        self.mode_name(),
                        op_name
                    );
                    // 재발급 후 재시도
                    self.refresh_api_token().await?;
                    // 재시도 (추가 만료는 상위로 전파)
                    with_rate_limit_retry(self.mode_name(), op_name, || make_future(), TimeoutRetryPolicy::default()).await
                } else {
                    Err(e)
                }
            }
        }
    }
    pub fn new_real() -> StockrsResult<Self> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async { Self::new(ApiMode::Real).await })
    }

    pub fn new_paper() -> StockrsResult<Self> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async { Self::new(ApiMode::Paper).await })
    }

    pub fn new_info() -> StockrsResult<Self> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async { Self::new(ApiMode::Info).await })
    }

    async fn new(mode: ApiMode) -> StockrsResult<Self> {
        let config = config::get_config()?;
        let token_manager = TokenManager::new()?;

        // 저장된 토큰 확인
        let api_type = match mode {
            ApiMode::Real => crate::utility::types::api::ApiType::Real,
            ApiMode::Paper => crate::utility::types::api::ApiType::Paper,
            ApiMode::Info => crate::utility::types::api::ApiType::Real, // Info는 Real과 동일한 토큰 사용
        };
        
        let saved_token = token_manager.get_token(api_type)?;
        
        let (token, approval_key) = if let Some(api_token) = saved_token {
            info!("저장된 토큰을 사용합니다: {:?}", mode);
            (Some(api_token.access_token), api_token.approval_key)
        } else {
            info!("새 토큰을 발급받습니다: {:?}", mode);
            (None, None)
        };

        let account = korea_investment_api::types::Account {
            cano: match mode {
                ApiMode::Real => config.korea_investment_api.real_account_number.clone(),
                ApiMode::Paper => config.korea_investment_api.paper_account_number.clone(),
                ApiMode::Info => config.korea_investment_api.info_account_number.clone(),
            },
            acnt_prdt_cd: match mode {
                ApiMode::Real => config
                    .korea_investment_api
                    .real_account_product_code
                    .clone(),
                ApiMode::Paper => config
                    .korea_investment_api
                    .paper_account_product_code
                    .clone(),
                ApiMode::Info => config
                    .korea_investment_api
                    .info_account_product_code
                    .clone(),
            },
        };

        let mode_name = match mode { ApiMode::Real => "실거래", ApiMode::Paper => "모의투자", ApiMode::Info => "정보용 실전 API" };

        let api = with_rate_limit_retry(
            mode_name,
            "API 초기화",
            || async {
                let api = korea_investment_api::KoreaInvestmentApi::new(
                    match mode {
                        ApiMode::Real => korea_investment_api::types::Environment::Real,
                        ApiMode::Paper => korea_investment_api::types::Environment::Virtual,
                        ApiMode::Info => korea_investment_api::types::Environment::Real,
                    },
                    match mode {
                        ApiMode::Real => &config.korea_investment_api.real_app_key,
                        ApiMode::Paper => &config.korea_investment_api.paper_app_key,
                        ApiMode::Info => &config.korea_investment_api.info_app_key,
                    },
                    match mode {
                        ApiMode::Real => &config.korea_investment_api.real_app_secret,
                        ApiMode::Paper => &config.korea_investment_api.paper_app_secret,
                        ApiMode::Info => &config.korea_investment_api.info_app_secret,
                    },
                    account.clone(),
                    "HTS_ID",
                    token.clone(),
                    approval_key.clone(),
                )
                .await
                .map_err(StockrsError::from)?;
                Ok(api)
            },
            TimeoutRetryPolicy { timeout_ms: 2_500, ..Default::default() },
        )
        .await?;

        // 새로 발급받은 토큰 저장
        if let (Some(token), Some(approval_key)) = (api.auth.get_token(), api.auth.get_approval_key()) {
            // OAuth 응답에서 토큰 정보 추출
            if let Some(token_response) = api.auth.get_token_response() {
                let api_token = ApiToken {
                    access_token: token,
                    token_type: token_response.get_token_type(),
                    expires_in: token_response.get_expires_in(),
                    access_token_token_expired: token_response.get_access_token_token_expired(),
                    issued_at: api.auth.get_token_issued_at().unwrap_or_else(|| Utc::now()),
                    approval_key: Some(approval_key),
                };
                
                token_manager.update_token(api_type, api_token)?;
                info!("토큰이 저장되었습니다: {:?}", mode);
            } else {
                // 토큰 응답 정보가 없는 경우 기본값 사용
                warn!("토큰 응답 정보가 없어 기본값을 사용합니다: {:?}", mode);
                let api_token = ApiToken {
                    access_token: token,
                    token_type: "Bearer".to_string(),
                    expires_in: 86400, // 24시간
                    access_token_token_expired: "2024-12-31 23:59:59".to_string(),
                    issued_at: Utc::now(),
                    approval_key: Some(approval_key),
                };
                
                token_manager.update_token(api_type, api_token)?;
                info!("토큰이 저장되었습니다 (기본값 사용): {:?}", mode);
            }
        }

        info!(
            "🔗 [KoreaApi] {} API 연결 완료",
            match mode {
                ApiMode::Real => "실거래",
                ApiMode::Paper => "모의투자",
                ApiMode::Info => "정보용 실전 API",
            }
        );

        Ok(Self {
            mode,
            api: RefCell::new(Rc::new(api)),
            token_manager,
            order_branch_map: RefCell::new(HashMap::new()),
        })
    }

    fn mode_name(&self) -> &'static str {
        match self.mode {
            ApiMode::Real => "실거래",
            ApiMode::Paper => "모의투자",
            ApiMode::Info => "정보용 실전 API",
        }
    }
}

impl StockApi for KoreaApi {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn execute_order(&self, order: &mut Order) -> StockrsResult<String> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            let result = self
                .call_with_token_refresh(
                    "주문 실행",
                    || async {
                        // Order 구조체를 korea-investment-api 파라미터로 변환 (매 시도 시 계산)
                        let dir = match order.side {
                            crate::utility::types::broker::OrderSide::Buy => korea_investment_api::types::Direction::Bid,
                            crate::utility::types::broker::OrderSide::Sell => korea_investment_api::types::Direction::Ask,
                        };
                        let api = { self.api.borrow().clone() };
                        let out = api
                            .order
                            .order_cash(
                                korea_investment_api::types::OrderClass::Market,
                                dir,
                                &order.stockcode,
                                korea_investment_api::types::Quantity::from(order.quantity),
                                korea_investment_api::types::Price::from(0),
                            )
                            .await
                            .map_err(StockrsError::from)?;
                        // 응답 상태 검증: 비정상(rt_cd != "0") 또는 핵심 필드 누락은 오류로 간주하여 재시도 분기(레이트리미트 등)에 태움
                        if out.rt_cd() != "0" || out.output().is_none() {
                            return Err(StockrsError::api(format!(
                                "KIS 주문 오류: rt_cd={}, msg_cd={}, msg1={}",
                                out.rt_cd(),
                                out.msg_cd(),
                                out.msg1()
                            )));
                        }
                        Ok(out)
                    },
                )
                .await?;

            let order_id = result
                .output()
                .as_ref()
                .ok_or_else(|| {
                    StockrsError::order_execution(
                        match order.side {
                            crate::utility::types::broker::OrderSide::Buy => "매수",
                            crate::utility::types::broker::OrderSide::Sell => "매도",
                        },
                        &order.stockcode,
                        order.quantity,
                        "API 응답에서 주문번호를 찾을 수 없음",
                    )
                })?
                .odno()
                .clone();

            // 주문 응답에서 지점/영업점 코드 추출 후 맵에 저장
            if let Some(out) = result.output().as_ref() {
                let branch = out.krx_fwdg_ord_orgno();
                self.order_branch_map
                    .borrow_mut()
                    .insert(order_id.clone(), branch.to_string());
                println!(
                    "🏷️ [KoreaApi:{}] 주문ID:{} 영업점코드 저장: {}",
                    self.mode_name(),
                    order_id,
                    branch
                );
            }

            println!(
                "📈 [KoreaApi:{}] 주문 실행: {} {} {}주 -> 주문번호: {}",
                self.mode_name(),
                order.stockcode,
                match order.side {
                    crate::utility::types::broker::OrderSide::Buy => "매수",
                    crate::utility::types::broker::OrderSide::Sell => "매도",
                },
                order.quantity,
                order_id
            );

            Ok(order_id)
        })
    }

    fn check_fill(&self, order_id: &str) -> StockrsResult<bool> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            let today = chrono::Local::now().format("%Y%m%d").to_string();

            let result = self
                .call_with_token_refresh(
                    "체결 조회",
                    || async {
                        let api = { self.api.borrow().clone() };
                        let out = api
                            .order
                            .inquire_daily_ccld(
                                &today, &today, "", "", "", order_id, "01", "00", "", "", "01", None, None,
                            )
                            .await
                            .map_err(StockrsError::from)?;
                        Ok(out)
                    },
                )
                .await?;

            let is_filled = !result
                .output1()
                .as_ref()
                .ok_or_else(|| StockrsError::OrderFillCheck {
                    order_id: order_id.to_string(),
                    reason: "API 응답에서 체결 정보를 찾을 수 없음".to_string(),
                })?
                .is_empty();

            println!(
                "🔍 [KoreaApi:{}] 체결 확인: 주문번호 {} -> {}",
                self.mode_name(),
                order_id,
                if is_filled { "체결됨" } else { "미체결" }
            );

            Ok(is_filled)
        })
    }

    fn cancel_order(&self, order_id: &str) -> StockrsResult<()> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            let _result = self
                .call_with_token_refresh(
                    "주문 취소",
                    || async {
                        let api = { self.api.borrow().clone() };
                        let out = api
                            .order
                            .correct(
                                korea_investment_api::types::OrderClass::Market,
                                "",
                                order_id,
                                korea_investment_api::types::CorrectionClass::Cancel,
                                true,
                                korea_investment_api::types::Quantity::from(0),
                                korea_investment_api::types::Price::from(0),
                            )
                            .await
                            .map_err(StockrsError::from)?;
                        if out.rt_cd() != "0" {
                            return Err(StockrsError::api(format!(
                                "KIS 주문 취소 오류: rt_cd={}, msg_cd={}, msg1={}",
                                out.rt_cd(),
                                out.msg_cd(),
                                out.msg1()
                            )));
                        }
                        Ok(out)
                    },
                )
                .await?;

            println!("❌ [KoreaApi:{}] 주문 취소: {}", self.mode_name(), order_id);
            Ok(())
        })
    }

    fn get_balance(&self) -> StockrsResult<AssetInfo> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            let result = self
                .call_with_token_refresh(
                    "잔고 조회",
                    || async {
                        let api = { self.api.borrow().clone() };
                        let out = api
                            .order
                            .inquire_balance("N", "02", "01", "N", "N", "00", None, None)
                            .await
                            .map_err(StockrsError::from)?;

                        // KIS 응답 본문이 에러이거나 핵심 출력이 비어있다면 재시도 대상으로 간주
                        let missing_output2 = out
                            .output2()
                            .as_ref()
                            .map(|v| v.is_empty())
                            .unwrap_or(true);
                        if out.rt_cd() != "0" || missing_output2 {
                            return Err(StockrsError::api(format!(
                                "KIS 잔고 조회 오류: rt_cd={}, msg_cd={}, msg1={}",
                                out.rt_cd(),
                                out.msg_cd(),
                                out.msg1()
                            )));
                        }

                        Ok(out)
                    },
                )
                .await?;

            let output2 = result
                .output2()
                .as_ref()
                .and_then(|output2_vec| output2_vec.first())
                .ok_or_else(|| StockrsError::BalanceInquiry {
                    reason: "API 응답에서 잔고 정보를 찾을 수 없음".to_string(),
                })?;

            // 주문 가능 금액 (D+2 예수금) 조회
            let amt_str = output2.dnca_tot_amt();
            let available_amount = amt_str
                .parse::<f64>()
                .map_err(|parse_err| StockrsError::Parsing {
                    data_type: "주문가능금액(D+2 예수금)".to_string(),
                    reason: format!("'{}'를 숫자로 변환 실패: {}", amt_str, parse_err),
                })?;

            // 유가증권 평가금액 조회
            let scts_evlu_amt_str = output2.scts_evlu_amt();
            let securities_value = scts_evlu_amt_str
                .parse::<f64>()
                .map_err(|parse_err| StockrsError::Parsing {
                    data_type: "유가증권 평가금액".to_string(),
                    reason: format!("'{}'를 숫자로 변환 실패: {}", scts_evlu_amt_str, parse_err),
                })?;

            // 총평가금액 (참고용)
            let tot_evlu_amt_str = output2.tot_evlu_amt();
            let total_asset = tot_evlu_amt_str
                .parse::<f64>()
                .map_err(|parse_err| StockrsError::Parsing {
                    data_type: "총평가금액".to_string(),
                    reason: format!("'{}'를 숫자로 변환 실패: {}", tot_evlu_amt_str, parse_err),
                })?;

            use chrono::Local;
            let now = Local::now().naive_local();
            
            println!(
                "💰 [KoreaApi:{}] 잔고 조회 완료 - 주문가능: {:.0}원, 유가증권: {:.0}원, 총평가금액: {:.0}원",
                self.mode_name(),
                available_amount,
                securities_value,
                total_asset
            );

            Ok(AssetInfo::new_with_api_total(now, available_amount, securities_value, total_asset))
        })
    }

    fn get_avg_price(&self, stockcode: &str) -> StockrsResult<f64> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            let result = self
                .call_with_token_refresh(
                    "평균가/잔고 조회",
                    || async {
                        let api = { self.api.borrow().clone() };
                        let out = api
                            .order
                            .inquire_balance("N", "02", "01", "N", "N", "00", None, None)
                            .await
                            .map_err(StockrsError::from)?;

                        // KIS 응답 본문이 에러이거나 핵심 출력이 비어있다면 재시도 대상으로 간주
                        let missing_output1 = out
                            .output1()
                            .as_ref()
                            .map(|v| v.is_empty())
                            .unwrap_or(true);
                        if out.rt_cd() != "0" || missing_output1 {
                            return Err(StockrsError::api(format!(
                                "KIS 잔고/보유종목 조회 오류: rt_cd={}, msg_cd={}, msg1={}",
                                out.rt_cd(),
                                out.msg_cd(),
                                out.msg1()
                            )));
                        }

                        Ok(out)
                    },
                )
                .await?;

            let output1 =
                result
                    .output1()
                    .as_ref()
                    .ok_or_else(|| StockrsError::BalanceInquiry {
                        reason: "API 응답에서 보유 종목 목록을 찾을 수 없음".to_string(),
                    })?;

            let holding_item = output1
                .iter()
                .find(|item| item.pdno() == stockcode)
                .ok_or_else(|| {
                    StockrsError::price_inquiry(
                        stockcode,
                        "평균가",
                        format!(
                            "보유하지 않은 종목입니다 (총 {}개 보유 종목 중 없음)",
                            output1.len()
                        ),
                    )
                })?;

            let price_str = holding_item.pchs_avg_pric();
            let avg_price =
                price_str
                    .parse::<f64>()
                    .map_err(|parse_err| StockrsError::Parsing {
                        data_type: format!("{} 평균가", stockcode),
                        reason: format!("'{}'를 숫자로 변환 실패: {}", price_str, parse_err),
                    })?;

            // 평균가 조회는 상위 레이어가 목적에 맞게 출력
            Ok(avg_price)
        })
    }

    fn get_current_price(&self, stockcode: &str) -> StockrsResult<f64> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            // 종목코드에서 'A' 제거
            let clean_stockcode = if stockcode.starts_with('A') {
                &stockcode[1..]
            } else {
                stockcode
            };
            
            let result = self
                .call_with_token_refresh(
                    "현재가 조회",
                    || async {
                        let api = { self.api.borrow().clone() };
                        let out = api
                            .quote
                            .current_price(
                                korea_investment_api::types::MarketCode::Stock,
                                clean_stockcode,
                            )
                            .await
                            .map_err(StockrsError::from)?;

                        // KIS 응답 본문이 에러이거나 핵심 출력이 비어있다면 재시도 대상으로 간주
                        let has_output = out.output().is_some();
                        if out.rt_cd() != "0" || !has_output {
                            return Err(StockrsError::api(format!(
                                "KIS 현재가 조회 오류: rt_cd={}, msg_cd={}, msg1={}",
                                out.rt_cd(),
                                out.msg_cd(),
                                out.msg1()
                            )));
                        }

                        Ok(out)
                    },
                )
                .await?;

            let output = result.output().as_ref().ok_or_else(|| {
                StockrsError::price_inquiry(
                    stockcode,
                    "현재가",
                    "API 응답에서 가격 데이터를 찾을 수 없음".to_string(),
                )
            })?;

            let price_str = output.stck_prpr();
            let current_price = price_str.parse::<f64>().map_err(|parse_err| StockrsError::Parsing {
                data_type: format!("{} 현재가", stockcode),
                reason: format!("'{}'를 숫자로 변환 실패: {}", price_str, parse_err),
            })?;

            Ok(current_price)
        })
    }

    fn set_current_time(&self, _time_str: &str) -> StockrsResult<()> {
        // KoreaApi는 백테스팅 모드가 아니므로 아무것도 하지 않음
        Ok(())
    }

    fn get_current_price_at_time(&self, _stockcode: &str, _time_str: &str) -> StockrsResult<f64> {
        // KoreaApi는 백테스팅 모드가 아니므로 지원하지 않음
        Err(StockrsError::UnsupportedFeature {
            feature: "시간 기반 현재가 조회".to_string(),
            phase: "실시간/모의투자 모드".to_string(),
        })
    }

    fn get_db_connection(&self) -> Option<rusqlite::Connection> {
        // KoreaApi는 DB 연결을 제공하지 않음
        None
    }

    fn get_daily_db_connection(&self) -> Option<rusqlite::Connection> {
        // KoreaApi는 일봉 DB 연결을 제공하지 않음
        None
    }
}

impl KoreaApi {
    /// 거래대금 순위 상위 종목 조회 (실전/모의 투자용)
    pub fn get_top_amount_stocks(&self, limit: usize) -> StockrsResult<Vec<String>> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            // 한국투자 시세 API의 거래량순위 엔드포인트를 거래대금 기준으로 설정하여 사용
            let result = self
                .call_with_token_refresh(
                    "거래대금 순위 조회",
                    || async {
                        let api = { self.api.borrow().clone() };
                        let params = korea_investment_api::types::request::stock::quote::VolumeRankParameter::new(
                            "0000".to_string(), // 전체 시장
                            korea_investment_api::types::ShareClassCode::Whole,
                            korea_investment_api::types::BelongClassCode::Amount, // 거래금액순
                            korea_investment_api::types::TargetClassCode {
                                margin_30: false,
                                margin_40: false,
                                margin_50: false,
                                margin_60: false,
                                margin_100: false,
                                credit_30: false,
                                credit_40: false,
                                credit_50: false,
                                credit_60: false,
                            },
                            korea_investment_api::types::TargetExeceptClassCode {
                                overheat: false,
                                administrated: false,
                                settlement_trading: false,
                                insufficient_posting: false,
                                preferred_share: false,
                                suspended: false,
                            },
                            None,
                            None,
                            None,
                        );

                        let out = api
                            .quote
                            .volume_rank(params)
                            .await
                            .map_err(StockrsError::from)?;

                        // 비정상 응답은 에러 처리하여 상위 재시도 경로로 진입
                        let has_output = out.output().is_some();
                        if out.rt_cd() != "0" || !has_output {
                            return Err(StockrsError::api(format!(
                                "KIS 거래대금 순위 조회 오류: rt_cd={}, msg_cd={}, msg1={}",
                                out.rt_cd(),
                                out.msg_cd(),
                                out.msg1()
                            )));
                        }

                        Ok(out)
                    },
                )
                .await?;

            let rows = out_put_list(&result)?;

            // acml_tr_pbmn(누적 거래대금) 기준으로 내림차순 정렬 후 상위 limit 추출
            let mut items: Vec<(String, f64)> = Vec::with_capacity(rows.len());
            for row in rows.iter() {
                let code = row.mksc_shrn_iscd().to_string();
                let amt_str = row.acml_tr_pbmn();
                let amt = amt_str.trim().parse::<f64>().map_err(|e| StockrsError::Parsing {
                    data_type: "누적 거래대금".to_string(),
                    reason: format!("'{}'를 숫자로 변환 실패: {}", amt_str, e),
                })?;
                items.push((code, amt));
            }

            items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_codes: Vec<String> = items
                .into_iter()
                .map(|(code, _)| code)
                .take(limit)
                .collect();

            println!(
                "🏆 [KoreaApi:{}] 거래대금 상위 {}종목: {}",
                self.mode_name(),
                top_codes.len(),
                top_codes.join(", ")
            );

            Ok(top_codes)
        })
    }

    /// 토큰 상태 정보 출력
    pub fn print_token_status(&self) -> StockrsResult<()> {
        self.token_manager.print_token_status()
    }

    /// 토큰 관리자 참조 가져오기
    pub fn get_token_manager(&self) -> &TokenManager {
        &self.token_manager
    }

    // OrderFillInfo는 상단 모듈 스코프로 이동됨

    /// 주문번호 기반 체결 상세 조회 (주식일별주문체결조회)
    pub fn get_order_fill_info(&self, order_id: &str) -> StockrsResult<Option<OrderFillInfo>> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async move {
            let today = chrono::Local::now().format("%Y%m%d").to_string();
            // 파라미터 매핑 (실전/모의 동일 정책, 모의는 KRX만 제공)
            let sll_buy_cd = "00";        // SLL_BUY_DVSN_CD: 전체
            let pdno = "";               // PDNO: 전체
            // ORD_GNO_BRNO: 주문시 부여된 지점코드를 사용. 모르면 기본 '00000'
            let ord_gno_brno = self
                .order_branch_map
                .borrow()
                .get(order_id)
                .cloned()
                .unwrap_or_else(|| "00000".to_string());
            let ccld_dvsn = "00";        // CCLD_DVSN: 전체
            let inqr_dvsn = "00";        // INQR_DVSN: 역순
            let inqr_dvsn_1 = "";        // INQR_DVSN_1: 전체 (명세상 '없음' → 빈 문자열)
            let inqr_dvsn_3 = "00";      // INQR_DVSN_3: 전체
            let excg_id = "01";          // EXCG_ID_DVSN_CD: KRX (모의투자는 KRX만 지원)
            // 요청 파라미터(의사 요청 본문) 디버그 문자열 구성
            let req_debug = format!(
                "INQR_STRT_DT={sd}, INQR_END_DT={ed}, SLL_BUY_DVSN_CD={bs}, PDNO={pd}, ORD_GNO_BRNO={br}, ODNO={oid}, CCLD_DVSN={cd}, INQR_DVSN={iv}, INQR_DVSN_1={iv1}, INQR_DVSN_3={iv3}, EXCG_ID_DVSN_CD={ex}",
                sd = today,
                ed = today,
                bs = sll_buy_cd,
                pd = if pdno.is_empty() { "<ALL>" } else { pdno },
                br = ord_gno_brno,
                oid = order_id,
                cd = ccld_dvsn,
                iv = inqr_dvsn,
                iv1 = if inqr_dvsn_1.is_empty() { "<ALL>" } else { inqr_dvsn_1 },
                iv3 = inqr_dvsn_3,
                ex = excg_id
            );
            println!(
                "🧾 [KoreaApi:{}] 체결조회 지점코드 사용 - ODNO:{} ORD_GNO_BRNO:{}",
                self.mode_name(),
                order_id,
                ord_gno_brno
            );
            let result = self
                .call_with_token_refresh(
                    "주문 체결 상세 조회",
                    || async {
                        let api = { self.api.borrow().clone() };
                        let out = api
                            .order
                            .inquire_daily_ccld(
                                &today,         // INQR_STRT_DT
                                &today,         // INQR_END_DT
                                sll_buy_cd,     // SLL_BUY_DVSN_CD
                                pdno,           // PDNO
                                &ord_gno_brno,  // ORD_GNO_BRNO
                                order_id,       // ODNO
                                ccld_dvsn,      // CCLD_DVSN
                                inqr_dvsn,      // INQR_DVSN
                                inqr_dvsn_1,    // INQR_DVSN_1
                                inqr_dvsn_3,    // INQR_DVSN_3
                                excg_id,        // EXCG_ID_DVSN_CD
                                None,
                                None,
                            )
                            .await
                            .map_err(StockrsError::from)?;
                        // 레이트리미트 등 비정상 응답은 즉시 오류로 전파하여 상위 재시도 경로(with_rate_limit_retry)로 진입
                        let missing_output1 = out
                            .output1()
                            .as_ref()
                            .map(|v| v.is_empty())
                            .unwrap_or(true);
                        if out.rt_cd() != "0" || missing_output1 {
                            return Err(StockrsError::api(format!(
                                "KIS 주문체결 조회 오류: rt_cd={}, msg_cd={}, msg1={}",
                                out.rt_cd(),
                                out.msg_cd(),
                                out.msg1()
                            )));
                        }
                        Ok(out)
                    },
                )
                .await?;

            // 핵심 출력(output1) 누락 시 요청/응답 본문 출력 후 오류 처리
            let output1 = match result.output1().as_ref() {
                Some(o) => o,
                None => {
                    println!(
                        "❌ [KoreaApi:{}] 주문 체결 상세 조회 요청: {}",
                        self.mode_name(),
                        req_debug
                    );
                    println!(
                        "❌ [KoreaApi:{}] 응답 요약: rt_cd={}, msg_cd={}, msg1={}",
                        self.mode_name(),
                        result.rt_cd(),
                        result.msg_cd(),
                        result.msg1()
                    );
                    return Err(StockrsError::OrderFillCheck {
                        order_id: order_id.to_string(),
                        reason: "API 응답에서 체결 정보(output1)를 찾을 수 없음".to_string(),
                    });
                }
            };

            // 주문번호에 해당하는 행 탐색 (없으면 아직 미반영 상태로 간주하여 None 반환)
            if let Some(row) = output1.iter().find(|row| row.odno() == order_id) {
                // 엄격한 파싱: 실패 시 요청/응답 본문과 문제 필드 함께 출력 후 오류 반환
                let parse_u32_strict = |s: &str, field: &str| -> StockrsResult<u32> {
                    match s.trim().parse::<u32>() {
                        Ok(v) => Ok(v),
                        Err(e) => {
                            println!(
                                "❌ [KoreaApi:{}] 주문 체결 상세 조회 요청: {}",
                                self.mode_name(),
                                req_debug
                            );
                            println!(
                                "❌ [KoreaApi:{}] 응답 요약: rt_cd={}, msg_cd={}, msg1={}",
                                self.mode_name(),
                                result.rt_cd(),
                                result.msg_cd(),
                                result.msg1()
                            );
                            println!(
                                "❌ [KoreaApi:{}] 파싱 실패 필드: {}='{}' (order_id={})",
                                self.mode_name(),
                                field,
                                s,
                                order_id
                            );
                            Err(StockrsError::Parsing { data_type: field.to_string(), reason: e.to_string() })
                        }
                    }
                };

                let parse_f64_strict = |s: &str, field: &str| -> StockrsResult<f64> {
                    match s.trim().parse::<f64>() {
                        Ok(v) => Ok(v),
                        Err(e) => {
                            println!(
                                "❌ [KoreaApi:{}] 주문 체결 상세 조회 요청: {}",
                                self.mode_name(),
                                req_debug
                            );
                            println!(
                                "❌ [KoreaApi:{}] 응답 요약: rt_cd={}, msg_cd={}, msg1={}",
                                self.mode_name(),
                                result.rt_cd(),
                                result.msg_cd(),
                                result.msg1()
                            );
                            println!(
                                "❌ [KoreaApi:{}] 파싱 실패 필드: {}='{}' (order_id={})",
                                self.mode_name(),
                                field,
                                s,
                                order_id
                            );
                            Err(StockrsError::Parsing { data_type: field.to_string(), reason: e.to_string() })
                        }
                    }
                };

                let ord_qty = parse_u32_strict(row.ord_qty(), "주문 수량")?;
                let tot_ccld_qty = parse_u32_strict(row.tot_ccld_qty(), "총 체결 수량")?;
                let rmn_qty = parse_u32_strict(row.rmn_qty(), "잔여 수량")?;
                let ord_unpr = parse_f64_strict(row.ord_unpr(), "주문 단가")?;
                let avg_prvs = parse_f64_strict(row.avg_prvs(), "평균가")?;

                let info = OrderFillInfo {
                    ord_dt: row.ord_dt().to_string(),
                    ord_tmd: row.ord_tmd().to_string(),
                    pdno: row.pdno().to_string(),
                    ord_qty,
                    tot_ccld_qty,
                    rmn_qty,
                    ord_unpr,
                    avg_prvs,
                };
                Ok(Some(info))
            } else {
                // 응답은 정상이나 주문번호가 아직 반영되지 않은 경우: 요청/응답 요약과 일부 샘플을 함께 출력
                println!(
                    "ℹ️ [KoreaApi:{}] 주문 체결 상세 조회 요청: {}",
                    self.mode_name(),
                    req_debug
                );
                println!(
                    "ℹ️ [KoreaApi:{}] 응답 요약: rt_cd={}, msg_cd={}, msg1={}, rows={}",
                    self.mode_name(),
                    result.rt_cd(),
                    result.msg_cd(),
                    result.msg1(),
                    output1.len()
                );
                // 최대 3개 행만 요약 출력
                for (idx, row) in output1.iter().take(3).enumerate() {
                    println!(
                        "   ├─[{}] odno:{} pdno:{} ord_qty:{} tot_ccld_qty:{} rmn_qty:{} ord_unpr:{} avg_prvs:{}",
                        idx,
                        row.odno(), row.pdno(), row.ord_qty(), row.tot_ccld_qty(), row.rmn_qty(), row.ord_unpr(), row.avg_prvs()
                    );
                }

                Ok(None)
            }
        })
    }
}

impl KoreaApi {
    /// 당일 분봉 차트 조회 (시작 시간 기준, 과거 데이터 포함 여부 선택)
    /// 반환: (date: YYYYMMDD, time: HHMMSS, close, open, high, low, volume, amount)
    pub fn get_minute_price_chart(
        &self,
        stockcode: &str,
        input_hour_hhmmss: &str,
        include_past_data: bool,
    ) -> StockrsResult<Vec<(String, String, f64, f64, f64, f64, f64, f64)>> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            let clean_stockcode: String = if stockcode.starts_with('A') {
                stockcode[1..].to_string()
            } else {
                stockcode.to_string()
            };

            let result = self
                .call_with_token_refresh(
                    "당일 분봉 차트 조회",
                    || async {
                        let api = { self.api.borrow().clone() };
                        let params = korea_investment_api::types::request::stock::quote::MinutePriceChartParameter::new(
                            "J",                    // KRX
                            &clean_stockcode,        // 종목코드(단축)
                            input_hour_hhmmss,      // 시작 시각(HHMMSS)
                            include_past_data,      // 과거 데이터 포함
                            "0",                   // 기본 구분 코드
                        );
                        let out = api
                            .quote
                            .minute_price_chart(params)
                            .await
                            .map_err(StockrsError::from)?;

                        // 응답 검증: rt_cd=='0' && output2(Some)
                        let has_output2 = out.output2().is_some();
                        if out.rt_cd() != "0" || !has_output2 {
                            return Err(StockrsError::api(format!(
                                "KIS 분봉 조회 오류: rt_cd={}, msg_cd={}, msg1={}",
                                out.rt_cd(),
                                out.msg_cd(),
                                out.msg1()
                            )));
                        }
                        Ok(out)
                    },
                )
                .await?;

            let output2 = result.output2().as_ref().ok_or_else(|| {
                StockrsError::general("API 응답에서 분봉 데이터(output2)를 찾을 수 없음".to_string())
            })?;

            let mut rows: Vec<(String, String, f64, f64, f64, f64, f64, f64)> =
                Vec::with_capacity(output2.len());
            for r in output2.iter() {
                let date = r.stck_bsop_date().to_string();
                let time = r.stck_cntg_hour().to_string();
                let close = r.stck_prpr().trim().parse::<f64>().map_err(|e| StockrsError::Parsing {
                    data_type: "분봉:현재가".to_string(),
                    reason: e.to_string(),
                })?;
                let open = r.stck_oprc().trim().parse::<f64>().map_err(|e| StockrsError::Parsing {
                    data_type: "분봉:시가".to_string(),
                    reason: e.to_string(),
                })?;
                let high = r.stck_hgpr().trim().parse::<f64>().map_err(|e| StockrsError::Parsing {
                    data_type: "분봉:고가".to_string(),
                    reason: e.to_string(),
                })?;
                let low = r.stck_lwpr().trim().parse::<f64>().map_err(|e| StockrsError::Parsing {
                    data_type: "분봉:저가".to_string(),
                    reason: e.to_string(),
                })?;
                let volume = r.cntg_vol().trim().parse::<f64>().map_err(|e| StockrsError::Parsing {
                    data_type: "분봉:체결거래량".to_string(),
                    reason: e.to_string(),
                })?;
                let amount = r.acml_tr_pbmn().trim().parse::<f64>().map_err(|e| StockrsError::Parsing {
                    data_type: "분봉:누적거래대금".to_string(),
                    reason: e.to_string(),
                })?;

                rows.push((date, time, close, open, high, low, volume, amount));
            }

            Ok(rows)
        })
    }
}

// 내부 헬퍼: VolumeRank 응답에서 핵심 리스트(output) 추출
fn out_put_list<'a>(resp: &'a korea_investment_api::types::response::stock::quote::VolumeRankResponse) -> StockrsResult<&'a Vec<korea_investment_api::types::response::stock::quote::output::VolumeRank>> {
    resp
        .output()
        .as_ref()
        .ok_or_else(|| StockrsError::general("API 응답에서 거래대금 순위(output)를 찾을 수 없음".to_string()))
}
