use korea_investment_api::types::config::Config;
use korea_investment_api::types::Account;
use korea_investment_api::KoreaInvestmentApi;
use std::io::Read;
use std::path::PathBuf;
use clap::Parser;
use thiserror::Error;
use env_logger;

#[derive(Parser)]
#[command(name = "opt", about = "example")]
struct Opt {
    config_path: PathBuf,
}

#[derive(Debug, Error)]
enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    TomlDeserializeError(#[from] toml::de::Error),
    #[error(transparent)]
    ApiError(#[from] korea_investment_api::Error),
}

fn get_config(path: &PathBuf) -> Result<Config, Error> {
    let mut buf = String::new();
    let mut fd = std::fs::File::open(path)?;
    let _len = fd.read_to_string(&mut buf)?;
    Ok(toml::from_str(&buf)?)
}

async fn get_api(config: &Config) -> Result<KoreaInvestmentApi, Error> {
    let account = Account {
        cano: config.cano(),
        acnt_prdt_cd: config.acnt_prdt_cd().to_string(),
    };
    Ok(KoreaInvestmentApi::new(
        config.environment().clone(),
        config.app_key(),
        config.app_secret(),
        account,
        config.hts_id(),
        config.token_as_option(),
        config.approval_key_as_option(),
    )
    .await?)
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let Opt { config_path } = Opt::parse();
    let config = get_config(&config_path).unwrap();
    let api = get_api(&config).await.unwrap();

    // 주식잔고조회 예시
    let balance = api
        .order
        .inquire_balance(
            "N", // afhr_flpr_yn: 시간외단일가 여부 (N: 기본값)
            "02", // inqr_dvsn: 조회구분 (02: 종목별)
            "01", // unpr_dvsn: 단가구분 (01: 기본값)
            "N", // fund_sttl_icld_yn: 펀드결제분포함여부 (N: 미포함)
            "N", // fncg_amt_auto_rdpt_yn: 융자금액자동상환여부 (N: 기본값)
            "00", // prcs_dvsn: 처리구분 (00: 전일매매포함)
            None,  // ctx_area_fk100: 연속조회검색조건100 (None: 최초조회)
            None,  // ctx_area_nk100: 연속조회키100 (None: 최초조회)
        )
        .await
        .unwrap();
    println!("주식잔고조회 결과: {:?}", balance);

    // 매수가능조회 예시
    let buying_power = api
        .order
        .inquire_psbl_order(
            "005930", // pdno: 종목코드 (삼성전자)
            "",  // ord_unpr: 주문단가 (시장가 시 "0" 또는 "" 입력)
            "01",     // ord_dvsn: 주문구분 (01: 시장가, 00: 지정가)
            "N",      // cma_evlu_amt_icld_yn: CMA평가금액포함여부 (Y: 포함, N: 미포함)
            "N",      // ovrs_icld_yn: 해외포함여부 (Y: 포함, N: 미포함)
        )
        .await
        .unwrap();
    println!("매수가능조회 결과: {:?}", buying_power);


    // 유량 제한으로 1초 휴식
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // 주식일별주문체결조회 예시
    let daily_ccld = api
        .order
        .inquire_daily_ccld(
            "20250701", // inqr_strt_dt: 조회시작일자 (YYYYMMDD)
            "20250717", // inqr_end_dt: 조회종료일자 (YYYYMMDD)  
            "00",       // sll_buy_dvsn_cd: 매도매수구분코드 (00: 전체, 01: 매도, 02: 매수)
            "",         // pdno: 종목코드 (공백: 전체)
            "",         // ord_gno_brno: 주문채번지점번호 (공백: 전체)
            "",         // odno: 주문번호 (공백: 전체)
            "00",       // ccld_dvsn: 체결구분 (00: 전체, 01: 체결, 02: 미체결)
            "00",       // inqr_dvsn: 조회구분 (00: 역순, 01: 정순)
            "",         // inqr_dvsn_1: 조회구분1 (공백: 전체)
            "",         // inqr_dvsn_3: 조회구분3 (공백: 전체)
            "",         // excg_id_dvsn_cd: 거래소ID구분코드 (공백: 전체)
            None,       // ctx_area_fk100: 연속조회검색조건100 (None: 최초조회)
            None,       // ctx_area_nk100: 연속조회키100 (None: 최초조회)
        )
        .await
        .unwrap();
    println!("주식일별주문체결조회 결과: {:?}", daily_ccld);

    // 유량 제한으로 1초 휴식
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // 주식당일분봉조회 예시
    let minute_chart_params = korea_investment_api::types::request::stock::quote::MinutePriceChartParameter::new(
        "J",         // market_div_code: 조건 시장 분류 코드 (J: KRX)
        "005930",    // stock_code: 입력 종목코드 (삼성전자)
        "093000",    // input_hour: 입력 시간1 (HHMMSS 형식, 오전 9시30분)
        false,       // include_past_data: 과거 데이터 포함 여부
        "",          // etc_cls_code: 기타 구분 코드
    );
    
    let minute_chart = api
        .quote
        .minute_price_chart(minute_chart_params)
        .await
        .unwrap();
    println!("주식당일분봉조회 결과: {:?}", minute_chart);
}
