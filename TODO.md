# StockAI 프로젝트 TODO 리스트

> **실행 환경**: stockai workspace  
> **빌드**: `cargo build` (workspace 루트)  
> **실행**: `cargo run -p stockrs --bin <binary_name>`  

## 📊 전체 진행 상황
- ✅ **완료**: 5개 주요 작업
- 🟡 **진행중**: 1개 작업  
- ⏳ **대기중**: 14개 작업

---

## 🔴 **즉시 필요** - 시스템 실행을 위한 필수 작업

### 1. ⚠️ 사용자 설정 작업 (Manual Required)
- [ ] `config.example.toml` → `config.toml` 복사
- [ ] `config.toml`에 실제 API 키, DB 경로 입력
- [ ] `.gitignore`에 설정 파일 제외 규칙 추가
```gitignore
**/config.toml
**/*.db
**/*.log
**/logs/
```

### 2. ✅ 데이터 파일 복구 완료 (stockai/data/)
- ✅ `best_model.onnx` - AI 모델 파일 (222KB)
- ✅ `extra_stocks.txt` - 제외 종목 리스트 (990개)
- ✅ `features.txt` - 모델 특징 리스트 (20개)
- ✅ `market_close_day_2025.txt` - 휴무일 정보 (20일)
- ✅ `rust_model_info.json` - ONNX 메타데이터

### 3. 🏗️ 의존성 확인 및 빌드 테스트
- [ ] `cargo build` 성공 확인
- [ ] ort 패키지 설치 확인
- [ ] korea-investment-api 빌드 확인

---

## 🟡 **핵심 기능** - API 연동 구현

### 4. 📡 Trading API 구현
- **RealApi** (한국투자증권 실제 거래)
  - [ ] `execute_order()` - 주문 실행
  - [ ] `check_fill()` - 체결 확인  
  - [ ] `cancel_order()` - 주문 취소
  - [ ] `get_balance()` - 잔고 조회

- **PaperApi** (모의투자)
  - [ ] 동일한 인터페이스로 모의거래 구현

- **DbApi** (백테스팅)  
  - [ ] DB 기반 거래 시뮬레이션

### 5. ⚖️ 주문 검증 시스템
- [ ] 수량/가격 유효성 검증
- [ ] 잔고/보유량 확인
- [ ] 상한가/하한가 체크

---

## 🟢 **비즈니스 로직** - 트레이딩 엔진

### 6. 🤖 AI 모델 연동
- [ ] ONNX 모델 로딩 및 예측
- [ ] 특징 데이터 계산
- [ ] 매수/매도 신호 생성

### 7. ⏰ 시간 관리 시스템  
- [ ] 거래시간 체크 (9:00-15:30)
- [ ] 영업일 확인 (휴무일 제외)
- [ ] 이벤트 스케줄링

### 8. 💼 포지션 및 리스크 관리
- [ ] 자금 관리 (포지션 사이즈)
- [ ] 손절매/익절매 로직
- [ ] 분할 매수/매도

---

## 🔵 **시스템 완성** - 통합 및 최적화

### 9. 🔄 컴포넌트 생명주기
- [ ] TimeService: on_start, update, on_end
- [ ] DBManager: on_start, on_event, on_end
- [ ] StockBroker: on_start, on_event, on_end  
- [ ] Model: on_start, on_event, on_end

### 10. 💾 데이터 시스템
- [ ] DataReader 구현 (API/DB 연동)
- [ ] 데이터베이스 스키마 최적화
- [ ] 실시간 시장 데이터 연동

### 11. 🛠️ 시스템 안정성
- [ ] 구조화된 에러 핸들링
- [ ] 로그 시스템 (debug/info/warn/error)
- [ ] 장애 복구 메커니즘

---

## 🟣 **테스트 및 완성** 

### 12. 🧪 테스트 구현
- [ ] 통합 테스트 (컴포넌트 간 연동)
- [ ] API 연동 테스트
- [ ] End-to-end 테스트

### 13. 🚀 실행 시스템 완성
- [ ] CLI 인터페이스 구현
- [ ] 실행 모드 선택 (real/paper/backtest)
- [ ] 사용법 문서화

### 14. 🧹 코드 정리
- [ ] deprecated 코드 제거
- [ ] 불필요한 TODO 주석 정리  
- [ ] 코드 문서화

---

## ✅ **완료된 작업**

- ✅ **프로젝트 구조 정리** - workspace 기준 경로 수정 완료
- ✅ **설정 시스템** - config.toml 지원, 환경변수 오버라이드  
- ✅ **기본 인프라** - 모듈 구조, 타입 정의
- ✅ **데이터 파일 복구** - 모든 필요한 파일 복원 (best_model.onnx, features.txt 등)
- ✅ **설정 파일 준비** - config.example.toml 생성 완료

---

## 🎯 **다음 단계 우선순위**

1. **🔴 즉시**: 사용자 설정 작업 (config.toml 생성) 
2. **🔴 즉시**: 빌드 테스트 및 의존성 확인
3. **🟡 1주차**: Trading API 구현 (RealApi 우선)
4. **🟡 2주차**: AI 모델 연동
5. **🟢 3주차**: 시간 관리 및 리스크 시스템

---

*마지막 업데이트: 2025-01-18*  
*총 작업: 20개 | 완료: 5개 | 진행률: 25%* 