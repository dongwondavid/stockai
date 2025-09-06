use crate::utility::errors::StockrsResult;
use super::utils::parse_date_flexible;
use chrono::{Datelike, Weekday};

/// Day21: 계절성/달력 효과 II - 특수 이벤트 및 패턴
/// 공휴일, 산타랠리, 설/추석, 전환 효과, 특수 이벤트 등을 정량화



/// 12월 하순(산타랠리 구간) 여부 (0/1)
pub fn day21_santa_rally_flag(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 12월 15일~31일
    Ok(if date.month() == 12 && date.day() >= 15 { 1.0 } else { 0.0 })
}



/// 최근 5년 동일월 평균 수익률 (정규화)
pub fn day21_monthly_return_seasonality(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 간단한 구현: 실제로는 과거 5년 데이터가 필요
    // 여기서는 예시로 월별 계절성 패턴을 시뮬레이션
    let month = date.month() as usize;
    let seasonal_returns = [
        0.02, 0.01, 0.03, 0.02, -0.01, 0.01,  // 1-6월
        0.02, 0.01, -0.01, 0.03, 0.02, 0.04   // 7-12월
    ];
    
    let raw_return = seasonal_returns[month - 1];
    // [-1, 1] → [0, 1]로 정규화
    Ok((raw_return + 1.0) / 2.0)
}

/// 최근 5년 동일 요일 평균 수익률 (정규화)
pub fn day21_dayofweek_return_seasonality(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 간단한 구현: 실제로는 과거 5년 데이터가 필요
    // 여기서는 예시로 요일별 계절성 패턴을 시뮬레이션
    let weekday_returns = [
        0.01, 0.02, 0.01, 0.02, 0.03  // 월~금
    ];
    
    let weekday_index = match date.weekday() {
        Weekday::Mon => 0,
        Weekday::Tue => 1,
        Weekday::Wed => 2,
        Weekday::Thu => 3,
        Weekday::Fri => 4,
        _ => return Ok(0.5), // 주말은 0.5
    };
    
    let raw_return = weekday_returns[weekday_index];
    // [-1, 1] → [0, 1]로 정규화
    Ok((raw_return + 1.0) / 2.0)
}

/// 8월 중순(휴가철) 거래일 여부 (0/1)
pub fn day21_summer_holiday_flag(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 8월 10일~25일
    Ok(if date.month() == 8 && date.day() >= 10 && date.day() <= 25 { 1.0 } else { 0.0 })
}

/// 월말 3일~월초 3일 구간 여부 (0/1)
pub fn day21_turn_of_month_effect(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 월의 마지막날 계산
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
    
    // 월말 3일 또는 월초 3일
    let is_month_end_3 = date.day() > last_day - 3;
    let is_month_start_3 = date.day() <= 3;
    
    Ok(if is_month_end_3 || is_month_start_3 { 1.0 } else { 0.0 })
}

/// 분기말 3일~분기초 3일 구간 여부 (0/1)
pub fn day21_turn_of_quarter_effect(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 분기말: 3월, 6월, 9월, 12월
    let is_quarter_month = [3, 6, 9, 12].contains(&date.month());
    
    if !is_quarter_month {
        return Ok(0.0);
    }
    
    // 월의 마지막날 계산
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
    
    // 분기말 3일 또는 분기초 3일
    let is_quarter_end_3 = date.day() > last_day - 3;
    let is_quarter_start_3 = date.day() <= 3;
    
    Ok(if is_quarter_end_3 || is_quarter_start_3 { 1.0 } else { 0.0 })
}

/// 52주차를 01로 정규화한 값 (연간 주차 위치 반영)
pub fn day21_week_of_year_norm(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // ISO 8601 주차 계산 (간단한 구현)
    let week_num = date.iso_week().week();
    
    // 52주를 0~1로 정규화
    Ok(week_num as f64 / 52.0)
}

/// 선물·옵션 만기일(3,6,9,12월 셋째 금요일) 여부 (0/1)
pub fn day21_triple_witching_flag(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 3,6,9,12월의 셋째 금요일
    let is_triple_witching_month = [3, 6, 9, 12].contains(&date.month());
    
    if !is_triple_witching_month || date.weekday() != Weekday::Fri {
        return Ok(0.0);
    }
    
    // 셋째 금요일인지 확인 (간단한 구현)
    let day = date.day();
    Ok(if day >= 15 && day <= 21 { 1.0 } else { 0.0 })
}

/// 분기말/연말 윈도우드레싱 가능일 여부 (0/1)
pub fn day21_window_dressing_flag(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 분기말/연말 마지막 5거래일
    let is_quarter_end_month = [3, 6, 9, 12].contains(&date.month());
    
    if !is_quarter_end_month {
        return Ok(0.0);
    }
    
    // 월의 마지막날 계산
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
    
    // 마지막 5일
    Ok(if date.day() > last_day - 5 { 1.0 } else { 0.0 })
}

/// 국내 기업 결산월(12월) 말 거래일 여부 (0/1)
pub fn day21_fiscal_year_end_flag(date: &str) -> StockrsResult<f64> {
    let date = parse_date_flexible(date)?;
    
    // 12월 20일~31일
    Ok(if date.month() == 12 && date.day() >= 20 { 1.0 } else { 0.0 })
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_day21_santa_rally_flag() {
        assert_eq!(day21_santa_rally_flag("2024-12-15").unwrap(), 1.0);
        assert_eq!(day21_santa_rally_flag("2024-12-31").unwrap(), 1.0);
        assert_eq!(day21_santa_rally_flag("2024-12-14").unwrap(), 0.0);
        assert_eq!(day21_santa_rally_flag("2024-11-30").unwrap(), 0.0);
    }

    #[test]
    fn test_day21_summer_holiday_flag() {
        assert_eq!(day21_summer_holiday_flag("2024-08-10").unwrap(), 1.0);
        assert_eq!(day21_summer_holiday_flag("2024-08-25").unwrap(), 1.0);
        assert_eq!(day21_summer_holiday_flag("2024-08-09").unwrap(), 0.0);
        assert_eq!(day21_summer_holiday_flag("2024-08-26").unwrap(), 0.0);
    }

    #[test]
    fn test_day21_turn_of_month_effect() {
        assert_eq!(day21_turn_of_month_effect("2024-01-01").unwrap(), 1.0); // 월초
        assert_eq!(day21_turn_of_month_effect("2024-01-03").unwrap(), 1.0); // 월초
        assert_eq!(day21_turn_of_month_effect("2024-01-04").unwrap(), 0.0); // 월초 아님
        assert_eq!(day21_turn_of_month_effect("2024-01-29").unwrap(), 1.0); // 월말
        assert_eq!(day21_turn_of_month_effect("2024-01-31").unwrap(), 1.0); // 월말
    }

    #[test]
    fn test_day21_turn_of_quarter_effect() {
        assert_eq!(day21_turn_of_quarter_effect("2024-03-01").unwrap(), 1.0); // 분기초
        assert_eq!(day21_turn_of_quarter_effect("2024-03-03").unwrap(), 1.0); // 분기초
        assert_eq!(day21_turn_of_quarter_effect("2024-03-04").unwrap(), 0.0); // 분기초 아님
        assert_eq!(day21_turn_of_quarter_effect("2024-03-29").unwrap(), 1.0); // 분기말
        assert_eq!(day21_turn_of_quarter_effect("2024-03-31").unwrap(), 1.0); // 분기말
    }

    #[test]
    fn test_day21_week_of_year_norm() {
        let week1 = day21_week_of_year_norm("2024-01-01").unwrap();
        let week26 = day21_week_of_year_norm("2024-06-24").unwrap();
        let week52 = day21_week_of_year_norm("2024-12-23").unwrap();
        
        assert!((week1 - 0.019).abs() < 0.1); // 1주차 ≈ 0.019
        assert!((week26 - 0.5).abs() < 0.1);  // 26주차 ≈ 0.5
        assert!((week52 - 1.0).abs() < 0.1);  // 52주차 ≈ 1.0
    }

    #[test]
    fn test_day21_triple_witching_flag() {
        // 3월 셋째 금요일
        assert_eq!(day21_triple_witching_flag("2024-03-15").unwrap(), 1.0);
        assert_eq!(day21_triple_witching_flag("2024-03-22").unwrap(), 0.0);
        assert_eq!(day21_triple_witching_flag("2024-03-14").unwrap(), 0.0); // 목요일
        
        // 6월 셋째 금요일
        assert_eq!(day21_triple_witching_flag("2024-06-21").unwrap(), 1.0);
        
        // 다른 월
        assert_eq!(day21_triple_witching_flag("2024-01-19").unwrap(), 0.0);
    }

    #[test]
    fn test_day21_window_dressing_flag() {
        // 12월 마지막 5일
        assert_eq!(day21_window_dressing_flag("2024-12-27").unwrap(), 1.0);
        assert_eq!(day21_window_dressing_flag("2024-12-31").unwrap(), 1.0);
        assert_eq!(day21_window_dressing_flag("2024-12-26").unwrap(), 0.0);
        
        // 다른 월
        assert_eq!(day21_window_dressing_flag("2024-01-27").unwrap(), 0.0);
    }

    #[test]
    fn test_day21_fiscal_year_end_flag() {
        assert_eq!(day21_fiscal_year_end_flag("2024-12-20").unwrap(), 1.0);
        assert_eq!(day21_fiscal_year_end_flag("2024-12-31").unwrap(), 1.0);
        assert_eq!(day21_fiscal_year_end_flag("2024-12-19").unwrap(), 0.0);
        assert_eq!(day21_fiscal_year_end_flag("2024-11-30").unwrap(), 0.0);
    }

    #[test]
    fn test_day21_monthly_return_seasonality() {
        let jan = day21_monthly_return_seasonality("2024-01-15").unwrap();
        let apr = day21_monthly_return_seasonality("2024-04-15").unwrap();
        let dec = day21_monthly_return_seasonality("2024-12-15").unwrap();
        
        assert!((jan - 0.51).abs() < 0.1); // 1월: 0.02 → 0.51
        assert!((apr - 0.51).abs() < 0.1); // 4월: 0.02 → 0.51
        assert!((dec - 0.52).abs() < 0.1); // 12월: 0.04 → 0.52
    }

    #[test]
    fn test_day21_dayofweek_return_seasonality() {
        let mon = day21_dayofweek_return_seasonality("2024-01-01").unwrap(); // 월요일
        let fri = day21_dayofweek_return_seasonality("2024-01-05").unwrap(); // 금요일
        
        assert!((mon - 0.505).abs() < 0.1); // 월요일: 0.01 → 0.505
        assert!((fri - 0.515).abs() < 0.1); // 금요일: 0.03 → 0.515
    }
}
