use crate::api::koreainvestapi::get_domestic006_result;
use crate::api::result::Domestic006Result;
use crate::types::api::ApiEnv;
use crate::types::data_reader::{DataReader, DataReaderType};
use crate::types::trading::AssetInfo;


struct KiDataReader {
    env: ApiEnv,
}

impl KiDataReader {
    fn new(env: ApiEnv) -> Self {
        Self { env }
    }
}

impl DataReader for KiDataReader {
    fn get_asset_info(&self) -> Result<AssetInfo, Box<dyn std::error::Error>> {
        let result: Domestic006Result = get_domestic006_result(self.env)?;
        Ok(result.into())
    }

    fn get_avg_price(&self, stockcode: String) -> Result<f64, Box<dyn std::error::Error>> {
        let result: Domestic006Result = get_domestic006_result(self.env)?;
        let avg = result.get_pchs_avg_pric(stockcode)?;
        Ok(avg)
    }
}

pub struct DbDataReader;
impl DataReader for DbDataReader {
    fn get_asset_info(&self) -> Result<AssetInfo, Box<dyn std::error::Error>> {
        todo!("get asset info");
    }
    fn get_avg_price(&self, stockcode: String) -> Result<f64, Box<dyn std::error::Error>> {
        todo!("get avg price");
    }
}

pub fn make_data_reader(kind: DataReaderType) -> Box<dyn DataReader> {
    match kind {
        DataReaderType::REAL => Box::new(KiDataReader::new(ApiEnv::Real)),
        DataReaderType::DB => Box::new(DbDataReader),
        DataReaderType::PAPER => Box::new(KiDataReader::new(ApiEnv::Paper)),
    }
}
