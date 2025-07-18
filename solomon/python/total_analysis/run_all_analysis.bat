@echo off
chcp 65001 >nul
REM Total Analysis 실행 스크립트
REM Windows 배치 파일에서 실행

echo === Solomon Total Analysis 시작 ===

REM 현재 디렉토리를 스크립트 위치로 변경
cd /d "%~dp0"

REM 1. 상관관계 분석
echo.
echo 1. 상관관계 분석 시작...
python correlation_analysis_total.py --db_path "D:/db/solomon.db" --output_dir "results/correlation"
if errorlevel 1 (
    echo 상관관계 분석 중 오류 발생!
    pause
    exit /b 1
)
echo 상관관계 분석 완료

REM 2. 모델 학습 (SMOTE 없이)
echo.
echo 2. 모델 학습 시작 (SMOTE 없이)...
python train_models_total.py --db_path "D:/db/solomon.db" --output_dir "results/models"
if errorlevel 1 (
    echo 모델 학습 중 오류 발생!
    pause
    exit /b 1
)
echo 모델 학습 완료 (SMOTE 없이)

REM 3. 모델 학습 (SMOTE 사용)
echo.
echo 3. 모델 학습 시작 (SMOTE 사용)...
python train_models_total.py --db_path "D:/db/solomon.db" --output_dir "results/models_smote" --use_smote
if errorlevel 1 (
    echo 모델 학습 중 오류 발생!
    pause
    exit /b 1
)
echo 모델 학습 완료 (SMOTE 사용)

REM 4. 예측 및 평가 (SMOTE 없이 학습된 모델)
echo.
echo 4. 예측 및 평가 시작 (SMOTE 없이 학습된 모델)...
python predict_and_evaluate_total.py --db_path "D:/db/solomon.db" --models_dir "results/models" --output_dir "results/predictions"
if errorlevel 1 (
    echo 예측 및 평가 중 오류 발생!
    pause
    exit /b 1
)
echo 예측 및 평가 완료 (SMOTE 없이 학습된 모델)

REM 5. 예측 및 평가 (SMOTE 사용하여 학습된 모델)
echo.
echo 5. 예측 및 평가 시작 (SMOTE 사용하여 학습된 모델)...
python predict_and_evaluate_total.py --db_path "D:/db/solomon.db" --models_dir "results/models_smote" --output_dir "results/predictions_smote"
if errorlevel 1 (
    echo 예측 및 평가 중 오류 발생!
    pause
    exit /b 1
)
echo 예측 및 평가 완료 (SMOTE 사용하여 학습된 모델)

REM 6. SHAP 값 분석 (day1_day2_day3_day4 모델)
echo.
echo 6. SHAP 값 분석 시작 (day1_day2_day3_day4 모델)...
python shap_analysis_total.py --db_path "D:/db/solomon.db" --model_path "results/models/day1_day2_day3_day4_model.joblib" --output_dir "results/shap_analysis" --sample_size 5000
if errorlevel 1 (
    echo SHAP 값 분석 중 오류 발생!
    pause
    exit /b 1
)
echo SHAP 값 분석 완료

echo.
echo === 모든 분석 완료! ===
echo 결과는 다음 디렉토리에 저장되었습니다:
echo   - 상관관계 분석: results/correlation/
echo   - 모델 학습 (SMOTE 없이): results/models/
echo   - 모델 학습 (SMOTE 사용): results/models_smote/
echo   - 예측 결과 (SMOTE 없이): results/predictions/
echo   - 예측 결과 (SMOTE 사용): results/predictions_smote/
echo   - SHAP 값 분석: results/shap_analysis/

pause 