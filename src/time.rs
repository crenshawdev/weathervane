// SPDX-License-Identifier: MIT OR Apache-2.0

use chrono::{Datelike, NaiveDate, Weekday};

/// Structured date parts for frontend formatting.
///
/// Core parses the ISO date string and returns the components.
/// Frontend maps `weekday` and `month` to translated names.
pub struct ParsedDate {
    pub weekday: Weekday,
    pub month: u32,
    pub day: u32,
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

/// Determines if current time is night (before sunrise or after sunset).
/// Falls back to 6pm-6am if parsing fails.
pub fn is_night_time(sunrise: &str, sunset: &str) -> bool {
    use chrono::{Local, NaiveDateTime, TimeZone, Timelike};

    let now = Local::now();

    let parse_time = |time_str: &str| -> Option<chrono::DateTime<Local>> {
        NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M"))
            .ok()
            .and_then(|naive| Local.from_local_datetime(&naive).single())
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
        dt.format("%I:%M %p")
            .to_string()
            .trim_start_matches('0')
            .to_string()
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
