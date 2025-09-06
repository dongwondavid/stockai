use std::path::PathBuf;
use stockrs::{
    utility::config::{Config, set_global_config},
    utility::errors::{StockrsError, StockrsResult},
    model::{JoonwooModel, DongwonModel, MinseopModel},
    runner::RunnerBuilder,
    utility::types::api::ApiType,
};
use tracing::{error, info};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter, Registry};
use clap::Parser;

#[derive(Parser)]
#[command(name = "stockrs")]
#[command(about = "Stock trading system with AI models")]
struct Args {
    /// 설정 파일 경로 (기본값: config.toml)
    #[arg(short, long, default_value = "config.toml")]
    config: String,
    
    /// 실행 모드 (real/paper/backtest, 기본값: 설정 파일의 default_mode)
    #[arg(short, long)]
    mode: Option<String>,
    
    /// 거래 DB 경로 (기본값: 설정 파일의 trading_db_path)
    #[arg(long)]
    trading_db: Option<String>,

    /// 모델 선택 (joonwoo/dongwon/minseop). 기본값: joonwoo
    #[arg(long, default_value = "joonwoo")]
    model: String,
}

fn main() -> StockrsResult<()> {
    // 명령행 인수 파싱
    let args = Args::parse();
    
    // 로깅 초기화 (콘솔 출력만)
    init_tracing().map_err(|e| StockrsError::general(format!("로그 시스템 초기화 실패: {}", e)))?;

    info!("🚀 stockrs 시작!");
    info!("📁 설정 파일: {}", args.config);

    // 설정 로드 (명령행에서 지정된 파일 사용)
    let config = Config::load_from_file(&args.config)?;
    
    // 전역 설정으로 설정 (다른 모듈에서 get_config()로 접근할 수 있도록)
    set_global_config(config.clone())?;
    
    info!("✅ 설정 로드 완료");
    info!(
        "📅 작동 기간: {} ~ {}",
        config.time_management.start_date, config.time_management.end_date
    );

    // 실행 모드 결정 (명령행 인수 우선, 없으면 설정 파일 값 사용)
    let mode = args.mode.as_deref().unwrap_or(&config.trading.default_mode);
    let api_type = match mode {
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
                mode
            )));
        }
    };

    // 거래 DB 경로 설정 (명령행 인수 우선, 없으면 설정 파일 값 사용)
    let trading_db_path = PathBuf::from(
        args.trading_db.as_deref().unwrap_or(&config.database.trading_db_path)
    );
    info!("💾 거래 DB 경로: {}", trading_db_path.display());

    // 모델 생성
    let model_name = args.model.to_lowercase();
    let model: Box<dyn stockrs::model::Model> = match model_name.as_str() {
        "dongwon" => {
            info!("🧠 dongwon 모델 생성");
            Box::new(DongwonModel::new())
        }
        "minseop" => {
            info!("🧠 minseop 모델 생성");
            Box::new(MinseopModel::new())
        }
        "joonwoo" | _ => {
            info!("🧠 joonwoo 모델 생성");
            Box::new(JoonwooModel::new())
        }
    };

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


/// tracing 초기화 함수
/// env_logger 대신 사용하며, JSON 구조화 로그와 스팬 트레이싱을 제공합니다.
fn init_tracing() -> Result<(), String> {
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