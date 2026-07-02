// SPDX-License-Identifier: MIT OR Apache-2.0

//! Date and time parsing for ISO timestamps from the weather API.

use chrono::{Datelike, NaiveDate, Weekday};

/// Structured date parts for frontend formatting.
///
/// Core parses the ISO date string and returns the components.
/// Frontend maps `weekday` and `month` to translated names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedDate {
    /// Day of week (Monday, Tuesday, etc.)
    pub weekday: Weekday,
    /// Month number, 1-12.
    pub month: u32,
    /// Day of month, 1-31.
    pub day: u32,
    /// Four-digit year.
    pub year: i32,
}

impl ParsedDate {
    /// Parses an ISO date string (e.g. "2025-11-25") into structured parts.
    /// Returns None if the string doesn't match the expected format.
    pub fn from_iso(date_str: &str) -> Option<Self> {
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
        Some(Self {
            weekday: date.weekday(),
            month: date.month(),
            day: date.day(),
            year: date.year(),
        })
    }
}

/// Formats an ISO timestamp to hour display (e.g. "14:00" or "2:00 PM").
pub fn format_hour(time_str: &str, military_time: bool) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_str) {
        return format_chrono_time(&dt, military_time);
    }

    // Fallback: parse "2025-01-20T14:00" manually
    if let Some(hour) = time_str
        .split('T')
        .nth(1)
        .and_then(|t| t.split(':').next()?.parse::<u32>().ok())
    {
        return format_hour_minute(hour, 0, military_time);
    }

    time_str.to_string()
}

/// Formats an ISO timestamp to time display (e.g. "14:30" or "2:30 PM").
pub fn format_time(time_str: &str, military_time: bool) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_str) {
        return format_chrono_time(&dt, military_time);
    }

    // Fallback: parse "2025-01-20T06:30:00" manually
    if let Some(time_part) = time_str.split('T').nth(1) {
        let parts: Vec<&str> = time_part.split(':').collect();
        if let (Some(Ok(hour)), Some(Ok(minute))) = (
            parts.first().map(|s| s.parse::<u32>()),
            parts.get(1).map(|s| s.parse::<u32>()),
        ) {
            return format_hour_minute(hour, minute, military_time);
        }
    }

    time_str.to_string()
}

/// Determines if it is currently night (before sunrise or after sunset) at the
/// location the forecast is for. Falls back to 6pm-6am if parsing fails.
///
/// `sunrise`/`sunset` are naive timestamps in the *location's* local time (as
/// returned by Open-Meteo's `timezone=auto`). `utc_offset_seconds` is that
/// location's offset east of UTC (`WeatherData::utc_offset_seconds`); it is
/// used to derive "now" in the same local frame, so the result is correct even
/// when the machine running this code is in a different timezone.
pub fn is_night_time(sunrise: &str, sunset: &str, utc_offset_seconds: i32) -> bool {
    use chrono::{Duration, NaiveDateTime, Timelike, Utc};

    // "Now" in the location's local frame: UTC plus the location's offset.
    let now = Utc::now().naive_utc() + Duration::seconds(utc_offset_seconds as i64);

    let parse_time = |time_str: &str| -> Option<NaiveDateTime> {
        NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M"))
            .ok()
    };

    match (parse_time(sunrise), parse_time(sunset)) {
        (Some(sunrise_time), Some(sunset_time)) => now < sunrise_time || now > sunset_time,
        _ => {
            let hour = now.hour();
            !(6..18).contains(&hour)
        }
    }
}

/// Formats a chrono DateTime according to the time format preference.
fn format_chrono_time<Tz: chrono::TimeZone>(
    dt: &chrono::DateTime<Tz>,
    military_time: bool,
) -> String
where
    Tz::Offset: std::fmt::Display,
{
    if military_time {
        dt.format("%H:%M").to_string()
    } else {
        let formatted = dt.format("%I:%M %p").to_string();
        formatted.trim_start_matches('0').to_string()
    }
}

/// Formats hour and minute values according to the time format preference.
fn format_hour_minute(hour: u32, minute: u32, military_time: bool) -> String {
    if military_time {
        format!("{:02}:{:02}", hour, minute)
    } else {
        let (display_hour, period) = match hour {
            0 => (12, "AM"),
            1..=11 => (hour, "AM"),
            12 => (12, "PM"),
            _ => (hour - 12, "PM"),
        };
        format!("{}:{:02} {}", display_hour, minute, period)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsed_date_from_iso() {
        let d = ParsedDate::from_iso("2025-11-25")
            .expect("parsed_date_from_iso: ISO date \"2025-11-25\" should parse");
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 11);
        assert_eq!(d.day, 25);
        assert!(ParsedDate::from_iso("not-a-date").is_none());
    }

    #[test]
    fn format_hour_handles_rfc3339_and_naive() {
        assert_eq!(format_hour("2025-01-20T14:00:00+09:00", true), "14:00");
        assert_eq!(format_hour("2025-01-20T14:00", true), "14:00");
        assert_eq!(format_hour("2025-01-20T00:00", false), "12:00 AM");
    }

    // The day/night window tests use far-past / far-future bounds so the real
    // "now" always falls clearly inside or outside, regardless of the offset
    // (any plausible UTC offset is well under a day) or when the test runs.
    #[test]
    fn night_when_now_is_outside_the_window() {
        // Daylight window entirely in the past → now is after sunset → night.
        assert!(is_night_time(
            "1970-01-01T06:00:00",
            "1970-01-01T18:00:00",
            0
        ));
        // Daylight window entirely in the future → now is before sunrise → night.
        assert!(is_night_time(
            "2999-01-01T06:00:00",
            "2999-01-01T18:00:00",
            9 * 3600
        ));
    }

    #[test]
    fn day_when_now_is_inside_the_window() {
        // Window spans all of recorded time → now is between → not night.
        assert!(!is_night_time(
            "1970-01-01T00:00:00",
            "2999-12-31T23:59:59",
            -5 * 3600
        ));
    }

    // Group A — format_time branches: RFC3339 (military + 12h trim-zero), naive
    // fallback (military + 12h), and unparseable-returns-input.

    #[test]
    fn format_time_rfc3339_military() {
        assert_eq!(format_time("2025-01-20T06:30:45+09:00", true), "06:30");
    }

    #[test]
    fn format_time_rfc3339_12h_trims_leading_zero() {
        // format_chrono_time's 12-hour branch formats "06:30 AM" then
        // trim_start_matches('0') strips the leading zero -> "6:30 AM".
        assert_eq!(format_time("2025-01-20T06:30:00+09:00", false), "6:30 AM");
    }

    #[test]
    fn format_time_naive_military() {
        assert_eq!(format_time("2025-01-20T14:30:00", true), "14:30");
    }

    #[test]
    fn format_time_naive_12h() {
        assert_eq!(format_time("2025-01-20T14:30:00", false), "2:30 PM");
    }

    #[test]
    fn format_time_unparseable_returns_input() {
        assert_eq!(format_time("garbage", true), "garbage");
        assert_eq!(format_time("2025-01-20", true), "2025-01-20");
    }

    // Group B — format_hour_minute AM/PM boundary arms, reached via format_time
    // so the fallback path is exercised end-to-end.

    #[test]
    fn format_hour_minute_boundary_12_pm_and_pm_arm() {
        // 12 -> (12, "PM")
        assert_eq!(format_time("2025-01-20T12:00:00", false), "12:00 PM");
        // _ -> (hour - 12, "PM")
        assert_eq!(format_time("2025-01-20T13:15:00", false), "1:15 PM");
        assert_eq!(format_time("2025-01-20T23:59:00", false), "11:59 PM");
        // 0 -> (12, "AM"), widened to a non-zero minute.
        assert_eq!(format_time("2025-01-20T00:15:00", false), "12:15 AM");
    }

    #[test]
    fn format_hour_minute_military_passthrough_pads_zeros() {
        assert_eq!(format_time("2025-01-20T05:07:00", true), "05:07");
    }

    // Group C — is_night_time unparseable-input fallback. Both cases pass
    // unparseable sunrise/sunset so the `_ => !(6..18).contains(&hour)` arm
    // runs; the offset is computed so the shifted local hour lands on a known
    // day/night bucket regardless of the current wall-clock hour.

    #[test]
    fn is_night_time_unparseable_fallback_returns_false_in_day_bucket() {
        use chrono::Timelike;
        let now_utc_hour = chrono::Utc::now().naive_utc().hour() as i64;
        let target_hour = 12i64;
        let offset_hours = (target_hour - now_utc_hour).rem_euclid(24);
        let offset_seconds = (offset_hours * 3600) as i32;
        assert!(!is_night_time(
            "garbage-no-T-separator",
            "also-garbage",
            offset_seconds
        ));
    }

    #[test]
    fn is_night_time_unparseable_fallback_returns_true_in_night_bucket() {
        use chrono::Timelike;
        let now_utc_hour = chrono::Utc::now().naive_utc().hour() as i64;
        let target_hour = 22i64;
        let offset_hours = (target_hour - now_utc_hour).rem_euclid(24);
        let offset_seconds = (offset_hours * 3600) as i32;
        assert!(is_night_time(
            "garbage-no-T-separator",
            "also-garbage",
            offset_seconds
        ));
    }

    #[test]
    fn is_night_time_partial_parse_falls_through_to_fallback() {
        use chrono::Timelike;
        let now_utc_hour = chrono::Utc::now().naive_utc().hour() as i64;
        let target_hour = 12i64;
        let offset_hours = (target_hour - now_utc_hour).rem_euclid(24);
        let day_offset_seconds = (offset_hours * 3600) as i32;
        assert!(!is_night_time(
            "2025-01-20T06:00:00",
            "garbage-end",
            day_offset_seconds
        ));
        assert!(!is_night_time(
            "garbage-start",
            "2025-01-20T18:00:00",
            day_offset_seconds
        ));
    }

    // Group D — format_hour unparseable-input fallback. "99:00" is NOT used
    // because "99" parses as u32 successfully and reaches format_hour_minute
    // instead of the terminal fallback.

    #[test]
    fn format_hour_unparseable_returns_input() {
        assert_eq!(
            format_hour("garbage-no-T-separator", true),
            "garbage-no-T-separator"
        );
        assert_eq!(format_hour("2025-01-20Tabc", true), "2025-01-20Tabc");
        assert_eq!(format_hour("2025-01-20T:00", true), "2025-01-20T:00");
    }
}
