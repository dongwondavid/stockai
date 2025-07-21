# TODO.md - StockAI 프로젝트 개선 계획

## 🔧 백테스팅 시스템 개선

### DBManager 로직 수정
- [x] runner.rs on_event 시간 포맷 수정 ("YYYYMMDDHHMM" 전달)
- [x] DbApi::get_current_price_at_time 쿼리 파라미터 바인딩 수정
- [x] 분봉 조회 fallback 로직 검증 및 개선
- [x] 전체 경로 (runner.rs → DBManager::on_event → get_current_price_at_time) 포맷 변환 수정

### 시간 처리 로직 개선
- [x] TimeService 포맷 통일 및 헬퍼 함수 모듈화
- [x] 하드코딩된 시장 시간 상수 설정 파일 분리
- [x] now() 호출 시점 일관성 보장 메커니즘 구현
- [x] 문자열 포맷 통일성 확보 ("YYYYMMDDHHMM" 표준화)
- [x] 주말·공휴일 체크 로직 분리 및 모듈화
- [x] 시간 관련 에러 처리 일관성 확보
- [x] TimeService 의존성 주입(DI) 패턴 적용
- [x] Duration 연산 코드 중복 제거
- [x] BacktestApi current_time 필드 제거 (TimeService 직접 활용)

## ⚙️ 설정 시스템 개선

### 성능 최적화 설정 구현
- [ ] 데이터베이스 연결 풀링 시스템 (db_pool_size) 구현
- [ ] API 요청 제한 시스템 (api_rate_limit) 구현
- [ ] 병렬 처리 스레드 관리 (worker_threads) 구현
- [ ] 메모리 캐시 관리 시스템 (cache_size_mb) 구현

### 로깅 시스템 개선
- [x] tracing crate 완벽히 사용
- [ ] 파일 로깅 시스템 (file_path, max_file_size, max_files) 구현
- [ ] 로그 로테이션 및 압축 기능 구현
- [ ] 구조화된 로깅 (JSON 형식) 지원

## 🏗️ 코드 구조 개선

### 거대한 단일 파일 리팩터링
- [ ] runner.rs 모듈 분리 (Scheduler, Executor, Reporter)
- [ ] db_manager.rs 모듈 분리 (416줄, 479줄 파일 분할)
- [ ] 비즈니스 로직과 DB I/O 분리
- [ ] 트레이딩 리포지토리 레이어 분리

### 코드 품질 개선
- [x] 매직 넘버 및 하드코딩 상수 해결을 위한 config.example.toml 재설계
- [x] 에러 처리 일관성 확보 (thiserror 기반)
- [x] SQL 쿼리 템플릿 중앙화
- [ ] DB 커넥션 풀링 구현 (r2d2 + rusqlite)

### Clippy 경고 해결 (우선순위 높음)
- [x] stockrs: assertions_on_constants 경고 해결 (errors.rs)
- [x] stockrs: 모든 clippy 경고 해결 (0개 경고 달성)
- [x] 코드 품질 개선 (unwrap() → expect(), 불필요한 변수 할당 제거)

### 로깅 및 테스트
- [x] tracing 크레이트 기반 로깅 시스템 통일
- [ ] 핵심 모듈 단위 테스트 작성
- [ ] 통합 테스트 구현 (가짜 DB 포함)
- [ ] 테스트 커버리지 측정 및 개선
