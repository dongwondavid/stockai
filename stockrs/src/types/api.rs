use std::error::Error;
use chrono::NaiveDateTime;
use crate::types::broker::Order;
use crate::types::trading::AssetInfo;
use crate::config::get_config;
use korea_investment_api::{KoreaInvestmentApi, types::{Environment, Account}};
use korea_investment_api::types::{Direction, OrderClass, Price, Quantity};

/// 실행 환경 타입 - prototype.py의 self.type과 동일
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApiType {
    Real,      // "real"
    Paper,     // "paper" 
    Backtest,  // "backtest"
}

/// 모든 API가 구현해야 하는 기본 trait
/// prototype.py의 real_api, paper_api, db_api가 동일한 인터페이스를 가지는 것처럼
pub trait StockApi {
    /// 주문 실행
    fn execute_order(&self, order: &Order) -> Result<String, Box<dyn Error>>;
    
    /// 주문 체결 확인  
    fn check_fill(&self, order_id: &str) -> Result<bool, Box<dyn Error>>;
    
    /// 주문 취소
    fn cancel_order(&self, order_id: &str) -> Result<(), Box<dyn Error>>;
    
    /// 잔고 조회 (한국투자증권 API 기준)
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>>;
    
    /// 평균가 조회 (data_reader 역할 통합)
    fn get_avg_price(&self, stockcode: &str) -> Result<f64, Box<dyn Error>>;
    
    /// 현재가 조회
    fn get_current_price(&self, stockcode: &str) -> Result<f64, Box<dyn Error>>;
}

/// API 팩토리 함수 - prototype.py의 조건부 생성 로직과 동일
pub fn create_api(api_type: ApiType, for_trading: bool) -> Box<dyn StockApi> {
    match (api_type, for_trading) {
        (ApiType::Real, true) => Box::new(RealApi),
        (ApiType::Paper, true) => Box::new(PaperApi::new()),
        (ApiType::Backtest, _) => Box::new(DbApi),
        (_, false) => Box::new(DbApi), // data-only operations
    }
}

/// 실제 한국투자증권 API 구현체
pub struct RealApi;

impl StockApi for RealApi {
    fn execute_order(&self, order: &Order) -> Result<String, Box<dyn Error>> {
        // TODO: 실제 한국투자증권 API 호출
        todo!("실제 API 주문 실행")
    }
    
    fn check_fill(&self, order_id: &str) -> Result<bool, Box<dyn Error>> {
        // TODO: 실제 API로 체결 확인
        todo!("실제 API 체결 확인")
    }
    
    fn cancel_order(&self, order_id: &str) -> Result<(), Box<dyn Error>> {
        // TODO: 실제 API로 주문 취소
        todo!("실제 API 주문 취소")
    }
    
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>> {
        // TODO: 실제 API로 잔고 조회
        todo!("실제 API 잔고 조회")
    }
    
    fn get_avg_price(&self, _stockcode: &str) -> Result<f64, Box<dyn Error>> {
        // TODO: 실제 API로 평균가 조회
        todo!("실제 API 평균가 조회")
    }
    
    fn get_current_price(&self, _stockcode: &str) -> Result<f64, Box<dyn Error>> {
        // TODO: 실제 API로 현재가 조회
        todo!("실제 API 현재가 조회")
    }
}

/// 모의투자 API 구현체
pub struct PaperApi {
    api: Option<KoreaInvestmentApi>,
}

impl PaperApi {
    pub fn new() -> Self {
        Self { api: None }
    }

    /// 한국투자증권 모의투자 API 초기화 (최초 사용 시)
    async fn ensure_api(&mut self) -> Result<&KoreaInvestmentApi, Box<dyn Error>> {
        if self.api.is_none() {
            let config = get_config()?;
            
            let account = Account {
                cano: config.korea_investment_api.paper_account_number.clone(),
                acnt_prdt_cd: config.korea_investment_api.paper_account_product_code.clone(),
            };
            
            let api = KoreaInvestmentApi::new(
                Environment::Virtual, // 모의투자
                &config.korea_investment_api.paper_app_key,
                &config.korea_investment_api.paper_app_secret,
                account,
                "HTS_ID", // TODO: config에서 읽기
                None, // token은 자동 생성
                None, // approval_key는 자동 생성
            ).await?;
            
            println!("🔗 [PaperApi] 모의투자 API 연결 완료");
            self.api = Some(api);
        }
        
        Ok(self.api.as_ref().unwrap())
    }
}

impl StockApi for PaperApi {
    fn execute_order(&self, order: &Order) -> Result<String, Box<dyn Error>> {
        // async 메서드를 sync context에서 호출하기 위해 tokio runtime 사용
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let mut api_wrapper = Self::new();
            let api = api_wrapper.ensure_api().await?;
            
            // Order 구조체를 korea-investment-api 파라미터로 변환
            let direction = match order.side {
                crate::types::broker::OrderSide::Buy => Direction::Bid,
                crate::types::broker::OrderSide::Sell => Direction::Ask,
            };
            
            let order_class = OrderClass::Market; // 시장가 주문 (향후 확장 가능)
            let quantity = Quantity::from(order.quantity);
            let price = Price::from(0); // 시장가는 0
            
            let result = api.order.order_cash(
                order_class,
                direction,
                &order.stockcode,
                quantity,
                price,
            ).await?;
            
            // 주문 번호 반환 - 올바른 메서드 사용
            let order_id = result.output()
                .as_ref()
                .map(|output| output.odno().clone())
                .unwrap_or_else(|| "UNKNOWN_ORDER".to_string());
                
            println!("📈 [PaperApi] 주문 실행 성공: {} {} {}주 -> 주문번호: {}", 
                     order.stockcode, 
                     match order.side { crate::types::broker::OrderSide::Buy => "매수", _ => "매도" },
                     order.quantity, 
                     order_id);
            
            Ok(order_id)
        })
    }
    
    fn check_fill(&self, order_id: &str) -> Result<bool, Box<dyn Error>> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let mut api_wrapper = Self::new();
            let api = api_wrapper.ensure_api().await?;
            
            // 주식일별주문체결조회로 체결 상태 확인
            // 오늘 날짜로 조회
            let today = chrono::Local::now().format("%Y%m%d").to_string();
            
            let result = api.order.inquire_daily_ccld(
                &today,           // inqr_strt_dt: 조회시작일자
                &today,           // inqr_end_dt: 조회종료일자  
                "",               // sll_buy_dvsn_cd: 매도매수구분코드 (공백: 전체)
                "",               // pdno: 상품번호 (공백: 전체)
                "",               // ord_gno_brno: 주문채번지점번호 
                order_id,         // odno: 주문번호
                "01",             // ccld_dvsn: 체결구분 (01: 체결)
                "00",             // inqr_dvsn: 조회구분
                "",               // inqr_dvsn_1: 조회구분1
                "",               // inqr_dvsn_3: 조회구분3
                "01",             // excg_id_dvsn_cd: 거래소ID구분코드
                None,             // ctx_area_fk100: 연속조회검색조건100
                None,             // ctx_area_nk100: 연속조회키100
            ).await?;
            
            // 체결 내역이 있으면 체결된 것으로 판단
            let is_filled = result.output1()
                .as_ref()
                .map(|output| !output.is_empty())
                .unwrap_or(false);
            
            println!("🔍 [PaperApi] 체결 확인: 주문번호 {} -> {}", 
                     order_id, 
                     if is_filled { "체결됨" } else { "미체결" });
            
            Ok(is_filled)
        })
    }
    
    fn cancel_order(&self, order_id: &str) -> Result<(), Box<dyn Error>> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let mut api_wrapper = Self::new();
            let api = api_wrapper.ensure_api().await?;
            
            // 주문 취소를 위해서는 원래 주문 정보가 필요
            // 일단 기본값으로 취소 시도 (실제로는 주문 정보를 조회해야 함)
            use korea_investment_api::types::CorrectionClass;
            
            let result = api.order.correct(
                OrderClass::Market,          // order_division
                "",                         // krx_fwdg_ord_orgno: KRX전송주문조직번호
                order_id,                   // orgn_odno: 원주문번호
                CorrectionClass::Cancel,    // rvse_cncl_dvsn_cd: 정정취소구분코드
                true,                       // qty_all_ord_yn: 잔량전부주문여부
                Quantity::from(0),          // qty: 주문수량 (취소시 0)
                Price::from(0),             // price: 주문가격
            ).await?;
            
            println!("❌ [PaperApi] 주문 취소 완료: {}", order_id);
            Ok(())
        })
    }
    
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let mut api_wrapper = Self::new();
            let api = api_wrapper.ensure_api().await?;
            
            let result = api.order.inquire_balance(
                "N",    // afhr_flpr_yn: 시간외단일가여부 (N: 기본값)
                "02",   // inqr_dvsn: 조회구분 (02: 종목별)
                "01",   // unpr_dvsn: 단가구분 (01: 기본값)
                "N",    // fund_sttl_icld_yn: 펀드결제분포함여부 (N: 미포함)
                "N",    // fncg_amt_auto_rdpt_yn: 융자금액자동상환여부 (N: 기본값)
                "00",   // prcs_dvsn: 처리구분 (00: 전일매매포함)
                None,   // ctx_area_fk100: 연속조회검색조건100
                None,   // ctx_area_nk100: 연속조회키100
            ).await?;
            
            // 응답에서 예수금 총액 추출
            let total_cash = result.output2()
                .as_ref()
                .and_then(|output2_vec| output2_vec.first())
                .map(|output2| output2.dnca_tot_amt())
                .and_then(|amt_str| amt_str.parse::<f64>().ok())
                .unwrap_or(0.0);
            
            println!("💰 [PaperApi] 잔고 조회 완료: 예수금 {}원", total_cash);
            
            // AssetInfo 생성
            use chrono::Local;
            Ok(AssetInfo::new(Local::now().naive_local(), total_cash))
        })
    }
    
    fn get_avg_price(&self, _stockcode: &str) -> Result<f64, Box<dyn Error>> {
        // TODO: 모의투자 API로 평균가 조회 (새 구조로 교체 예정)
        todo!("모의투자 API 평균가 조회")
    }
    
    fn get_current_price(&self, _stockcode: &str) -> Result<f64, Box<dyn Error>> {
        // TODO: 모의투자 API로 현재가 조회 (새 구조로 교체 예정)
        todo!("모의투자 API 현재가 조회")
    }
}

/// 백테스팅용 DB API 구현체
pub struct DbApi;

impl StockApi for DbApi {
    fn execute_order(&self, _order: &Order) -> Result<String, Box<dyn Error>> {
        // 더미 구현: 항상 성공하는 주문
        println!("🔹 [DbApi] Order executed successfully (simulated)");
        Ok("DUMMY_ORDER_123".to_string())
    }
    
    fn check_fill(&self, _order_id: &str) -> Result<bool, Box<dyn Error>> {
        // 더미 구현: 항상 체결됨
        println!("🔹 [DbApi] Order filled: {} (simulated)", _order_id);
        Ok(true)
    }
    
    fn cancel_order(&self, _order_id: &str) -> Result<(), Box<dyn Error>> {
        // 더미 구현: 항상 취소 성공
        println!("🔹 [DbApi] Order cancelled: {} (simulated)", _order_id);
        Ok(())
    }
    
    fn get_balance(&self) -> Result<AssetInfo, Box<dyn Error>> {
        // 더미 구현: 가상의 잔고 정보
        use chrono::Local;
        println!("🔹 [DbApi] Balance retrieved (simulated)");
        Ok(AssetInfo::new(Local::now().naive_local(), 1000000.0))
    }
    
    fn get_avg_price(&self, _stockcode: &str) -> Result<f64, Box<dyn Error>> {
        // TODO: DB에서 평균가 조회 (새 구조로 교체 예정)
        todo!("DB API 평균가 조회")
    }
    
    fn get_current_price(&self, _stockcode: &str) -> Result<f64, Box<dyn Error>> {
        // TODO: DB에서 현재가 조회 (새 구조로 교체 예정)
        todo!("DB API 현재가 조회")
    }
} 
