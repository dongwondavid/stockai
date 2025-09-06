# score.py
import sqlite3
import pandas as pd
import numpy as np
from scipy import stats

# 1) DB 연결 및 데이터 로드
conn = sqlite3.connect('eval/ridge_model.db')
overview = pd.read_sql_query("SELECT date, roi, turnover, fee FROM overview", conn, parse_dates=['date'])
trading = pd.read_sql_query("SELECT date, time, buy_or_sell, profit, roi, fee, stockcode FROM trading", conn, parse_dates=['date'])

# 강제적으로 date 컬럼을 datetime으로 파싱 (정수 형태 20240514 등도 안전하게 처리)
overview['date'] = pd.to_datetime(overview['date'].astype(str), errors='coerce')
trading['date'] = pd.to_datetime(trading['date'].astype(str), errors='coerce')

# ROI를 퍼센트에서 소수점으로 변환
overview['roi'] = overview['roi'] / 100
trading['roi'] = trading['roi'] / 100

# date와 time을 합쳐서 정확한 datetime 생성
trading['datetime'] = pd.to_datetime(
    trading['date'].dt.strftime('%Y-%m-%d') + ' ' + trading['time'].astype(str),
    errors='coerce'
)

# 2) 기간 계산
overview.sort_values('date', inplace=True)
start_date = overview['date'].iloc[0]
end_date = overview['date'].iloc[-1]
years = (end_date - start_date).days / 365.25

# 3) 수익성 지표
cumulative_return = (1 + overview['roi']).prod() - 1
cagr = (1 + cumulative_return)**(1/years) - 1

# 4) 위험 지표
daily_vol = overview['roi'].std(ddof=1)
annual_vol = daily_vol * np.sqrt(252)
var_95 = overview['roi'].quantile(0.05)
cvar_95 = overview.loc[overview['roi'] <= var_95, 'roi'].mean()

# 5) 무위험 이율 설정 (월별 기준금리)
rf_monthly = {
    '2023-09': 0.035, '2023-10': 0.0325, '2023-11': 0.03, '2023-12': 0.03,
    '2024-01': 0.03, '2024-02': 0.03, '2024-03': 0.03, '2024-04': 0.03,
    '2024-05': 0.03, '2024-06': 0.03, '2024-07': 0.03, '2024-08': 0.03,
    '2024-09': 0.0275, '2024-10': 0.0275, '2024-11': 0.0275, '2024-12': 0.0275,
    '2025-01': 0.03, '2025-02': 0.0275, '2025-03': 0.0275, '2025-04': 0.0275
}

# 월별 무위험 이율을 일별로 매핑
overview['month'] = overview['date'].dt.to_period('M')
overview['rf_monthly'] = overview['month'].astype(str).map(rf_monthly)
overview['rf_daily'] = (1 + overview['rf_monthly'])**(1/252) - 1

# 6) 위험조정 성과 지표
excess = overview['roi'] - overview['rf_daily']
sharpe = excess.mean() / excess.std(ddof=1) * np.sqrt(252)

# 소르티노 비율 수정: 하방 변동성은 수익률이 평균 이하일 때의 표준편차
downside_std = overview.loc[overview['roi'] < overview['roi'].mean(), 'roi'].std(ddof=1)
sortino = excess.mean() / downside_std * np.sqrt(252) if downside_std > 0 else np.nan

# 7) 드로우다운 지표
cum_returns = (1 + overview['roi']).cumprod()
# 드로우다운 계산에서 날짜 인덱스를 사용하여 기간 계산 시 Timedelta를 얻도록 함
cum_returns.index = overview['date']
rolling_max = cum_returns.cummax()
drawdowns = cum_returns / rolling_max - 1
max_drawdown = drawdowns.min()

# 회복 기간 계산
def calculate_recovery_period(drawdowns):
    """최대 낙폭에서 회복되는 기간을 계산"""
    max_dd_idx = drawdowns.idxmin()
    recovery_idx = drawdowns[max_dd_idx:].loc[drawdowns[max_dd_idx:] >= 0].index
    if len(recovery_idx) > 0:
        recovery_date = recovery_idx[0]
        recovery_period = (recovery_date - max_dd_idx).days
        return recovery_period
    return None

# 드로우다운 지속 기간 계산
def calculate_drawdown_duration(drawdowns, dates):
    """각 드로우다운 기간의 지속 기간을 계산하고 최대값을 반환"""
    durations = []
    in_drawdown = False
    start_idx = None
    
    for i, dd in enumerate(drawdowns):
        if dd < 0 and not in_drawdown:
            # 드로우다운 시작
            in_drawdown = True
            start_idx = i
        elif dd >= 0 and in_drawdown:
            # 드로우다운 종료
            in_drawdown = False
            if start_idx is not None:
                duration = (dates.iloc[i] - dates.iloc[start_idx]).days
                durations.append(duration)
                start_idx = None
    
    # 마지막에 드로우다운이 끝나지 않은 경우
    if in_drawdown and start_idx is not None:
        duration = (dates.iloc[-1] - dates.iloc[start_idx]).days
        durations.append(duration)
    
    return max(durations) if durations else 0

recovery_period = calculate_recovery_period(drawdowns)
max_drawdown_duration = calculate_drawdown_duration(drawdowns, overview['date'])

calmar = cagr / abs(max_drawdown) if max_drawdown != 0 else np.nan
recovery_factor = cumulative_return / abs(max_drawdown) if max_drawdown != 0 else np.nan
ulcer_index = np.sqrt((drawdowns**2).mean())

# 8) 거래 지표
# 매도 거래만 필터링 (구매 거래는 수수료로 인한 손실이 필연적이므로 제외)
sell_trades = trading[trading['buy_or_sell'] == 'sell']
win_rate = (sell_trades['profit'] > 0).mean() if len(sell_trades) > 0 else np.nan
gross_profit = trading.loc[trading['profit'] > 0, 'profit'].sum()
gross_loss = -trading.loc[trading['profit'] < 0, 'profit'].sum()
profit_factor = gross_profit / gross_loss if gross_loss > 0 else np.nan
avg_gain = trading.loc[trading['profit'] > 0, 'profit'].mean()
avg_loss = abs(trading.loc[trading['profit'] < 0, 'profit'].mean())
expectancy = win_rate * avg_gain - (1 - win_rate) * avg_loss if not np.isnan(win_rate) else np.nan
trade_count = len(trading)

# 평균 보유 기간 계산 (매수-매도 쌍을 찾아서 계산)
def calculate_avg_holding_period(trading_df):
    """거래당 평균 보유 기간을 계산 (시간까지 고려)"""
    # 매수/매도 쌍을 찾기 위해 stockcode별로 그룹화
    if 'stockcode' in trading_df.columns and 'datetime' in trading_df.columns:
        holding_periods = []
        
        for stockcode in trading_df['stockcode'].unique():
            stock_trades = trading_df[trading_df['stockcode'] == stockcode].sort_values('datetime')
            
            # 매수와 매도 거래를 분리
            buy_trades = stock_trades[stock_trades['buy_or_sell'] == 'buy'].copy()
            sell_trades = stock_trades[stock_trades['buy_or_sell'] == 'sell'].copy()
            
            # 매수와 매도 거래의 개수가 같아야 함
            if len(buy_trades) == len(sell_trades):
                # 순서대로 매수-매도 쌍 매칭
                for i in range(len(buy_trades)):
                    buy_trade = buy_trades.iloc[i]
                    sell_trade = sell_trades.iloc[i]
                    
                    # datetime을 사용해서 정확한 시간 차이 계산
                    holding_period = (sell_trade['datetime'] - buy_trade['datetime']).total_seconds() / (24 * 3600)  # 일 단위로 변환
                    holding_periods.append(holding_period)

        # holding_periods를 텍스트 파일로 저장
        with open("holding_periods.txt", "w") as f:
            for period in holding_periods:
                f.write(f"{period}\n")
        
        return np.mean(holding_periods) if holding_periods else np.nan
    return np.nan

avg_holding_period = calculate_avg_holding_period(trading)

# 9) 비용 지표
total_fees = trading['fee'].sum()
fee_ratio = total_fees / overview['turnover'].sum()

# 10) 벤치마크 비교 지표 (월별)
bench = pd.DataFrame({
    'month': pd.period_range('2023-09', '2025-04', freq='M'),
    'rb': [0.0191, -0.0056, 0.0027, 0.0489, -0.0234, -0.0409,
           -0.0158, -0.0464, -0.0498, -0.0092, 0.0721, -0.0190,
           -0.0254, 0.0536, 0.0575, -0.0608, 0.0578, 0.1076,
           -0.0647, -0.0240]
})

monthly_pf = overview.groupby('month')['roi'].apply(lambda x: (1 + x).prod() - 1).rename('rp').reset_index()
dfm = pd.merge(monthly_pf, bench, on='month').dropna()

beta = np.cov(dfm['rp'], dfm['rb'], ddof=1)[0,1] / np.var(dfm['rb'], ddof=1)
alpha = dfm['rp'].mean() - beta * dfm['rb'].mean()
tracking_error_monthly = (dfm['rp'] - dfm['rb']).std(ddof=1)
tracking_error_annual = tracking_error_monthly * np.sqrt(12)

# 11) 결과 출력
print("=== 수익성 지표 ===")
print(f"Cumulative Return: {cumulative_return:.2%}")
print(f"CAGR: {cagr:.2%}")

print("\n=== 위험 지표 ===")
print(f"Annual Volatility: {annual_vol:.2%}")
print(f"VaR 95%: {var_95:.2%}")
print(f"CVaR 95%: {cvar_95:.2%}")

print("\n=== 위험조정 성과 지표 ===")
print(f"Sharpe Ratio: {sharpe:.2f}")
print(f"Sortino Ratio: {sortino:.2f}")
print(f"Calmar Ratio: {calmar:.2f}")
print(f"Recovery Factor: {recovery_factor:.2f}")
print(f"Ulcer Index: {ulcer_index:.4f}")

print("\n=== 드로우다운 지표 ===")
print(f"Max Drawdown: {max_drawdown:.2%}")
if recovery_period is not None:
    print(f"Recovery Period: {recovery_period} days")
else:
    print("Recovery Period: Not yet recovered")
print(f"Max Drawdown Duration: {max_drawdown_duration} days")

print("\n=== 거래 지표 ===")
print(f"Win Rate: {win_rate:.2%}")
print(f"Profit Factor: {profit_factor:.2f}")
print(f"Expectancy: {expectancy:.2f}")
print(f"Trade Count: {trade_count}")
if not np.isnan(avg_holding_period):
    if avg_holding_period < 1:
        # 1일보다 작으면 시간 단위로 변환
        hours = avg_holding_period * 24
        if hours < 1:
            # 1시간보다 작으면 분 단위로 변환
            minutes = hours * 60
            print(f"Average Holding Period: {minutes:.1f} minutes")
        else:
            print(f"Average Holding Period: {hours:.1f} hours")
    else:
        print(f"Average Holding Period: {avg_holding_period:.1f} days")
else:
    print("Average Holding Period: Unable to calculate (missing stock_code data)")

print("\n=== 비용 지표 ===")
print(f"Total Fees: {total_fees:.2f}")
print(f"Fee Ratio: {fee_ratio:.2%}")

print("\n=== 벤치마크 비교 지표 ===")
print(f"Beta: {beta:.3f}")
print(f"Alpha: {alpha:.2%}")
print(f"Tracking Error (annual): {tracking_error_annual:.2%}")

conn.close()
