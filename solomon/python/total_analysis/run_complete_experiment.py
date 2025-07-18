import sqlite3
import polars as pl
import numpy as np
import pandas as pd
from sklearn.ensemble import RandomForestClassifier
from sklearn.model_selection import train_test_split, cross_val_score, RandomizedSearchCV
from sklearn.preprocessing import StandardScaler
from sklearn.metrics import accuracy_score, precision_score, recall_score, f1_score, confusion_matrix, classification_report, roc_auc_score
from pathlib import Path
from joblib import dump, load
import argparse
from imblearn.over_sampling import SMOTE
from imblearn.pipeline import Pipeline as ImbPipeline
import matplotlib.pyplot as plt
import seaborn as sns
import json
from datetime import datetime
import shap

# 한글 폰트 설정
import matplotlib.font_manager as fm

# Windows에서 사용 가능한 한글 폰트 찾기
font_list = [f.name for f in fm.fontManager.ttflist]
korean_fonts = [f for f in font_list if 'Malgun' in f or '맑은' in f or 'Gulim' in f or '굴림' in f]

if korean_fonts:
    plt.rcParams['font.family'] = korean_fonts[0]
    print(f"한글 폰트 설정: {korean_fonts[0]}")
else:
    # 폰트가 없으면 기본 설정
    plt.rcParams['font.family'] = 'DejaVu Sans'
    print("한글 폰트를 찾을 수 없어 기본 폰트를 사용합니다.")

plt.rcParams['axes.unicode_minus'] = False     # 마이너스 기호 깨짐 방지

# ============================================================================
# 실험 설정 딕셔너리 - 여기서 True/False로 실험 설정을 변경하세요
# ============================================================================
EXPERIMENT_CONFIG = {
    # 데이터베이스 설정
    'db_path': 'D:/db/solomon.db',
    'split_ratio': 0.5,  # 앞 절반 데이터로 학습, 뒷 절반으로 평가
    
    # 모델 설정
    'use_smote': False,  # SMOTE 사용 여부
    'test_size': 0.2,    # 훈련/테스트 분할 비율
    'random_state': 42,
    
    # 하이퍼파라미터 최적화 설정
    'hyperparameter_optimization': {
        'enabled': False,  # 하이퍼파라미터 최적화 사용 여부
        'scoring': 'precision',  # 최적화 기준: 'accuracy', 'precision', 'recall', 'f1', 'roc_auc'
        'cv_folds': 5,  # 교차 검증 폴드 수
        'n_iter': 50,  # 랜덤 서치 반복 횟수
        'param_distributions': {
            'n_estimators': [50, 100, 200, 300],
            'max_depth': [5, 10, 15, 20, None],
            'min_samples_split': [2, 5, 10, 15],
            'min_samples_leaf': [1, 2, 4, 8],
            'max_features': ['sqrt', 'log2', None]
        }
    },
    
    # 랜덤포레스트 하이퍼파라미터 (최적화 비활성화시 사용)
    'n_estimators': 100,
    'max_depth': 10,
    'min_samples_split': 5,
    'min_samples_leaf': 2,
    'class_weight': 'balanced',
    
    # 실험할 모델 조합들 (True로 설정된 것만 실행)
    'experiments': {
        'day1': False,
        'day2': False,
        'day3': False,
        'day4': False,
        'day1_day2_day3': False,
        'day1_day2_day3_day4': True
    },
    
    # 평가 설정
    'top_k': 1,  # 하루에 선택할 상위 종목 수
    
    # 특성 선택 (True로 설정된 특성만 사용)
    'feature_selection': {
        'use_all_features': False,  # False로 설정하여 특정 특성만 사용
        'selected_features': [
            'day4_macd_histogram_increasing',
            'day4_short_macd_cross_signal',
            'day1_current_price_ratio',
            'day1_high_price_ratio',
            'day1_low_price_ratio',
            'day4_open_to_now_return',
            'day4_is_long_bull_candle',
            'day1_price_position_ratio',
            'day3_morning_mdd',
            'day1_fourth_derivative',
            'day1_long_candle_ratio',
            'day1_fifth_derivative',
            'day3_breaks_6month_high',
            'day2_prev_day_range_ratio',
            'day2_prev_close_to_now_ratio',
            'day4_macd_histogram',
            'day1_sixth_derivative',
            'day4_pos_vs_high_5d',
            'day4_rsi_value',
            'day4_pos_vs_high_3d'
        ]
    }
}

# ============================================================================
# 유틸리티 함수들
# ============================================================================

def load_data_from_db(day_types, db_path, split_ratio=0.5, use_train_data=True):
    """
    SQLite 데이터베이스에서 지정된 day 테이블들과 answer 테이블을 조인하여 데이터를 로드합니다.
    use_train_data: True면 앞 절반(학습용), False면 뒷 절반(평가용)
    """
    try:
        conn = sqlite3.connect(db_path)
        
        if isinstance(day_types, str):
            day_types = [day_types]
        
        # 단일 테이블인 경우
        if len(day_types) == 1:
            query = f"""
            SELECT d1.*, a.is_answer
            FROM {day_types[0]} d1
            INNER JOIN answer_v3 a ON CAST(REPLACE(d1.date, '-', '') AS INTEGER) = a.date 
                               AND d1.stock_code = a.stock_code
            WHERE a.date < 20230601
            ORDER BY d1.date, d1.stock_code
            """
        else:
            # 여러 테이블인 경우 컬럼명을 명시적으로 지정
            table_columns = {}
            for day_type in day_types:
                cursor = conn.cursor()
                cursor.execute(f"PRAGMA table_info({day_type})")
                columns = [row[1] for row in cursor.fetchall()]
                table_columns[day_type] = columns
            
            # SELECT 절 구성 (중복 컬럼 제외)
            select_parts = []
            used_columns = set()
            
            for i, day_type in enumerate(day_types):
                alias = f"d{i+1}"
                for col in table_columns[day_type]:
                    if col in ['date', 'stock_code']:
                        if col not in used_columns:
                            select_parts.append(f"{alias}.{col}")
                            used_columns.add(col)
                    else:
                        select_parts.append(f"{alias}.{col} AS {day_type}_{col}")
            
            select_clause = ", ".join(select_parts)
            
            # FROM과 JOIN 절 구성
            from_clause = f"FROM {day_types[0]} d1"
            join_clause = ""
            
            for i, day_type in enumerate(day_types[1:], 2):
                join_clause += f" INNER JOIN {day_type} d{i} ON d1.date = d{i}.date AND d1.stock_code = d{i}.stock_code"
            
            query = f"""
            SELECT {select_clause}, a.is_answer
            {from_clause}
            {join_clause}
            INNER JOIN answer_v3 a ON CAST(REPLACE(d1.date, '-', '') AS INTEGER) = a.date 
                               AND d1.stock_code = a.stock_code
            WHERE a.date < 20230601
            ORDER BY d1.date, d1.stock_code
            """
        
        # pandas로 먼저 읽고 polars로 변환
        df_pandas = pd.read_sql_query(query, conn)
        conn.close()
        
        # 컬럼명을 문자열로 확실히 변환
        df_pandas.columns = df_pandas.columns.astype(str)
        
        # 데이터 타입 정리 (polars 호환성을 위해)
        for col in df_pandas.columns:
            col_series = df_pandas[col]
            if isinstance(col_series, pd.DataFrame):
                col_series = col_series.iloc[:, 0]
            if hasattr(col_series, 'dtype'):
                if str(col_series.dtype) in ['Int64', 'int64']:
                    df_pandas[col] = col_series.astype('int64')
                elif str(col_series.dtype) in ['Float64', 'float64']:
                    df_pandas[col] = col_series.astype('float64')
                elif str(col_series.dtype) in ['boolean', 'bool']:
                    df_pandas[col] = col_series.astype('bool')
        
        # polars DataFrame으로 변환
        df = pl.from_pandas(df_pandas)
        
        # 데이터 분할
        total_rows = len(df)
        split_point = int(total_rows * split_ratio)
        
        if use_train_data:
            df = df.head(split_point)
            data_type = "학습용"
        else:
            df = df.tail(total_rows - split_point)
            data_type = "평가용"
        
        print(f"{data_type} 데이터 로드 완료: {len(df)} 행, {len(df.columns)} 컬럼")
        if 'is_answer' in df.columns:
            print(f"is_answer 컬럼 분포: {df['is_answer'].value_counts().to_dict()}")
        
        return df
        
    except Exception as e:
        print(f"데이터 로드 중 오류 발생: {e}")
        import traceback
        traceback.print_exc()
        return None

def filter_features_by_config(df, config):
    """
    실험 설정에 따라 특성을 필터링합니다.
    """
    if config['feature_selection']['use_all_features']:
        return df
    
    # 특정 특성만 선택
    selected_features = config['feature_selection']['selected_features']
    
    # 선택된 특성들이 실제로 존재하는지 확인
    available_features = []
    missing_features = []
    
    for feature in selected_features:
        if feature in df.columns:
            available_features.append(feature)
        else:
            missing_features.append(feature)
    
    if missing_features:
        print(f"경고: 다음 특성들이 데이터에 없습니다: {missing_features}")
    
    if not available_features:
        print("오류: 선택된 특성 중 사용 가능한 특성이 없습니다.")
        return df
    
    # 선택된 특성과 필수 컬럼들만 포함
    required_columns = ['date', 'stock_code', 'is_answer'] + available_features
    filtered_df = df.select(required_columns)
    
    print(f"특성 필터링 완료: {len(available_features)}개 특성 사용")
    print(f"사용된 특성: {available_features}")
    
    return filtered_df

def prepare_data_for_model(df, config):
    """
    모델 학습을 위해 데이터를 준비합니다.
    """
    # 특성 필터링
    df = filter_features_by_config(df, config)
    
    # date와 stock_code는 제외하고 수치형 컬럼만 선택
    numeric_columns = []
    for col in df.columns:
        if col not in ['date', 'stock_code', 'is_answer']:
            if df[col].dtype in [pl.Float64, pl.Float32, pl.Int64, pl.Int32]:
                numeric_columns.append(col)
    
    # is_answer 컬럼 추가
    selected_columns = numeric_columns + ['is_answer']
    
    # NaN 값 처리
    df_clean = df.select(selected_columns).drop_nulls()
    
    print(f"정리된 데이터: {len(df_clean)} 행, {len(df_clean.columns)} 컬럼")
    
    # polars DataFrame을 pandas로 변환 (sklearn 호환성을 위해)
    df_pandas = df_clean.to_pandas()
    
    # 특성과 타겟 분리
    X = df_pandas.drop('is_answer', axis=1)
    y = df_pandas['is_answer']
    
    print(f"특성 수: {X.shape[1]}, 타겟 분포: {y.value_counts().to_dict()}")
    
    return X, y

def optimize_hyperparameters(X_train, y_train, config):
    """
    하이퍼파라미터 최적화를 수행합니다.
    """
    print(f"하이퍼파라미터 최적화 시작...")
    print(f"최적화 기준: {config['hyperparameter_optimization']['scoring']}")
    print(f"교차 검증 폴드: {config['hyperparameter_optimization']['cv_folds']}")
    print(f"반복 횟수: {config['hyperparameter_optimization']['n_iter']}")
    
    # 기본 랜덤포레스트 모델
    base_rf = RandomForestClassifier(
        random_state=config['random_state'],
        n_jobs=-1,
        class_weight=config['class_weight']
    )
    
    # 랜덤 서치 설정
    random_search = RandomizedSearchCV(
        estimator=base_rf,
        param_distributions=config['hyperparameter_optimization']['param_distributions'],
        n_iter=config['hyperparameter_optimization']['n_iter'],
        scoring=config['hyperparameter_optimization']['scoring'],
        cv=config['hyperparameter_optimization']['cv_folds'],
        random_state=config['random_state'],
        n_jobs=-1,
        verbose=1
    )
    
    # 최적화 실행
    random_search.fit(X_train, y_train)
    
    print(f"최적화 완료!")
    print(f"최적 하이퍼파라미터: {random_search.best_params_}")
    print(f"최적 점수: {random_search.best_score_:.4f}")
    
    return random_search.best_estimator_, random_search.best_params_, random_search.best_score_

def train_random_forest(X, y, config):
    """
    랜덤포레스트 모델을 학습합니다.
    """
    # 데이터 분할
    X_train, X_test, y_train, y_test = train_test_split(
        X, y, test_size=config['test_size'], random_state=config['random_state'], stratify=y
    )
    
    print(f"훈련 데이터: {X_train.shape[0]} 행, 테스트 데이터: {X_test.shape[0]} 행")
    print(f"훈련 데이터 클래스 분포: {pd.Series(y_train).value_counts().to_dict()}")
    
    # SMOTE 적용 여부
    if config['use_smote']:
        print("SMOTE를 적용하여 클래스 불균형을 해결합니다...")
        smote = SMOTE(random_state=config['random_state'], k_neighbors=5)
        X_train_resampled, y_train_resampled = smote.fit_resample(X_train, y_train)
        print(f"SMOTE 적용 후 훈련 데이터: {X_train_resampled.shape[0]} 행")
        print(f"SMOTE 적용 후 클래스 분포: {pd.Series(y_train_resampled).value_counts().to_dict()}")
    else:
        X_train_resampled, y_train_resampled = X_train, y_train
        print("SMOTE를 적용하지 않습니다.")
    
    # 하이퍼파라미터 최적화 여부 확인
    if config['hyperparameter_optimization']['enabled']:
        print("하이퍼파라미터 최적화를 수행합니다...")
        rf_model, best_params, best_score = optimize_hyperparameters(X_train_resampled, y_train_resampled, config)
        print(f"최적화된 모델 학습 완료 (최적 점수: {best_score:.4f})")
    else:
        print("기본 하이퍼파라미터로 모델을 학습합니다...")
        # 랜덤포레스트 모델 생성 및 학습
        rf_model = RandomForestClassifier(
            n_estimators=config['n_estimators'],
            max_depth=config['max_depth'],
            min_samples_split=config['min_samples_split'],
            min_samples_leaf=config['min_samples_leaf'],
            random_state=config['random_state'],
            n_jobs=-1,
            class_weight=config['class_weight']
        )
        
        print("랜덤포레스트 모델 학습 중...")
        rf_model.fit(X_train_resampled, y_train_resampled)
    
    return rf_model, X_test, y_test

def evaluate_model(model, X_test, y_test):
    """
    모델의 성능을 평가합니다.
    """
    # 예측 수행
    y_pred = model.predict(X_test)
    y_pred_proba = model.predict_proba(X_test)
    
    # 성능 지표 계산
    accuracy = accuracy_score(y_test, y_pred)
    precision = precision_score(y_test, y_pred, zero_division=0)
    recall = recall_score(y_test, y_pred, zero_division=0)
    f1 = f1_score(y_test, y_pred, zero_division=0)
    
    # 혼동 행렬
    cm = confusion_matrix(y_test, y_pred)
    
    # 상세 분류 리포트
    report = classification_report(y_test, y_pred, output_dict=True)
    
    # 교차 검증
    cv_scores = cross_val_score(model, X_test, y_test, cv=5, scoring='accuracy')
    
    metrics = {
        'accuracy': accuracy,
        'precision': precision,
        'recall': recall,
        'f1_score': f1,
        'confusion_matrix': cm.tolist(),
        'classification_report': report,
        'cv_mean': cv_scores.mean(),
        'cv_std': cv_scores.std(),
        'total_samples': len(y_test),
        'positive_samples': sum(y_test == 1),
        'negative_samples': sum(y_test == 0)
    }
    
    return metrics

def print_evaluation_results(metrics, model_name, eval_type="기본"):
    """
    평가 결과를 간략하게 출력합니다.
    """
    print(f"\n{'='*50}")
    print(f"[{model_name}] {eval_type} 평가")
    print(f"{'='*50}")
    
    print(f"📊 성능 지표:")
    print(f"   정확도: {metrics['accuracy']:.4f} | 정밀도: {metrics['precision']:.4f} | 재현율: {metrics['recall']:.4f} | F1: {metrics['f1_score']:.4f}")
    
    print(f"📈 데이터 현황:")
    print(f"   총 샘플: {metrics['total_samples']:,} | 급등: {metrics['positive_samples']:,} | 비급등: {metrics['negative_samples']:,} | 급등비율: {metrics['positive_samples']/metrics['total_samples']*100:.1f}%")
    
    # 혼동 행렬 간략화
    cm = metrics['confusion_matrix']
    print(f"🎯 혼동행렬: TN={cm[0][0]:,} | FP={cm[0][1]:,} | FN={cm[1][0]:,} | TP={cm[1][1]:,}")

def predict_and_evaluate_on_test_data(model, day_types, config):
    """
    테스트 데이터에서 예측 및 평가를 수행합니다.
    """
    # 테스트 데이터 로드
    df_test = load_data_from_db(day_types, config['db_path'], config['split_ratio'], use_train_data=False)
    if df_test is None:
        return None
    
    # 예측용 데이터 준비
    X_test_full, y_test_full = prepare_data_for_model(df_test, config)
    
    # 예측 수행
    y_pred = model.predict(X_test_full)
    y_pred_proba = model.predict_proba(X_test_full)
    
    # 성능 지표 계산
    metrics = evaluate_model(model, X_test_full, y_test_full)
    
    return metrics, y_pred, y_pred_proba, df_test

def predict_top_per_day(model, day_types, config):
    """
    하루 기준으로 가장 확률이 높은 종목만 선택하여 평가합니다.
    """
    # 테스트 데이터 로드
    df_test = load_data_from_db(day_types, config['db_path'], config['split_ratio'], use_train_data=False)
    if df_test is None:
        return None
    
    # 예측용 데이터 준비
    X_test_full, y_test_full = prepare_data_for_model(df_test, config)
    
    # 예측 수행
    y_pred = model.predict(X_test_full)
    y_pred_proba = model.predict_proba(X_test_full)
    
    # 결과 데이터프레임 생성
    result_df = df_test.select(['date', 'stock_code']).to_pandas()
    result_df['predicted_class'] = y_pred
    result_df['predicted_proba_0'] = y_pred_proba[:, 0]
    result_df['predicted_proba_1'] = y_pred_proba[:, 1]
    result_df['actual_class'] = df_test.select('is_answer').to_pandas()
    
    # 하루 기준으로 급등 확률이 높은 순으로 정렬하고 상위 k개 선택
    result_df = result_df.sort_values(['date', 'predicted_proba_1'], ascending=[True, False])
    
    # 각 날짜별로 상위 k개 선택
    top_stocks = []
    for date in result_df['date'].unique():
        day_data = result_df[result_df['date'] == date]
        top_day_stocks = day_data.head(config['top_k'])
        top_stocks.append(top_day_stocks)
    
    top_result_df = pd.concat(top_stocks, ignore_index=True)
    
    # 성능 지표 계산 (이미 예측된 결과를 사용)
    y_pred_top = top_result_df['predicted_class'].values
    y_actual_top = top_result_df['actual_class'].values
    
    # 성능 지표 계산
    accuracy = accuracy_score(y_actual_top, y_pred_top)
    precision = precision_score(y_actual_top, y_pred_top, zero_division=0)
    recall = recall_score(y_actual_top, y_pred_top, zero_division=0)
    f1 = f1_score(y_actual_top, y_pred_top, zero_division=0)
    
    # 혼동 행렬
    cm = confusion_matrix(y_actual_top, y_pred_top)
    
    # 상세 분류 리포트
    report = classification_report(y_actual_top, y_pred_top, output_dict=True)
    
    metrics = {
        'accuracy': accuracy,
        'precision': precision,
        'recall': recall,
        'f1_score': f1,
        'confusion_matrix': cm.tolist(),
        'classification_report': report,
        'cv_mean': accuracy,  # Top-k의 경우 교차검증 의미 없음
        'cv_std': 0.0,
        'total_samples': len(y_actual_top),
        'positive_samples': sum(y_actual_top == 1),
        'negative_samples': sum(y_actual_top == 0)
    }
    
    return metrics, top_result_df

def print_top_per_day_results(metrics, model_name, top_k, result_df):
    """
    Top-k per day 결과를 간략하게 출력합니다.
    """
    print(f"\n{'='*50}")
    print(f"[{model_name}] Top-{top_k} per Day 평가")
    print(f"{'='*50}")
    
    # 날짜 범위 출력
    start_date = result_df['date'].min()
    end_date = result_df['date'].max()
    print(f"📅 평가 기간: {start_date} ~ {end_date} ({result_df['date'].nunique()}일)")
    print(f"📊 선택 종목: {len(result_df):,}개")
    
    print(f"📈 성능 지표:")
    print(f"   정확도: {metrics['accuracy']:.4f} | 정밀도: {metrics['precision']:.4f} | 재현율: {metrics['recall']:.4f} | F1: {metrics['f1_score']:.4f}")
    
    print(f"📊 데이터 현황:")
    print(f"   총 샘플: {metrics['total_samples']:,} | 급등: {metrics['positive_samples']:,} | 비급등: {metrics['negative_samples']:,} | 급등비율: {metrics['positive_samples']/metrics['total_samples']*100:.1f}%")
    
    # 혼동 행렬 간략화
    cm = metrics['confusion_matrix']
    print(f"🎯 혼동행렬: TN={cm[0][0]:,} | FP={cm[0][1]:,} | FN={cm[1][0]:,} | TP={cm[1][1]:,}")
    
    # 확률 분포 시각화
    plot_probability_distribution(result_df, model_name, top_k)

def perform_shap_analysis(model, X_train, X_test, feature_names, model_name, config):
    """
    SHAP 분석을 수행하고 결과를 시각화합니다.
    """
    print(f"\n🔍 SHAP 분석 시작: {model_name}")
    
    # SHAP Explainer 생성 (TreeExplainer for Random Forest)
    explainer = shap.TreeExplainer(model)
    
    # 테스트 데이터에서 SHAP 값 계산 (샘플링하여 속도 향상)
    sample_size = min(1000, len(X_test))
    X_test_sample = X_test.sample(n=sample_size, random_state=config['random_state'])
    
    print(f"SHAP 값 계산 중... (샘플 크기: {sample_size})")
    shap_values = explainer.shap_values(X_test_sample)
    
    # 결과 저장 디렉토리 생성
    results_dir = Path("results/shap_analysis")
    results_dir.mkdir(parents=True, exist_ok=True)
    
    # 1. 특성 중요도 요약 플롯
    plt.figure(figsize=(12, 8))
    shap.summary_plot(shap_values, X_test_sample, feature_names=feature_names, 
                     show=False, plot_type="bar")
    plt.title(f'{model_name} - SHAP 특성 중요도')
    plt.tight_layout()
    plt.savefig(results_dir / f'{model_name}_shap_summary_bar.png', dpi=300, bbox_inches='tight')
    plt.show()
    
    # 2. SHAP 요약 플롯 (점 플롯)
    plt.figure(figsize=(12, 10))
    shap.summary_plot(shap_values, X_test_sample, feature_names=feature_names, show=False)
    plt.title(f'{model_name} - SHAP 요약 플롯')
    plt.tight_layout()
    plt.savefig(results_dir / f'{model_name}_shap_summary_dots.png', dpi=300, bbox_inches='tight')
    plt.show()
    
    # 3. 상위 10개 특성에 대한 개별 SHAP 플롯
    feature_importance = np.abs(shap_values).mean(0)
    top_features_idx = np.argsort(feature_importance)[-10:]
    top_features = [feature_names[i] for i in top_features_idx]
    
    print(f"상위 10개 중요 특성: {top_features}")
    
    # 4. 의존성 플롯 (상위 5개 특성)
    for i, feature_idx in enumerate(top_features_idx[-5:]):
        feature_name = feature_names[feature_idx]
        plt.figure(figsize=(10, 6))
        shap.dependence_plot(feature_idx, shap_values, X_test_sample, 
                           feature_names=feature_names, show=False)
        plt.title(f'{model_name} - {feature_name} 의존성 플롯')
        plt.tight_layout()
        plt.savefig(results_dir / f'{model_name}_shap_dependence_{feature_name}.png', 
                   dpi=300, bbox_inches='tight')
        plt.show()
    
    # 5. 특성 중요도 데이터프레임 생성 및 저장
    importance_df = pd.DataFrame({
        'feature': feature_names,
        'importance': feature_importance
    }).sort_values('importance', ascending=False)
    
    importance_df.to_csv(results_dir / f'{model_name}_feature_importance.csv', index=False)
    
    # 6. 상위 특성들의 SHAP 값 분포
    plt.figure(figsize=(15, 10))
    for i, feature_idx in enumerate(top_features_idx[-6:]):
        plt.subplot(2, 3, i+1)
        feature_name = feature_names[feature_idx]
        shap.dependence_plot(feature_idx, shap_values, X_test_sample, 
                           feature_names=feature_names, show=False)
        plt.title(f'{feature_name}')
        plt.xlabel('')
        plt.ylabel('')
    
    plt.suptitle(f'{model_name} - 상위 6개 특성 SHAP 의존성')
    plt.tight_layout()
    plt.savefig(results_dir / f'{model_name}_shap_top_features.png', dpi=300, bbox_inches='tight')
    plt.show()
    
    # 7. SHAP 값 통계 정보 출력
    print(f"\n📊 SHAP 분석 결과 요약:")
    print(f"   - 분석된 샘플 수: {sample_size}")
    print(f"   - 특성 수: {len(feature_names)}")
    print(f"   - 상위 5개 중요 특성:")
    for i, (feature, importance) in enumerate(importance_df.head().values):
        print(f"     {i+1}. {feature}: {importance:.4f}")
    
    return {
        'explainer': explainer,
        'shap_values': shap_values,
        'feature_importance': importance_df,
        'top_features': top_features
    }

def plot_probability_distribution(result_df, model_name, top_k):
    """
    선정된 종목들의 급등 확률을 실제 급등/비급등으로 나누어 시각화합니다.
    """
    # 첫 번째 그래프: 확률 분포
    plt.figure(figsize=(15, 10))
    
    # 실제 급등/비급등으로 데이터 분리
    actual_positive = result_df[result_df['actual_class'] == 1]['predicted_proba_1']
    actual_negative = result_df[result_df['actual_class'] == 0]['predicted_proba_1']
    
    # 박스플롯 그리기
    plt.subplot(2, 3, 1)
    data_to_plot = [actual_negative, actual_positive]
    labels = ['실제 비급등', '실제 급등']
    colors = ['lightcoral', 'lightgreen']
    
    bp = plt.boxplot(data_to_plot, labels=labels, patch_artist=True)
    for patch, color in zip(bp['boxes'], colors):
        patch.set_facecolor(color)
    
    plt.title(f'{model_name} Top-{top_k} 급등 확률 분포 (박스플롯)')
    plt.ylabel('급등 확률')
    plt.grid(True, alpha=0.3)
    
    # 히스토그램 그리기 (개수 기준)
    plt.subplot(2, 3, 2)
    plt.hist(actual_negative, bins=20, alpha=0.7, label='실제 비급등', color='lightcoral', density=False)
    plt.hist(actual_positive, bins=20, alpha=0.7, label='실제 급등', color='lightgreen', density=False)
    plt.title(f'{model_name} Top-{top_k} 급등 확률 분포 (히스토그램)')
    plt.xlabel('급등 확률')
    plt.ylabel('개수')
    plt.legend()
    plt.grid(True, alpha=0.3)
    
    # 바이올린 플롯 그리기
    plt.subplot(2, 3, 3)
    from matplotlib.patches import Rectangle
    
    # 바이올린 플롯 데이터 준비
    violin_data = [actual_negative, actual_positive]
    violin_parts = plt.violinplot(violin_data, positions=[1, 2], showmeans=True)
    
    # 바이올린 플롯 색상 설정
    for i, pc in enumerate(violin_parts['bodies']):
        if i == 0:
            pc.set_facecolor('lightcoral')
            pc.set_alpha(0.7)
        else:
            pc.set_facecolor('lightgreen')
            pc.set_alpha(0.7)
    
    plt.xticks([1, 2], ['실제 비급등', '실제 급등'])
    plt.title(f'{model_name} Top-{top_k} 급등 확률 분포 (바이올린 플롯)')
    plt.ylabel('급등 확률')
    plt.grid(True, alpha=0.3)
    
    # PR Curve 그리기
    plt.subplot(2, 3, 4)
    from sklearn.metrics import precision_recall_curve
    
    y_true = result_df['actual_class'].values
    y_scores = result_df['predicted_proba_1'].values
    
    precision, recall, thresholds = precision_recall_curve(y_true, y_scores)
    
    plt.plot(recall, precision, 'b-', linewidth=2, label=f'PR Curve')
    plt.fill_between(recall, precision, alpha=0.3, color='blue')
    
    # 랜덤 분류기 기준선
    no_skill = len(y_true[y_true == 1]) / len(y_true)
    plt.axhline(y=no_skill, color='red', linestyle='--', label=f'Random Classifier ({no_skill:.3f})')
    
    plt.xlabel('재현율 (Recall)')
    plt.ylabel('정밀도 (Precision)')
    plt.title(f'{model_name} Top-{top_k} PR Curve')
    plt.legend()
    plt.grid(True, alpha=0.3)
    
    # Precision vs Threshold 그래프
    plt.subplot(2, 3, 5)
    
    # 임계값별 정밀도 계산
    threshold_range = np.arange(0.1, 1.0, 0.05)
    precision_at_threshold = []
    recall_at_threshold = []
    
    for threshold in threshold_range:
        y_pred_threshold = (y_scores >= threshold).astype(int)
        if sum(y_pred_threshold) > 0:
            precision_at_threshold.append(precision_score(y_true, y_pred_threshold, zero_division=0))
            recall_at_threshold.append(recall_score(y_true, y_pred_threshold, zero_division=0))
        else:
            precision_at_threshold.append(0)
            recall_at_threshold.append(0)
    
    plt.plot(threshold_range, precision_at_threshold, 'g-', linewidth=2, label='정밀도 (Precision)')
    plt.plot(threshold_range, recall_at_threshold, 'r-', linewidth=2, label='재현율 (Recall)')
    plt.xlabel('임계값 (Threshold)')
    plt.ylabel('점수')
    plt.title(f'{model_name} Top-{top_k} Precision/Recall vs Threshold')
    plt.legend()
    plt.grid(True, alpha=0.3)
    
    # 통계 정보 표시
    plt.subplot(2, 3, 6)
    plt.axis('off')
    
    # 통계 정보 계산
    pos_mean = actual_positive.mean()
    pos_std = actual_positive.std()
    pos_median = actual_positive.median()
    neg_mean = actual_negative.mean()
    neg_std = actual_negative.std()
    neg_median = actual_negative.median()
    
    # PR Curve AUC 계산
    from sklearn.metrics import auc
    pr_auc = auc(recall, precision)
    
    stats_text = f"""
    통계 정보:
    
    실제 급등 종목:
    - 평균: {pos_mean:.4f}
    - 표준편차: {pos_std:.4f}
    - 중앙값: {pos_median:.4f}
    - 개수: {len(actual_positive)}
    
    실제 비급등 종목:
    - 평균: {neg_mean:.4f}
    - 표준편차: {neg_std:.4f}
    - 중앙값: {neg_median:.4f}
    - 개수: {len(actual_negative)}
    
    확률 차이:
    - 평균 차이: {pos_mean - neg_mean:.4f}
    - 중앙값 차이: {pos_median - neg_median:.4f}
    
    PR Curve AUC: {pr_auc:.4f}
    """
    
    plt.text(0.1, 0.9, stats_text, transform=plt.gca().transAxes, 
             fontsize=9, verticalalignment='top', fontfamily=plt.rcParams['font.family'],
             bbox=dict(boxstyle="round,pad=0.3", facecolor="lightblue", alpha=0.8))
    
    plt.tight_layout()
    plt.show()
    
    # 추가로 확률 구간별 분석
    print(f"\n{'='*40}")
    print("확률 구간별 분석")
    print(f"{'='*40}")
    
    # 확률 구간 설정
    prob_ranges = [(0.0, 0.2), (0.2, 0.4), (0.4, 0.6), (0.6, 0.8), (0.8, 1.0)]
    
    for low, high in prob_ranges:
        range_data = result_df[(result_df['predicted_proba_1'] >= low) & (result_df['predicted_proba_1'] < high)]
        if len(range_data) > 0:
            pos_count = sum(range_data['actual_class'] == 1)
            neg_count = sum(range_data['actual_class'] == 0)
            total_count = len(range_data)
            pos_rate = pos_count / total_count * 100 if total_count > 0 else 0
            
            print(f"확률 {low:.1f}-{high:.1f}: {total_count}개 종목")
            print(f"  - 실제 급등: {pos_count}개 ({pos_rate:.1f}%)")
            print(f"  - 실제 비급등: {neg_count}개 ({100-pos_rate:.1f}%)")
    
    # 최고 확률 구간 분석
    high_prob_data = result_df[result_df['predicted_proba_1'] >= 0.8]
    if len(high_prob_data) > 0:
        print(f"\n높은 확률 (≥0.8) 종목 분석:")
        print(f"  - 총 개수: {len(high_prob_data)}")
        print(f"  - 실제 급등: {sum(high_prob_data['actual_class'] == 1)}개")
        print(f"  - 실제 비급등: {sum(high_prob_data['actual_class'] == 0)}개")
        print(f"  - 급등 비율: {sum(high_prob_data['actual_class'] == 1)/len(high_prob_data)*100:.1f}%")

def run_single_experiment(day_types, config):
    """
    단일 실험을 실행합니다.
    """
    if isinstance(day_types, str):
        day_types = [day_types]
    
    model_name = "_".join(day_types)
    print(f"\n{'='*60}")
    print(f"🚀 {model_name.upper()} 실험 시작")
    print(f"{'='*60}")
    
    # 1. 데이터 로드 및 모델 학습
    print("\n📥 1. 데이터 로드 및 모델 학습")
    df_train = load_data_from_db(day_types, config['db_path'], config['split_ratio'], use_train_data=True)
    if df_train is None:
        print(f"❌ {model_name} 데이터 로드 실패")
        return None
    
    X_train, y_train = prepare_data_for_model(df_train, config)
    model, X_test, y_test = train_random_forest(X_train, y_train, config)
    
    # 하이퍼파라미터 최적화 결과 저장
    optimization_info = None
    if config['hyperparameter_optimization']['enabled']:
        # 최적화된 모델의 하이퍼파라미터 정보 저장
        optimization_info = {
            'scoring': config['hyperparameter_optimization']['scoring'],
            'cv_folds': config['hyperparameter_optimization']['cv_folds'],
            'n_iter': config['hyperparameter_optimization']['n_iter'],
            'best_params': model.get_params()
        }
    
    # 2. 기본 평가 (훈련/테스트 분할)
    print("\n📊 2. 기본 평가 (훈련/테스트 분할)")
    basic_metrics = evaluate_model(model, X_test, y_test)
    print_evaluation_results(basic_metrics, model_name, "기본")
    
    # 3. 전체 테스트 데이터에서 평가
    print("\n🔍 3. 전체 테스트 데이터에서 평가")
    test_metrics, y_pred, y_pred_proba, df_test = predict_and_evaluate_on_test_data(model, day_types, config)
    if test_metrics:
        print_evaluation_results(test_metrics, model_name, "전체 테스트")
    
    # 4. Top-k per day 평가
    print(f"\n🎯 4. Top-{config['top_k']} per day 평가")
    top_metrics, top_result_df = predict_top_per_day(model, day_types, config)
    if top_metrics:
        print_top_per_day_results(top_metrics, model_name, config['top_k'], top_result_df)
    
    # 5. SHAP 분석
    print(f"\n🔍 5. SHAP 분석")
    shap_results = perform_shap_analysis(model, X_train, X_test, list(X_train.columns), model_name, config)
    
    return {
        'model_name': model_name,
        'basic_metrics': basic_metrics,
        'test_metrics': test_metrics,
        'top_metrics': top_metrics,
        'shap_results': shap_results,
        'model': model,
        'feature_names': list(X_train.columns),
        'optimization_info': optimization_info
    }

def main():
    """
    메인 함수: 설정된 모든 실험을 실행합니다.
    """
    print("="*80)
    print("통합 실험 시작")
    print("="*80)
    print(f"실험 설정:")
    print(f"  - 데이터베이스: {EXPERIMENT_CONFIG['db_path']}")
    print(f"  - SMOTE 사용: {EXPERIMENT_CONFIG['use_smote']}")
    print(f"  - Top-k: {EXPERIMENT_CONFIG['top_k']}")
    print(f"  - 하이퍼파라미터 최적화: {EXPERIMENT_CONFIG['hyperparameter_optimization']['enabled']}")
    if EXPERIMENT_CONFIG['hyperparameter_optimization']['enabled']:
        print(f"    - 최적화 기준: {EXPERIMENT_CONFIG['hyperparameter_optimization']['scoring']}")
        print(f"    - 교차 검증 폴드: {EXPERIMENT_CONFIG['hyperparameter_optimization']['cv_folds']}")
        print(f"    - 반복 횟수: {EXPERIMENT_CONFIG['hyperparameter_optimization']['n_iter']}")
    print(f"  - 실험 조합: {list(EXPERIMENT_CONFIG['experiments'].keys())}")
    
    # 실험할 모델 조합들
    model_combinations = {
        'day1': ['day1'],
        'day2': ['day2'],
        'day3': ['day3'],
        'day4': ['day4'],
        'day1_day2_day3': ['day1', 'day2', 'day3'],
        'day1_day2_day3_day4': ['day1', 'day2', 'day3', 'day4']
    }
    
    all_results = {}
    
    # 각 실험 조합에 대해 실행
    for exp_name, enabled in EXPERIMENT_CONFIG['experiments'].items():
        if enabled and exp_name in model_combinations:
            try:
                result = run_single_experiment(model_combinations[exp_name], EXPERIMENT_CONFIG)
                if result is not None:
                    all_results[exp_name] = result
            except Exception as e:
                print(f"{exp_name} 실험 중 오류 발생: {e}")
                import traceback
                traceback.print_exc()
    
    # 전체 요약
    print(f"\n{'='*80}")
    print("🎯 전체 실험 결과 요약")
    print(f"{'='*80}")
    
    for exp_name, result in all_results.items():
        print(f"\n📊 {exp_name.upper()} 모델:")
        
        # 하이퍼파라미터 최적화 정보
        if result['optimization_info']:
            print(f"   🔧 최적화: {result['optimization_info']['scoring']} 기준")
        
        # 성능 지표 요약
        metrics_summary = []
        if result['basic_metrics']:
            metrics_summary.append(f"기본 F1: {result['basic_metrics']['f1_score']:.3f}")
        if result['test_metrics']:
            metrics_summary.append(f"테스트 F1: {result['test_metrics']['f1_score']:.3f}")
        if result['top_metrics']:
            metrics_summary.append(f"Top-{EXPERIMENT_CONFIG['top_k']} F1: {result['top_metrics']['f1_score']:.3f}")
        
        print(f"   📈 성능: {' | '.join(metrics_summary)}")
        print(f"   🔍 특성: {len(result['feature_names'])}개")
        
        # SHAP 분석 결과 요약
        if result['shap_results']:
            top_features = result['shap_results']['top_features'][:3]  # 상위 3개만 표시
            print(f"   🎯 SHAP 상위 특성: {', '.join(top_features)}")

if __name__ == "__main__":
    main() 