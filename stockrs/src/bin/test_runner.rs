use std::error::Error;
use chrono::NaiveDateTime;
use stockrs::model::Model;
use stockrs::runner::{Runner, RunnerBuilder};
use stockrs::time::TimeService;
use stockrs::types::api::ApiType;
use stockrs::types::broker::{Order, OrderSide};
use stockrs::types::data_reader::DataReaderType;

/// 테스트용 더미 모델 - 매번 간단한 주문을 생성
struct DummyModel {
    call_count: u32,
}

impl DummyModel {
    fn new() -> Self {
        Self { call_count: 0 }
    }
}

impl Model for DummyModel {
    fn on_start(&mut self) -> Result<(), Box<dyn Error>> {
        println!("📊 DummyModel started!");
        Ok(())
    }

    fn on_event(&mut self, time: &TimeService) -> Result<Option<Order>, Box<dyn Error>> {
        self.call_count += 1;
        println!("📊 DummyModel event #{} at time: {:?}", self.call_count, time.now());
        
        // 3번째 호출에서만 주문 생성 (테스트용)
        if self.call_count == 3 {
            let order = Order {
                date: chrono::DateTime::from_timestamp(1640995200, 0).unwrap().naive_local(), // 임시 날짜
                stockcode: "005930".to_string(), // 삼성전자
                side: OrderSide::Buy,
                quantity: 1,
                price: 70000.0,
                fee: 100.0,
                strategy: "DummyTest".to_string(),
            };
            println!("📊 DummyModel generated order: {:?}", order.stockcode);
            return Ok(Some(order));
        }
        
        // 5번 호출하면 종료
        if self.call_count >= 5 {
            println!("📊 DummyModel finished - stopping runner");
            return Err("Test complete".into());
        }
        
        Ok(None)
    }

    fn on_end(&mut self) -> Result<(), Box<dyn Error>> {
        println!("📊 DummyModel ended! Total calls: {}", self.call_count);
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("🚀 Testing Runner with DummyModel...");
    println!("📋 This simulates the exact prototype.py workflow:");
    println!("   1. Initialize all components");
    println!("   2. Start lifecycle (on_start)");
    println!("   3. Run main loop (time.update -> model.on_event -> broker.on_event)");
    println!("   4. End lifecycle (on_end)");
    println!();

    // Create dummy model
    let model = Box::new(DummyModel::new());
    
    // Build runner - prototype.py 스타일
    let mut runner = RunnerBuilder::new()
        .api_type(ApiType::Backtest)  // 백테스팅 모드로 테스트
        .model(model)
        .db_path("test.db")
        .data_reader_type(DataReaderType::DB)
        .build()?;

    println!("🏃 Starting Runner...");
    
    // Run until error (our dummy model will trigger an error after 5 calls)
    match runner.run() {
        Ok(_) => println!("✅ Runner completed successfully"),
        Err(e) => {
            if e.to_string().contains("Test complete") {
                println!("✅ Test completed as expected: {}", e);
            } else {
                println!("❌ Runner failed: {}", e);
                return Err(e);
            }
        }
    }

    println!("🎉 All tests passed! prototype.py pattern working in Rust!");
    Ok(())
} 