# 현재 진행 중인 TASK

## [ ] 슬리피지 기록 시스템 구현

### 📌 목적 / 배경
- 현재 trading 테이블은 주문 시도 가격(`price`)만 저장하고 있어 실제 체결 가격과 차이가 있음
- overview 테이블의 거래대금 계산이 주문 시도 가격 기준으로 되어 실제 손익과 불일치
- 슬리피지가 적용된 실제 체결 가격을 별도로 기록하여 정확한 거래 분석 가능

### 🔧 입력 / 출력
- **입력**: 
  - 기존 주문 시도 가격 (`price`)
  - config.toml의 슬리피지 설정 (`buy_slippage_rate`, `sell_slippage_rate`)
- **출력**:
  - `trading` 테이블에 `real_price` 컬럼 추가
  - 실제 체결 가격 (슬리피지 적용) 저장
  - `overview` 테이블 거래대금 계산을 `real_price` 기준으로 변경

### ✅ 완료 조건
1. `trading` 테이블에 `real_price` 컬럼이 정상적으로 추가됨
2. 백테스팅 시 슬리피지가 적용된 실제 가격이 `real_price`에 저장됨
3. `overview` 테이블의 거래대금이 `real_price * quantity`로 계산됨
4. 기존 `price` 필드는 주문 시도 가격으로 유지됨
5. 모든 거래 기록에서 `price`와 `real_price`가 정확히 구분되어 저장됨

### 🧩 관련 모듈
- `stockrs/src/db_manager.rs` - DB 스키마 및 저장 로직
- `stockrs/src/utility/types/trading.rs` - Trading 구조체
- `stockrs/src/utility/apis/backtest_api.rs` - 슬리피지 계산 로직
- `stockrs/src/utility/config.rs` - 슬리피지 설정 (이미 완성됨)

### 📋 구현 체크리스트
- [ ] Trading 구조체에 `real_price` 필드 추가
- [ ] TradingResult 구조체에 `real_price` 필드 추가
- [ ] DB 스키마에 `real_price` 컬럼 추가
- [ ] BacktestApi에서 실제 체결 가격 계산 로직 구현
- [ ] DB 저장 시 `real_price` 포함하여 저장
- [ ] Overview 거래대금 계산을 `real_price` 기준으로 변경
- [ ] 기존 데이터와의 호환성 확인
- [ ] 테스트 실행 및 검증

### 🔄 진행 상황
- [x] 코드 분석 완료
- [x] 구현 계획 수립
- [x] 데이터 타입 수정
- [x] DB 스키마 변경
- [x] API 로직 수정
- [x] 저장 로직 수정
- [x] Overview 계산 수정
- [x] 기존 데이터와의 호환성 확인
- [x] 컴파일 성공 확인
- [x] 실제 환경(모의투자/실전투자)에서 실제 체결 가격 받아오기 구현
- [x] broker.rs에서 실제 체결 가격 처리 로직 개선
- [ ] 테스트 실행 및 검증
