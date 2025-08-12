use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::types::api::StockApi;
use crate::utility::types::broker::Order;
use crate::utility::types::trading::AssetInfo;
use crate::utility::config;
use crate::utility::token_manager::{TokenManager, ApiToken};

use std::any::Any;
use std::rc::Rc;
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
    api: Rc<korea_investment_api::KoreaInvestmentApi>,
    token_manager: TokenManager,
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
            base_delay_ms: 1_000,
            max_delay_ms: 6_000,
            timeout_ms: 1_500,
            retry_on_error: true,
        }
    }
}

fn is_retryable_error_message(message: &str) -> bool {
    let m = message;
    // KIS rate limit and common transient HTTP errors
    m.contains("EGW00201")
        || m.contains("초당 거래건수")
        || m.contains("Too Many Requests")
        || m.contains("429")
        || m.contains("status=5")
        || m.contains("HTTP 5")
        || m.contains("gateway time-out")
        || m.contains("timed out")
}

async fn with_timeout_retry<T, Fut, F>(mode_name: &str, op_name: &str, mut make_future: F, policy: TimeoutRetryPolicy) -> StockrsResult<T>
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
                if policy.retry_on_error && attempt_allowed && is_retryable_error_message(&msg) {
                    let delay_ms = current_delay_ms;
                    println!(
                        "⚠️ [KoreaApi:{}] {} 오류 재시도({}/{}): {} (대기 {}ms)",
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
                if attempt_index > policy.max_retries {
                    return Err(StockrsError::Network {
                        operation: format!("{} ({})", op_name, mode_name),
                        reason: format!("요청 타임아웃(>{}ms) - 최대 재시도 초과 {}", policy.timeout_ms, policy.max_retries),
                    });
                }

                let delay_ms = current_delay_ms;
                println!(
                    "⏳ [KoreaApi:{}] {} 타임아웃 - 재시도 {}/{} (대기 {}ms)",
                    mode_name,
                    op_name,
                    attempt_index,
                    policy.max_retries,
                    delay_ms
                );

                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                // Exponential backoff up to max_delay_ms
                current_delay_ms = (current_delay_ms.saturating_mul(2)).min(policy.max_delay_ms);
                continue;
            }
        }
    }
}

impl KoreaApi {
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

        let api = with_timeout_retry(
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
            api: Rc::new(api),
            token_manager,
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
        let api = Rc::clone(&self.api);

        rt.block_on(async {
            let result = with_timeout_retry(
                self.mode_name(),
                "주문 실행",
                || async {
                    // Order 구조체를 korea-investment-api 파라미터로 변환 (클로저 내부에서 매 시도 시 계산)
                    let dir = match order.side {
                        crate::utility::types::broker::OrderSide::Buy => korea_investment_api::types::Direction::Bid,
                        crate::utility::types::broker::OrderSide::Sell => korea_investment_api::types::Direction::Ask,
                    };

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
                    Ok(out)
                },
                TimeoutRetryPolicy::default(),
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
        let api = Rc::clone(&self.api);

        rt.block_on(async {
            let today = chrono::Local::now().format("%Y%m%d").to_string();

            let result = with_timeout_retry(
                self.mode_name(),
                "체결 조회",
                || async {
                    let out = api
                        .order
                        .inquire_daily_ccld(
                            &today, &today, "", "", "", order_id, "01", "00", "", "", "01", None, None,
                        )
                        .await
                        .map_err(StockrsError::from)?;
                    Ok(out)
                },
                TimeoutRetryPolicy::default(),
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
        let api = Rc::clone(&self.api);

        rt.block_on(async {
            let _result = with_timeout_retry(
                self.mode_name(),
                "주문 취소",
                || async {
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
                    Ok(out)
                },
                TimeoutRetryPolicy::default(),
            )
            .await?;

            println!("❌ [KoreaApi:{}] 주문 취소: {}", self.mode_name(), order_id);
            Ok(())
        })
    }

    fn get_balance(&self) -> StockrsResult<AssetInfo> {
        let rt = tokio::runtime::Runtime::new()?;
        let api = Rc::clone(&self.api);

        rt.block_on(async {
            let result = with_timeout_retry(
                self.mode_name(),
                "잔고 조회",
                || async {
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
                TimeoutRetryPolicy::default(),
            )
            .await?;

            let output2 = result
                .output2()
                .as_ref()
                .and_then(|output2_vec| output2_vec.first())
                .ok_or_else(|| StockrsError::BalanceInquiry {
                    reason: "API 응답에서 잔고 정보를 찾을 수 없음".to_string(),
                })?;

            let amt_str = output2.dnca_tot_amt();
            let total_cash = amt_str
                .parse::<f64>()
                .map_err(|parse_err| StockrsError::Parsing {
                    data_type: "예수금 총액".to_string(),
                    reason: format!("'{}'를 숫자로 변환 실패: {}", amt_str, parse_err),
                })?;

            // println은 호출 목적에 따라 호출부에서 출력하도록 위임

            use chrono::Local;
            Ok(AssetInfo::new(Local::now().naive_local(), total_cash))
        })
    }

    fn get_avg_price(&self, stockcode: &str) -> StockrsResult<f64> {
        let rt = tokio::runtime::Runtime::new()?;
        let api = Rc::clone(&self.api);

        rt.block_on(async {
            let result = with_timeout_retry(
                self.mode_name(),
                "평균가/잔고 조회",
                || async {
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
                TimeoutRetryPolicy::default(),
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
        let api = Rc::clone(&self.api);

        rt.block_on(async {
            // 종목코드에서 'A' 제거
            let clean_stockcode = if stockcode.starts_with('A') {
                &stockcode[1..]
            } else {
                stockcode
            };
            
            let result = with_timeout_retry(
                self.mode_name(),
                "현재가 조회",
                || async {
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
                TimeoutRetryPolicy::default(),
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
    pub fn get_top_amount_stocks(&self, _limit: usize) -> StockrsResult<Vec<String>> {
        // TODO: 구현 필요
        Ok(vec![])
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
        let api = Rc::clone(&self.api);

        rt.block_on(async move {
            let today = chrono::Local::now().format("%Y%m%d").to_string();
            let result = with_timeout_retry(
                self.mode_name(),
                "주문 체결 상세 조회",
                || async {
                    let out = api
                        .order
                        .inquire_daily_ccld(
                            &today,     // 시작일
                            &today,     // 종료일
                            "",         // 매도매수구분 전체
                            "",         // 종목 전체
                            "",         // 지점 전체
                            order_id,   // 주문번호
                            "00",      // 체결구분 전체
                            "00",      // 조회구분 역순
                            "",        // 조회구분1 전체
                            "",        // 조회구분3 전체
                            "01",      // 거래소ID구분코드 (KRX)
                            None,
                            None,
                        )
                        .await
                        .map_err(StockrsError::from)?;
                    Ok(out)
                },
                TimeoutRetryPolicy::default(),
            )
            .await?;

            let maybe = result
                .output1()
                .as_ref()
                .and_then(|v| v.iter().find(|row| row.odno() == order_id))
                .cloned();

            if let Some(row) = maybe {
                // 안전한 파싱 유틸
                fn parse_u32(s: &str) -> u32 { s.trim().parse::<u32>().unwrap_or(0) }
                fn parse_f64(s: &str) -> f64 { s.trim().parse::<f64>().unwrap_or(0.0) }

                let info = OrderFillInfo {
                    ord_dt: row.ord_dt().to_string(),
                    ord_tmd: row.ord_tmd().to_string(),
                    pdno: row.pdno().to_string(),
                    ord_qty: parse_u32(row.ord_qty()),
                    tot_ccld_qty: parse_u32(row.tot_ccld_qty()),
                    rmn_qty: parse_u32(row.rmn_qty()),
                    ord_unpr: parse_f64(row.ord_unpr()),
                    avg_prvs: parse_f64(row.avg_prvs()),
                };
                Ok(Some(info))
            } else {
                Ok(None)
            }
        })
    }
}
