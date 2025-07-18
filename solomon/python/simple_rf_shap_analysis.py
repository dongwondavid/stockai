#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
단순 랜덤포레스트 모델 + SHAP 분석
상위 20개 특징 출력
"""

import pandas as pd
import numpy as np
import sqlite3
from sklearn.model_selection import train_test_split
from sklearn.ensemble import RandomForestClassifier
from sklearn.metrics import accuracy_score, precision_score, recall_score, f1_score
from sklearn.preprocessing import StandardScaler
import warnings
import logging
import sys
import os
from typing import List, Tuple
import matplotlib.pyplot as plt
import shap

# 경고 무시
warnings.filterwarnings('ignore')

# 한글 폰트 설정
plt.rcParams['font.family'] = 'Malgun Gothic'  # Windows 기본 한글 폰트
plt.rcParams['axes.unicode_minus'] = False     # 마이너스 기호 깨짐 방지

# 콘솔 인코딩 설정
if sys.platform.startswith('win'):
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')

logging.basicConfig(
    level=logging.INFO, 
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler(sys.stdout)
    ]
)
logger = logging.getLogger(__name__)

class SimpleRFShapAnalysis:
    """단순 랜덤포레스트 + SHAP 분석 클래스"""
    
    def __init__(self, db_path: str = "D:/db/solomon.db", features_file: str = "features.txt"):
        self.db_path = db_path
        self.features_file = features_file
        self.features = self._load_features()
        self.random_state = 42
        
    def _load_features(self) -> List[str]:
        """features.txt에서 특성 목록 로드"""
        try:
            with open(self.features_file, 'r', encoding='utf-8') as f:
                features = [line.strip() for line in f if line.strip()]
            logger.info(f"로드된 특성 수: {len(features)}")
            return features
        except FileNotFoundError:
            logger.error(f"특성 파일을 찾을 수 없습니다: {self.features_file}")
            raise
    
    def load_data(self) -> Tuple[pd.DataFrame, pd.Series]:
        """데이터 로딩"""
        logger.info("데이터베이스에서 데이터 로딩 중...")
        
        try:
            conn = sqlite3.connect(self.db_path)
            
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
            
            features_str = ', '.join([feature_mapping[feature] for feature in self.features])
            query = f"""
            SELECT {features_str}, e.is_answer
            FROM day1 a
            INNER JOIN day2 b ON a.date = b.date AND a.stock_code = b.stock_code
            INNER JOIN day3 c ON a.date = c.date AND a.stock_code = c.stock_code
            INNER JOIN day4 d ON a.date = d.date AND a.stock_code = d.stock_code
            INNER JOIN answer_v3 e ON CAST(REPLACE(a.date, '-', '') AS INTEGER) = e.date 
                               AND a.stock_code = e.stock_code
            WHERE e.date < 20230601
            ORDER BY a.date, a.stock_code
            """
            
            df = pd.read_sql_query(query, conn)
            conn.close()
            
            # X, y 분리
            X = df[self.features]
            y = df['is_answer']
            
            logger.info(f"데이터 로딩 완료: X shape={X.shape}, y shape={y.shape}")
            logger.info(f"클래스 분포: {y.value_counts().to_dict()}")
            
            return X, y
            
        except Exception as e:
            logger.error(f"데이터 로딩 실패: {e}")
            raise
    
    def preprocess_data(self, X: pd.DataFrame) -> pd.DataFrame:
        """데이터 전처리"""
        logger.info("데이터 전처리 중...")
        
        # 특성 순서 재정렬
        X = X[self.features]
        
        # 수치형 변환
        for col in X.columns:
            X[col] = pd.to_numeric(X[col], errors='coerce')
        
        # 결측값 처리 (평균값으로 대체)
        X = X.fillna(X.mean())
        
        # float64 타입으로 변환
        X = X.astype('float64')
        
        logger.info(f"전처리 완료: X shape={X.shape}")
        return X
    
    def train_random_forest(self, X: pd.DataFrame, y: pd.Series) -> RandomForestClassifier:
        """랜덤포레스트 모델 학습"""
        logger.info("랜덤포레스트 모델 학습 중...")
        
        # Train/Test 분할
        X_train, X_test, y_train, y_test = train_test_split(
            X, y, 
            test_size=0.2, 
            random_state=self.random_state,
            stratify=y
        )
        
        # 랜덤포레스트 모델 생성 및 학습
        rf_model = RandomForestClassifier(
            n_estimators=100,
            max_depth=10,
            random_state=self.random_state,
            n_jobs=-1
        )
        
        rf_model.fit(X_train, y_train)
        
        # 성능 평가
        y_pred = rf_model.predict(X_test)
        accuracy = accuracy_score(y_test, y_pred)
        precision = precision_score(y_test, y_pred, zero_division=0)
        recall = recall_score(y_test, y_pred, zero_division=0)
        f1 = f1_score(y_test, y_pred, zero_division=0)
        
        logger.info(f"모델 성능:")
        logger.info(f"  Accuracy: {accuracy:.4f}")
        logger.info(f"  Precision: {precision:.4f}")
        logger.info(f"  Recall: {recall:.4f}")
        logger.info(f"  F1-Score: {f1:.4f}")
        
        return rf_model, X_test
    
    def perform_shap_analysis(self, model: RandomForestClassifier, X_test: pd.DataFrame) -> pd.DataFrame:
        """SHAP 분석 수행"""
        logger.info("SHAP 분석 수행 중...")
        
        # SHAP Tree Explainer 생성
        explainer = shap.TreeExplainer(model)
        
        # SHAP 값 계산 (샘플링하여 계산 속도 향상)
        sample_size = min(1000, len(X_test))  # 최대 1000개 샘플 사용
        X_sample = X_test.sample(n=sample_size, random_state=self.random_state)
        
        logger.info(f"SHAP 값 계산 중... (샘플 크기: {sample_size})")
        shap_values = explainer.shap_values(X_sample)
        
        # SHAP 값이 리스트인 경우 (분류 문제)
        if isinstance(shap_values, list):
            shap_values = shap_values[1]  # 양성 클래스에 대한 SHAP 값
        
        # 특성 중요도 계산 (절댓값 평균)
        feature_importance = np.abs(shap_values).mean(axis=0)
        
        # 결과를 DataFrame으로 변환
        shap_df = pd.DataFrame({
            'Feature': self.features,
            'SHAP_Importance': feature_importance
        })
        
        # 중요도 기준으로 정렬
        shap_df = shap_df.sort_values('SHAP_Importance', ascending=False)
        
        return shap_df
    
    def print_top_features(self, shap_df: pd.DataFrame, top_n: int = 20):
        """상위 특징 출력"""
        print(f"\n{'='*60}")
        print(f"SHAP 분석 결과 - 상위 {top_n}개 특징")
        print(f"{'='*60}")
        
        top_features = shap_df.head(top_n)
        
        for i, (_, row) in enumerate(top_features.iterrows(), 1):
            feature = row['Feature']
            importance = row['SHAP_Importance']
            print(f"{i:2d}. {feature:<30} {importance:.6f}")
        
        print(f"{'='*60}")
        
        return top_features
    
    def save_results(self, shap_df: pd.DataFrame, filename: str = "shap_importance.csv"):
        """결과 저장"""
        logger.info(f"SHAP 중요도를 {filename}에 저장 중...")
        shap_df.to_csv(filename, index=False, encoding='utf-8-sig')
        logger.info(f"결과가 {filename}에 저장되었습니다.")
    
    def run_analysis(self):
        """전체 분석 실행"""
        print("="*60)
        print("단순 랜덤포레스트 + SHAP 분석 시작")
        print("="*60)
        
        try:
            # 1. 데이터 로딩
            X, y = self.load_data()
            
            # 2. 전처리
            X = self.preprocess_data(X)
            
            # 3. 랜덤포레스트 모델 학습
            model, X_test = self.train_random_forest(X, y)
            
            # 4. SHAP 분석
            shap_df = self.perform_shap_analysis(model, X_test)
            
            # 5. 상위 20개 특징 출력
            top_features = self.print_top_features(shap_df, top_n=20)
            
            # 6. 결과 저장
            self.save_results(shap_df)
            
            print("="*60)
            print("분석 완료!")
            print("="*60)
            
            return shap_df, top_features
            
        except Exception as e:
            logger.error(f"분석 실행 실패: {e}")
            raise

def main():
    """메인 실행 함수"""
    # 한글 인코딩 문제 해결을 위한 추가 설정
    os.environ['PYTHONIOENCODING'] = 'utf-8'
    
    # 분석 인스턴스 생성
    analyzer = SimpleRFShapAnalysis()
    
    # 분석 실행
    shap_df, top_features = analyzer.run_analysis()
    
    return shap_df, top_features

if __name__ == "__main__":
    main() 