//! Date helpers.
//!
//! Domain dates are naive local `YYYY-MM-DD` strings. They are deliberately not
//! `DateTime<Utc>`: the original schema stored them as text and comparisons
//! happen as strings, which keeps ordering chronological and avoids timezone
//! drift on a device that moves timezones.

use chrono::{Datelike, Duration, Local, NaiveDate, Utc};

pub const ISO: &str = "%Y-%m-%d";

/// The SQLite timestamp format used by `datetime('now')`.
pub const STAMP: &str = "%Y-%m-%d %H:%M:%S";

/// Today's local date.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn today_iso() -> String {
    today().format(ISO).to_string()
}

/// The timestamp written to `created_at`/`updated_at`.
///
/// These columns defaulted to SQLite's `datetime('now')`, which is UTC, so the
/// writes here are UTC too: mixing local stamps into the same columns would make
/// the existing rows sort wrong. `today` stays local, as in the original server.
pub fn now_stamp() -> String {
    Utc::now().format(STAMP).to_string()
}

/// Parse a domain date. Tolerates the timestamp format so a column that holds
/// `2026-03-01 08:00:00` still parses as a date.
pub fn parse(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(s, ISO)
        .ok()
        .or_else(|| NaiveDate::parse_from_str(&s[..s.len().min(10)], ISO).ok())
}

/// Add days across calendar boundaries and DST transitions. `addDays` goes
/// through the calendar rather than adding 86 400 second spans, so a day is a
/// day even when the wall clock shifts.
pub fn add_days(d: NaiveDate, days: i64) -> NaiveDate {
    d + Duration::days(days)
}

/// Whole days from `from` to `to`. Negative when `to` precedes `from`.
pub fn days_between(from: NaiveDate, to: NaiveDate) -> i64 {
    (to - from).num_days()
}

/// `days` days ago, inclusive window start (a 7 day window is 6 days back).
pub fn window_start(ref_date: NaiveDate, days: i64) -> NaiveDate {
    add_days(ref_date, -(days - 1))
}

/// The same calendar day `months` in the past.
///
/// Mirrors `setMonth(getMonth() - n)` semantics from the original, including
/// its overflow behaviour (31 March minus one month lands in early March).
pub fn months_ago(ref_date: NaiveDate, months: i64) -> NaiveDate {
    let total = ref_date.year() as i64 * 12 + (ref_date.month0() as i64 - months);
    let year = total.div_euclid(12);
    let month = total.rem_euclid(12) as u32 + 1;
    let max_day = days_in_month(year, month);
    // Built from parts rather than stepped with `with_month`, because stepping
    // validates the day against the new month and 31 March minus one month is
    // not a real February date.
    NaiveDate::from_ymd_opt(year as i32, month, ref_date.day().min(max_day)).unwrap_or(ref_date)
}

fn days_in_month(year: i64, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first = NaiveDate::from_ymd_opt(year as i32, month, 1).unwrap_or(NaiveDate::MIN);
    let next = NaiveDate::from_ymd_opt(ny as i32, nm, 1).unwrap_or(NaiveDate::MAX);
    (next - first).num_days() as u32
}

pub fn year_of(d: NaiveDate) -> i64 {
    d.year() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        parse(s).unwrap()
    }

    #[test]
    fn add_days_crosses_months_and_years() {
        assert_eq!(add_days(d("2026-02-27"), 3), d("2026-03-02"));
        assert_eq!(add_days(d("2026-12-31"), 1), d("2027-01-01"));
        assert_eq!(add_days(d("2026-01-01"), -1), d("2025-12-31"));
    }

    #[test]
    fn window_start_is_inclusive() {
        // 7 day window = today plus the 6 days before it.
        assert_eq!(
            days_between(window_start(d("2026-03-07"), 7), d("2026-03-07")),
            6
        );
    }

    #[test]
    fn days_between_is_signed() {
        assert_eq!(days_between(d("2026-03-01"), d("2026-03-10")), 9);
        assert_eq!(days_between(d("2026-03-10"), d("2026-03-01")), -9);
        assert_eq!(days_between(d("2026-03-01"), d("2026-03-01")), 0);
    }

    #[test]
    fn parse_tolerates_timestamps() {
        assert_eq!(parse("2026-03-01 08:00:00"), Some(d("2026-03-01")));
        assert_eq!(parse(""), None);
        assert_eq!(parse("nonsense"), None);
    }

    #[test]
    fn months_ago_clamps_short_months() {
        assert_eq!(months_ago(d("2026-03-31"), 1), d("2026-02-28"));
        assert_eq!(months_ago(d("2026-06-15"), 12), d("2025-06-15"));
    }
}
