pub mod db_api;
pub mod korea_api;

pub use db_api::DbApi;
pub use korea_api::{KoreaApi, ApiMode};

// 깔끔한 타입 별칭
pub type RealApi = KoreaApi;
pub type PaperApi = KoreaApi; 