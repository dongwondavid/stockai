use crate::utility::config::get_config;
use crate::model::{ApiBundle, Model, ONNXPredictor};
use crate::time::TimeService;
use crate::utility::types::broker::{Order, OrderSide};
use crate::utility::types::trading::TradingMode;
use chrono::{NaiveDateTime, Timelike};
use std::error::Error;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, PartialEq)]
pub enum TradingState {
    WaitingForEntry, // 9:30 매수 대기
    Holding,         // 포지션 보유 중
    Closed,          // 모든 포지션 정리됨
}

pub struct JoonwooModel {
    // ONNX 예측기
    predictor: Option<ONNXPredictor>,

    // 포지션 관리
    current_stock: Option<String>,
    position_size: u32,
    remaining_size: u32,
    entry_price: f64,
    entry_time: Option<NaiveDateTime>,

    // 상태 추적
    state: TradingState,

    // 설정값들 (config에서 로드)
    stop_loss_pct: f64,
    take_profit_pct: f64,
    entry_time_str: String,
    force_close_time_str: String,
    entry_asset_ratio: f64,
    fixed_entry_amount: f64,  // 고정 매수 금액 (원)
}

impl Default for JoonwooModel {
    fn default() -> Self {
        info!("🚀 joonwoo 모델 생성 중...");

        // 설정에서 값들을 로드
        let config = match get_config() {
            Ok(config) => config,
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
            entry_time: None,
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

impl JoonwooModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// 기본값으로 생성 (설정 로드 실패 시 사용)
    fn with_default_values() -> Self {
        Self {
            predictor: None,
            current_stock: None,
            position_size: 0,
            remaining_size: 0,
            entry_price: 0.0,
            entry_time: None,
            state: TradingState::WaitingForEntry,
            stop_loss_pct: 1.0,     // -1%
            take_profit_pct: 2.0,   // +2%
            entry_time_str: "09:30:00".to_string(),
            force_close_time_str: "12:00:00".to_string(),
            entry_asset_ratio: 90.0,
            fixed_entry_amount: 1000000.0, // 기본 고정 매수 금액
        }
    }

    /// 시간 문자열을 파싱하여 시간과 분을 추출
    fn parse_time_string(&self, time_str: &str) -> Result<(u32, u32), Box<dyn Error>> {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 3 {
            return Err(format!("잘못된 시간 형식: {}", time_str).into());
        }

        let hour = parts[0].parse::<u32>()
            .map_err(|_| format!("시간 파싱 실패: {}", parts[0]))?;
        let minute = parts[1].parse::<u32>()
            .map_err(|_| format!("분 파싱 실패: {}", parts[1]))?;

        Ok((hour, minute))
    }

    /// 전역 TimeService를 사용하는 매수 시간 계산
    fn get_entry_time_for_today_global(&self) -> Result<(u32, u32), Box<dyn Error>> {
        let current_time = crate::time::TimeService::global_now()
            .map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;
        let current_date = current_time.date_naive();
        
        // 기본 매수 시간 파싱
        let (mut hour, mut minute) = self.parse_time_string(&self.entry_time_str)?;
        
        // 전역 TimeService에서 특별한 날 체크
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

    /// 전역 TimeService를 사용하는 강제 정리 시간 계산
    fn get_force_close_time_for_today_global(&self) -> Result<(u32, u32), Box<dyn Error>> {
        let current_time = crate::time::TimeService::global_now()
            .map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;
        let current_date = current_time.date_naive();
        
        // 기본 강제 정리 시간 파싱
        let (mut hour, mut minute) = self.parse_time_string(&self.force_close_time_str)?;
        
        // 전역 TimeService에서 특별한 날 체크
        let global_time_service = crate::time::TimeService::get()
            .map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;
        let time_service_guard = global_time_service.lock()
            .map_err(|e| format!("TimeService 뮤텍스 락 실패: {}", e))?;
        
        if let Some(time_service) = time_service_guard.as_ref() {
            if time_service.is_special_start_date(current_date) {
                let offset = time_service.special_start_time_offset_minutes;
                let total_minutes = hour as i32 * 60 + minute as i32 + offset;
                if total_minutes < 0 || total_minutes >= 24 * 60 {
                    return Err(format!("force_close_time 오프셋 적용 결과가 0~24시 범위를 벗어남: {}분", total_minutes).into());
                }
                hour = (total_minutes / 60) as u32;
                minute = (total_minutes % 60) as u32;
            }
        }
        
        Ok((hour, minute))
    }

    /// 전역 TimeService를 사용하는 매수 시도
    fn try_entry_global(
        &mut self,
        apis: &ApiBundle,
    ) -> Result<Option<Order>, Box<dyn Error>> {
        if self.state != TradingState::WaitingForEntry {
            debug!("이미 진입했거나 종료된 상태입니다: {:?}", self.state);
            return Ok(None);
        }

        let current_time = crate::time::TimeService::global_now()
            .map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;

        // 설정된 매수 시간 확인
        let (entry_hour, entry_minute) = self.get_entry_time_for_today_global()?;
        if current_time.hour() != entry_hour || current_time.minute() != entry_minute {
            debug!("매수 시간이 아닙니다: {}:{} (설정: {}:{:02})", 
                current_time.hour(), current_time.minute(), entry_hour, entry_minute);
            return Ok(None);
        }

        // ONNX 모델로 최고 확률 종목 예측
        let predictor = self
            .predictor
            .as_mut()
            .ok_or("ONNX 예측기가 초기화되지 않았습니다")?;

        // DB 연결 가져오기 (백테스트 모드에서만 사용)
        let db = apis
            .db_api
            .get_db_connection()
            .ok_or("DB 연결을 가져올 수 없습니다")?;
        let daily_db = apis
            .db_api
            .get_daily_db_connection()
            .ok_or("일봉 DB 연결을 가져올 수 없습니다")?;

        let today_str = TimeService::format_local_ymd(&current_time);
        let target_stock = match predictor.predict_top_stock(&today_str, &db, &daily_db) {
            Ok(Some(stock)) => stock,
            Ok(None) => {
                info!("🔮 [joonwoo] 예측 결과가 없습니다 - 매수하지 않음");
                return Ok(None);
            }
            Err(e) => {
                return Err(e.into());
            }
        };

        // 현재가 조회 (모드별 자동 선택)
        let current_price = apis
            .get_current_price(&target_stock)
            .map_err(|e| {
                println!("❌ [joonwoo] 현재가 조회 실패: {}", e);
                Box::new(e) as Box<dyn Error>
            })?;

        // 잔고 조회
        let balance_info = apis
            .get_balance()
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;

        // 고정 매수 금액 우선 시도, 부족하면 비율 기반 매수
        let mut quantity_to_buy = 0;
        let available_balance = balance_info.get_asset();
        
        if self.fixed_entry_amount > 0.0 {
            // 고정 금액으로 매수 시도
            let max_quantity_fixed = (self.fixed_entry_amount / current_price) as u32;
            if max_quantity_fixed > 0 && self.fixed_entry_amount <= available_balance {
                quantity_to_buy = max_quantity_fixed;
                info!("📈 [joonwoo] 고정 매수: {}주 @{:.0}원 (고정 금액: {:.0}원)", 
                    quantity_to_buy, current_price, self.fixed_entry_amount);
            } else {
                // 고정 금액으로 매수할 수 없으면 비율 기반 매수
                let available_amount = available_balance * (self.entry_asset_ratio / 100.0);
                let max_quantity_ratio = (available_amount / current_price) as u32;
                if max_quantity_ratio > 0 {
                    quantity_to_buy = max_quantity_ratio;
                    info!("📈 [joonwoo] 비율 매수: {}주 @{:.0}원 (자산 비율: {:.1}%)", 
                        quantity_to_buy, current_price, self.entry_asset_ratio);
                }
            }
        } else {
            // 비율 기반 매수만
            let available_amount = available_balance * (self.entry_asset_ratio / 100.0);
            let max_quantity_ratio = (available_amount / current_price) as u32;
            if max_quantity_ratio > 0 {
                quantity_to_buy = max_quantity_ratio;
                info!("📈 [joonwoo] 비율 매수: {}주 @{:.0}원 (자산 비율: {:.1}%)", 
                    quantity_to_buy, current_price, self.entry_asset_ratio);
            }
        }

        if quantity_to_buy == 0 {
            info!("💰 [joonwoo] 매수 가능한 수량이 없습니다 (잔고: {:.0}원, 필요: {:.0}원)", 
                available_balance, current_price);
            return Ok(None);
        }

        // 매수 주문 생성
        let order = Order {
            date: current_time.naive_local(),
            stockcode: target_stock.clone(),
            side: OrderSide::Buy,
            quantity: quantity_to_buy,
            price: current_price,
            fee: 0.0, // 수수료는 나중에 계산
            strategy: "joonwoo_entry".to_string(),
        };

        // 상태 업데이트
        self.current_stock = Some(target_stock.clone());
        self.position_size = quantity_to_buy;
        self.remaining_size = quantity_to_buy;
        self.entry_price = current_price;
        self.entry_time = Some(current_time.naive_local());
        self.state = TradingState::Holding;

        info!("🚀 [joonwoo] 매수 주문 생성: {} {}주 @{:.0}원", 
            target_stock, quantity_to_buy, current_price);

        Ok(Some(order))
    }

    /// 전역 TimeService를 사용하는 강제 정리
    fn force_close_all_global(
        &mut self,
        apis: &ApiBundle,
    ) -> Result<Option<Order>, Box<dyn Error>> {
        if self.state != TradingState::Holding {
            debug!("포지션이 없습니다: {:?}", self.state);
            return Ok(None);
        }

        let current_time = crate::time::TimeService::global_now()
            .map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;

        // 설정된 강제 정리 시간 확인
        let (force_close_hour, force_close_minute) = self.get_force_close_time_for_today_global()?;
        if current_time.hour() != force_close_hour || current_time.minute() != force_close_minute {
            debug!("강제 정리 시간이 아닙니다: {}:{} (설정: {}:{:02})", 
                current_time.hour(), current_time.minute(), force_close_hour, force_close_minute);
            return Ok(None);
        }

        // 현재가 조회
        let current_price = apis
            .get_current_price(self.current_stock.as_ref().unwrap())
            .map_err(|e| {
                println!("❌ [joonwoo] 현재가 조회 실패: {}", e);
                Box::new(e) as Box<dyn Error>
            })?;

        // 강제 정리 주문 생성
        let order = self.create_sell_all_order_global(current_price, "force_close")?;

        // 상태 업데이트
        self.state = TradingState::Closed;

        info!("🔚 [joonwoo] 강제 정리 주문 생성: {} {}주 @{:.0}원", 
            self.current_stock.as_ref().unwrap(), self.remaining_size, current_price);

        Ok(order)
    }

    /// 전역 TimeService를 사용하는 종료 조건 체크
    fn check_exit_conditions_global(
        &mut self,
        apis: &ApiBundle,
    ) -> Result<Option<Order>, Box<dyn Error>> {
        if self.state != TradingState::Holding {
            return Ok(None);
        }

        let current_price = apis
            .get_current_price(self.current_stock.as_ref().unwrap())
            .map_err(|e| {
                println!("❌ [joonwoo] 현재가 조회 실패: {}", e);
                Box::new(e) as Box<dyn Error>
            })?;

        let price_change_pct = ((current_price - self.entry_price) / self.entry_price) * 100.0;

        // 손절 조건
        if price_change_pct <= -self.stop_loss_pct {
            let order = self.create_sell_all_order_global(current_price, "stop_loss")?;
            self.state = TradingState::Closed;
            info!("📉 [joonwoo] 손절: {:.1}% 손실 @{:.0}원", price_change_pct, current_price);
            return Ok(order);
        }

        // 익절 조건
        if price_change_pct >= self.take_profit_pct {
            let order = self.create_sell_all_order_global(current_price, "take_profit")?;
            self.state = TradingState::Closed;
            info!("📈 [joonwoo] 익절: {:.1}% 이익 @{:.0}원", price_change_pct, current_price);
            return Ok(order);
        }

        Ok(None)
    }

    /// 전역 TimeService를 사용하는 전량 매도 주문 생성
    fn create_sell_all_order_global(
        &mut self,
        price: f64,
        reason: &str,
    ) -> Result<Option<Order>, Box<dyn Error>> {
        if self.remaining_size == 0 {
            return Ok(None);
        }

        let order = Order {
            date: crate::time::TimeService::global_now()
                .map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?
                .naive_local(),
            stockcode: self.current_stock.as_ref().unwrap().clone(),
            side: OrderSide::Sell,
            quantity: self.remaining_size,
            price,
            fee: 0.0, // 수수료는 나중에 계산
            strategy: reason.to_string(),
        };

        self.remaining_size = 0;

        Ok(Some(order))
    }


}

impl Model for JoonwooModel {
    fn on_start(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🚀 [joonwoo] 모델 시작!");

        // ONNX 예측기 초기화
        match ONNXPredictor::new(TradingMode::Backtest) {
            Ok(predictor) => {
                self.predictor = Some(predictor);
                info!("✅ [joonwoo] ONNX 예측기 초기화 완료");
            }
            Err(e) => {
                return Err(Box::new(e));
            }
        }

        self.state = TradingState::WaitingForEntry;
        info!(
            "📊 [joonwoo] 설정 - 손절: -{:.1}%, 익절: +{:.1}%, 매수시간: {}, 강제정리시간: {}, 자산비율: {:.1}%, 고정매수금액: {:.0}원",
            self.stop_loss_pct, self.take_profit_pct, 
            self.entry_time_str, self.force_close_time_str, self.entry_asset_ratio, self.fixed_entry_amount
        );

        Ok(())
    }

    fn on_event(
        &mut self,
        apis: &ApiBundle,
    ) -> Result<Option<Order>, Box<dyn Error>> {
        // 전역 TimeService 인스턴스 사용
        let current_time = crate::time::TimeService::global_now()
            .map_err(|e| format!("전역 TimeService 접근 실패: {}", e))?;
        let hour = current_time.hour();
        let minute = current_time.minute();

        // 설정된 시간에 따른 로직 분기
        let (entry_hour, entry_minute) = match self.get_entry_time_for_today_global() {
            Ok((h, m)) => (h, m),
            Err(e) => {
                warn!("매수 시간 파싱 실패: {}", e);
                return Ok(None);
            }
        };

        let (force_close_hour, force_close_minute) = match self.get_force_close_time_for_today_global() {
            Ok((h, m)) => (h, m),
            Err(e) => {
                warn!("강제 정리 시간 파싱 실패: {}", e);
                return Ok(None);
            }
        };

        // 시간대별 로직 분기
        match (hour, minute) {
            // 설정된 매수 시간
            (h, m) if h == entry_hour && m == entry_minute => {
                debug!("⏰ [joonwoo] 매수 타이밍 (설정: {}:{:02})", h, m);
                self.try_entry_global(apis)
            }
            // 설정된 강제 정리 시간
            (h, m) if h == force_close_hour && m == force_close_minute => {
                debug!("⏰ [joonwoo] 강제 정리 타이밍 (설정: {}:{:02})", h, m);
                self.force_close_all_global(apis)
            }
            // 일반 시간대 조건 체크 (매수 시간 ~ 강제 정리 시간)
            (h, m) if (h > entry_hour || (h == entry_hour && m >= entry_minute)) 
                     && (h < force_close_hour || (h == force_close_hour && m <= force_close_minute)) => {
                self.check_exit_conditions_global(apis)
            }
            // 기타 시간대는 아무것도 하지 않음
            _ => {
                debug!("⏸️ [joonwoo] 비활성 시간대: {}:{:02} (매수: {}:{:02}, 정리: {}:{:02})", 
                    hour, minute, entry_hour, entry_minute, force_close_hour, force_close_minute);
                Ok(None)
            }
        }
    }

    fn on_end(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🏁 [joonwoo] 모델 종료");
        info!("📊 [joonwoo] 최종 상태: {:?}", self.state);

        if let Some(ref stock) = self.current_stock {
            info!("📋 [joonwoo] 거래 종목: {}", stock);
            info!("💰 [joonwoo] 진입가: {:.0}원", self.entry_price);
            info!("📦 [joonwoo] 잔여 수량: {}주", self.remaining_size);
        }

        Ok(())
    }

    fn reset_for_new_day(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🔄 [joonwoo] 새로운 거래일을 위한 리셋");

        self.current_stock = None;
        self.position_size = 0;
        self.remaining_size = 0;
        self.entry_price = 0.0;
        self.entry_time = None;
        self.state = TradingState::WaitingForEntry;

        Ok(())
    }
}
