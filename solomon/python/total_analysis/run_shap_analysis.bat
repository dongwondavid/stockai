@echo off
chcp 65001 >nul
REM SHAP Analysis 실행 스크립트
REM Windows 배치 파일에서 실행

echo === Solomon SHAP Analysis 시작 ===

REM 현재 디렉토리를 스크립트 위치로 변경
cd /d "%~dp0"

REM SHAP 값 분석 (day1_day2_day3_day4 모델)
echo.
echo SHAP 값 분석 시작 (day1_day2_day3_day4 모델)...
python shap_analysis_total.py --db_path "D:/db/solomon.db" --model_path "results/models/day1_day2_day3_day4_model.joblib" --output_dir "results/shap_analysis" --sample_size 5000
if errorlevel 1 (
    echo SHAP 값 분석 중 오류 발생!
    pause
    exit /b 1
)
echo SHAP 값 분석 완료

echo.
echo === SHAP Analysis 완료! ===
echo 결과는 다음 디렉토리에 저장되었습니다:
echo   - SHAP 값 분석: results/shap_analysis/
echo.
echo 주요 결과 파일들:
echo   - feature_importance_shap.csv: SHAP 기반 특성 중요도
echo   - shap_summary_plot.png: SHAP 요약 플롯
echo   - shap_bar_plot.png: SHAP 바 플롯
echo   - top_features_comparison.png: 상위 특성 비교
echo   - shap_analysis_summary.json: 분석 요약

pause 