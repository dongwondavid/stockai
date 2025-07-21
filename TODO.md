# TODO.md - StockAI 프로젝트 개선 계획

## 🔧 백테스팅 시스템 개선

### DBManager 로직 수정
- [x] runner.rs on_event 시간 포맷 수정 ("YYYYMMDDHHMM" 전달)
- [x] DbApi::get_current_price_at_time 쿼리 파라미터 바인딩 수정
- [x] 분봉 조회 fallback 로직 검증 및 개선
- [x] 전체 경로 (runner.rs → DBManager::on_event → get_current_price_at_time) 포맷 변환 수정

### 시간 처리 로직 개선
- [x] TimeService 포맷 통일 및 헬퍼 함수 모듈화
- [ ] 하드코딩된 시장 시간 상수 설정 파일 분리
- [ ] now() 호출 시점 일관성 보장 메커니즘 구현
- [x] 문자열 포맷 통일성 확보 ("YYYYMMDDHHMM" 표준화)
- [ ] 주말·공휴일 체크 로직 분리 및 모듈화
- [ ] 시간 관련 에러 처리 일관성 확보
- [x] TimeService 의존성 주입(DI) 패턴 적용
- [ ] Duration 연산 코드 중복 제거
- [x] BacktestApi current_time 필드 제거 (TimeService 직접 활용)

## ⚙️ 설정 시스템 개선

### 리스크 관리 시스템 구현
- [ ] 일일 최대 손실 한도 (daily_max_loss) 구현
- [ ] 총 자산 대비 최대 투자 비율 (max_investment_ratio) 구현
- [ ] 단일 종목 최대 투자 비율 (max_single_stock_ratio) 구현
- [ ] VaR(Value at Risk) 계산 시스템 구현 (var_confidence_level 활용)

### 예측 모델 설정 시스템 구현
- [ ] 예측 임계값 기반 매수/매도 로직 (buy_threshold, sell_threshold) 구현
- [ ] 특징 정규화 시스템 (normalize_features) 구현
- [ ] 거래대금 상위 종목 수 설정 (top_volume_stocks) 동적 적용

### 성능 최적화 설정 구현
- [ ] 데이터베이스 연결 풀링 시스템 (db_pool_size) 구현
- [ ] API 요청 제한 시스템 (api_rate_limit) 구현
- [ ] 병렬 처리 스레드 관리 (worker_threads) 구현
- [ ] 메모리 캐시 관리 시스템 (cache_size_mb) 구현

### 로깅 시스템 개선
- [ ] 파일 로깅 시스템 (file_path, max_file_size, max_files) 구현
- [ ] 로그 로테이션 및 압축 기능 구현
- [ ] 구조화된 로깅 (JSON 형식) 지원

### 거래 설정 확장
- [ ] 익절매 비율 (take_profit_ratio) 기반 매도 로직 구현
- [ ] 최소 주문 금액 (min_order_amount) 검증 로직 구현
- [ ] 거래세율 및 증권거래세율 (transaction_tax_rate, securities_tax_rate) 적용

## 🤖 예측 모델 검증

### ONNX 모델 정합성 확인
- [ ] solomon 프로젝트 재검토 및 모델 정합성 검증
- [ ] Python 모델과 Rust ONNX 모델 예측 결과 비교
- [ ] 모델 메타데이터 및 피처 매핑 검증
- [ ] 예측 성능 일관성 확인

## 🏗️ 코드 구조 개선

### 거대한 단일 파일 리팩터링
- [ ] runner.rs 모듈 분리 (Scheduler, Executor, Reporter)
- [ ] db_manager.rs 모듈 분리 (416줄, 479줄 파일 분할)
- [ ] 비즈니스 로직과 DB I/O 분리
- [ ] 트레이딩 리포지토리 레이어 분리

### 코드 품질 개선
- [ ] 매직 넘버 및 하드코딩 상수 해결을 위한 config.example.toml 재설계
- [ ] 에러 처리 일관성 확보 (thiserror 기반)
- [ ] SQL 쿼리 템플릿 중앙화
- [ ] DB 커넥션 풀링 구현 (r2d2 + rusqlite)

### 로깅 및 테스트
- [ ] tracing 크레이트 기반 로깅 시스템 통일
- [ ] 핵심 모듈 단위 테스트 작성
- [ ] 통합 테스트 구현 (가짜 DB 포함)
- [ ] 테스트 커버리지 측정 및 개선
