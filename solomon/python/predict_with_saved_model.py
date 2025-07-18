#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
저장된 모델을 사용하여 예측을 수행하는 스크립트
analyze_best_models.py에서 저장한 모델을 로드하여 새로운 데이터에 대해 예측합니다.
"""

import pandas as pd
import numpy as np
import sqlite3
import polars as pl
from joblib import load
from pathlib import Path
import json
from datetime import datetime
import warnings
import logging
from typing import Dict, List, Any, Tuple

# 경고 무시
warnings.filterwarnings('ignore')

# 로깅 설정
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

class ModelPredictor:
    """저장된 모델을 사용하여 예측을 수행하는 클래스"""
    
    def __init__(self, db_path: str = "D:/db/solomon.db"):
        self.db_path = db_path
        self.model = None
        self.metadata = None
        self.features = None
        
    def load_model_and_metadata(self, model_path: str, metadata_path: str = None):
        """저장된 모델과 메타데이터를 로드합니다."""
        try:
            # 모델 로드
            self.model = load(model_path)
            logger.info(f"모델 로드 완료: {model_path}")
            
            # 메타데이터 로드
            if metadata_path is None:
                # 모델 경로에서 메타데이터 경로 추정
                metadata_path = model_path.replace('.joblib', '_metadata.json')
            
            if Path(metadata_path).exists():
                with open(metadata_path, 'r', encoding='utf-8') as f:
                    self.metadata = json.load(f)
                self.features = self.metadata['features']
                logger.info(f"메타데이터 로드 완료: {metadata_path}")
                logger.info(f"특성 수: {len(self.features)}")
            else:
                logger.warning(f"메타데이터 파일을 찾을 수 없습니다: {metadata_path}")
                
        except Exception as e:
            logger.error(f"모델/메타데이터 로드 실패: {e}")
            raise
    
    def load_prediction_data(self, start_date: str = None, end_date: str = None, 
                           limit: int = None) -> pd.DataFrame:
        """예측을 위한 데이터를 로드합니다."""
        logger.info("예측용 데이터 로딩 중...")
        
        try:
            conn = sqlite3.connect(self.db_path)
            
            if self.features is None:
                raise ValueError("특성 목록이 로드되지 않았습니다. 먼저 모델을 로드하세요.")
            
            # 특성별로 테이블 매핑
            feature_mapping = {}
            for feature in self.features:
                if feature.startswith('day1_'):
                    column_name = feature.replace('day1_', '')
                    feature_mapping[feature] = f"a.{column_name} AS {feature}"
                elif feature.startswith('day2_'):
                    column_name = feature.replace('day2_', '')
                    feature_mapping[feature] = f"b.{column_name} AS {feature}"
                elif feature.startswith('day3_'):
                    column_name = feature.replace('day3_', '')
                    feature_mapping[feature] = f"c.{column_name} AS {feature}"
                elif feature.startswith('day4_'):
                    column_name = feature.replace('day4_', '')
                    feature_mapping[feature] = f"d.{column_name} AS {feature}"
                else:
                    feature_mapping[feature] = f"a.{feature} AS {feature}"
            
            # 쿼리 생성
            features_str = ', '.join([feature_mapping[feature] for feature in self.features])
            
            # 날짜 조건 추가
            date_condition = ""
            if start_date and end_date:
                date_condition = f"AND a.date BETWEEN '{start_date}' AND '{end_date}'"
            elif start_date:
                date_condition = f"AND a.date >= '{start_date}'"
            elif end_date:
                date_condition = f"AND a.date <= '{end_date}'"
            
            # LIMIT 조건 추가
            limit_clause = ""
            if limit:
                limit_clause = f"LIMIT {limit}"
            
            query = f"""
            SELECT a.date, a.stock_code, {features_str}
            FROM day1 a
            INNER JOIN day2 b ON a.date = b.date AND a.stock_code = b.stock_code
            INNER JOIN day3 c ON a.date = c.date AND a.stock_code = c.stock_code
            INNER JOIN day4 d ON a.date = d.date AND a.stock_code = d.stock_code
            WHERE 1=1 {date_condition}
            ORDER BY a.date, a.stock_code
            {limit_clause}
            """
            
            # 데이터 로드
            df = pd.read_sql_query(query, conn)
            conn.close()
            
            logger.info(f"예측용 데이터 로드 완료: {len(df)} 행")
            return df
            
        except Exception as e:
            logger.error(f"예측용 데이터 로드 실패: {e}")
            raise
    
    def preprocess_data(self, X: pd.DataFrame) -> pd.DataFrame:
        """데이터 전처리"""
        if self.features is None:
            raise ValueError("특성 목록이 로드되지 않았습니다.")
        
        # 특성 순서 재정렬
        X = X[self.features]
        
        # 수치형 변환
        for col in X.columns:
            X[col] = pd.to_numeric(X[col], errors='coerce')
        
        # 결측값 처리 (평균값으로 대체)
        X = X.fillna(X.mean())
        
        # float64 타입으로 변환
        X = X.astype('float64')
        
        return X
    
    def predict(self, X: pd.DataFrame) -> Tuple[np.ndarray, np.ndarray]:
        """예측을 수행합니다."""
        if self.model is None:
            raise ValueError("모델이 로드되지 않았습니다.")
        
        # 데이터 전처리
        X_processed = self.preprocess_data(X)
        
        # 예측 수행
        predictions = self.model.predict(X_processed)
        prediction_probas = self.model.predict_proba(X_processed)
        
        return predictions, prediction_probas
    
    def predict_top_stocks_per_day(self, df: pd.DataFrame, top_k: int = 1) -> pd.DataFrame:
        """하루 기준으로 상위 종목을 선택합니다."""
        # 예측 수행
        predictions, prediction_probas = self.predict(df)
        
        # 결과 데이터프레임 생성
        result_df = df[['date', 'stock_code']].copy()
        result_df['predicted_class'] = predictions
        result_df['predicted_proba_0'] = prediction_probas[:, 0]  # 급등 아님 확률
        result_df['predicted_proba_1'] = prediction_probas[:, 1]  # 급등 확률
        
        # 하루 기준으로 급등 확률이 높은 순으로 정렬하고 상위 k개 선택
        result_df = result_df.sort_values(['date', 'predicted_proba_1'], ascending=[True, False])
        
        # 각 날짜별로 상위 k개 선택
        top_stocks = []
        for date in result_df['date'].unique():
            day_data = result_df[result_df['date'] == date]
            top_day_stocks = day_data.head(top_k)
            top_stocks.append(top_day_stocks)
        
        top_result_df = pd.concat(top_stocks, ignore_index=True)
        
        return top_result_df
    
    def save_predictions(self, result_df: pd.DataFrame, output_path: str):
        """예측 결과를 저장합니다."""
        Path(output_path).parent.mkdir(parents=True, exist_ok=True)
        
        result_df.to_csv(output_path, index=False, encoding='utf-8-sig')
        logger.info(f"예측 결과가 저장되었습니다: {output_path}")
    
    def print_prediction_summary(self, result_df: pd.DataFrame):
        """예측 결과 요약을 출력합니다."""
        print("\n" + "="*60)
        print("예측 결과 요약")
        print("="*60)
        
        # 기본 통계
        print(f"총 예측 종목 수: {len(result_df)}")
        print(f"고유 날짜 수: {result_df['date'].nunique()}")
        print(f"고유 종목 수: {result_df['stock_code'].nunique()}")
        
        # 급등 예측 통계
        positive_predictions = (result_df['predicted_class'] == 1).sum()
        print(f"급등 예측 종목 수: {positive_predictions}")
        print(f"급등 예측 비율: {positive_predictions/len(result_df)*100:.2f}%")
        
        # 확률 분포 통계
        print(f"\n급등 확률 통계:")
        print(f"  평균: {result_df['predicted_proba_1'].mean():.4f}")
        print(f"  중앙값: {result_df['predicted_proba_1'].median():.4f}")
        print(f"  최대값: {result_df['predicted_proba_1'].max():.4f}")
        print(f"  최소값: {result_df['predicted_proba_1'].min():.4f}")
        
        # 날짜 범위
        unique_dates = sorted(result_df['date'].unique())
        if len(unique_dates) > 0:
            print(f"\n예측 기간: {unique_dates[0]} ~ {unique_dates[-1]}")
        
        # 상위 확률 종목들
        print(f"\n급등 확률 상위 10개 종목:")
        top_10 = result_df.nlargest(10, 'predicted_proba_1')
        for i, (_, row) in enumerate(top_10.iterrows(), 1):
            print(f"  {i}. {row['date']} - {row['stock_code']} (확률: {row['predicted_proba_1']:.4f})")

def main():
    """메인 실행 함수"""
    # JSON 설정 파일 로드
    config_file = "config_prediction.json"
    
    try:
        with open(config_file, 'r', encoding='utf-8') as f:
            config = json.load(f)
    except FileNotFoundError:
        # 기본 설정으로 config 파일 생성
        config = {
            # 저장된 모델 파일 경로
            "model_path": "results/best_model_analysis/models/RandomForestClassifier_optimized.joblib",
            
            # 메타데이터 파일 경로 (선택사항, 자동으로 추정됨)
            "metadata_path": None,
            
            # 데이터베이스 경로
            "db_path": "D:/db/solomon.db",
            
            # 예측할 날짜 범위 (선택사항)
            "start_date": None,  # 예: "2023-01-01"
            "end_date": None,    # 예: "2023-12-31"
            
            # 데이터 제한 (선택사항, 테스트용)
            "limit": None,       # 예: 1000
            
            # 하루에 선택할 상위 종목 수
            "top_k": 1,
            
            # 결과 저장 경로
            "output_path": "results/predictions/prediction_results.csv"
        }
        with open(config_file, 'w', encoding='utf-8') as f:
            json.dump(config, f, ensure_ascii=False, indent=2)
        print(f"기본 설정 파일 '{config_file}'이 생성되었습니다.")
        print("\n=== 설정 옵션 설명 ===")
        print("model_path: 저장된 모델 파일 경로")
        print("start_date/end_date: 예측할 날짜 범위 (선택사항)")
        print("limit: 데이터 제한 (테스트용, 선택사항)")
        print("top_k: 하루에 선택할 종목 수")
        print("output_path: 결과 저장 경로")
        print("\n필요에 따라 config_prediction.json 파일을 수정하세요.")
        return
    
    # 예측기 인스턴스 생성
    predictor = ModelPredictor(config['db_path'])
    
    # 모델 및 메타데이터 로드
    predictor.load_model_and_metadata(config['model_path'], config['metadata_path'])
    
    # 예측용 데이터 로드
    df = predictor.load_prediction_data(
        config['start_date'], 
        config['end_date'], 
        config['limit']
    )
    
    # 하루 기준 상위 종목 예측
    result_df = predictor.predict_top_stocks_per_day(df, config['top_k'])
    
    # 예측 결과 요약 출력
    predictor.print_prediction_summary(result_df)
    
    # 결과 저장
    predictor.save_predictions(result_df, config['output_path'])
    
    print("\n" + "="*60)
    print("예측 완료!")
    print("="*60)

if __name__ == "__main__":
    main() 