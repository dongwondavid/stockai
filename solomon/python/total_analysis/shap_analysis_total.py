import sqlite3
import polars as pl
import numpy as np
import pandas as pd
from joblib import load
from pathlib import Path
import matplotlib.pyplot as plt
import seaborn as sns
import argparse
import json
import shap
from sklearn.model_selection import train_test_split
import warnings
warnings.filterwarnings('ignore')

# 한글 폰트 설정
plt.rcParams['font.family'] = 'Malgun Gothic'  # Windows 기본 한글 폰트
plt.rcParams['axes.unicode_minus'] = False     # 마이너스 기호 깨짐 방지

def load_trained_model(model_path):
    """
    저장된 랜덤포레스트 모델을 로드합니다.
    """
    try:
        model = load(model_path)
        print(f"모델이 '{model_path}'에서 성공적으로 로드되었습니다.")
        return model
    except Exception as e:
        print(f"모델 로드 중 오류 발생: {e}")
        return None

def load_data_from_db(day_types, db_path='D:/db/solomon.db', split_ratio=0.5, sample_size=None):
    """
    SQLite 데이터베이스에서 지정된 day 테이블들과 answer 테이블을 조인하여 데이터를 로드합니다.
    split_ratio: 뒷 절반 데이터만 사용 (0.5 = 50%)
    sample_size: SHAP 분석을 위한 샘플 크기 (None이면 전체 사용)
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
        
        # 뒷 절반 데이터만 사용
        total_rows = len(df)
        split_point = int(total_rows * split_ratio)
        df = df.tail(total_rows - split_point)
        
        # SHAP 분석을 위한 샘플링
        if sample_size and sample_size < len(df):
            df = df.sample(n=sample_size, seed=42)
            print(f"SHAP 분석을 위해 {sample_size}개 샘플로 다운샘플링")
        
        print(f"데이터 로드 완료: {len(df)} 행, {len(df.columns)} 컬럼")
        if 'is_answer' in df.columns:
            print(f"is_answer 컬럼 분포: {df['is_answer'].value_counts().to_dict()}")
        
        return df
        
    except Exception as e:
        print(f"데이터 로드 중 오류 발생: {e}")
        import traceback
        traceback.print_exc()
        return None

def prepare_data_for_shap(df):
    """
    SHAP 분석을 위해 데이터를 준비합니다.
    """
    # date와 stock_code는 제외하고 수치형 컬럼만 선택
    numeric_columns = []
    for col in df.columns:
        if col not in ['date', 'stock_code', 'is_answer']:
            # polars에서 컬럼 타입 확인
            if df[col].dtype in [pl.Float64, pl.Float32, pl.Int64, pl.Int32]:
                numeric_columns.append(col)
    
    # NaN 값 처리
    df_clean = df.select(numeric_columns).drop_nulls()
    
    # polars DataFrame을 pandas로 변환 (SHAP 호환성을 위해)
    X = df_clean.to_pandas()
    
    # 타겟 변수 준비
    y = None
    if 'is_answer' in df.columns:
        y = df.select('is_answer').drop_nulls().to_pandas()
    
    return X, y

def create_shap_explainer(model, X, sample_size=1000):
    """
    SHAP Explainer를 생성합니다.
    """
    print("SHAP Explainer 생성 중...")
    
    # TreeExplainer 사용 (RandomForest에 적합)
    explainer = shap.TreeExplainer(model)
    
    # 백그라운드 데이터 준비 (전체 데이터에서 샘플링)
    if len(X) > sample_size:
        background_data = X.sample(n=sample_size, random_state=42)
    else:
        background_data = X
    
    print(f"백그라운드 데이터 크기: {len(background_data)}")
    
    return explainer, background_data

def calculate_shap_values(explainer, X, background_data):
    """
    SHAP 값을 계산합니다.
    """
    print("SHAP 값 계산 중...")
    
    # SHAP 값 계산
    shap_values = explainer.shap_values(X, background_data)
    
    # RandomForest의 경우 클래스별 SHAP 값이 반환되므로 급등 클래스(1)의 값 사용
    if isinstance(shap_values, list):
        shap_values = shap_values[1]  # 급등 클래스에 대한 SHAP 값
    
    print(f"SHAP 값 계산 완료: {shap_values.shape}")
    
    # SHAP 값이 3차원인 경우 (샘플, 특성, 클래스) 2차원으로 변환
    if len(shap_values.shape) == 3:
        # 급등 클래스(1)에 대한 SHAP 값만 사용
        shap_values = shap_values[:, :, 1]
        print(f"3차원 SHAP 값을 2차원으로 변환: {shap_values.shape}")
    
    return shap_values

def plot_shap_summary(shap_values, X, output_dir='results/shap_analysis'):
    """
    SHAP 요약 플롯을 생성합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    plt.figure(figsize=(12, 10))
    
    # SHAP 요약 플롯
    shap.summary_plot(shap_values, X, show=False, max_display=30)
    plt.title('SHAP Feature Importance Summary', fontsize=16, pad=20)
    plt.tight_layout()
    
    output_path = f"{output_dir}/shap_summary_plot.png"
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    plt.close()
    
    print(f"SHAP 요약 플롯이 '{output_path}'에 저장되었습니다.")

def plot_shap_bar_plot(shap_values, X, output_dir='results/shap_analysis'):
    """
    SHAP 바 플롯을 생성합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    plt.figure(figsize=(12, 10))
    
    # SHAP 바 플롯 (평균 절댓값 기준)
    shap.summary_plot(shap_values, X, plot_type="bar", show=False, max_display=30)
    plt.title('SHAP Feature Importance (Mean |SHAP|)', fontsize=16, pad=20)
    plt.tight_layout()
    
    output_path = f"{output_dir}/shap_bar_plot.png"
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    plt.close()
    
    print(f"SHAP 바 플롯이 '{output_path}'에 저장되었습니다.")

def plot_shap_waterfall(shap_values, X, sample_indices=[0, 1, 2], output_dir='results/shap_analysis'):
    """
    SHAP 워터폴 플롯을 생성합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    for i, idx in enumerate(sample_indices):
        if idx < len(X):
            plt.figure(figsize=(12, 8))
            
            # SHAP 워터폴 플롯
            shap.waterfall_plot(
                shap.Explanation(
                    values=shap_values[idx],
                    base_values=shap_values[idx].sum(),
                    data=X.iloc[idx],
                    feature_names=X.columns
                ),
                show=False,
                max_display=20
            )
            plt.title(f'SHAP Waterfall Plot - Sample {idx}', fontsize=16, pad=20)
            plt.tight_layout()
            
            output_path = f"{output_dir}/shap_waterfall_sample_{idx}.png"
            plt.savefig(output_path, dpi=300, bbox_inches='tight')
            plt.close()
            
            print(f"SHAP 워터폴 플롯 (샘플 {idx})이 '{output_path}'에 저장되었습니다.")

def plot_shap_dependence_plots(shap_values, X, top_features=10, output_dir='results/shap_analysis'):
    """
    상위 특성들에 대한 SHAP 의존성 플롯을 생성합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    # 평균 절댓값 기준으로 상위 특성 선택
    mean_abs_shap = np.abs(shap_values).mean(axis=0)
    top_indices = np.argsort(mean_abs_shap)[-top_features:]
    top_features_list = [X.columns[i] for i in top_indices]
    
    print(f"상위 {top_features}개 특성: {top_features_list}")
    
    for i, feature in enumerate(top_features_list):
        plt.figure(figsize=(10, 6))
        
        # SHAP 의존성 플롯
        shap.dependence_plot(
            feature, 
            shap_values, 
            X, 
            show=False,
            interaction_index=None
        )
        plt.title(f'SHAP Dependence Plot - {feature}', fontsize=14, pad=20)
        plt.tight_layout()
        
        output_path = f"{output_dir}/shap_dependence_{feature.replace('/', '_').replace(' ', '_')}.png"
        plt.savefig(output_path, dpi=300, bbox_inches='tight')
        plt.close()
        
        print(f"SHAP 의존성 플롯 ({feature})이 '{output_path}'에 저장되었습니다.")

def create_feature_importance_dataframe(shap_values, X):
    """
    SHAP 값을 기반으로 특성 중요도 데이터프레임을 생성합니다.
    """
    # 평균 절댓값 계산
    mean_abs_shap = np.abs(shap_values).mean(axis=0)
    
    # 특성 중요도 데이터프레임 생성
    importance_df = pd.DataFrame({
        'feature': X.columns,
        'mean_abs_shap': mean_abs_shap,
        'mean_shap': shap_values.mean(axis=0),
        'std_shap': shap_values.std(axis=0)
    })
    
    # 중요도 순으로 정렬
    importance_df = importance_df.sort_values('mean_abs_shap', ascending=False)
    
    return importance_df

def save_feature_importance(importance_df, output_dir='results/shap_analysis'):
    """
    특성 중요도를 CSV 파일로 저장합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    output_path = f"{output_dir}/feature_importance_shap.csv"
    importance_df.to_csv(output_path, index=False)
    print(f"특성 중요도가 '{output_path}'에 저장되었습니다.")
    
    # 상위 20개 특성 출력
    print("\n상위 20개 중요 특성:")
    print(importance_df.head(20)[['feature', 'mean_abs_shap']].to_string(index=False))

def plot_top_features_comparison(importance_df, top_n=20, output_dir='results/shap_analysis'):
    """
    상위 특성들의 중요도를 비교하는 플롯을 생성합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    top_features = importance_df.head(top_n)
    
    plt.figure(figsize=(14, 10))
    
    # 상위 특성들의 평균 절댓값 SHAP 플롯
    plt.subplot(2, 1, 1)
    bars = plt.barh(range(len(top_features)), top_features['mean_abs_shap'])
    plt.yticks(range(len(top_features)), top_features['feature'])
    plt.xlabel('Mean |SHAP|')
    plt.title(f'Top {top_n} Features by SHAP Importance (Mean |SHAP|)', fontsize=14)
    plt.gca().invert_yaxis()
    
    # 색상 구분 (양수/음수 평균 SHAP)
    colors = ['red' if x < 0 else 'blue' for x in top_features['mean_shap']]
    for bar, color in zip(bars, colors):
        bar.set_color(color)
    
    # 상위 특성들의 평균 SHAP 플롯
    plt.subplot(2, 1, 2)
    bars = plt.barh(range(len(top_features)), top_features['mean_shap'])
    plt.yticks(range(len(top_features)), top_features['feature'])
    plt.xlabel('Mean SHAP')
    plt.title(f'Top {top_n} Features by SHAP Importance (Mean SHAP)', fontsize=14)
    plt.gca().invert_yaxis()
    
    # 색상 구분 (양수/음수)
    colors = ['red' if x < 0 else 'blue' for x in top_features['mean_shap']]
    for bar, color in zip(bars, colors):
        bar.set_color(color)
    
    plt.tight_layout()
    
    output_path = f"{output_dir}/top_features_comparison.png"
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    plt.close()
    
    print(f"상위 특성 비교 플롯이 '{output_path}'에 저장되었습니다.")

def analyze_feature_interactions(shap_values, X, top_features=10, output_dir='results/shap_analysis'):
    """
    상위 특성들 간의 상호작용을 분석합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    # 평균 절댓값 기준으로 상위 특성 선택
    mean_abs_shap = np.abs(shap_values).mean(axis=0)
    top_indices = np.argsort(mean_abs_shap)[-top_features:]
    top_features_list = [X.columns[i] for i in top_indices]
    
    print(f"\n상위 {top_features}개 특성 간 상호작용 분석:")
    
    # 상호작용 히트맵 데이터 준비
    interaction_matrix = np.zeros((top_features, top_features))
    
    for i, feature1 in enumerate(top_features_list):
        for j, feature2 in enumerate(top_features_list):
            if i != j:
                # SHAP 의존성 플롯에서 상호작용 인덱스 사용
                interaction_values = shap.dependence_plot(
                    feature1, 
                    shap_values, 
                    X, 
                    interaction_index=feature2,
                    show=False
                )
                # 상호작용 강도 계산 (간단한 상관관계 기반)
                correlation = np.corrcoef(X[feature1], X[feature2])[0, 1]
                interaction_matrix[i, j] = abs(correlation)
    
    # 상호작용 히트맵 플롯
    plt.figure(figsize=(12, 10))
    sns.heatmap(
        interaction_matrix, 
        annot=True, 
        cmap='coolwarm', 
        xticklabels=top_features_list,
        yticklabels=top_features_list,
        fmt='.3f'
    )
    plt.title('Feature Interaction Heatmap (Correlation-based)', fontsize=14)
    plt.tight_layout()
    
    output_path = f"{output_dir}/feature_interaction_heatmap.png"
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    plt.close()
    
    print(f"특성 상호작용 히트맵이 '{output_path}'에 저장되었습니다.")

def save_shap_analysis_summary(importance_df, output_dir='results/shap_analysis'):
    """
    SHAP 분석 요약을 JSON 파일로 저장합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    summary = {
        'total_features': len(importance_df),
        'top_10_features': importance_df.head(10)[['feature', 'mean_abs_shap', 'mean_shap']].to_dict('records'),
        'analysis_summary': {
            'highest_importance_feature': importance_df.iloc[0]['feature'],
            'highest_importance_value': float(importance_df.iloc[0]['mean_abs_shap']),
            'lowest_importance_feature': importance_df.iloc[-1]['feature'],
            'lowest_importance_value': float(importance_df.iloc[-1]['mean_abs_shap']),
            'importance_range': float(importance_df['mean_abs_shap'].max() - importance_df['mean_abs_shap'].min())
        }
    }
    
    output_path = f"{output_dir}/shap_analysis_summary.json"
    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump(summary, f, ensure_ascii=False, indent=2)
    
    print(f"SHAP 분석 요약이 '{output_path}'에 저장되었습니다.")

def main():
    """
    메인 함수: day1_day2_day3_day4 모델에 대한 SHAP 분석을 수행합니다.
    """
    parser = argparse.ArgumentParser(description='day1_day2_day3_day4 모델의 SHAP 값 분석')
    parser.add_argument('--db_path', type=str, default='D:/db/solomon.db', 
                       help='SQLite 데이터베이스 경로')
    parser.add_argument('--model_path', type=str, default='results/models/day1_day2_day3_day4_model.joblib',
                       help='학습된 모델 파일 경로')
    parser.add_argument('--output_dir', type=str, default='results/shap_analysis',
                       help='결과 저장 디렉토리')
    parser.add_argument('--sample_size', type=int, default=5000,
                       help='SHAP 분석을 위한 샘플 크기')
    parser.add_argument('--background_sample_size', type=int, default=1000,
                       help='SHAP 백그라운드 데이터 샘플 크기')
    
    args = parser.parse_args()
    
    print(f"{'='*60}")
    print("day1_day2_day3_day4 모델 SHAP 값 분석 시작")
    print(f"{'='*60}")
    
    # 모델 로드
    model = load_trained_model(args.model_path)
    if model is None:
        print("모델 로드 실패")
        return
    
    # 데이터 로드
    day_types = ['day1', 'day2', 'day3', 'day4']
    df = load_data_from_db(day_types, args.db_path, split_ratio=0.5, sample_size=args.sample_size)
    if df is None:
        print("데이터 로드 실패")
        return
    
    # SHAP 분석용 데이터 준비
    X, y = prepare_data_for_shap(df)
    print(f"SHAP 분석용 데이터 준비 완료: {X.shape}")
    
    # SHAP Explainer 생성
    explainer, background_data = create_shap_explainer(model, X, args.background_sample_size)
    
    # SHAP 값 계산
    shap_values = calculate_shap_values(explainer, X, background_data)
    
    # 특성 중요도 데이터프레임 생성
    importance_df = create_feature_importance_dataframe(shap_values, X)
    
    # 결과 저장
    save_feature_importance(importance_df, args.output_dir)
    save_shap_analysis_summary(importance_df, args.output_dir)
    
    # 시각화 생성
    print("\n시각화 생성 중...")
    plot_shap_summary(shap_values, X, args.output_dir)
    plot_shap_bar_plot(shap_values, X, args.output_dir)
    plot_shap_waterfall(shap_values, X, sample_indices=[0, 1, 2], output_dir=args.output_dir)
    plot_shap_dependence_plots(shap_values, X, top_features=10, output_dir=args.output_dir)
    plot_top_features_comparison(importance_df, top_n=20, output_dir=args.output_dir)
    analyze_feature_interactions(shap_values, X, top_features=10, output_dir=args.output_dir)
    
    print(f"\n{'='*60}")
    print("SHAP 값 분석 완료!")
    print(f"{'='*60}")
    print(f"결과가 '{args.output_dir}' 디렉토리에 저장되었습니다.")
    print(f"주요 파일들:")
    print(f"- feature_importance_shap.csv: 특성 중요도 순위")
    print(f"- shap_summary_plot.png: SHAP 요약 플롯")
    print(f"- shap_bar_plot.png: SHAP 바 플롯")
    print(f"- top_features_comparison.png: 상위 특성 비교")
    print(f"- shap_analysis_summary.json: 분석 요약")

if __name__ == "__main__":
    main() 