use crate::types::api::{ApiEnv, Domestic006Result};
use crate::types::broker::Order;



// 주식잔고조회[v1_국내주식-006]

// output1 Object Array
//  pdno
//  상품번호	String	Y	12	종목번호(뒷 6자리)
//  pchs_avg_pric
//  매입평균가격	String	Y	22	매입금액 / 보유수량

// output2 Object Array
// dnca_tot_amt
// 예수금총금액	String	Y	19	예수금
//  nass_amt
//  순자산금액	String	Y	19	

pub fn get_domestic006_result(_env: ApiEnv) -> Result<Domestic006Result, Box<dyn std::error::Error>> {
    todo!("get domestic006 result");
}

pub fn execute_order(_order: &Order, _env: ApiEnv) -> Result<String, Box<dyn std::error::Error>> {
    todo!("execute stock order");
}

pub fn check_fill(_order_id: &str, _env: ApiEnv) -> Result<bool, Box<dyn std::error::Error>> {
    todo!("check order fill");
}

pub fn cancel_order(_order_id: &str, _env: ApiEnv) -> Result<(), Box<dyn std::error::Error>> {
    todo!("cancel order");
}
