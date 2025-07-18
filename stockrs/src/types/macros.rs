#[macro_export]
macro_rules! local_time {
    ($date:expr, $hour:expr, $min:expr, $sec:expr) => {
        chrono::Local.from_local_datetime(&$date.and_time(chrono::NaiveTime::from_hms_opt($hour, $min, $sec).unwrap())).unwrap()
    };
}
