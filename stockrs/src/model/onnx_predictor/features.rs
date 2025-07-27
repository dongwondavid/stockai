pub mod day1;
pub mod day2;
pub mod day3;
pub mod day4;
pub mod utils;

use crate::utility::errors::StockrsResult;
use rusqlite::Connection;
use tracing::{warn, info};

// 재수출
pub use utils::*;

/// 특징 계산을 위한 통합 함수
/// Solomon의 calculate_features_for_stock_optimized와 동일한 역할
pub fn calculate_features_for_stock_optimized(
    db: &Connection,
    daily_db: &Connection,
    stock_code: &str,
    date: &str,
    features: &[String],
    trading_dates: &[String],
) -> StockrsResult<Vec<f64>> {
    let mut feature_values = Vec::new();

    for feature in features {
        info!("🔍 [Features] 특징 계산 중: {} (종목: {})", feature, stock_code);
        let value = match feature.as_str() {
            // day4 관련 특징들
            "day4_macd_histogram_increasing" => {
                day4::calculate_macd_histogram_increasing(db, stock_code, date)?
            }
            "day4_short_macd_cross_signal" => {
                day4::calculate_short_macd_cross_signal(db, stock_code, date)?
            }
            "day4_open_to_now_return" => {
                day4::calculate_open_to_now_return(db, stock_code, date)?
            }
            "day4_is_long_bull_candle" => {
                day4::calculate_is_long_bull_candle(db, stock_code, date)?
            }
            "day4_macd_histogram" => {
                day4::calculate_macd_histogram(db, stock_code, date)?
            }
            "day4_pos_vs_high_5d" => {
                day4::calculate_pos_vs_high_5d(daily_db, stock_code, date)?
            }
            "day4_rsi_value" => day4::calculate_rsi_value(db, stock_code, date)?,
            "day4_pos_vs_high_3d" => {
                day4::calculate_pos_vs_high_3d(daily_db, stock_code, date)?
            }
            "day4_pos_vs_high_10d" => {
                day4::calculate_pos_vs_high_10d(daily_db, stock_code, date)?
            }

            // day1 관련 특징들
            "day1_current_price_ratio" => {
                day1::calculate_current_price_ratio(db, stock_code, date)?
            }
            "day1_high_price_ratio" => {
                day1::calculate_high_price_ratio(db, stock_code, date)?
            }
            "day1_low_price_ratio" => {
                day1::calculate_low_price_ratio(db, stock_code, date)?
            }
            "day1_price_position_ratio" => {
                day1::calculate_price_position_ratio(db, stock_code, date)?
            }
            "day1_fourth_derivative" => {
                day1::calculate_fourth_derivative(db, stock_code, date)?
            }
            "day1_long_candle_ratio" => {
                day1::calculate_long_candle_ratio(db, stock_code, date)?
            }
            "day1_fifth_derivative" => {
                day1::calculate_fifth_derivative(db, stock_code, date)?
            }
            "day1_sixth_derivative" => {
                day1::calculate_sixth_derivative(db, stock_code, date)?
            }
            "day1_volume_ratio" => {
                day1::calculate_volume_ratio(db, stock_code, date)?
            }
            "day1_vwap_position_ratio" => {
                day1::calculate_vwap_position_ratio(db, stock_code, date)?
            }

            // day3 관련 특징들
            "day3_morning_mdd" => day3::calculate_morning_mdd(db, stock_code, date)?,
            "day3_breaks_6month_high" => {
                day3::calculate_breaks_6month_high(daily_db, stock_code, date, trading_dates)?
            }
            "day3_morning_volume_ratio" => {
                day3::calculate_morning_volume_ratio(db, daily_db, stock_code, date)?
            }

            // day2 관련 특징들
            "day2_prev_day_range_ratio" => {
                day2::calculate_prev_day_range_ratio(daily_db, stock_code, date, trading_dates)?
            }
            "day2_prev_close_to_now_ratio" => day2::calculate_prev_close_to_now_ratio(
                db,
                daily_db,
                stock_code,
                date,
                trading_dates,
            )?,
            "day2_volume_ratio_vs_prevday" => {
                day2::calculate_volume_ratio_vs_prevday(db, daily_db, stock_code, date, trading_dates)?
            }

            _ => {
                warn!("⚠️ [Features] 알 수 없는 특징: {} (종목: {})", feature, stock_code);
                0.0
            }
        };
        feature_values.push(value);
    }

    Ok(feature_values)
}
