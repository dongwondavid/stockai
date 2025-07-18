use std::error::Error;
use log::{debug, warn, info};
use chrono::{NaiveDateTime, Timelike};
use crate::model::{Model, ONNXPredictor};
use crate::types::broker::{Order, OrderSide};
use crate::time::TimeService;

#[derive(Debug, Clone, PartialEq)]
pub enum TradingState {
    WaitingForEntry,    // 9:30 매수 대기
    Holding,           // 포지션 보유 중  
    PartialSold,       // 절반 익절 후 잔여분 보유
    Closed,            // 모든 포지션 정리됨
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
    highest_price_after_2pct: Option<f64>,
    
    // 설정값들
    stop_loss_pct: f64,      // -1%
    take_profit_pct: f64,    // +2% 
    trailing_stop_pct: f64,  // -0.7%
}

impl JoonwooModel {
    pub fn new() -> Self {
        info!("🚀 joonwoo 모델 생성 중...");
        
        Self {
            predictor: None,
            current_stock: None,
            position_size: 0,
            remaining_size: 0,
            entry_price: 0.0,
            entry_time: None,
            state: TradingState::WaitingForEntry,
            highest_price_after_2pct: None,
            stop_loss_pct: 1.0,      // -1%
            take_profit_pct: 2.0,    // +2%
            trailing_stop_pct: 0.7,  // -0.7%
        }
    }
    
    /// 매수 시도 (9:30)
    fn try_entry(&mut self, time: &TimeService) -> Result<Option<Order>, Box<dyn Error>> {
        if self.state != TradingState::WaitingForEntry {
            debug!("이미 진입했거나 종료된 상태입니다: {:?}", self.state);
            return Ok(None);
        }
        
        let current_time = time.now();
        info!("📈 [joonwoo] {}에 매수 시도 중...", current_time.format("%H:%M:%S"));
        
        // ONNX 모델로 최고 확률 종목 예측
        let today_str = current_time.format("%Y%m%d").to_string();
        let target_stock = self.predictor
            .as_ref()
            .ok_or("ONNX 예측기가 초기화되지 않았습니다")?
            .predict_top_stock(&today_str)?;
        
        // TODO: 실제 가격 조회 API 연동 필요
        // 현재는 더미 가격 사용
        let current_price = self.get_current_price_dummy(&target_stock)?;
        
        // TODO: 실제 잔고 조회 API 연동 필요  
        // 현재는 더미 잔고 사용
        let available_cash = self.get_available_cash_dummy()?;
        let max_quantity = (available_cash / current_price) as u32;
        
        if max_quantity == 0 {
            warn!("💸 [joonwoo] 매수 가능한 수량이 없습니다. 잔고: {}, 가격: {}", available_cash, current_price);
            return Ok(None);
        }
        
        // 매수 주문 생성
        let order = Order {
            date: current_time.naive_local(),
            stockcode: target_stock.clone(),
            side: OrderSide::Buy,
            quantity: max_quantity,
            price: current_price,
            fee: 0.0,
            strategy: "joonwoo_entry".to_string(),
        };
        
        // 상태 업데이트
        self.current_stock = Some(target_stock.clone());
        self.position_size = max_quantity;
        self.remaining_size = max_quantity;
        self.entry_price = current_price;
        self.entry_time = Some(current_time.naive_local());
        self.state = TradingState::Holding;
        
        info!("🎯 [joonwoo] 매수 주문: {} {}주 @{:.0}원", 
              order.stockcode, order.quantity, order.price);
        
        Ok(Some(order))
    }
    
    /// 손절/익절 조건 체크
    fn check_exit_conditions(&mut self, time: &TimeService) -> Result<Option<Order>, Box<dyn Error>> {
        if let Some(ref stock_code) = self.current_stock {
            let current_price = self.get_current_price_dummy(stock_code)?;
            let price_change_pct = (current_price - self.entry_price) / self.entry_price * 100.0;
            
            match self.state {
                TradingState::Holding => {
                    // -1% 손절 체크
                    if price_change_pct <= -self.stop_loss_pct {
                        info!("💀 [joonwoo] 손절 조건 달성: {:.2}% (목표: -{:.1}%)", 
                              price_change_pct, self.stop_loss_pct);
                        return self.create_sell_all_order(time, current_price, "stop_loss");
                    }
                    
                    // +2% 익절 체크 (절반만)
                    if price_change_pct >= self.take_profit_pct {
                        info!("🎉 [joonwoo] 익절 조건 달성: {:.2}% (목표: +{:.1}%) - 절반 익절", 
                              price_change_pct, self.take_profit_pct);
                        self.highest_price_after_2pct = Some(current_price);
                        self.state = TradingState::PartialSold;
                        return self.create_sell_half_order(time, current_price, "take_profit");
                    }
                }
                TradingState::PartialSold => {
                    // 고가 대비 -0.7% 추가 손절 체크
                    if let Some(highest) = self.highest_price_after_2pct {
                        let new_highest = highest.max(current_price);
                        self.highest_price_after_2pct = Some(new_highest);
                        
                        let drop_from_high = (new_highest - current_price) / new_highest * 100.0;
                        if drop_from_high >= self.trailing_stop_pct {
                            info!("📉 [joonwoo] 고가 대비 하락 손절: {:.2}% (고가: {:.0} → 현재: {:.0})", 
                                  drop_from_high, new_highest, current_price);
                            return self.create_sell_remaining_order(time, current_price, "trailing_stop");
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(None)
    }
    
    /// 12:00 강제 정리
    fn force_close_all(&mut self, time: &TimeService) -> Result<Option<Order>, Box<dyn Error>> {
        if let Some(ref stock_code) = self.current_stock {
            if self.remaining_size > 0 {
                let current_price = self.get_current_price_dummy(stock_code)?;
                info!("⏰ [joonwoo] 12:00 시간 손절 - 모든 포지션 정리");
                return self.create_sell_all_order(time, current_price, "time_stop");
            }
        }
        Ok(None)
    }
    
    /// 전량 매도 주문 생성
    fn create_sell_all_order(&mut self, time: &TimeService, price: f64, reason: &str) -> Result<Option<Order>, Box<dyn Error>> {
        if let Some(ref stock_code) = self.current_stock {
            let order = Order {
                date: time.now().naive_local(),
                stockcode: stock_code.clone(),
                side: OrderSide::Sell,
                quantity: self.remaining_size,
                price,
                fee: 0.0,
                strategy: format!("joonwoo_{}", reason),
            };
            
            self.remaining_size = 0;
            self.state = TradingState::Closed;
            
            Ok(Some(order))
        } else {
            Ok(None)
        }
    }
    
    /// 절반 매도 주문 생성
    fn create_sell_half_order(&mut self, time: &TimeService, price: f64, reason: &str) -> Result<Option<Order>, Box<dyn Error>> {
        if let Some(ref stock_code) = self.current_stock {
            let sell_quantity = self.remaining_size / 2;
            
            let order = Order {
                date: time.now().naive_local(),
                stockcode: stock_code.clone(),
                side: OrderSide::Sell,
                quantity: sell_quantity,
                price,
                fee: 0.0,
                strategy: format!("joonwoo_{}", reason),
            };
            
            self.remaining_size -= sell_quantity;
            
            Ok(Some(order))
        } else {
            Ok(None)
        }
    }
    
    /// 잔여분 매도 주문 생성
    fn create_sell_remaining_order(&mut self, time: &TimeService, price: f64, reason: &str) -> Result<Option<Order>, Box<dyn Error>> {
        if let Some(ref stock_code) = self.current_stock {
            let order = Order {
                date: time.now().naive_local(),
                stockcode: stock_code.clone(),
                side: OrderSide::Sell,
                quantity: self.remaining_size,
                price,
                fee: 0.0,
                strategy: format!("joonwoo_{}", reason),
            };
            
            self.remaining_size = 0;
            self.state = TradingState::Closed;
            
            Ok(Some(order))
        } else {
            Ok(None)
        }
    }
    
    // TODO: 실제 API 연동으로 대체 필요
    fn get_current_price_dummy(&self, _stock_code: &str) -> Result<f64, Box<dyn Error>> {
        // 더미 구현: 시간에 따른 가격 변동 시뮬레이션
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let base_price = self.entry_price;
        let variation = ((timestamp % 60) as f64 - 30.0) * 0.001; // ±3% 범위
        Ok(base_price * (1.0 + variation))
    }
    
    fn get_available_cash_dummy(&self) -> Result<f64, Box<dyn Error>> {
        // 더미 구현: 1000만원 가정
        Ok(10_000_000.0)
    }
}

impl Model for JoonwooModel {
    fn on_start(&mut self) -> Result<(), Box<dyn Error>> {
        info!("🚀 [joonwoo] 모델 시작!");
        
        // ONNX 예측기 초기화
        match ONNXPredictor::new() {
            Ok(predictor) => {
                self.predictor = Some(predictor);
                info!("✅ [joonwoo] ONNX 예측기 초기화 완료");
            }
            Err(e) => {
                warn!("⚠️ [joonwoo] ONNX 예측기 초기화 실패: {} - 더미 모드로 진행", e);
                // ONNX 로드에 실패해도 더미 모드로 진행
            }
        }
        
        self.state = TradingState::WaitingForEntry;
        info!("📊 [joonwoo] 설정 - 손절: -{:.1}%, 익절: +{:.1}%, 추가손절: -{:.1}%", 
              self.stop_loss_pct, self.take_profit_pct, self.trailing_stop_pct);
        
        Ok(())
    }
    
    fn on_event(&mut self, time: &TimeService) -> Result<Option<Order>, Box<dyn Error>> {
        let current_time = time.now();
        let hour = current_time.hour();
        let minute = current_time.minute();
        
        // 시간대별 로직 분기
        match (hour, minute) {
            // 9:30 정확히 매수
            (9, 30) => {
                debug!("⏰ [joonwoo] 9:30 매수 타이밍");
                self.try_entry(time)
            }
            // 12:00 강제 정리
            (12, 0) => {
                debug!("⏰ [joonwoo] 12:00 강제 정리 타이밍");
                self.force_close_all(time)
            }
            // 일반 시간대 조건 체크 (9:30 ~ 12:00)
            (9, m) if m >= 30 => self.check_exit_conditions(time),
            (10, _) | (11, _) => self.check_exit_conditions(time),
            // 기타 시간대는 아무것도 하지 않음
            _ => {
                debug!("⏸️ [joonwoo] 비활성 시간대: {}:{:02}", hour, minute);
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
} 