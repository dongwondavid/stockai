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
        
        todo!("실제 가격 조회 API와 잔고 조회 API 연동하여 매수 주문 생성")
    }
    
    /// 손절/익절 조건 체크
    fn check_exit_conditions(&mut self, time: &TimeService) -> Result<Option<Order>, Box<dyn Error>> {
        if let Some(ref _stock_code) = self.current_stock {
            todo!("실제 API를 통한 현재가 조회 및 손절/익절 조건 검사 구현")
        }
        Ok(None)
    }
    
    /// 12:00 강제 정리
    fn force_close_all(&mut self, time: &TimeService) -> Result<Option<Order>, Box<dyn Error>> {
        if let Some(ref _stock_code) = self.current_stock {
            if self.remaining_size > 0 {
                info!("⏰ [joonwoo] 12:00 시간 손절 - 모든 포지션 정리");
                todo!("실제 API를 통한 현재가 조회 및 강제 정리 주문 생성")
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