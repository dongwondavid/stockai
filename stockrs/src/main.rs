use std::path::PathBuf;
use stockrs::{
    config::get_config,
    errors::{StockrsError, StockrsResult},
    init_tracing,
    model::JoonwooModel,
    runner::RunnerBuilder,
    types::api::ApiType,
};
use tracing::{error, info};

fn main() -> StockrsResult<()> {
    // 로깅 초기화 (콘솔 출력만)
    init_tracing().map_err(|e| StockrsError::general(format!("로그 시스템 초기화 실패: {}", e)))?;

    info!("🚀 stockrs 시작!");

    // 설정 로드
    let config = get_config()?;
    info!("✅ 설정 로드 완료");
    info!(
        "📅 작동 기간: {} ~ {}",
        config.time_management.start_date, config.time_management.end_date
    );

    // 기본 모드 확인
    let api_type = match config.trading.default_mode.as_str() {
        "real" => {
            info!("💰 실전 거래 모드");
            ApiType::Real
        }
        "paper" => {
            info!("📊 모의투자 모드");
            ApiType::Paper
        }
        "backtest" => {
            info!("🔬 백테스팅 모드");
            ApiType::Backtest
        }
        _ => {
            return Err(StockrsError::general(format!(
                "지원하지 않는 거래 모드: {}",
                config.trading.default_mode
            )));
        }
    };

    // 거래 DB 경로 설정
    let trading_db_path = PathBuf::from(&config.database.trading_db_path);
    info!("💾 거래 DB 경로: {}", trading_db_path.display());

    // 모델 생성
    let model = Box::new(JoonwooModel::new());
    info!("🧠 joonwoo 모델 생성 완료");

    // Runner 생성 및 실행
    let mut runner = RunnerBuilder::new()
        .api_type(api_type)
        .model(model)
        .db_path(trading_db_path)
        .build()?;

    info!("🎯 Runner 생성 완료, 거래 시작!");

    // 실행 (Ctrl+C로 중단할 수 있도록 처리)
    match runner.run() {
        Ok(()) => {
            info!("✨ 거래 완료!");
        }
        Err(e) => {
            // 백테스팅 종료일 도달은 정상 종료로 처리
            if e.to_string().contains("백테스팅 종료일 도달") {
                info!("🏁 백테스팅이 정상적으로 종료되었습니다!");
            } else {
                error!("❌ 거래 중 오류 발생: {}", e);
                return Err(e);
            }
        }
    }

    info!("🏁 stockrs 종료");
    Ok(())
}
