#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
딥러닝 MLP 완전한 파이프라인: PyTorch 기반
데이터 로딩 → 전처리 → 분할 → 모델 학습 → k-fold 평가 → 하이퍼파라미터 튜닝 → 성능 측정
"""

import pandas as pd
import numpy as np
import sqlite3
import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import DataLoader, TensorDataset
from sklearn.model_selection import train_test_split, StratifiedKFold
from sklearn.metrics import accuracy_score, precision_score, recall_score, f1_score, roc_auc_score
from sklearn.preprocessing import StandardScaler
import time
import warnings
import logging
from typing import Tuple, Dict, List, Any, Optional
import os
import matplotlib.pyplot as plt
from itertools import product

# 경고 무시
warnings.filterwarnings('ignore')

# 한글 폰트 설정
plt.rcParams['font.family'] = 'Malgun Gothic'  # Windows 기본 한글 폰트
plt.rcParams['axes.unicode_minus'] = False     # 마이너스 기호 깨짐 방지

# 콘솔 인코딩 설정
import sys
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

# GPU 사용 가능 여부 확인
device = torch.device('cuda' if torch.cuda.is_available() else 'cpu')
logger.info(f"사용 디바이스: {device}")

class StockMLP(nn.Module):
    """주식 예측용 MLP 모델"""
    
    def __init__(self, input_dim: int, hidden_dims: List[int] = [128, 64], 
                 dropout_rate: float = 0.3, use_batch_norm: bool = True):
        super(StockMLP, self).__init__()
        
        layers = []
        prev_dim = input_dim
        
        for hidden_dim in hidden_dims:
            layers.append(nn.Linear(prev_dim, hidden_dim))
            
            if use_batch_norm:
                layers.append(nn.BatchNorm1d(hidden_dim))
            
            layers.append(nn.ReLU())
            layers.append(nn.Dropout(dropout_rate))
            prev_dim = hidden_dim
        
        # 출력층
        layers.append(nn.Linear(prev_dim, 1))
        layers.append(nn.Sigmoid())
        
        self.model = nn.Sequential(*layers)
        
    def forward(self, x):
        return self.model(x)

class EarlyStopping:
    """Early Stopping 구현"""
    
    def __init__(self, patience: int = 10, min_delta: float = 0.001, restore_best_weights: bool = True):
        self.patience = patience
        self.min_delta = min_delta
        self.restore_best_weights = restore_best_weights
        self.best_score = None
        self.counter = 0
        self.best_weights = None
        
    def __call__(self, val_score: float, model: nn.Module) -> bool:
        if self.best_score is None:
            self.best_score = val_score
            if self.restore_best_weights:
                self.best_weights = model.state_dict().copy()
        elif val_score > self.best_score + self.min_delta:
            self.best_score = val_score
            self.counter = 0
            if self.restore_best_weights:
                self.best_weights = model.state_dict().copy()
        else:
            self.counter += 1
            
        if self.counter >= self.patience:
            if self.restore_best_weights and self.best_weights is not None:
                model.load_state_dict(self.best_weights)
            return True
        return False

class CompleteMLPPipeline:
    """딥러닝 MLP 완전한 파이프라인 클래스"""
    
    def __init__(self, db_path: str = "D:/db/solomon.db", features_file: str = "features.txt"):
        self.db_path = db_path
        self.features_file = features_file
        self.features = self._load_features()
        self.random_state = 42
        self.test_split_ratio = 0.2
        self.scaler = StandardScaler()
        
        # PyTorch 시드 설정
        torch.manual_seed(self.random_state)
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
            
            # Train/Val과 Test 분할
            X_train_val, X_test, y_train_val, y_test = self._split_train_test(X, y)
            
            return X_train_val, y_train_val, X_test, y_test
            
        except Exception as e:
            logger.error(f"데이터 로딩 실패: {e}")
            raise
    
    def _split_train_test(self, X: pd.DataFrame, y: pd.Series) -> Tuple[pd.DataFrame, pd.DataFrame, pd.Series, pd.Series]:
        """Train/Val과 Test 데이터 분할"""
        logger.info(f"Train/Val ({1-self.test_split_ratio:.1%})과 Test ({self.test_split_ratio:.1%}) 분할 중...")
        
        X_train_val, X_test, y_train_val, y_test = train_test_split(
            X, y, 
            test_size=self.test_split_ratio, 
            random_state=self.random_state,
            stratify=y
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
        
        logger.info(f"전처리 완료: X shape={X.shape}")
        return X
    
    def get_model_configs(self) -> Dict[str, Dict[str, Any]]:
        """✅ [3] MLP 모델 하이퍼파라미터 조합 정의 (최적화된 버전)"""
        return {
            'MLP': {
                'hidden_dims': [[128, 64], [256, 128]],  # 3개 → 2개로 축소
                'dropout_rate': [0.2, 0.3],  # 3개 → 2개로 축소
                'learning_rate': [0.001, 0.0005],  # 3개 → 2개로 축소
                'batch_size': [64, 128],  # 3개 → 2개로 축소
                'use_batch_norm': [True],  # 2개 → 1개로 축소
                'weight_decay': [0.0, 0.0001]  # 3개 → 2개로 축소
            }
        }
    
    def create_data_loaders(self, X_train: np.ndarray, y_train: np.ndarray, 
                           X_val: np.ndarray, y_val: np.ndarray, 
                           batch_size: int) -> Tuple[DataLoader, DataLoader]:
        """PyTorch DataLoader 생성"""
        # 텐서 변환
        X_train_tensor = torch.FloatTensor(X_train).to(device)
        y_train_tensor = torch.FloatTensor(y_train.values).to(device)
        X_val_tensor = torch.FloatTensor(X_val).to(device)
        y_val_tensor = torch.FloatTensor(y_val.values).to(device)
        
        # Dataset 생성
        train_dataset = TensorDataset(X_train_tensor, y_train_tensor)
        val_dataset = TensorDataset(X_val_tensor, y_val_tensor)
        
        # DataLoader 생성
        train_loader = DataLoader(train_dataset, batch_size=batch_size, shuffle=True)
        val_loader = DataLoader(val_dataset, batch_size=batch_size, shuffle=False)
        
        return train_loader, val_loader
    
    def train_model(self, model: nn.Module, train_loader: DataLoader, val_loader: DataLoader,
                   learning_rate: float, weight_decay: float, max_epochs: int = 100) -> Dict[str, List[float]]:
        """모델 학습"""
        criterion = nn.BCELoss()
        optimizer = optim.Adam(model.parameters(), lr=learning_rate, weight_decay=weight_decay)
        early_stopping = EarlyStopping(patience=15, min_delta=0.001)
        
        train_losses = []
        val_losses = []
        val_f1_scores = []
        
        for epoch in range(max_epochs):
            # 학습
            model.train()
            train_loss = 0.0
            for batch_X, batch_y in train_loader:
                optimizer.zero_grad()
                outputs = model(batch_X).squeeze()
                loss = criterion(outputs, batch_y)
                loss.backward()
                optimizer.step()
                train_loss += loss.item()
            
            train_loss /= len(train_loader)
            train_losses.append(train_loss)
            
            # 검증
            model.eval()
            val_loss = 0.0
            val_preds = []
            val_targets = []
            
            with torch.no_grad():
                for batch_X, batch_y in val_loader:
                    outputs = model(batch_X).squeeze()
                    loss = criterion(outputs, batch_y)
                    val_loss += loss.item()
                    
                    val_preds.extend((outputs > 0.5).cpu().numpy())
                    val_targets.extend(batch_y.cpu().numpy())
            
            val_loss /= len(val_loader)
            val_losses.append(val_loss)
            
            # F1 스코어 계산
            val_f1 = f1_score(val_targets, val_preds, zero_division=0)
            val_f1_scores.append(val_f1)
            
            if epoch % 10 == 0:
                logger.info(f"Epoch {epoch}: Train Loss={train_loss:.4f}, Val Loss={val_loss:.4f}, Val F1={val_f1:.4f}")
            
            # Early Stopping
            if early_stopping(val_f1, model):
                logger.info(f"Early stopping at epoch {epoch}")
                break
        
        return {
            'train_losses': train_losses,
            'val_losses': val_losses,
            'val_f1_scores': val_f1_scores
        }
    
    def evaluate_model(self, model: nn.Module, X: np.ndarray, y: pd.Series) -> Dict[str, float]:
        """모델 평가"""
        model.eval()
        X_tensor = torch.FloatTensor(X).to(device)
        
        with torch.no_grad():
            outputs = model(X_tensor).squeeze()
            predictions = (outputs > 0.5).cpu().numpy()
            probabilities = outputs.cpu().numpy()
        
        metrics = {
            'Accuracy': accuracy_score(y, predictions),
            'Precision': precision_score(y, predictions, zero_division=0),
            'Recall': recall_score(y, predictions, zero_division=0),
            'F1': f1_score(y, predictions, zero_division=0),
            'ROC_AUC': roc_auc_score(y, probabilities)
        }
        
        return metrics
    
    def run_kfold_evaluation(self, X: pd.DataFrame, y: pd.Series, 
                           model_configs: Dict[str, Dict[str, Any]], k_folds: int = 3) -> List[Dict[str, Any]]:
        """✅ [4] 각 하이퍼파라미터 조합별 k-fold 교차검증 평가"""
        logger.info("하이퍼파라미터 조합별 k-fold 교차검증 시작...")
        
        results = []
        
        for model_name, config in model_configs.items():
            logger.info(f"{model_name} k-fold 평가 중...")
            
            # 모든 하이퍼파라미터 조합 생성
            param_names = list(config.keys())
            param_values = list(config.values())
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
                        
                        # 스케일링
                        X_train_scaled = self.scaler.fit_transform(X_train)
                        X_val_scaled = self.scaler.transform(X_val)
                        
                        # DataLoader 생성
                        train_loader, val_loader = self.create_data_loaders(
                            X_train_scaled, y_train, X_val_scaled, y_val, 
                            params_dict['batch_size']
                        )
                        
                        # 모델 생성
                        model = StockMLP(
                            input_dim=len(self.features),
                            hidden_dims=params_dict['hidden_dims'],
                            dropout_rate=params_dict['dropout_rate'],
                            use_batch_norm=params_dict['use_batch_norm']
                        ).to(device)
                        
                        # 모델 학습
                        start_time = time.time()
                        training_history = self.train_model(
                            model, train_loader, val_loader,
                            params_dict['learning_rate'],
                            params_dict['weight_decay']
                        )
                        training_time = time.time() - start_time
                        training_times.append(training_time)
                        
                        # 검증 데이터로 평가
                        val_metrics = self.evaluate_model(model, X_val_scaled, y_val)
                        
                        for metric_name, value in val_metrics.items():
                            fold_metrics[metric_name].append(value)
                    
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
    
    def save_results(self, results: List[Dict[str, Any]], filename: str = "mlp_result.csv"):
        """✅ [5] 결과 저장"""
        logger.info(f"결과를 {filename}에 저장 중...")
        
        df_results = pd.DataFrame(results)
        
        if 'F1' in df_results.columns:
            df_results = df_results.sort_values('F1', ascending=False)
        
        columns = ['ModelName', 'BaseModel', 'Accuracy', 'Precision', 'Recall', 'F1', 'ROC_AUC', 'TrainingTime', 'BestParams']
        available_columns = [col for col in columns if col in df_results.columns]
        df_results = df_results[available_columns]
        
        df_results.to_csv(filename, index=False, encoding='utf-8-sig')
        
        print(f"\n결과가 {filename}에 저장되었습니다.")
        print(f"총 {len(df_results)}개 모델 평가 완료")
        
        if 'F1' in df_results.columns:
            best_model = df_results.iloc[0]
            print(f"최고 F1: {best_model['F1']:.4f} ({best_model['ModelName']})")
        
        return df_results
    
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
        
        # 스케일링
        X_train_val_scaled = self.scaler.fit_transform(X_train_val)
        X_test_scaled = self.scaler.transform(X_test)
        
        # DataLoader 생성
        train_loader, _ = self.create_data_loaders(
            X_train_val_scaled, y_train_val, X_test_scaled, y_test,
            best_params['batch_size']
        )
        
        # 모델 생성
        model = StockMLP(
            input_dim=len(self.features),
            hidden_dims=best_params['hidden_dims'],
            dropout_rate=best_params['dropout_rate'],
            use_batch_norm=best_params['use_batch_norm']
        ).to(device)
        
        # 모델 학습 (전체 Train/Val 데이터 사용)
        start_time = time.time()
        training_history = self.train_model(
            model, train_loader, None,  # 검증 로더는 None으로 설정
            best_params['learning_rate'],
            best_params['weight_decay']
        )
        training_time = time.time() - start_time
        
        # Test Set 평가
        test_metrics = self.evaluate_model(model, X_test_scaled, y_test)
        test_metrics['Test_TrainingTime'] = training_time
        
        logger.info(f"Test Set 성능:")
        for metric, value in test_metrics.items():
            if 'Time' in metric:
                logger.info(f"  {metric}: {value:.4f}초")
            else:
                logger.info(f"  {metric}: {value:.4f}")
        
        return test_metrics
    
    def run_complete_pipeline(self, k_folds: int = 3):
        """완전한 파이프라인 실행"""
        print("="*60)
        print("딥러닝 MLP 완전한 파이프라인 시작")
        print("="*60)
        
        try:
            # 1. 데이터 로딩
            X_train_val, y_train_val, X_test, y_test = self.load_data()
            
            # 2. 전처리
            X_train_val = self.preprocess_data(X_train_val)
            X_test = self.preprocess_data(X_test)
            
            # 3. 모델 정의
            model_configs = self.get_model_configs()
            
            # 4. k-fold 평가
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

def main():
    """메인 실행 함수"""
    os.environ['PYTHONIOENCODING'] = 'utf-8'
    
    # 파이프라인 인스턴스 생성
    pipeline = CompleteMLPPipeline()
    
    # 완전한 파이프라인 실행 (빠른 테스트용)
    results = pipeline.run_complete_pipeline(k_folds=2)
    
    return results

if __name__ == "__main__":
    main() 