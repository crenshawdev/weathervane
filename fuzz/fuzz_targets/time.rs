#![no_main]

use libfuzzer_sys::fuzz_target;
use weathervane::time::{format_hour, format_time, is_night_time, ParsedDate};

// The time and date strings come from Open-Meteo responses.
fuzz_target!(|input: (String, String, i32, bool)| {
    let (first, second, utc_offset_seconds, military_time) = input;
    let _ = format_hour(&first, military_time);
    let _ = format_time(&first, military_time);
    let _ = ParsedDate::from_iso(&first);
    let _ = is_night_time(&first, &second, utc_offset_seconds);
});
