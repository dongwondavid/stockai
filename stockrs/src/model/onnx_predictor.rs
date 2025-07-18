use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use log::{debug, warn, info};
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use ort::{Environment, SessionBuilder, Value};
use std::sync::Arc;
use ndarray::Array2;
use std::collections::HashSet;

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
    input_name: String,
    output_name: String,
    extra_stocks_set: HashSet<String>,
}

impl ONNXPredictor {
    /// ONNX 모델을 로드하고 Predictor를 생성합니다
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // 기본 경로들 (config에서 읽어오도록 나중에 수정)
        let model_info_path = "data/rust_model_info.json";
        let extra_stocks_path = "data/extra_stocks.txt";
        let features_path = "data/features.txt";
        
        // 모델 정보 로드
        let model_info = Self::load_model_info(model_info_path)?;
        
        // ONNX Runtime 환경 초기화
        let environment = Arc::new(
            Environment::builder()
                .with_name("joonwoo_predictor")
                .build()?
        );
        
        // 세션 생성
        let session = SessionBuilder::new(&environment)?
            .with_model_from_file(&model_info.onnx_model_path)?;
        
        // extra_stocks.txt 로드
        let extra_stocks_set = Self::load_extra_stocks(extra_stocks_path)?;
        
        // features.txt 로드
        let features = Self::load_features(features_path)?;
        
        info!("ONNX 모델 로드 완료: {}", model_info.onnx_model_path);
        info!("특징 수: {}, 제외 종목 수: {}", features.len(), extra_stocks_set.len());
        
        Ok(ONNXPredictor {
            session,
            features,
            input_name: model_info.input_name,
            output_name: model_info.output_name,
            extra_stocks_set,
        })
    }
    
    /// 최고 확률 종목을 예측합니다
    pub fn predict_top_stock(&self, date: &str) -> Result<String, Box<dyn Error>> {
        // 일단 예시 구현 - 실제로는 DB 연결 후 분석
        info!("🔮 [ONNX] {}일 최고 확률 종목 예측 중...", date);
        
        // TODO: 실제 DB 연결 및 분석 로직
        // 현재는 더미 데이터로 대체
        let dummy_predictions = vec![
            PredictionResult {
                stock_code: "A005930".to_string(), // 삼성전자
                probability: 0.8,
            },
            PredictionResult {
                stock_code: "A000660".to_string(), // SK하이닉스
                probability: 0.7,
            },
        ];
        
        // 최고 확률 종목 반환
        let best_stock = dummy_predictions
            .iter()
            .max_by(|a, b| a.probability.partial_cmp(&b.probability).unwrap())
            .ok_or("예측 결과가 없습니다")?;
        
        info!("🎯 [ONNX] 선택된 종목: {} (확률: {:.2}%)", 
              best_stock.stock_code, best_stock.probability * 100.0);
        
        Ok(best_stock.stock_code.clone())
    }
    
    fn load_model_info(path: &str) -> Result<ONNXModelInfo, Box<dyn Error>> {
        if !Path::new(path).exists() {
            return Err(format!("ONNX 모델 정보 파일을 찾을 수 없습니다: {}", path).into());
        }
        
        let file = File::open(path)?;
        let model_info: ONNXModelInfo = serde_json::from_reader(file)?;
        Ok(model_info)
    }
    
    fn load_extra_stocks(path: &str) -> Result<HashSet<String>, Box<dyn Error>> {
        if !Path::new(path).exists() {
            warn!("extra_stocks.txt 파일이 없습니다: {}", path);
            return Ok(HashSet::new());
        }
        
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut extra_stocks = HashSet::new();
        
        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            
            // 헤더나 빈 줄 건너뛰기
            if line.is_empty() || line.contains("=") || line.contains("총") {
                continue;
            }
            
            extra_stocks.insert(line.to_string());
        }
        
        debug!("extra_stocks.txt에서 {}개 종목 로드됨", extra_stocks.len());
        Ok(extra_stocks)
    }
    
    fn load_features(path: &str) -> Result<Vec<String>, Box<dyn Error>> {
        if !Path::new(path).exists() {
            warn!("features.txt 파일이 없습니다: {}", path);
            return Ok(vec!["dummy_feature".to_string()]);
        }
        
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut features = Vec::new();
        
        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            
            if !line.is_empty() {
                features.push(line.to_string());
            }
        }
        
        debug!("features.txt에서 {}개 특징 로드됨", features.len());
        Ok(features)
    }
    
    // TODO: solomon에서 나머지 핵심 기능들 포팅 예정
    // - get_top_volume_stocks: 거래대금 상위 종목 조회
    // - calculate_features_for_stocks: 종목별 특징 계산
    // - predict_with_onnx_model: 실제 ONNX 모델 예측
    // 현재는 더미 구현으로 대체하여 기본 구조 테스트
} 