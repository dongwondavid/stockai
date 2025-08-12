# 현재 진행 중인 TASK

- [ ] 주식 API 재시도 로직 구현 (exponential backoff + jitter)
  - 📌 목적 / 배경: 외부 주식 API(시세/주문/잔고) 호출 시 429, 5xx, 네트워크 일시 오류에 대한 복원력 확보. 주문 엔드포인트는 멱등성 보장을 고려하여 안전한 재시도 설계.
  - 🔧 입력 / 출력:
    - 입력: `RetryPolicy { max_retries, base_delay_ms, max_delay_ms, jitter_ratio, timeout_ms, retryable_status_codes, retry_on_network_errors }`, 호출 클로저(`Fn() -> Future<Result<T, ApiError>>`)
    - 출력: `Result<T, ApiError>`; 모든 시도/성공/최종 실패를 콘솔 로그로 구조화 출력
  - ✅ 완료 조건:
    - 코어 호출 경로에 공통 재시도 래퍼 적용: 시세조회, 주문, 잔고조회
    - 재시도 정책 단위 테스트 작성(성공 전환, 최대 재시도 후 실패, 지수 백오프 증가, 지터 분산, 타임아웃 전파)
    - 통합 테스트(모킹 서버/에러 주입)로 429/5xx/네트워크 오류 시 동작 검증
    - 로그에 시도 횟수, 대기 시간, 에러 코드 출력. 파일 로깅 금지, 콘솔만 사용
  - 🧩 관련 모듈:
    - `stockrs/src/utility/apis/korea_api.rs` (호출 지점 래핑)
    - `korea-investment-api/src/stock/order.rs`, `.../quote.rs`, `.../balance.rs` (호출 함수 연결)
    - `stockrs/src/utility/errors.rs` (에러 타입 확장), `stockrs/src/utility/config.rs` (정책 설정값 주입, 기본값 제공)
    - 선택: `stockrs/src/utility/apis/db_api.rs`에도 동일 래퍼 적용(백테스팅 안정성)