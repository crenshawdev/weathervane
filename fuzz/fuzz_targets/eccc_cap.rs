#![no_main]

use libfuzzer_sys::fuzz_target;
use weathervane::fuzz_api;

fuzz_target!(|input: (f64, f64, String)| {
    let (latitude, longitude, xml) = input;
    fuzz_api::eccc_cap(latitude, longitude, &xml);
});
