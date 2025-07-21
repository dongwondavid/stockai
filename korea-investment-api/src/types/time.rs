use crate::Error;
use chrono::{DateTime, LocalResult};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct Time(DateTime<chrono_tz::Tz>);

impl Time {
    pub fn now() -> Self {
        let now = SystemTime::now();
        let millis = now.duration_since(UNIX_EPOCH).unwrap().as_millis();
        let dt = DateTime::from_timestamp_millis(millis as i64).unwrap();
        Self(dt.with_timezone(&chrono_tz::Asia::Seoul))
    }
    pub fn parse(s: &str, fmt: &str) -> Result<Self, Error> {
        let naive_dt = chrono::NaiveDateTime::parse_from_str(s, fmt)?;
        match naive_dt.and_local_timezone(chrono_tz::Asia::Seoul) {
            LocalResult::None => Err(Error::InvalidData),
            LocalResult::Single(t) => Ok(Time(t)),
            LocalResult::Ambiguous(_, _) => Err(Error::InvalidData),
        }
    }
    pub fn inner(&self) -> DateTime<chrono_tz::Tz> {
        self.0
    }
    pub fn date(&self) -> String {
        format!("{}", self.0.format("%Y%m%d"))
    }
}
