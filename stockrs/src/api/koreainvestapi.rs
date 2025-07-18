use crate::types::broker::Order;
use std::error::Error;

// 기존 한국투자증권 API 함수들은 새로운 StockApi trait에서 구현되므로
// 여기서는 삭제하거나 deprecated 처리

/// 기존 함수들은 이제 types/api.rs의 RealApi, PaperApi에서 구현됨
/// 이 파일은 향후 삭제 예정이거나 헬퍼 함수들만 유지

#[deprecated(note = "Use StockApi trait implementations instead")]
pub fn execute_order(_order: &Order) -> Result<String, Box<dyn Error>> {
    todo!("Use RealApi::execute_order instead");
}

#[deprecated(note = "Use StockApi trait implementations instead")]
pub fn check_fill(_order_id: &str) -> Result<bool, Box<dyn Error>> {
    todo!("Use RealApi::check_fill instead");
}

#[deprecated(note = "Use StockApi trait implementations instead")]
pub fn cancel_order(_order_id: &str) -> Result<(), Box<dyn Error>> {
    todo!("Use RealApi::cancel_order instead");
}
