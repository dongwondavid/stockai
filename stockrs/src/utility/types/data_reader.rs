use crate::utility::types::trading::AssetInfo;

pub enum DataReaderType {
    DB,
    PAPER,
    REAL,
}

pub trait DataReader {
    fn get_asset_info(&self) -> Result<AssetInfo, Box<dyn std::error::Error>>;
    fn get_avg_price(&self, stockcode: String) -> Result<f64, Box<dyn std::error::Error>>;
}
