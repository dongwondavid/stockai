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
use serde_json;
use tracing::{debug, info, error};

use crate::utility::apis::db_api::DbApi;
use crate::utility::apis::korea_api::KoreaApi;
use crate::utility::config::get_config;
use crate::utility::errors::{StockrsError, StockrsResult};
use crate::utility::types::trading::TradingMode;
use crate::time::TimeService;
use features::calculate_features_for_stock_optimized;

#[derive(Debug, Serialize, Deserialize)]
pub struct StockFeatures {
    pub stock_code: String,
    pub features: Vec<f64>,
}

fn argmax_f64(xs: &[f64]) -> Option<usize> {
    if xs.is_empty() { return None; }
    let mut best_i = 0usize;
    let mut best_v = xs[0];
    for (i, &v) in xs.iter().enumerate().skip(1) {
        if v.partial_cmp(&best_v).unwrap_or(std::cmp::Ordering::Less).is_gt() {
            best_v = v; best_i = i;
        }
    }
    Some(best_i)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PredictionResult {
    pub stock_code: String,
    pub probability: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegressionPredictionResult {
    pub stock_code: String,
    pub value: f64,
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
        // 특징 계산에 사용하는 거래일 파일은 온nx 모델 섹션의 별도 경로를 사용
        let trading_dates_path = &config.onnx_model.features_trading_dates_file_path;

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
                // KIS 응답은 접두사 'A'가 없는 단축코드이므로, 이후 로직(stocks.txt 비교, DB 조회)의 일관성을 위해 'A' 접두사를 부여
                let korea_api = KoreaApi::new_info()?;
                let codes = korea_api.get_top_amount_stocks(30)?;
                codes
                    .into_iter()
                    .map(|c| if c.starts_with('A') { c } else { format!("A{}", c) })
                    .collect::<Vec<String>>()
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

        // 필터링 후 10개 초과로 개수가 남았다면 순위대로 10개만 남겨서 사용
        let final_stocks = if filtered_stocks.len() > 10 {
            debug!("필터링된 종목이 10개 초과 ({}개) - 상위 10개만 사용", filtered_stocks.len());
            filtered_stocks.into_iter().take(10).collect::<Vec<String>>()
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
            // 저장 시도 (설정에서 허용될 때만)
            if let Err(e) = self.save_model_record_classifier(
                date,
                &final_stocks,
                &features_data,
                &predictions,
                &best_stock.stock_code,
                best_stock.probability,
                "onnx_classifier",
            ) { error!("[ONNX] 모델 기록 저장 실패: {}", e); }
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

            // println!("{:?}", input_vec);

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

            // println!("predicted_class: {}", predicted_class);
            // println!("outputs: {:?}", outputs);

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

    /// 회귀 ONNX 모델을 사용해 최고 종목을 예측 (배치 입력)
    pub fn predict_top_stock_regression(
        &mut self,
        date: &str,
        db: &Connection,
        daily_db: &Connection,
    ) -> StockrsResult<Option<(String, f64, Vec<RegressionPredictionResult>)>> {
        info!(
            "🧮 [ONNX-REG] {}일 최고 회귀값 종목 예측 중... (모드: {:?})",
            date, self.trading_mode
        );

        if self.trading_mode == TradingMode::Backtest && self.trading_dates.is_empty() {
            return Err(StockrsError::prediction("거래일 리스트가 없습니다".to_string()));
        }

        // 투자 모드별 거래대금 상위 30개 → stocks.txt 포함 종목으로 필터 → 상위 10개
        let top_stocks = match self.trading_mode {
            TradingMode::Real | TradingMode::Paper => {
                let korea_api = KoreaApi::new_info()?;
                let codes = korea_api.get_top_amount_stocks(30)?;
                codes
                    .into_iter()
                    .map(|c| if c.starts_with('A') { c } else { format!("A{}", c) })
                    .collect::<Vec<String>>()
            }
            TradingMode::Backtest => {
                let (date_start, date_end) = crate::model::onnx_predictor::features::utils::get_time_range_for_date(date);
                let db_api = DbApi::new()?;
                db_api.get_top_amount_stocks(date, 30, &date_start, &date_end)?
            }
        };

        let filtered_stocks: Vec<String> = top_stocks
            .into_iter()
            .filter(|s| self.included_stocks_set.contains(s))
            .collect();

        if filtered_stocks.is_empty() {
            return Err(StockrsError::prediction("분석할 종목이 없습니다".to_string()));
        }

        let final_stocks = if filtered_stocks.len() > 10 {
            filtered_stocks.into_iter().take(10).collect::<Vec<String>>()
        } else {
            filtered_stocks
        };

        // 특징 계산 (N종목 배치 입력용)
        let features_data = self.calculate_features_for_stocks(&final_stocks, date, db, daily_db)?;
        if features_data.is_empty() {
            return Err(StockrsError::prediction("계산된 특징이 없습니다".to_string()));
        }

        // ONNX 회귀 추론 (한 번에)
        let (best_idx, values) = self.predict_with_onnx_regression(&features_data)?;
        // 방어: best_idx가 범위를 벗어나면 argmax로 대체 (정렬로 최종 선택)
        if !(best_idx >= 0 && (best_idx as usize) < features_data.len()) {
            debug!("[ONNX-REG] best_idx={} 범위 밖 → argmax로 대체", best_idx);
            let _ = argmax_f64(&values).unwrap_or(0);
        }

        // 결과 매핑 및 정렬
        let mut all = Vec::with_capacity(features_data.len());
        for (i, s) in features_data.iter().enumerate() {
            all.push(RegressionPredictionResult {
                stock_code: s.stock_code.clone(),
                value: values[i],
            });
        }
        all.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));

        let best = &all[0];
        info!("🏆 [ONNX-REG] 최고 종목: {} (value: {:.6})", best.stock_code, best.value);
        // 저장 시도 (설정에서 허용될 때만)
        if let Err(e) = self.save_model_record_regression(
            date,
            &final_stocks,
            &features_data,
            &all,
            &best.stock_code,
            best.value,
            "onnx_regression",
        ) { error!("[ONNX-REG] 모델 기록 저장 실패: {}", e); }

        Ok(Some((best.stock_code.clone(), best.value, all)))
    }

    /// 회귀 ONNX: 출력0 = best_index(i64, scalar), 출력1 = values(f32, [N] or [N,1] or [1,N])
    fn predict_with_onnx_regression(
        &self,
        features_data: &[StockFeatures],
    ) -> StockrsResult<(i64, Vec<f64>)> {
        let n = features_data.len();
        let f = self.features.len();

        // 1) 배치 입력 Array2<f32> (N, F)
        let mut mat = Array2::<f32>::zeros((n, f));
        for (row, sf) in features_data.iter().enumerate() {
            if sf.features.len() != f {
                return Err(StockrsError::prediction(format!(
                    "특징 수 불일치: 기대 {} vs 실제 {} (종목 {})",
                    f,
                    sf.features.len(),
                    sf.stock_code
                )));
            }
            for (col, &v) in sf.features.iter().enumerate() {
                let val = v as f32;
                mat[(row, col)] = if val.is_finite() { val } else { 0.0 };
            }
        }

        // 2) 텐서로 변환
        use ndarray::CowArray;
        let input_dyn = mat.into_dyn();
        let input_cow = CowArray::from(input_dyn);
        let input_tensor = Value::from_array(&self.session.allocator() as *const _ as *mut _, &input_cow)
            .map_err(|e| StockrsError::prediction(format!("입력 텐서 생성 실패: {}", e)))?;

        // 3) 실행 (출력 0: i64 scalar, 출력 1: f32 vector/2D)
        let outputs = self.session
            .run(vec![input_tensor])
            .map_err(|e| StockrsError::prediction(format!("ONNX 모델 실행 실패: {}", e)))?;

        if outputs.len() < 2 {
            return Err(StockrsError::prediction(format!(
                "ONNX 출력이 2개 미만입니다 (got: {})", outputs.len()
            )));
        }

        // 4) best_index 추출
        let best_idx: i64 = {
            let o0 = &outputs[0];
            let t = o0.try_extract::<i64>()
                .map_err(|_| StockrsError::prediction("best_index 텐서 추출 실패".to_string()))?;
            let view = t.view();
            let slice = view.as_slice().ok_or_else(|| {
                StockrsError::prediction("best_index 슬라이스 추출 실패".to_string())
            })?;
            if slice.is_empty() {
                return Err(StockrsError::prediction("best_index 비어있음".to_string()));
            }
            slice[0]
        };

        // 5) values 추출 (shape: [N], [N,1], [1,N] 모두 대응)
        let values_f64: Vec<f64> = {
            let o1 = &outputs[1];
            let t = o1.try_extract::<f32>()
                .map_err(|_| StockrsError::prediction("values 텐서 추출 실패".to_string()))?;
            let view = t.view();
            let shape: Vec<usize> = view.shape().to_vec();

            // 가능한 모양에 유연 대응
            let flatten: Vec<f32> = match shape.len() {
                1 => {
                    // [N]
                    view.as_slice()
                        .ok_or_else(|| StockrsError::prediction("values 슬라이스 실패([N])".to_string()))?
                        .to_vec()
                }
                2 => {
                    use ndarray::Axis;
                    let (d0, d1) = (shape[0], shape[1]);
                    if d0 == n && d1 == 1 {
                        // [N,1] → squeeze
                        view.index_axis(Axis(1), 0)
                            .to_owned()
                            .iter()
                            .cloned()
                            .collect()
                    } else if d0 == 1 && d1 == n {
                        // [1,N] → squeeze
                        view.index_axis(Axis(0), 0)
                            .to_owned()
                            .iter()
                            .cloned()
                            .collect()
                    } else if d0 == n && d1 == f {
                        // [N,F]가 나오는 경우 방어: 평균으로 스칼라화
                        view.outer_iter()
                            .map(|row| {
                                let mut s = 0.0f32;
                                let mut c = 0usize;
                                for v in row.iter() { s += *v; c += 1; }
                                if c > 0 { s / (c as f32) } else { 0.0 }
                            })
                            .collect()
                    } else {
                        return Err(StockrsError::prediction(format!(
                            "알 수 없는 values shape: {:?}, 기대 N={}, 또는 [N,1]/[1,N]",
                            shape, n
                        )));
                    }
                }
                _ => {
                    return Err(StockrsError::prediction(format!(
                        "values 차원 수 비정상: {:?}",
                        shape
                    )));
                }
            };

            if flatten.len() != n {
                return Err(StockrsError::prediction(format!(
                    "values 길이 불일치: 기대 {} vs 실제 {}",
                    n, flatten.len()
                )));
            }

            flatten
                .into_iter()
                .map(|v| {
                    let x = if v.is_finite() { v as f64 } else { 0.0 };
                    x.clamp(f64::NEG_INFINITY, f64::INFINITY)
                })
                .collect()
        };

        Ok((best_idx, values_f64))
    }

    fn trading_mode_str(&self) -> &'static str {
        match self.trading_mode {
            TradingMode::Real => "real",
            TradingMode::Paper => "paper",
            TradingMode::Backtest => "backtest",
        }
    }

    fn current_time_hhmm() -> StockrsResult<String> {
        let ymdhm = TimeService::global_format_ymdhm()?; // YYYYMMDDHHMM
        Ok(ymdhm[8..12].to_string())
    }

    fn normalize_f64(x: f64) -> f64 {
        if x.is_finite() { x } else { 0.0 }
    }

    fn save_model_record_classifier(
        &self,
        date: &str,
        stocks: &Vec<String>,
        features_data: &Vec<StockFeatures>,
        predictions: &Vec<PredictionResult>,
        best_stock: &str,
        best_score: f64,
        model_name: &str,
    ) -> StockrsResult<()> {
        let cfg = get_config()?;
        if !cfg.logging.store_model_records { return Ok(()); }

        // 시간/모드
        let time_hhmm = Self::current_time_hhmm()?;
        let mode_str = self.trading_mode_str();

        // features/stocks
        let features_json = serde_json::to_string(&self.features)
            .map_err(|e| StockrsError::parsing("features_json", format!("{}", e)))?;
        let stocks_json = serde_json::to_string(stocks)
            .map_err(|e| StockrsError::parsing("stocks_json", format!("{}", e)))?;

        // feature matrix
        if features_data.len() != stocks.len() {
            return Err(StockrsError::prediction("features_data와 stocks 길이 불일치".to_string()));
        }
        let mut feature_matrix: Vec<Vec<f64>> = Vec::with_capacity(features_data.len());
        for sf in features_data.iter() {
            let mut row = Vec::with_capacity(sf.features.len());
            for &v in sf.features.iter() { row.push(Self::normalize_f64(v)); }
            feature_matrix.push(row);
        }
        let feature_matrix_json = serde_json::to_string(&feature_matrix)
            .map_err(|e| StockrsError::parsing("feature_matrix_json", format!("{}", e)))?;

        // class probs aligned with stocks
        use std::collections::HashMap;
        let mut prob_map: HashMap<&str, f64> = HashMap::new();
        for p in predictions.iter() { prob_map.insert(p.stock_code.as_str(), Self::normalize_f64(p.probability)); }
        let class_probs: Vec<f64> = stocks.iter().map(|s| *prob_map.get(s.as_str()).unwrap_or(&0.0)).collect();
        let class_probs_json = serde_json::to_string(&class_probs)
            .map_err(|e| StockrsError::parsing("class_probs_json", format!("{}", e)))?;

        // write to trading DB
        let trading_db_path = &cfg.database.trading_db_path;
        let conn = rusqlite::Connection::open(trading_db_path)
            .map_err(|e| StockrsError::database("trading DB 열기", e.to_string()))?;
        conn.execute(
            "INSERT INTO model (
                date, time, mode, model_name, features_json, stocks_json,
                feature_matrix_json, class_probs_json, reg_values_json,
                best_stock, best_score, version, notes
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                date,
                time_hhmm.as_str(),
                mode_str,
                model_name,
                features_json.as_str(),
                stocks_json.as_str(),
                feature_matrix_json.as_str(),
                class_probs_json.as_str(),
                Option::<&str>::None,
                best_stock,
                Self::normalize_f64(best_score),
                Some(1i64),
                Option::<&str>::None,
            ),
        ).map_err(|e| StockrsError::database("model 레코드 저장", e.to_string()))?;

        Ok(())
    }

    fn save_model_record_regression(
        &self,
        date: &str,
        stocks: &Vec<String>,
        features_data: &Vec<StockFeatures>,
        all: &Vec<RegressionPredictionResult>,
        best_stock: &str,
        best_score: f64,
        model_name: &str,
    ) -> StockrsResult<()> {
        let cfg = get_config()?;
        if !cfg.logging.store_model_records { return Ok(()); }

        // 시간/모드
        let time_hhmm = Self::current_time_hhmm()?;
        let mode_str = self.trading_mode_str();

        // features/stocks/matrix
        let features_json = serde_json::to_string(&self.features)
            .map_err(|e| StockrsError::parsing("features_json", format!("{}", e)))?;
        let stocks_json = serde_json::to_string(stocks)
            .map_err(|e| StockrsError::parsing("stocks_json", format!("{}", e)))?;
        if features_data.len() != stocks.len() {
            return Err(StockrsError::prediction("features_data와 stocks 길이 불일치".to_string()));
        }
        let mut feature_matrix: Vec<Vec<f64>> = Vec::with_capacity(features_data.len());
        for sf in features_data.iter() {
            let mut row = Vec::with_capacity(sf.features.len());
            for &v in sf.features.iter() { row.push(Self::normalize_f64(v)); }
            feature_matrix.push(row);
        }
        let feature_matrix_json = serde_json::to_string(&feature_matrix)
            .map_err(|e| StockrsError::parsing("feature_matrix_json", format!("{}", e)))?;

        // regression values aligned with stocks
        use std::collections::HashMap;
        let mut val_map: HashMap<&str, f64> = HashMap::new();
        for r in all.iter() { val_map.insert(r.stock_code.as_str(), Self::normalize_f64(r.value)); }
        let values: Vec<f64> = stocks.iter().map(|s| *val_map.get(s.as_str()).unwrap_or(&0.0)).collect();
        let reg_values_json = serde_json::to_string(&values)
            .map_err(|e| StockrsError::parsing("reg_values_json", format!("{}", e)))?;

        // write
        let trading_db_path = &cfg.database.trading_db_path;
        let conn = rusqlite::Connection::open(trading_db_path)
            .map_err(|e| StockrsError::database("trading DB 열기", e.to_string()))?;
        conn.execute(
            "INSERT INTO model (
                date, time, mode, model_name, features_json, stocks_json,
                feature_matrix_json, class_probs_json, reg_values_json,
                best_stock, best_score, version, notes
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                date,
                time_hhmm.as_str(),
                mode_str,
                model_name,
                features_json.as_str(),
                stocks_json.as_str(),
                feature_matrix_json.as_str(),
                Option::<&str>::None,
                reg_values_json.as_str(),
                best_stock,
                Self::normalize_f64(best_score),
                Some(1i64),
                Option::<&str>::None,
            ),
        ).map_err(|e| StockrsError::database("model 레코드 저장", e.to_string()))?;

        Ok(())
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
