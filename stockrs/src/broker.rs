use crate::config::get_config;
use crate::db_manager::DBManager;
use crate::types::api::SharedApi;
use crate::types::broker::{Broker, Order};
use crate::types::trading::TradingMode;
use std::error::Error;
use tracing::{debug, error, info, warn};

/// 통합된 Broker 구현체
/// prototype.py의 broker(broker_api) 패턴과 동일
pub struct StockBroker {
    api: SharedApi,
    trading_mode: TradingMode,
}

impl StockBroker {
    pub fn new(api: SharedApi) -> Self {
        debug!("🔄 [StockBroker::new] StockBroker 생성 시작");

        // config에서 거래 모드 확인
        let trading_mode = match get_config() {
            Ok(config) => match config.trading.default_mode.as_str() {
                "real" => TradingMode::Real,
                "paper" => TradingMode::Paper,
                "backtest" => TradingMode::Backtest,
                _ => TradingMode::Backtest, // 기본값
            },
            Err(_) => TradingMode::Backtest, // 설정 로드 실패 시 기본값
        };

        info!(
            "✅ [StockBroker::new] StockBroker 생성 완료 - 모드: {:?}",
            trading_mode
        );

        Self { api, trading_mode }
    }
}

impl Broker for StockBroker {
    fn validate(&self, order: &Order) -> Result<(), Box<dyn Error>> {
        let stockcode = order.get_stockcode();
        let quantity = order.get_quantity();
        let price = order.get_price();
        let is_buy = order.get_buy_or_sell();

        debug!("🔄 [StockBroker::validate] 주문 검증 시작 - 종목: {}, 수량: {}, 가격: {}, 매수여부: {}", 
            stockcode, quantity, price, is_buy);

        // 기본 검증
        if stockcode.is_empty() {
            error!("❌ [StockBroker::validate] 종목코드가 비어있습니다");
            return Err("종목코드가 비어있습니다.".into());
        }

        if quantity == 0 {
            error!("❌ [StockBroker::validate] 주문 수량이 0입니다");
            return Err("주문 수량이 0입니다.".into());
        }

        if price <= 0.0 {
            error!(
                "❌ [StockBroker::validate] 주문 가격이 0 이하입니다: {}",
                price
            );
            return Err("주문 가격이 0 이하입니다.".into());
        }

        // 백테스팅 모드에서 추가 검증
        if self.trading_mode == TradingMode::Backtest {
            debug!("🔍 [StockBroker::validate] 백테스팅 모드 추가 검증");

            if is_buy {
                // 매수 시 잔고 확인
                match self.api.get_balance() {
                    Ok(balance) => {
                        let order_amount = price * quantity as f64;
                        let fee = order_amount * 0.00015; // 0.015% 수수료
                        let total_amount = order_amount + fee;

                        debug!("💰 [StockBroker::validate] 매수 검증 - 주문금액: {:.0}원, 보유자산: {:.0}원", 
                            total_amount, balance.get_asset());

                        if balance.get_asset() < total_amount {
                            error!("❌ [StockBroker::validate] 잔고 부족 - 주문금액: {:.0}원, 보유자산: {:.0}원", 
                                total_amount, balance.get_asset());
                            return Err(format!(
                                "잔고 부족: 주문금액 {:.0}원, 보유자산 {:.0}원",
                                total_amount,
                                balance.get_asset()
                            )
                            .into());
                        }
                    }
                    Err(e) => {
                        error!("❌ [StockBroker::validate] 잔고 조회 실패: {}", e);
                        return Err(format!("잔고 조회 실패: {}", e).into());
                    }
                }
            } else {
                // 매도 시 보유 수량 확인
                match self.api.get_avg_price(stockcode) {
                    Ok(_avg_price) => {
                        // 보유 종목이 있으면 매도 가능
                        info!(
                            "✅ [StockBroker::validate] 매도 주문 검증 통과: {} {}주",
                            stockcode, quantity
                        );
                    }
                    Err(e) => {
                        error!(
                            "❌ [StockBroker::validate] 보유하지 않은 종목 매도 시도: {} - {}",
                            stockcode, e
                        );
                        return Err(format!(
                            "보유하지 않은 종목을 매도할 수 없습니다: {}",
                            stockcode
                        )
                        .into());
                    }
                }
            }
        }

        info!(
            "✅ [StockBroker::validate] 주문 검증 통과: {} {} {}주 × {}원",
            if is_buy { "매수" } else { "매도" },
            stockcode,
            quantity,
            price
        );

        Ok(())
    }

    fn execute(&self, order: &mut Order, db: &DBManager) -> Result<(), Box<dyn Error>> {
        debug!("🔄 [StockBroker::execute] 주문 실행 시작");

        // 주문 검증
        self.validate(order)?;

        // 매도 주문의 경우 평균가를 미리 조회 (주문 실행 후에는 보유 종목에서 제거됨)
        let avg_price = if !order.get_buy_or_sell() {
            match self.api.get_avg_price(order.get_stockcode()) {
                Ok(price) => {
                    debug!(
                        "📊 [StockBroker::execute] 매도 주문 평균가 미리 조회: {} -> {:.0}원",
                        order.get_stockcode(),
                        price
                    );
                    price
                }
                Err(e) => {
                    error!(
                        "❌ [StockBroker::execute] 매도 주문 평균가 조회 실패: {}",
                        e
                    );
                    return Err(e.into());
                }
            }
        } else {
            0.0 // 매수 주문은 나중에 조회
        };

        // API를 통한 주문 실행
        let order_id = match self.api.execute_order(order) {
            Ok(id) => {
                info!("✅ [StockBroker::execute] 주문 실행 성공 - 주문ID: {}", id);
                id
            }
            Err(e) => {
                error!("❌ [StockBroker::execute] 주문 실행 실패: {}", e);
                return Err(e.into());
            }
        };

        // 체결 확인
        let filled = match self.api.check_fill(&order_id) {
            Ok(filled) => filled,
            Err(e) => {
                error!("❌ [StockBroker::execute] 체결 확인 실패: {}", e);
                return Err(e.into());
            }
        };

        if filled {
            // 거래 결과를 DB에 저장
            let trading = order.to_trading();

            // 평균가 계산
            let final_avg_price = if order.get_buy_or_sell() {
                // 매수 주문: 현재 평균가 조회
                self.api.get_avg_price(order.get_stockcode()).unwrap_or(0.0)
            } else {
                // 매도 주문: 미리 조회한 평균가 사용
                avg_price
            };

            match db.save_trading(trading, final_avg_price) {
                Ok(_) => {
                    info!("✅ [StockBroker::execute] 거래 DB 저장 완료");
                }
                Err(e) => {
                    error!("❌ [StockBroker::execute] 거래 DB 저장 실패: {}", e);
                    return Err(e.into());
                }
            }
        } else {
            // 미체결 시 주문 취소
            warn!(
                "⚠️ [StockBroker::execute] 미체결 주문 취소 - 주문ID: {}",
                order_id
            );
            if let Err(e) = self.api.cancel_order(&order_id) {
                error!("❌ [StockBroker::execute] 주문 취소 실패: {}", e);
                return Err(e.into());
            }
        }

        info!("✅ [StockBroker::execute] 주문 실행 완료");
        Ok(())
    }
}

/// 생명주기 패턴 추가 - prototype.py와 동일
impl StockBroker {
    /// API 참조 반환 (BacktestApi 접근용)
    pub fn get_api(&self) -> &SharedApi {
        &self.api
    }

    /// broker 시작 시 호출
    /// API 연결 상태 확인 및 초기화
    pub fn on_start(&mut self) -> Result<(), Box<dyn Error>> {
        info!(
            "🔄 [StockBroker::on_start] 브로커 초기화 시작 (모드: {:?})",
            self.trading_mode
        );

        // 거래 모드별 초기화
        match self.trading_mode {
            TradingMode::Backtest => {
                debug!("🔍 [StockBroker::on_start] 백테스팅 모드 초기화");
                // 백테스팅 모드: 잔고 조회로 초기 상태 확인
                match self.api.get_balance() {
                    Ok(balance) => {
                        info!(
                            "✅ [StockBroker::on_start] 백테스팅 초기 잔고: {:.0}원",
                            balance.get_asset()
                        );
                    }
                    Err(e) => {
                        error!("❌ [StockBroker::on_start] 백테스팅 잔고 조회 실패: {}", e);
                        return Err(format!("백테스팅 잔고 조회 오류: {}", e).into());
                    }
                }
            }
            TradingMode::Real | TradingMode::Paper => {
                debug!("🔍 [StockBroker::on_start] 실전/모의투자 모드 초기화");
                // 실전/모의투자 모드: API 연결 상태 확인
                match self.api.get_balance() {
                    Ok(balance) => {
                        info!(
                            "✅ [StockBroker::on_start] API 연결 확인 완료 - 현재 잔고: {:.0}원",
                            balance.get_asset()
                        );
                    }
                    Err(e) => {
                        error!("❌ [StockBroker::on_start] API 연결 확인 실패: {}", e);
                        return Err(format!("API 연결 오류: {}", e).into());
                    }
                }
            }
        }

        info!("✅ [StockBroker::on_start] 완료");
        Ok(())
    }

    /// broker 이벤트 처리 - prototype.py의 broker.on_event(result)와 동일
    pub fn on_event(&mut self, order: &mut Order, db: &DBManager) -> Result<(), Box<dyn Error>> {
        debug!("🔄 [StockBroker::on_event] 브로커 이벤트 처리 시작");

        let result = self.execute(order, db);
        match &result {
            Ok(_) => {
                debug!("✅ [StockBroker::on_event] 브로커 이벤트 처리 완료");
                println!(
                    "✅ [StockBroker::on_event] 거래 실행 완료: {} {} {}주 × {:.0}원",
                    if order.get_buy_or_sell() {
                        "매수"
                    } else {
                        "매도"
                    },
                    order.get_stockcode(),
                    order.get_quantity(),
                    order.get_price()
                );
            }
            Err(e) => {
                error!("❌ [StockBroker::on_event] 브로커 이벤트 처리 실패: {}", e);
                println!("❌ [StockBroker::on_event] 거래 실행 실패: {}", e);
            }
        }
        result
    }

    /// broker 종료 시 호출
    /// 미체결 주문 정리 및 리소스 해제
    pub fn on_end(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🏁 [StockBroker::on_end] 브로커 종료 처리 시작");

        // 백테스팅 모드에서는 즉시 체결되므로 특별한 정리 작업 불필요
        // 실제 거래 모드에서는 미체결 주문 취소 등의 로직이 필요할 수 있음

        info!("✅ [StockBroker::on_end] 완료");
        Ok(())
    }

    /// 매일 새로운 거래일을 위해 브로커 상태 리셋
    pub fn reset_for_new_day(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🔄 [StockBroker::reset_for_new_day] 새로운 거래일을 위해 브로커 리셋");

        // 백테스팅 모드에서는 특별한 리셋 작업 불필요
        // 실제 거래 모드에서는 미체결 주문 취소 등의 로직이 필요할 수 있음

        info!("✅ [StockBroker::reset_for_new_day] 브로커 리셋 완료");
        Ok(())
    }
}
