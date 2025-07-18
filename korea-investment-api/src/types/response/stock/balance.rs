use getset::Getters;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, Getters)]
pub struct BalanceResponse {
    /// 0: 성공, 0 이외의 값: 실패
    #[getset(get = "pub")]
    rt_cd: String,
    /// 응답코드
    #[getset(get = "pub")]
    msg_cd: String,
    /// 응답메시지
    #[getset(get = "pub")]
    msg1: String,
    /// 연속조회검색조건100
    #[getset(get = "pub")]
    ctx_area_fk100: Option<String>,
    /// 연속조회키100
    #[getset(get = "pub")]
    ctx_area_nk100: Option<String>,
    /// 보유종목 리스트
    #[getset(get = "pub")]
    output1: Option<Vec<Output1>>, // 주식잔고 목록
    /// 계좌 요약 정보
    #[getset(get = "pub")]
    output2: Option<Vec<Output2>>, // 계좌 요약 정보
}

#[derive(Clone, Debug, Deserialize, Getters)]
pub struct Output1 {
    #[getset(get = "pub")]
    pdno: String,
    #[getset(get = "pub")]
    prdt_name: String,
    #[getset(get = "pub")]
    trad_dvsn_name: String,
    #[getset(get = "pub")]
    bfdy_buy_qty: String,
    #[getset(get = "pub")]
    bfdy_sll_qty: String,
    #[getset(get = "pub")]
    thdt_buyqty: String,
    #[getset(get = "pub")]
    thdt_sll_qty: String,
    #[getset(get = "pub")]
    hldg_qty: String,
    #[getset(get = "pub")]
    ord_psbl_qty: String,
    #[getset(get = "pub")]
    pchs_avg_pric: String,
    #[getset(get = "pub")]
    pchs_amt: String,
    #[getset(get = "pub")]
    prpr: String,
    #[getset(get = "pub")]
    evlu_amt: String,
    #[getset(get = "pub")]
    evlu_pfls_amt: String,
    #[getset(get = "pub")]
    evlu_pfls_rt: String,
    #[getset(get = "pub")]
    evlu_erng_rt: String,
    #[getset(get = "pub")]
    loan_dt: String,
    #[getset(get = "pub")]
    loan_amt: String,
    #[getset(get = "pub")]
    stln_slng_chgs: String,
    #[getset(get = "pub")]
    expd_dt: String,
    #[getset(get = "pub")]
    fltt_rt: String,
    #[getset(get = "pub")]
    bfdy_cprs_icdc: String,
    #[getset(get = "pub")]
    item_mgna_rt_name: String,
    #[getset(get = "pub")]
    grta_rt_name: String,
    #[getset(get = "pub")]
    sbst_pric: String,
    #[getset(get = "pub")]
    stck_loan_unpr: String,
}

#[derive(Clone, Debug, Deserialize, Getters)]
pub struct Output2 {
    #[getset(get = "pub")]
    dnca_tot_amt: String,
    #[getset(get = "pub")]
    nxdy_excc_amt: String,
    #[getset(get = "pub")]
    prvs_rcdl_excc_amt: String,
    #[getset(get = "pub")]
    cma_evlu_amt: String,
    #[getset(get = "pub")]
    bfdy_buy_amt: String,
    #[getset(get = "pub")]
    thdt_buy_amt: String,
    #[getset(get = "pub")]
    nxdy_auto_rdpt_amt: String,
    #[getset(get = "pub")]
    bfdy_sll_amt: String,
    #[getset(get = "pub")]
    thdt_sll_amt: String,
    #[getset(get = "pub")]
    d2_auto_rdpt_amt: String,
    #[getset(get = "pub")]
    bfdy_tlex_amt: String,
    #[getset(get = "pub")]
    thdt_tlex_amt: String,
    #[getset(get = "pub")]
    tot_loan_amt: String,
    #[getset(get = "pub")]
    scts_evlu_amt: String,
    #[getset(get = "pub")]
    tot_evlu_amt: String,
    #[getset(get = "pub")]
    nass_amt: String,
    #[getset(get = "pub")]
    fncg_gld_auto_rdpt_yn: String,
    #[getset(get = "pub")]
    pchs_amt_smtl_amt: String,
    #[getset(get = "pub")]
    evlu_amt_smtl_amt: String,
    #[getset(get = "pub")]
    evlu_pfls_smtl_amt: String,
    #[getset(get = "pub")]
    tot_stln_slng_chgs: String,
    #[getset(get = "pub")]
    bfdy_tot_asst_evlu_amt: String,
    #[getset(get = "pub")]
    asst_icdc_amt: String,
    #[getset(get = "pub")]
    asst_icdc_erng_rt: String,
} 