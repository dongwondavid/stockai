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
            account,
            "HTS_ID",
            token,
            approval_key,
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
            // Order 구조체를 korea-investment-api 파라미터로 변환
            let direction =             match order.side {
                crate::utility::types::broker::OrderSide::Buy => korea_investment_api::types::Direction::Bid,
                crate::utility::types::broker::OrderSide::Sell => {
                    korea_investment_api::types::Direction::Ask
                }
            };

            let result = api
                .order
                .order_cash(
                    korea_investment_api::types::OrderClass::Market,
                    direction,
                    &order.stockcode,
                    korea_investment_api::types::Quantity::from(order.quantity),
                    korea_investment_api::types::Price::from(0), // 시장가
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
                        crate::utility::types::broker::OrderSide::Buy => "매도",
                    _ => "매도",
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

            let result = api
                .order
                .inquire_daily_ccld(
                    &today, &today, "", "", "", order_id, "01", "00", "", "", "01", None, None,
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
            let _result = api
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
                .await?;

            println!("❌ [KoreaApi:{}] 주문 취소: {}", self.mode_name(), order_id);
            Ok(())
        })
    }

    fn get_balance(&self) -> StockrsResult<AssetInfo> {
        let rt = tokio::runtime::Runtime::new()?;
        let api = Rc::clone(&self.api);

        rt.block_on(async {
            let result = api
                .order
                .inquire_balance("N", "02", "01", "N", "N", "00", None, None)
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

            println!(
                "💰 [KoreaApi:{}] 잔고 조회: 예수금 {}원",
                self.mode_name(),
                total_cash
            );

            use chrono::Local;
            Ok(AssetInfo::new(Local::now().naive_local(), total_cash))
        })
    }

    fn get_avg_price(&self, stockcode: &str) -> StockrsResult<f64> {
        let rt = tokio::runtime::Runtime::new()?;
        let api = Rc::clone(&self.api);

        rt.block_on(async {
            let result = api
                .order
                .inquire_balance("N", "02", "01", "N", "N", "00", None, None)
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

            println!(
                "📊 [KoreaApi:{}] 평균가 조회: {} -> {}원",
                self.mode_name(),
                stockcode,
                avg_price
            );
            Ok(avg_price)
        })
    }

    fn get_current_price(&self, stockcode: &str) -> StockrsResult<f64> {
        let rt = tokio::runtime::Runtime::new()?;
        let api = Rc::clone(&self.api);

        Ok(rt.block_on(async {
            // 종목코드에서 'A' 제거
            let clean_stockcode = if stockcode.starts_with('A') {
                &stockcode[1..]
            } else {
                stockcode
            };
            
            // 재시도 로직 (최대 3회)
            let mut retry_count = 0;
            let max_retries = 3;
            
            let result = loop {
                let api_result = api
                    .quote
                    .current_price(
                        korea_investment_api::types::MarketCode::Stock,
                        clean_stockcode,
                    )
                    .await;
                
                match api_result {
                    Ok(result) => break result,
                    Err(e) => {
                        let error_msg = e.to_string();
                        if error_msg.contains("EGW00201") && retry_count < max_retries {
                            retry_count += 1;
                            println!(
                                "⚠️ [KoreaApi:{}] 초당 거래건수 초과 (EGW00201) - 1초 대기 후 재시도 ({}/{})",
                                self.mode_name(),
                                retry_count,
                                max_retries
                            );
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            continue;
                        } else {
                            return Err(StockrsError::api(format!("Korea Investment API 오류: {}", e)));
                        }
                    }
                }
            };

                    let output = result.output().as_ref().ok_or_else(|| {
            StockrsError::price_inquiry(
                stockcode,
                "현재가",
                "API 응답에서 가격 데이터를 찾을 수 없음".to_string(),
            )
        })?;

        let price_str = output.stck_prpr();
            let current_price =
                price_str
                    .parse::<f64>()
                    .map_err(|parse_err| StockrsError::Parsing {
                        data_type: format!("{} 현재가", stockcode),
                        reason: format!("'{}'를 숫자로 변환 실패: {}", price_str, parse_err),
                    })?;

            println!(
                "💹 [KoreaApi:{}] 현재가 조회: {} -> {}원",
                self.mode_name(),
                stockcode,
                current_price
            );
            Ok(current_price)
        })?)
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
}
