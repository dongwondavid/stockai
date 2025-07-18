# StockAI 프로젝트 TODO 리스트

> **실행 환경**: stockai workspace  
> **빌드**: `cargo build` (workspace 루트)  
> **실행**: `cargo run -p stockrs --bin <binary_name>`  
> **마지막 업데이트**: Git 상태 기반 (2024년 기준)

## 📊 전체 진행 상황
- ✅ **완료**: 8개 주요 작업 (+3 추가 완료)
- 🔥 **진행중**: 6개 Arc 리팩토링 작업  
- ⏳ **대기중**: 5개 작업

---

## 🔥 **진행중** - Arc 기반 API 공유 구조 리팩토링

> **목표**: prototype.py와 동일하게 각 데이터 출처마다 하나의 연결만 존재하도록 보장  
> **현재 상태**: 주요 구조 변경 작업들이 진행중 🚧

### 🚀 Step 1: 새로운 API 구조 만들기
- ✅ **`stockrs/src/apis/` 디렉토리 생성** (git에서 확인됨)
- 🔄 **`StockApi` trait 확장** (`stockrs/src/types/api.rs`) - 수정중
  ```rust
  pub trait StockApi: Send + Sync {
      // 기존 메서드들...
      fn get_avg_price(&self, stockcode: &str) -> Result<f64, Box<dyn Error>>;
      fn get_current_price(&self, stockcode: &str) -> Result<f64, Box<dyn Error>>;
  }
  ```
- 🔄 **API 중복 제거 구조** (`stockrs/src/apis/korea_api.rs`) - 구현중
  - Enum 기반 `KoreaApi` 구조체 (Real/Paper 모드 통합)
  - `ApiMode::Real`, `ApiMode::Paper` enum 구분
  - `new_real()`, `new_paper()` 생성자
- 🔄 **DbApi 백테스팅 구현** (`stockrs/src/apis/db_api.rs`) - 구현중
  - solomon DB 연결 (`stock_daily_db`, `stock_5min_db`)
  - 포트폴리오 시뮬레이션 (`BacktestState`)
  - 시장 데이터 조회 및 거래 체결 시뮬레이션

### 🗑️ Step 2: data_reader 완전 제거
- ✅ **`stockrs/src/data_reader.rs` 파일 삭제** (git에서 확인됨)
- 🔄 **관련 import 정리** - 진행중
  - `stockrs/src/lib.rs`에서 data_reader 모듈 제거
  - `stockrs/src/runner.rs`에서 DataReaderType import 제거
  - `stockrs/src/db_manager.rs`에서 DataReader import 제거

### 🔄 Step 3: Runner Arc 구조로 변경
- 🔄 **Runner 구조 변경** (`stockrs/src/runner.rs`) - 수정중
  ```rust
  pub struct Runner {
      // prototype.py와 동일한 API 관리
      real_api: Arc<dyn StockApi>,
      paper_api: Arc<dyn StockApi>, 
      db_api: Arc<dyn StockApi>,
      
      // 기존 컴포넌트들
      broker: StockBroker,
      db_manager: DBManager,
      // ...
  }
  ```
- 🔄 **prototype.py 방식 API 생성 로직** - 구현중
  ```rust
  // ApiType::Paper인 경우
  real_api: Arc::new(DbApi::new()?),      // 대체용
  paper_api: Arc::new(KoreaApi::new_paper()?),  // 실제 API
  db_api: Arc::new(DbApi::new()?),        // 백테스팅용
  ```
- 🔄 **`create_api` 팩토리 함수 제거** - 진행중

### ⚙️ Step 4: 컴포넌트들 Arc 사용으로 수정
- 🔄 **DBManager 수정** (`stockrs/src/db_manager.rs`) - 수정중
  - `data_reader` 필드 제거
  - 생성자에서 `DataReaderType` 파라미터 제거
  - `save_trading()` 메서드에 API Arc 저장 또는 외부 주입
- 🔄 **StockBroker 수정** (`stockrs/src/broker.rs`) - 수정중
  - `api: Box<dyn StockApi>` → `api: Arc<dyn StockApi>` 변경
  - `execute()` 메서드에서 API 파라미터 제거 (내부 Arc 사용)
- ✅ **Model 구조 변경** (stockrs/src/model/ 디렉토리로 이동 완료)

### 🧪 Step 5: 테스트 및 검증
- [ ] **단위 테스트 작성**
  - API 인스턴스 생성 횟수 확인
  - 백테스팅 시뮬레이션 정확성 검증
- 🔄 **통합 테스트** - 수정중
  - `stockrs/src/bin/test_runner.rs` 수정중 (git에서 확인됨)
  - 전체 워크플로우 정상 작동 확인

---

## 🔄 **현재 Git 상태 기반 진행 상황**

### ✅ 완료된 작업들 (Git에서 확인)
- ✅ `stockrs/src/apis/` 디렉토리 생성 (untracked)
- ✅ `stockrs/src/data_reader.rs` 파일 삭제  
- ✅ `stockrs/src/model.rs` → `stockrs/src/model/` 디렉토리 구조 변경
- ✅ 데이터 파일 복구 완료 (stockai/data/)

### 🔄 수정 진행중인 파일들
- 🔄 `stockrs/src/types/api.rs` - StockApi trait 확장
- 🔄 `stockrs/src/broker.rs` - Arc 구조로 변경  
- 🔄 `stockrs/src/runner.rs` - Runner 구조 변경
- 🔄 `stockrs/src/db_manager.rs` - data_reader 제거
- 🔄 `stockrs/src/bin/test_runner.rs` - 테스트 코드 수정
- 🔄 Cargo.toml 파일들 - 의존성 업데이트

### 📌 다음 우선 작업
1. **빌드 테스트**: `cargo build`로 현재 변경사항 컴파일 확인
2. **API 구현 완성**: `stockrs/src/apis/korea_api.rs`, `db_api.rs` 
3. **통합 테스트**: 전체 워크플로우 검증

---

## ⚠️ **설정 및 준비 작업**

### 1. 사용자 설정 작업 (Manual Required)
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

### 3. 의존성 확인 및 빌드 테스트
- ⚠️ **즉시 필요**: `cargo build` 성공 확인 (현재 수정사항 컴파일 체크)
- [ ] ort 패키지 설치 확인
- [ ] korea-investment-api 빌드 확인

---

## 🎯 **핵심 기능** (Arc 리팩토링 완료 후 구현)

> **주의**: 현재 Arc 기반 API 구조 리팩토링이 완료되어야 아래 기능들을 안정적으로 구현할 수 있습니다.

### 4. 🔗 API 통합 구현 (최우선)

#### 4.1 한국투자증권 API 구현 (`apis/korea_api.rs`)
- [ ] 실제 한국투자증권 API 주문 실행 구현
- [ ] 실제 한국투자증권 API 체결 확인 구현  
- [ ] 실제 한국투자증권 API 주문 취소 구현
- [ ] 실제 한국투자증권 API 잔고 조회 구현
- [ ] 실제 한국투자증권 API 평균가 조회 구현
- [ ] 실제 한국투자증권 API 현재가 조회 구현

#### 4.2 백테스팅 DB API 구현 (`apis/db_api.rs`)
- [ ] 백테스팅 DB API 주문 실행 구현
- [ ] 백테스팅 DB API 체결 확인 구현
- [ ] 백테스팅 DB API 주문 취소 구현
- [ ] 백테스팅 DB API 잔고 조회 구현
- [ ] 백테스팅 DB API 평균가 조회 구현
- [ ] 백테스팅 DB API 현재가 조회 구현
- [ ] 실제 DB 연결 구현 (현재 빈 구조체)

#### 4.3 기존 API 모듈 정리 (`api/` 디렉토리)
- [ ] `api/db_api.rs` - DB API 주문 실행, 체결 확인, 취소 구현
- [ ] `api/koreainvestapi.rs` - deprecated된 함수들 정리 또는 제거

### 5. 🤖 모델 구현

#### 5.1 ONNX 예측기 완성 (`model/onnx_predictor.rs`)
- [ ] ONNX 모델을 사용한 실제 종목 예측 구현
- [ ] DB 연결 및 실시간 데이터 분석 로직
- [ ] solomon 크레이트에서 핵심 기능들 포팅:
  - [ ] `get_top_volume_stocks`: 거래대금 상위 종목 조회
  - [ ] `calculate_features_for_stocks`: 종목별 특징 계산
  - [ ] `predict_with_onnx_model`: 실제 ONNX 모델 예측

#### 5.2 Joonwoo 모델 완성 (`model/joonwoo.rs`)
- [ ] 실제 가격 조회 API와 잔고 조회 API 연동하여 매수 주문 생성
- [ ] 실제 API를 통한 현재가 조회 및 손절/익절 조건 검사 구현
- [ ] 실제 API를 통한 현재가 조회 및 강제 정리 주문 생성
- [ ] ONNX 예측기와의 완전한 통합

#### 5.3 모델 활성화
- [ ] `model.rs`에서 `joonwoo` 모듈 다시 활성화 (현재 주석 처리됨)

### 6. ⏰ 시간 관리 시스템
- [ ] time 초기화 로직 구현 (`time.rs`)
- [ ] time 정리 로직 구현 (`time.rs`)
- [ ] 시간 기반 이벤트 대기 로직 구현 (`runner.rs`) (현재 todo!)
- [ ] 시장 시간 체크
- [ ] 이벤트 스케줄링
- [ ] 백테스팅 시간 진행

### 7. 🏦 브로커 & DB 관리

#### 7.1 브로커 로직 완성 (`broker.rs`)
- [ ] 주문 유효성 검증 로직 구현
- [ ] broker 초기화 로직 구현
- [ ] broker 정리 로직 구현
- [ ] 리스크 관리 시스템
- [ ] 포지션 관리

#### 7.2 DB 관리자 완성 (`db_manager.rs`)
- [ ] db_manager 초기화 로직 구현
- [ ] broker 결과 처리 로직 구현
- [ ] db_manager 정리 로직 구현

### 8. ⚙️ 설정 및 기타

#### 8.1 설정 개선 (`types/api.rs`)
- [ ] 모의투자 API HTS_ID를 config에서 읽기 (현재 하드코딩)
- [ ] 모의투자 API 평균가 조회 구현 (새 구조로 교체 예정)
- [ ] 모의투자 API 현재가 조회 구현 (새 구조로 교체 예정)

### 9. 모니터링 및 로깅
- [ ] 거래 로그 시스템
- [ ] 성능 모니터링
- [ ] 오류 추적

---

## 📋 **개발 우선순위**

### 🚨 High Priority (즉시 필요)
1. **Arc 기반 API 구조 리팩토링 완료** - 현재 진행중인 Step 1-5 완료
2. **한국투자증권 API 실제 구현** - 실거래/모의투자 동작을 위해 필수
3. **ONNX 예측기 완성** - 모델의 핵심 기능
4. **Joonwoo 모델 API 연동** - 전체 트레이딩 로직 완성

### ⚡ Medium Priority (단계적 구현)
5. **백테스팅 DB API** - 과거 데이터 기반 테스팅
6. **시간 관리 개선** - 정확한 타이밍 제어
7. **브로커/DB 로직 완성** - 안정성 향상

### 🔧 Low Priority (최적화)
8. **설정 개선** - 유지보수성 향상
9. **기존 API 모듈 정리** - 코드 정리
10. **모니터링 시스템** - 운영 환경 최적화

---

## 🔄 **단계별 개발 플로우**

### Phase 1: 기반 구조 완성
1. **Arc 기반 API 구조 리팩토링 완료** (현재 진행중)
2. **빌드 테스트 및 기본 구조 검증**

### Phase 2: 핵심 API 구현
1. **한국투자증권 API 기본 구현** (주문, 체결, 취소, 잔고)
2. **백테스팅 DB API 구현**
3. **기존 API 모듈 정리**

### Phase 3: 모델 및 전략 구현
1. **ONNX 예측기와 Joonwoo 모델 완성**
2. **solomon 크레이트 핵심 기능 포팅**
3. **모델 간 통합 및 전략 로직 구현**

### Phase 4: 시스템 통합
1. **전체 시스템 통합 테스트**
2. **시간 관리 및 브로커/DB 로직 완성**
3. **설정 시스템 개선**

### Phase 5: 운영 최적화
1. **백테스팅 시스템 완전 구현**
2. **모니터링 및 로깅 시스템**
3. **최적화 및 안정성 개선**

> **⚠️ 참고**: 현재 모든 더미 구현이 `todo!`로 교체되어 컴파일 시 패닉이 발생합니다. 
> 개발 우선순위에 따라 단계적으로 구현해야 합니다.

---

## 📋 **예상 결과**

### 리팩토링 전 (현재)
```
ApiType::Paper 모드:
- broker용 PaperApi 인스턴스 1개
- data_reader용 PaperApi 인스턴스 1개
- 총 2개+ 연결 생성 ❌
```

### 리팩토링 후 (목표)
```
ApiType::Paper 모드:
- real_api: DbApi (대체용)
- paper_api: KoreaApi (실제 연결) ← 하나만!
- db_api: DbApi (백테스팅용)
- 각 타입당 정확히 1개 연결 ✅
``` 