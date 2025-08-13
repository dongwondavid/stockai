# 📝 변경 이력 로그

2025-08-12T11:23:25+09:00: stockrs/src/utility/apis/korea_api.rs: 주문 실행 로그의 매수/매도 표기 오류 수정 및 잔고/평균가 조회에 EGW00201 발생 시 1초 대기 후 최대 3회 재시도 로직 추가
2025-08-12T11:29:45+09:00: stockrs/src/runner.rs: 실전/모의 모드에서 매 분마다 보류 주문 처리와 overview 갱신을 수행하도록 메인 루프에 주기적 업데이트 로직 추가 (process_pending, update_overview 호출)

2025-08-12T01:58: korea-investment-api/src/stock/order.rs: 잔고 조회(inquire_balance) 실패 시 디버그 출력 추가 - 요청 쿼리스트링과 응답 본문/HTTP 상태를 콘솔에 출력하여 "API 응답에서 잔고 정보를 찾을 수 없음" 오류 원인 분석 용이화

2025-01-27T16:25: stockrs/src/time.rs: 모드별 대기 로직 추가 - TradingMode import 추가, wait_until_next_event, handle_next_trading_day, handle_overnight_signal 메서드 구현, 백테스팅은 즉시 진행하고 실거래/모의투자는 실제 대기하는 로직 구현

2025-01-27T16:25: stockrs/src/runner.rs: 모드별 대기 로직을 time.rs로 이동 - wait_until_next_event 메서드에서 기존 조건부 로직을 time.rs의 새로운 메서드들(handle_next_trading_day, handle_overnight_signal) 사용하도록 리팩토링, 책임 분리 개선

2025-01-27T16:20: stockrs/src/model.rs: 명확한 분류 검증 완료 - runner, time, model, main, utility, broker, joonwoo 모듈에서 TradingMode와 ApiBundle current_mode 사용 현황 분석 완료, 모든 모듈에서 적절한 모드 분류가 구현되어 있음을 확인

2025-01-27T16:15: stockrs/src/model.rs: ApiBundle에 현재 모드 정보 추가 - current_mode 필드와 TradingMode 기반 완벽한 분류 시스템 구현, get_balance() 메서드를 현재 모드에 따라 정확한 API 호출하도록 개선, 편의 메서드들(is_backtest_mode, is_real_mode, is_paper_mode, get_current_api) 추가

2025-01-27T16:10: stockrs/src/model.rs: 모의투자 잔고 조회 API 수정 - ApiBundle::get_balance() 메서드에서 모의투자 모드일 때 db_api 대신 paper_api(KoreaApi 모의투자 API) 사용하도록 수정, 모의투자에서 BacktestApi 호출 오류 해결

2025-01-27T16:05: evalutor/score.py: Drawdown Duration 계산 오류 수정 - calculate_drawdown_duration 함수에 dates 매개변수 추가, drawdowns.index 대신 dates.iloc 사용하여 실제 날짜 객체로 기간 계산, AttributeError: 'int' object has no attribute 'days' 오류 해결

2025-01-27T16:00: evalutor/score.py: Drawdown Duration 지표 추가 - 각 드로우다운 기간의 지속 기간을 계산하고 최대값을 반환하는 calculate_drawdown_duration 함수 구현, 드로우다운 지표 출력 섹션에 Max Drawdown Duration 추가

2025-07-27T19:10: stockrs/src/model/onnx_predictor.rs: 예측 결과가 없을 때 에러 대신 None 반환하도록 수정 - predict_top_stock 함수 반환 타입을 StockrsResult<String>에서 StockrsResult<Option<String>>으로 변경, 예측 결과가 없을 때 Err 대신 Ok(None) 반환하여 에러 제거
2025-07-27T19:10: stockrs/src/model/joonwoo.rs: 예측 결과가 None일 때 매수하지 않도록 처리 개선 - try_entry 함수에서 predict_top_stock 결과가 None일 때 매수 주문 생성하지 않고 None 반환, 예측 결과가 없을 때도 정상적으로 처리
2025-07-27T19:10: stockrs/src/db_manager.rs: 거래가 없어도 안전하게 작동하는 overview 로직 개선 - finish_overview에서 COALESCE 사용하여 거래 기록 조회 시 NULL 처리, insert_overview와 update_overview에서 query_row 실패 시 unwrap_or(0) 사용, open 값 조회 실패 시 현재 자산으로 대체, high/low 값 조회 실패 시 현재 자산으로 초기화

2025-07-27T10:15: stockrs/src/model/onnx_predictor/features/day2.rs: calculate_volume_ratio_vs_prevday 함수 매개변수 수정 - db 매개변수 추가하여 5분봉 DB에서 get_morning_data 호출하도록 수정, daily_db 대신 db 사용하여 올바른 데이터베이스에서 당일 오전 거래량 조회
2025-07-27T10:15: stockrs/src/model/onnx_predictor/features.rs: day2_volume_ratio_vs_prevday 특징 호출 시 db 매개변수 추가 - calculate_volume_ratio_vs_prevday 함수 호출 시 db와 daily_db 모두 전달하도록 수정, 데이터베이스 매개변수 전달 오류 해결

2025-07-27T10:00: stockrs/src/model/onnx_predictor/features/utils.rs: get_daily_data 및 is_first_trading_day 함수에 상세 로깅 추가 - SQL 쿼리 문자열과 파라미터 출력, 데이터베이스 반환값 상세 로깅, 테이블 존재 여부 및 데이터 개수 확인 로그 추가, 사용자 제공 데이터와 실제 쿼리 결과 불일치 문제 디버깅을 위한 로깅 강화

2025-07-27T09:30: stockrs/src/model/onnx_predictor/features/day2.rs: 특징 계산 오류 분석 완료 - day2_volume_ratio_vs_prevday 특징에서 종목 A277810의 20230831(전일) 일봉 데이터 없음으로 인한 오류 발생 확인, 전일 데이터 의존적 특징들의 첫 거래일 처리 로직 개선 필요성 파악

2025-07-27T09:15: stockrs/src/model/onnx_predictor/features/day2.rs: 특징 계산 로깅 개선 - debug! 레벨을 info! 레벨로 변경하여 로그 가시성 향상, day2_volume_ratio_vs_prevday 특징에서 전일 데이터 없음 오류 발생 시 상세 로그 출력, 오류 발생 위치 정확히 파악 가능하도록 개선

2025-01-27T11:00: stockrs/src/model/joonwoo.rs: 고정 매수 금액 기능 구현 - 고정 금액 우선 매수 후 자금 부족 시 비율 기반 매수하는 로직으로 변경, fixed_entry_amount 필드 추가, 매수 로직 개선하여 고정 금액으로 매수할 수 없을 때 자동으로 비율 기반 매수로 전환
2025-01-27T11:00: stockrs/src/utility/config.rs: JoonwooConfig에 fixed_entry_amount 필드 추가 - 고정 매수 금액 설정을 위한 새로운 필드 추가, 설정 유효성 검증 로직 추가
2025-01-27T11:00: config.example.toml: joonwoo 섹션에 fixed_entry_amount 설정 추가 - 고정 매수 금액 설정 예시 추가 (기본값: 1,000,000원)

2025-07-26T21:00: stockrs/src/model/joonwoo.rs: 트레일링 스탑 로직 제거 및 전량 매도로 변경 - 절반 매도 후 잔여분 관리 구조를 제거하고 익절 시 한 번에 모든 포지션을 매도하도록 수정, PartialSold 상태와 highest_price_after_2pct 필드 제거, create_sell_half_order와 create_sell_remaining_order 함수 제거, trailing_stop_pct 설정 제거

2025-07-26T20:55: stockrs/src/model/onnx_predictor.rs: trading_dates 경로를 config에서 로드하도록 수정 - 하드코딩된 경로 제거하고 config.time_management.trading_dates_file_path 사용
2025-07-26T20:50: stockrs/src/model/onnx_predictor.rs: trading_dates를 1일봉 데이터에서 로드하도록 수정 - backtest_dates_1.txt 대신 samsung_1day_dates.txt 사용하여 전체 1일봉 거래일 활용
2025-07-26T20:43: stockrs/src/model/onnx_predictor.rs: 예측 결과가 없을 때 처리 개선 - 확률이 0.5 미만인 경우 매수하지 않도록 수정, 예측 실패 시 명확한 로그 메시지 추가
2025-07-26T20:43: stockrs/src/model/joonwoo.rs: 예측 실패 시 매수하지 않도록 처리 개선 - try_entry 함수에서 예측 실패 시 None 반환하여 매수 주문 생성하지 않음
2025-07-26T20:43: stockrs/src/db_manager.rs: 거래 기록이 없을 때 처리 개선 - finish_overview 함수에서 거래 기록이 없는 경우 기본값(0) 사용, 에러 대신 정상 처리

2025-01-27T10:35: config.example.toml: 자동 날짜 설정 옵션 추가 - auto_set_dates_from_file 설정 추가, trading_dates_file_path에서 자동으로 시작/종료 날짜 설정 가능하도록 기능 구현
2025-01-27T10:35: stockrs/src/utility/config.rs: TimeManagementConfig에 auto_set_dates_from_file 필드 추가 - 자동 날짜 설정 기능을 위한 새로운 필드 추가, is_valid_date_format 메서드 구현, 설정 유효성 검증 로직 개선

2025-07-26T20:22: stockrs/src/model/onnx_predictor.rs: ONNX 모델 출력 처리 로직 개선 - outputs[0]에서 클래스 정보 먼저 확인하여 클래스 0인 경우 결과에 추가하지 않도록 수정, 클래스 1인 경우에만 확률 추출 및 결과 추가
2025-07-26T18:14: TASK.md: onnx_predictor 간단한 버전으로 작성 작업 완료 - TASK.md에서 COMPLETE.md로 이동, 모든 작업 완료 확인
2025-07-26T18:14: COMPLETE.md: onnx_predictor 간단한 버전으로 작성 완료 작업 추가 - rust_model_info.json 삭제, extra_stocks.txt 대신 stocks.txt 사용, onnx 실행가능하게 만들기, config 정리 완료
2025-07-26T16:20: stockrs/src/model/onnx_predictor.rs: 필터링 후 15개 초과시 상위 15개만 사용하는 로직 추가 - final_stocks 변수 도입, take(15) 메서드로 순위 기반 제한, 디버그 로그 추가
2025-07-26T16:20: TASK.md: extra_stocks.txt 대신 stocks.txt 사용하는 로직으로 변경 작업 완료 - 필터링 후 15개 초과시 상위 15개만 사용하는 로직 추가 완료

2025-07-26T16:15: TASK.md: start1000.txt 날짜 기반 시스템 시작 시간 1시간 지연 기능 구현 완료 - TASK.md에서 COMPLETE.md로 이동, 모든 Phase 완료 확인
2025-07-26T16:10: TASK.md: start1000.txt 날짜 기반 시스템 시작 시간 1시간 지연 기능 구현 완료 - Phase 1-4 모든 단계 완료, 설정 시스템 확장부터 특징 추출 시간 범위 조정까지 전체 기능 구현 완료

2025-07-26T16:05: stockrs/src/time.rs: parse_time_string 함수 NaiveDateTime iterator 오류 수정 - date.and_time() 반환값 처리 로직 개선, 변수 할당 구조 정리하여 컴파일 오류 해결

2025-07-26T16:00: stockrs/src/time.rs: TimeService에 should_skip_to_next_trading_day 메서드 추가 - Runner의 중복된 주말/공휴일 체크 로직을 TimeService로 통합, 책임 분리 개선
2025-07-26T16:00: stockrs/src/runner.rs: HolidayChecker import 제거 및 TimeService 통합 로직 적용 - 중복된 holiday_checker 로직 제거, self.time.should_skip_to_next_trading_day() 사용으로 코드 간소화
2025-07-26T15:00: stockrs/src/utility/trading_calender.rs: TradingCalender 완전 재구현 - time.rs에서 사용하는 3개 함수만 남기고 holiday/weekend 내부 로직 모두 제거, samsung_1min_dates.txt 파일 기반 거래일 관리로 변경
2025-07-26T15:00: stockrs/src/time.rs: HolidayChecker를 TradingCalender로 완전 교체 - 모든 holiday_checker 참조를 trading_calender로 변경, 테스트 코드도 새로운 로직에 맞게 수정

2024-12-19T15:30: stockrs/src/utility/types/macros.rs: unwrap() 사용을 expect()로 개선 - LocalResult.single() 메서드 사용하여 안전한 시간 변환 구현
2024-12-19T15:30: stockrs/src/utility/types/trading.rs: unwrap() 사용을 expect()로 개선 - Default 구현에서 안전한 날짜/시간 생성
2024-12-19T15:30: stockrs/src/utility/holiday_checker.rs: 테스트 코드 unwrap() 사용을 expect()로 개선 - 모든 테스트에서 안전한 날짜 생성
2024-12-19T15:30: stockrs/src/time.rs: 테스트 코드 unwrap() 사용을 expect()로 개선 - LocalResult.single() 메서드 사용하여 안전한 시간 변환
2024-12-19T15:30: stockrs/src/utility/errors.rs: 테스트 코드 unwrap() 사용을 expect()로 개선 - 안전한 결과 처리
2024-12-19T15:30: stockrs/src/time.rs: 불필요한 변수 할당 제거 - update_cache 메서드에서 중간 변수 제거, compute_next_time 메서드에서 불필요한 변수 할당 제거
2024-12-19T15:30: stockrs/src/time.rs: wait_until 메서드 불필요한 변수 할당 제거 - Local::now() 직접 사용으로 최적화
2024-12-19T15:30: TASK.md: stockrs Clippy 경고 해결 및 코드 품질 개선 작업 완료 표시 - 모든 clippy 경고 해결, 코드 품질 개선 완료
2024-12-19T15:30: COMPLETE.md: stockrs Clippy 경고 해결 및 코드 품질 개선 작업 추가 - 완료된 작업 목록에 추가
2024-12-19T15:35: TODO.md: TASK.md 완료 내용 반영 - Clippy 경고 해결 작업 완료 표시, 코드 품질 개선 항목 업데이트

2025-07-21T13:45: TODO.md: korea-investment-api와 solomon 프로젝트 관련 항목 제거 (stockrs 프로젝트에만 집중)
2025-07-21T13:45: TASK.md: stockrs 프로젝트 Clippy 경고 해결 작업으로 범위 축소 (1개 경고만 해결)

2025-07-21T13:38: TODO.md: 시간 처리 로직 개선 완료 항목 체크 (8개 항목 완료)
2025-07-21T13:38: TASK.md: Clippy 경고 해결 작업 세부 조건 추가 (21개 경고 분석)
2025-07-21T13:38: COMPLETE.md: TODO/TASK 상태 업데이트 완료 작업 추가

2025-07-21T09:00: stockrs/src/time.rs: TimeService에 special_start_dates(HashSet) 및 오프셋 필드 추가, 파일 로드 및 is_special_start_date/parse_time_string/compute_next_time에서 오프셋 적용 로직 구현, 특별 날짜에만 시간 지연 반영

2025-07-21T08:20: config.example.toml: joonwoo 모델 전용 설정 섹션 추가 - 손절매/익절매/추가손절매 비율, 매수/강제정리 시간, 자산비율 설정 추가
2025-07-21T08:20: stockrs/src/config.rs: JoonwooConfig 구조체 추가 - joonwoo 모델 설정을 위한 새로운 구조체 정의, Config 구조체에 joonwoo 필드 추가, 유효성 검증 로직 추가
2025-07-21T08:20: stockrs/src/model/joonwoo.rs: 설정 기반 동작으로 변경 - 하드코딩된 값들을 config에서 로드하도록 수정, 시간 파싱 함수 추가, 설정값을 활용한 동적 시간 체크 로직 구현

2025-07-21T08:15: config.example.toml: 미사용 설정 제거 - RiskManagementConfig, ModelPredictionConfig, PerformanceConfig 섹션 전체 제거, LoggingConfig의 file_path, max_file_size, max_files 제거, TradingConfig의 take_profit_ratio, min_order_amount 제거, BacktestConfig의 transaction_tax_rate, securities_tax_rate 제거
2025-07-21T08:15: stockrs/src/config.rs: 미사용 설정 구조체 제거 - RiskManagementConfig, ModelPredictionConfig, PerformanceConfig 구조체 전체 제거, Config 구조체에서 해당 필드들 제거, 관련 유효성 검증 로직 제거, 테스트 코드에서 해당 필드들 제거

2025-07-21T08:00: stockrs/src/holiday_checker.rs: is_non_trading_day 메서드 시그니처 수정 - &self에서 &mut self로 변경하여 is_holiday 메서드 호출 가능하도록 수정
2025-07-21T08:00: stockrs/src/time.rs: TimeService의 holiday_checker 필드 활용 - next_trading_day, previous_trading_day, is_non_trading_day 메서드 추가, 기존 독립 함수 제거
2025-07-21T08:00: stockrs/src/time.rs: compute_next_time 메서드 수정 - next_trading_day 함수 호출을 임시 HolidayChecker 인스턴스 사용으로 변경
2025-07-21T08:00: stockrs/src/time.rs: skip_to_next_trading_day 메서드 수정 - next_trading_day 함수 호출을 self.next_trading_day로 변경
2025-07-21T08:00: stockrs/src/time.rs: 테스트 코드 수정 - next_trading_day 함수 호출을 HolidayChecker 인스턴스 사용으로 변경, weekday() 메서드 사용 제거
2025-07-21T08:00: stockrs/src/time.rs: 불필요한 import 제거 - Weekday, Datelike import 제거하여 경고 해결

2025-07-21T06:30: config.example.toml: 시장 시간 설정 섹션 추가 - market_hours 섹션에 data_prep_time, trading_start_time, trading_end_time, last_update_time, market_close_time 설정 추가
2025-07-21T06:30: stockrs/src/config.rs: MarketHoursConfig 구조체 추가 - 시장 시간 관련 설정을 위한 새로운 구조체 정의, Config 구조체에 market_hours 필드 추가
2025-07-21T06:30: stockrs/src/config.rs: TimeManagementConfig 구조체 수정 - trading_start_time, trading_end_time 필드 제거 (market_hours로 이동)
2025-07-21T06:30: stockrs/src/time.rs: TimeService 하드코딩된 시간 상수 제거 - compute_next_time 함수에서 설정 파일 기반 시간 사용, parse_time_string 헬퍼 함수 추가, compute_next_time_fallback 함수 추가
2025-07-21T06:30: stockrs/src/time.rs: skip_to_next_trading_day 함수 수정 - 설정 파일에서 거래 시작 시간 읽어오도록 변경

2025-07-21T06:45: stockrs/src/time.rs: TimeService에 시간 캐싱 메커니즘 추가 - cached_time, cache_timestamp, cache_duration 필드 추가, update_cache, invalidate_cache 메서드 구현
2025-07-21T06:45: stockrs/src/time.rs: now() 메서드 캐싱 로직 적용 - 캐시된 시간이 유효한 경우 사용, 백테스팅 모드에서 시간 단위 일관성 보장
2025-07-21T06:45: stockrs/src/time.rs: advance(), update(), skip_to_next_trading_day 메서드에서 캐시 업데이트 - 시간 변경 시 자동으로 캐시 갱신

2025-07-21T07:00: stockrs/src/holiday_checker.rs: HolidayChecker 모듈 생성 - 공휴일 체크 로직을 분리하고 모듈화, 캐싱 기능과 에러 처리 개선
2025-07-21T07:00: stockrs/src/lib.rs: holiday_checker 모듈 추가 - 새로운 HolidayChecker 모듈을 라이브러리에 포함
2025-07-21T07:00: stockrs/src/time.rs: TimeService에서 HolidayChecker 사용 - 기존 공휴일 관련 함수들 제거, HolidayChecker 인스턴스 추가, next_trading_day 함수 수정

2025-07-21T07:15: stockrs/src/time.rs: TimeService 일관된 에러 처리 적용 - new(), parse_time_string(), 생명주기 메서드들에서 StockrsError::Time 사용
2025-07-21T07:15: stockrs/src/holiday_checker.rs: HolidayChecker 일관된 에러 처리 적용 - HolidayCheckerError 제거, StockrsError::Time 사용, load_holidays_for_year, holiday_count_for_year 메서드 수정

2025-07-21T07:30: stockrs/src/time.rs: TimeService에 Duration 연산 헬퍼 함수들 추가 - add_minute, add_minutes, add_hours, add_days, subtract_* 함수들 및 diff_* 함수들 추가
2025-07-21T07:30: stockrs/src/time.rs: TimeService 내부 Duration 연산 중복 제거 - compute_next_time, compute_next_time_fallback 함수에서 add_minute() 헬퍼 함수 사용

2025-07-21T06:15: stockrs/src/time.rs: TimeService에 시간 포맷 변환 헬퍼 함수들 추가 - format_ymdhm, format_ymd, format_hms, format_iso_date, format_iso_datetime 및 정적 함수들 추가, Clone trait 구현
2025-07-21T06:15: stockrs/src/runner.rs: TimeService 헬퍼 함수 사용으로 변경 - format_ymdhm() 사용하여 중복된 포맷 변환 로직 제거, BacktestApi set_current_time 호출 제거
2025-07-21T06:15: stockrs/src/model/joonwoo.rs: TimeService 정적 헬퍼 함수 사용 - format_local_ymd, format_local_ymdhm 사용하여 포맷 변환 로직 통일
2025-07-21T06:15: stockrs/src/apis/backtest_api.rs: current_time 필드 제거 및 TimeService 직접 활용 - time_service 필드로 변경, set_current_time 메서드 제거, get_current_time을 TimeService 기반으로 변경
2025-07-21T06:15: TASK.md: 시간 처리 로직 개선 작업 완료 표시 - TimeService 포맷 통일 및 BacktestApi current_time 필드 제거 작업 완료
2025-07-21T05:33: TODO.md: DBManager 로직 수정 작업 완료 표시 - 4개 작업 모두 [x] 체크로 변경, TASK.md에 시간 처리 로직 개선 작업 추가
2025-07-21T05:26: stockrs/src/runner.rs: 백테스팅 시간 포맷 수정 - on_event, on_start, on_end, finish_overview 호출 시 "%H:%M:%S"에서 "%Y%m%d%H%M" 포맷으로 변경하여 분봉 DB 조회 성공 보장
2025-07-21T05:26: stockrs/src/db_manager.rs: ApiTypeDetector::calculate_balance_in_backtest 함수 수정 - 시간 파라미터를 실제로 사용하여 BacktestApi::calculate_balance_at_time 호출, 시간 기반 잔고 계산 정확성 확보
2025-07-21T05:26: stockrs/src/apis/db_api.rs: get_current_price_at_time 쿼리 파라미터 바인딩 확인 - 이미 올바르게 time_str 파라미터를 쿼리에 바인딩하고 있어 수정 불필요
2025-07-21T05:26: stockrs/src/broker.rs: 손절 계산 로직 확인 - joonwoo.rs에서 "%Y%m%d%H%M" 포맷 사용하여 일관성 확보, fallback 로직 정상 동작 확인

2025-07-20T17:10: stockrs/src/types/trading.rs: Clippy 경고 해결 - TradingResult::new 함수 제거 (11개 파라미터로 too_many_arguments 경고), Builder 패턴만 사용하도록 정리
2025-07-20T17:05: stockrs/src/apis/db_api.rs: 빌드 오류 해결 - StockrsError::insufficient_balance를 StockrsError::BalanceInquiry로 수정, 사용하지 않는 변수에 언더스코어 추가
2025-07-20T17:05: stockrs/src/model/onnx_predictor/features/utils.rs: 빌드 오류 해결 - 사용하지 않는 warn import 제거, answer_v3 테이블 없을 때 대체 로직 구현
2025-07-20T17:05: stockrs/src/apis/db_api.rs: Phase 1 성능 최적화 적용 - DB 인덱스 추가 (WAL 모드, 캐시 크기, 메모리 최적화), SQL 쿼리 최적화, 불필요한 로그 제거
2025-07-20T17:00: stockrs/src/model/onnx_predictor.rs: Phase 1 성능 최적화 적용 - 벡터 사전 할당 (Vec::with_capacity), 메모리 최적화, 불필요한 로그 제거
2025-07-20T17:00: stockrs/src/model/joonwoo.rs: Phase 1 성능 최적화 적용 - 현재가 조회 최적화, 불필요한 로그 제거, 메모리 최적화
2025-07-20T17:00: stockrs/src/model/onnx_predictor/features/utils.rs: Phase 1 성능 최적화 적용 - SQL 쿼리 최적화, 이진 탐색으로 거래일 검색 최적화, 벡터 사전 할당
2025-07-20T17:00: stockrs/src/runner.rs: Phase 1 성능 최적화 적용 - 조건문 최적화, 불필요한 로그 제거, 메모리 최적화

2025-07-20T16:55: stockrs/src/apis/db_api.rs: 불필요한 현재 시간 설정 완료 로그 제거 - 로그 출력량 최적화
2025-07-20T16:55: stockrs/src/runner.rs: 불필요한 현재 시간 설정 로그 제거 - 초기화 및 broker.on_start() 전 시간 설정 로그 정리

2025-07-20T16:50: stockrs/src/runner.rs: 백테스팅 모드에서 broker.on_start() 호출 전 현재 시간 설정 로직 추가 - "백테스팅 모드에서 현재 시간이 설정되지 않았습니다" 오류 근본 해결

2025-07-20T16:45: stockrs/src/runner.rs: API 인스턴스 구조 단순화 - 하나의 데이터 소스당 하나의 API 인스턴스만 생성하여 시간 설정 공유 문제 해결, db_api_direct 제거
2025-07-20T16:45: stockrs/src/model.rs: ApiBundle의 get_db_api() 메서드 수정 - db_api를 다운캐스팅하여 직접 접근하도록 변경
2025-07-20T16:45: stockrs/src/types/api.rs: StockApi trait에 as_any() 메서드 추가 - trait object에서 다운캐스팅을 위한 인터페이스 제공
2025-07-20T16:45: stockrs/src/apis/korea_api.rs: KoreaApi에 as_any() 메서드 구현 및 Any trait import 추가
2025-07-20T16:45: stockrs/src/apis/db_api.rs: DbApi에 as_any() 메서드 구현 및 Any trait import 추가

2025-07-20T16:30: stockrs/src/runner.rs: 백테스팅 모드 초기화 시 DbApi에 현재 시간 설정 로직 추가 - broker.on_start() 호출 전에 초기 시간을 설정하여 "현재 시간이 설정되지 않았습니다" 오류 해결

2025-07-20T16:19: stockrs/src/db_manager.rs: NewType 패턴을 활용한 근본적 오류 처리 개선 - DBResult, BacktestMode, ApiTypeDetector NewType 도입으로 컴파일 오류 해결
2025-07-20T16:19: stockrs/src/types/api.rs: StockApi trait에 get_balance_at_time 메서드 추가 - 백테스팅 모드에서 특정 시간 잔고 계산을 위한 안전한 인터페이스 제공
2025-07-20T16:19: stockrs/src/apis/db_api.rs: get_balance_at_time 메서드 구현 - calculate_balance_at_time을 trait 메서드로 노출
2025-07-20T16:19: stockrs/src/db_manager.rs: as_any() 다운캐스팅 문제 해결 - trait object에서 안전한 백테스팅 모드 처리 로직 구현
2025-07-20T16:19: stockrs/src/db_manager.rs: StockrsError와 rusqlite::Error 간 변환 문제 해결 - DBResult NewType으로 타입 안전성 확보

2025-07-20T08:30: stockrs/src/apis/db_api.rs: execute_backtest_order 함수 수정 - Order 객체의 fee 필드를 수수료 계산 후 업데이트하도록 수정
2025-07-20T08:30: stockrs/src/types/broker.rs: Broker trait의 execute 메서드 시그니처 수정 - order를 &mut로 받도록 변경
2025-07-20T08:30: stockrs/src/types/api.rs: StockApi trait의 execute_order 메서드 시그니처 수정 - order를 &mut로 받도록 변경
2025-07-20T08:30: stockrs/src/broker.rs: StockBroker의 execute 및 on_event 메서드 수정 - order를 &mut로 받도록 변경
2025-07-20T08:30: stockrs/src/apis/db_api.rs: DbApi의 execute_order 구현 수정 - order를 &mut로 받도록 변경
2025-07-20T08:30: stockrs/src/apis/korea_api.rs: KoreaApi의 execute_order 구현 수정 - order를 &mut로 받도록 변경
2025-07-20T08:30: stockrs/src/runner.rs: broker.on_event 호출 수정 - order를 &mut로 전달하도록 변경

2025-07-20T08:00: config.example.toml: 백테스팅 거래 비용 설정 추가 - 매수/매도 수수료율, 슬리피지율, 거래세율, 증권거래세율 설정 섹션 추가
2025-07-20T08:00: stockrs/src/config.rs: BacktestConfig 구조체 추가 - 백테스팅용 거래 비용 설정을 위한 새로운 설정 구조체 정의
2025-07-20T08:00: stockrs/src/config.rs: Config 구조체에 backtest 필드 추가 - 백테스팅 설정을 메인 설정에 포함
2025-07-20T08:00: stockrs/src/config.rs: 백테스팅 설정 유효성 검증 추가 - 수수료율, 슬리피지율, 세율 범위 검증 (0~10%)
2025-07-20T08:00: stockrs/src/apis/db_api.rs: 백테스팅 거래 비용 계산 로직 개선 - 설정 기반 수수료/슬리피지/세금 적용, 매수/매도별 차별화된 비용 계산
2025-07-20T08:00: stockrs/src/apis/db_api.rs: 백테스팅 거래 로깅 강화 - 수수료, 슬리피지, 거래세, 증권거래세 상세 정보 출력

2025-07-20T07:00: stockrs/src/broker.rs: 매도 주문 평균가 조회 로직 수정 - 주문 실행 전에 평균가를 미리 조회하여 매도 후 보유 종목에서 제거된 상태에서 평균가 조회 시 발생하는 오류 해결
2025-07-20T07:00: stockrs/src/db_manager.rs: save_trading 함수 시그니처 변경 - 평균가를 파라미터로 받도록 수정하여 매도 주문의 평균가 문제 해결
2025-07-20T07:00: stockrs/src/broker.rs: Trading 구조체 import 추가 - save_trading 함수에서 사용하기 위해 import

2025-07-20T06:30: stockrs/src/apis/db_api.rs: 백테스팅 시간 기반 가격 조회 수정 - get_current_price_from_db_latest() 함수 제거, calculate_balance()와 get_current_price() 함수를 시간 기반으로 동작하도록 수정, current_time 필드 추가 및 set_current_time() 메서드 구현
2025-07-20T06:30: stockrs/src/runner.rs: 백테스팅 모드에서 DbApi에 현재 시간 설정 로직 추가 - TimeService의 현재 시간을 DB 형식(%Y%m%d%H%M)으로 변환하여 DbApi.set_current_time() 호출

2025-07-20T06:30: stockrs/src/apis/db_api.rs: SQL 오류 처리 개선 - 실제 SQL 쿼리가 실행된 정확한 라인과 SQL 쿼리 자체를 출력하도록 수정 (get_current_price_from_db, get_current_price_from_db_latest, get_top_amount_stocks, debug_db_structure 함수)
2025-07-20T06:30: stockrs/src/model/onnx_predictor.rs: 백테스트 시 특이 거래일에 맞는 시간대를 넘기도록 수정 (get_time_range_for_date 사용)
2025-07-20T06:30: stockrs/src/utility/apis/db_api.rs: get_top_amount_stocks 함수가 시간대를 인자로 받아 유연하게 거래대금 계산이 가능하도록 리팩터링

2025-07-20T05:22: stockrs/src/runner.rs: 불필요한 로그 출력 제거 - 현재 시각, 공휴일/주말, 다음 거래일 이동, 객체 리셋 등 자주 출력되는 로그들 제거
2025-07-20T05:22: stockrs/src/model/joonwoo.rs: 불필요한 로그 출력 제거 - 매수 시도, 현재가 조회, 손익 체크 등 자주 출력되는 로그들 제거, 매수/매도 핵심 정보만 출력
2025-07-20T05:22: stockrs/src/apis/db_api.rs: 불필요한 로그 출력 제거 - 매수/매도 체결 로그 간소화, 디버그 로그들 제거, 핵심 거래 정보만 출력
2025-07-20T05:22: stockrs/src/apis/db_api.rs: 현재가 조회 로그 제거 - 정확한 시간 조회, 대체 조회, 데이터 발견 등 자주 출력되는 현재가 조회 로그들 모두 제거

2025-07-20T05:00: stockrs/src/apis/db_api.rs: fallback 패턴 제거 - unwrap_or, unwrap_or_else를 에러 발생 코드로 변경 (보유 수량 조회, 거래대금 조회 등)
2025-07-20T05:00: stockrs/src/db_manager.rs: fallback 패턴 제거 - fee_sum.unwrap_or(0.0), turnover_sum.unwrap_or(0.0)를 에러 발생 코드로 변경
2025-07-20T05:00: stockrs/src/time.rs: fallback 패턴 제거 - unwrap_or_else(|_| panic!())를 에러 발생 코드로 변경, TimeService::new() 반환 타입을 Result로 변경
2025-07-20T05:00: stockrs/src/runner.rs: fallback 패턴 제거 - unwrap_or_else(|_| panic!())를 에러 발생 코드로 변경, TimeService::new() 호출 수정
2025-07-20T05:00: stockrs/src/model/onnx_predictor.rs: fallback 패턴 제거 - sort_by에서 unwrap()을 에러 발생 코드로 변경
2025-07-20T05:00: stockrs/src/model/onnx_predictor/features/utils.rs: fallback 패턴 제거 - unwrap_or(0)을 에러 발생 코드로 변경 (테이블 존재 여부, 데이터 존재 여부 확인)
2025-07-20T05:00: stockrs/src/model/onnx_predictor/features/day2.rs: fallback 패턴 제거 - unwrap_or(0), warn!() + return Ok()를 에러 발생 코드로 변경
2025-07-20T05:00: stockrs/src/model/onnx_predictor/features/day3.rs: fallback 패턴 제거 - unwrap_or(), warn!() + return Ok()를 에러 발생 코드로 변경
2025-07-20T05:00: stockrs/src/model/onnx_predictor/features/day4.rs: fallback 패턴 제거 - unwrap_or(), warn!() + return Ok()를 에러 발생 코드로 변경
2025-07-20T05:00: stockrs/src/lib.rs: fallback 패턴 제거 - unwrap_or_else(|e| panic!())를 에러 발생 코드로 변경, init_tracing() 반환 타입을 Result로 변경
2025-07-20T05:00: stockrs/src/main.rs: init_tracing() 호출 수정 - 에러 처리 추가

2025-07-20T04:30: stockrs/src/apis/db_api.rs: 현재가 조회 로직 수정 - 정확한 시간 데이터 우선 조회, 대체 조회 로직 개선 (41150원 고정 문제 해결)
2025-07-20T04:30: stockrs/src/apis/db_api.rs: DB 구조 디버깅 함수 추가 (테이블 스키마, 샘플 데이터, 전체 개수 확인)
2025-07-20T04:30: stockrs/src/model/joonwoo.rs: 매수 시도 시 DB 구조 디버깅 추가 (현재가 조회 문제 파악)
2025-07-20T04:30: stockrs/src/model.rs: ApiBundle에서 DbApi 직접 접근 로직 수정 (as_any().downcast_ref 사용)

2025-07-20T04:15: stockrs/src/runner.rs: 공휴일 처리 로직 수정 - 2일씩 건너뛰는 문제 해결 (return 제거, 계속 진행하도록 수정)
2025-07-20T04:15: stockrs/src/runner.rs: 전체 실행 흐름 로깅 강화 (현재 시각, 시그널, 이벤트 처리 결과 상세 출력)
2025-07-20T04:15: stockrs/src/apis/db_api.rs: 현재가 조회 로깅 강화 (쿼리 실행, 조회 시간, 성공/실패 상세 로그)
2025-07-20T04:15: stockrs/src/broker.rs: 거래 실행 결과 로깅 강화 (성공/실패 시 상세 정보 출력)
2025-07-20T04:15: stockrs/src/runner.rs: broker 결과 처리 로직 개선 (성공 시에만 db_manager.on_event 호출)
2025-07-20T04:15: stockrs/src/db_manager.rs: on_event 함수 구현 개선 (overview 업데이트 로직 명확화)
2025-07-20T04:15: stockrs/src/model/joonwoo.rs: 현재가 조회 로깅 추가 (매수 시도, 손익 체크 시 상세 로그)
2025-07-20T04:15: TASK.md: 백테스팅 실행 로직 디버깅 및 수정 태스크로 변경 (발견된 문제점들 정리)

2025-07-20T03:41: stockrs/src/db_manager.rs: DBManager에서 TimeService 의존성 제거, 날짜를 매개변수로 받도록 수정 (unsafe 코드 제거, Rust 표준 준수)
2025-07-20T03:41: stockrs/src/runner.rs: Runner에서 DBManager 메서드 호출 시 현재 날짜 전달하도록 수정 (TimeService와 DBManager 분리)
2025-07-20T03:41: stockrs/src/time.rs: 사용하지 않는 import 제거 (Deref, Arc)
2025-07-20T03:33: stockrs/src/db_manager.rs: DBManager에 TimeService 의존성 추가 (overview 함수들에서 TimeService의 현재 날짜 사용)
2025-07-20T03:33: stockrs/src/time.rs: TimeServiceRef 래퍼 구조체 추가 (Arc<TimeService>를 위한 가변 메서드 접근)
2025-07-20T03:33: stockrs/src/runner.rs: Runner에서 TimeService를 Arc로 공유하고 TimeServiceRef 사용 (DBManager와 시간 동기화)
2025-07-20T03:07: stockrs/src/runner.rs: 매일 새로운 거래일 시작 시 모든 객체 리셋 로직 추가 (모델, 브로커, DB 매니저 상태 초기화)
2025-07-20T03:07: stockrs/src/db_manager.rs: reset_for_new_day 메서드 추가 (새로운 거래일을 위한 overview 데이터 초기화)
2025-07-20T03:07: stockrs/src/broker.rs: reset_for_new_day 메서드 추가 (새로운 거래일을 위한 브로커 상태 리셋)
2025-07-20T03:07: stockrs/src/model.rs: Model trait에 reset_for_new_day 메서드 추가 (매일 새로운 거래일을 위한 모델 상태 리셋)
2025-07-20T03:07: stockrs/src/model/joonwoo.rs: reset_for_new_day 메서드 구현 (모든 거래 상태 초기화, WaitingForEntry로 리셋)
2025-07-20T03:04: stockrs/src/time.rs: skip_to_next_trading_day에서 다음 거래일을 09:00으로 설정하도록 수정 (거래 로직 실행을 위한 시간 설정)
2025-07-20T03:02: stockrs/src/runner.rs: wait_until_next_event에서 Overnight 신호 시 skip_to_next_trading_day 호출하도록 수정 (무한 루프 문제 해결)
2025-07-20T03:00: stockrs/src/time.rs: TimeService::update 메서드 수정 (Overnight 신호에서 다음 거래일로 실제 이동하도록 개선, 무한 루프 문제 해결)
2025-07-20T02:15: stockrs/src/time.rs: load_holidays 함수에서 config 경로의 {} 플레이스홀더를 연도로 대체하는 로직 추가 (공휴일 파일 경로 오류 수정)
2025-07-20T02:10: stockrs/src/runner.rs: 공휴일/주말 체크 로직 추가 (거래 불가능한 날은 다음 거래일로 자동 넘어가기, TimeService.skip_to_next_trading_day 메서드 활용)
2025-07-20T02:10: stockrs/src/time.rs: skip_to_next_trading_day 메서드 추가 (공휴일/주말 건너뛰기 기능)
2025-07-20T02:10: stockrs/src/time.rs: is_weekend, is_holiday, load_holidays 함수를 pub으로 변경 (runner에서 접근 가능하도록)
2025-07-20T02:10: stockrs/src/apis/db_api.rs: 공휴일 체크 로직 제거 (runner에서 처리하므로 중복 제거)
2025-07-20T02:02: stockrs/src/apis/db_api.rs: 거래대금 조회 및 공휴일 체크 함수에 상세 로깅 추가 (디버깅 개선, 진행률 표시, 오류 상세 정보)
2025-07-19T23:30: stockrs/src/apis/db_api.rs: 공휴일 체크 로직을 파일 기반으로 개선 (하드코딩 → market_close_day_*.txt 파일 사용, fallback 제거하여 에러 발생)
2025-07-19T23:30: stockrs/src/apis/db_api.rs: is_holiday 메서드 추가 (파일에서 공휴일 목록 로드)
2025-07-19T23:30: stockrs/src/errors.rs: Box<dyn Error> 변환 시 중복 "오류:" 메시지 제거 로직 추가
2025-07-19T23:30: data/market_close_day_2025.txt: 2025년 공휴일 목록 생성 (설정 파일 경로와 일치)
2025-07-19T23:15: stockrs/src/time.rs: 공휴일 파일이 없을 때 에러 발생하도록 수정 (빈 벡터 반환 → panic)
2025-07-19T23:15: stockrs/src/time.rs: expect 호출들을 unwrap_or_else로 변경하여 더 명확한 에러 메시지 제공
2025-07-19T23:15: stockrs/src/runner.rs: expect 호출들을 unwrap_or_else로 변경하여 더 명확한 에러 메시지 제공
2025-07-19T23:15: stockrs/src/apis/db_api.rs: 공휴일/주말 체크 로직 추가 (거래대금 조회 시 에러 발생)
2025-07-19T23:15: stockrs/src/model/onnx_predictor.rs: extra_stocks.txt 파일이 없을 때 에러 발생하도록 수정 (경고 → 에러)
2025-07-26T17:23: stockrs/src/model/onnx_predictor.rs: extra_stocks.txt 대신 stocks.txt 사용하는 로직으로 완전 변경 (함수명, 필드명, 필터링 로직, 파일 읽기 로직 모두 변경)
2025-07-26T17:23: stockrs/src/utility/config.rs: OnnxModelConfig 구조체에서 extra_stocks_file_path → included_stocks_file_path로 변경
2025-07-26T17:23: config.example.toml: extra_stocks_file_path → included_stocks_file_path 설정 변경
2025-07-19T23:15: stockrs/src/lib.rs: expect 호출들을 unwrap_or_else로 변경하여 더 명확한 에러 메시지 제공
2025-07-19T23:15: stockrs/src/errors.rs: 테스트 코드의 panic을 assert로 변경
2025-07-19T23:20: stockrs/src/errors.rs: 에러 메시지 중복 문제 수정 (Box<dyn Error> 변환 시 StockrsError 중복 방지)
2025-07-19T22:45: stockrs/src/runner.rs: 백테스팅 end_date 체크 로직 추가 (wait_until_next_event에서 종료일 도달 시 에러 반환)
2025-07-19T22:45: stockrs/src/main.rs: 백테스팅 종료일 도달 에러를 정상 종료로 처리하도록 수정
2025-07-19T22:30: TASK.md: 백테스팅 잔고 관리 시스템 구현 태스크 추가
2025-07-19T22:30: stockrs/src/apis/db_api.rs: 백테스팅용 잔고 관리 기능 추가 (Holding 구조체, 주문 시뮬레이션, 잔고 계산)
2025-07-19T22:30: stockrs/src/broker.rs: 백테스팅 모드별 처리 로직 추가 (TradingMode 구분, 안전한 잔고 조회)
2025-07-19T22:30: stockrs/src/db_manager.rs: 백테스팅 모드에서 안전한 잔고 조회 처리 추가
2025-07-19T21:56: TODO.md: todogenerator 규칙에 따라 Phase 2 구현 필요 항목 및 Phase 3 고급 기능 체계적 추가
2025-07-19T21:05: 백테스팅: 모드 실행 검증 완료 (A204270 매수/매도 거래 성공, DB 저장 확인)
2025-07-19T21:05: stockrs/src/db_manager.rs: 매도 후 평균가 조회 시 panic 수정 (unwrap → match 패턴)
2025-07-19T21:05: TASK.md: 백테스팅 모드 실행 검증 작업 완료로 표시
2025-07-19T21:05: COMPLETE.md: 백테스팅 검증 완료 내역 상세 기록
2025-07-19T20:34: config.example.toml: 작동 기간 설정 추가 (start_date, end_date)
2025-07-19T20:34: stockrs/src/config.rs: TimeManagementConfig에 start_date, end_date 필드 추가
2025-07-19T20:34: stockrs/src/config.rs: 테스트 코드에 start_date, end_date 기본값 설정
2025-07-19T20:34: stockrs/src/main.rs: 실행 가능한 바이너리 생성 (Runner + joonwoo 모델)
2025-07-19T20:34: TODO.md: 모의투자 장기 계획 및 아이디어 체계적 정리
2025-07-19T20:34: TASK.md: 모의투자 개발 우선순위 실행 과제 10개 정의
2025-07-19T20:34: 프로젝트: 모의투자 개발 체계적 관리 시작 (프로젝트 관리 규칙 적용)
2025-07-19T21:34: korea-investment-api/src/types/mod.rs: todo!() 매크로 3개 제거하여 실제 에러 발생시키도록 수정
2025-07-19T21:34: korea-investment-api/src/types/stream/: 암호화 데이터 "None // TODO" 제거하여 실제 에러 발생시키도록 수정
2025-07-19T21:34: stockrs/src/apis/db_api.rs: 시뮬레이션/백테스팅 코드 제거, 주문 실행 관련 메서드는 에러 발생시키도록 변경
2025-07-19T21:34: stockrs/src/time.rs: TODO 주석 및 "임시 초기값", "시뮬레이션" 관련 주석 제거
2025-07-19T21:34: solomon/src/bin/analyze_high_break.rs: 테스트용 하드코딩 데이터 제거 (실행 시 panic 발생)
2025-07-19T21:49: solomon/Cargo.toml: log, env_logger 의존성 추가 (analyze_high_break.rs 컴파일 오류 해결)
2025-01-19 17:15:00: stockrs/src/apis/db_api.rs: 거래대금 계산 로깅 강화 (처음 5개 종목 상세 분석, 카테고리별 카운터, 상세 진행률 추가)
2025-01-19 17:16:00: stockrs/src/apis/db_api.rs: borrow of moved value 에러 수정 (stock_code.clone() 사용)
2025-01-19 17:17:00: stockrs/src/apis/db_api.rs: 테이블 스키마 확인 로깅 추가 (PRAGMA table_info, 샘플 데이터 출력)
2025-01-19 17:18:00: stockrs/src/apis/db_api.rs: column_count 에러 수정 및 stock_prices 테이블 제외 (실제 종목 테이블만 사용)
2025-01-19 17:19:00: stockrs/src/apis/db_api.rs: 코드 정리 (불필요한 로깅 제거, 핵심 기능만 유지)
2025-01-19 17:20:00: stockrs/src/apis/db_api.rs: predict_top_stocks.rs 구현을 그대로 적용 (검증된 로직 사용)
2024-12-19 15:30:00: stockrs/src/runner.rs: 장 종료 후 대기 모드 로그 메시지를 주석으로 변경
2024-12-19 15:30:00: stockrs/src/apis/db_api.rs: 백테스팅용 현재가 조회 로직을 1분봉 DB 사용하도록 수정 (get_current_price_from_db, get_current_price_from_db_latest 함수)
2024-12-19 15:30:00: stockrs/src/runner.rs: 새로운 거래일 시작 시 날짜 로깅 추가 (📅 새로운 거래일 시작, 🔄 객체 리셋 시작, ✅ 리셋 완료)

2025-07-20T05:50: data/market_close_day_2024.txt: 1월 2일 공휴일 제거 (실제로는 거래일이므로)
2025-07-20T05:50: stockrs/src/time.rs: next_trading_day 함수에 디버깅 로그 추가하여 1월 3일, 1월 4일 건너뛰기 문제 진단
2025-07-20T05:50: stockrs/src/time.rs: 디버깅 로그 제거 - 문제 해결 완료 (1월 2일이 잘못 공휴일로 등록되어 1월 3일, 1월 4일이 건너뛰어졌던 문제)
2025-07-20T06:15: stockrs/src/runner.rs: wait_until_next_event에서 공휴일/주말 체크와 Overnight 신호 처리를 통합 (중복 skip_to_next_trading_day 호출 문제 해결)

2024-12-19 15:30:00: stockrs/src/model.rs: fallback 처리 제거하고 에러 발생하도록 수정 (사용자 규칙 준수)
2024-12-19 15:35:00: stockrs/src/model.rs: InvalidOperation 에러 타입을 UnsupportedFeature로 수정 (컴파일 에러 해결)

2025-07-20 09:36: stockrs/src/apis/backtest_api.rs: 백테스팅 전용 API 모듈 생성 (잔고 관리 및 주문 시뮬레이션 전담)
2025-07-20 09:36: stockrs/src/apis.rs: backtest_api 모듈 export 추가
2025-07-20 09:36: stockrs/src/apis/db_api.rs: 잔고 관리 로직 제거, 데이터 조회 전담으로 리팩토링
2025-07-20 09:36: stockrs/src/broker.rs: get_api 메서드 추가 (BacktestApi 접근용)
2025-07-20 09:36: stockrs/src/runner.rs: 백테스팅 모드에서 BacktestApi 사용하도록 수정
2025-07-20 09:36: stockrs/src/model.rs: ApiBundle에 backtest_api 필드 추가, get_balance 메서드 구현
2025-07-20 09:36: stockrs/src/model/joonwoo.rs: apis.get_balance() 사용하도록 수정
2025-07-20 09:36: TODO.md: 백테스팅 아키텍처 리팩토링 작업 추가
2025-07-20 09:36: TASK.md: 백테스팅 아키텍처 리팩토링 태스크 추가

2025-07-21 05:21:24: TODO.md: 프로젝트 개선 계획 체계적 정리 (백테스팅 시스템, 시간 처리, DBManager, 코드 구조, 시스템 인프라, 예측 모델, 실전/모의투자, 기술 연구, UI, 보안/안정성)
2025-07-21 05:22:23: TODO.md: 사용자 제시 문제점 중심으로 재구성 (DBManager 로직, 시간 처리, 코드 구조 개선)
2025-07-21 05:24:41: TASK.md: DBManager 로직 수정 태스크 4개 구체적 작성 (시간 포맷 수정, 쿼리 바인딩, fallback 로직, 전체 경로 수정)
2025-07-21 06:02:28: TASK.md: TODO.md의 시간 처리 로직 개선 항목들을 상세한 TASK 형식으로 작성 (하드코딩된 시장 시간 상수 분리, now() 호출 일관성, 주말·공휴일 체크 모듈화, 시간 에러 처리 일관성, Duration 연산 중복 제거)
2024-12-19 15:30:00: TODO.md: BacktestApi current_time 필드 제거 항목 추가 (시간 관리 중복 해결)
2024-12-19 15:30:00: TODO.md: ONNX 모델 정합성 확인 섹션 추가 (solomon 프로젝트 재검토 포함)
2025-07-21 12:12:37: 프로젝트: 파일 구조 변경 리팩토링 완료 (stockrs/src/utility/apis/, stockrs/src/utility/types/, stockrs/src/model/onnx_predictor/features/ 구조로 모듈화)

2024-12-19 15:30: stockrs/src/utility/types.rs: 모듈 에러 확인 및 해결 - 실제로는 IDE 일시적 문제였음
2024-12-19 15:30: solomon/src/bin/analyze_high_break.rs: 불필요한 mut 키워드 제거
2024-12-19 15:30: solomon/src/bin/analyze_foreign_ratio.rs: 불필요한 mut 키워드 제거
2024-12-19 15:30: 전체 프로젝트 빌드 성공 - 모든 에러 및 경고 해결 완료

2025-07-20T05:30: stockrs/src/lib.rs: init_tracing 함수 제거 - 애플리케이션 초기화 함수를 lib.rs에서 main.rs로 이동
2025-07-20T05:30: stockrs/src/main.rs: init_tracing 함수 추가 - 라이브러리 API가 아닌 애플리케이션 초기화 함수를 main.rs에 배치

2025-07-21T08:30: stockrs/src/utility/config.rs: TimeManagementConfig 구조체 확장 - special_start_dates_file_path, special_start_time_offset_minutes 필드 추가, 설정 유효성 검증 로직 추가
2025-07-21T08:30: config.example.toml: 특별한 시작 시간 설정 섹션 추가 - special_start_dates_file_path, special_start_time_offset_minutes 설정 및 주석 추가
2025-07-21T08:30: stockrs/src/utility/config.rs: 환경 변수 오버라이드 로직 추가 - SPECIAL_START_DATES_FILE_PATH, SPECIAL_START_TIME_OFFSET_MINUTES 환경 변수 지원
2025-07-21T08:30: TASK.md: Phase 1 설정 시스템 확장 완료 체크 - config.example.toml 설정 추가, TimeManagementConfig 구조체 확장, 기본값 설정 및 로드 로직 구현 완료

2025-07-21T09:30: stockrs/src/model/joonwoo.rs: 특별한 날짜에 entry_time/force_close_time 오프셋 적용 - get_entry_time_for_today, get_force_close_time_for_today 헬퍼 추가, try_entry/force_close_all/on_event 등에서 오프셋 반영

2024-06-09: stockrs/src/model/onnx_predictor.rs, stockrs/src/utility/config.rs: rust_model_info.json 완전 제거 - ONNXModelInfo 구조체 삭제, model_file_path 직접 사용, 환경변수 ONNX_MODEL_FILE_PATH로 변경, 테스트 코드 수정

2025-01-27T10:30: config.example.toml: market_close_file_path 설정 제거 - deprecated된 HolidayChecker 관련 설정 삭제, TradingCalender로 완전 교체됨
2025-01-27T10:30: stockrs/src/utility/config.rs: market_close_file_path 필드 제거 - TimeManagementConfig에서 사용하지 않는 필드 삭제

2025-01-27T10:50: stockrs/src/utility/config.rs: auto_set_dates_from_file이 true일 때 trading_dates_file_path에서 시작/종료 날짜를 자동으로 읽어와 start_date, end_date에 반영하는 로직 구현

2024-12-19 15:30:00: features.txt - 특징 목록을 20개에서 10개로 변경하고 새로운 특징들 추가
2024-12-19 15:35:00: stockrs/src/model/onnx_predictor/features/day1.rs - 새로운 특징 함수들 정의 추가 (calculate_volume_ratio, calculate_vwap_position_ratio)
2024-12-19 15:35:00: stockrs/src/model/onnx_predictor/features/day2.rs - 새로운 특징 함수 정의 추가 (calculate_volume_ratio_vs_prevday)
2024-12-19 15:35:00: stockrs/src/model/onnx_predictor/features/day3.rs - 새로운 특징 함수 정의 추가 (calculate_morning_volume_ratio)
2024-12-19 15:35:00: stockrs/src/model/onnx_predictor/features/day4.rs - 새로운 특징 함수 정의 추가 (calculate_pos_vs_high_10d)
2024-12-19 15:35:00: stockrs/src/model/onnx_predictor/features.rs - 새로운 특징들의 매핑 추가

2024-12-19 15:40:00: stockrs/src/model/onnx_predictor/features/day1.rs - calculate_volume_ratio, calculate_vwap_position_ratio 함수 구현 완료
2024-12-19 15:40:00: stockrs/src/model/onnx_predictor/features/utils.rs - MorningData와 DailyData 구조체에 volumes 필드 및 관련 메서드들 추가, RSI 계산 함수 추가

2024-12-19 15:45:00: stockrs/src/model/onnx_predictor/features/day2.rs - calculate_volume_ratio_vs_prevday 함수 구현 완료

2024-12-19 15:50:00: stockrs/src/model/onnx_predictor/features/day3.rs - calculate_morning_volume_ratio 함수 구현 완료

2024-12-19 15:55:00: stockrs/src/model/onnx_predictor/features/day4.rs - calculate_pos_vs_high_10d 함수 구현 완료

2025-01-27 15:30:00: evalutor/score.py: README.md 명시 지표 완전 구현 - 소르티노 비율 계산 오류 수정, 회복 기간 계산 추가, 평균 보유 기간 계산 추가, 월별 무위험 이율 적용, 결과 출력 구조화

2025-01-27 15:35:00: evalutor/score.py: ROI 퍼센트 단위 수정 - overview와 trading 테이블의 roi 컬럼을 100으로 나누어 소수점으로 변환

2025-01-27 15:40:00: evalutor/score.py: 평균 보유 기간 계산을 위해 trading 테이블에서 stock_code 컬럼 추가 로드

2025-01-27 15:45:00: evalutor/score.py: 평균 보유 기간 계산 함수에서 stock_code 컬럼명 일치 수정 (stockcode → stock_code)

2025-01-27 15:50:00: evalutor/score.py: 데이터베이스 실제 컬럼명에 맞춰 stockcode로 통일 (SQL 쿼리와 함수 내부 로직 모두 stockcode 사용)

2025-01-27T16:00: evalutor/score.py: Drawdown Duration 지표 추가 - 각 드로우다운 기간의 지속 기간을 계산하고 최대값을 반환하는 calculate_drawdown_duration 함수 구현, 드로우다운 지표 출력 섹션에 Max Drawdown Duration 추가

2025-01-27T16:05: evalutor/score.py: Drawdown Duration 계산 오류 수정 - calculate_drawdown_duration 함수에 dates 매개변수 추가, drawdowns.index 대신 dates.iloc 사용하여 실제 날짜 객체로 기간 계산, AttributeError: 'int' object has no attribute 'days' 오류 해결

2025-01-27T16:10: evalutor/score.py: 평균 보유 기간 계산 로직 수정 - 순서대로 매수-매도 쌍을 매칭하도록 개선, 기존 로직은 첫 번째 매수에 가장 가까운 매도를 찾아서 잘못된 기간 계산, 수정 후 0.0일로 정확한 결과 도출

2025-01-27T16:15: evalutor/score.py: trading 테이블 time 컬럼 고려한 평균 보유 기간 계산 개선 - date와 time을 합쳐서 datetime 생성, 시간까지 고려한 정확한 보유 기간 계산, 결과 0.0123일(약 17.7분)로 정확한 시간 차이 반영

2025-01-27T16:20: evalutor/score.py: 평균 보유 기간 출력 형식 개선 - 1일보다 작을 때 시간 단위로 변환, 1시간보다 작을 때 분 단위로 변환하여 직관적인 표시 (17.7 minutes)

2024-12-19 15:30:00: evalutor/score.py: 승률 계산 로직 개선 - 매도 거래만 고려하도록 수정 (구매 거래는 수수료로 인한 손실이 필연적이므로 제외)

2025-07-29 06:31:55: TASK.md: OAuth 토큰 저장 시스템 구현 작업 추가 - config.example.toml 기반 토큰 관리 시스템 설계 및 구현 계획 수립

2025-07-29 06:45:00: config.example.toml: [token_management] 섹션 추가 - OAuth 토큰 관리 설정 (토큰 파일 경로, 자동 갱신, 백업 등) 정의

2025-07-29 06:47:00: stockrs/src/utility/config.rs: TokenManagementConfig 구조체 추가 - 토큰 관리 설정을 위한 새로운 설정 타입 정의

2025-07-29 06:50:00: stockrs/src/utility/token_manager.rs: 토큰 관리자 모듈 생성 - ApiToken, TokenData, TokenManager 구조체 구현 (OAuth 토큰 24시간 유효기간, 6시간 갱신 주기 고려)

2025-07-29 06:52:00: stockrs/src/utility.rs: token_manager 모듈 등록 - 새로운 토큰 관리 모듈을 utility 패키지에 추가

2025-07-29 07:15:00: korea-investment-api/src/types/response/auth.rs: TokenCreation 구조체에 access_token_token_expired 필드 추가 - OAuth 응답의 만료 시간 정보 저장

2025-07-29 07:18:00: korea-investment-api/src/auth.rs: Auth 구조체에 토큰 응답 정보 저장 필드 추가 - token_response, token_issued_at 필드 및 관련 메서드 구현

2025-07-29 07:25:00: stockrs/src/utility/apis/korea_api.rs: KoreaApi 생성자에 토큰 관리자 통합 - 저장된 토큰 우선 사용, 새 토큰 발급 시 자동 저장 로직 구현

2025-07-29 07:30:00: stockrs/Cargo.toml: chrono 의존성에 serde 기능 추가 - DateTime<Utc> 직렬화/역직렬화 지원

2025-07-29 07:35:00: stockrs/src/utility/token_manager.rs: 컴파일 오류 수정 - update_token 메서드 로직 개선 및 타입 안전성 강화

2024-12-19 15:30:00: TASK.md: OAuth 토큰 저장 시스템 구현 완료 상태 업데이트 (체크박스 [x]로 변경, 완료 조건 및 관련 모듈에 ✅ 표시 추가, 완료 상태 섹션 추가)
2024-12-19 15:35:00: COMPLETE.md: OAuth 토큰 저장 시스템 구현 완료 항목 추가 (2024-12-19 섹션에 상세 내용 포함)
2024-12-19 15:35:00: TASK.md: 완료된 OAuth 토큰 저장 시스템 제거 (COMPLETE.md로 이동 완료)
2024-12-19 15:40:00: TASK.md: 완료된 작업 내용 삭제 및 새로운 작업 대기 상태로 초기화
2024-12-19 15:40:00: TODO.md: 토큰 저장 시스템 완료 체크 및 다음 우선순위 작업들 추가 (모의투자/실전투자/예측모델/시스템인프라/성능최적화 카테고리별 정리)
2025-08-12 11:58:13: TASK.md: 주식 API 재시도 로직 구현 작업 추가 (목적/입출력/완료조건/관련 모듈 정리)
2025-08-12T12:48:52+09:00: stockrs/src/utility/apis/korea_api.rs: 재시도 로직 강화 - 지수 백오프(max 6s), 최대 5회 재시도로 상향. 잔고/평균가/현재가 조회 시 rt_cd!="0" 또는 핵심 output 비어있을 때 오류로 간주하여 공통 재시도 경로로 유도. 중복 수동 루프 제거로 모든 함수가 단일 재시도 헬퍼를 통해 동작
2025-08-12T13:05:00+09:00: stockrs/src/db_manager.rs: 모드 감지 로직을 API 타입 기반으로 엄격화 - `BacktestMode` 제거, `ApiTypeDetector::is_backtest()` 추가, `get_balance_with_context`로 통합하여 BacktestApi일 때만 시간 기반 잔고 계산 수행
2025-08-12T13:12:30+09:00: stockrs/src/broker.rs: 보류 주문 처리 개선 - 대기열이 0개면 로그 없이 즉시 반환, 체결 조회 오류 발생 시 즉시 에러 반환하고 큐 상태 보존하여 다음 주기에 재시도 가능하도록 변경
2025-08-13T11:02:37+09:00: stockrs/src/utility/apis/korea_api.rs: get_order_fill_info에서 예상치 못한 응답(rt_cd!="0", output1 누락, 파싱 실패)에 대해 에러를 반환하도록 변경. 주문번호에 해당 레코드가 아직 없을 때만 Ok(None) 유지
2025-08-13T11:30:36+09:00: TASK.md: KIS 토큰 만료 감지(EGW00123/"기간이 만료된 token") 시 재발급 및 1회 재시도 로직 구현 태스크 추가 (목적/입출력/완료조건/관련 모듈/설계 메모 정리)
2025-08-13T12:38:42+09:00: stockrs/src/utility/apis/korea_api.rs: 토큰 만료 자동 복구 구현 - 내부 API를 RefCell<Rc<...>>로 변경하여 재초기화 지원, 만료 감지 헬퍼/refresh_api_token/call_with_token_refresh 추가, 주문/잔고/현재가/체결/취소/체결상세 모든 호출에 1회 자동 재시도 적용
2025-08-13T12:52:45+09:00: stockrs/src/utility/apis/korea_api.rs: nested Runtime panic 수정 - refresh_api_token을 async로 변경하고 call_with_token_refresh에서 await하여 런타임 중첩 문제 해결
2025-08-13T13:13:01+09:00: TASK.md: 'KIS 토큰 만료 감지 후 재발급 및 1회 재시도 로직 추가' 태스크 완료 체크 및 완료 조건 상태 갱신
2025-08-13T13:13:01+09:00: COMPLETE.md: 완료 이력에 'KIS 토큰 만료 감지 후 재발급 및 1회 재시도 로직 추가' 항목 추가
2025-08-13T13:16:30+09:00: TODO.md: 'korea api 재시도 강제하는 로직 추가', '토큰 만료 시 재발급 로직 추가' 항목 완료 체크
