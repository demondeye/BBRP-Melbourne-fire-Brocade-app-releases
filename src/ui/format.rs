//! Dates and times, written the same way the web version showed them ("8:46 PM", "Wed, Oct 7").

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone};

pub fn local(ms: i64) -> DateTime<Local> {
    Local.timestamp_millis_opt(ms).single().unwrap_or_else(Local::now)
}

/// "8:46 PM"
pub fn time(t: DateTime<Local>) -> String {
    t.format("%-I:%M %p").to_string()
}
pub fn time_ms(ms: i64) -> String {
    time(local(ms))
}
/// "Wed, Oct 7"
pub fn day(t: DateTime<Local>) -> String {
    t.format("%a, %b %-d").to_string()
}
pub fn day_ms(ms: i64) -> String {
    day(local(ms))
}
/// "Wednesday, October 7"
pub fn long_date(t: DateTime<Local>) -> String {
    t.format("%A, %B %-d").to_string()
}
/// "October 2026"
pub fn month_label_key(key: &str) -> String {
    match parse_key(key) {
        Some(d) => d.format("%B %Y").to_string(),
        None => key.to_string(),
    }
}
/// "Oct"
pub fn month_short(month: u32) -> String {
    NaiveDate::from_ymd_opt(2000, month, 1).map(|d| d.format("%b").to_string()).unwrap_or_default()
}

/// "02:15:09"
pub fn duration(ms: i64) -> String {
    let s = (ms / 1000).max(0);
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}
/// "12h 05m" for monthly totals.
pub fn hours(ms: i64) -> String {
    let m = (ms / 60000).max(0);
    format!("{}h {:02}m", m / 60, m % 60)
}

/// "2026-10" for the month a time falls in.
pub fn month_key(ms: i64) -> String {
    let d = local(ms);
    format!("{}-{:02}", d.year(), d.month())
}
fn parse_key(key: &str) -> Option<NaiveDate> {
    let (y, m) = key.split_once('-')?;
    NaiveDate::from_ymd_opt(y.parse().ok()?, m.parse().ok()?, 1)
}
/// The month `step` months before/after a "YYYY-MM" key.
pub fn shift_key(key: &str, step: i32) -> String {
    let Some(d) = parse_key(key) else { return key.to_string() };
    let total = d.year() * 12 + d.month0() as i32 + step;
    format!("{}-{:02}", total.div_euclid(12), total.rem_euclid(12) + 1)
}
