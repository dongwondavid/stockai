use crate::utility::errors::StockrsResult;
use super::utils::parse_date_flexible;
use chrono::{Datelike, Weekday};

/// Day20: 계절성/달력 효과 I - 기본 요인
/// 요일별 효과, 월별 주기성, 월초/월말, 계절성 효과를 정량화

/// 월요일 여부 (0/1)
pub fn day20_is_monday(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    Ok(if date.weekday() == Weekday::Mon { 1.0 } else { 0.0 })
}

/// 화요일 여부 (0/1)
pub fn day20_is_tuesday(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    Ok(if date.weekday() == Weekday::Tue { 1.0 } else { 0.0 })
}

/// 수요일 여부 (0/1)
pub fn day20_is_wednesday(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    Ok(if date.weekday() == Weekday::Wed { 1.0 } else { 0.0 })
}

/// 목요일 여부 (0/1)
pub fn day20_is_thursday(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    Ok(if date.weekday() == Weekday::Thu { 1.0 } else { 0.0 })
}

/// 금요일 여부 (0/1)
pub fn day20_is_friday(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    Ok(if date.weekday() == Weekday::Fri { 1.0 } else { 0.0 })
}

/// 월(1~12) 사인 변환값 (주기성 반영)
pub fn day20_month_sin(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    let month = date.month() as f64;
    // 1월을 0으로, 12월을 11로 변환하여 0~11 범위로 만들기
    let month_normalized = month - 1.0;
    let sin_value = (2.0 * std::f64::consts::PI * month_normalized / 12.0).sin();
    
    // [-1, 1] → [0, 1]로 정규화
    Ok((sin_value + 1.0) / 2.0)
}

/// 월(1~12) 코사인 변환값
pub fn day20_month_cos(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    let month = date.month() as f64;
    // 1월을 0으로, 12월을 11로 변환하여 0~11 범위로 만들기
    let month_normalized = month - 1.0;
    let cos_value = (2.0 * std::f64::consts::PI * month_normalized / 12.0).cos();
    
    // [-1, 1] → [0, 1]로 정규화
    Ok((cos_value + 1.0) / 2.0)
}

/// 월초 첫 거래일 여부 (0/1)
pub fn day20_is_month_start(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 간단한 구현: 월의 첫날이면 1.0, 아니면 0.0
    // 실제로는 거래일 기준으로 계산해야 함
    Ok(if date.day() == 1 { 1.0 } else { 0.0 })
}

/// 월말 마지막 거래일 여부 (0/1)
pub fn day20_is_month_end(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 간단한 구현: 월의 마지막날이면 1.0, 아니면 0.0
    // 실제로는 거래일 기준으로 계산해야 함
    let last_day = if date.month() == 2 {
        if date.year() % 4 == 0 && (date.year() % 100 != 0 || date.year() % 400 == 0) {
            29
        } else {
            28
        }
    } else if [4, 6, 9, 11].contains(&date.month()) {
        30
    } else {
        31
    };
    
    Ok(if date.day() == last_day { 1.0 } else { 0.0 })
}

/// 월말까지 남은 거래일 수 (정규화)
pub fn day20_days_to_month_end(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 간단한 구현: 월의 남은 일수를 정규화
    let last_day = if date.month() == 2 {
        if date.year() % 4 == 0 && (date.year() % 100 != 0 || date.year() % 400 == 0) {
            29
        } else {
            28
        }
    } else if [4, 6, 9, 11].contains(&date.month()) {
        30
    } else {
        31
    };
    
    let remaining_days = last_day - date.day() + 1;
    let max_days = last_day;
    
    Ok(remaining_days as f64 / max_days as f64)
}

/// 분기말 마지막 거래일 여부 (0/1)
pub fn day20_is_quarter_end(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 분기말: 3월, 6월, 9월, 12월의 마지막날
    let is_quarter_month = [3, 6, 9, 12].contains(&date.month());
    
    if !is_quarter_month {
        return Ok(0.0);
    }
    
    // 월의 마지막날 확인
    let last_day = if date.month() == 2 {
        if date.year() % 4 == 0 && (date.year() % 100 != 0 || date.year() % 400 == 0) {
            29
        } else {
            28
        }
    } else if [4, 6, 9, 11].contains(&date.month()) {
        30
    } else {
        31
    };
    
    Ok(if date.day() == last_day { 1.0 } else { 0.0 })
}

/// 반기말 마지막 거래일 여부 (0/1)
pub fn day20_is_half_year_end(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 반기말: 6월, 12월의 마지막날
    let is_half_year_month = [6, 12].contains(&date.month());
    
    if !is_half_year_month {
        return Ok(0.0);
    }
    
    // 월의 마지막날 확인
    let last_day = if date.month() == 2 {
        if date.year() % 4 == 0 && (date.year() % 100 != 0 || date.year() % 400 == 0) {
            29
        } else {
            28
        }
    } else if [4, 6, 9, 11].contains(&date.month()) {
        30
    } else {
        31
    };
    
    Ok(if date.day() == last_day { 1.0 } else { 0.0 })
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_day20_is_monday() {
        assert_eq!(day20_is_monday("2024-01-01").unwrap(), 1.0); // 월요일
        assert_eq!(day20_is_monday("2024-01-02").unwrap(), 0.0); // 화요일
    }

    #[test]
    fn test_day20_month_sin() {
        let jan = day20_month_sin("2024-01-15").unwrap();
        let apr = day20_month_sin("2024-04-15").unwrap();
        let jul = day20_month_sin("2024-07-15").unwrap();
        let oct = day20_month_sin("2024-10-15").unwrap();
        
        assert!((jan - 0.5).abs() < 0.1); // 1월: sin(π/6) ≈ 0.5
        assert!((apr - 1.0).abs() < 0.1); // 4월: sin(π/2) = 1.0
        assert!((jul - 0.5).abs() < 0.1); // 7월: sin(7π/6) ≈ 0.5
        assert!((oct - 0.0).abs() < 0.1); // 10월: sin(5π/3) ≈ 0.0
    }

    #[test]
    fn test_day20_month_cos() {
        let jan = day20_month_cos("2024-01-15").unwrap();
        let apr = day20_month_cos("2024-04-15").unwrap();
        let jul = day20_month_cos("2024-07-15").unwrap();
        let oct = day20_month_cos("2024-10-15").unwrap();
        
        assert!((jan - 1.0).abs() < 0.1); // 1월: cos(π/6) ≈ 1.0
        assert!((apr - 0.5).abs() < 0.1); // 4월: cos(π/2) = 0.0 → 정규화 후 0.5
        assert!((jul - 0.5).abs() < 0.1); // 7월: cos(7π/6) ≈ 0.5
        assert!((oct - 1.0).abs() < 0.1); // 10월: cos(5π/3) ≈ 1.0
    }

    #[test]
    fn test_day20_is_month_start() {
        assert_eq!(day20_is_month_start("2024-01-01").unwrap(), 1.0);
        assert_eq!(day20_is_month_start("2024-01-15").unwrap(), 0.0);
    }

    #[test]
    fn test_day20_is_month_end() {
        assert_eq!(day20_is_month_end("2024-01-31").unwrap(), 1.0);
        assert_eq!(day20_is_month_end("2024-01-15").unwrap(), 0.0);
        assert_eq!(day20_is_month_end("2024-02-29").unwrap(), 1.0); // 윤년
    }

    #[test]
    fn test_day20_days_to_month_end() {
        assert_eq!(day20_days_to_month_end("2024-01-01").unwrap(), 1.0); // 월초
        assert_eq!(day20_days_to_month_end("2024-01-31").unwrap(), 1.0/31.0); // 월말
        assert_eq!(day20_days_to_month_end("2024-01-16").unwrap(), 16.0/31.0); // 중간
    }

    #[test]
    fn test_day20_is_quarter_end() {
        assert_eq!(day20_is_quarter_end("2024-03-31").unwrap(), 1.0);
        assert_eq!(day20_is_quarter_end("2024-06-30").unwrap(), 1.0);
        assert_eq!(day20_is_quarter_end("2024-09-30").unwrap(), 1.0);
        assert_eq!(day20_is_quarter_end("2024-12-31").unwrap(), 1.0);
        assert_eq!(day20_is_quarter_end("2024-01-31").unwrap(), 0.0);
    }

    #[test]
    fn test_day20_is_half_year_end() {
        assert_eq!(day20_is_half_year_end("2024-06-30").unwrap(), 1.0);
        assert_eq!(day20_is_half_year_end("2024-12-31").unwrap(), 1.0);
        assert_eq!(day20_is_half_year_end("2024-03-31").unwrap(), 0.0);
    }


}
