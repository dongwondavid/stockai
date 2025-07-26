pub mod features;

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

use ndarray::Array2;
use ort::{Environment, SessionBuilder, Value};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::utility::apis::db_api::DbApi;
use crate::utility::apis::korea_api::KoreaApi;
use crate::utility::config::get_config;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::types::trading::TradingMode;
use features::{calculate_features_for_stock_optimized};

#[derive(Debug, Serialize, Deserialize)]
pub struct StockFeatures {
    pub stock_code: String,
    pub features: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PredictionResult {
    pub stock_code: String,
    pub probability: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ONNXModelInfo {
    onnx_model_path: String,
    features: Vec<String>,
    feature_count: usize,
    input_name: String,
    input_shape: Vec<usize>,
    output_name: String,
    output_shape: Vec<usize>,
}

pub struct ONNXPredictor {
    session: ort::Session,
    features: Vec<String>,
    extra_stocks_set: HashSet<String>,
    trading_dates: Vec<String>,
    trading_mode: TradingMode,
}

impl ONNXPredictor {
    /// ONNX 모델을 로드하고 Predictor를 생성합니다
    pub fn new(trading_mode: TradingMode) -> StockrsResult<Self> {
        let config = get_config()?;

        // config에서 경로들 로드
        let model_info_path = &config.onnx_model.model_info_path;
        let extra_stocks_path = &config.onnx_model.extra_stocks_file_path;
        let features_path = &config.onnx_model.features_file_path;
        let trading_dates_path = &config.time_management.trading_dates_file_path;

        // 모델 정보 로드
        let model_info = Self::load_model_info(model_info_path)?;

        // ONNX Runtime 환경 초기화
        let environment = Arc::new(
            Environment::builder()
                .with_name("stockrs_predictor")
                .build()
                .map_err(|e| {
                    StockrsError::model_loading(format!("ONNX Runtime 환경 초기화 실패: {}", e))
                })?,
        );

        // 세션 생성
        let session = SessionBuilder::new(&environment)
            .map_err(|e| {
                StockrsError::model_loading(format!("ONNX SessionBuilder 생성 실패: {}", e))
            })?
            .with_model_from_file(&model_info.onnx_model_path)
            .map_err(|e| StockrsError::model_loading(format!("ONNX 모델 파일 로드 실패: {}", e)))?;

        // extra_stocks.txt 로드
        let extra_stocks_set = Self::load_extra_stocks(extra_stocks_path)?;

        // features.txt 로드
        let features = Self::load_features(features_path)?;

        info!("ONNX 모델 로드 완료: {}", model_info.onnx_model_path);
        info!(
            "특징 수: {}, 제외 종목 수: {}",
            features.len(),
            extra_stocks_set.len()
        );
        
        // 1일봉 날짜 목록 로드
        let file = File::open(trading_dates_path)
            .map_err(|e| StockrsError::prediction(format!("1일봉 날짜 파일 읽기 실패: {}", e)))?;
        
        let reader = BufReader::new(file);
        let mut trading_dates: Vec<String> = Vec::new();
        
        for line in reader.lines() {
            let line = line.map_err(|e| StockrsError::prediction(format!("파일 읽기 오류: {}", e)))?;
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                trading_dates.push(trimmed.to_string());
            }
        }

        Ok(ONNXPredictor {
            session,
            features,
            extra_stocks_set,
            trading_dates,
            trading_mode,
        })
    }

    /// 최고 확률 종목을 예측합니다 (solomon의 핵심 로직 구현) - 최적화됨
    pub fn predict_top_stock(
        &mut self,
        date: &str,
        db: &Connection,
        daily_db: &Connection,
    ) -> StockrsResult<String> {
        info!(
            "🔮 [ONNX] {}일 최고 확률 종목 예측 중... (모드: {:?})",
            date, self.trading_mode
        );

        // 거래일 리스트가 없으면 로드 (백테스팅 모드에서만 필요)
        if self.trading_mode == TradingMode::Backtest && self.trading_dates.is_empty() {
            return Err(StockrsError::prediction(
                "거래일 리스트가 없습니다".to_string(),
            ));
        }

        // 투자 모드에 따라 거래대금 상위 30개 종목 조회
        let top_stocks = match self.trading_mode {
            TradingMode::Real | TradingMode::Paper => {
                // 실전/모의투자: 정보 API로 실시간 거래대금 순위 조회
                let korea_api = KoreaApi::new_info()?;
                korea_api.get_top_amount_stocks(30)?
            }
            TradingMode::Backtest => {
                // 백테스팅: DB에서 과거 데이터로 거래대금 계산
                let db_api = DbApi::new()?;
                db_api.get_top_amount_stocks(date, 30)?
            }
        };

        debug!("거래대금 상위 30개 종목: {:?}", top_stocks);

        // extra_stocks.txt에 없는 종목들만 필터링
        let filtered_stocks: Vec<String> = top_stocks
            .into_iter()
            .filter(|stock| !self.extra_stocks_set.contains(stock))
            .collect();

        debug!("필터링된 종목 수: {}개", filtered_stocks.len());

        if filtered_stocks.is_empty() {
            return Err(StockrsError::prediction(
                "분석할 종목이 없습니다".to_string(),
            ));
        }

        // 각 종목에 대해 특징 계산 (최적화됨)
        let features_data =
            self.calculate_features_for_stocks(&filtered_stocks, date, db, daily_db)?;

        if features_data.is_empty() {
            return Err(StockrsError::prediction(
                "계산된 특징이 없습니다".to_string(),
            ));
        }

        // ONNX 모델로 예측 (최적화됨)
        debug!("ONNX 모델로 예측 시작...");
        let mut predictions = self.predict_with_onnx_model(&features_data)?;

        // 결과 정렬 (확률 높은 순)
        predictions.sort_by(|a, b| {
            b.probability
                .partial_cmp(&a.probability)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 최고 확률 종목 반환
        if let Some(best_stock) = predictions.first() {
            info!(
                "최고 확률 종목: {} ({:.4})",
                best_stock.stock_code, best_stock.probability
            );
            Ok(best_stock.stock_code.clone())
        } else {
            Err(StockrsError::prediction("예측 결과가 없습니다".to_string()))
        }
    }

    /// 각 종목에 대해 특징 계산 (최적화됨)
    fn calculate_features_for_stocks(
        &self,
        stocks: &[String],
        date: &str,
        db: &Connection,
        daily_db: &Connection,
    ) -> StockrsResult<Vec<StockFeatures>> {
        // 벡터 사전 할당으로 메모리 최적화
        let mut features_data = Vec::with_capacity(stocks.len());

        for stock_code in stocks {
            match calculate_features_for_stock_optimized(
                db,
                daily_db,
                stock_code,
                date,
                &self.features,
                &self.trading_dates,
            ) {
                Ok(feature_values) => {
                    features_data.push(StockFeatures {
                        stock_code: stock_code.clone(),
                        features: feature_values,
                    });
                    debug!("종목 {} 특징 계산 완료", stock_code);
                }
                Err(e) => {
                    warn!(
                        "종목 {} 특징 계산 실패: {} - 기본값으로 진행",
                        stock_code, e
                    );
                    // 실패한 경우에도 기본값으로 특징 계산을 시도
                    let default_features = vec![0.0; self.features.len()];
                    features_data.push(StockFeatures {
                        stock_code: stock_code.clone(),
                        features: default_features,
                    });
                }
            }
        }

        Ok(features_data)
    }

    /// ONNX 모델로 예측 수행 (solomon 포팅) - 최적화됨
    fn predict_with_onnx_model(
        &self,
        features_data: &[StockFeatures],
    ) -> StockrsResult<Vec<PredictionResult>> {
        // 벡터 사전 할당으로 메모리 최적화
        let mut results = Vec::with_capacity(features_data.len());

        debug!("=== ONNX 모델 예측 시작 ===");
        debug!("입력 특징 수: {}", self.features.len());
        debug!("예측할 종목 수: {}", features_data.len());

        for (idx, stock_data) in features_data.iter().enumerate() {
            debug!(
                "--- 종목 {} 예측 중 ({}/{}) ---",
                stock_data.stock_code,
                idx + 1,
                features_data.len()
            );

            // 1. 특성 벡터를 f32 배열로 변환 (NaN이나 무한대 값 처리) - 최적화됨
            let input_vec: Vec<f32> = stock_data
                .features
                .iter()
                .map(|&x| {
                    let val = x as f32;
                    if val.is_nan() || val.is_infinite() {
                        0.0
                    } else {
                        val
                    }
                })
                .collect();

            // 2. ndarray 배열로 변환 (배치 1개, 특성 수만큼)
            let input_array = Array2::from_shape_vec((1, input_vec.len()), input_vec)
                .map_err(|e| StockrsError::prediction(format!("입력 배열 생성 실패: {}", e)))?;

            // 3. ONNX 텐서 생성
            use ndarray::CowArray;
            let input_dyn = input_array.into_dyn();
            let input_cow = CowArray::from(input_dyn);

            let input_tensor =
                Value::from_array(&self.session.allocator() as *const _ as *mut _, &input_cow)
                    .map_err(|e| StockrsError::prediction(format!("입력 텐서 생성 실패: {}", e)))?;

            // 4. 예측 수행
            let outputs = self
                .session
                .run(vec![input_tensor])
                .map_err(|e| StockrsError::prediction(format!("ONNX 모델 실행 실패: {}", e)))?;

            // 5. 두 번째 출력에서 확률 추출
            let output_value = &outputs[1];

            let probability = if let Ok(output_tensor) = output_value.try_extract::<f32>() {
                let view = output_tensor.view();
                let slice = view.as_slice().ok_or_else(|| {
                    StockrsError::prediction(format!(
                        "텐서 슬라이스 추출 실패 (종목: {})",
                        stock_data.stock_code
                    ))
                })?;

                if slice.len() >= 2 {
                    slice[1] as f64
                } else if slice.len() == 1 {
                    slice[0] as f64
                } else {
                    return Err(StockrsError::prediction(format!(
                        "유효하지 않은 출력 텐서 크기: {} (종목: {})",
                        slice.len(),
                        stock_data.stock_code
                    )));
                }
            } else {
                return Err(StockrsError::prediction(format!(
                    "텐서 추출 실패 (종목: {})",
                    stock_data.stock_code
                )));
            };

            let probability = probability.clamp(0.0, 1.0);

            results.push(PredictionResult {
                stock_code: stock_data.stock_code.clone(),
                probability,
            });

            debug!(
                "종목 {} 예측 완료: {:.6}",
                stock_data.stock_code, probability
            );
        }

        info!("=== ONNX 모델 예측 완료 ===");
        info!("총 예측 종목 수: {}개", results.len());

        Ok(results)
    }

    // 유틸리티 함수들
    fn load_model_info(path: &str) -> StockrsResult<ONNXModelInfo> {
        if !Path::new(path).exists() {
            return Err(StockrsError::file_not_found(format!(
                "ONNX 모델 정보 파일을 찾을 수 없습니다: {}",
                path
            )));
        }

        let file = File::open(path)
            .map_err(|e| StockrsError::file_io(format!("모델 정보 파일 읽기 실패: {}", e)))?;
        let model_info: ONNXModelInfo = serde_json::from_reader(file)
            .map_err(|e| StockrsError::file_parse(format!("모델 정보 파싱 실패: {}", e)))?;
        Ok(model_info)
    }

    fn load_extra_stocks(path: &str) -> StockrsResult<HashSet<String>> {
        if !Path::new(path).exists() {
            return Err(StockrsError::file_not_found(format!(
                "extra_stocks.txt 파일이 없습니다: {}",
                path
            )));
        }

        let file = File::open(path)
            .map_err(|e| StockrsError::file_io(format!("extra_stocks 파일 읽기 실패: {}", e)))?;
        let reader = BufReader::new(file);
        let mut extra_stocks = HashSet::new();

        for line in reader.lines() {
            let line = line.map_err(|e| StockrsError::file_io(format!("라인 읽기 실패: {}", e)))?;
            let line = line.trim();

            // 헤더나 빈 줄 건너뛰기
            if line.is_empty() || line.contains("=") || line.contains("총") {
                continue;
            }

            extra_stocks.insert(line.to_string());
        }

        if extra_stocks.is_empty() {
            return Err(StockrsError::file_parse(format!(
                "extra_stocks.txt 파일이 비어있거나 파싱할 수 없습니다: {}",
                path
            )));
        }

        debug!("extra_stocks.txt에서 {}개 종목 로드됨", extra_stocks.len());
        Ok(extra_stocks)
    }

    fn load_features(path: &str) -> StockrsResult<Vec<String>> {
        if !Path::new(path).exists() {
            return Err(StockrsError::file_not_found(format!(
                "features.txt 파일이 없습니다: {}",
                path
            )));
        }

        let file = File::open(path)
            .map_err(|e| StockrsError::file_io(format!("features 파일 읽기 실패: {}", e)))?;
        let reader = BufReader::new(file);
        let mut features = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| StockrsError::file_io(format!("라인 읽기 실패: {}", e)))?;
            let line = line.trim();

            if !line.is_empty() {
                features.push(line.to_string());
            }
        }

        debug!("features.txt에서 {}개 특징 로드됨", features.len());
        Ok(features)
    }
}
