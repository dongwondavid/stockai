#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
최고 모델 분석 및 평가 파이프라인
complete_ml_pipeline.py의 결과를 분석하여 상위 모델을 선택하고
테스트 세트에서 평가한 후 하루 기준 분석을 수행합니다.
"""

import pandas as pd
import numpy as np
import sqlite3
import polars as pl
from joblib import load, dump
from pathlib import Path
from sklearn.metrics import accuracy_score, precision_score, recall_score, f1_score, confusion_matrix, classification_report, roc_curve, auc, precision_recall_curve, roc_auc_score
import matplotlib.pyplot as plt
import seaborn as sns
import json
from datetime import datetime

import warnings
import logging
from typing import Dict, List, Any, Tuple

# 경고 무시
warnings.filterwarnings('ignore')

# 한글 폰트 설정
plt.rcParams['font.family'] = 'Malgun Gothic'  # Windows 기본 한글 폰트
plt.rcParams['axes.unicode_minus'] = False     # 마이너스 기호 깨짐 방지

# 로깅 설정
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

class BestModelAnalyzer:
    """최고 모델 분석 및 평가 클래스"""
    
    def __init__(self, db_path: str = "D:/db/solomon.db", features_file: str = "features.txt"):
        self.db_path = db_path
        self.features_file = features_file
        self.features = self._load_features()
        self.random_state = 42
        self.test_split_ratio = 0.2
        
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
    
    def load_and_sort_results(self, result_file: str = "result.csv", 
                            sort_by: str = "F1", ascending: bool = False) -> pd.DataFrame:
        """
        result.csv 파일을 로드하고 지정된 기준으로 정렬하여 상위 5개 모델을 반환합니다.
        
        Args:
            result_file: 결과 파일 경로
            sort_by: 정렬 기준 컬럼 (F1, Accuracy, Precision, Recall, ROC_AUC)
            ascending: 오름차순 여부 (False: 내림차순)
        """
        try:
            # 결과 파일 로드
            df_results = pd.read_csv(result_file, encoding='utf-8-sig')
            logger.info(f"결과 파일 로드 완료: {len(df_results)}개 모델")
            
            # 정렬 기준 컬럼이 존재하는지 확인
            if sort_by not in df_results.columns:
                available_columns = [col for col in df_results.columns if col not in ['ModelName', 'BaseModel', 'BestParams']]
                logger.warning(f"'{sort_by}' 컬럼이 없습니다. 사용 가능한 컬럼: {available_columns}")
                sort_by = available_columns[0] if available_columns else 'F1'
            
            # 지정된 기준으로 정렬
            df_sorted = df_results.sort_values(by=sort_by, ascending=ascending)
            
            # 상위 5개 선택
            top_5_models = df_sorted.head(5).copy()
            
            logger.info(f"상위 5개 모델 (정렬 기준: {sort_by}):")
            for i, (_, row) in enumerate(top_5_models.iterrows(), 1):
                logger.info(f"  {i}. {row['ModelName']} - {sort_by}: {row[sort_by]:.4f}")
            
            return top_5_models
            
        except Exception as e:
            logger.error(f"결과 파일 로드 실패: {e}")
            raise
    
    def load_data_for_evaluation(self) -> Tuple[pd.DataFrame, pd.Series, pd.DataFrame, pd.Series]:
        """평가를 위한 데이터 로드 (Train/Val과 Test 분할)"""
        logger.info("평가용 데이터 로딩 중...")
        
        try:
            # SQLite 연결
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
            
            # 쿼리 생성
            features_str = ', '.join([feature_mapping[feature] for feature in self.features])
            query = f"""
            SELECT {features_str}, e.is_answer
            FROM day1 a
            INNER JOIN day2 b ON a.date = b.date AND a.stock_code = b.stock_code
            INNER JOIN day3 c ON a.date = c.date AND a.stock_code = c.stock_code
            INNER JOIN day4 d ON a.date = d.date AND a.stock_code = c.stock_code
            INNER JOIN answer_v3 e ON CAST(REPLACE(a.date, '-', '') AS INTEGER) = e.date 
                               AND a.stock_code = e.stock_code
            WHERE e.date < 20230601
            ORDER BY a.date, a.stock_code
            """
            
            # 데이터 로드
            df = pd.read_sql_query(query, conn)
            conn.close()
            
            # X, y 분리
            X = df[self.features]
            y = df['is_answer']
            
            # Train/Val과 Test 분할
            from sklearn.model_selection import train_test_split
            X_train_val, X_test, y_train_val, y_test = train_test_split(
                X, y, 
                test_size=self.test_split_ratio, 
                random_state=self.random_state,
                stratify=y
            )
            
            # 전처리
            X_train_val = self.preprocess_data(X_train_val)
            X_test = self.preprocess_data(X_test)
            
            logger.info(f"데이터 로딩 완료: Train/Val={len(X_train_val)}, Test={len(X_test)}")
            return X_train_val, y_train_val, X_test, y_test
            
        except Exception as e:
            logger.error(f"데이터 로딩 실패: {e}")
            raise
    
    def preprocess_data(self, X: pd.DataFrame) -> pd.DataFrame:
        """데이터 전처리"""
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
    
    def create_model_from_config(self, model_config: pd.Series) -> Any:
        """모델 설정으로부터 모델 객체 생성"""
        from sklearn.ensemble import RandomForestClassifier
        
        # 파라미터 문자열을 딕셔너리로 변환
        import ast
        try:
            params_str = model_config['BestParams']
            params = ast.literal_eval(params_str)
            
            # RandomForestClassifier 모델 생성
            model = RandomForestClassifier(**params, random_state=self.random_state)
            return model
            
        except Exception as e:
            logger.error(f"모델 생성 실패: {e}")
            return None
    
    def save_model(self, model: Any, model_name: str, output_dir: str = 'results/best_model_analysis/models') -> str:
        """학습된 모델을 저장합니다."""
        Path(output_dir).mkdir(parents=True, exist_ok=True)
        
        # 모델명에서 특수문자 제거
        safe_model_name = model_name.replace(' ', '_').replace('(', '').replace(')', '').replace('/', '_')
        model_path = f"{output_dir}/{safe_model_name}.joblib"
        
        try:
            dump(model, model_path)
            logger.info(f"모델이 저장되었습니다: {model_path}")
            return model_path
        except Exception as e:
            logger.error(f"모델 저장 실패: {e}")
            raise
    
    def load_saved_model(self, model_path: str) -> Any:
        """저장된 모델을 로드합니다."""
        try:
            model = load(model_path)
            logger.info(f"모델이 로드되었습니다: {model_path}")
            return model
        except Exception as e:
            logger.error(f"모델 로드 실패: {e}")
            raise
    
    def save_model_metadata(self, model_config: pd.Series, test_metrics: Dict[str, float], 
                          model_path: str, output_dir: str = 'results/best_model_analysis/models') -> str:
        """모델 메타데이터를 저장합니다."""
        Path(output_dir).mkdir(parents=True, exist_ok=True)
        
        # 모델명에서 특수문자 제거
        safe_model_name = model_config['ModelName'].replace(' ', '_').replace('(', '').replace(')', '').replace('/', '_')
        metadata_path = f"{output_dir}/{safe_model_name}_metadata.json"
        
        metadata = {
            'model_name': model_config['ModelName'],
            'base_model': model_config['BaseModel'],
            'best_params': model_config['BestParams'],
            'test_metrics': test_metrics,
            'model_path': model_path,
            'features': self.features,
            'saved_at': datetime.now().isoformat(),
            'random_state': self.random_state,
            'test_split_ratio': self.test_split_ratio
        }
        
        try:
            with open(metadata_path, 'w', encoding='utf-8') as f:
                json.dump(metadata, f, ensure_ascii=False, indent=2)
            logger.info(f"모델 메타데이터가 저장되었습니다: {metadata_path}")
            return metadata_path
        except Exception as e:
            logger.error(f"모델 메타데이터 저장 실패: {e}")
            raise
    
    def evaluate_model_on_test_set(self, model_config: pd.Series, 
                                 X_train_val: pd.DataFrame, y_train_val: pd.Series,
                                 X_test: pd.DataFrame, y_test: pd.Series) -> Dict[str, float]:
        """테스트 세트에서 모델 평가"""
        logger.info(f"모델 '{model_config['ModelName']}' 테스트 세트 평가 중...")
        
        # 모델 생성
        model = self.create_model_from_config(model_config)
        if model is None:
            return {}
        
        # 모델 학습 (전체 Train/Val 데이터 사용)
        import time
        start_time = time.time()
        model.fit(X_train_val, y_train_val)
        training_time = time.time() - start_time
        
        # 테스트 세트 예측
        y_pred = model.predict(X_test)
        y_pred_proba = model.predict_proba(X_test)[:, 1]
        
        # 성능 지표 계산
        test_metrics = {
            'Test_Accuracy': accuracy_score(y_test, y_pred),
            'Test_Precision': precision_score(y_test, y_pred, zero_division=0),
            'Test_Recall': recall_score(y_test, y_pred, zero_division=0),
            'Test_F1': f1_score(y_test, y_pred, zero_division=0),
            'Test_ROC_AUC': roc_auc_score(y_test, y_pred_proba),
            'Test_TrainingTime': training_time
        }
        
        logger.info(f"테스트 성능: F1={test_metrics['Test_F1']:.4f}, ROC-AUC={test_metrics['Test_ROC_AUC']:.4f}")
        
        return test_metrics
    
    def load_data_for_daily_analysis(self, split_ratio: float = 0.5) -> pl.DataFrame:
        """하루 기준 분석을 위한 데이터 로드 (뒷 절반)"""
        logger.info("하루 기준 분석용 데이터 로딩 중...")
        
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
            
            # 쿼리 생성
            features_str = ', '.join([feature_mapping[feature] for feature in self.features])
            query = f"""
            SELECT a.date, a.stock_code, {features_str}, e.is_answer
            FROM day1 a
            INNER JOIN day2 b ON a.date = b.date AND a.stock_code = b.stock_code
            INNER JOIN day3 c ON a.date = c.date AND a.stock_code = c.stock_code
            INNER JOIN day4 d ON a.date = d.date AND a.stock_code = d.stock_code
            INNER JOIN answer_v3 e ON CAST(REPLACE(a.date, '-', '') AS INTEGER) = e.date 
                               AND a.stock_code = e.stock_code
            WHERE e.date < 20230601
            ORDER BY a.date, a.stock_code
            """
            
            # pandas로 먼저 읽고 polars로 변환
            df_pandas = pd.read_sql_query(query, conn)
            conn.close()
            
            # polars DataFrame으로 변환
            df = pl.from_pandas(df_pandas)
            
            # 뒷 절반 데이터만 사용
            total_rows = len(df)
            split_point = int(total_rows * split_ratio)
            df = df.tail(total_rows - split_point)
            
            logger.info(f"하루 기준 분석용 데이터 로드 완료: {len(df)} 행")
            return df
            
        except Exception as e:
            logger.error(f"하루 기준 분석용 데이터 로드 실패: {e}")
            raise
    
    def prepare_data_for_prediction(self, df: pl.DataFrame) -> pd.DataFrame:
        """예측을 위해 데이터를 준비합니다."""
        # date와 stock_code는 제외하고 수치형 컬럼만 선택
        numeric_columns = []
        for col in df.columns:
            if col not in ['date', 'stock_code', 'is_answer']:
                if df[col].dtype in [pl.Float64, pl.Float32, pl.Int64, pl.Int32]:
                    numeric_columns.append(col)
        
        # NaN 값 처리
        df_clean = df.select(numeric_columns).drop_nulls()
        
        # polars DataFrame을 pandas로 변환
        df_pandas = df_clean.to_pandas()
        
        # 전처리 적용
        df_pandas = self.preprocess_data(df_pandas)
        
        return df_pandas
    
    def select_top_stock_per_day(self, df_original: pl.DataFrame, predictions: np.ndarray, 
                                prediction_probas: np.ndarray, top_k: int = 1) -> pd.DataFrame:
        """하루 기준으로 가장 확률이 높은 종목만 선택합니다."""
        # 원본 데이터에서 필요한 컬럼들 가져오기
        result_df = df_original.select(['date', 'stock_code']).to_pandas()
        
        # 예측 결과 추가
        result_df['predicted_class'] = predictions
        result_df['predicted_proba_0'] = prediction_probas[:, 0]  # 급등 아님 확률
        result_df['predicted_proba_1'] = prediction_probas[:, 1]  # 급등 확률
        
        # 실제 값이 있다면 추가
        if 'is_answer' in df_original.columns:
            result_df['actual_class'] = df_original.select('is_answer').to_pandas()
        
        # 하루 기준으로 급등 확률이 높은 순으로 정렬하고 상위 k개 선택
        result_df = result_df.sort_values(['date', 'predicted_proba_1'], ascending=[True, False])
        
        # 각 날짜별로 상위 k개 선택
        top_stocks = []
        for date in result_df['date'].unique():
            day_data = result_df[result_df['date'] == date]
            top_day_stocks = day_data.head(top_k)
            top_stocks.append(top_day_stocks)
        
        top_result_df = pd.concat(top_stocks, ignore_index=True)
        
        # 날짜 범위 출력
        unique_dates = sorted(top_result_df['date'].unique())
        start_date = unique_dates[0]
        end_date = unique_dates[-1]
        
        logger.info(f"하루 기준 상위 {top_k}개 종목 선택 완료: {len(top_result_df)} 종목")
        logger.info(f"고유 날짜 수: {len(unique_dates)}")
        logger.info(f"분석 기간: {start_date} ~ {end_date}")
        
        return top_result_df
    
    def calculate_daily_performance_metrics(self, result_df: pd.DataFrame) -> Dict[str, Any]:
        """하루 기준 예측 성능 지표를 계산합니다."""
        if 'actual_class' not in result_df.columns:
            logger.warning("실제 정답 데이터가 없어서 성능 지표를 계산할 수 없습니다.")
            return None
        
        # NaN 값 제거
        valid_data = result_df.dropna(subset=['actual_class', 'predicted_class'])
        
        if len(valid_data) == 0:
            logger.warning("유효한 데이터가 없어서 성능 지표를 계산할 수 없습니다.")
            return None
        
        y_true = valid_data['actual_class']
        y_pred = valid_data['predicted_class']
        
        # 기본 성능 지표 계산
        accuracy = accuracy_score(y_true, y_pred)
        precision = precision_score(y_true, y_pred, zero_division=0)
        recall = recall_score(y_true, y_pred, zero_division=0)
        f1 = f1_score(y_true, y_pred, zero_division=0)
        
        # 혼동 행렬
        cm = confusion_matrix(y_true, y_pred)
        
        # 상세 분류 리포트
        report = classification_report(y_true, y_pred, output_dict=True)
        
        metrics = {
            'accuracy': accuracy,
            'precision': precision,
            'recall': recall,
            'f1_score': f1,
            'confusion_matrix': cm.tolist(),
            'classification_report': report,
            'total_samples': len(valid_data),
            'positive_samples': sum(y_true == 1),
            'negative_samples': sum(y_true == 0)
        }
        
        return metrics
    
    def plot_daily_analysis(self, result_df: pd.DataFrame, model_name: str, top_k: int, 
                          output_dir: str = 'results/best_model_analysis'):
        """하루 기준 분석 결과를 시각화합니다."""
        Path(output_dir).mkdir(parents=True, exist_ok=True)
        
        plt.figure(figsize=(15, 10))
        
        # 1. 선택된 종목들의 급등 확률 분포
        plt.subplot(2, 3, 1)
        plt.hist(result_df['predicted_proba_1'], bins=30, alpha=0.7, color='skyblue', edgecolor='black')
        plt.title(f'선택된 상위 {top_k}개 종목의 급등 확률 분포')
        plt.xlabel('급등 확률')
        plt.ylabel('빈도')
        plt.grid(alpha=0.3)
        
        # 2. 실제 급등 vs 비급등에 따른 확률 분포
        if 'actual_class' in result_df.columns:
            plt.subplot(2, 3, 2)
            actual_positive = result_df[result_df['actual_class'] == 1]['predicted_proba_1']
            actual_negative = result_df[result_df['actual_class'] == 0]['predicted_proba_1']
            
            plt.hist(actual_negative, bins=20, alpha=0.7, label='실제 비급등', color='red')
            plt.hist(actual_positive, bins=20, alpha=0.7, label='실제 급등', color='blue')
            plt.title('실제 클래스별 예측 확률 분포')
            plt.xlabel('급등 확률')
            plt.ylabel('빈도')
            plt.legend()
            plt.grid(alpha=0.3)
            
            # 3. ROC 커브
            plt.subplot(2, 3, 3)
            fpr, tpr, _ = roc_curve(result_df['actual_class'], result_df['predicted_proba_1'])
            roc_auc = auc(fpr, tpr)
            
            plt.plot(fpr, tpr, color='darkorange', lw=2, label=f'ROC curve (AUC = {roc_auc:.3f})')
            plt.plot([0, 1], [0, 1], color='navy', lw=2, linestyle='--')
            plt.xlim([0.0, 1.0])
            plt.ylim([0.0, 1.05])
            plt.xlabel('False Positive Rate')
            plt.ylabel('True Positive Rate')
            plt.title('ROC Curve')
            plt.legend(loc="lower right")
            plt.grid(alpha=0.3)
            
            # 4. Precision-Recall 커브
            plt.subplot(2, 3, 4)
            precision, recall, _ = precision_recall_curve(result_df['actual_class'], result_df['predicted_proba_1'])
            
            plt.plot(recall, precision, color='green', lw=2)
            plt.xlabel('Recall')
            plt.ylabel('Precision')
            plt.title('Precision-Recall Curve')
            plt.grid(alpha=0.3)
            
            # 5. 날짜별 급등 확률 변화
            plt.subplot(2, 3, 5)
            result_df['date'] = pd.to_datetime(result_df['date'])
            result_df_sorted = result_df.sort_values('date')
            
            plt.scatter(result_df_sorted['date'], result_df_sorted['predicted_proba_1'], 
                       alpha=0.6, s=20, c=result_df_sorted['actual_class'], cmap='RdYlBu')
            plt.title('날짜별 선택된 종목의 급등 확률')
            plt.xlabel('날짜')
            plt.ylabel('급등 확률')
            plt.xticks(rotation=45)
            plt.grid(alpha=0.3)
            
            # 6. 급등 확률 구간별 성공률
            plt.subplot(2, 3, 6)
            bins = [0, 0.2, 0.4, 0.6, 0.8, 1.0]
            labels = ['0-0.2', '0.2-0.4', '0.4-0.6', '0.6-0.8', '0.8-1.0']
            result_df['prob_bin'] = pd.cut(result_df['predicted_proba_1'], bins=bins, labels=labels)
            
            success_rate = result_df.groupby('prob_bin')['actual_class'].mean()
            count_per_bin = result_df.groupby('prob_bin').size()
            
            x_pos = range(len(success_rate))
            plt.bar(x_pos, success_rate.values, alpha=0.7, color='lightcoral')
            plt.title('확률 구간별 실제 급등 비율')
            plt.xlabel('급등 확률 구간')
            plt.ylabel('실제 급등 비율')
            plt.xticks(x_pos, success_rate.index, rotation=45)
            plt.grid(alpha=0.3)
            
            # 각 막대 위에 개수 표시
            for i, (rate, count) in enumerate(zip(success_rate.values, count_per_bin.values)):
                plt.text(i, rate + 0.01, f'n={count}', ha='center', va='bottom')
        
        plt.tight_layout()
        
        output_path = f"{output_dir}/{model_name}_top{top_k}_daily_analysis.png"
        plt.savefig(output_path, dpi=300, bbox_inches='tight')
        plt.close()
        
        logger.info(f"하루 기준 분석 그래프가 '{output_path}'에 저장되었습니다.")
    
    def save_analysis_results(self, test_metrics: Dict[str, float], daily_metrics: Dict[str, Any], 
                            model_name: str, top_k: int, output_dir: str = 'results/best_model_analysis'):
        """분석 결과를 저장합니다."""
        Path(output_dir).mkdir(parents=True, exist_ok=True)
        
        # 결과 통합
        results = {
            'model_name': model_name,
            'top_k': top_k,
            'test_metrics': test_metrics,
            'daily_metrics': daily_metrics,
            'analysis_timestamp': datetime.now().isoformat()
        }
        
        # JSON 파일로 저장
        output_path = f"{output_dir}/{model_name}_analysis_results.json"
        with open(output_path, 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)
        
        logger.info(f"분석 결과가 '{output_path}'에 저장되었습니다.")
    
    def run_complete_analysis(self, result_file: str = "result.csv", sort_by: str = "F1", 
                            selected_model_index: int = 0, top_k: int = 1):
        """완전한 분석 파이프라인 실행"""
        print("="*60)
        print("최고 모델 분석 및 평가 파이프라인 시작")
        print("="*60)
        
        try:
            # 1. 결과 파일 로드 및 상위 5개 모델 선택
            top_5_models = self.load_and_sort_results(result_file, sort_by)
            
            # 2. 선택된 모델 정보 출력
            selected_model = top_5_models.iloc[selected_model_index]
            print(f"\n선택된 모델: {selected_model['ModelName']}")
            print(f"정렬 기준 {sort_by}: {selected_model[sort_by]:.4f}")
            
            # 3. 테스트 세트 평가를 위한 데이터 로드
            X_train_val, y_train_val, X_test, y_test = self.load_data_for_evaluation()
            
            # 4. 테스트 세트에서 모델 평가
            test_metrics = self.evaluate_model_on_test_set(
                selected_model, X_train_val, y_train_val, X_test, y_test
            )
            
            print(f"\n테스트 세트 평가 결과:")
            print(f"  정확도: {test_metrics['Test_Accuracy']:.4f}")
            print(f"  정밀도: {test_metrics['Test_Precision']:.4f}")
            print(f"  재현율: {test_metrics['Test_Recall']:.4f}")
            print(f"  F1 점수: {test_metrics['Test_F1']:.4f}")
            print(f"  ROC-AUC: {test_metrics['Test_ROC_AUC']:.4f}")
            
            # 5. 모델 저장
            model = self.create_model_from_config(selected_model)
            model.fit(X_train_val, y_train_val)  # 전체 학습 데이터로 학습
            
            # 모델 저장
            model_path = self.save_model(model, selected_model['ModelName'])
            
            # 모델 메타데이터 저장
            metadata_path = self.save_model_metadata(selected_model, test_metrics, model_path)
            
            print(f"\n모델 저장 완료:")
            print(f"  모델 파일: {model_path}")
            print(f"  메타데이터: {metadata_path}")
            
            # 6. 하루 기준 분석을 위한 데이터 로드
            df_daily = self.load_data_for_daily_analysis()
            
            # 하루 기준 분석용 데이터 준비
            X_daily = self.prepare_data_for_prediction(df_daily)
            
            # 예측 수행
            predictions = model.predict(X_daily)
            prediction_probas = model.predict_proba(X_daily)
            
            # 하루 기준 상위 종목 선택
            result_df = self.select_top_stock_per_day(df_daily, predictions, prediction_probas, top_k)
            
            # 7. 하루 기준 성능 지표 계산
            daily_metrics = self.calculate_daily_performance_metrics(result_df)
            
            if daily_metrics:
                print(f"\n하루 기준 상위 {top_k}개 종목 분석 결과:")
                print(f"  총 선택된 종목 수: {daily_metrics['total_samples']}")
                print(f"  실제 급등 종목 수: {daily_metrics['positive_samples']}")
                print(f"  급등 성공률: {daily_metrics['positive_samples']/daily_metrics['total_samples']*100:.2f}%")
                print(f"  정확도: {daily_metrics['accuracy']:.4f}")
                print(f"  정밀도: {daily_metrics['precision']:.4f}")
                print(f"  재현율: {daily_metrics['recall']:.4f}")
                print(f"  F1 점수: {daily_metrics['f1_score']:.4f}")
            
            # 8. 결과 시각화
            model_name_clean = selected_model['ModelName'].replace(' ', '_').replace('(', '').replace(')', '')
            self.plot_daily_analysis(result_df, model_name_clean, top_k)
            
            # 9. 결과 저장
            self.save_analysis_results(test_metrics, daily_metrics, model_name_clean, top_k)
            
            # 10. CSV 파일로 예측 결과 저장
            output_dir = 'results/best_model_analysis'
            Path(output_dir).mkdir(parents=True, exist_ok=True)
            csv_path = f"{output_dir}/{model_name_clean}_top{top_k}_predictions.csv"
            result_df.to_csv(csv_path, index=False)
            logger.info(f"예측 결과가 '{csv_path}'에 저장되었습니다.")
            
            print("="*60)
            print("분석 완료!")
            print("="*60)
            
            return {
                'selected_model': selected_model,
                'test_metrics': test_metrics,
                'daily_metrics': daily_metrics,
                'result_df': result_df,
                'model_path': model_path,
                'metadata_path': metadata_path
            }
            
        except Exception as e:
            logger.error(f"분석 실패: {e}")
            raise

def main():
    """메인 실행 함수"""
    # JSON 설정 파일 로드
    config_file = "config_analysis.json"
    
    try:
        with open(config_file, 'r', encoding='utf-8') as f:
            config = json.load(f)
    except FileNotFoundError:
        # 기본 설정으로 config 파일 생성
        config = {
            # 분석할 결과 파일 경로 (complete_ml_pipeline.py에서 생성된 파일)
            "result_file": "result.csv",
            
            # 정렬 기준 (상위 5개 모델 중 선택할 기준)
            # 가능한 값: "F1", "Accuracy", "Precision", "Recall", "ROC_AUC"
            "sort_by": "Precision",
            
            # 선택할 모델 인덱스 (0-4, 상위 5개 중 몇 번째 모델을 선택할지)
            # 0: 1위 모델, 1: 2위 모델, 2: 3위 모델, 3: 4위 모델, 4: 5위 모델
            "selected_model_index": 0,
            
            # 하루에 선택할 상위 종목 수
            # 1: 하루 1개 종목, 3: 하루 3개 종목, 5: 하루 5개 종목 등
            "top_k": 1,
            
            # 데이터베이스 경로
            "db_path": "D:/db/solomon.db",
            
            # 특성 파일 경로 (features.txt)
            "features_file": "features.txt"
        }
        with open(config_file, 'w', encoding='utf-8') as f:
            json.dump(config, f, ensure_ascii=False, indent=2)
        print(f"기본 설정 파일 '{config_file}'이 생성되었습니다.")
        print("\n=== 설정 옵션 설명 ===")
        print("sort_by: 정렬 기준")
        print("  - F1: F1 점수 기준 (기본값)")
        print("  - Accuracy: 정확도 기준")
        print("  - Precision: 정밀도 기준")
        print("  - Recall: 재현율 기준")
        print("  - ROC_AUC: ROC-AUC 점수 기준")
        print("\nselected_model_index: 선택할 모델 (0-4)")
        print("  - 0: 1위 모델, 1: 2위 모델, 2: 3위 모델, 3: 4위 모델, 4: 5위 모델")
        print("\ntop_k: 하루에 선택할 종목 수")
        print("  - 1: 하루 1개 종목, 3: 하루 3개 종목, 5: 하루 5개 종목 등")
        print("\n필요에 따라 config_analysis.json 파일을 수정하세요.")
    
    # 분석기 인스턴스 생성
    analyzer = BestModelAnalyzer(config['db_path'], config['features_file'])
    
    # 완전한 분석 실행
    results = analyzer.run_complete_analysis(
        config['result_file'], 
        config['sort_by'], 
        config['selected_model_index'], 
        config['top_k']
    )
    
    return results

if __name__ == "__main__":
    main() 