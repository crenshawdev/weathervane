#![no_main]

use libfuzzer_sys::fuzz_target;
use weathervane::fuzz_api;

fuzz_target!(
    |input: (Vec<String>, String, Vec<(String, String)>, Vec<String>)| {
        let (place_names, country_code, codenames, area_names) = input;
        fuzz_api::meteoalarm_match(place_names, &country_code, codenames, area_names);
    }
);
