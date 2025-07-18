import sqlite3
import polars as pl
import numpy as np
import pandas as pd
from joblib import load
from pathlib import Path
from sklearn.metrics import accuracy_score, precision_score, recall_score, f1_score, confusion_matrix, classification_report, roc_curve, auc, precision_recall_curve
import argparse
import matplotlib.pyplot as plt
import seaborn as sns
import json
from datetime import datetime

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

def load_data_from_db(day_types, db_path='D:/db/solomon.db', split_ratio=0.5):
    """
    SQLite 데이터베이스에서 지정된 day 테이블들과 answer 테이블을 조인하여 데이터를 로드합니다.
    split_ratio: 뒷 절반 데이터만 사용 (0.5 = 50%)
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
        
        print(f"데이터 로드 완료: {len(df)} 행 (전체 {total_rows} 중 뒷 {split_ratio*100}%), {len(df.columns)} 컬럼")
        if 'is_answer' in df.columns:
            print(f"is_answer 컬럼 분포: {df['is_answer'].value_counts().to_dict()}")
        
        return df
        
    except Exception as e:
        print(f"데이터 로드 중 오류 발생: {e}")
        import traceback
        traceback.print_exc()
        return None

def prepare_data_for_prediction(df):
    """
    예측을 위해 데이터를 준비합니다.
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
    
    # polars DataFrame을 pandas로 변환 (sklearn 호환성을 위해)
    df_pandas = df_clean.to_pandas()
    
    return df_pandas

def make_predictions(model, X):
    """
    모델을 사용하여 예측을 수행합니다.
    """
    # 예측 수행
    predictions = model.predict(X)
    prediction_probas = model.predict_proba(X)
    
    return predictions, prediction_probas

def select_top_stock_per_day(df_original, predictions, prediction_probas, top_k=1):
    """
    하루 기준으로 가장 확률이 높은 종목만 선택합니다.
    top_k: 하루에 선택할 종목 수 (기본값: 1)
    """
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
    
    print(f"하루 기준 상위 {top_k}개 종목 선택 완료: {len(top_result_df)} 종목")
    print(f"고유 날짜 수: {len(unique_dates)}")
    print(f"분석 기간: {start_date} ~ {end_date}")
    print(f"총 {len(unique_dates)}일간의 데이터 분석")
    
    return top_result_df

def calculate_performance_metrics(result_df):
    """
    예측 성능 지표를 계산합니다.
    """
    if 'actual_class' not in result_df.columns:
        print("실제 정답 데이터가 없어서 성능 지표를 계산할 수 없습니다.")
        return None
    
    # NaN 값 제거
    valid_data = result_df.dropna(subset=['actual_class', 'predicted_class'])
    
    if len(valid_data) == 0:
        print("유효한 데이터가 없어서 성능 지표를 계산할 수 없습니다.")
        return None
    
    y_true = valid_data['actual_class']
    y_pred = valid_data['predicted_class']
    
    # 예측 결과 확인
    unique_predictions = np.unique(y_pred)
    print(f"예측된 클래스: {unique_predictions}")
    print(f"예측 분포: {np.bincount(y_pred)}")
    
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

def print_performance_metrics(metrics, model_name, top_k, start_date=None, end_date=None):
    """
    성능 지표를 출력합니다.
    """
    if metrics is None:
        return
    
    print(f"\n{'='*60}")
    print(f"{model_name} 모델 - 하루 기준 상위 {top_k}개 종목 예측 성능 분석 결과")
    print(f"{'='*60}")
    
    if start_date and end_date:
        print(f"분석 기간: {start_date} ~ {end_date}")
    
    print(f"총 선택된 종목 수: {metrics['total_samples']}")
    print(f"실제 급등 종목 수: {metrics['positive_samples']}")
    print(f"실제 비급등 종목 수: {metrics['negative_samples']}")
    print(f"급등 비율: {metrics['positive_samples']/metrics['total_samples']*100:.2f}%")
    
    print(f"\n정확도 (Accuracy): {metrics['accuracy']:.4f}")
    print(f"정밀도 (Precision): {metrics['precision']:.4f}")
    print(f"재현율 (Recall): {metrics['recall']:.4f}")
    print(f"F1 점수: {metrics['f1_score']:.4f}")
    
    print(f"\n혼동 행렬:")
    print("          예측")
    print("실제    0(비급등)  1(급등)")
    cm = metrics['confusion_matrix']
    print(f"0(비급등)  {cm[0][0]:>8}  {cm[0][1]:>6}")
    print(f"1(급등)    {cm[1][0]:>8}  {cm[1][1]:>6}")

def plot_top_stock_analysis(result_df, model_name, top_k, output_dir='results/top_predictions'):
    """
    상위 종목 선택 결과를 시각화합니다.
    """
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
    
    output_path = f"{output_dir}/{model_name}_top{top_k}_analysis.png"
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    plt.close()
    
    print(f"상위 종목 분석 그래프가 '{output_path}'에 저장되었습니다.")

def save_top_predictions_to_csv(result_df, model_name, top_k, output_dir='results/top_predictions'):
    """
    상위 종목 예측 결과를 CSV 파일로 저장합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    output_path = f"{output_dir}/{model_name}_top{top_k}_predictions.csv"
    result_df.to_csv(output_path, index=False)
    print(f"상위 종목 예측 결과가 '{output_path}'에 저장되었습니다.")

def save_evaluation_results(metrics, model_name, top_k, output_dir='results/top_predictions'):
    """
    평가 결과를 JSON 파일로 저장합니다.
    """
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    
    # top_k 정보 추가
    metrics['top_k'] = top_k
    metrics['model_name'] = model_name
    
    output_path = f"{output_dir}/{model_name}_top{top_k}_evaluation_results.json"
    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump(metrics, f, ensure_ascii=False, indent=2)
    print(f"평가 결과가 '{output_path}'에 저장되었습니다.")

def predict_and_evaluate_top_stocks(day_types, model_path, db_path='D:/db/solomon.db', top_k=1):
    """
    특정 모델을 사용하여 하루 기준 상위 종목을 선택하고 평가합니다.
    """
    if isinstance(day_types, str):
        day_types = [day_types]
    
    model_name = "_".join(day_types)
    print(f"\n{'='*60}")
    print(f"{model_name} 모델 - 하루 기준 상위 {top_k}개 종목 예측 및 평가 시작")
    print(f"{'='*60}")
    
    # 모델 로드
    model = load_trained_model(model_path)
    if model is None:
        print(f"{model_name} 모델 로드 실패")
        return None
    
    # 데이터 로드 (뒷 절반)
    df = load_data_from_db(day_types, db_path, split_ratio=0.5)
    if df is None:
        print(f"{model_name} 데이터 로드 실패")
        return None
    
    # 예측용 데이터 준비
    X = prepare_data_for_prediction(df)
    
    # 예측 수행
    predictions, prediction_probas = make_predictions(model, X)
    
    # 하루 기준 상위 종목 선택
    result_df = select_top_stock_per_day(df, predictions, prediction_probas, top_k)
    
    # 날짜 범위 추출
    unique_dates = sorted(result_df['date'].unique())
    start_date = unique_dates[0]
    end_date = unique_dates[-1]
    
    # 성능 지표 계산
    metrics = calculate_performance_metrics(result_df)
    if metrics is not None:
        print_performance_metrics(metrics, model_name, top_k, start_date, end_date)
        
        # 결과 저장
        save_top_predictions_to_csv(result_df, model_name, top_k)
        save_evaluation_results(metrics, model_name, top_k)
        plot_top_stock_analysis(result_df, model_name, top_k)
    
    return {
        'model_name': model_name,
        'top_k': top_k,
        'result_df': result_df,
        'metrics': metrics,
        'start_date': start_date,
        'end_date': end_date
    }

def main():
    """
    메인 함수: 학습된 모델에 대해 하루 기준 상위 종목 선택 및 평가를 수행합니다.
    """
    parser = argparse.ArgumentParser(description='하루 기준 상위 종목 선택 및 예측 성능 분석')
    parser.add_argument('--db_path', type=str, default='D:/db/solomon.db', 
                       help='SQLite 데이터베이스 경로')
    parser.add_argument('--models_dir', type=str, default='results/models',
                       help='학습된 모델들이 저장된 디렉토리')
    parser.add_argument('--output_dir', type=str, default='results/top_predictions',
                       help='결과 저장 디렉토리')
    parser.add_argument('--top_k', type=int, default=1,
                       help='하루에 선택할 상위 종목 수 (기본값: 1)')
    parser.add_argument('--model_combination', type=str, default='day1_day2_day3_day4',
                       help='분석할 모델 조합 (예: day1_day2_day3_day4)')
    
    args = parser.parse_args()
    
    # 결과 디렉토리 생성
    Path(args.output_dir).mkdir(parents=True, exist_ok=True)
    
    # 모델 조합 파싱
    day_types = args.model_combination.split('_')
    
    # 모델 경로
    model_path = f"{args.models_dir}/{args.model_combination}_model.joblib"
    
    # 예측 및 평가 수행
    result = predict_and_evaluate_top_stocks(day_types, model_path, args.db_path, args.top_k)
    
    if result is not None:
        print(f"\n{'='*60}")
        print(f"{args.model_combination} 모델 - 하루 기준 상위 {args.top_k}개 종목 분석 완료")
        print(f"{'='*60}")
        
        if result['metrics'] is not None:
            metrics = result['metrics']
            print(f"최종 성능 요약:")
            print(f"- 분석 기간: {result['start_date']} ~ {result['end_date']}")
            print(f"- 정확도: {metrics['accuracy']:.4f}")
            print(f"- 정밀도: {metrics['precision']:.4f}")
            print(f"- 재현율: {metrics['recall']:.4f}")
            print(f"- F1 점수: {metrics['f1_score']:.4f}")
            print(f"- 총 선택된 종목 수: {metrics['total_samples']}")
            print(f"- 실제 급등 종목 수: {metrics['positive_samples']}")
            print(f"- 급등 성공률: {metrics['positive_samples']/metrics['total_samples']*100:.2f}%")
    else:
        print(f"{args.model_combination} 모델 분석 실패")

if __name__ == "__main__":
    main() 