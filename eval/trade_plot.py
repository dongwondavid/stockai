# =========================
# DB에서 임의의 완결 거래 N개 뽑아 차트 그리기
# =========================
from dataclasses import dataclass
from typing import List, Optional, Tuple
import random

TRADING_DB_PATH = r"test_2.db"   # <-- trading 테이블이 있는 DB 경로로 바꿔주세요!


import sqlite3
import polars as pl
import matplotlib.pyplot as plt
from datetime import datetime, timedelta
import matplotlib.font_manager as fm
import platform
import matplotlib.dates as mdates
import numpy as np
from matplotlib.patches import Rectangle

# 운영체제별 기본 폰트 설정
if platform.system() == 'Windows':
    plt.rc('font', family='Malgun Gothic')
elif platform.system() == 'Darwin':  # macOS
    plt.rc('font', family='AppleGothic')
else:  # Linux
    plt.rc('font', family='NanumGothic')

# 마이너스 기호 깨짐 방지
plt.rc('axes', unicode_minus=False)

db_path = "D:/db/stock_price(1min).db"

def get_stock_code_map():
    """종목명과 종목코드를 매핑하는 딕셔너리를 반환합니다."""
    try:
        df = pl.read_csv("sector.csv")
        name_to_code = {name: f"A{code}" for code, name in zip(df["종목코드"], df["종목명"])}
        return name_to_code
    except Exception as e:
        print(f"⛔ sector.csv 파일 읽기 오류: {e}")
        return {}

def convert_to_stock_code(stock_input):
    """종목명 또는 종목코드를 입력받아 종목코드를 반환합니다."""
    if isinstance(stock_input, str):
        # 이미 종목코드 형식('A'로 시작하는 6자리 숫자)인 경우
        if stock_input.startswith('A') and len(stock_input) == 7:
            return stock_input
        
        # 종목명으로 변환 시도
        name_to_code = get_stock_code_map()
        if stock_input in name_to_code:
            return name_to_code[stock_input]
        
        # 숫자만 있는 경우 'A' 접두사 추가
        if stock_input.isdigit() and len(stock_input) == 6:
            return f"A{stock_input}"
    
    return stock_input

def get_1min_data(conn, table_name, date_str):
    date_base = date_str.replace("-", "")
    from_ts = int(date_base + "0900")
    to_ts = int(date_base + "1530")

    query = f"""
        SELECT date, open, high, low, close, volume FROM "{table_name}"
        WHERE date BETWEEN {from_ts} AND {to_ts}
        ORDER BY date ASC
    """
    df = pl.read_database(query, connection=conn)
    if len(df) == 0:
        return None
        
    df = df.with_columns([
        pl.col("date").cast(pl.Utf8).str.to_datetime(format="%Y%m%d%H%M"),
        pl.col("open").cast(pl.Float64),
        pl.col("high").cast(pl.Float64),
        pl.col("low").cast(pl.Float64),
        pl.col("close").cast(pl.Float64),
        pl.col("volume").cast(pl.Float64)
    ])
    return df

def get_previous_day_close(conn, table_name, date_str):
    """장 기준 전날 종가를 가져오는 함수 (주말, 공휴일 제외)"""
    try:
        # 입력 날짜를 datetime 객체로 변환
        current_date = datetime.strptime(date_str, "%Y-%m-%d")
        
        # 최대 30일 전까지 탐색 (충분한 여유)
        for i in range(1, 31):
            test_date = current_date - timedelta(days=i)
            test_date_str = test_date.strftime("%Y%m%d")
            
            # 해당 날짜의 종가 데이터 조회
            query = f"""
                SELECT close FROM "{table_name}"
                WHERE date = {test_date_str}1530
                ORDER BY date DESC
                LIMIT 1
            """
            
            df = pl.read_database(query, connection=conn)
            
            if len(df) > 0:
                # 데이터가 있으면 해당 날짜가 장 기준 전날
                if i > 1:  # 바로 전날이 아닌 경우 디버깅 정보 출력
                    print(f"📅 {table_name}: {date_str} 기준 장 전날은 {test_date.strftime('%Y-%m-%d')} (총 {i}일 전)")
                return df["close"][0]
        
        # 30일 내에 데이터가 없으면 None 반환
        print(f"⚠️ {table_name}: {date_str} 기준으로 최근 30일 내 거래 데이터가 없습니다.")
        return None
        
    except Exception as e:
        print(f"전날 종가 조회 오류: {e}")
        return None

def plot_candlestick(ax, df, target_time, target_price, prev_close_price):
    """캔들스틱 차트를 그리는 함수"""
    colors = []
    for i in range(len(df)):
        if df["close"][i] >= df["open"][i]:
            colors.append('red')  # 상승봉
        else:
            colors.append('blue')  # 하락봉
    
    # 거래량 상위 10개 시간 찾기 (1분봉이므로 더 많은 거래량 포인트 표시)
    volume_sorted = df.sort("volume", descending=True)
    top_10_volume_times = volume_sorted.head(10)["date"].to_list()
    
    # 캔들스틱 그리기
    for i in range(len(df)):
        open_val = df["open"][i]
        close_val = df["close"][i]
        high_val = df["high"][i]
        low_val = df["low"][i]
        date_val = df["date"][i]
        
        # 캔들스틱 너비 설정 (1분 간격 데이터이므로 0.5분 너비로 설정)
        candle_width = np.timedelta64(30, 's')  # 30초 너비
        candle_center = date_val
        
        # 몸통 그리기
        body_height = abs(close_val - open_val)
        body_bottom = min(open_val, close_val)
        
        if body_height > 0:
            # 몸통을 캔들스틱 중심에 위치시키기
            rect = Rectangle((candle_center - candle_width/2, body_bottom), 
                           candle_width, body_height,
                           facecolor=colors[i], edgecolor='black', linewidth=0.3)
            ax.add_patch(rect)
        
        # 꼬리 그리기 (캔들스틱 중심에서 수직으로)
        ax.plot([candle_center, candle_center], [low_val, high_val], 
               color='black', linewidth=0.5)
        
        # 거래량 상위 10개 시간에 별표 표시
        if date_val in top_10_volume_times:
            volume_val = df["volume"][i]
            ax.scatter(candle_center, high_val + (high_val * 0.001),  # 고가 위에 약간 여유를 두고 표시
                      color='purple', s=60, marker='*', zorder=10, 
                      edgecolors='black', linewidth=0.5)
            # 거래량 수치 표시 (더 작은 폰트)
            ax.annotate(f'{volume_val:,.0f}', 
                       (candle_center, high_val + (high_val * 0.002)),
                       xytext=(0, 3), textcoords='offset points',
                       ha='center', va='bottom',
                       color='purple', fontweight='bold', fontsize=6,
                       bbox=dict(boxstyle='round,pad=0.1', facecolor='white', alpha=0.8))
    
    # 9시 30분 지점에 특별한 마커
    ax.scatter(target_time, target_price, color='orange', s=80, 
              marker='*', zorder=10, edgecolors='black', linewidth=1)
    
    # 전날 종가 대비 상승률 계산 및 표시
    if prev_close_price is not None:
        change_rate = ((target_price - prev_close_price) / prev_close_price) * 100
        ax.annotate(f'{change_rate:+.1f}%', 
                   (target_time, target_price),
                   xytext=(10, 10), textcoords='offset points',
                   ha='left', va='bottom',
                   color='red' if change_rate >= 0 else 'blue',
                   fontweight='bold', fontsize=9,
                   bbox=dict(boxstyle='round,pad=0.3', facecolor='white', alpha=0.8))

def plot_stocks_1min(date_str, stock_list):
    """
    날짜와 종목 리스트를 받아 1분봉 캔들스틱 차트를 그립니다.
    거래량 상위 10개 시간은 보라색 별표로 표시됩니다.
    stock_list는 종목코드("A000660") 또는 종목명("SK하이닉스")을 포함할 수 있습니다.
    """
    conn = sqlite3.connect(db_path)
    num_stocks = len(stock_list)
    
    # 적절한 n x m 그리드 계산
    if num_stocks <= 4:
        rows, cols = 2, 2
    elif num_stocks <= 6:
        rows, cols = 2, 3
    elif num_stocks <= 9:
        rows, cols = 3, 3
    elif num_stocks <= 12:
        rows, cols = 3, 4
    elif num_stocks <= 16:
        rows, cols = 4, 4
    elif num_stocks <= 20:
        rows, cols = 4, 5
    else:
        # 더 많은 종목의 경우 5열로 고정
        cols = 5
        rows = (num_stocks + cols - 1) // cols  # 올림 나눗셈
    
    print(f"📊 1분봉 차트 레이아웃: {rows}행 x {cols}열 (총 {num_stocks}개 종목)")
    
    # 각 종목당 1개의 subplot (캔들스틱만)
    fig, axes = plt.subplots(rows, cols, figsize=(4*cols, 3*rows))
    
    # 단일 종목일 경우 axes를 2D 배열로 변환
    if num_stocks == 1:
        axes = axes.reshape(1, 1)
    # 1차원 배열인 경우 2차원으로 변환
    elif axes.ndim == 1:
        axes = axes.reshape(1, -1)
    
    name_to_code = get_stock_code_map()
    code_to_name = {v: k for k, v in name_to_code.items()}
    
    # 9시 30분 시점의 datetime 객체 생성
    date_base = date_str.replace("-", "")
    target_time = datetime.strptime(f"{date_base}0930", "%Y%m%d%H%M")
    
    for idx, stock in enumerate(stock_list):
        try:
            code = convert_to_stock_code(stock)
            
            # 처음 3개 종목만 상세 디버깅
            if idx < 3:
                print(f"🔍 디버깅: {stock} -> {code}")
            
            df = get_1min_data(conn, code, date_str)
            
            if df is None or len(df) == 0:
                print(f"⚠️ {stock} ({code}): 데이터가 없습니다.")
                continue
            
            # 처음 3개 종목만 데이터프레임 내용 출력
            if idx < 3:
                print(f"📊 1분봉 데이터프레임 크기: {len(df)}")
                print(f"📋 데이터프레임 처음 5행:")
                print(df.head())
                print("---")
            
            # 종목명 표시 (종목코드와 함께)
            display_name = code_to_name.get(code, code)
            title = f"{display_name} ({code}) - 1분봉"
            
            # 9시 1분 시가와 9시 30분 종가 찾기
            open_data = df.filter(pl.col("date").dt.strftime("%H%M") == "0901")
            
            if len(open_data) == 0:
                if idx < 3:  # 처음 3개만 상세 출력
                    print(f"⚠️ {stock}: 9시 1분 데이터가 없습니다.")
                    print(f"🕐 사용 가능한 시간대:")
                    time_data = df.select(pl.col("date").dt.strftime("%H%M")).unique().sort("date")
                    print(time_data.head(10))  # 처음 10개 시간대만 출력
                    print("---")
                
                # 9시 1분이 없으면 사용 가능한 첫 번째 시간대 사용
                available_times = df.select(pl.col("date").dt.strftime("%H%M")).unique().sort("date")
                if len(available_times) == 0:
                    print(f"⚠️ {stock}: 사용 가능한 시간대가 없습니다.")
                    continue
                
                first_available_time = available_times["date"][0]
                print(f"🔄 {stock}: 9시 1분 대신 {first_available_time} 데이터를 사용합니다.")
                
                open_data = df.filter(pl.col("date").dt.strftime("%H%M") == first_available_time)
                if len(open_data) == 0:
                    print(f"⚠️ {stock}: {first_available_time} 데이터도 없습니다.")
                    continue
                
            open_price = open_data["open"][0]
            
            if idx < 3:
                print(f"💰 9시 1분 시가: {open_price}")
            
            target_price = df.filter(pl.col("date").dt.strftime("%H%M") == "0930")
            
            if len(target_price) > 0:
                target_price = target_price["close"][0]
                prev_close_price = get_previous_day_close(conn, code, date_str)
                
                if prev_close_price is None:
                    if idx < 3:
                        print(f"⚠️ {stock}: 전날 종가 데이터가 없습니다.")
                    continue
                
                if idx < 3:
                    print(f"📈 상승률 계산 디버깅:")
                    print(f"   - 장 기준 전날 종가: {prev_close_price:,.0f}원")
                    print(f"   - 9시 30분 종가: {target_price:,.0f}원")
                    change_rate = ((target_price - prev_close_price) / prev_close_price) * 100
                    print(f"   - 가격 변화: {target_price - prev_close_price:+,.0f}원")
                    print(f"   - 전날 종가 대비 상승률: {change_rate:+.2f}%")
                    print("---")
                
                # 현재 subplot의 위치 계산
                row = idx // cols
                col = idx % cols
                current_ax = axes[row, col]
                
                # 캔들스틱 차트 그리기 (거래량 상위 10개 포함)
                plot_candlestick(current_ax, df, target_time, target_price, prev_close_price)
                
                # 차트 설정
                current_ax.set_title(title, fontsize=11, fontweight='bold')
                current_ax.set_ylabel("가격 (원)", fontsize=9)
                current_ax.set_xlabel("시간", fontsize=9)
                current_ax.grid(True, alpha=0.3)
                current_ax.tick_params(axis='both', which='major', labelsize=7)
                
                # x축 시간 포맷 설정 (1분봉이므로 더 세밀한 설정)
                current_ax.xaxis.set_major_locator(mdates.MinuteLocator(byminute=[0, 15, 30, 45]))  # 15분 간격
                current_ax.xaxis.set_major_formatter(mdates.DateFormatter('%H:%M'))
                current_ax.xaxis.set_minor_locator(mdates.MinuteLocator(interval=5))  # 5분 간격 보조 눈금
                plt.setp(current_ax.xaxis.get_majorticklabels(), rotation=45, ha='right')
            
        except Exception as e:
            print(f"⛔ {stock} 오류: {e}")
            if idx < 3:  # 처음 3개만 상세 오류 출력
                import traceback
                print(f"📋 상세 오류: {traceback.format_exc()}")
    
    # 사용하지 않는 subplot 숨기기
    for idx in range(num_stocks, rows * cols):
        row = idx // cols
        col = idx % cols
        axes[row, col].set_visible(False)
    
    plt.suptitle(f"{date_str} 종목별 1분봉 캔들스틱 차트 (장 기준 전날 종가 대비 상승률, 보라색 ★ = 거래량 상위 10개)", y=0.98, fontsize=15, fontweight='bold')
    plt.tight_layout()
    plt.show()
    conn.close()


@dataclass
class Trade:
    date: str          # "YYYY-MM-DD"
    code: str          # "A123456"
    side: str          # 항상 'buy' (엔트리 기준)
    qty: int
    entry_time: str    # "HH:MM:SS"
    entry_price: float
    exit_time: str     # "HH:MM:SS"
    exit_price: float
    reason: str        # 매도행 strategy
    fee_entry: float   # 매수 수수료
    fee_exit: float    # 매도 손익(혹은 수수료)
    roi: float         # 매도행 roi (퍼센트값)

def _to_datetime_str(date_str: str, time_str: str) -> str:
    # SQLite에 "HH:MM"만 있을 수 있어 보호 코드
    t = time_str if len(time_str) == 8 else (time_str + ":00" if len(time_str) == 5 else time_str)
    return f"{date_str} {t}"

def _pair_day_trades(df_day: pl.DataFrame) -> List[Trade]:
    """
    같은 날짜의 거래들(df_day)에서 매수->매도를 순차 매칭해 Trade 리스트로 반환.
    trading 스키마:
      date, time, stockcode, buy_or_sell, quantity, price, fee, strategy, avg_price, profit, roi
    """
    # 시간 순 정렬
    df_day = df_day.sort(by=["stockcode", "time"])

    trades: List[Trade] = []
    # 종목별로 큐를 만들어 buy를 쌓고 이후 sell과 순차 매칭
    for g in df_day.partition_by("stockcode", as_dict=False):
        # 파이썬 리스트로 순회
        rows = g.select([
            pl.col("date"), pl.col("time"), pl.col("stockcode"),
            pl.col("buy_or_sell"), pl.col("quantity"), pl.col("price"),
            pl.col("fee"), pl.col("strategy"), pl.col("avg_price"),
            pl.col("profit"), pl.col("roi")
        ]).to_dicts()

        buy_queue = []
        for r in rows:
            side = str(r["buy_or_sell"]).lower()
            if side == "buy":
                buy_queue.append(r)
            elif side == "sell" and buy_queue:
                b = buy_queue.pop(0)
                # 매수와 매도의 수량이 다를 수 있지만, 여기선 매수 수량 기준으로 구성
                tr = Trade(
                    date=str(r["date"]),
                    code=str(r["stockcode"]),
                    side="buy",
                    qty=int(b["quantity"]),
                    entry_time=str(b["time"]) if len(str(b["time"])) == 8 else (str(b["time"]) + ":00"),
                    entry_price=float(b["price"]),
                    exit_time=str(r["time"]) if len(str(r["time"])) == 8 else (str(r["time"]) + ":00"),
                    exit_price=float(r["price"]),
                    reason=str(r.get("strategy") or ""),
                    fee_entry=float(b.get("fee") or 0.0),
                    fee_exit=float(r.get("profit") or 0.0),  # 테이블 설계상 profit 컬럼에 청산 손익 저장되어 있음
                    roi=float(r.get("roi") or 0.0)
                )
                trades.append(tr)
        # 남은 buy는 당일 미청산 → 버림
    return trades

def _sample_completed_trades_from_db(trading_db_path: str, n: int, seed: Optional[int] = None) -> List[Trade]:
    """
    trading 테이블 전체를 로드해서, '완결(매수→매도)'된 거래 중 임의의 N개를 뽑아 Trade 리스트 반환
    """
    conn = sqlite3.connect(trading_db_path)

    # 전체 테이블 읽기
    df_all = pl.read_database("""
        SELECT
            date, time, stockcode, buy_or_sell, quantity, price, fee,
            strategy, avg_price, profit, roi
        FROM trading
    """, connection=conn)

    if len(df_all) == 0:
        conn.close()
        raise RuntimeError("trading 테이블에 데이터가 없습니다.")

    # 문자열형으로 통일(일부 DB에서 숫자/텍스트 혼재 방지)
    df_all = df_all.with_columns([
        pl.col("date").cast(pl.Utf8),
        pl.col("time").cast(pl.Utf8),
        pl.col("stockcode").cast(pl.Utf8),
        pl.col("buy_or_sell").cast(pl.Utf8),
        pl.col("strategy").cast(pl.Utf8),
        pl.col("quantity").cast(pl.Int64),
        pl.col("price").cast(pl.Float64),
        pl.col("fee").cast(pl.Float64),
        pl.col("avg_price").cast(pl.Float64),
        pl.col("profit").cast(pl.Float64),
        pl.col("roi").cast(pl.Float64),
    ])

    # 날짜별로 쪼개서 매수-매도 페어링
    trades_all: List[Trade] = []
    for g in df_all.partition_by("date", as_dict=False):
        trades_all.extend(_pair_day_trades(g))

    conn.close()

    if not trades_all:
        raise RuntimeError("완결(매수→매도) 거래를 찾지 못했습니다.")

    # 랜덤 샘플링
    if seed is not None:
        random.seed(seed)
    if n >= len(trades_all):
        sampled = trades_all
    else:
        sampled = random.sample(trades_all, n)

    # 정렬(보기 좋게 날짜, 종목, 진입시간)
    sampled.sort(key=lambda t: (t.date, t.code, t.entry_time))
    return sampled

def plot_trades_overview(trades: List[Trade]):
    """
    - 같은 날짜의 거래들을 묶어 하루 한 Figure로 그립니다.
    - 각 거래는 1개의 subplot(캔들 + 진입/청산 오버레이).
    - 상단엔 날짜 타이틀, 각 축엔 종목명/사유/ROI 등 표시.
    """
    if not trades:
        print("표시할 거래가 없습니다.")
        return

    # 날짜별 그룹화
    trades_by_date = {}
    for t in trades:
        trades_by_date.setdefault(t.date, []).append(t)

    name_to_code = get_stock_code_map()
    code_to_name = {v: k for k, v in name_to_code.items()}

    for date_str, day_trades in trades_by_date.items():
        n = len(day_trades)
        # 그리드 계산
        if n <= 2:
            rows, cols = 1, 2
        elif n <= 4:
            rows, cols = 2, 2
        elif n <= 6:
            rows, cols = 2, 3
        else:
            cols = 3
            rows = (n + cols - 1) // cols

        fig, axes = plt.subplots(rows, cols, figsize=(4*cols, 3*rows))
        if isinstance(axes, np.ndarray) and axes.ndim == 1:
            axes = axes.reshape(1, -1)
        elif not isinstance(axes, np.ndarray):
            axes = np.array([[axes]])

        fig.suptitle(f"{date_str} 거래 차트 (진입/청산, ROI/손익, 사유 표시)", fontsize=15, fontweight='bold', y=0.98)

        # 1분봉 DB(가격 DB)는 호출마다 열지 말고 한 번만
        price_conn = sqlite3.connect(db_path)

        for idx, tr in enumerate(day_trades):
            ax = axes[idx // cols, idx % cols]

            code = convert_to_stock_code(tr.code)
            display_name = code_to_name.get(code, code)

            # 1분봉 로드
            df = get_1min_data(price_conn, code, date_str)
            if df is None or len(df) == 0:
                ax.set_title(f"{display_name} ({code})\n데이터 없음", fontsize=11, fontweight='bold')
                ax.axis('off')
                continue

            # 09:30 기준 전일대비 표시
            target_time = datetime.strptime(date_str.replace("-", "") + "0930", "%Y%m%d%H%M")
            target_price_row = df.filter(pl.col("date").dt.strftime("%H%M") == "0930")
            target_price = float(target_price_row["close"][0]) if len(target_price_row) > 0 else float(df["close"][0])
            prev_close = get_previous_day_close(price_conn, code, date_str)

            plot_candlestick(ax, df, target_time, target_price, prev_close)

            # 진입/청산 마커
            e_dt = datetime.strptime(_to_datetime_str(tr.date, tr.entry_time), "%Y-%m-%d %H:%M:%S")
            x_dt = datetime.strptime(_to_datetime_str(tr.date, tr.exit_time), "%Y-%m-%d %H:%M:%S")

            ax.scatter(e_dt, tr.entry_price, s=90, marker='*', color='orange', edgecolors='black', linewidth=1.0, zorder=12)
            ax.annotate(f'ENTRY\n{tr.entry_price:,.0f}',
                        (e_dt, tr.entry_price),
                        xytext=(0, 15), textcoords='offset points',
                        ha='center', va='bottom', fontsize=8,
                        bbox=dict(boxstyle='round,pad=0.2', facecolor='white', alpha=0.8))

            # 청산 사유별 스타일
            reason = (tr.reason or "").lower()
            if "take" in reason:
                mk = dict(marker='D', color='green', label='take_profit')
            elif "stop" in reason:
                mk = dict(marker='X', color='red', label='stop_loss')
            elif "force" in reason:
                mk = dict(marker='s', color='gray', label='force_close')
            else:
                mk = dict(marker='o', color='black', label=reason or 'exit')

            ax.scatter(x_dt, tr.exit_price, s=60, edgecolors='black', linewidth=0.8, zorder=12, **mk)
            ax.annotate(f'EXIT\n{tr.exit_price:,.0f}\n{tr.reason}',
                        (x_dt, tr.exit_price),
                        xytext=(0, -30), textcoords='offset points',
                        ha='center', va='top', fontsize=8, color=mk.get("color", "black"),
                        bbox=dict(boxstyle='round,pad=0.2', facecolor='white', alpha=0.85))

            # ROI/손익(매도행 기준)
            pnl_color = 'red' if tr.roi >= 0 else 'blue'
            ax.annotate(f'ROI: {tr.roi:+.2f}%\nPnL: {tr.fee_exit:,.0f}',
                        (x_dt, (tr.entry_price + tr.exit_price)/2),
                        xytext=(10, 0), textcoords='offset points',
                        ha='left', va='center', fontsize=8, color=pnl_color,
                        bbox=dict(boxstyle='round,pad=0.2', facecolor='white', alpha=0.85))

            # 엔트리 가격선
            ax.axhline(tr.entry_price, linestyle='--', linewidth=0.8, color='gray', alpha=0.6)

            ax.set_title(f"{display_name} ({code})  x{tr.qty}", fontsize=11, fontweight='bold')
            ax.set_ylabel("가격 (원)", fontsize=9)
            ax.set_xlabel("시간", fontsize=9)
            ax.grid(True, alpha=0.3)
            ax.tick_params(axis='both', which='major', labelsize=7)
            ax.xaxis.set_major_locator(mdates.MinuteLocator(byminute=[0, 15, 30, 45]))
            ax.xaxis.set_major_formatter(mdates.DateFormatter('%H:%M'))
            ax.xaxis.set_minor_locator(mdates.MinuteLocator(interval=5))
            plt.setp(ax.xaxis.get_majorticklabels(), rotation=45, ha='right')

        price_conn.close()

        # 남는 축 숨기기
        total = rows * cols
        for k in range(n, total):
            r, c = k // cols, k % cols
            axes[r, c].set_visible(False)

        plt.tight_layout()
        plt.show()

def plot_random_trades_from_db(n: int = 5, seed: Optional[int] = 42):
    """
    trading DB에서 완결 거래를 임의로 n개 추출해 차트로 표시
    """
    trades = _sample_completed_trades_from_db(TRADING_DB_PATH, n=n, seed=seed)
    print(f"🎯 무작위 {len(trades)}건 선택 완료:")
    for t in trades:
        print(f"- {t.date} {t.code} {t.entry_time}→{t.exit_time} {t.reason} ROI={t.roi:+.2f}%")
    plot_trades_overview(trades)

def plot_date_trades_from_db(dates: List[str]):
    """
    날짜 문자열 리스트(["YYYY-MM-DD", ...])로 선택한 날들의 '완결(매수→매도)' 거래만 차트로 표시합니다.
    """
    if not dates:
        print("표시할 날짜 리스트가 비어 있습니다.")
        return

    conn = sqlite3.connect(TRADING_DB_PATH)

    # 전체 trading 읽고 스키마 정규화
    df_all = pl.read_database(
        """
        SELECT
            date, time, stockcode, buy_or_sell, quantity, price, fee,
            strategy, avg_price, profit, roi
        FROM trading
        """,
        connection=conn,
    )

    if len(df_all) == 0:
        conn.close()
        print("trading 테이블에 데이터가 없습니다.")
        return

    df_all = df_all.with_columns([
        pl.col("date").cast(pl.Utf8),
        pl.col("time").cast(pl.Utf8),
        pl.col("stockcode").cast(pl.Utf8),
        pl.col("buy_or_sell").cast(pl.Utf8),
        pl.col("strategy").cast(pl.Utf8),
        pl.col("quantity").cast(pl.Int64),
        pl.col("price").cast(pl.Float64),
        pl.col("fee").cast(pl.Float64),
        pl.col("avg_price").cast(pl.Float64),
        pl.col("profit").cast(pl.Float64),
        pl.col("roi").cast(pl.Float64),
    ])

    # date 정규화: "YYYY-MM-DD" 또는 "YYYYMMDD"를 모두 받아 최종 "YYYY-MM-DD"로 통일
    parsed_dash = pl.col("date").str.strptime(pl.Date, format="%Y-%m-%d", strict=False)
    parsed_compact = pl.col("date").str.strptime(pl.Date, format="%Y%m%d", strict=False)
    df_all = df_all.with_columns(
        pl.coalesce([parsed_dash, parsed_compact]).dt.strftime("%Y-%m-%d").alias("date")
    )

    # 요청한 날짜만 필터링
    df_sel = df_all.filter(pl.col("date").is_in(dates))

    if len(df_sel) == 0:
        conn.close()
        print("요청한 날짜에 해당하는 거래가 없습니다.")
        return

    # 날짜별로 페어링 후 차트
    trades: List[Trade] = []
    for g in df_sel.partition_by("date", as_dict=False):
        trades.extend(_pair_day_trades(g))

    conn.close()

    if not trades:
        print("완결(매수→매도) 거래를 찾지 못했습니다.")
        return

    # 보기 좋게 날짜, 종목, 진입시간 정렬
    trades.sort(key=lambda t: (t.date, t.code, t.entry_time))
    print(f"🗓️ 선택한 날짜 {sorted(set(dates))} 에서 완결 {len(trades)}건 찾음")
    for t in trades:
        print(f"- {t.date} {t.code} {t.entry_time}→{t.exit_time} {t.reason} ROI={t.roi:+.2f}%")

    plot_trades_overview(trades)

# ===== 실행 예시 =====
if __name__ == "__main__":
    # 예: 임의 5건
    # plot_random_trades_from_db(n=5)

    # 원하는 날짜 리스트로 차트 그리기
    plot_date_trades_from_db(["2023-09-15","2023-10-20","2024-02-01"])
