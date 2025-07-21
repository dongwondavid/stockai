use tracing_log::LogTracer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter, Registry};

/// tracing 초기화 함수  
/// env_logger 대신 사용하며, JSON 구조화 로그와 스팬 트레이싱을 제공합니다.
pub fn init_tracing() {
    // 기존 log! 매크로 호환
    LogTracer::init().expect("Failed to set LogTracer");

    // JSON 구조화 로그 + RUST_LOG 기반 레벨 필터링
    let subscriber = Registry::default()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().json());

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
}

pub mod sector_manager;

pub use sector_manager::{DateSectorCache, FiveMinData, SectorManager, StockInfo};
