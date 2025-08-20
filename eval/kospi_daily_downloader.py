"""
KOSPI daily index downloader (2020-01-01 ~ 2025-04-30)
- Source: Public Data Portal - 금융위_지수시세정보 getStockMarketIndex
- Saves into SQLite DB (market_index.db -> table kospi_daily)
- Requires environment variable: DATA_GO_KR_SERVICE_KEY

Usage examples:
  DATA_GO_KR_SERVICE_KEY="YOUR_KEY" python kospi_index_downloader.py
  DATA_GO_KR_SERVICE_KEY="YOUR_KEY" python kospi_index_downloader.py --begin 20200101 --end 20250430 --db market_index.db
"""

import os
import sys
import time
import json
import argparse
import sqlite3
from typing import Any, Dict, List, Optional, Tuple, Union

import requests

BASE_URL = "https://apis.data.go.kr/1160100/service/GetMarketIndexInfoService/getStockMarketIndex"

# ✅ 발급받은 서비스키 (이미 인코딩된 값 그대로 넣음)
SERVICE_KEY = r"UDE3%2Fu8mFL8mU7ZOuifQt9CWy%2B3oA6fYcLufkXVQCdULe2D6NuKn2ARc54KKfVsnuJlVEAz8VMHTZ6%2BDzdtV%2Bw%3D%3D"

DEFAULT_BEGIN = "20200101"
DEFAULT_END = "20250430"
DEFAULT_DB = "kospi_daily.db"
IDX_NM = "코스피"
NUM_OF_ROWS = 1000  # try max per page
SLEEP_BETWEEN_CALLS_SEC = 0.12  # ~120ms (API limit 30 TPS)

def init_db(conn: sqlite3.Connection) -> None:
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS kospi_daily (
            basDt TEXT PRIMARY KEY,         -- 기준일자(YYYYMMDD)
            idxNm TEXT,
            idxCsf TEXT,
            epyItmsCnt TEXT,
            clpr TEXT,
            vs TEXT,
            fltRt TEXT,
            mkp TEXT,
            hipr TEXT,
            lopr TEXT,
            trqu TEXT,
            trPrc TEXT,
            lstgMrktTotAmt TEXT,
            lsYrEdVsFltRg TEXT,
            lsYrEdVsFltRt TEXT,
            yrWRcrdHgst TEXT,
            yrWRcrdHgstDt TEXT,
            yrWRcrdLwst TEXT,
            yrWRcrdLwstDt TEXT,
            basPntm TEXT,
            basIdx TEXT
        );
        """
    )
    conn.commit()

def upsert_item(conn: sqlite3.Connection, it: Dict[str, Any]) -> None:
    # map missing fields to ""
    def g(k: str) -> str:
        v = it.get(k)
        return "" if v is None else str(v).strip()

    conn.execute(
        """
        INSERT INTO kospi_daily (
            basDt, idxNm, idxCsf, epyItmsCnt, clpr, vs, fltRt, mkp, hipr, lopr,
            trqu, trPrc, lstgMrktTotAmt, lsYrEdVsFltRg, lsYrEdVsFltRt, yrWRcrdHgst,
            yrWRcrdHgstDt, yrWRcrdLwst, yrWRcrdLwstDt, basPntm, basIdx
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?
        )
        ON CONFLICT(basDt) DO UPDATE SET
            idxNm=excluded.idxNm,
            idxCsf=excluded.idxCsf,
            epyItmsCnt=excluded.epyItmsCnt,
            clpr=excluded.clpr,
            vs=excluded.vs,
            fltRt=excluded.fltRt,
            mkp=excluded.mkp,
            hipr=excluded.hipr,
            lopr=excluded.lopr,
            trqu=excluded.trqu,
            trPrc=excluded.trPrc,
            lstgMrktTotAmt=excluded.lstgMrktTotAmt,
            lsYrEdVsFltRg=excluded.lsYrEdVsFltRg,
            lsYrEdVsFltRt=excluded.lsYrEdVsFltRt,
            yrWRcrdHgst=excluded.yrWRcrdHgst,
            yrWRcrdHgstDt=excluded.yrWRcrdHgstDt,
            yrWRcrdLwst=excluded.yrWRcrdLwst,
            yrWRcrdLwstDt=excluded.yrWRcrdLwstDt,
            basPntm=excluded.basPntm,
            basIdx=excluded.basIdx
        ;
        """,
        (
            g("basDt"),
            g("idxNm"),
            g("idxCsf"),
            g("epyItmsCnt"),
            g("clpr"),
            g("vs"),
            g("fltRt"),
            g("mkp"),
            g("hipr"),
            g("lopr"),
            g("trqu"),
            g("trPrc"),
            g("lstgMrktTotAmt"),
            g("lsYrEdVsFltRg"),
            g("lsYrEdVsFltRt"),
            g("yrWRcrdHgst"),
            g("yrWRcrdHgstDt"),
            g("yrWRcrdLwst"),
            g("yrWRcrdLwstDt"),
            g("basPntm"),
            g("basIdx"),
        ),
    )

def fetch_page(params: Dict[str, Any], max_retries: int = 3, timeout: int = 15) -> Optional[Dict[str, Any]]:
    """Call the API once with retries; return parsed JSON dict or None on fatal error."""
    for attempt in range(1, max_retries + 1):
        try:
            r = requests.get(BASE_URL, params=params, timeout=timeout)
            if r.status_code != 200:
                print(f"[경고] HTTP {r.status_code} (pageNo={params.get('pageNo')}). 재시도 {attempt}/{max_retries}")
                time.sleep(0.8 * attempt)
                continue
            return r.json()
        except Exception as e:
            print(f"[경고] 요청/파싱 실패 (pageNo={params.get('pageNo')}): {e}. 재시도 {attempt}/{max_retries}")
            time.sleep(0.8 * attempt)
    return None

def normalize_items(items_field: Any) -> List[Dict[str, Any]]:
    if items_field is None:
        return []
    # items: { "item": ... }
    item = items_field.get("item") if isinstance(items_field, dict) else None
    if item is None:
        return []
    if isinstance(item, list):
        return item
    if isinstance(item, dict):
        return [item]
    return []

def main() -> None:
    parser = argparse.ArgumentParser(description="Download KOSPI daily index into SQLite.")
    parser.add_argument("--begin", default=DEFAULT_BEGIN, help="시작 기준일자(YYYYMMDD)")
    parser.add_argument("--end", default=DEFAULT_END, help="종료 기준일자(YYYYMMDD)")
    parser.add_argument("--db", default=DEFAULT_DB, help="SQLite DB 경로")
    parser.add_argument("--rows", type=int, default=NUM_OF_ROWS, help="페이지당 행수")
    parser.add_argument("--sleep", type=float, default=SLEEP_BETWEEN_CALLS_SEC, help="호출 간 슬립(초)")
    args = parser.parse_args()

    conn = sqlite3.connect(args.db)
    init_db(conn)
    cur = conn.cursor()

    inserted = 0
    page_no = 1

    while True:
        params = {
            "serviceKey": SERVICE_KEY,   # requests가 알아서 쿼리스트링 인코딩
            "resultType": "json",
            "idxNm": IDX_NM,
            "beginBasDt": args.begin,
            "endBasDt": args.end,
            "numOfRows": args.rows,
            "pageNo": page_no,
        }

        data = fetch_page(params)
        if data is None:
            print("[오류] API 호출 실패: 반복 실패로 중단합니다.")
            break

        # resultCode 체크
        try:
            header = data["response"]["header"]
            result_code = header.get("resultCode")
            result_msg = header.get("resultMsg")
        except Exception:
            print(f"[경고] 예상치 못한 응답 형태 (pageNo={page_no}).")
            result_code, result_msg = None, None

        if result_code != "00":
            print(f"[경고] API 코드 {result_code} / {result_msg} (pageNo={page_no}). 다음 페이지로 계속.")
            page_no += 1
            time.sleep(args.sleep)
            continue

        body = data["response"].get("body") if isinstance(data.get("response"), dict) else None
        if not body:
            print("[정보] 응답 body 없음. 종료합니다.")
            break

        items = normalize_items(body.get("items"))
        if not items:
            print("[정보] 더 이상 항목이 없습니다. 종료합니다.")
            break

        for it in items:
            if it.get("basDt"):  # 날짜가 있는 레코드만 저장
                upsert_item(conn, it)
                inserted += 1

        conn.commit()

        # 다음 페이지
        page_no += 1
        time.sleep(args.sleep)

        # 안전장치: totalCount 기반 대략적 상한
        total_count = body.get("totalCount") or 0
        if page_no > (int(total_count) // args.rows + 10):
            # 일부 API에서 totalCount 부정확할 수 있어 약간의 버퍼를 둠.
            pass

    cur.close()
    conn.close()
    print(f"[완료] 총 {inserted}건 저장/갱신 (DB: {args.db})")

if __name__ == "__main__":
    main()