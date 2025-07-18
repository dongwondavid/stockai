use rusqlite::{Connection, Result};
use crate::types::trading::Trading;
use std::path::PathBuf;
use crate::types::data_reader::{DataReader, DataReaderType};
use crate::data_reader::make_data_reader;

pub struct DBManager {
    conn: Connection,
    data_reader: Box<dyn DataReader>,
}

impl DBManager {
    pub fn new(path: PathBuf, data_reader_type: DataReaderType) -> Result<Self> {
        let conn = Connection::open(path)?;
        let data_reader = make_data_reader(data_reader_type);

        // Create trading table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS trading (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT,
                time TEXT,
                stockcode TEXT,
                buy_or_sell TEXT,
                quantity INTEGER,
                price REAL,
                fee REAL,
                strategy TEXT,
                avg_price REAL,
                profit REAL,
                roi REAL
            )",
            (),
        )?;

        // Create overview table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS overview (
                date TEXT PRIMARY KEY,
                open REAL,
                high REAL,
                low REAL,
                close REAL,
                volume INTEGER,
                turnover REAL,
                profit REAL,
                roi REAL,
                fee REAL
            )",
            (),
        )?;

        Ok(Self { conn, data_reader })
    }

    // Save trading data to database
    pub fn save_trading(&self, trading: Trading) -> Result<()> {
        let avg_price = self.data_reader.get_avg_price(trading.get_stockcode().to_string()).unwrap();
        let trading_result = trading.to_trading_result(avg_price);
        
        // Insert trading data
        self.conn.execute(
            "INSERT INTO trading (
                date, time, stockcode, buy_or_sell, quantity, 
                price, fee, strategy, avg_price, profit, roi
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            trading_result.to_db_tuple(),
        )?;

        Ok(())
    }

    // Initialize today's overview data
    pub fn insert_overview(&self) -> Result<()> {
        let result = self.data_reader.get_asset_info().unwrap();
        let date = result.get_date();
        let asset = result.get_asset();

        // Insert overview data
        self.conn.execute(
            "INSERT INTO overview (date, open, high, low) VALUES (?, ?, ?, ?)",
            (date.date().to_string(), asset, asset, asset),
        )?;

        Ok(())
    }

    // Update today's overview data
    pub fn update_overview(&self) -> Result<()> {
        let result = self.data_reader.get_asset_info().unwrap();
        let date = result.get_date();
        let asset = result.get_asset();

        // Get today's high and low values
        let (high, low) = self.conn.query_row(
            "SELECT high, low FROM overview WHERE date = ?",
            (date.date().to_string(),),
            |row| {
                let high: f64 = row.get(0)?;
                let low: f64 = row.get(1)?;
                Ok((high, low))
            },
        )?;

        // Update with new values
        let new_high = high.max(asset);
        let new_low = low.min(asset);

        self.conn.execute(
            "UPDATE overview SET high = ?, low = ? WHERE date = ?",
            (new_high, new_low, date.date().to_string()),
        )?;

        Ok(())
    }

    // Finalize today's overview data
    pub fn finish_overview(&self) -> Result<()> {
        let result = self.data_reader.get_asset_info().unwrap();
        let date = result.get_date();
        let asset = result.get_asset();

        let open: f64 = self.conn.query_row(
            "SELECT open FROM overview WHERE date = ?",
            (date.date().to_string(),),
            |row| row.get(0),
        )?;

        let close = asset;

        let daily_profit = close - open;
        let daily_roi = daily_profit / open * 100.0;

        // 오늘 날짜의 수수료, 총 거래대금 조회
        let fee_sum: Option<f64> = self.conn.query_row(
            "SELECT SUM(fee) FROM trading WHERE date = ?",
            (date.date().to_string(),),
            |row| row.get(0),
        )?;

        let turnover_sum: Option<f64> = self.conn.query_row(
            "SELECT SUM(price * quantity) FROM trading WHERE date = ?",
            (date.date().to_string(),),
            |row| row.get(0),
        )?;

        // Update with new values
        let fee = fee_sum.unwrap_or(0.0);
        let turnover = turnover_sum.unwrap_or(0.0);

        self.conn.execute(
            "UPDATE overview SET close = ?, profit = ?, roi = ?, fee = ?, turnover = ? WHERE date = ?",
            (close, daily_profit, daily_roi, fee, turnover, date.date().to_string()),
        )?;

        Ok(())
    }
}

/// 생명주기 패턴 추가 - prototype.py와 동일
impl DBManager {
    /// db_manager 시작 시 호출 - prototype.py의 self.db_manager.on_start()
    pub fn on_start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: db_manager 초기화 로직
        Ok(())
    }

    /// db_manager 이벤트 처리 - prototype.py의 self.db_manager.on_event(broker_result)
    pub fn on_event(&mut self, _broker_result: ()) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: broker 결과 처리 로직
        Ok(())
    }

    /// db_manager 종료 시 호출 - prototype.py의 self.db_manager.on_end()
    pub fn on_end(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: db_manager 정리 로직
        Ok(())
    }
}