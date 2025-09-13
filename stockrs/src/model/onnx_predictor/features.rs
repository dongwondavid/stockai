pub mod day1;
pub mod day1_new;
pub mod day2;
pub mod day2_new;
pub mod day3;
pub mod day3_new;
pub mod day4;
pub mod day4_new;
pub mod day5;
pub mod day6;
pub mod day7;
pub mod day8;
pub mod day9;
pub mod day10;
pub mod day11;
pub mod day12;
pub mod day13;
pub mod day14;
pub mod day15;
pub mod day16;
pub mod day17;
pub mod day18;
pub mod day19;
pub mod day20;
pub mod day21;
pub mod day22;
pub mod day23;
pub mod day24;
pub mod day25;
pub mod day26;
pub mod day27;
pub mod day28;
pub mod indicators;


pub mod utils;

use crate::utility::errors::StockrsResult;
use rusqlite::Connection;
use tracing::info;

// 재수출
pub use utils::*;

/// 특징 계산을 위한 통합 함수
/// Solomon의 calculate_features_for_stock_optimized와 동일한 역할
pub fn calculate_features_for_stock_optimized(
    db_5min: &Connection,
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

            "day16_di_cross_flag" => {
                day16::day16_di_cross_flag(db_5min, daily_db, stock_code, date)?
            }


            "day16_adx_trend_change" => {
                day16::day16_adx_trend_change(db_5min, daily_db, stock_code, date)?
            }
            "day2_was_prevday_long_candle" => {
                day2_new::calculate_was_prevday_long_candle(daily_db, stock_code, date, trading_dates)?
            }
            "day1_second_derivative" => {
                day1_new::calculate_second_derivative(db_5min, stock_code, date)?
            }
            "day1_third_derivative" => {
                day1_new::calculate_third_derivative(db_5min, stock_code, date)?
            }
            "day1_is_long_candle" => {
                day1_new::calculate_is_long_candle(db_5min, stock_code, date)?
            }
            "day5_prev_day_range_change" => {
                day5::calculate_day5_prev_day_range_change(daily_db, stock_code, date, trading_dates)?
            }
            "day5_pos_vs_high_250d" => {
                day5::calculate_day5_pos_vs_high_250d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day9_bearish_engulfing_strength" => {
                day9::calculate_bearish_engulfing_strength(db_5min, stock_code, date)?
            }
            "day9_morning_star_strength" => {
                day9::calculate_morning_star_strength(db_5min, stock_code, date)?
            }
            "day9_evening_star_strength" => {
                day9::calculate_evening_star_strength(db_5min, stock_code, date)?
            }
            "day9_hammer_strength" => {
                day9::calculate_hammer_strength(db_5min, stock_code, date)?
            }
            "day12_stoch_oversold_persistence" => {
                day12::day12_stoch_oversold_persistence(daily_db, stock_code, date, trading_dates)?
            }
            "day12_stoch_overbought_persistence" => {
                day12::day12_stoch_overbought_persistence(daily_db, stock_code, date, trading_dates)?
            }
            "day12_stoch_rsi_signal_cross" => {
                day12::day12_stoch_rsi_signal_cross(daily_db, stock_code, date, trading_dates)?
            }
            "day12_rsi_divergence_flag" => {
                day12::day12_rsi_divergence_flag(daily_db, stock_code, date, trading_dates)?
            }
            "day12_roc_momentum_diff" => {
                day12::day12_roc_momentum_diff(daily_db, stock_code, date, trading_dates)?
            }
            "day14_volume_trend_slope20" => {
                day14::day14_volume_trend_slope20(db_5min, daily_db, stock_code, date)?
            }
            "day13_bollinger_band_squeeze_flag" => {
                day13::day13_bollinger_band_squeeze_flag(db_5min, daily_db, stock_code, date)?
            }
            "day24_prev_day_gain_and_morning_follow" => {
                day24::calculate_day24_prev_day_gain_and_morning_follow(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day25_gap_vs_prev_trend_alignment" => {
                day25::calculate_day25_gap_vs_prev_trend_alignment(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day18_lower_shadow_percentile_60d" => {
                day18::day18_lower_shadow_percentile_60d(db_5min, daily_db, stock_code, date)?
            }
            "day18_doji_flag" => {
                day18::day18_doji_flag(db_5min, daily_db, stock_code, date)?
            }
            "day18_three_black_crows_flag" => {
                day18::day18_three_black_crows_flag(db_5min, daily_db, stock_code, date)?
            }
            "day18_three_white_soldiers_flag" => {
                day18::day18_three_white_soldiers_flag(db_5min, daily_db, stock_code, date)?
            }
            "day18_tweezer_top_flag" => {
                day18::day18_tweezer_top_flag(db_5min, daily_db, stock_code, date)?
            }
            "day18_tweezer_bottom_flag" => {
                day18::day18_tweezer_bottom_flag(db_5min, daily_db, stock_code, date)?
            }
            "day18_spinning_top_flag" => {
                day18::day18_spinning_top_flag(db_5min, daily_db, stock_code, date)?
            }
            "day18_marubozu_flag" => {
                day18::day18_marubozu_flag(db_5min, daily_db, stock_code, date)?
            }
            "day18_upper_shadow_percentile_60d" => {
                day18::day18_upper_shadow_percentile_60d(db_5min, daily_db, stock_code, date)?
            }
            "day18_shooting_star_flag" => {
                day18::day18_shooting_star_flag(db_5min, daily_db, stock_code, date)?
            }
            
            "day3_market_cap_over_3000b" => {
                day3_new::calculate_market_cap_over_3000b(daily_db, stock_code, date, trading_dates)?
            }
            "day26_foreign_holding_ratio" => {
                day26::day26_foreign_holding_ratio(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day19_extreme_gap_flag" => {
                day19::day19_extreme_gap_flag(db_5min, daily_db, stock_code, date)?
            }
            "day14_morning_volume_abs" => {
                day14::day14_morning_volume_abs(db_5min, daily_db, stock_code, date)?
            }
            "day14_up_vs_down_volume_ratio" => {
                day14::day14_up_vs_down_volume_ratio(db_5min, daily_db, stock_code, date)?
            }
            "day27_var_95_norm" => {
                day27::day27_var_95_norm(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day14_volume_volatility_10d" => {
                day14::day14_volume_volatility_10d(db_5min, daily_db, stock_code, date)?
            }
            "day1_long_candle_strength" => {
                day1::calculate_long_candle_strength(db_5min, stock_code, date)?
            }
            "day14_turnover_rate_20d" => {
                day14::day14_turnover_rate_20d(db_5min, daily_db, stock_code, date)?
            }
            "day23_return_vol_of_vol_20d" => {
                day23::calculate_day23_return_vol_of_vol_20d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day7_volatility_regime" => {
                day7::calculate_volatility_regime(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day7_breakout_strength" => {
                day7::calculate_breakout_strength(db_5min, daily_db, stock_code, date)?
            }
            "day7_resistance_break_ratio" => {
                day7::calculate_resistance_break_ratio(db_5min, daily_db, stock_code, date)?
            }
            "day7_pattern_strength" => {
                day7::calculate_pattern_strength(db_5min, stock_code, date)?
            }
            "day7_candle_body_ratio" => {
                day7::calculate_candle_body_ratio(db_5min, stock_code, date)?
            }
            "day25_multi_tf_vol_ratio" => {
                day25::calculate_day25_multi_tf_vol_ratio(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day25_morning_volatility_vs_prev5d_mean" => {
                day25::calculate_day25_morning_volatility_vs_prev5d_mean(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day25_morning_return_vs_prev5d_mean" => {
                day25::calculate_day25_morning_return_vs_prev5d_mean(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day25_opening_return_vs_prev20d_vol" => {
                day25::calculate_day25_opening_return_vs_prev20d_vol(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day25_opening_trend_consistency" => {
                day25::calculate_day25_opening_trend_consistency(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day25_morning_gain_loss_balance" => {
                day25::calculate_day25_morning_gain_loss_balance(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day25_gap_continue_or_fade_flag" => {
                day25::calculate_day25_gap_continue_or_fade_flag(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day19_gap_above_prev_high_flag" => {
                day19::day19_gap_above_prev_high_flag(db_5min, daily_db, stock_code, date)?
            }
            "day19_gap_below_prev_low_flag" => {
                day19::day19_gap_below_prev_low_flag(db_5min, daily_db, stock_code, date)?
            }
            "day22_kurtosis_60d" => {
                day22::calculate_day22_kurtosis_60d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day22_mean_return_5d" => {
                day22::calculate_day22_mean_return_5d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day22_gain_loss_std_ratio_20d" => {
                day22::calculate_day22_gain_loss_std_ratio_20d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            // Day20 Seasonality & Calendar
            "day20_is_monday" => { day20::day20_is_monday(date)? }
            "day20_is_tuesday" => { day20::day20_is_tuesday(date)? }
            "day20_is_month_start" => { day20::day20_is_month_start(date)? }
            "day20_is_month_end" => { day20::day20_is_month_end(date)? }
            "day20_days_to_month_end" => { day20::day20_days_to_month_end(date)? }
            "day20_is_quarter_end" => { day20::day20_is_quarter_end(date)? }
            "day20_is_half_year_end" => { day20::day20_is_half_year_end(date)? }
            "day20_month_sin" => { day20::day20_month_sin(date)? }
            "day20_month_cos" => { day20::day20_month_cos(date)? }
            // Day21 Special Events
            "day21_santa_rally_flag" => { day21::day21_santa_rally_flag(date)? }
            "day21_summer_holiday_flag" => { day21::day21_summer_holiday_flag(date)? }
            "day21_turn_of_month_effect" => { day21::day21_turn_of_month_effect(date)? }
            "day21_turn_of_quarter_effect" => { day21::day21_turn_of_quarter_effect(date)? }
            "day21_triple_witching_flag" => { day21::day21_triple_witching_flag(date)? }
            "day21_window_dressing_flag" => { day21::day21_window_dressing_flag(date)? }
            "day21_fiscal_year_end_flag" => { day21::day21_fiscal_year_end_flag(date)? }
            "day28_pivot_support3" => {
                day28::day28_pivot_support3(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_daily_pivot_point" => {
                day28::day28_daily_pivot_point(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_pivot_resistance1" => {
                day28::day28_pivot_resistance1(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_pivot_support1" => {
                day28::day28_pivot_support1(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_pivot_resistance2" => {
                day28::day28_pivot_resistance2(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_pivot_support2" => {
                day28::day28_pivot_support2(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_price_vs_pivot_ratio" => {
                day28::day28_price_vs_pivot_ratio(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_price_vs_r1_gap" => {
                day28::day28_price_vs_r1_gap(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_price_vs_s1_gap" => {
                day28::day28_price_vs_s1_gap(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_intraday_pivot_break_flag" => {
                day28::day28_intraday_pivot_break_flag(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_risk_regime_flag" => {
                day27::day27_risk_regime_flag(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day15_volume_rsi_14" => {
                day15::day15_volume_rsi_14(db_5min, daily_db, stock_code, date)?
            }
            "day28_intraday_r1_break_flag" => {
                day28::day28_intraday_r1_break_flag(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_intraday_s1_break_flag" => {
                day28::day28_intraday_s1_break_flag(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_pivot_bandwidth" => {
                day28::day28_pivot_bandwidth(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day28_pivot_regime_score" => {
                day28::day28_pivot_regime_score(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_tail_index_hill" => {
                day23::calculate_day23_tail_index_hill(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_autocorr_2d" => {
                day23::calculate_day23_autocorr_2d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_autocorr_5d" => {
                day23::calculate_day23_autocorr_5d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_partial_autocorr_1d" => {
                day23::calculate_day23_partial_autocorr_1d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_return_entropy_20d" => {
                day23::calculate_day23_return_entropy_20d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_return_percentile_95_20d" => {
                day23::calculate_day23_return_percentile_95_20d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_return_percentile_5_20d" => {
                day23::calculate_day23_return_percentile_5_20d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_expected_shortfall_5p" => {
                day23::calculate_day23_expected_shortfall_5p(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_var_5p_20d" => {
                day23::calculate_day23_var_5p_20d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_hurst_exponent_100d" => {
                day23::calculate_day23_hurst_exponent_100d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_long_memory_score" => {
                day23::calculate_day23_long_memory_score(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_regime_switching_flag" => {
                day23::calculate_day23_regime_switching_flag(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day23_volatility_clustering_score" => {
                day23::calculate_day23_volatility_clustering_score(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day19_gap_fill_intraday_flag" => {
                day19::day19_gap_fill_intraday_flag(db_5min, daily_db, stock_code, date)?
            }
            "day19_gap_up_flag" => {
                day19::day19_gap_up_flag(db_5min, daily_db, stock_code, date)?
            }
            "day15_extreme_money_inflow_flag" => {
                day15::day15_extreme_money_inflow_flag(db_5min, daily_db, stock_code, date)?
            }
            "day15_obv_change_5d" => {
                day15::day15_obv_change_5d(db_5min, daily_db, stock_code, date)?
            }
            "day15_mfi_trend_slope5" => {
                day15::day15_mfi_trend_slope5(db_5min, daily_db, stock_code, date)?
            }
            "day15_price_volume_divergence_flag" => {
                day15::day15_price_volume_divergence_flag(db_5min, daily_db, stock_code, date)?
            }
            "day15_obv_divergence_flag" => {
                day15::day15_obv_divergence_flag(db_5min, daily_db, stock_code, date)?
            }
            "day26_net_buy_percentile_60d" => {
                day26::day26_net_buy_percentile_60d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_foreign_buy_pressure_intraday" => {
                day26::day26_foreign_buy_pressure_intraday(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_foreign_net_buy_1d" => {
                day26::day26_foreign_net_buy_1d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_foreign_net_buy_5d_sum" => {
                day26::day26_foreign_net_buy_5d_sum(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_foreign_holding_change_5d" => {
                day26::day26_foreign_holding_change_5d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_institution_net_buy_1d" => {
                day26::day26_institution_net_buy_1d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_institution_net_buy_20d_sum" => {
                day26::day26_institution_net_buy_20d_sum(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_institution_holding_ratio" => {
                day26::day26_institution_holding_ratio(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_institution_buy_pressure_intraday" => {
                day26::day26_institution_buy_pressure_intraday(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_foreign_vs_institution_balance" => {
                day26::day26_foreign_vs_institution_balance(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_foreign_flow_volatility" => {
                day26::day26_foreign_flow_volatility(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_institution_flow_volatility" => {
                day26::day26_institution_flow_volatility(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_foreign_institution_correlation" => {
                day26::day26_foreign_institution_correlation(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day26_flow_regime_flag" => {
                day26::day26_flow_regime_flag(daily_db, stock_code, date, trading_dates)?
            }
            "day14_morning_turnover_ratio" => {
                day14::day14_morning_turnover_ratio(db_5min, daily_db, stock_code, date)?
            }
            "day14_volume_vs_20d_avg" => {
                day14::day14_volume_vs_20d_avg(db_5min, daily_db, stock_code, date)?
            }
            "day8_consecutive_bull_candle_strength" => {
                day8::calculate_consecutive_bull_candle_strength(db_5min, stock_code, date)?
            }
            "day8_macd_histogram_value" => {
                day8::calculate_macd_histogram_value(db_5min, stock_code, date)?
            }
            "day8_foreign_ratio_change" => {
                day8::calculate_foreign_ratio_change(daily_db, stock_code, date, trading_dates)?
            }
            "day24_opening_volatility_ratio" => {
                day24::calculate_day24_opening_volatility_ratio(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day24_morning_vs_prev_range_ratio" => {
                day24::calculate_day24_morning_vs_prev_range_ratio(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day24_gap_and_morning_trend_flag" => {
                day24::calculate_day24_gap_and_morning_trend_flag(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            // duplicate keys removed (already mapped above)
            "day24_prev_day_loss_and_morning_follow" => {
                day24::calculate_day24_prev_day_loss_and_morning_follow(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day24_morning_candle_strength_ratio" => {
                day24::calculate_day24_morning_candle_strength_ratio(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day24_opening_gap_and_range_alignment" => {
                day24::calculate_day24_opening_gap_and_range_alignment(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day11_distance_percentile_vs_sma20_120d" => {
                day11::calculate_day11_distance_percentile_vs_sma20_120d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day11_sma60_curvature5" => {
                day11::calculate_day11_sma60_curvature5(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day11_ma_ribbon_tightness_lookback60" => {
                day11::calculate_day11_ma_ribbon_tightness_lookback60(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day14_morning_vs_prevday_volume" => {
                day14::day14_morning_vs_prevday_volume(db_5min, daily_db, stock_code, date)?
            }
            "day15_chaikin_mf_trend" => {
                day15::day15_chaikin_mf_trend(db_5min, daily_db, stock_code, date)?
            }
            "day14_volume_percentile_60d" => {
                day14::day14_volume_percentile_60d(db_5min, daily_db, stock_code, date)?
            }
            "day14_volume_volatility_20d" => {
                day14::day14_volume_volatility_20d(db_5min, daily_db, stock_code, date)?
            }
            "day14_buying_pressure_score" => {
                day14::day14_buying_pressure_score(db_5min, daily_db, stock_code, date)?
            }
            "day14_turnover_rate_5d" => {
                day14::day14_turnover_rate_5d(db_5min, daily_db, stock_code, date)?
            }
            "day25_prev_day_volume_and_morning_intensity" => {
                day25::calculate_day25_prev_day_volume_and_morning_intensity(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day22_kurtosis_10d" => {
                day22::calculate_day22_kurtosis_10d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day24_morning_vs_prev_volatility_percentile" => {
                day24::calculate_day24_morning_vs_prev_volatility_percentile(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            // Extra Day14 & Day13 features
            "day14_low_volume_dry_flag" => {
                day14::day14_low_volume_dry_flag(db_5min, daily_db, stock_code, date)?
            }
            "day14_volume_acceleration_10d" => {
                day14::day14_volume_acceleration_10d(db_5min, daily_db, stock_code, date)?
            }
            "day13_volatility_spike_flag" => {
                day13::day13_volatility_spike_flag(db_5min, daily_db, stock_code, date)?
            }
            "day10_sma_slope_change_ratio" => {
                day10::calculate_day10_sma_slope_change_ratio(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day10_cross_up_5_20" => {
                day10::calculate_day10_cross_up_5_20(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day10_cross_down_5_20" => {
                day10::calculate_day10_cross_down_5_20(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day10_cross_up_20_60" => {
                day10::calculate_day10_cross_up_20_60(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day10_cross_down_20_60" => {
                day10::calculate_day10_cross_down_20_60(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day14_high_volume_spike_flag" => {
                day14::day14_high_volume_spike_flag(db_5min, daily_db, stock_code, date)?
            }
            "day6_donchian20_break_strength" => {
                day6::calculate_donchian20_break_strength(daily_db, stock_code, date)?
            }
            "day6_adx_14" => {
                day6::calculate_adx_14(daily_db, stock_code, date)?
            }
            "day27_drawdown_volatility_ratio" => {
                day27::day27_drawdown_volatility_ratio(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day13_atr_slope5" => {
                day13::day13_atr_slope5(db_5min, daily_db, stock_code, date)?
            }
            "day27_max_drawdown_20d" => {
                day27::day27_max_drawdown_20d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_max_drawdown_60d" => {
                day27::day27_max_drawdown_60d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_recovery_days_from_last_peak" => {
                day27::day27_recovery_days_from_last_peak(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_cvar_95" => {
                day27::day27_cvar_95(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_ulcer_index_20d" => {
                day27::day27_ulcer_index_20d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_pain_index_60d" => {
                day27::day27_pain_index_60d(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_stress_var_flag" => {
                day27::day27_stress_var_flag(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_time_to_recover_index" => {
                day27::day27_time_to_recover_index(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_risk_of_ruin_proxy" => {
                day27::day27_risk_of_ruin_proxy(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_drawdown_skewness" => {
                day27::day27_drawdown_skewness(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_drawdown_kurtosis" => {
                day27::day27_drawdown_kurtosis(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day27_tail_dependence_index" => {
                day27::day27_tail_dependence_index(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day15_ad_line_change_5d" => {
                day15::day15_ad_line_change_5d(db_5min, daily_db, stock_code, date)?
            }
            // day4 관련 특징들
            "day4_macd_histogram_increasing" => {
                day4::calculate_macd_histogram_increasing(db_5min, stock_code, date)?
            }
            "day4_short_macd_cross_signal" => {
                day4::calculate_short_macd_cross_signal(db_5min, stock_code, date)?
            }
            "day4_open_to_now_return" => {
                day4::calculate_open_to_now_return(db_5min, stock_code, date)?
            }
            "day4_is_long_bull_candle" => {
                day4::calculate_is_long_bull_candle(db_5min, stock_code, date)?
            }
            "day4_macd_histogram" => {
                day4::calculate_macd_histogram(db_5min, stock_code, date)?
            }
            "day4_bollinger_band_width" => {
                day4::calculate_bollinger_band_width(db_5min, stock_code, date)?
            }
            "day4_pos_vs_high_5d" => {
                day4::calculate_pos_vs_high_5d(db_5min, daily_db, stock_code, date)?
            }
            "day4_rsi_value" => day4::calculate_rsi_value(db_5min, stock_code, date)?,
            "day4_pos_vs_high_3d" => {
                day4::calculate_pos_vs_high_3d(db_5min, daily_db, stock_code, date)?
            }
            "day4_pos_vs_high_10d" => {
                day4::calculate_pos_vs_high_10d(db_5min, daily_db, stock_code, date)?
            }
            "day4_rsi_overbought" => {
                day4_new::calculate_rsi_overbought(db_5min, stock_code, date)?
            }
            "day4_is_highest_volume_bull_candle" => {
                day4_new::calculate_is_highest_volume_bull_candle(db_5min, stock_code, date)?
            }
            "day4_is_hammer" => {
                day4_new::calculate_is_hammer(db_5min, stock_code, date)?
            }
            "day4_is_evening_star" => {
                day4_new::calculate_is_evening_star(db_5min, stock_code, date)?
            }
            "day4_is_morning_star" => {
                day4_new::calculate_is_morning_star(db_5min, stock_code, date)?
            }
            "day4_is_breaking_upper_band" => {
                day4_new::calculate_is_breaking_upper_band(db_5min, stock_code, date)?
            }
            "day4_high_volume_early_count" => {
                day4_new::calculate_high_volume_early_count(db_5min, stock_code, date)?
            }

            // day1 관련 특징들
            "day1_current_price_ratio" => {
                day1::calculate_current_price_ratio(db_5min, stock_code, date)?
            }
            "day1_high_price_ratio" => {
                day1::calculate_high_price_ratio(db_5min, stock_code, date)?
            }
            "day1_low_price_ratio" => {
                day1::calculate_low_price_ratio(db_5min, stock_code, date)?
            }
            "day1_price_position_ratio" => {
                day1::calculate_price_position_ratio(db_5min, stock_code, date)?
            }
            "day1_fourth_derivative" => {
                day1::calculate_fourth_derivative(db_5min, stock_code, date)?
            }
            "day1_long_candle_ratio" => {
                day1::calculate_long_candle_ratio(db_5min, stock_code, date)?
            }
            "day1_fifth_derivative" => {
                day1::calculate_fifth_derivative(db_5min, stock_code, date)?
            }
            "day1_sixth_derivative" => {
                day1::calculate_sixth_derivative(db_5min, stock_code, date)?
            }
            "day1_volume_ratio" => {
                day1::calculate_volume_ratio(db_5min, stock_code, date)?
            }
            "day1_vwap_position_ratio" => {
                day1::calculate_vwap_position_ratio(db_5min, stock_code, date)?
            }

            // day3 관련 특징들
            "day3_morning_mdd" => day3::calculate_morning_mdd(db_5min, stock_code, date)?,
            "day3_breaks_6month_high" => {
                day3::calculate_breaks_6month_high(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day3_morning_volume_ratio" => {
                day3_new::calculate_morning_volume_ratio(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day3_breaks_6month_high_with_long_candle" => {
                day3_new::calculate_breaks_6month_high_with_long_candle(db_5min, daily_db, stock_code, date, trading_dates)?
            }
            "day3_foreign_ratio_3day_rising" => {
                day3_new::calculate_foreign_ratio_3day_rising(daily_db, stock_code, date, trading_dates)?
            }
            "day3_near_price_boundary" => {
                day3_new::calculate_near_price_boundary(db_5min, stock_code, date)?
            }
            "day3_consecutive_3_positive_candles" => {
                day3_new::calculate_consecutive_3_positive_candles(db_5min, stock_code, date)?
            }

            // day2 관련 특징들
            "day2_prev_day_range_ratio" => {
                day2::calculate_prev_day_range_ratio(daily_db, stock_code, date, trading_dates)?
            }
            "day2_prev_close_to_now_ratio" => day2::calculate_prev_close_to_now_ratio(
                db_5min,
                daily_db,
                stock_code,
                date,
                trading_dates,
            )?,
            "day2_volume_ratio_vs_prevday" => {
                day2::calculate_volume_ratio_vs_prevday(db_5min, daily_db, stock_code, date, trading_dates)?
            }


            "day17_resistance_touch_count_3m" => {
                day17::day17_resistance_touch_count_3m(db_5min, daily_db, stock_code, date)?
            }
            "day17_support_touch_count_3m" => {
                day17::day17_support_touch_count_3m(db_5min, daily_db, stock_code, date)?
            }
            "day17_near_52w_high_flag" => {
                day17::day17_near_52w_high_flag(db_5min, daily_db, stock_code, date)?
            }
            "day17_near_52w_low_flag" => {
                day17::day17_near_52w_low_flag(db_5min, daily_db, stock_code, date)?
            }
            "day17_breaks_52w_high_flag" => {
                day17::day17_breaks_52w_high_flag(db_5min, daily_db, stock_code, date)?
            }
            "day17_breaks_52w_low_flag" => {
                day17::day17_breaks_52w_low_flag(db_5min, daily_db, stock_code, date)?
            }
            _ => {
                println!("⚠️ [Features] 알 수 없는 특징: {} (종목: {})", feature, stock_code);
                0.0
            }
        };
        feature_values.push(value);
    }

    Ok(feature_values)
}
