# [ ] 거래일 파일 역할 분리 및 설정/코드 반영

📌 목적 / 배경
- 현재 `time_management.trading_dates_file_path`가 두 가지 의미로 혼용되어 사용되고 있어 설정 의미가 불명확함
  1) 실행 기간 자동 설정용(시작/종료일 범위 산출)
  2) 실제 거래일 목록(시간 서비스/달력·다음/이전 거래일 계산)
- 설정 책임을 분리하여 혼동을 제거하고, 각 용도를 명확히 구분된 키와 사용처로 고정

🔧 입력 / 출력
- 입력
  - config.example.toml (사용자는 config.toml 직접 수정 불가)
  - 파일: `data/schedule_dates.txt` (기간 설정용, YYYYMMDD 라인 목록)
  - 파일: `data/samsung_1min_dates.txt` (시장 개장일 목록)
- 출력
  - `time_management` 섹션에 신규 키 추가 및 주석 정리
  - 코드에서 기간 자동 설정은 `schedule_dates_file_path`를 사용
  - 코드에서 시장 개장일(특징/시간 서비스)은 기존 `trading_dates_file_path` 유지

✅ 완료 조건
- config 로더에서 `auto_set_dates_from_file` 작동 시 참조 경로를 `schedule_dates_file_path`로 변경
- `TradingCalender`는 계속 `trading_dates_file_path`만 사용하도록 유지(변경 없음)
- ONNX 특징 계산은 이미 `onnx_model.features_trading_dates_file_path`를 사용 중임을 확인하고 주석으로 명시
- `config.example.toml`에 두 파일 경로와 설명을 분리·명확화
- `LOG.md`에 작업 이력 타임스탬프 기록

🧩 관련 모듈
- `stockrs/src/utility/config.rs`
- `stockrs/src/utility/trading_calender.rs`
- `config.example.toml`

---

## Phase 1 — 설정 스키마 및 예시 파일 정리
- [ ] `time_management`에 `schedule_dates_file_path`(기본값: `data/samsung_1min_dates.txt`) 추가
- [ ] `auto_set_dates_from_file` 사용 시 시작/종료일 산출 대상 파일을 `schedule_dates_file_path`로 변경
- [ ] `config.example.toml`에 두 경로의 역할을 한국어로 명확히 주석화

## Phase 2 — 코드 참조 경로 분리 반영
- [ ] `TradingCalender`는 변경 없음(시장 개장일 목록은 `trading_dates_file_path` 유지)
- [ ] `Config::load_from_file` 자동 기간 설정 로직이 `schedule_dates_file_path`를 사용하도록 수정

## Phase 3 — 검증
- [ ] config.example.toml만으로 백테스트 실행 시(오늘 이전 데이터는 DBAPI) 시작/종료일 자동 설정 정상 동작
- [ ] `stockrs` 빌드 성공 및 런타임에서 설정 경로 로깅으로 분리 확인
