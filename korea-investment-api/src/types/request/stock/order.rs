use crate::types::{CustomerType, TrId};
use getset::{Getters, Setters};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Getters, Setters, Serialize, Deserialize)]
pub struct Header {
    #[getset(get = "pub", set = "pub")]
    authorization: String,
    #[getset(get = "pub", set = "pub")]
    appkey: String,
    #[getset(get = "pub", set = "pub")]
    appsecret: String,
    //#[getset(get = "pub", set = "pub")]
    // personalseckey: String // TODO: 법인용
    #[getset(get = "pub", set = "pub")]
    tr_id: TrId,
    #[getset(get = "pub", set = "pub")]
    custtype: CustomerType,
}

impl Header {
    pub fn new(token: String, appkey: String, appsecret: String, tr_id: TrId) -> Self {
        Self {
            authorization: token,
            appkey,
            appsecret,
            tr_id,
            custtype: CustomerType::Personal,
        }
    }
}

#[allow(non_snake_case)]
pub mod Body {
    use crate::types::{CorrectionClass, OrderClass, Price, Quantity};
    use getset::{Getters, Setters};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Getters, Setters, Serialize, Deserialize)]
    #[serde(rename_all = "UPPERCASE")]
    pub struct Order {
        #[getset(get = "pub", set = "pub")]
        /// 종합계좌번호(계좌번호 체계(8-2)의 앞 8자리)
        cano: String,
        #[getset(get = "pub", set = "pub")]
        /// 계좌상품코드(계좌번호 체계(8-2)의 뒤 2자리)
        acnt_prdt_cd: String,
        #[getset(get = "pub", set = "pub")]
        /// 종목코드(6자리)
        pdno: String,
        #[getset(get = "pub", set = "pub")]
        /// 주문구분
        ord_dvsn: OrderClass,
        #[getset(get = "pub", set = "pub")]
        /// 주문수량(주문주식수)
        ord_qty: Quantity,
        #[getset(get = "pub", set = "pub")]
        /// 주문단가(1주당 가격; 시장가는 0으로)
        ord_unpr: Price,
    }

    impl Order {
        pub fn new(
            cano: String,
            acnt_prdt_cd: String,
            pdno: String,
            ord_dvsn: OrderClass,
            ord_qty: Quantity,
            ord_unpr: Price,
        ) -> Self {
            Self {
                cano,
                acnt_prdt_cd,
                pdno,
                ord_dvsn,
                ord_qty,
                ord_unpr,
            }
        }
        pub fn get_json_string(self) -> String {
            serde_json::json!(self).to_string()
        }
    }

    #[derive(Debug, Clone, PartialEq, Getters, Setters, Serialize, Deserialize)]
    #[serde(rename_all = "UPPERCASE")]
    pub struct Correction {
        /// 종합계좌번호(계좌번호 체계(8-2)의 앞 8자리)
        #[getset(get = "pub", set = "pub")]
        cano: String,
        /// 계좌상품코드(계좌번호 체계(8-2)의 뒤 2자리)
        #[getset(get = "pub", set = "pub")]
        acnt_prdt_cd: String,
        /// 한국거래소전송주문조직번호(주문시 한국투자증권 시스템에서
        /// 지정된 영업점코드)
        #[getset(get = "pub", set = "pub")]
        krx_fwdg_ord_orgno: String,
        /// 원주문번호(주식일별주문체결조회 API output1의 odno(주문번호) 값 입력.
        /// 주문시 한국투자증권 시스템에서 채번된 주문번호)
        #[getset(get = "pub", set = "pub")]
        orgn_odno: String,
        /// 주문구분
        #[getset(get = "pub", set = "pub")]
        ord_dvsn: OrderClass,
        /// 정정취소구분코드
        #[getset(get = "pub", set = "pub")]
        rvse_cncl_dvsn_cd: CorrectionClass,
        /// 주문수량(주문주식수)
        #[getset(get = "pub", set = "pub")]
        ord_qty: Quantity,
        /// 주문단가([정정] 정정주문 1주당 가격, [취소] "0")
        #[getset(get = "pub", set = "pub")]
        ord_unpr: Price,
        /// 잔량전부주문여부([정정/취소] Y: 잔량전부, N: 잔량일부)
        #[getset(get = "pub", set = "pub")]
        qty_all_ord_yn: bool,
    }
    impl Correction {
        pub fn new(
            cano: String,
            acnt_prdt_cd: String,
            krx_fwdg_ord_orgno: String,
            orgn_odno: String,
            ord_dvsn: OrderClass,
            rvse_cncl_dvsn_cd: CorrectionClass,
            ord_qty: Quantity,
            ord_unpr: Price,
            qty_all_ord_yn: bool,
        ) -> Self {
            Self {
                cano,
                acnt_prdt_cd,
                krx_fwdg_ord_orgno,
                orgn_odno,
                ord_dvsn,
                rvse_cncl_dvsn_cd,
                ord_qty,
                ord_unpr,
                qty_all_ord_yn,
            }
        }
        pub fn get_json_string(self) -> String {
            serde_json::json!(self).to_string()
        }
    }

    /// 매수가능조회 Query Parameter
    #[derive(Debug, Clone, Getters, Setters, Serialize)]
    #[serde(rename_all = "UPPERCASE")]
    pub struct InquirePsblOrder {
        /// 종합계좌번호 (계좌번호 체계(8-2)의 앞 8자리)
        #[getset(get = "pub", set = "pub")]
        cano: String,
        /// 계좌상품코드 (계좌번호 체계(8-2)의 뒤 2자리)
        #[getset(get = "pub", set = "pub")]
        acnt_prdt_cd: String,
        /// 상품번호 (종목번호(6자리))
        #[getset(get = "pub", set = "pub")]
        pdno: String,
        /// 주문단가 (1주당 가격, 시장가 시 공란)
        #[getset(get = "pub", set = "pub")]
        ord_unpr: String,
        /// 주문구분 (00:지정가, 01:시장가 등)
        #[getset(get = "pub", set = "pub")]
        ord_dvsn: String,
        /// CMA평가금액포함여부 (Y:포함, N:포함하지않음)
        #[getset(get = "pub", set = "pub")]
        cma_evlu_amt_icld_yn: String,
        /// 해외포함여부 (Y:포함, N:포함하지않음)
        #[getset(get = "pub", set = "pub")]
        ovrs_icld_yn: String,
    }

    impl InquirePsblOrder {
        pub fn new(
            cano: String,
            acnt_prdt_cd: String,
            pdno: String,
            ord_unpr: String,
            ord_dvsn: String,
            cma_evlu_amt_icld_yn: String,
            ovrs_icld_yn: String,
        ) -> Self {
            Self {
                cano,
                acnt_prdt_cd,
                pdno,
                ord_unpr,
                ord_dvsn,
                cma_evlu_amt_icld_yn,
                ovrs_icld_yn,
            }
        }

        /// Query parameter로 변환
        pub fn into_iter(&self) -> Vec<(&'static str, String)> {
            vec![
                ("CANO", self.cano.clone()),
                ("ACNT_PRDT_CD", self.acnt_prdt_cd.clone()),
                ("PDNO", self.pdno.clone()),
                ("ORD_UNPR", self.ord_unpr.clone()),
                ("ORD_DVSN", self.ord_dvsn.clone()),
                ("CMA_EVLU_AMT_ICLD_YN", self.cma_evlu_amt_icld_yn.clone()),
                ("OVRS_ICLD_YN", self.ovrs_icld_yn.clone()),
            ]
        }
    }
    /// 주식정정취소가능주문조회 Query Parameter
    #[derive(Debug, Clone, Getters, Setters, Serialize)]
    #[serde(rename_all = "UPPERCASE")]
    pub struct InquirePsblRvsecncl {
        #[getset(get = "pub", set = "pub")]
        pub cano: String,
        #[getset(get = "pub", set = "pub")]
        pub acnt_prdt_cd: String,
        #[getset(get = "pub", set = "pub")]
        pub ctx_area_fk100: Option<String>,
        #[getset(get = "pub", set = "pub")]
        pub ctx_area_nk100: Option<String>,
        #[getset(get = "pub", set = "pub")]
        pub inqr_dvsn_1: String,
        #[getset(get = "pub", set = "pub")]
        pub inqr_dvsn_2: String,
    }

    impl InquirePsblRvsecncl {
        pub fn new(
            cano: String,
            acnt_prdt_cd: String,
            ctx_area_fk100: Option<String>,
            ctx_area_nk100: Option<String>,
            inqr_dvsn_1: String,
            inqr_dvsn_2: String,
        ) -> Self {
            Self {
                cano,
                acnt_prdt_cd,
                ctx_area_fk100,
                ctx_area_nk100,
                inqr_dvsn_1,
                inqr_dvsn_2,
            }
        }

        pub fn into_iter(&self) -> Vec<(&'static str, String)> {
            vec![
                ("CANO", self.cano.clone()),
                ("ACNT_PRDT_CD", self.acnt_prdt_cd.clone()),
                (
                    "CTX_AREA_FK100",
                    self.ctx_area_fk100.clone().unwrap_or_default(),
                ),
                (
                    "CTX_AREA_NK100",
                    self.ctx_area_nk100.clone().unwrap_or_default(),
                ),
                ("INQR_DVSN_1", self.inqr_dvsn_1.clone()),
                ("INQR_DVSN_2", self.inqr_dvsn_2.clone()),
            ]
        }
    }

    /// 주식일별주문체결조회 Query Parameter
    #[derive(Debug, Clone, Getters, Setters, Serialize)]
    #[serde(rename_all = "UPPERCASE")]
    pub struct InquireDailyCcld {
        #[getset(get = "pub", set = "pub")]
        pub cano: String,
        #[getset(get = "pub", set = "pub")]
        pub acnt_prdt_cd: String,
        #[getset(get = "pub", set = "pub")]
        pub inqr_strt_dt: String,
        #[getset(get = "pub", set = "pub")]
        pub inqr_end_dt: String,
        #[getset(get = "pub", set = "pub")]
        pub sll_buy_dvsn_cd: String,
        #[getset(get = "pub", set = "pub")]
        pub pdno: String,
        #[getset(get = "pub", set = "pub")]
        pub ord_gno_brno: String,
        #[getset(get = "pub", set = "pub")]
        pub odno: String,
        #[getset(get = "pub", set = "pub")]
        pub ccld_dvsn: String,
        #[getset(get = "pub", set = "pub")]
        pub inqr_dvsn: String,
        #[getset(get = "pub", set = "pub")]
        pub inqr_dvsn_1: String,
        #[getset(get = "pub", set = "pub")]
        pub inqr_dvsn_3: String,
        #[getset(get = "pub", set = "pub")]
        pub excg_id_dvsn_cd: String,
        #[getset(get = "pub", set = "pub")]
        pub ctx_area_fk100: Option<String>,
        #[getset(get = "pub", set = "pub")]
        pub ctx_area_nk100: Option<String>,
    }

    impl InquireDailyCcld {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            cano: String,
            acnt_prdt_cd: String,
            inqr_strt_dt: String,
            inqr_end_dt: String,
            sll_buy_dvsn_cd: String,
            pdno: String,
            ord_gno_brno: String,
            odno: String,
            ccld_dvsn: String,
            inqr_dvsn: String,
            inqr_dvsn_1: String,
            inqr_dvsn_3: String,
            excg_id_dvsn_cd: String,
            ctx_area_fk100: Option<String>,
            ctx_area_nk100: Option<String>,
        ) -> Self {
            Self {
                cano,
                acnt_prdt_cd,
                inqr_strt_dt,
                inqr_end_dt,
                sll_buy_dvsn_cd,
                pdno,
                ord_gno_brno,
                odno,
                ccld_dvsn,
                inqr_dvsn,
                inqr_dvsn_1,
                inqr_dvsn_3,
                excg_id_dvsn_cd,
                ctx_area_fk100,
                ctx_area_nk100,
            }
        }

        pub fn into_iter(&self) -> Vec<(&'static str, String)> {
            vec![
                ("CANO", self.cano.clone()),
                ("ACNT_PRDT_CD", self.acnt_prdt_cd.clone()),
                ("INQR_STRT_DT", self.inqr_strt_dt.clone()),
                ("INQR_END_DT", self.inqr_end_dt.clone()),
                ("SLL_BUY_DVSN_CD", self.sll_buy_dvsn_cd.clone()),
                ("PDNO", self.pdno.clone()),
                ("ORD_GNO_BRNO", self.ord_gno_brno.clone()),
                ("ODNO", self.odno.clone()),
                ("CCLD_DVSN", self.ccld_dvsn.clone()),
                ("INQR_DVSN", self.inqr_dvsn.clone()),
                ("INQR_DVSN_1", self.inqr_dvsn_1.clone()),
                ("INQR_DVSN_3", self.inqr_dvsn_3.clone()),
                ("EXCG_ID_DVSN_CD", self.excg_id_dvsn_cd.clone()),
                (
                    "CTX_AREA_FK100",
                    self.ctx_area_fk100.clone().unwrap_or_default(),
                ),
                (
                    "CTX_AREA_NK100",
                    self.ctx_area_nk100.clone().unwrap_or_default(),
                ),
            ]
        }
    }
}
