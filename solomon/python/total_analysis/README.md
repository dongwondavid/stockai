# Solomon Total Analysis

solomon/python/total_analysis

day1.rs day2.rs day3.rs를 통해 생성한 데이터 solomon.db에서 다음 분석을 진행한다.

## 분석 목표

1. **상관관계 분석**
   
   Day1 특징들과 급등 여부(is_answer) 간의 상관계수 계산 => 결과를 CSV 파일로 저장
   Day2 특징들과 급등 여부(is_answer) 간의 상관계수 계산 => 결과를 CSV 파일로 저장
   Day3 특징들과 급등 여부(is_answer) 간의 상관계수 계산 => 결과를 CSV 파일로 저장

2. **머신러닝 모델 학습 (5년 데이터 중 앞 절반에 대해서)**
   
   RandomForest 모델로 Day1 특징에 대해 급등 여부 예측하도록 학습 => 특징 중요도 결과를 CSV 파일로 저장
   RandomForest 모델로 Day2 특징에 대해 급등 여부 예측하도록 학습 => 특징 중요도 결과를 CSV 파일로 저장
   RandomForest 모델로 Day3 특징에 대해 급등 여부 예측하도록 학습 => 특징 중요도 결과를 CSV 파일로 저장

   RandomForest 모델로 Day1 Day2 Day3 특징에 대해 급등 여부 예측하도록 학습 => 특징 중요도 결과를 CSV 파일로 저장

3. **예측 및 성능 평가 (5년 데이터 중 뒷 절반에 대해서)**
   
   위에서 학습한 Day1으로 예측하는 모델로 뒷 절반에 대해서 예측 => 혼동행렬을 CSV 파일로 저장
   위에서 학습한 Day2으로 예측하는 모델로 뒷 절반에 대해서 예측 => 혼동행렬을 CSV 파일로 저장
   위에서 학습한 Day3으로 예측하는 모델로 뒷 절반에 대해서 예측 => 혼동행렬을 CSV 파일로 저장
   
   위에서 학습한 Day1,2,3으로 예측하는 모델로 뒷 절반에 대해서 예측 => 혼동행렬을 CSV 파일로 저장

4. **SHAP 값 분석 (day1_day2_day3_day4 모델)**
   
   day1_day2_day3_day4 모델의 특성 중요도를 SHAP 값으로 분석 => 진짜 중요 변수 시각화
   SHAP 요약 플롯, 바 플롯, 워터폴 플롯, 의존성 플롯 생성
   특성 간 상호작용 분석 및 시각화

## 파일 구조

```
total_analysis/
├── correlation_analysis_total.py    # Day1, Day2, Day3 상관관계 분석
├── train_models_total.py           # Day1, Day2, Day3, Day1+2+3 모델 학습
├── predict_and_evaluate_total.py   # 학습된 모델들로 예측 및 평가
├── shap_analysis_total.py         # day1_day2_day3_day4 모델 SHAP 값 분석
├── run_all_analysis.ps1           # PowerShell 전체 실행 스크립트
├── run_all_analysis.bat           # Windows 배치 전체 실행 스크립트
├── run_shap_analysis.ps1          # PowerShell SHAP 분석 전용 스크립트
├── run_shap_analysis.bat          # Windows 배치 SHAP 분석 전용 스크립트
├── requirements.txt               # 필요한 Python 패키지
└── README.md                     # 이 파일
```

## 사용법

### 1. 전체 분석 실행 (권장)

#### PowerShell 사용:
```powershell
.\run_all_analysis.ps1
```

#### Windows 배치 파일 사용:
```cmd
run_all_analysis.bat
```

### 2. 개별 분석 실행

#### 상관관계 분석:
```bash
python correlation_analysis_total.py --db_path "D:/db/solomon.db" --output_dir "results/correlation"
```

#### 모델 학습 (SMOTE 없이):
```bash
python train_models_total.py --db_path "D:/db/solomon.db" --output_dir "results/models"
```

#### 모델 학습 (SMOTE 사용):
```bash
python train_models_total.py --db_path "D:/db/solomon.db" --output_dir "results/models_smote" --use_smote
```

#### 예측 및 평가:
```bash
python predict_and_evaluate_total.py --db_path "D:/db/solomon.db" --models_dir "results/models" --output_dir "results/predictions"
```

#### SHAP 값 분석:
```bash
python shap_analysis_total.py --db_path "D:/db/solomon.db" --model_path "results/models/day1_day2_day3_day4_model.joblib" --output_dir "results/shap_analysis" --sample_size 5000
```

### 3. 독립 실행 스크립트

#### SHAP 분석만 실행:
```powershell
# PowerShell
.\run_shap_analysis.ps1
```

```cmd
# Windows 배치 파일
run_shap_analysis.bat
```

## 결과 파일 구조

실행 후 다음과 같은 디렉토리 구조가 생성됩니다:

```
results/
├── correlation/                    # 상관관계 분석 결과
│   ├── day1_correlation_results.csv
│   ├── day1_correlation_heatmap.png
│   ├── day1_top_correlations.png
│   ├── day2_correlation_results.csv
│   ├── day2_correlation_heatmap.png
│   ├── day2_top_correlations.png
│   ├── day3_correlation_results.csv
│   ├── day3_correlation_heatmap.png
│   └── day3_top_correlations.png
├── models/                        # SMOTE 없이 학습된 모델들
│   ├── day1_model.joblib
│   ├── day1_feature_importance.csv
│   ├── day1_feature_importance.png
│   ├── day1_evaluation_results.json
│   ├── day2_model.joblib
│   ├── day2_feature_importance.csv
│   ├── day2_feature_importance.png
│   ├── day2_evaluation_results.json
│   ├── day3_model.joblib
│   ├── day3_feature_importance.csv
│   ├── day3_feature_importance.png
│   ├── day3_evaluation_results.json
│   ├── day1_day2_day3_model.joblib
│   ├── day1_day2_day3_feature_importance.csv
│   ├── day1_day2_day3_feature_importance.png
│   └── day1_day2_day3_evaluation_results.json
├── models_smote/                  # SMOTE 사용하여 학습된 모델들
│   └── [위와 동일한 구조]
├── predictions/                   # SMOTE 없이 학습된 모델들의 예측 결과
│   ├── day1_predictions.csv
│   ├── day1_confusion_matrix.csv
│   ├── day1_evaluation_results.json
│   ├── day1_prediction_distribution.png
│   ├── day2_predictions.csv
│   ├── day2_confusion_matrix.csv
│   ├── day2_evaluation_results.json
│   ├── day2_prediction_distribution.png
│   ├── day3_predictions.csv
│   ├── day3_confusion_matrix.csv
│   ├── day3_evaluation_results.json
│   ├── day3_prediction_distribution.png
│   ├── day1_day2_day3_predictions.csv
│   ├── day1_day2_day3_confusion_matrix.csv
│   ├── day1_day2_day3_evaluation_results.json
│   ├── day1_day2_day3_prediction_distribution.png
│   └── model_performance_comparison.csv
└── predictions_smote/             # SMOTE 사용하여 학습된 모델들의 예측 결과
    └── [위와 동일한 구조]
└── shap_analysis/                 # SHAP 값 분석 결과
    ├── feature_importance_shap.csv
    ├── shap_summary_plot.png
    ├── shap_bar_plot.png
    ├── shap_waterfall_sample_0.png
    ├── shap_waterfall_sample_1.png
    ├── shap_waterfall_sample_2.png
    ├── shap_dependence_[feature_name].png
    ├── top_features_comparison.png
    ├── feature_interaction_heatmap.png
    └── shap_analysis_summary.json
```

## 주요 기능

### 1. 상관관계 분석 (`correlation_analysis_total.py`)
- Day1, Day2, Day3 각각의 특성들과 급등 여부 간의 상관관계 계산
- 상관관계 히트맵 및 상위/하위 상관관계 시각화
- 결과를 CSV 파일로 저장

### 2. 모델 학습 (`train_models_total.py`)
- RandomForest 모델을 사용하여 각 Day 조합별로 모델 학습
- SMOTE 옵션으로 클래스 불균형 해결 가능
- 특성 중요도 분석 및 시각화
- 교차 검증을 통한 모델 성능 평가

### 3. 예측 및 평가 (`predict_and_evaluate_total.py`)
- 학습된 모델들을 사용하여 뒷 절반 데이터에 대해 예측
- 정확도, 정밀도, 재현율, F1 점수 등 성능 지표 계산
- 혼동 행렬 생성 및 저장
- ROC 커브, Precision-Recall 커브 등 시각화
- 모델 간 성능 비교

### 4. SHAP 값 분석 (`shap_analysis_total.py`)
- day1_day2_day3_day4 모델의 특성 중요도를 SHAP 값으로 분석
- SHAP 요약 플롯, 바 플롯, 워터폴 플롯 생성
- 특성별 의존성 플롯으로 특성-예측 관계 분석
- 특성 간 상호작용 히트맵 생성
- 상위 중요 특성들의 비교 시각화
- 진짜 중요 변수 식별 및 시각화

## 요구사항

- Python 3.7+
- SQLite 데이터베이스 (solomon.db)
- 필요한 Python 패키지들 (requirements.txt 참조)

## 설치

```bash
pip install -r requirements.txt
```

## 주의사항

1. 데이터베이스 경로가 올바른지 확인하세요 (기본값: D:/db/solomon.db)
2. 충분한 디스크 공간이 있는지 확인하세요 (결과 파일들이 생성됩니다)
3. 메모리 사용량이 많을 수 있으므로 충분한 RAM을 확보하세요
4. 한글 폰트 설정이 되어 있어야 그래프의 한글이 올바르게 표시됩니다