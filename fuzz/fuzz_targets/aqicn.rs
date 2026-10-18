#![no_main]

use libfuzzer_sys::fuzz_target;
use weathervane::fuzz_api;

fuzz_target!(|data: &[u8]| {
    fuzz_api::aqicn(&String::from_utf8_lossy(data));
});
