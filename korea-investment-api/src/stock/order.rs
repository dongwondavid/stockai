use crate::types::{
    request, response, Account, CorrectionClass, Direction, Environment, OrderClass, Price,
    Quantity, TrId,
};
use crate::{auth, Error};

#[derive(Clone)]
pub struct Korea {
    client: reqwest::Client,
    endpoint_url: String,
    environment: Environment,
    auth: auth::Auth,
    account: Account,
}

impl Korea {
    /// 국내 주식 주문에 관한 API
    /// [국내주식주문](https://apiportal.koreainvestment.com/apiservice/apiservice-domestic-stock#L_aade4c72-5fb7-418a-9ff2-254b4d5f0ceb)
    pub fn new(
        client: &reqwest::Client,
        environment: Environment,
        auth: auth::Auth,
        account: Account,
    ) -> Result<Self, Error> {
        let endpoint_url = match environment {
            Environment::Real => "https://openapi.koreainvestment.com:9443",
            Environment::Virtual => "https://openapivts.koreainvestment.com:29443",
        }
        .to_string();
        Ok(Self {
            client: client.clone(),
            endpoint_url,
            environment,
            auth,
            account,
        })
    }

    /// 주식주문(현금)[v1_국내주식-001]
    /// [Docs](https://apiportal.koreainvestment.com/apiservice/apiservice-domestic-stock#L_aade4c72-5fb7-418a-9ff2-254b4d5f0ceb)
    pub async fn order_cash(
        &self,
        order_division: OrderClass,
        order_direction: Direction,
        pdno: &str,
        qty: Quantity,
        price: Price,
    ) -> Result<response::stock::order::Body::Order, Error> {
        // KIS 요구사항: ORD_QTY/ORD_UNPR는 문자열, ORD_DVSN은 코드 문자열이어야 함
        let ord_dvsn_code: String = order_division.into();
        let qty_str: String = qty.into();
        let unpr_str: String = price.into();
        // 거래소ID구분코드: 모의투자(KRX), 실전(SOR)
        let excg_id = match self.environment {
            Environment::Real => "SOR",
            Environment::Virtual => "KRX",
        };
        let request = serde_json::json!({
            "CANO": self.account.cano,
            "ACNT_PRDT_CD": self.account.acnt_prdt_cd,
            "PDNO": pdno,
            "ORD_DVSN": ord_dvsn_code,
            "ORD_QTY": qty_str,
            "ORD_UNPR": unpr_str,
            "EXCG_ID_DVSN_CD": excg_id,
        }).to_string();
        let tr_id: String = match self.environment {
            Environment::Real => match order_direction {
                Direction::Bid => TrId::RealStockCashBidOrder.into(),
                Direction::Ask => TrId::RealStockCashAskOrder.into(),
            },
            Environment::Virtual => match order_direction {
                Direction::Bid => TrId::VirtualStockCashBidOrder.into(),
                Direction::Ask => TrId::VirtualStockCashAskOrder.into(),
            },
        };
        let hash = self.auth.get_hash(request.clone()).await?;
        let resp = self
            .client
            .post(format!(
                "{}/uapi/domestic-stock/v1/trading/order-cash",
                self.endpoint_url
            ))
            .header("Content-Type", "application/json; charset=UTF-8")
            .header(
                "Authorization",
                match self.auth.get_token() {
                    Some(token) => format!("Bearer {}", token),
                    None => {
                        return Err(Error::AuthInitFailed("token".to_string()));
                    }
                },
            )
            .header("appkey", self.auth.get_appkey())
            .header("appsecret", self.auth.get_appsecret())
            .header("tr_id", tr_id)
            .header("hashkey", hash)
            .header("custtype", "P")
            .body(request.clone())
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;
        let parsed: response::stock::order::Body::Order = serde_json::from_str(&text)?;

        // 주문 실패 추정 시, 요청/응답 전문 출력 (디버그용)
        if parsed.output().is_none() {
            println!("[KIS Order Debug] order-cash request body: {}", request);
            println!(
                "[KIS Order Debug] order-cash response (status={}): {}",
                status.as_u16(),
                text
            );
        }

        Ok(parsed)
    }

    // TODO: 주식주문(신용)[v1_국내주식-002]
    // [Docs](https://apiportal.koreainvestment.com/apiservice/apiservice-domestic-stock#L_f5769e4a-24d5-44f9-a2d8-232d45abf988)

    /// 주식주문(정정취소)[v1_국내주식-003] TODO: test
    /// [Docs](https://apiportal.koreainvestment.com/apiservice/apiservice-domestic-stock#L_4bfdfb2b-34a7-43f6-935a-e637724f960a)
    pub async fn correct(
        &self,
        order_division: OrderClass,
        krx_fwdg_ord_orgno: &str,
        orgn_odno: &str,
        rvse_cncl_dvsn_cd: CorrectionClass,
        qty_all_ord_yn: bool,
        qty: Quantity,
        price: Price,
    ) -> Result<response::stock::order::Body::Order, Error> {
        let request = request::stock::order::Body::Correction::new(
            self.account.cano.clone(),
            self.account.acnt_prdt_cd.clone(),
            krx_fwdg_ord_orgno.to_string(),
            orgn_odno.to_string(),
            order_division,
            rvse_cncl_dvsn_cd,
            qty,
            price,
            qty_all_ord_yn,
        )
        .get_json_string();
        let tr_id: String = match self.environment {
            Environment::Real => TrId::RealStockCorrection.into(),
            Environment::Virtual => TrId::VirtualStockCorrection.into(),
        };
        let hash = self.auth.get_hash(request.clone()).await?;
        Ok(self
            .client
            .post(format!(
                "{}/uapi/domestic-stock/v1/trading/order-rvsecncl",
                self.endpoint_url
            ))
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                match self.auth.get_token() {
                    Some(token) => token,
                    None => {
                        return Err(Error::AuthInitFailed("token".to_string()));
                    }
                },
            )
            .header("appkey", self.auth.get_appkey())
            .header("appsecret", self.auth.get_appsecret())
            .header("tr_id", tr_id)
            .header("hashkey", hash)
            .body(request)
            .send()
            .await?
            .json::<response::stock::order::Body::Order>()
            .await?)
    }

    /// 주식정정취소가능주문조회[v1_국내주식-004]
    /// [Docs](https://apiportal.koreainvestment.com/apiservice/apiservice-domestic-stock#L_d4537e9c-73f7-414c-9fb0-4eae3bc397d0)
    pub async fn inquire_psbl_rvsecncl(
        &self,
        inqr_dvsn_1: &str,
        inqr_dvsn_2: &str,
        ctx_area_fk100: Option<&str>,
        ctx_area_nk100: Option<&str>,
    ) -> Result<response::stock::order::Body::InquirePsblRvsecncl, Error> {
        use crate::types::request::stock::order::Body::InquirePsblRvsecncl as Param;
        let params = Param::new(
            self.account.cano.clone(),
            self.account.acnt_prdt_cd.clone(),
            ctx_area_fk100.map(|s| s.to_string()),
            ctx_area_nk100.map(|s| s.to_string()),
            inqr_dvsn_1.to_string(),
            inqr_dvsn_2.to_string(),
        );
        let tr_id = "TTTC0084R";
        let token = match self.auth.get_token() {
            Some(token) => format!("Bearer {}", token),
            None => return Err(Error::AuthInitFailed("token".to_string())),
        };
        let mut req = self.client.get(format!(
            "{}/uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl",
            "https://openapi.koreainvestment.com:9443"
        ));
        req = req
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Authorization", token)
            .header("appkey", self.auth.get_appkey())
            .header("appsecret", self.auth.get_appsecret())
            .header("tr_id", tr_id)
            .header("custtype", "P");
        for (k, v) in params.into_iter() {
            req = req.query(&[(k, v)]);
        }
        Ok(req
            .send()
            .await?
            .json::<response::stock::order::Body::InquirePsblRvsecncl>()
            .await?)
    }
    /// 주식일별주문체결조회[v1_국내주식-005]
    /// [Docs](https://apiportal.koreainvestment.com/apiservice/apiservice-domestic-stock#L_bc51f9f7-146f-4971-a5ae-ebd574acec12)
    #[allow(clippy::too_many_arguments)]
    pub async fn inquire_daily_ccld(
        &self,
        inqr_strt_dt: &str,
        inqr_end_dt: &str,
        sll_buy_dvsn_cd: &str,
        pdno: &str,
        ord_gno_brno: &str,
        odno: &str,
        ccld_dvsn: &str,
        inqr_dvsn: &str,
        inqr_dvsn_1: &str,
        inqr_dvsn_3: &str,
        excg_id_dvsn_cd: &str,
        ctx_area_fk100: Option<&str>,
        ctx_area_nk100: Option<&str>,
    ) -> Result<response::stock::order::Body::InquireDailyCcld, Error> {
        use crate::types::request::stock::order::Body::InquireDailyCcld as Param;
        let params = Param::new(
            self.account.cano.clone(),
            self.account.acnt_prdt_cd.clone(),
            inqr_strt_dt.to_string(),
            inqr_end_dt.to_string(),
            sll_buy_dvsn_cd.to_string(),
            pdno.to_string(),
            ord_gno_brno.to_string(),
            odno.to_string(),
            ccld_dvsn.to_string(),
            inqr_dvsn.to_string(),
            inqr_dvsn_1.to_string(),
            inqr_dvsn_3.to_string(),
            excg_id_dvsn_cd.to_string(),
            ctx_area_fk100.map(|s| s.to_string()),
            ctx_area_nk100.map(|s| s.to_string()),
        );
        let tr_id = match self.environment {
            Environment::Real => "TTTC0081R",
            Environment::Virtual => "VTTC0081R",
        };
        let token = match self.auth.get_token() {
            Some(token) => format!("Bearer {}", token),
            None => return Err(Error::AuthInitFailed("token".to_string())),
        };
        let mut req = self.client.get(format!(
            "{}/uapi/domestic-stock/v1/trading/inquire-daily-ccld",
            self.endpoint_url
        ));
        req = req
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Authorization", token)
            .header("appkey", self.auth.get_appkey())
            .header("appsecret", self.auth.get_appsecret())
            .header("tr_id", tr_id)
            .header("custtype", "P");
        for (k, v) in params.into_iter() {
            req = req.query(&[(k, v)]);
        }
        let response = req
            .send()
            .await?;
        
        // missing field 오류 디버깅을 위해 원본 응답을 먼저 text로 가져옴
        let response_text = response.text().await?;
        
        // JSON 파싱 시도
        match serde_json::from_str::<response::stock::order::Body::InquireDailyCcld>(&response_text) {
            Ok(result) => Ok(result),
            Err(e) => {
                // missing field 오류가 발생한 경우 원본 응답을 출력
                if e.to_string().contains("missing field") {
                    eprintln!("🔍 [KoreaInvestmentApi] inquire_daily_ccld missing field 오류 발생");
                    eprintln!("🔍 [KoreaInvestmentApi] 오류 상세: {}", e);
                    eprintln!("🔍 [KoreaInvestmentApi] 원본 응답 (raw):");
                    eprintln!("{}", response_text);
                }
                Err(e.into())
            }
        }
    }

    /// 주식잔고조회[v1_국내주식-006]
    /// [Docs](https://apiportal.koreainvestment.com/apiservice-apiservice?/uapi/domestic-stock/v1/trading/inquire-balance)
    pub async fn inquire_balance(
        &self,
        afhr_flpr_yn: &str,
        inqr_dvsn: &str,
        unpr_dvsn: &str,
        fund_sttl_icld_yn: &str,
        fncg_amt_auto_rdpt_yn: &str,
        prcs_dvsn: &str,
        ctx_area_fk100: Option<&str>,
        ctx_area_nk100: Option<&str>,
    ) -> Result<crate::types::response::stock::balance::BalanceResponse, Error> {
        use crate::types::request::stock::balance::BalanceParameter;
        let params = BalanceParameter::new(
            self.account.cano.clone(),
            self.account.acnt_prdt_cd.clone(),
            afhr_flpr_yn.to_string(),
            inqr_dvsn.to_string(),
            unpr_dvsn.to_string(),
            fund_sttl_icld_yn.to_string(),
            fncg_amt_auto_rdpt_yn.to_string(),
            prcs_dvsn.to_string(),
            ctx_area_fk100.map(|s| s.to_string()),
            ctx_area_nk100.map(|s| s.to_string()),
        );
        let tr_id = match self.environment {
            Environment::Real => "TTTC8434R",
            Environment::Virtual => "VTTC8434R",
        };
        let token = match self.auth.get_token() {
            Some(token) => format!("Bearer {}", token),
            None => return Err(Error::AuthInitFailed("token".to_string())),
        };
        let mut req = self.client.get(format!(
            "{}/uapi/domestic-stock/v1/trading/inquire-balance",
            self.endpoint_url
        ));
        req = req
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Authorization", token)
            .header("appkey", self.auth.get_appkey())
            .header("appsecret", self.auth.get_appsecret())
            .header("tr_id", tr_id)
            .header("custtype", "P");

        // Build query and keep a debug string of the request parameters
        let query_pairs = params.into_iter();
        for (k, v) in query_pairs.clone().into_iter() {
            req = req.query(&[(k, v.clone())]);
        }
        let debug_query = query_pairs
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        // Send request and read raw response text for robust error logging
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await?;

        // Try to parse JSON response; on parse error, dump debug and return
        match serde_json::from_str::<crate::types::response::stock::balance::BalanceResponse>(&text)
        {
            Ok(parsed) => {
                // On suspected error state, print request and response bodies for debugging
                let missing_output2 = parsed
                    .output2()
                    .as_ref()
                    .map(|v| v.is_empty())
                    .unwrap_or(true);
                if parsed.rt_cd() != "0" || missing_output2 {
                    println!(
                        "[KIS Balance Debug] request: GET {}/uapi/domestic-stock/v1/trading/inquire-balance?{}",
                        self.endpoint_url, debug_query
                    );
                    println!(
                        "[KIS Balance Debug] response (status={}): {}",
                        status.as_u16(), text
                    );
                }
                Ok(parsed)
            }
            Err(e) => {
                println!(
                    "[KIS Balance Debug] request: GET {}/uapi/domestic-stock/v1/trading/inquire-balance?{}",
                    self.endpoint_url, debug_query
                );
                println!(
                    "[KIS Balance Debug] response (status={}): {}",
                    status.as_u16(), text
                );
                Err(e.into())
            }
        }
    }

    /// 매수가능조회[v1_국내주식-007]
    /// [Docs](https://apiportal.koreainvestment.com/apiservice/apiservice-domestic-stock#L_806e407c-3082-44c0-9d71-e8534db5ad54)
    pub async fn inquire_psbl_order(
        &self,
        pdno: &str,
        ord_unpr: &str,
        ord_dvsn: &str,
        cma_evlu_amt_icld_yn: &str,
        ovrs_icld_yn: &str,
    ) -> Result<response::stock::order::Body::InquirePsblOrder, Error> {
        use crate::types::request::stock::order::Body::InquirePsblOrder;
        let params = InquirePsblOrder::new(
            self.account.cano.clone(),
            self.account.acnt_prdt_cd.clone(),
            pdno.to_string(),
            ord_unpr.to_string(),
            ord_dvsn.to_string(),
            cma_evlu_amt_icld_yn.to_string(),
            ovrs_icld_yn.to_string(),
        );
        let tr_id = match self.environment {
            Environment::Real => "TTTC8908R",
            Environment::Virtual => "VTTC8908R",
        };
        let token = match self.auth.get_token() {
            Some(token) => format!("Bearer {}", token),
            None => return Err(Error::AuthInitFailed("token".to_string())),
        };
        let mut req = self.client.get(format!(
            "{}/uapi/domestic-stock/v1/trading/inquire-psbl-order",
            self.endpoint_url
        ));
        req = req
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Authorization", token)
            .header("appkey", self.auth.get_appkey())
            .header("appsecret", self.auth.get_appsecret())
            .header("tr_id", tr_id)
            .header("custtype", "P");
        for (k, v) in params.into_iter() {
            req = req.query(&[(k, v)]);
        }
        let resp = req
            .send()
            .await?
            .json::<response::stock::order::Body::InquirePsblOrder>()
            .await?;
        Ok(resp)
    }
}
