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
        let request = request::stock::order::Body::Order::new(
            self.account.cano.clone(),
            self.account.acnt_prdt_cd.clone(),
            pdno.to_string(),
            order_division,
            qty,
            price,
        )
        .get_json_string();
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
        Ok(self
            .client
            .post(format!(
                "{}/uapi/domestic-stock/v1/trading/order-cash",
                self.endpoint_url
            ))
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                match self.auth.get_token() {
                    Some(token) => format!("Bearer {}", token),
                    None => {
                        return Err(Error::AuthInitFailed("token"));
                    }
                },
            )
            .header("appkey", self.auth.get_appkey())
            .header("appsecret", self.auth.get_appsecret())
            .header("tr_id", tr_id)
            .header("hashkey", hash)
            .header("custtype", "P")
            .body(request)
            .send()
            .await?
            .json::<response::stock::order::Body::Order>()
            .await?)
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
                        return Err(Error::AuthInitFailed("token"));
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
            None => return Err(Error::AuthInitFailed("token")),
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
            None => return Err(Error::AuthInitFailed("token")),
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
        Ok(req
            .send()
            .await?
            .json::<response::stock::order::Body::InquireDailyCcld>()
            .await?)
    }

    /// 주식잔고조회[v1_국내주식-006]
    /// [Docs](https://apiportal.koreainvestment.com/apiservice/apiservice-domestic-stock#L_66c61080-674f-4c91-a0cc-db5e64e9a5e6)
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
            None => return Err(Error::AuthInitFailed("token")),
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
        for (k, v) in params.into_iter() {
            req = req.query(&[(k, v)]);
        }
        let resp = req
            .send()
            .await?
            .json::<crate::types::response::stock::balance::BalanceResponse>()
            .await?;
        Ok(resp)
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
            None => return Err(Error::AuthInitFailed("token")),
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
