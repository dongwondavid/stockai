## 예측모델
- [ ] ONNX 예측 특징·결과 저장 파이프라인 구축 (`model` 테이블)
  - 목적: `onnx_predictor.rs`에서 계산한 특징 [N,M] 행렬과 예측 결과(분류/회귀)를 영속화하여 재현/분석 가능하도록 함
  - 스키마(초안): `model(date TEXT, time TEXT, mode TEXT, model_name TEXT, features_json TEXT, stocks_json TEXT, feature_matrix_json TEXT, class_probs_json TEXT, reg_values_json TEXT, best_stock TEXT, best_score REAL, version INTEGER, notes TEXT)`
  - 저장 규격: 날짜 `YYYYMMDD`, 시간 `HHMM` 고정, N=stocks 순서, M=features 순서 일치. NaN/inf는 0.0으로 정규화 후 저장. JSON 인코딩(UTF-8)
  - 저장 지점: `predict_top_stock` / `predict_top_stock_regression` 수행 직후 1건 기록 (동일 일시·모델 중복 시 에러 반환)

## DB 구조 개선
- [ ] `model` 테이블 인덱스/제약 설계
  - 제약: `(date, time, mode, model_name)` 유니크, `best_score` NOT NULL
  - 인덱스: `date`, `(date, model_name)`, `best_stock`
  - 용량 전략: 초기 JSON 저장, 필요 시 `feature_matrix_blob`(zstd) 컬럼 추가 전환 옵션 설계

## config.example.toml 수정
- [ ] 실행 시 `model` 기록 옵션화
  - 설정 제안: `logging.store_model_records`(default=true). 예시값은 `config.example.toml`에만 추가, 실제 config.toml은 사용자 적용
  - 재현성: 동일 날짜/시간/모드/모델로 1회만 기록되도록 가드

## 성능/검증
- [ ] 다른 프로젝트 db와 비교