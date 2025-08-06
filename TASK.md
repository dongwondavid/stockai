# TASK.md - 현재 진행 중인 실행 과제

### 📌 목적 / 배경
- **Time 로직 문제점 해결**: 현재 time.rs와 runner.rs에서 시간 관리 로직에 문제가 있어 백테스팅과 모의투자가 제대로 작동하지 않음
- **백테스팅 정상화**: 시간 기반 백테스팅이 올바른 순서로 진행되도록 수정
- **모의투자 정상화**: 실시간 모의투자가 장 시간에 맞춰 정확히 작동하도록 수정
- **장 중간 진입 로직 구현**: 모의투자에서 장 중간에 시작할 때의 동작 방식 정의 및 구현

### 🔧 입력 / 출력
**입력:**
- 현재 time.rs의 TimeService 구조체
- runner.rs의 wait_until_next_event 메서드
- config.toml의 time_management 설정
- 모의투자 관련 API 설정

**출력:**
- 수정된 time.rs (시간 관리 로직 개선)
- 수정된 runner.rs (이벤트 처리 로직 개선)
- 모의투자 장 중간 진입 로직
- 테스트 결과 및 검증

### ✅ 완료 조건
1. **백테스팅이 올바른 시간 순서로 진행됨** (08:30 → 09:00 → 09:01~15:29 → 15:20 → Overnight)
2. **모의투자가 실시간으로 정확한 시간에 이벤트 발생**
3. **장 중간 진입 시 올바른 동작 방식 정의 및 구현**
4. **모든 시간 관련 로직이 TradingMode에 따라 적절히 분기 처리**

### 🧩 관련 모듈
- `stockrs/src/time.rs` - TimeService 구조체 및 시간 관리 로직
- `stockrs/src/runner.rs` - 메인 실행 루프 및 이벤트 처리
- `stockrs/src/utility/types/trading.rs` - TradingMode enum
- `stockrs/src/utility/config.rs` - 설정 관리
- `stockrs/src/utility/apis/korea_api.rs` - 실시간 API 연동

### 📋 구현 세부사항

#### 1. 현재 Time 로직 문제점 분석
**문제점:**
- `wait_until_next_event()` 메서드에서 백테스팅과 실시간 모드의 로직이 혼재
- `should_skip_to_next_trading_day()` 체크가 중복으로 발생
- Overnight 신호 처리 로직이 중복됨
- 모의투자에서 장 중간 진입 시 동작 방식이 정의되지 않음
- **NEW**: `on_start()` 메서드가 비어있어 백테스팅 초기화가 제대로 되지 않음
- **NEW**: `wait_until_next_event`에서 백테스팅 모드일 때 `compute_next_time()` 호출로 인한 무한 루프 가능성
- **NEW**: 백테스팅 시작 시 첫 번째 이벤트(08:30)로 설정되지 않음
- **NEW**: `compute_next_time()`에서 `self.add_minute()` 호출로 인한 2분씩 건너뛰는 문제
- **NEW**: 메인 루프에서 `self.time.update()`와 `wait_until_next_event()`가 중복으로 시간을 업데이트하는 문제
- **NEW**: db_api에서 fallback 처리가 있어서 데이터가 없어도 에러가 발생하지 않는 문제
- **NEW**: 백테스팅에서 `chrono::Local::now()`를 사용하여 현재 시간(2025년)을 체크하는 문제
- **NEW**: 백테스팅에서 09:01 이전이나 특별한 날의 10:01 이전에는 실제 데이터가 없어서 가격 조회 실패하는 문제

**해결 방안:**
- TimeService에 TradingMode별 전용 메서드 구현
- 중복 로직 제거 및 통합
- 장 중간 진입 시 현재 시간부터 시작하는 로직 추가

#### 2. TimeService 개선
**새로운 메서드 추가:**
```rust
// TradingMode별 대기 로직
pub fn wait_until_next_event(&mut self, trading_mode: TradingMode) -> StockrsResult<()>

// 장 중간 진입 처리
pub fn handle_mid_session_entry(&mut self, trading_mode: TradingMode) -> StockrsResult<()>

// 다음 거래일 이동 처리
pub fn handle_next_trading_day(&mut self, trading_mode: TradingMode) -> StockrsResult<()>
```

#### 3. Runner 로직 개선
**wait_until_next_event 메서드 리팩토링:**
- 중복 체크 로직 제거
- TradingMode별 분기 처리 명확화
- 에러 처리 개선

#### 4. 모의투자 장 중간 진입 로직
**동작 방식:**
1. **시작 시간 확인**: 현재 시간이 거래 시간(09:00~15:30) 내인지 확인
2. **현재 상태 설정**: 현재 시간에 맞는 TimeSignal로 설정
3. **이벤트 스케줄링**: 다음 이벤트 시간 계산
4. **API 연결**: 실시간 데이터 수신 준비

### 🎯 진행 상태
**✅ Time 로직 문제점 해결 완료**

**완료된 작업:**
1. ✅ **market_hours 설정 수정**: trading_end_time과 last_update_time을 올바르게 설정 (15:29, 15:30)
2. ✅ **wait_until_next_event 로직 개선**: 백테스팅에서 현재 거래일 내에서 다음 이벤트로 이동하도록 수정
3. ✅ **중복 로직 제거**: runner.rs에서 중복된 should_skip_to_next_trading_day 체크 제거
4. ✅ **장 중간 진입 로직 구현**: handle_mid_session_entry 메서드 추가
5. ✅ **컴파일 에러 수정**: Timelike trait import 추가 및 경고 해결
6. ✅ **on_start() 메서드 구현**: 백테스팅 시작 시 08:30 DataPrep으로 초기화
7. ✅ **2분씩 건너뛰는 문제 해결**: compute_next_time에서 Duration::minutes(1) 사용
8. ✅ **중복 시간 업데이트 문제 해결**: 메인 루프에서 self.time.update() 제거
9. ✅ **fallback 처리 제거**: db_api에서 정확한 시간에 데이터가 없으면 에러 발생
10. ✅ **백테스팅 시간 문제 해결**: TimeService 참조를 ApiBundle에 추가하여 올바른 백테스팅 시간 사용
11. ✅ **백테스팅용 특별한 fallback 로직 추가**: 09:01 이전은 09:01로, 특별한 날 10:01 이전은 10:01로 조회

**다음 단계:**
1. 🔄 **실제 테스트**: 백테스팅과 모의투자 모드에서 시간 로직 검증
2. 🔄 **추가 문제점 발견 시**: TASK.md에 추가하고 해결
3. 🔄 **최종 검증**: 모든 테스트 체크리스트 통과 확인

### 🧪 테스트 체크리스트

#### 백테스팅 테스트
- [ ] **시간 순서 테스트**: 08:30 → 09:00 → 09:01~15:29 → 15:20 → Overnight 순서로 진행
- [ ] **거래일 이동 테스트**: 하루 종료 후 다음 거래일로 올바르게 이동
- [ ] **종료일 도달 테스트**: end_date 도달 시 정상 종료
- [ ] **객체 리셋 테스트**: 새로운 거래일 시작 시 모든 객체가 올바르게 리셋

잔고 계산 버그 존재 해결 필요

#### 모의투자 테스트
- [ ] **실시간 대기 테스트**: 실제 시간에 맞춰 이벤트 발생
- [ ] **장 시작 전 대기**: 09:00 이전에 시작 시 올바른 대기
- [ ] **장 중간 진입 테스트**: 10:30, 14:00 등 장 중간에 시작 시 동작
- [ ] **장 종료 후 대기**: 15:30 이후 시작 시 다음 거래일까지 대기

#### 장 중간 진입 테스트
- [ ] **10:30 진입 테스트**: 현재 시간이 10:30일 때 올바른 TimeSignal 설정
- [ ] **14:00 진입 테스트**: 현재 시간이 14:00일 때 올바른 TimeSignal 설정
- [ ] **15:20 이후 진입 테스트**: 장 마감 후 진입 시 Overnight 신호로 설정
- [ ] **API 연결 테스트**: 진입 시점에 실시간 API 연결 확인

#### 통합 테스트
- [ ] **모드별 분기 테스트**: Backtest/Paper/Real 모드별 올바른 동작
- [ ] **에러 처리 테스트**: 잘못된 시간 설정 시 적절한 에러 메시지
- [ ] **성능 테스트**: 시간 계산 및 대기 로직의 성능 확인
- [ ] **로깅 테스트**: 시간 관련 로그가 올바르게 출력되는지 확인