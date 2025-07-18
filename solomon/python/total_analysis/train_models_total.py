import sqlite3
import polars as pl
import numpy as np
import pandas as pd
from sklearn.ensemble import RandomForestClassifier
from sklearn.model_selection import train_test_split, GridSearchCV, cross_val_score
from sklearn.preprocessing import StandardScaler
from sklearn.metrics import accuracy_score, precision_score, recall_score, f1_score, confusion_matrix, classification_report
from pathlib import Path
from joblib import dump
import argparse
from imblearn.over_sampling import SMOTE
from imblearn.pipeline import Pipeline as ImbPipeline
import matplotlib.pyplot as plt
import seaborn as sns
import json

# 한글 폰트 설정
plt.rcParams['font.family'] = 'Malgun Gothic'  # Windows 기본 한글 폰트
plt.rcParams['axes.unicode_minus'] = False     # 마이너스 기호 깨짐 방지

def load_data_from_db(day_types, db_path='D:/db/solomon.db', split_ratio=0.5):
    """
    SQLite 데이터베이스에서 지정된 day 테이블들과 answer 테이블을 조인하여 데이터를 로드합니다.
    split_ratio: 앞 절반 데이터만 사용 (0.5 = 50%)
    """
    try:
        conn = sqlite3.connect(db_path)
        
        # 여러 day 테이블을 조인하여 데이터 가져오기
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
            # 먼저 각 테이블의 컬럼 정보를 가져옴
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
        
        print(f"원본 컬럼: {list(df_pandas.columns)}")
        
        # 컬럼명을 문자열로 확실히 변환
        df_pandas.columns = df_pandas.columns.astype(str)
        
        # 데이터 타입 정리 (polars 호환성을 위해)
        for col in df_pandas.columns:
            col_series = df_pandas[col]
            # 혹시라도 DataFrame이 들어오면 첫 번째 컬럼만 사용
            if isinstance(col_series, pd.DataFrame):
                col_series = col_series.iloc[:, 0]
            # dtype 속성이 있는지 확인
            if hasattr(col_series, 'dtype'):
                if str(col_series.dtype) in ['Int64', 'int64']:
                    df_pandas[col] = col_series.astype('int64')
                elif str(col_series.dtype) in ['Float64', 'float64']:
                    df_pandas[col] = col_series.astype('float64')
                elif str(col_series.dtype) in ['boolean', 'bool']:
                    df_pandas[col] = col_series.astype('bool')
        
        print(f"데이터 타입 정리 완료")
        
        # polars DataFrame으로 변환
        df = pl.from_pandas(df_pandas)
        
        # 앞 절반 데이터만 사용
        total_rows = len(df)
        split_point = int(total_rows * split_ratio)
        df = df.head(split_point)
        
        print(f"데이터 로드 완료: {len(df)} 행 (전체 {total_rows} 중 앞 {split_ratio*100}%), {len(df.columns)} 컬럼")
        if 'is_answer' in df.columns:
            print(f"is_answer 컬럼 분포: {df['is_answer'].value_counts().to_dict()}")
        
        return df
        
    except Exception as e:
        print(f"데이터 로드 중 오류 발생: {e}")
        import traceback
        traceback.print_exc()
        return None

def prepare_data_for_model(df):
    """
    모델 학습을 위해 데이터를 준비합니다.
    """
    # date와 stock_code는 제외하고 수치형 컬럼만 선택
    numeric_columns = []
    for col in df.columns:
        if col not in ['date', 'stock_code', 'is_answer']:
            # polars에서 컬럼 타입 확인
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
    print(f"특성 컬럼: {list(X.columns)}")
    
    return X, y

def train_random_forest(X, y, test_size=0.2, random_state=42, use_smote=False):
    """
    랜덤포레스트 모델을 학습합니다.
    """
    # 데이터 분할
    X_train, X_test, y_train, y_test = train_test_split(
        X, y, test_size=test_size, random_state=random_state, stratify=y
    )
    
    print(f"훈련 데이터: {X_train.shape[0]} 행, 테스트 데이터: {X_test.shape[0]} 행")
    print(f"훈련 데이터 클래스 분포: {pd.Series(y_train).value_counts().to_dict()}")
    
    # SMOTE 적용 여부
    if use_smote:
        print("SMOTE를 적용하여 클래스 불균형을 해결합니다...")
        smote = SMOTE(random_state=random_state, k_neighbors=5)
        X_train_resampled, y_train_resampled = smote.fit_resample(X_train, y_train)
        print(f"SMOTE 적용 후 훈련 데이터: {X_train_resampled.shape[0]} 행")
        print(f"SMOTE 적용 후 클래스 분포: {pd.Series(y_train_resampled).value_counts().to_dict()}")
    else:
        X_train_resampled, y_train_resampled = X_train, y_train
        print("SMOTE를 적용하지 않습니다.")
    
    # 랜덤포레스트 모델 생성 및 학습
    rf_model = RandomForestClassifier(
        n_estimators=100,
        max_depth=10,
        min_samples_split=5,
        min_samples_leaf=2,
        random_state=random_state,
        n_jobs=-1,
        class_weight='balanced'  # 클래스 가중치 추가
    )
    
    print("랜덤포레스트 모델 학습 중...")
    rf_model.fit(X_train_resampled, y_train_resampled)
    
    # 모델 예측 확률 확인
    train_proba = rf_model.predict_proba(X_train_resampled)
    print(f"훈련 데이터 예측 확률 분포:")
    print(f"  - 급등 확률 최소값: {train_proba[:, 1].min():.4f}")
    print(f"  - 급등 확률 최대값: {train_proba[:, 1].max():.4f}")
    print(f"  - 급등 확률 평균값: {train_proba[:, 1].mean():.4f}")
    print(f"  - 급등 확률 0.5 이상 비율: {(train_proba[:, 1] >= 0.5).mean():.4f}")
    
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

def print_evaluation_results(metrics, model_name):
    """
    평가 결과를 출력합니다.
    """
    print(f"\n{'='*60}")
    print(f"{model_name} 모델 평가 결과")
    print(f"{'='*60}")
    
    print(f"총 샘플 수: {metrics['total_samples']}")
    print(f"실제 급등 종목 수: {metrics['positive_samples']}")
    print(f"실제 비급등 종목 수: {metrics['negative_samples']}")
    print(f"급등 비율: {metrics['positive_samples']/metrics['total_samples']*100:.2f}%")
    
    print(f"\n정확도 (Accuracy): {metrics['accuracy']:.4f}")
    print(f"정밀도 (Precision): {metrics['precision']:.4f}")
    print(f"재현율 (Recall): {metrics['recall']:.4f}")
    print(f"F1 점수: {metrics['f1_score']:.4f}")
    print(f"교차 검증 정확도: {metrics['cv_mean']:.4f} (+/- {metrics['cv_std']*2:.4f})")
    
    print(f"\n혼동 행렬:")
    print("          예측")
    print("실제    0(비급등)  1(급등)")
    cm = metrics['confusion_matrix']
    print(f"0(비급등)  {cm[0][0]:>8}  {cm[0][1]:>6}")
    print(f"1(급등)    {cm[1][0]:>8}  {cm[1][1]:>6}")

def plot_feature_importance(model, feature_names, model_name, output_dir='results/models'):
    """
    특성 중요도를 시각화합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    # 특성 중요도 추출
    importances = model.feature_importances_
    indices = np.argsort(importances)[::-1]
    
    # 상위 20개 특성만 시각화
    top_n = min(20, len(feature_names))
    
    plt.figure(figsize=(12, 10))
    
    # 상위 특성들의 중요도
    top_features = [feature_names[i] for i in indices[:top_n]]
    top_importances = importances[indices[:top_n]]
    
    # 막대 그래프 그리기
    bars = plt.barh(range(len(top_features)), top_importances, alpha=0.7, color='skyblue')
    
    # y축 레이블 설정
    plt.yticks(range(len(top_features)), top_features, fontsize=10)
    
    # 제목과 레이블
    plt.title(f'{model_name} 특성 중요도 (상위 {top_n}개)', fontsize=14, pad=20)
    plt.xlabel('중요도', fontsize=12)
    plt.ylabel('특성명', fontsize=12)
    
    # 격자 추가
    plt.grid(axis='x', alpha=0.3)
    
    # 값 표시
    for i, (bar, importance) in enumerate(zip(bars, top_importances)):
        plt.text(importance + 0.001, i, f'{importance:.4f}', 
                va='center', ha='left', fontsize=9)
    
    plt.tight_layout()
    
    output_path = f"{output_dir}/{model_name}_feature_importance.png"
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    plt.close()
    
    print(f"특성 중요도 그래프가 '{output_path}'에 저장되었습니다.")

def save_feature_importance(model, feature_names, model_name, output_dir='results/models'):
    """
    특성 중요도를 CSV 파일로 저장합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    # 특성 중요도 추출
    importances = model.feature_importances_
    
    # DataFrame 생성
    importance_df = pd.DataFrame({
        'feature': feature_names,
        'importance': importances
    }).sort_values('importance', ascending=False)
    
    output_path = f"{output_dir}/{model_name}_feature_importance.csv"
    importance_df.to_csv(output_path, index=False)
    print(f"특성 중요도가 '{output_path}'에 저장되었습니다.")
    
    return importance_df

def save_model(model, model_name, output_dir='results/models'):
    """
    모델을 파일로 저장합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    output_path = f"{output_dir}/{model_name}_model.joblib"
    dump(model, output_path)
    print(f"모델이 '{output_path}'에 저장되었습니다.")
    
    return output_path

def save_evaluation_results(metrics, model_name, output_dir='results/models'):
    """
    평가 결과를 JSON 파일로 저장합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    output_path = f"{output_dir}/{model_name}_evaluation_results.json"
    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump(metrics, f, ensure_ascii=False, indent=2)
    print(f"평가 결과가 '{output_path}'에 저장되었습니다.")

def train_and_evaluate_model(day_types, db_path='D:/db/solomon.db', use_smote=False):
    """
    특정 day 조합에 대한 모델을 학습하고 평가합니다.
    """
    if isinstance(day_types, str):
        day_types = [day_types]
    
    model_name = "_".join(day_types)
    print(f"\n{'='*60}")
    print(f"{model_name} 모델 학습 시작")
    print(f"{'='*60}")
    
    # 데이터 로드
    df = load_data_from_db(day_types, db_path)
    if df is None:
        print(f"{model_name} 데이터 로드 실패")
        return None
    
    # 모델 학습용 데이터 준비
    X, y = prepare_data_for_model(df)
    
    # 모델 학습
    model, X_test, y_test = train_random_forest(X, y, use_smote=use_smote)
    
    # 모델 평가
    metrics = evaluate_model(model, X_test, y_test)
    print_evaluation_results(metrics, model_name)
    
    # 결과 저장
    save_model(model, model_name)
    save_evaluation_results(metrics, model_name)
    plot_feature_importance(model, X.columns, model_name)
    feature_importance_df = save_feature_importance(model, X.columns, model_name)
    
    return {
        'model': model,
        'metrics': metrics,
        'feature_importance': feature_importance_df,
        'feature_names': list(X.columns)
    }

def main():
    """
    메인 함수: Day1, Day2, Day3, Day4, Day1+2+3, Day1+2+3+4의 모델을 학습합니다.
    """
    parser = argparse.ArgumentParser(description='Day1, Day2, Day3, Day4, Day1+2+3, Day1+2+3+4 모델 학습')
    parser.add_argument('--db_path', type=str, default='D:/db/solomon.db', 
                       help='SQLite 데이터베이스 경로')
    parser.add_argument('--output_dir', type=str, default='results/models',
                       help='결과 저장 디렉토리')
    parser.add_argument('--use_smote', action='store_true',
                       help='SMOTE를 사용하여 클래스 불균형 해결')
    
    args = parser.parse_args()
    
    # 결과 디렉토리 생성
    Path(args.output_dir).mkdir(parents=True, exist_ok=True)
    
    # 학습할 모델 조합들
    model_combinations = [
        ['day1'],
        ['day2'], 
        ['day3'],
        ['day4'],
        ['day1', 'day2', 'day3'],
        ['day1', 'day2', 'day3', 'day4']
    ]
    
    all_results = {}
    
    # 각 모델 조합에 대해 학습 및 평가 수행
    for day_types in model_combinations:
        try:
            result = train_and_evaluate_model(day_types, args.db_path, args.use_smote)
            if result is not None:
                model_name = "_".join(day_types)
                all_results[model_name] = result
        except Exception as e:
            print(f"{'_'.join(day_types)} 모델 학습 중 오류 발생: {e}")
            import traceback
            traceback.print_exc()
    
    # 전체 요약
    print(f"\n{'='*60}")
    print("전체 모델 학습 완료")
    print(f"{'='*60}")
    
    for model_name, result in all_results.items():
        metrics = result['metrics']
        print(f"\n{model_name} 모델 최종 성능:")
        print(f"  - 정확도: {metrics['accuracy']:.4f}")
        print(f"  - F1 점수: {metrics['f1_score']:.4f}")
        print(f"  - 특성 수: {len(result['feature_names'])}")
        print(f"  - 최고 중요 특성: {result['feature_importance'].iloc[0]['feature']}")

if __name__ == "__main__":
    main() 