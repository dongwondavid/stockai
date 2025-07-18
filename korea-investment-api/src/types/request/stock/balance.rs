use getset::{Getters, Setters};
use serde::Serialize;

#[derive(Debug, Clone, Getters, Setters, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct BalanceParameter {
    #[getset(get = "pub", set = "pub")]
    pub cano: String,
    #[getset(get = "pub", set = "pub")]
    pub acnt_prdt_cd: String,
    #[getset(get = "pub", set = "pub")]
    pub afhr_flpr_yn: String, // N: 기본값, Y: 시간외단일가, X: NXT 정규장
    #[getset(get = "pub", set = "pub")]
    pub inqr_dvsn: String,    // 01: 대출일별, 02: 종목별
    #[getset(get = "pub", set = "pub")]
    pub unpr_dvsn: String,    // 01: 기본값
    #[getset(get = "pub", set = "pub")]
    pub fund_sttl_icld_yn: String, // N: 포함하지 않음, Y: 포함
    #[getset(get = "pub", set = "pub")]
    pub fncg_amt_auto_rdpt_yn: String, // N: 기본값
    #[getset(get = "pub", set = "pub")]
    pub prcs_dvsn: String,    // 00: 전일매매포함, 01: 전일매매미포함
    #[getset(get = "pub", set = "pub")]
    pub ofl_yn: String,       // Always send as ""
    #[getset(get = "pub", set = "pub")]
    pub ctx_area_fk100: Option<String>,
    #[getset(get = "pub", set = "pub")]
    pub ctx_area_nk100: Option<String>,
}

impl BalanceParameter {
    pub fn new(
        cano: String,
        acnt_prdt_cd: String,
        afhr_flpr_yn: String,
        inqr_dvsn: String,
        unpr_dvsn: String,
        fund_sttl_icld_yn: String,
        fncg_amt_auto_rdpt_yn: String,
        prcs_dvsn: String,
        ctx_area_fk100: Option<String>,
        ctx_area_nk100: Option<String>,
    ) -> Self {
        Self {
            cano,
            acnt_prdt_cd,
            afhr_flpr_yn,
            inqr_dvsn,
            unpr_dvsn,
            fund_sttl_icld_yn,
            fncg_amt_auto_rdpt_yn,
            prcs_dvsn,
            ofl_yn: String::new(), // Always blank
            ctx_area_fk100,
            ctx_area_nk100,
        }
    }
    pub fn into_iter(&self) -> Vec<(&'static str, String)> {
        let mut params = vec![
            ("CANO", self.cano.clone()),
            ("ACNT_PRDT_CD", self.acnt_prdt_cd.clone()),
            ("AFHR_FLPR_YN", self.afhr_flpr_yn.clone()),
            ("INQR_DVSN", self.inqr_dvsn.clone()),
            ("UNPR_DVSN", self.unpr_dvsn.clone()),
            ("FUND_STTL_ICLD_YN", self.fund_sttl_icld_yn.clone()),
            ("FNCG_AMT_AUTO_RDPT_YN", self.fncg_amt_auto_rdpt_yn.clone()),
            ("PRCS_DVSN", self.prcs_dvsn.clone()),
            ("OFL_YN", self.ofl_yn.clone()), // Always blank
            ("CTX_AREA_FK100", self.ctx_area_fk100.clone().unwrap_or_default()),
            ("CTX_AREA_NK100", self.ctx_area_nk100.clone().unwrap_or_default()),
        ];
        params
    }
} 