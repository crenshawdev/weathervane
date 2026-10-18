#![no_main]

use libfuzzer_sys::fuzz_target;
use weathervane::fuzz_api;

fuzz_target!(|data: &[u8]| {
    fuzz_api::open_meteo_weather(data);
    fuzz_api::open_meteo_air_quality(data);
    fuzz_api::open_meteo_pollen(data);
    fuzz_api::open_meteo_geocoding(data);
});
