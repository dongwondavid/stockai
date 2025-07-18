use std::error::Error;
use std::thread;
use std::time::Duration;

use crate::time::TimeService;
use crate::model::Model;
use crate::broker::StockBroker;
use crate::db_manager::DBManager;
use crate::types::api::{ApiType, StockApi, create_api};
use crate::types::data_reader::DataReaderType;

/// prototype.py의 runner 클래스와 동일한 구조
pub struct Runner {
    /// "real" or "paper" or "backtest" - prototype.py의 self.type
    pub api_type: ApiType,
    
    /// prototype.py의 각 컴포넌트들
    pub time: TimeService,
    pub model: Box<dyn Model>,
    pub broker: StockBroker,
    pub db_manager: DBManager,
    
    /// prototype.py의 self.stop_condition
    pub stop_condition: bool,
}

impl Runner {
    /// prototype.py의 __init__과 동일한 초기화 로직
    pub fn new(
        api_type: ApiType,
        model: Box<dyn Model>,
        db_path: std::path::PathBuf,
        data_reader_type: DataReaderType,
    ) -> Result<Self, Box<dyn Error>> {
        // prototype.py: self.broker_api = real_api() if self.type == "real" else paper_api() if self.type == "paper" else db_api()
        let broker_api = create_api(api_type, true);
        
        Ok(Runner {
            api_type,
            time: TimeService::new(),
            model,
            broker: StockBroker::new(broker_api),
            db_manager: DBManager::new(db_path, data_reader_type)?,
            stop_condition: false,
        })
    }

    /// prototype.py의 run() 메서드와 동일한 메인 루프
    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // prototype.py: on start
        self.time.on_start()?;
        self.model.on_start()?;
        self.db_manager.on_start()?;
        self.broker.on_start()?;

        // prototype.py: while not self.stop_condition:
        while !self.stop_condition {
            // prototype.py: self.time.update()
            self.time.update()?;
            
            // prototype.py: wait_until_next_event(self.time)
            self.wait_until_next_event()?;

            // prototype.py: result = self.model.on_event(self.time)
            let result = self.model.on_event(&self.time)?;

            // prototype.py: broker on event
            if let Some(order) = result {
                // prototype.py: broker_result = self.broker.on_event(result)
                let broker_result = self.broker.on_event(order, &self.db_manager);
                
                // prototype.py: if broker_result is not None: self.db_manager.on_event(broker_result)
                if broker_result.is_ok() {
                    self.db_manager.on_event(())?;
                }
            }
        }

        // prototype.py: on end
        self.model.on_end()?;
        self.db_manager.on_end()?;
        self.broker.on_end()?;

        Ok(())
    }

    /// prototype.py의 wait_until_next_event 함수와 동일
    fn wait_until_next_event(&self) -> Result<(), Box<dyn Error>> {
        // TODO: 실제 시간 대기 로직 구현
        // 현재는 간단한 sleep으로 시뮬레이션
        thread::sleep(Duration::from_secs(1));
        Ok(())
    }

    /// runner 중지 요청
    pub fn stop(&mut self) {
        self.stop_condition = true;
    }
}

/// prototype.py의 구조를 따른 Builder 패턴
pub struct RunnerBuilder {
    api_type: ApiType,
    model: Option<Box<dyn Model>>,
    db_path: Option<std::path::PathBuf>,
    data_reader_type: DataReaderType,
}

impl RunnerBuilder {
    pub fn new() -> Self {
        Self {
            api_type: ApiType::Backtest, // 기본값은 백테스팅
            model: None,
            db_path: None,
            data_reader_type: DataReaderType::DB, // 기본값
        }
    }

    pub fn api_type(mut self, api_type: ApiType) -> Self {
        self.api_type = api_type;
        self
    }

    pub fn model(mut self, model: Box<dyn Model>) -> Self {
        self.model = Some(model);
        self
    }

    pub fn db_path<P: Into<std::path::PathBuf>>(mut self, path: P) -> Self {
        self.db_path = Some(path.into());
        self
    }

    pub fn data_reader_type(mut self, data_reader_type: DataReaderType) -> Self {
        self.data_reader_type = data_reader_type;
        self
    }

    pub fn build(self) -> Result<Runner, Box<dyn Error>> {
        let model = self.model.ok_or("Model is required")?;
        let db_path = self.db_path.ok_or("DB path is required")?;
        
        Runner::new(self.api_type, model, db_path, self.data_reader_type)
    }
}
