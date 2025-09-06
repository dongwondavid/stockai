use crate::utility::config::get_config;
use crate::model::{ApiBundle, Model, ONNXPredictor};
use crate::time::TimeService;
use crate::utility::types::broker::{Order, OrderSide};
use chrono::Timelike;
use std::error::Error;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, PartialEq)]
pub enum TradingState {
    WaitingForEntry,
    Holding,
    Closed,
}

pub struct MinseopModel {
    predictor: Option<ONNXPredictor>,

    current_stock: Option<String>,
    position_size: u32,
    remaining_size: u32,
    entry_price: f64,

    state: TradingState,

    // 설정값 (joonwoo 설정을 재사용)
    stop_loss_pct: f64,
    take_profit_pct: f64,
    entry_time_str: String,
    force_close_time_str: String,
    entry_asset_ratio: f64,
    fixed_entry_amount: f64,
}

impl Default for MinseopModel {
    fn default() -> Self {
        println!("🚀 minseop 모델 생성 중...");

        let config = match get_config() {
            Ok(c) => c,
            Err(e) => {
                warn!("설정 로드 실패, 기본값 사용: {}", e);
                return Self::with_default_values();
            }
        };

        Self {
            predictor: None,
            current_stock: None,
            position_size: 0,
            remaining_size: 0,
            entry_price: 0.0,
            state: TradingState::WaitingForEntry,
            stop_loss_pct: config.joonwoo.stop_loss_pct,
            take_profit_pct: config.joonwoo.take_profit_pct,
            entry_time_str: config.joonwoo.entry_time.clone(),
            force_close_time_str: config.joonwoo.force_close_time.clone(),
            entry_asset_ratio: config.joonwoo.entry_asset_ratio,
            fixed_entry_amount: config.joonwoo.fixed_entry_amount,
        }
    }
}

impl MinseopModel {
    pub fn new() -> Self { Self::default() }

    fn with_default_values() -> Self {
        Self {
            predictor: None,
            current_stock: None,
            position_size: 0,
            remaining_size: 0,
            entry_price: 0.0,
            state: TradingState::WaitingForEntry,
            stop_loss_pct: 1.0,
            take_profit_pct: 2.0,
            entry_time_str: "09:30:00".to_string(),
            force_close_time_str: "12:00:00".to_string(),
            entry_asset_ratio: 90.0,
            fixed_entry_amount: 1_000_000.0,
        }
    }

    fn parse_time_string(&self, time_str: &str) -> Result<(u32, u32), Box<dyn Error>> {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 3 { return Err(format!("잘못된 시간 형식: {}", time_str).into()); }
        let hour = parts[0].parse::<u32>().map_err(|_| format!("시간 파싱 실패: {}", parts[0]))?;
        let minute = parts[1].parse::<u32>().map_err(|_| format!("분 파싱 실패: {}", parts[1]))?;
        Ok((hour, minute))
    }

    fn get_entry_time_for_today_global(&self) -> Result<(u32, u32), Box<dyn Error>> {
        let current_time = crate::time::TimeService::global_now()
            .map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;
        let current_date = current_time.date_naive();
        let (mut hour, mut minute) = self.parse_time_string(&self.entry_time_str)?;
        let global_time_service = crate::time::TimeService::get()
            .map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;
        let time_service_guard = global_time_service.lock()
            .map_err(|e| format!("TimeService 뮤텍스 락 실패: {}", e))?;
        if let Some(time_service) = time_service_guard.as_ref() {
            if time_service.is_special_start_date(current_date) {
                let offset = time_service.special_start_time_offset_minutes;
                let total_minutes = hour as i32 * 60 + minute as i32 + offset;
                if total_minutes < 0 || total_minutes >= 24 * 60 {
                    return Err(format!("entry_time 오프셋 적용 결과가 0~24시 범위를 벗어남: {}분", total_minutes).into());
                }
                hour = (total_minutes / 60) as u32;
                minute = (total_minutes % 60) as u32;
            }
        }
        Ok((hour, minute))
    }

    fn get_force_close_time_for_today_global(&self) -> Result<(u32, u32), Box<dyn Error>> {
        let (hour, minute) = self.parse_time_string(&self.force_close_time_str)?;
        Ok((hour, minute))
    }

    fn try_entry_global(
        &mut self,
        apis: &ApiBundle,
    ) -> Result<Option<Order>, Box<dyn Error>> {
        if self.state != TradingState::WaitingForEntry { return Ok(None); }

        let current_time = crate::time::TimeService::global_now()
            .map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;

        let (entry_hour, entry_minute) = self.get_entry_time_for_today_global()?;
        if current_time.hour() != entry_hour || current_time.minute() != entry_minute {
            debug!("매수 시간이 아닙니다: {}:{} (설정: {}:{:02})",
                current_time.hour(), current_time.minute(), entry_hour, entry_minute);
            return Ok(None);
        }

        if self.predictor.is_none() {
            let mode = apis.get_current_mode().clone();
            match ONNXPredictor::new(mode) {
                Ok(p) => { self.predictor = Some(p); info!("✅ [minseop] ONNX 예측기 초기화 완료"); }
                Err(e) => { return Err(Box::new(e)); }
            }
        }

        let predictor = self.predictor.as_mut().ok_or("ONNX 예측기 초기화 실패")?;

        let db = apis.db_api.get_db_connection().ok_or("DB 연결을 가져올 수 없습니다")?;
        let daily_db = apis.db_api.get_daily_db_connection().ok_or("일봉 DB 연결을 가져올 수 없습니다")?;

        let today_str = TimeService::format_local_ymd(&current_time);
        let (target_stock, reg_value) = match predictor.predict_top_stock_regression(&today_str, &db, &daily_db) {
            Ok(Some((stock, value, _all))) => (stock, value),
            Ok(None) => { info!("🔮 [minseop] 예측 결과가 없습니다 - 매수하지 않음"); return Ok(None); }
            Err(e) => { return Err(e.into()); }
        };

        // 회귀값 기준: 0 이상일 때만 매수
        if reg_value < 0.0 { info!("🧮 [minseop] 회귀값이 0 미만 - 매수하지 않음 (value={:.6})", reg_value); return Ok(None); }

        let current_price = match apis.get_current_price(&target_stock) {
            Ok(p) => p,
            Err(e) => {
                if apis.is_backtest_mode() {
                    if let Ok(cfg) = get_config() { if cfg.backtest.skip_missing_price_as_unavailable {
                        let msg = e.to_string();
                        if msg.contains("해당 종목의 데이터가 1분봉 DB에 존재하지 않습니다") {
                            info!("⚠️ [minseop] 가격 데이터 없음 - 종목 {} 틱 스킵", target_stock);
                            return Ok(None);
                        }
                    }}
                }
                println!("❌ [minseop] 현재가 조회 실패: {}", e);
                return Err(Box::new(e) as Box<dyn Error>);
            }
        };

        let balance_info = apis.get_balance().map_err(|e| Box::new(e) as Box<dyn Error>)?;
        let mut quantity_to_buy = 0u32;
        let available_balance = balance_info.get_asset();

        if self.fixed_entry_amount > 0.0 {
            let max_quantity_fixed = (self.fixed_entry_amount / current_price) as u32;
            if max_quantity_fixed > 0 && self.fixed_entry_amount <= available_balance {
                quantity_to_buy = max_quantity_fixed;
                info!("📈 [minseop] 고정 매수: {}주 @{:.0}원 (고정 금액: {:.0}원)", quantity_to_buy, current_price, self.fixed_entry_amount);
            } else {
                let available_amount = available_balance * (self.entry_asset_ratio / 100.0);
                let max_quantity_ratio = (available_amount / current_price) as u32;
                if max_quantity_ratio > 0 { quantity_to_buy = max_quantity_ratio; info!("📈 [minseop] 비율 매수: {}주 @{:.0}원 (자산 비율: {:.1}%)", quantity_to_buy, current_price, self.entry_asset_ratio); }
            }
        } else {
            let available_amount = available_balance * (self.entry_asset_ratio / 100.0);
            let max_quantity_ratio = (available_amount / current_price) as u32;
            if max_quantity_ratio > 0 { quantity_to_buy = max_quantity_ratio; info!("📈 [minseop] 비율 매수: {}주 @{:.0}원 (자산 비율: {:.1}%)", quantity_to_buy, current_price, self.entry_asset_ratio); }
        }

        if quantity_to_buy == 0 { info!("💰 [minseop] 매수 가능한 수량이 없습니다 (잔고: {:.0}원, 필요: {:.0}원)", available_balance, current_price); return Ok(None); }

        let order = Order { date: current_time.naive_local(), stockcode: target_stock.clone(), side: OrderSide::Buy, quantity: quantity_to_buy, price: current_price, fee: 0.0, strategy: "minseop_entry".to_string() };

        self.current_stock = Some(target_stock.clone());
        self.position_size = quantity_to_buy;
        self.remaining_size = quantity_to_buy;
        self.entry_price = current_price;
        self.state = TradingState::Holding;

        info!("🚀 [minseop] 매수 주문 생성: {} {}주 @{:.0}원 (value={:.6})", target_stock, quantity_to_buy, current_price, reg_value);
        Ok(Some(order))
    }

    fn force_close_all_global(&mut self, apis: &ApiBundle) -> Result<Option<Order>, Box<dyn Error>> {
        if self.state != TradingState::Holding { return Ok(None); }
        let current_time = crate::time::TimeService::global_now().map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;
        let (force_close_hour, force_close_minute) = self.get_force_close_time_for_today_global()?;
        let now_h = current_time.hour(); let now_m = current_time.minute();
        let is_before_force_close = now_h < force_close_hour || (now_h == force_close_hour && now_m < force_close_minute);
        if is_before_force_close { return Ok(None); }

        let current_price = apis.get_current_price(self.current_stock.as_ref().unwrap())?;
        let order = self.create_sell_all_order_global(current_price, "force_close")?;
        self.state = TradingState::Closed;
        info!("🔚 [minseop] 강제 정리 주문 생성: {} {}주 @{:.0}원", self.current_stock.as_ref().unwrap(), self.remaining_size, current_price);
        Ok(order)
    }

    fn check_exit_conditions_global(&mut self, apis: &ApiBundle) -> Result<Option<Order>, Box<dyn Error>> {
        if self.state != TradingState::Holding { return Ok(None); }
        let current_price = apis.get_current_price(self.current_stock.as_ref().unwrap())?;
        let price_change_pct = ((current_price - self.entry_price) / self.entry_price) * 100.0;
        if price_change_pct <= -self.stop_loss_pct {
            let order = self.create_sell_all_order_global(current_price, "stop_loss")?; self.state = TradingState::Closed; info!("📉 [minseop] 손절: {:.1}% 손실 @{:.0}원", price_change_pct, current_price); return Ok(order);
        }
        if price_change_pct >= self.take_profit_pct {
            let order = self.create_sell_all_order_global(current_price, "take_profit")?; self.state = TradingState::Closed; info!("📈 [minseop] 익절: {:.1}% 이익 @{:.0}원", price_change_pct, current_price); return Ok(order);
        }
        Ok(None)
    }

    fn create_sell_all_order_global(&mut self, price: f64, reason: &str) -> Result<Option<Order>, Box<dyn Error>> {
        if self.remaining_size == 0 { return Ok(None); }
        let order = Order { date: crate::time::TimeService::global_now().map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?.naive_local(), stockcode: self.current_stock.as_ref().unwrap().clone(), side: OrderSide::Sell, quantity: self.remaining_size, price, fee: 0.0, strategy: reason.to_string() };
        self.remaining_size = 0; Ok(Some(order))
    }
}

impl Model for MinseopModel {
    fn on_start(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🚀 [minseop] 모델 시작!");
        self.predictor = None;
        self.state = TradingState::WaitingForEntry;
        info!(
            "📊 [minseop] 설정 - 손절: -{:.1}%, 익절: +{:.1}%, 매수시간: {}, 강제정리시간: {}, 자산비율: {:.1}%, 고정매수금액: {:.0}원",
            self.stop_loss_pct, self.take_profit_pct, self.entry_time_str, self.force_close_time_str, self.entry_asset_ratio, self.fixed_entry_amount
        );
        Ok(())
    }

    fn on_event(&mut self, apis: &ApiBundle) -> Result<Option<Order>, Box<dyn Error>> {
        let current_time = crate::time::TimeService::global_now().map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;
        let hour = current_time.hour(); let minute = current_time.minute();
        let (entry_hour, entry_minute) = match self.get_entry_time_for_today_global() { Ok((h,m)) => (h,m), Err(e) => { warn!("매수 시간 파싱 실패: {}", e); return Ok(None); } };
        let (force_close_hour, force_close_minute) = match self.get_force_close_time_for_today_global() { Ok((h,m)) => (h,m), Err(e) => { warn!("강제 정리 시간 파싱 실패: {}", e); return Ok(None); } };

        match (hour, minute) {
            (h, m) if h == entry_hour && m == entry_minute => { debug!("⏰ [minseop] 매수 타이밍 (설정: {}:{:02})", h, m); self.try_entry_global(apis) }
            (h, m) if (h > force_close_hour) || (h == force_close_hour && m >= force_close_minute) => { debug!("⏰ [minseop] 강제 정리 타이밍 도달/경과 (설정: {}:{:02})", force_close_hour, force_close_minute); self.force_close_all_global(apis) }
            (h, m) if (h > entry_hour || (h == entry_hour && m >= entry_minute)) && (h < force_close_hour || (h == force_close_hour && m <= force_close_minute)) => { self.check_exit_conditions_global(apis) }
            _ => { debug!("⏸️ [minseop] 비활성 시간대: {}:{:02} (매수: {}:{:02}, 정리: {}:{:02})", hour, minute, entry_hour, entry_minute, force_close_hour, force_close_minute); Ok(None) }
        }
    }

    fn on_end(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🏁 [minseop] 모델 종료");
        info!("📊 [minseop] 최종 상태: {:?}", self.state);
        if let Some(ref stock) = self.current_stock { info!("📋 [minseop] 거래 종목: {}", stock); info!("💰 [minseop] 진입가: {:.0}원", self.entry_price); info!("📦 [minseop] 잔여 수량: {}주", self.remaining_size); }
        Ok(())
    }

    fn reset_for_new_day(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🔄 [minseop] 새로운 거래일을 위한 리셋");
        self.current_stock = None; self.position_size = 0; self.remaining_size = 0; self.entry_price = 0.0; self.state = TradingState::WaitingForEntry; Ok(())
    }
}


