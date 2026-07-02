# Milestones

## v0.9.1 Test Coverage Lift (Shipped: 2026-07-02)

**Phases completed:** 3 phases, 11 plans, 33 tasks

**Key accomplishments:**

- Extracted NWS/BOM/MeteoAlarm transform logic into testable private sync helpers and proved both regional JSON parsers plus `AlertSeverity::from_cap_string` with 12 fixture-based tests, lifting `src/alerts.rs` line coverage from 0.00% to 46.37%.
- Proved MeteoAlarm and ECCC CAP XML parsers plus region-routing decisions with 13 fixture-based tests, no extraction or contingency needed — `src/alerts.rs` line coverage landed at 76.53%, clearing the 65% gate with an 11.5pp margin.
- Extracted the Open-Meteo response builder and JMA current-temp override decision out of `fetch_weather` into two private sync helpers and proved both with 9 fixture-based tests, lifting `src/weather.rs` line coverage from 0.00% to 89.08%.
- Extracted the headline-AQI selection ternary from `fetch_air_quality` into `resolve_headline_aqi` and proved it plus the US/EU category boundaries, `AirQualityData::standard()`, and pollutant Option-defaults with 21 fixture-based tests, lifting `src/air_quality.rs` line coverage from 10.26% to 88.39%.
- Extracted the IP-API success branch from detect_location into a testable private sync helper and proved all four named location surfaces (geocoding parse, IP-geolocation parse, saved-location coord matching, imperial-units lookup) with 17 fixture-based tests, lifting `src/location.rs` line coverage from 27.96% to 88.61%.
- src/error.rs line coverage lifted from 22.22% to 94.39% via a 17-test module proving Display strings, source-layer PII scrubbing, and the two From impls, with zero production code changes
- src/codes.rs line coverage lifted 62.37% -> 99.52% (+37.15pp) via 16 new fixture-based unit tests covering every WMO code arm, icon_name variant, and CompassDirection label
- src/geo.rs line coverage lifted 70.13% -> 94.49% (+24.36pp) via 8 new fixture-based unit tests covering get_meteoalarm_info country/alias/case arms, approximate_european_country bounding boxes, and is_us_bounds continental bands
- src/time.rs line coverage lifted 74.51% -> 99.41% (+24.90pp) via 11 new fixture-based unit tests covering format_time branches, format_hour_minute AM/PM boundaries, format_chrono_time's 12-hour trim-zero path, is_night_time's unparseable fallback, and format_hour's terminal fallback
- src/weather_jma.rs line coverage lifted 71.95% -> 87.70% (+15.75pp) by extracting select_temp_from_map as a private sync fn from override_current_temp and covering its 8 decision arms plus the parse_station_entry non-temp-station drop arm
- Workspace line coverage verified at 85.20% post-Wave-1 (10.20pp above the 75% gate), then `--fail-under-lines 75` landed in `.gitlab-ci.yml`'s `coverage` job as a single-line Form A edit, closing out the v0.9.1 Test Coverage Lift milestone (COV-06, COV-07, COV-08)

---

## v0.9 TechDebt (Shipped: 2026-07-01)

**Phases completed:** 2 phases, 5 plans, 5 tasks

**Key accomplishments:**

- aqicn token stripped from all five tracing::debug! sites in HTTP client using e.without_url(); five wiremock+tracing-test leak-assertion tests added inline; wire boundary pinned via wire_contract.rs sentinel assertion.
- ip-api.com coordinates range/NaN-validated in detect_location() before return, rejecting out-of-range/non-finite lat/lon with Error::LocationDetection (SEC-03).
- D-Bus message deserialization in network.rs and sleep.rs made panic-safe via decode helpers and explicit-match loops, malformed signals dropped observably at debug level without breaking the stream (SEC-04).
- NaN-safe test sort comparator plus observable (debug-logged), code-only drop paths for malformed JMA station table entries, with three new pinning tests.
- Replaced 4 bare `.unwrap()` calls in `src/pollen.rs` tests and upgraded the thin `.expect("valid ISO date")` in `src/time.rs` so a corrupted inline-JSON fixture or a regressed date literal now names the failing test and parsing step instead of an opaque panic.

---
