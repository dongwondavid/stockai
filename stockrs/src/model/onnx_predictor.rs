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
use tracing::{debug, info, error};

use crate::utility::apis::db_api::DbApi;
use crate::utility::apis::korea_api::KoreaApi;
use crate::utility::config::get_config;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::types::trading::TradingMode;
use features::calculate_features_for_stock_optimized;

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

pub struct ONNXPredictor {
    session: ort::Session,
    features: Vec<String>,
    included_stocks_set: HashSet<String>,
    trading_dates: Vec<String>,
    trading_mode: TradingMode,
}

impl ONNXPredictor {
    /// ONNX 모델을 로드하고 Predictor를 생성합니다
    pub fn new(trading_mode: TradingMode) -> StockrsResult<Self> {
        let config = get_config()?;

        // config에서 경로들 로드
        let model_file_path = &config.onnx_model.model_file_path;
        let included_stocks_path = &config.onnx_model.included_stocks_file_path;
        let features_path = &config.onnx_model.features_file_path;
        let trading_dates_path = &config.time_management.trading_dates_file_path;

        // ONNX Runtime 환경 초기화
        let environment = Arc::new(
            Environment::builder()
                .with_name("stockrs_predictor")
                .build()
                .map_err(|e| {
                    StockrsError::model_loading(format!("ONNX Runtime 환경 초기화 실패: {}", e))
                })?,
        );

        println!("ONNX Runtime 환경 초기화 완료");

        // 세션 생성
        let session = SessionBuilder::new(&environment)
            .map_err(|e| {
                StockrsError::model_loading(format!("ONNX SessionBuilder 생성 실패: {}", e))
            })?
            .with_model_from_file(model_file_path)
            .map_err(|e| StockrsError::model_loading(format!("ONNX 모델 파일 로드 실패: {}", e)))?;

        // stocks.txt 로드
        let included_stocks_set = Self::load_included_stocks(included_stocks_path)?;

        // features.txt 로드
        let features = Self::load_features(features_path)?;

        println!("ONNX 모델 로드 완료: {}", model_file_path);
        println!(
            "특징 수: {}, 포함 종목 수: {}",
            features.len(),
            included_stocks_set.len()
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
            included_stocks_set,
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
    ) -> StockrsResult<Option<String>> {
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
                let (date_start, date_end) = crate::model::onnx_predictor::features::utils::get_time_range_for_date(date);
                let db_api = DbApi::new()?;
                db_api.get_top_amount_stocks(date, 30, &date_start, &date_end)?
            }
        };

        debug!("거래대금 상위 30개 종목: {:?}", top_stocks);

        // stocks.txt에 있는 종목들만 필터링
        let filtered_stocks: Vec<String> = top_stocks
            .into_iter()
            .filter(|stock| self.included_stocks_set.contains(stock))
            .collect();

        debug!("필터링된 종목 수: {}개", filtered_stocks.len());

        if filtered_stocks.is_empty() {
            return Err(StockrsError::prediction(
                "분석할 종목이 없습니다".to_string(),
            ));
        }

        // 필터링 후 15개 초과로 개수가 남았다면 순위대로 15개만 남겨서 사용
        let final_stocks = if filtered_stocks.len() > 15 {
            debug!("필터링된 종목이 15개 초과 ({}개) - 상위 15개만 사용", filtered_stocks.len());
            filtered_stocks.into_iter().take(15).collect::<Vec<String>>()
        } else {
            filtered_stocks
        };

        debug!("최종 분석 대상 종목 수: {}개", final_stocks.len());

        // 각 종목에 대해 특징 계산 (최적화됨)
        let features_data =
            self.calculate_features_for_stocks(&final_stocks, date, db, daily_db)?;

        if features_data.is_empty() {
            return Err(StockrsError::prediction(
                "계산된 특징이 없습니다".to_string(),
            ));
        }

        // println!("{:?}", features_data);

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
            Ok(Some(best_stock.stock_code.clone()))
        } else {
            info!("🔮 [ONNX] 예측 결과가 없습니다 - 매수하지 않음");
            Ok(None)
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
            info!("🔍 [ONNX] 종목 {} 특징 계산 시작", stock_code);
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
                    info!("✅ [ONNX] 종목 {} 특징 계산 완료", stock_code);
                }
                Err(e) => {
                    error!("❌ [ONNX] 종목 {} 특징 계산 실패: {}", stock_code, e);
                    return Err(StockrsError::prediction(format!("종목 {} 특징 계산 실패: {}", stock_code, e)));
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

            // 5. 첫 번째 출력에서 클래스 정보 확인
            let class_output = &outputs[0];
            let predicted_class = if let Ok(class_tensor) = class_output.try_extract::<i64>() {
                let view = class_tensor.view();
                let slice = view.as_slice().ok_or_else(|| {
                    StockrsError::prediction(format!(
                        "클래스 텐서 슬라이스 추출 실패 (종목: {})",
                        stock_data.stock_code
                    ))
                })?;

                if slice.is_empty() {
                    return Err(StockrsError::prediction(format!(
                        "빈 클래스 텐서 (종목: {})",
                        stock_data.stock_code
                    )));
                }
                slice[0]
            } else {
                return Err(StockrsError::prediction(format!(
                    "클래스 텐서 추출 실패 (종목: {})",
                    stock_data.stock_code
                )));
            };

            // 클래스가 0이면 결과에 추가하지 않음
            if predicted_class == 0 {
                info!(
                    "종목 {} 예측 결과: 클래스 0 (매수하지 않음)",
                    stock_data.stock_code
                );
                continue;
            }

            // 6. 두 번째 출력에서 확률 추출
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
    fn load_included_stocks(path: &str) -> StockrsResult<HashSet<String>> {
        if !Path::new(path).exists() {
            return Err(StockrsError::file_not_found(format!(
                "stocks.txt 파일이 없습니다: {}",
                path
            )));
        }

        let file = File::open(path)
            .map_err(|e| StockrsError::file_io(format!("stocks 파일 읽기 실패: {}", e)))?;
        let reader = BufReader::new(file);
        let mut included_stocks = HashSet::new();

        for line in reader.lines() {
            let line = line.map_err(|e| StockrsError::file_io(format!("라인 읽기 실패: {}", e)))?;
            let line = line.trim();

            // 빈 줄만 건너뛰기 (stocks.txt는 깔끔한 종목코드 목록)
            if line.is_empty() {
                continue;
            }

            included_stocks.insert(line.to_string());
        }

        if included_stocks.is_empty() {
            return Err(StockrsError::file_parse(format!(
                "stocks.txt 파일이 비어있거나 파싱할 수 없습니다: {}",
                path
            )));
        }

        debug!("stocks.txt에서 {}개 종목 로드됨", included_stocks.len());
        Ok(included_stocks)
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
