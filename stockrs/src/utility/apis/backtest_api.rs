use chrono::Utc;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::utility::config::get_config;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::time::TimeService;
use crate::utility::types::api::StockApi;
use crate::utility::types::broker::Order;
use crate::utility::types::trading::AssetInfo;

use std::rc::Rc;

/// 백테스팅용 보유 종목 정보
#[derive(Debug, Clone)]
struct Holding {
    quantity: u32,
    avg_price: f64,
    total_cost: f64,
}

/// 백테스팅 전용 API - 잔고 관리 및 주문 시뮬레이션 전담
pub struct BacktestApi {
    /// 백테스팅용 잔고 관리 (보유 종목, 현금)
    holdings: Mutex<HashMap<String, Holding>>,
    /// 현재 현금
    cash: Mutex<f64>,
    /// DB API 참조 (가격 조회용)
    db_api: Rc<dyn StockApi>,
}

impl BacktestApi {
    pub fn new(db_api: Rc<dyn StockApi>) -> StockrsResult<Self> {
        debug!("🔄 [BacktestApi::new] BacktestApi 초기화 시작");

        // config에서 초기 자본금 로드
        let config = get_config()?;
        let initial_capital = config.trading.initial_capital;

        info!(
            "💰 [BacktestApi::new] 백테스팅 초기 자본금: {:.0}원",
            initial_capital
        );

        debug!("✅ [BacktestApi::new] BacktestApi 초기화 완료");

        Ok(BacktestApi {
            holdings: Mutex::new(HashMap::new()),
            cash: Mutex::new(initial_capital),
            db_api,
        })
    }

    /// TimeService에서 현재 시간을 YYYYMMDDHHMM 형식으로 조회
    fn get_current_time(&self) -> StockrsResult<String> {
        TimeService::global_format_ymdhm()
    }

    /// 백테스팅용 잔고 계산 (현재 시간 기준)
    fn calculate_balance(&self) -> StockrsResult<AssetInfo> {
        debug!("🔄 [BacktestApi::calculate_balance] 잔고 계산 시작");

        // 백테스팅 모드에서는 현재 시간의 가격을 사용
        let current_time = self.get_current_time()?;

        let holdings = self
            .holdings
            .lock()
            .map_err(|e| StockrsError::general(format!("잔고 계산 중 뮤텍스 오류: {}", e)))?;

        let cash = self
            .cash
            .lock()
            .map_err(|e| StockrsError::general(format!("현금 조회 중 뮤텍스 오류: {}", e)))?;

        let mut total_asset = *cash;

        // 보유 종목 평가금액 계산 (현재 시간의 가격 사용)
        for (stockcode, holding) in holdings.iter() {
            let current_price =
                if let Some(db_api) = self.db_api.as_any().downcast_ref::<crate::utility::apis::DbApi>() {
                    db_api.get_current_price_at_time(stockcode, &current_time)?
                } else {
                    return Err(StockrsError::general(
                        "DbApi를 찾을 수 없습니다".to_string(),
                    ));
                };
            let stock_value = current_price * holding.quantity as f64;
            total_asset += stock_value;

            debug!("📊 [BacktestApi::calculate_balance] 보유 종목 평가: {} {}주 × {}원 = {:.0}원 (시간: {})", 
                stockcode, holding.quantity, current_price, stock_value, current_time);
        }

        let now = Utc::now().naive_local();
        let asset_info = AssetInfo::new_with_stocks(now, *cash, total_asset - *cash);

        info!("💰 [BacktestApi::calculate_balance] 총 자산 계산: 주문가능 {:.0}원 + 유가증권 = {:.0}원 (시간: {})", *cash, total_asset, current_time);

        Ok(asset_info)
    }

    /// 백테스팅용 주문 실행 (시뮬레이션)
    pub fn execute_backtest_order(&self, order: &mut Order) -> StockrsResult<String> {
        // order의 값들을 먼저 추출하여 borrow checker 문제 해결
        let stockcode = order.get_stockcode().to_string();
        let quantity = order.get_quantity();
        let price = order.get_price();
        let is_buy = order.get_buy_or_sell();

        debug!("🔄 [BacktestApi::execute_backtest_order] 주문 실행 시작 - 종목: {}, 수량: {}, 가격: {}, 매수여부: {}", 
            stockcode, quantity, price, is_buy);

        let mut holdings = self
            .holdings
            .lock()
            .map_err(|e| StockrsError::general(format!("주문 실행 중 뮤텍스 오류: {}", e)))?;

        let mut cash = self
            .cash
            .lock()
            .map_err(|e| StockrsError::general(format!("현금 업데이트 중 뮤텍스 오류: {}", e)))?;

        let order_amount = price * quantity as f64;

        // 설정에서 수수료율 가져오기
        let config = get_config()?;
        let fee_rate = if is_buy {
            config.backtest.buy_fee_rate
        } else {
            config.backtest.sell_fee_rate
        };
        let fee = order_amount * (fee_rate / 100.0);

        // Order 객체의 fee 필드 업데이트
        order.fee = fee;

        // 슬리피지 적용
        let slippage_rate = if is_buy {
            config.backtest.buy_slippage_rate
        } else {
            config.backtest.sell_slippage_rate
        };

        let slippage = order_amount * (slippage_rate / 100.0);
        let total_cost = order_amount + fee + slippage;

        if is_buy {
            // 매수: 현금 차감, 보유 종목 추가
            if total_cost > *cash {
                return Err(StockrsError::BalanceInquiry {
                    reason: format!(
                        "매수 주문 실행 실패: 필요금액 {:.0}원 > 보유현금 {:.0}원",
                        total_cost, *cash
                    ),
                });
            }

            *cash -= total_cost;

            // 보유 종목 업데이트
            let holding = holdings.entry(stockcode.clone()).or_insert(Holding {
                quantity: 0,
                avg_price: 0.0,
                total_cost: 0.0,
            });

            let new_quantity = holding.quantity + quantity;
            let new_total_cost = holding.total_cost + order_amount;
            let new_avg_price = new_total_cost / new_quantity as f64;

            holding.quantity = new_quantity;
            holding.avg_price = new_avg_price;
            holding.total_cost = new_total_cost;

            info!("✅ [BacktestApi::execute_backtest_order] 매수 완료: {} {}주 @{:.0}원 (수수료: {:.0}원, 슬리피지: {:.0}원)", 
                stockcode, quantity, price, fee, slippage);
        } else {
            // 매도: 보유 종목 차감, 현금 추가
            let holding =
                holdings
                    .get_mut(&stockcode)
                    .ok_or_else(|| StockrsError::BalanceInquiry {
                        reason: format!(
                            "매도 주문 실행 실패: 보유 종목이 없습니다 ({})",
                            stockcode
                        ),
                    })?;

            if holding.quantity < quantity {
                return Err(StockrsError::BalanceInquiry {
                    reason: format!(
                        "매도 주문 실행 실패: 보유수량 {}주 < 매도수량 {}주",
                        holding.quantity, quantity
                    ),
                });
            }

            // 매도 수익 계산
            let sell_amount = order_amount - fee - slippage;
            *cash += sell_amount;

            // 보유 종목 업데이트
            holding.quantity -= quantity;
            if holding.quantity == 0 {
                // 전량 매도 시 보유 종목 제거
                holdings.remove(&stockcode);
            } else {
                // 부분 매도 시 평균가 유지 (FIFO 방식)
                holding.total_cost = holding.avg_price * holding.quantity as f64;
            }

            info!("✅ [BacktestApi::execute_backtest_order] 매도 완료: {} {}주 @{:.0}원 (수수료: {:.0}원, 슬리피지: {:.0}원)", 
                stockcode, quantity, price, fee, slippage);
        }

        // 주문 ID 생성 (백테스팅용)
        let order_id = format!(
            "backtest_{}_{}",
            stockcode,
            chrono::Utc::now().timestamp_millis()
        );

        Ok(order_id)
    }

    /// 시간 기반 잔고 계산 (백테스팅용)
    pub fn calculate_balance_at_time(&self, time_str: &str) -> StockrsResult<AssetInfo> {
        let holdings = self
            .holdings
            .lock()
            .map_err(|e| StockrsError::general(format!("잔고 계산 중 뮤텍스 오류: {}", e)))?;

        let cash = self
            .cash
            .lock()
            .map_err(|e| StockrsError::general(format!("현금 조회 중 뮤텍스 오류: {}", e)))?;

        let mut total_asset = *cash;

        // 보유 종목 평가금액 계산
        for (stockcode, holding) in holdings.iter() {
            let current_price =
                if let Some(db_api) = self.db_api.as_any().downcast_ref::<crate::utility::apis::DbApi>() {
                    db_api.get_current_price_at_time(stockcode, time_str)?
                } else {
                    return Err(StockrsError::general(
                        "DbApi를 찾을 수 없습니다".to_string(),
                    ));
                };
            let stock_value = current_price * holding.quantity as f64;
            total_asset += stock_value;

            debug!(
                "📊 [BacktestApi] 보유 종목 평가: {} {}주 × {}원 = {:.0}원 (시간: {})",
                stockcode, holding.quantity, current_price, stock_value, time_str
            );
        }

        let now = Utc::now().naive_local();
        let asset_info = AssetInfo::new_with_stocks(now, *cash, total_asset - *cash);

        debug!(
            "💰 [BacktestApi] 총 자산 계산: 주문가능 {:.0}원 + 유가증권 = {:.0}원 (시간: {})",
            *cash, total_asset, time_str
        );

        Ok(asset_info)
    }
}

impl StockApi for BacktestApi {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn execute_order(&self, order: &mut Order) -> StockrsResult<String> {
        // 백테스팅용 주문 실행
        self.execute_backtest_order(order)
    }

    fn check_fill(&self, _order_id: &str) -> StockrsResult<bool> {
        // 백테스팅에서는 모든 주문이 즉시 체결됨
        Ok(true)
    }

    fn cancel_order(&self, _order_id: &str) -> StockrsResult<()> {
        // 백테스팅에서는 주문 취소 불가 (이미 체결됨)
        Err(StockrsError::order_execution(
            "주문 취소".to_string(),
            "N/A".to_string(),
            0,
            "백테스팅에서는 주문 취소를 지원하지 않습니다.".to_string(),
        ))
    }

    fn get_balance(&self) -> StockrsResult<AssetInfo> {
        // 백테스팅용 잔고 계산
        self.calculate_balance()
    }

    fn get_avg_price(&self, stockcode: &str) -> StockrsResult<f64> {
        debug!(
            "🔄 [BacktestApi::get_avg_price] 평균가 조회 시작 - 종목: {}",
            stockcode
        );

        let holdings = self
            .holdings
            .lock()
            .map_err(|e| StockrsError::general(format!("평균가 조회 중 뮤텍스 오류: {}", e)))?;

        if let Some(holding) = holdings.get(stockcode) {
            if holding.quantity > 0 {
                info!(
                    "📊 [BacktestApi::get_avg_price] 평균가 조회: {} -> {:.0}원 ({}주 보유)",
                    stockcode, holding.avg_price, holding.quantity
                );
                return Ok(holding.avg_price);
            } else {
                warn!(
                    "⚠️ [BacktestApi::get_avg_price] 보유 수량이 0: {} (평균가: {:.0}원)",
                    stockcode, holding.avg_price
                );
            }
        } else {
            debug!(
                "📊 [BacktestApi::get_avg_price] 보유하지 않는 종목: {}",
                stockcode
            );
        }

        error!("❌ [BacktestApi::get_avg_price] 평균가 조회 실패: {} - 해당 종목을 보유하고 있지 않습니다", stockcode);
        Err(StockrsError::price_inquiry(
            stockcode,
            "평균가",
            "해당 종목을 보유하고 있지 않습니다.".to_string(),
        ))
    }

    fn get_current_price(&self, stockcode: &str) -> StockrsResult<f64> {
        // 백테스팅 모드에서는 현재 시간의 가격을 사용
        let current_time = self.get_current_time()?;
        
        // 디버그 로그 추가
        println!("🔍 [BacktestApi::get_current_price] 현재가 조회: {} (시간: {})", stockcode, current_time);
        
        if let Some(db_api) = self.db_api.as_any().downcast_ref::<crate::utility::apis::DbApi>() {
            let result = db_api.get_current_price_at_time(stockcode, &current_time);
            
            // 결과 로그 추가
            match &result {
                Ok(price) => println!("✅ [BacktestApi::get_current_price] 조회 성공: {} = {:.0}원", stockcode, price),
                Err(e) => println!("❌ [BacktestApi::get_current_price] 조회 실패: {} - {}", stockcode, e),
            }
            
            result
        } else {
            Err(StockrsError::general(
                "DbApi를 찾을 수 없습니다".to_string(),
            ))
        }
    }

    fn get_current_price_at_time(&self, stockcode: &str, time_str: &str) -> StockrsResult<f64> {
        // 백테스팅 모드에서는 지정된 시간의 가격을 사용
        if let Some(db_api) = self.db_api.as_any().downcast_ref::<crate::utility::apis::DbApi>() {
            db_api.get_current_price_at_time(stockcode, time_str)
        } else {
            Err(StockrsError::general(
                "DbApi를 찾을 수 없습니다".to_string(),
            ))
        }
    }

    fn set_current_time(&self, _time_str: &str) -> StockrsResult<()> {
        // TimeService를 직접 사용하므로 더 이상 필요하지 않음
        Ok(())
    }

    /// DB 연결을 반환 (특징 계산용) - DbApi에서 위임
    fn get_db_connection(&self) -> Option<rusqlite::Connection> {
        self.db_api.get_db_connection()
    }

    /// 일봉 DB 연결을 반환 (특징 계산용) - DbApi에서 위임
    fn get_daily_db_connection(&self) -> Option<rusqlite::Connection> {
        self.db_api.get_daily_db_connection()
    }
}
