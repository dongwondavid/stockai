import sqlite3

DB_PATH = r"D:\db\stock_price(1day)_with_data.db"
STOCK_CODES = [
    "A005930", "A000660", "A233740", "A252670", "A122630", "A196170", "A069500", "A105560", "A025950", "A045660",
    "A459580", "A012450", "A251340", "A035420", "A010130", "A035900", "A173130", "A035720", "A229200", "A005380",
    "A377300", "A000100", "A034020", "A084690", "A053800", "A051910", "A114800", "A042660", "A005490", "A328130"
]
DATE = "20241206"  # Rust 로그에서 전일로 사용된 날짜

conn = sqlite3.connect(DB_PATH)
cursor = conn.cursor()

for code in STOCK_CODES:
    # 테이블 존재 여부
    tables = [t[0] for t in cursor.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall()]
    if code not in tables:
        print(f"[X] 테이블 없음: {code}")
        continue

    # 해당 날짜 row 존재 여부
    rows = cursor.execute(f'SELECT * FROM "{code}" WHERE date=?', (DATE,)).fetchall()
    if rows:
        print(f"[O] {code} {DATE} 데이터 존재: {len(rows)}개")
    else:
        print(f"[ ] {code} 테이블에 {DATE} 데이터 없음")

conn.close()