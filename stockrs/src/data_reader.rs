use crate::types::api::{ApiType, StockApi, create_api};
use crate::types::data_reader::{DataReader, DataReaderType};
use crate::types::trading::AssetInfo;

struct ApiDataReader {
    api: Box<dyn StockApi>,
}

impl ApiDataReader {
    fn new(api_type: ApiType) -> Self {
        Self { 
            api: create_api(api_type, false) // data-only operation
        }
    }
}

impl DataReader for ApiDataReader {
    fn get_asset_info(&self) -> Result<AssetInfo, Box<dyn std::error::Error>> {
        self.api.get_balance()
    }

    fn get_avg_price(&self, _stockcode: String) -> Result<f64, Box<dyn std::error::Error>> {
        // TODO: 개별 종목 평균가 조회 로직 구현
        todo!("get average price for specific stock")
    }
}

pub struct DbDataReader;
impl DataReader for DbDataReader {
    fn get_asset_info(&self) -> Result<AssetInfo, Box<dyn std::error::Error>> {
        // 더미 구현: 임의의 자산 정보 반환
        use chrono::Local;
        println!("🔹 [DbDataReader] Asset info retrieved (simulated): 1000000.0");
        Ok(AssetInfo::new(Local::now().naive_local(), 1000000.0))
    }
    
    fn get_avg_price(&self, _stockcode: String) -> Result<f64, Box<dyn std::error::Error>> {
        // 더미 구현: 임의의 평균가 반환
        println!("🔹 [DbDataReader] Average price retrieved (simulated): 70000.0");
        Ok(70000.0)
    }
}

pub fn make_data_reader(kind: DataReaderType) -> Box<dyn DataReader> {
    match kind {
        DataReaderType::REAL => Box::new(ApiDataReader::new(ApiType::Real)),
        DataReaderType::PAPER => Box::new(ApiDataReader::new(ApiType::Paper)),
        DataReaderType::DB => Box::new(DbDataReader),
    }
}
