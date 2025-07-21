pub mod backtest_api;
pub mod db_api;
pub mod korea_api;

pub use backtest_api::BacktestApi;
pub use db_api::DbApi;
pub use korea_api::{ApiMode, KoreaApi};

// 깔끔한 타입 별칭
pub type RealApi = KoreaApi;
pub type PaperApi = KoreaApi;
pub type BacktestApiType = BacktestApi;
