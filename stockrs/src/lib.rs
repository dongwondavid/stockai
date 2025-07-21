use tracing_log::LogTracer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter, Registry};

/// tracing 초기화 함수
/// env_logger 대신 사용하며, JSON 구조화 로그와 스팬 트레이싱을 제공합니다.
pub fn init_tracing() -> Result<(), String> {
    // 기존 log! 매크로 호환
    LogTracer::init().map_err(|e| {
        eprintln!("Failed to set LogTracer: {}", e);
        format!("로그 시스템 초기화 실패: {}", e)
    })?;

    // JSON 구조화 로그 + RUST_LOG 기반 레벨 필터링 + 함수명/모듈명 포함
    let subscriber = Registry::default()
        .with(EnvFilter::from_default_env())
        .with(
            fmt::layer()
                .json()
                .with_file(true)
                .with_line_number(true)
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true),
        );

    tracing::subscriber::set_global_default(subscriber).map_err(|e| {
        eprintln!("Failed to set tracing subscriber: {}", e);
        format!("로그 시스템 초기화 실패: {}", e)
    })?;

    Ok(())
}

pub mod apis;
pub mod broker;
pub mod config;
pub mod db_manager;
pub mod errors;
pub mod holiday_checker;
pub mod model;
pub mod runner;
pub mod time;
pub mod types;
