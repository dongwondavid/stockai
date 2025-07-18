# Solomon 주식 분석 프로젝트

20230601 이전 데이터를 기반으로 한 주식 급등 예측 분석 시스템입니다.

## 주요 변경사항 (v3)

- **테이블명 변경**: `answer` → `answer_v3`
- **데이터 범위**: 20230601 이전 데이터만 사용
- **조건**: 고점상승률 ≥ 2% AND 저점하락률 ≤ 1%

## 전체 프로젝트 실행

### PowerShell 스크립트 (권장)
```powershell
.\run_complete_analysis.ps1
```

### 배치 파일
```cmd
run_complete_analysis.bat
```

## 개별 실행

### 1. 데이터베이스 생성
```bash
cargo run --bin make_db
```
- `answer_v3` 테이블 생성
- 20230601 이전 데이터만 처리

### 2. Day1 특징 계산
```bash
cargo run --bin day1
```
- 당일 가격 위치, 섹터 정보, 거래량 등 계산

### 3. Day2 특징 계산
```bash
cargo run --bin day2
```
- 전일 대비 변화율, 섹터 순위 등 계산

### 4. Day3 특징 계산
```bash
cargo run --bin day3
```
- 전고점 돌파, 과거 이력, 기술적 지표 등 계산

### 5. Day4 특징 계산
```bash
cargo run --bin day4
```
- 모멘텀, 캔들 패턴, 볼린저 밴드 등 계산

### 6. Python 분석
```bash
cd python/total_analysis
python train_models_total.py
```

### 7. 거래대금 상위 종목 예측 (새로운 기능)
```powershell
.\run_predict_top_stocks.ps1 20241201
```
- 특정 날짜의 거래대금 상위 30개 종목에서 `extra_stocks.txt`에 없는 종목들을 필터링
- `features.txt`의 20개 특징을 계산
- 저장된 Python 랜덤포레스트 모델로 최고 확률 종목 선정

### 8. 최고 모델 분석 및 저장 (새로운 기능)
```powershell
.\run_analyze_best_models.ps1
```
- `complete_ml_pipeline.py`의 결과에서 상위 모델 선택
- 테스트 세트에서 성능 평가
- 하루 기준 분석 수행
- 학습된 모델을 `.joblib` 파일로 저장
- 모델 메타데이터를 JSON 파일로 저장

### 9. 저장된 모델을 사용한 예측 (새로운 기능)
```powershell
.\run_predict_with_saved_model.ps1
```
- 저장된 모델을 로드하여 새로운 데이터에 대해 예측
- 날짜 범위 지정 가능
- 하루 기준 상위 종목 선택
- 예측 결과를 CSV 파일로 저장

### 10. ONNX 모델 변환 및 Rust 통합 (새로운 기능)
```powershell
# ONNX 모델 변환
.\run_onnx_conversion.ps1

# Rust에서 ONNX 모델 사용
cargo run --bin predict_top_stocks 20241201
```
- Python 모델을 ONNX 형식으로 변환
- Rust에서 Python 없이 직접 모델 사용
- 성능 향상 및 단일 바이너리 배포 가능

## 데이터베이스 구조

### answer_v3 테이블
- `date`: 날짜 (YYYYMMDD 형식)
- `stock_code`: 종목 코드
- `rank`: 거래대금 순위
- `total_gain`: 총상승률
- `high_gain`: 고점상승률
- `low_drop`: 저점하락률
- `mdd`: Maximum Drawdown
- `is_answer`: 정답 여부 (1: 급등, 0: 비급등)

### Day 테이블들
- `day1`: 당일 특징 (22개 컬럼)
- `day2`: 전일 대비 특징 (16개 컬럼)
- `day3`: 과거 이력 특징 (14개 컬럼)
- `day4`: 기술적 지표 특징 (27개 컬럼)

## 환경 설정

### 환경 변수
```bash
RUST_LOG=info
SOLOMON_DB_PATH=D:\db\solomon.db
STOCK_DB_PATH=D:\db\stock_price(5min).db
DAILY_DB_PATH=D:\db\stock_price(1day)_with_data.db
ONNX_MODEL_INFO_PATH=models/rust_model_info.json
```

### 필요한 파일들
- `sector_utf8.csv`: 섹터 정보 파일
- `D:\db\stock_price(5min).db`: 5분봉 데이터
- `D:\db\stock_price(1day)_with_data.db`: 일봉 데이터

## Python 분석 결과

분석 결과는 `python/total_analysis/results/` 디렉토리에 저장됩니다:

- `correlation/`: 상관관계 분석 결과
- `models/`: 학습된 모델 및 성능 지표
- `predictions/`: 예측 결과 및 분포
- `summary/`: 종합 분석 리포트

### 최고 모델 분석 결과

`results/best_model_analysis/` 디렉토리에 저장됩니다:

- `models/`: 저장된 모델 파일들 (`.joblib`, `_metadata.json`)
- `*.png`: 하루 기준 분석 시각화 그래프
- `*.json`: 분석 결과 및 성능 지표
- `*.csv`: 예측 결과 데이터

### ONNX 모델 파일

`models/` 디렉토리에 저장됩니다:

- `best_model.onnx`: ONNX 형식 모델 파일
- `onnx_model_metadata.json`: ONNX 모델 메타데이터
- `rust_model_info.json`: Rust 통합용 모델 정보

### 예측 결과

`results/predictions/` 디렉토리에 저장됩니다:

- `prediction_results.csv`: 예측 결과 (날짜, 종목코드, 예측클래스, 확률)

## 성능 최적화

- **병렬 처리**: Day3에서 Rayon을 사용한 멀티스레딩
- **캐싱**: 중복 계산 방지를 위한 메모리 캐시
- **배치 처리**: 데이터베이스 저장 시 배치 단위 처리
- **인덱스 활용**: SQL 쿼리 최적화

## 주의사항

1. **데이터 크기**: 전체 실행 시 상당한 시간이 소요될 수 있습니다
2. **메모리 사용량**: Day3 특징 계산 시 높은 메모리 사용량
3. **디스크 공간**: 결과 파일들이 대용량일 수 있습니다
4. **Python 환경**: 가상환경 사용을 권장합니다

## 문제 해결

### 일반적인 오류
- **데이터베이스 연결 실패**: 경로 확인 및 권한 확인
- **메모리 부족**: Day3 실행 시 시스템 메모리 확인
- **Python 패키지 오류**: `pip install -r requirements.txt` 재실행

### 로그 확인
```bash
# Rust 로그 레벨 설정
set RUST_LOG=debug
cargo run --bin make_db
```

## 라이선스

이 프로젝트는 교육 및 연구 목적으로만 사용되어야 합니다.