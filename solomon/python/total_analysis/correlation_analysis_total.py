import sqlite3
import polars as pl
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns
from pathlib import Path
import argparse

# 한글 폰트 설정
plt.rcParams['font.family'] = 'Malgun Gothic'  # Windows 기본 한글 폰트
plt.rcParams['axes.unicode_minus'] = False     # 마이너스 기호 깨짐 방지

def load_data_from_db(day_type, db_path='D:/db/solomon.db'):
    """
    SQLite 데이터베이스에서 지정된 day 테이블과 answer 테이블을 조인하여 데이터를 로드합니다.
    """
    try:
        conn = sqlite3.connect(db_path)
        
        # day 테이블과 answer 테이블을 조인하여 데이터 가져오기 (20230601 이전 데이터만)
        query = f"""
                SELECT d.*, a.is_answer
        FROM {day_type} d
        INNER JOIN answer_v3 a ON CAST(REPLACE(d.date, '-', '') AS INTEGER) = a.date
                           AND d.stock_code = a.stock_code
        WHERE a.date < 20230601
        """
        
        # pandas로 먼저 읽고 polars로 변환
        df_pandas = pd.read_sql_query(query, conn)
        conn.close()
        
        print(f"{day_type} 원본 컬럼: {list(df_pandas.columns)}")
        
        # 컬럼명을 문자열로 확실히 변환
        df_pandas.columns = df_pandas.columns.astype(str)
        
        # 데이터 타입 정리 (polars 호환성을 위해)
        for col in df_pandas.columns:
            if df_pandas[col].dtype == 'Int64':
                df_pandas[col] = df_pandas[col].astype('int64')
            elif df_pandas[col].dtype == 'Float64':
                df_pandas[col] = df_pandas[col].astype('float64')
            elif df_pandas[col].dtype == 'boolean':
                df_pandas[col] = df_pandas[col].astype('bool')
        
        print(f"데이터 타입 정리 완료")
        
        # polars DataFrame으로 변환
        df = pl.from_pandas(df_pandas)
        
        print(f"{day_type} 데이터 로드 완료: {len(df)} 행, {len(df.columns)} 컬럼")
        if 'is_answer' in df.columns:
            print(f"is_answer 컬럼 분포: {df['is_answer'].value_counts().to_dict()}")
        
        return df
        
    except Exception as e:
        print(f"{day_type} 데이터 로드 중 오류 발생: {e}")
        import traceback
        traceback.print_exc()
        return None

def prepare_data_for_correlation(df):
    """
    상관관계 분석을 위해 데이터를 준비합니다.
    """
    # date와 stock_code는 제외하고 수치형 컬럼만 선택
    numeric_columns = []
    for col in df.columns:
        if col not in ['date', 'stock_code']:
            # polars에서 컬럼 타입 확인
            if df[col].dtype in [pl.Float64, pl.Float32, pl.Int64, pl.Int32]:
                numeric_columns.append(col)
    
    # NaN 값 처리
    df_clean = df.select(numeric_columns).drop_nulls()
    
    # polars DataFrame을 pandas로 변환
    df_pandas = df_clean.to_pandas()
    
    print(f"상관관계 분석용 데이터: {len(df_pandas)} 행, {len(df_pandas.columns)} 컬럼")
    
    return df_pandas

def calculate_correlations(df, target_column='is_answer'):
    """
    모든 특성과 타겟 변수 간의 상관관계를 계산합니다.
    """
    if target_column not in df.columns:
        print(f"타겟 컬럼 '{target_column}'이 데이터에 없습니다.")
        return None
    
    # 타겟 변수와의 상관관계 계산
    correlations = df.corr()[target_column].sort_values(ascending=False)
    
    # 타겟 변수 자체 제외
    correlations = correlations.drop(target_column)
    
    return correlations

def plot_correlation_heatmap(df, day_type, output_dir='results/correlation'):
    """
    상관관계 히트맵을 그립니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    plt.figure(figsize=(20, 16))
    
    # 상관관계 행렬 계산
    corr_matrix = df.corr()
    
    # 히트맵 그리기
    mask = np.triu(np.ones_like(corr_matrix, dtype=bool))
    sns.heatmap(corr_matrix, mask=mask, annot=False, cmap='coolwarm', center=0,
                square=True, linewidths=0.5, cbar_kws={"shrink": .8})
    
    plt.title(f'{day_type} 특성 상관관계 히트맵', fontsize=16, pad=20)
    plt.tight_layout()
    
    output_path = f"{output_dir}/{day_type}_correlation_heatmap.png"
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    plt.close()
    
    print(f"상관관계 히트맵이 '{output_path}'에 저장되었습니다.")

def plot_top_correlations(correlations, day_type, top_n=20, output_dir='results/correlation'):
    """
    상위 상관관계를 막대 그래프로 그립니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    plt.figure(figsize=(12, 10))
    
    # 상위 N개와 하위 N개 선택
    top_corr = correlations.head(top_n)
    bottom_corr = correlations.tail(top_n)
    
    # 양쪽을 합쳐서 그리기
    combined_corr = pd.concat([top_corr, bottom_corr])
    
    # 색상 설정 (양의 상관관계는 빨간색, 음의 상관관계는 파란색)
    colors = ['red' if x > 0 else 'blue' for x in combined_corr.values]
    
    # 막대 그래프 그리기
    bars = plt.barh(range(len(combined_corr)), combined_corr.values, color=colors, alpha=0.7)
    
    # y축 레이블 설정
    plt.yticks(range(len(combined_corr)), combined_corr.index, fontsize=10)
    
    # 제목과 레이블
    plt.title(f'{day_type} 특성과 급등 여부 간의 상관관계 (상위/하위 {top_n}개)', fontsize=14, pad=20)
    plt.xlabel('상관계수', fontsize=12)
    plt.ylabel('특성명', fontsize=12)
    
    # 격자 추가
    plt.grid(axis='x', alpha=0.3)
    
    # 0선 추가
    plt.axvline(x=0, color='black', linestyle='-', alpha=0.5)
    
    # 값 표시
    for i, (bar, value) in enumerate(zip(bars, combined_corr.values)):
        plt.text(value + (0.01 if value > 0 else -0.01), i, f'{value:.3f}', 
                va='center', ha='left' if value > 0 else 'right', fontsize=9)
    
    plt.tight_layout()
    
    output_path = f"{output_dir}/{day_type}_top_correlations.png"
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    plt.close()
    
    print(f"상위 상관관계 그래프가 '{output_path}'에 저장되었습니다.")

def save_correlation_results(correlations, day_type, output_dir='results/correlation'):
    """
    상관관계 결과를 CSV 파일로 저장합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    correlations_df = pd.DataFrame({
        'feature': correlations.index,
        'correlation': correlations.values,
        'abs_correlation': abs(correlations.values)
    }).sort_values('abs_correlation', ascending=False)
    
    output_path = f"{output_dir}/{day_type}_correlation_results.csv"
    correlations_df.to_csv(output_path, index=False)
    print(f"상관관계 결과가 '{output_path}'에 저장되었습니다.")
    
    return correlations_df

def analyze_feature_groups(correlations, day_type):
    """
    특성들을 그룹별로 분석합니다.
    """
    print(f"\n=== {day_type} 특성 그룹별 분석 ===")
    
    # 양의 상관관계와 음의 상관관계 분리
    positive_corr = correlations[correlations > 0]
    negative_corr = correlations[correlations < 0]
    
    print(f"양의 상관관계 특성 수: {len(positive_corr)}")
    print(f"음의 상관관계 특성 수: {len(negative_corr)}")
    
    print(f"\n가장 강한 양의 상관관계 (상위 5개):")
    for i, (feature, corr) in enumerate(positive_corr.head(5).items(), 1):
        print(f"{i}. {feature}: {corr:.4f}")
    
    print(f"\n가장 강한 음의 상관관계 (상위 5개):")
    for i, (feature, corr) in enumerate(negative_corr.head(5).items(), 1):
        print(f"{i}. {feature}: {corr:.4f}")

def analyze_day_correlations(day_type, db_path='D:/db/solomon.db'):
    """
    특정 day의 상관관계 분석을 수행합니다.
    """
    print(f"\n{'='*60}")
    print(f"{day_type} 상관관계 분석 시작")
    print(f"{'='*60}")
    
    # 데이터 로드
    df = load_data_from_db(day_type, db_path)
    if df is None:
        print(f"{day_type} 데이터 로드 실패")
        return None
    
    # 상관관계 분석용 데이터 준비
    df_corr = prepare_data_for_correlation(df)
    
    # 상관관계 계산
    correlations = calculate_correlations(df_corr)
    if correlations is None:
        print(f"{day_type} 상관관계 계산 실패")
        return None
    
    # 결과 저장
    correlations_df = save_correlation_results(correlations, day_type)
    
    # 시각화
    plot_correlation_heatmap(df_corr, day_type)
    plot_top_correlations(correlations, day_type)
    
    # 특성 그룹 분석
    analyze_feature_groups(correlations, day_type)
    
    return correlations_df

def main():
    """
    메인 함수: Day1, Day2, Day3, Day4의 상관관계 분석을 수행합니다.
    """
    parser = argparse.ArgumentParser(description='Day1, Day2, Day3, Day4 상관관계 분석')
    parser.add_argument('--db_path', type=str, default='D:/db/solomon.db', 
                       help='SQLite 데이터베이스 경로')
    parser.add_argument('--output_dir', type=str, default='results/correlation',
                       help='결과 저장 디렉토리')
    
    args = parser.parse_args()
    
    # 결과 디렉토리 생성
    Path(args.output_dir).mkdir(parents=True, exist_ok=True)
    
    # 각 day별 상관관계 분석 수행
    day_types = ['day1', 'day2', 'day3', 'day4']
    all_results = {}
    
    for day_type in day_types:
        try:
            result = analyze_day_correlations(day_type, args.db_path)
            if result is not None:
                all_results[day_type] = result
        except Exception as e:
            print(f"{day_type} 분석 중 오류 발생: {e}")
            import traceback
            traceback.print_exc()
    
    # 전체 요약
    print(f"\n{'='*60}")
    print("전체 상관관계 분석 완료")
    print(f"{'='*60}")
    
    for day_type, result in all_results.items():
        print(f"\n{day_type} 분석 결과:")
        print(f"  - 총 특성 수: {len(result)}")
        print(f"  - 최고 상관관계: {result.iloc[0]['feature']} ({result.iloc[0]['correlation']:.4f})")
        print(f"  - 최저 상관관계: {result.iloc[-1]['feature']} ({result.iloc[-1]['correlation']:.4f})")

if __name__ == "__main__":
    main() 