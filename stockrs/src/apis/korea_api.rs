use crate::errors::{StockrsError, StockrsResult};
use crate::types::api::StockApi;
use crate::types::broker::Order;
use crate::types::trading::AssetInfo;
use std::any::Any;
use std::rc::Rc;

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
        let config = crate::config::get_config()?;

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
            None,
            None,
        )
        .await?;

        println!(
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
            let direction = match order.side {
                crate::types::broker::OrderSide::Buy => korea_investment_api::types::Direction::Bid,
                crate::types::broker::OrderSide::Sell => {
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
                            crate::types::broker::OrderSide::Buy => "매수",
                            crate::types::broker::OrderSide::Sell => "매도",
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
                    crate::types::broker::OrderSide::Buy => "매수",
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

        rt.block_on(async {
            let result = api
                .quote
                .daily_price(
                    korea_investment_api::types::MarketCode::Stock,
                    stockcode,
                    korea_investment_api::types::PeriodCode::ThirtyDays,
                    false,
                )
                .await?;

            let output = result.output().as_ref().ok_or_else(|| {
                StockrsError::price_inquiry(
                    stockcode,
                    "현재가",
                    "API 응답에서 가격 데이터를 찾을 수 없음".to_string(),
                )
            })?;

            let first_day = output.first().ok_or_else(|| {
                StockrsError::price_inquiry(
                    stockcode,
                    "현재가",
                    "가격 데이터가 비어있음 (거래일이 아니거나 종목 코드 오류)".to_string(),
                )
            })?;

            let price_str = first_day.stck_clpr();
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
        let api = Rc::clone(&self.api);

        rt.block_on(async {
            // 거래대금순 조회 파라미터 설정
            let params =
                korea_investment_api::types::request::stock::quote::VolumeRankParameter::new(
                    "0000".to_string(),                                   // 전체 종목
                    korea_investment_api::types::ShareClassCode::Whole,   // 전체 (보통주 + 우선주)
                    korea_investment_api::types::BelongClassCode::Amount, // 거래금액순
                    korea_investment_api::types::TargetClassCode {
                        margin_30: true,
                        margin_40: true,
                        margin_50: true,
                        margin_60: true,
                        margin_100: true,
                        credit_30: true,
                        credit_40: true,
                        credit_50: true,
                        credit_60: true,
                    }, // 모든 대상 포함
                    korea_investment_api::types::TargetExeceptClassCode {
                        overheat: false,
                        administrated: false,
                        settlement_trading: false,
                        insufficient_posting: false,
                        preferred_share: false,
                        suspended: false,
                    }, // 예외 없음
                    None,                                                 // 최소 가격 제한 없음
                    None,                                                 // 최대 가격 제한 없음
                    None,                                                 // 거래량 제한 없음
                );

            let result = api.quote.volume_rank(params).await?;

            let output = result.output().as_ref().ok_or_else(|| {
                StockrsError::api("거래대금 순위 API 응답에서 데이터를 찾을 수 없음".to_string())
            })?;

            // 상위 limit개 종목 코드 추출
            let top_stocks: Vec<String> = output
                .iter()
                .take(limit)
                .map(|item| item.mksc_shrn_iscd().to_string())
                .collect();

            println!(
                "💰 [KoreaApi:{}] 거래대금 상위 {}개 종목 조회 완료",
                self.mode_name(),
                top_stocks.len()
            );

            Ok(top_stocks)
        })
    }
}
