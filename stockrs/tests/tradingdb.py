import sqlite3
import os

def sum_profits_from_trading_table():
    """
    stockrs/trading.db의 trading 테이블에서 profit 컬럼값을 모두 더한 결과를 출력
    """
    # 데이터베이스 파일 경로
    db_path = os.path.join(os.path.dirname(__file__), '..', 'trading.db')
    
    try:
        # SQLite 데이터베이스 연결
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        
        # trading 테이블의 profit 컬럼 합계 조회
        cursor.execute("SELECT SUM(profit) FROM trading")
        result = cursor.fetchone()
        
        # 결과 출력
        total_profit = result[0] if result[0] is not None else 0
        print(f"Trading 테이블의 총 수익: {total_profit:,.2f}")
        
        # 추가 정보: 레코드 수와 평균 수익
        cursor.execute("SELECT COUNT(*), AVG(profit) FROM trading")
        count_result = cursor.fetchone()
        record_count = count_result[0]
        avg_profit = count_result[1] if count_result[1] is not None else 0
        
        print(f"총 거래 건수: {record_count:,}건")
        print(f"평균 수익: {avg_profit:,.2f}")
        
        # 수익이 있는 거래와 손실이 있는 거래 개수
        cursor.execute("SELECT COUNT(*) FROM trading WHERE profit > 0")
        profit_count = cursor.fetchone()[0]
        
        cursor.execute("SELECT COUNT(*) FROM trading WHERE profit < 0")
        loss_count = cursor.fetchone()[0]
        
        print(f"수익 거래: {profit_count:,}건")
        print(f"손실 거래: {loss_count:,}건")
        
    except sqlite3.Error as e:
        print(f"데이터베이스 오류: {e}")
    except FileNotFoundError:
        print(f"데이터베이스 파일을 찾을 수 없습니다: {db_path}")
    finally:
        if 'conn' in locals():
            conn.close()

if __name__ == "__main__":
    sum_profits_from_trading_table()
