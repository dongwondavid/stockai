[-] 제목: features_new 기반 45개 특징 이식 및 활성화 계획 (features 디렉토리 정규화)

📌 목적 / 배경
- 새로운 모델에서 사용하는 `features_new/` 구현과 `features_45_summary.txt` 기준 45개 특징을 `stockrs/src/model/onnx_predictor/features/`(이하 features)로 정식 이식하고, 러너/모델에서 해당 목록만 안정적으로 사용할 수 있게 한다.
- 백테스팅은 `config.toml` 없이 `config.example.toml`로 실행 가능하도록 유지한다. 실전/모의 모드에서는 `config.toml` 직접 수정이 불가하므로 필요한 설정 항목을 문서화하여 사용자 적용을 유도한다.

🔧 입력 / 출력
- 입력: `features_new/` 전 파일, `features_new/features_45_summary.txt`, 기존 features 모듈(`stockrs/src/model/onnx_predictor/features/`), `features.txt`(모델 특징 리스트), 일/분봉 DB, `config.example.toml`
- 출력: 
  - `features/` 하위에 동등 기능 구현 파일(컴파일 통합)
  - `features.txt`를 45개 목록으로 동기화(+ 주석 가이드)
  - 모델에서 45개만 로딩하도록 보장된 경로/이름 정합성

✅ 완료 조건
- 빌드 성공(`cargo build` at workspace root) 및 특징 로딩 경로가 `features.txt`와 일치
- 백테스팅에서 45개 특징 전부 계산 시 에러 없이 동작(첫 거래일/데이터 없음 케이스는 명시적 오류 또는 설계된 보수값 반환)
- 실전/모의 모드에서도 오전 5분봉 경로가 정상 동작(`utils::get_morning_data`의 real/paper 분기)
- `LOG.md`에 모든 변경 기록 추가, `TODO.md` 영향 항목 업데이트 필요 시 반영

🧩 관련 모듈
- `stockrs/src/model/onnx_predictor/features/` 전체, `stockrs/src/model/onnx_predictor.rs`, `stockrs/src/utility/config.rs`, `features.txt`, `features_new/`

—

[ ] Phase 0 — 사전 정합성 점검 (사용자 수동 복사 대응 포함)
- **작업 내용**:
  - 사용자가 `features_new/` 파일을 수동 복사해 `features/`로 붙여넣는 시나리오와, 우리가 선택적으로 필요한 함수만 포팅하는 시나리오 둘 다 지원할 계획 수립
  - 파일명/모듈 경로 충돌 점검: Rust 2018 스타일 유지(모듈 디렉토리 방식, `mod.rs` 미사용)
  - `features_45_summary.txt`의 45개 심볼이 실제 함수로 매핑 가능한지 목록 대조
- **검증 포인트**: 심볼명 ↔ 함수명/파일 경로 매핑표 작성

[ ] Phase 1 — 45개 특징 소스 식별 및 매핑표 확정
- **작업 내용**:
  - `features_new/features_45_summary.txt`의 각 항목을 해당 구현 파일/함수로 매핑
  - 예: `day4_bollinger_band_width` → `features_new/day4_new.rs` 또는 `day4.rs`의 `calculate_bollinger_band_width`
  - 이미 `features/`에 동등 구현이 존재하면 재사용, 없으면 이식 대상으로 표기
- **산출물**: 매핑표(JSON/md) 임시 작성(저장 불요, 코멘트 기반으로 진행)

[ ] Phase 2 — 공통 유틸 정합화
- **작업 내용**:
  - `utils.rs`에서 제공하는 공통 함수가 `features_new`와 동일한 시그니처/동작인지 점검
  - 특히 다음 함수/로직의 합치 확인 및 필요 시 업데이트: `get_morning_data`, `get_daily_data`, `is_first_trading_day`, `get_previous_trading_day`, `get_time_range_for_date`, `calculate_ema`, `calculate_rsi`
  - 실전/모의 모드에서 API 경로 분기와 백테스팅에서 DB 경로 분기가 동일하게 작동하도록 유지
- **검증 포인트**: 단위 특징에서 기대하는 반환 타입/에러 타입(`StockrsResult<f64>`, `StockrsError`) 일치

[ ] Phase 3 — 기능 이식(선택적 복사 허용)
- **작업 내용**:
  - 사용자가 먼저 `features_new/dayX.rs`를 `features/dayX.rs`로 복사해둘 수 있음(권장). 이 경우, import 경로만 조정하고, 불필요/중복 함수는 제거
  - 사용자가 복사하지 않은 경우: 45개에 필요한 함수만 `features/dayX.rs`로 신규 생성/편입
  - 모든 함수는 명확한 에러 처리 원칙 준수(복구 가능한 케이스만 Ok 보수값 허용, 나머지는 Err)
- **에지 처리**:
  - 전일 데이터 의존 특징: 첫 거래일이면 설계된 기본값 또는 명시적 Err(현재 코드 정책에 맞춤)
  - 분모 0.0, 데이터 미존재: 현재 레포 방침에 맞춰 Err 또는 보수값 반환(기능별로 이미 정해진 정책 준수)

[ ] Phase 3a — 비중복군 우선 이식(day6 ~ day28)
- **배경**: 기존 day1~day4는 일부 이미 정합화됨. 충돌/중복 리스크가 낮은 day6~day28부터 이식하여 빌드 안정성을 우선 확보
- **작업 내용**:
  - 대상 파일: `features_new/day6.rs` ~ `features_new/day28.rs` 및 관련 인디케이터(`features_new/indicators/*.rs`)
  - 방법 A(사용자 수동 복사): 파일을 `stockrs/src/model/onnx_predictor/features/`로 복사 후, 상위 `features.rs`에 `pub mod dayX;` 추가, 충돌 함수 제거/정리
  - 방법 B(선택 포팅): 45개 목록에 필요한 함수만 개별 dayX 파일로 포팅
  - 인디케이터/유틸 함수는 `features/indicators.rs` 또는 `features/utils.rs`로 합류시키고, 중복 정의 제거
- **검증**:
  - 각 dayX 모듈 추가 후 `cargo build`로 점진 검증
  - `features.txt` 45개 목록 대비 누락 없는지 확인

[ ] Phase 4 — 모듈 선언/exports 정리
- **작업 내용**:
  - `features/mod.rs` 미사용 원칙 준수: 개별 파일 선언은 상위 `features.rs`에서 `pub mod dayX;` 형태로 유지
  - 신규/이식된 파일들에 맞춰 `features.rs`의 `pub use`/매핑 로직 업데이트

[ ] Phase 5 — features.txt 동기화(45개만 활성)
- **작업 내용**:
  - `features.txt`를 45개 목록으로 갱신(정렬 및 주석: “이 파일이 모델 입력 순서를 결정”)
  - 필요 시 `stockrs/src/model/onnx_predictor/features.rs`의 로더가 `features.txt`만 신뢰하도록 확인
- **주의**: 파일 인코딩/개행 윈도우 규칙 유지

[ ] Phase 6 — onnx_predictor 연동 및 시그니처 확인
- **작업 내용**:
  - `stockrs/src/model/onnx_predictor.rs`에서 특징 호출부가 모두 컴파일되는지 확인
  - 일/분봉 DB 커넥션 및 `trading_dates` 전달 시그니처가 최신 함수 정의들과 호환되는지 점검/수정

[ ] Phase 7 — 빌드/런 및 데이터 스폿 체크
- **작업 내용**:
  - 빌드: `cargo build`
  - 백테스트 샘플 실행: `cargo run -p stockrs`
  - 대표 종목/날짜로 특징 3~5개 스폿 검증(첫 거래일, 특이일, 정상일 케이스)
- **PowerShell 주의**: `;`로 명령 연결, 한글 로그는 콘솔 인코딩 영향 주의

[ ] Phase 8 — 문서/로그 정리
- **작업 내용**:
  - `LOG.md`에 파일 이동/이식, 로더/시그니처 변경, `features.txt` 갱신 기록
  - `TODO.md` 장기 과제에 “특징 검증 자동화(샘플 벡터 스냅샷 비교)” 항목 추가 제안

—

📎 사용자가 직접 파일을 복사해 넣는 경우 가이드
1) `features_new/`에서 필요한 `dayX.rs` 파일을 `stockrs/src/model/onnx_predictor/features/`로 복사
2) 상위 `features.rs`에 `pub mod dayX;` 추가 및 매핑 함수에서 해당 심볼 연결
3) 충돌 함수는 하나만 남기고, 정책(에러 처리/시그니처)을 현재 레포 기준으로 통일
4) `features.txt`를 45개로 갱신 후 빌드/런으로 검증

🧪 빠른 검증 커맨드(사용자 실행용)
PowerShell:
; cargo build
; cargo run -p stockrs -- -m backtest


