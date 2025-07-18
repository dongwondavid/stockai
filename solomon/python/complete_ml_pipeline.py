#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
완전한 ML 파이프라인: Rust Smartcore → Python scikit-learn 이식
데이터 로딩 → 전처리 → 분할 → 모델 학습 → k-fold 평가 → GridSearch → 성능 측정
"""

import pandas as pd
import numpy as np
import sqlite3
from sklearn.model_selection import train_test_split
from sklearn.ensemble import RandomForestClassifier
from sklearn.metrics import accuracy_score, precision_score, recall_score, f1_score, roc_auc_score
from sklearn.preprocessing import StandardScaler
import time
import warnings
import logging
from typing import Tuple, Dict, List, Any
import os

# 경고 무시
warnings.filterwarnings('ignore')

# 로깅 설정 (한글 인코딩 문제 해결)
import sys
import locale

# 콘솔 인코딩 설정
if sys.platform.startswith('win'):
    # Windows에서 한글 출력을 위한 설정
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

class CompleteMLPipeline:
    """완전한 ML 파이프라인 클래스"""
    
    def __init__(self, db_path: str = "D:/db/solomon.db", features_file: str = "features.txt"):
        self.db_path = db_path
        self.features_file = features_file
        self.features = self._load_features()
        self.random_state = 42
        self.test_split_ratio = 0.2  # Test set 비율 (20%)
        np.random.seed(self.random_state)
        
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
    
    def load_data(self) -> Tuple[pd.DataFrame, pd.Series, pd.DataFrame, pd.Series]:
        """✅ [1] 데이터 로딩 및 Train/Val/Test 분할"""
        logger.info("데이터베이스에서 데이터 로딩 중...")
        
        try:
            # SQLite 연결
            conn = sqlite3.connect(self.db_path)
            
            # 특성별로 테이블 매핑 (접두사 제거)
            feature_mapping = {}
            for feature in self.features:
                if feature.startswith('day1_'):
                    # day1_ 접두사 제거
                    column_name = feature.replace('day1_', '')
                    feature_mapping[feature] = f"a.{column_name} AS {feature}"
                elif feature.startswith('day2_'):
                    # day2_ 접두사 제거
                    column_name = feature.replace('day2_', '')
                    feature_mapping[feature] = f"b.{column_name} AS {feature}"
                elif feature.startswith('day3_'):
                    # day3_ 접두사 제거
                    column_name = feature.replace('day3_', '')
                    feature_mapping[feature] = f"c.{column_name} AS {feature}"
                elif feature.startswith('day4_'):
                    # day4_ 접두사 제거
                    column_name = feature.replace('day4_', '')
                    feature_mapping[feature] = f"d.{column_name} AS {feature}"
                else:
                    # 기본적으로 day1에서 가져오기
                    feature_mapping[feature] = f"a.{feature} AS {feature}"
            
            # 동적 쿼리 생성 (run_complete_experiment.py 방식 적용)
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
            
            # 디버깅: 쿼리 출력
            logger.info(f"실행할 SQL 쿼리:")
            logger.info(query)
            
            # 데이터 로드
            df = pd.read_sql_query(query, conn)
            conn.close()
            
            # 디버깅: 결과 확인
            logger.info(f"쿼리 결과: {len(df)} 행")
            if len(df) == 0:
                logger.warning("데이터가 없습니다. 테이블별 데이터 수를 확인해보겠습니다.")
                
                # 각 테이블의 데이터 수 확인
                conn = sqlite3.connect(self.db_path)
                tables = ['day1', 'day2', 'day3', 'day4', 'answer_v3']
                for table in tables:
                    count_query = f"SELECT COUNT(*) as count FROM {table}"
                    count_df = pd.read_sql_query(count_query, conn)
                    logger.info(f"{table} 테이블: {count_df['count'].iloc[0]} 행")
                
                # 날짜 범위 확인
                date_query = """
                SELECT 
                    MIN(a.date) as min_date, 
                    MAX(a.date) as max_date,
                    COUNT(DISTINCT a.date) as unique_dates
                FROM day1 a
                """
                date_df = pd.read_sql_query(date_query, conn)
                logger.info(f"day1 테이블 날짜 범위: {date_df['min_date'].iloc[0]} ~ {date_df['max_date'].iloc[0]}")
                logger.info(f"고유 날짜 수: {date_df['unique_dates'].iloc[0]}")
                
                conn.close()
            
            # X, y 분리
            X = df[self.features]
            y = df['is_answer']
            
            logger.info(f"데이터 로딩 완료: X shape={X.shape}, y shape={y.shape}")
            logger.info(f"클래스 분포: {y.value_counts().to_dict()}")
            
            # Train/Val과 Test 분할
            X_train_val, X_test, y_train_val, y_test = self._split_train_test(X, y)
            
            return X_train_val, y_train_val, X_test, y_test
            
        except Exception as e:
            logger.error(f"데이터 로딩 실패: {e}")
            raise
    
    def _split_train_test(self, X: pd.DataFrame, y: pd.Series) -> Tuple[pd.DataFrame, pd.DataFrame, pd.Series, pd.Series]:
        """Train/Val과 Test 데이터 분할"""
        from sklearn.model_selection import train_test_split
        
        logger.info(f"Train/Val ({1-self.test_split_ratio:.1%})과 Test ({self.test_split_ratio:.1%}) 분할 중...")
        
        X_train_val, X_test, y_train_val, y_test = train_test_split(
            X, y, 
            test_size=self.test_split_ratio, 
            random_state=self.random_state,
            stratify=y  # 클래스 비율 유지
        )
        
        logger.info(f"Train/Val 데이터: {len(X_train_val)} 행")
        logger.info(f"Test 데이터: {len(X_test)} 행")
        logger.info(f"Train/Val 클래스 분포: {y_train_val.value_counts().to_dict()}")
        logger.info(f"Test 클래스 분포: {y_test.value_counts().to_dict()}")
        
        return X_train_val, X_test, y_train_val, y_test
    
    def preprocess_data(self, X: pd.DataFrame) -> pd.DataFrame:
        """✅ [2] 전처리"""
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
        
        logger.info(f"전처리 완료: X shape={X.shape}, dtype={X.dtypes.unique()}")
        return X
    
    def get_model_configs(self) -> Dict[str, Dict[str, Any]]:
        """✅ [4] 모델 정의"""
        return {
            'RandomForestClassifier': {
                'model': RandomForestClassifier(random_state=self.random_state),
                'params': {
                    'n_estimators': [50, 100, 200],
                    'max_depth': [5, 10, None],
                    'criterion': ['gini', 'entropy'],
                    'min_samples_split': [2, 5],
                    'min_samples_leaf': [1, 2]
                }
            }
        }
    

    
    def run_kfold_evaluation(self, X: pd.DataFrame, y: pd.Series, 
                           model_configs: Dict[str, Dict[str, Any]], k_folds: int = 3) -> List[Dict[str, Any]]:
        """✅ [4] 각 하이퍼파라미터 조합별 k-fold 교차검증 평가"""
        logger.info("하이퍼파라미터 조합별 k-fold 교차검증 시작...")
        
        from sklearn.model_selection import StratifiedKFold
        from itertools import product
        
        results = []
        
        for model_name, config in model_configs.items():
            logger.info(f"{model_name} k-fold 평가 중...")
            
            # 모든 하이퍼파라미터 조합에 대해 개별 평가
            param_names = list(config['params'].keys())
            param_values = list(config['params'].values())
            param_combinations = list(product(*param_values))
            
            logger.info(f"{len(param_combinations)}개 조합 평가 중...")
            
            for param_combo in param_combinations:
                # 하이퍼파라미터 딕셔너리 생성
                params_dict = dict(zip(param_names, param_combo))
                
                # 모델 이름 생성
                param_str = "_".join([f"{k}_{v}" for k, v in params_dict.items()])
                model_name_with_params = f"{model_name}_{param_str}"
                
                try:
                    # k-fold 교차검증 수행
                    skf = StratifiedKFold(n_splits=k_folds, shuffle=True, random_state=self.random_state)
                    
                    # 각 fold의 성능을 저장할 리스트
                    fold_metrics = {
                        'Accuracy': [],
                        'Precision': [],
                        'Recall': [],
                        'F1': [],
                        'ROC_AUC': []
                    }
                    training_times = []
                    
                    logger.info(f"  {model_name_with_params} - {k_folds}개 fold 평가 중...")
                    
                    for fold, (train_idx, val_idx) in enumerate(skf.split(X, y), 1):
                        # 데이터 분할
                        X_train, X_val = X.iloc[train_idx], X.iloc[val_idx]
                        y_train, y_val = y.iloc[train_idx], y.iloc[val_idx]
                        
                        # 모델 생성 및 학습
                        model = config['model'].__class__(**params_dict, random_state=self.random_state)
                        
                        start_time = time.time()
                        model.fit(X_train, y_train)
                        training_time = time.time() - start_time
                        training_times.append(training_time)
                        
                        # 검증 데이터로 예측
                        y_pred = model.predict(X_val)
                        y_pred_proba = model.predict_proba(X_val)[:, 1] if hasattr(model, 'predict_proba') else None
                        
                        # 성능 지표 계산
                        fold_metrics['Accuracy'].append(accuracy_score(y_val, y_pred))
                        fold_metrics['Precision'].append(precision_score(y_val, y_pred, zero_division=0))
                        fold_metrics['Recall'].append(recall_score(y_val, y_pred, zero_division=0))
                        fold_metrics['F1'].append(f1_score(y_val, y_pred, zero_division=0))
                        
                        # ROC-AUC 계산
                        if y_pred_proba is not None:
                            try:
                                fold_metrics['ROC_AUC'].append(roc_auc_score(y_val, y_pred_proba))
                            except:
                                fold_metrics['ROC_AUC'].append(0.0)
                        else:
                            fold_metrics['ROC_AUC'].append(0.0)
                    
                    # 평균 성능 계산
                    avg_metrics = {}
                    for metric_name, values in fold_metrics.items():
                        avg_metrics[metric_name] = np.mean(values)
                    
                    avg_training_time = np.mean(training_times)
                    
                    # 결과 저장
                    result = {
                        'ModelName': model_name_with_params,
                        'BaseModel': model_name,
                        'BestParams': str(params_dict),
                        'Accuracy': avg_metrics['Accuracy'],
                        'Precision': avg_metrics['Precision'],
                        'Recall': avg_metrics['Recall'],
                        'F1': avg_metrics['F1'],
                        'ROC_AUC': avg_metrics['ROC_AUC'],
                        'TrainingTime': avg_training_time
                    }
                    results.append(result)
                    
                    logger.info(f"  {model_name_with_params} - 평균 F1: {avg_metrics['F1']:.4f}")
                    
                except Exception as e:
                    logger.error(f"  {model_name_with_params} 실패: {e}")
        
        return results
    
    def save_results(self, results: List[Dict[str, Any]], filename: str = "result.csv"):
        """✅ [5] 결과 저장"""
        logger.info(f"결과를 {filename}에 저장 중...")
        
        # DataFrame 생성
        df_results = pd.DataFrame(results)
        
        # F1 스코어 기준으로 정렬 (내림차순)
        if 'F1' in df_results.columns:
            df_results = df_results.sort_values('F1', ascending=False)
        
        # 컬럼 순서 조정
        columns = ['ModelName', 'BaseModel', 'Accuracy', 'Precision', 'Recall', 'F1', 'ROC_AUC', 'TrainingTime', 'BestParams']
        
        # 존재하는 컬럼만 선택
        available_columns = []
        for col in columns:
            if col in df_results.columns:
                available_columns.append(col)
        
        df_results = df_results[available_columns]
        
        # CSV 저장
        df_results.to_csv(filename, index=False, encoding='utf-8-sig')
        
        # 간단한 요약 출력
        print(f"\n결과가 {filename}에 저장되었습니다.")
        print(f"총 {len(df_results)}개 모델 평가 완료")
        
        if 'F1' in df_results.columns:
            best_model = df_results.iloc[0]
            print(f"최고 F1: {best_model['F1']:.4f} ({best_model['ModelName']})")
        
        return df_results
    
    def run_complete_pipeline(self, k_folds: int = 3):
        """완전한 파이프라인 실행"""
        print("="*60)
        print("완전한 ML 파이프라인 시작")
        print("="*60)
        
        try:
            # 1. 데이터 로딩 (이미 분할된 학습용 데이터)
            X_train_val, y_train_val, X_test, y_test = self.load_data()
            
            # 2. 전처리
            X_train_val = self.preprocess_data(X_train_val)
            
            # 3. 모델 정의
            model_configs = self.get_model_configs()
            
            # 4. k-fold 평가 (내부에서 train/test 분할 포함)
            results = self.run_kfold_evaluation(X_train_val, y_train_val, model_configs, k_folds)
            
            # 5. 결과 저장
            df_results = self.save_results(results)
            
            # 6. 최고 모델 테스트 세트 평가
            if 'F1' in df_results.columns:
                best_model_config = df_results.iloc[0]
                test_metrics = self.evaluate_on_test_set(X_train_val, y_train_val, X_test, y_test, best_model_config)
                print("="*60)
                print("테스트 세트 평가 결과:")
                print("="*60)
                for metric, value in test_metrics.items():
                    print(f"{metric}: {value:.4f}")
            else:
                print("최고 F1 모델을 찾을 수 없어 테스트 세트 평가를 건너뛰었습니다.")
            
            print("="*60)
            print("파이프라인 완료!")
            print("="*60)
            
            return df_results
            
        except Exception as e:
            logger.error(f"파이프라인 실행 실패: {e}")
            raise

    def evaluate_on_test_set(self, X_train_val: pd.DataFrame, y_train_val: pd.Series, 
                           X_test: pd.DataFrame, y_test: pd.Series,
                           best_model_config: Dict[str, Any]) -> Dict[str, float]:
        """Test Set에서 최종 평가"""
        logger.info("Test Set에서 최종 평가 중...")
        
        # 최고 성능 모델 생성
        model_name = best_model_config['BaseModel']
        best_params_str = best_model_config['BestParams']
        
        # 파라미터 문자열을 딕셔너리로 변환
        import ast
        try:
            best_params = ast.literal_eval(best_params_str)
        except:
            logger.error(f"파라미터 파싱 실패: {best_params_str}")
            return {}
        
        # 모델 생성
        model_configs = self.get_model_configs()
        model = model_configs[model_name]['model'].__class__(**best_params, random_state=self.random_state)
        
        # 모델 학습 (전체 Train/Val 데이터 사용)
        start_time = time.time()
        model.fit(X_train_val, y_train_val)  # Train/Val 데이터로 학습
        training_time = time.time() - start_time
        
        # Test Set 예측
        y_pred = model.predict(X_test)
        y_pred_proba = model.predict_proba(X_test)[:, 1] if hasattr(model, 'predict_proba') else None
        
        # 성능 지표 계산
        test_metrics = {
            'Test_Accuracy': accuracy_score(y_test, y_pred),
            'Test_Precision': precision_score(y_test, y_pred, zero_division=0),
            'Test_Recall': recall_score(y_test, y_pred, zero_division=0),
            'Test_F1': f1_score(y_test, y_pred, zero_division=0),
            'Test_TrainingTime': training_time
        }
        
        # ROC-AUC 계산
        if y_pred_proba is not None:
            try:
                test_metrics['Test_ROC_AUC'] = roc_auc_score(y_test, y_pred_proba)
            except:
                test_metrics['Test_ROC_AUC'] = 0.0
        else:
            test_metrics['Test_ROC_AUC'] = 0.0
        
        logger.info(f"Test Set 성능:")
        for metric, value in test_metrics.items():
            if 'Time' in metric:
                logger.info(f"  {metric}: {value:.4f}초")
            else:
                logger.info(f"  {metric}: {value:.4f}")
        
        return test_metrics

def main():
    """메인 실행 함수"""
    # 한글 인코딩 문제 해결을 위한 추가 설정
    import os
    os.environ['PYTHONIOENCODING'] = 'utf-8'
    
    # 파이프라인 인스턴스 생성
    pipeline = CompleteMLPipeline()
    
    # 완전한 파이프라인 실행
    results = pipeline.run_complete_pipeline(k_folds=3)
    
    return results

if __name__ == "__main__":
    main() 