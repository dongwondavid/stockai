# TASK.md - 현재 진행 중인 실행 과제

## [ ] start1000.txt 날짜 기반 시스템 시작 시간 1시간 지연 기능 구현

### 📌 목적 / 배경
- `data/start1000.txt`에 명시된 특정 날짜들에 대해서만 시스템 시작 시간을 1시간 늦춰서 10:00에 시작하도록 하는 기능 구현
- 기존 09:00 시작 시스템을 특정 날짜에만 10:00 시작으로 조정하여 유연한 시간 관리 제공
- 관련된 모든 시간 기반 작업들(매수/매도 시간, 데이터 준비 시간 등)도 함께 조정

### 🔧 입력 / 출력
**입력:**
- `data/start1000.txt`: 특별한 시작 시간이 적용되는 날짜 목록 (YYYYMMDD 형식)
- `config.example.toml`: 시간 오프셋 설정 및 파일 경로 설정
- 현재 시스템의 모든 시간 기반 로직

**출력:**
- 특별한 날짜에 대해 1시간 지연된 시스템 시작
- 조정된 매수/매도 시간 (09:30 → 10:30, 12:00 → 13:00)
- 조정된 데이터 준비 시간 (08:30 → 09:30)
- 조정된 특징 추출 시간 범위 (09:00-09:30 → 10:00-10:30)

### ✅ 완료 조건
1. **설정 시스템 확장**
   - [ ] `config.example.toml`에 `special_start_dates_file_path` 설정 추가
   - [ ] `config.example.toml`에 `special_start_time_offset_minutes` 설정 추가
   - [ ] `TimeManagementConfig` 구조체에 새로운 필드들 추가
   - [ ] 기본값 설정 및 설정 로드 로직 구현

2. **TimeService 핵심 로직 수정**
   - [ ] `TimeService`에 `special_start_dates` 필드 추가
   - [ ] 특별한 날짜 파일 로드 로직 구현
   - [ ] `compute_next_time` 함수에서 특별한 날짜 체크 로직 추가
   - [ ] 시간 계산 시 오프셋 적용 로직 구현
   - [ ] `parse_time_string` 함수에 오프셋 적용 기능 추가

3. **joonwoo 모델 시간 조정**
   - [ ] `joonwoo.rs`의 `entry_time`과 `force_close_time`에 오프셋 적용
   - [ ] 매수/매도 시간 계산 로직 수정
   - [ ] 시간 기반 상태 전환 로직 조정

4. **특징 추출 시간 범위 조정**
   - [ ] `features/utils.rs`의 `get_time_range_for_date` 함수 수정
   - [ ] `is_special_trading_date` 함수에 start1000.txt 날짜 체크 로직 추가
   - [ ] 특별한 날짜의 시간 범위를 10:00-10:30으로 조정

5. **테스트 및 검증**
   - [ ] start1000.txt에 있는 날짜들에 대한 시간 조정 테스트
   - [ ] 일반 날짜들에 대한 기존 시간 유지 테스트
   - [ ] 설정 파일 오류 시 기본값 동작 테스트
   - [ ] 시간 오프셋이 다른 모듈에 미치는 영향 검증

### 🧩 관련 모듈
- **설정 관리**: `stockrs/src/utility/config.rs`
- **시간 서비스**: `stockrs/src/time.rs`
- **joonwoo 모델**: `stockrs/src/model/joonwoo.rs`
- **특징 추출**: `stockrs/src/model/onnx_predictor/features/utils.rs`
- **설정 파일**: `config.example.toml`
- **특별 날짜 파일**: `data/start1000.txt`

### 📋 구현 세부 계획

#### Phase 1: 설정 시스템 확장
1. `config.example.toml`에 새로운 설정 섹션 추가
2. `TimeManagementConfig` 구조체 확장
3. 설정 로드 및 기본값 처리 로직 구현

#### Phase 2: TimeService 핵심 기능 구현
1. 특별한 날짜 파일 로드 기능 구현
2. 시간 오프셋 적용 로직 구현
3. 기존 시간 계산 함수들 수정

#### Phase 3: 모델 및 유틸리티 수정
1. joonwoo 모델의 시간 기반 로직 수정
2. 특징 추출 시간 범위 조정
3. 관련 헬퍼 함수들 수정

#### Phase 4: 테스트 및 검증
1. 단위 테스트 작성
2. 통합 테스트 수행
3. 실제 날짜 데이터로 검증

### 🔍 기술적 고려사항
- **성능**: 특별한 날짜 체크를 위한 효율적인 데이터 구조 사용 (HashSet)
- **에러 처리**: 파일 로드 실패 시 기본값으로 동작하도록 구현
- **확장성**: 향후 다른 시간 오프셋이나 특별한 날짜 패턴 추가 가능하도록 설계
- **일관성**: 모든 시간 기반 로직에서 동일한 오프셋 적용 보장


