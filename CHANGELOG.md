# Changelog

## 0.9.1 — 2026-07-02

### Testing
- Workspace line coverage lifted 48.72% → 85.20% (measured via `cargo llvm-cov`).
  ~140 fixture-based tests added across 10 modules; no live HTTP, no D-Bus, no mocks.
  - `alerts.rs` 0.00% → 76.53% — NWS/BOM JSON parsers, MeteoAlarm/ECCC CAP XML
    parsers, `AlertSeverity::from_cap_string`, expiry filter, region dispatch.
  - `weather.rs` 0.00% → 89.08% — Open-Meteo response parse and JMA current-temp
    override decision.
  - `air_quality.rs` 10.26% → 88.39% — headline-AQI selection, US/EU category
    boundaries, `AirQualityData::standard()`, pollutant Option defaults.
  - `location.rs` 27.96% → 88.61% — geocoding parse, IP-geolocation parse,
    saved-location coord matching, imperial-units lookup.
  - `error.rs` 22.22% → 94.39% — `Display` for all 8 variants, source-layer PII
    scrub, `From<quick_xml::DeError>`, `From<reqwest::Error>`.
  - `codes.rs` 62.37% → 99.52% — every WMO code arm, `icon_name` day/night
    variants, `CompassDirection::as_str`.
  - `geo.rs` 70.13% → 94.49% — `get_meteoalarm_info` country/alias/case arms,
    `approximate_european_country` bounding boxes, `is_us_bounds` continental bands.
  - `time.rs` 74.51% → 99.41% — `format_time` branches, AM/PM boundaries,
    `format_chrono_time` 12-hour trim-zero, `is_night_time` unparseable fallback.
  - `weather_jma.rs` 71.95% → 87.70% — `select_temp_from_map` decision arms and
    `parse_station_entry` non-temp-station drop.

### Internal
- `select_temp_from_map` extracted as a private sync `fn` from
  `override_current_temp` in `src/weather_jma.rs` to make its decision logic
  testable without an async runtime. Behavior at the call site (including
  `tracing::debug!` messages) is unchanged.
- Private sync helpers extracted from the domain fetchers to support fixture
  tests without live HTTP: `nws_alerts_from_response`, `bom_alerts_from_response`,
  `meteoalarm_alerts_from_feed`, `resolve_current_temp`, `weather_from_open_meteo`,
  `resolve_headline_aqi`, `detected_from_ip_api`. All private; no wire impact.

### CI
- GitLab `coverage` job gated with `cargo llvm-cov --fail-under-lines 75`.
  Coverage regressions below 75% now fail the pipeline.

### Wire contract
- Byte-identical across the entire milestone. `tests/wire_contract.rs` and
  `tests/snapshots/` unchanged; no `cargo insta review` needed.

## 0.9.0 — 2026-07-01

### Security
- aqicn API token and full request URLs stripped from all five
  `tracing::debug!` sites in `src/client.rs` via `reqwest::Error::without_url()`.
  Enforced by five inline `wiremock` + `tracing-test` leak-assertion tests and a
  URL-PII sentinel assertion in `tests/wire_contract.rs`.
- `ip-api.com` coordinates range/NaN-validated in `detect_location()` before
  return. Out-of-range or non-finite lat/lon reject with `Error::LocationDetection`.
- D-Bus message deserialization in `src/network.rs` and `src/sleep.rs` made
  panic-safe: malformed signals now drop observably at debug level without
  breaking the stream.

### Reliability
- NaN-safe sort comparator for the JMA station table and observable
  debug-logged drop paths for malformed station entries in `src/weather_jma.rs`;
  three pinning tests added.
- Test-helper `.unwrap()` calls in `src/pollen.rs` and `src/time.rs` upgraded to
  `.expect()` with fixture context so a corrupted inline-JSON fixture or a
  regressed date literal names the failing test and parsing step.

### Wire contract
- Public serde shapes unchanged (`tests/wire_contract.rs` + snapshots pinned).

## 0.8.0

### Added
- `MeasurementSystem::precipitation_unit()` — returns the display label for
  precipitation depth: `"in"` (Imperial) or `"mm"` (Metric). Distinct from
  `precipitation_api_param()` (`"inch"`/`"mm"`), which is unchanged.
- `AirQualityData.aqi_source: AqiSource` — records which provider supplied the
  headline AQI for a given fetch: `Aqicn` when aqicn.org/US EPA data was used,
  `OpenMeteo` otherwise (Europe, no token, or aqicn fallback).
- `AqiSource { Aqicn, OpenMeteo }` enum (re-exported at the crate root as
  `weathervane::AqiSource`).

### Back-compat
- `aqi_source` is `#[serde(default)]` and defaults to `OpenMeteo`, so existing
  serialized `AirQualityData` payloads without the field still deserialize
  cleanly.

## 0.7.0

### Breaking
- `AirQualityData`: the `standard` field is removed. Use the new
  `standard()` method (derived from `category`).
- `AqiCategory` now serializes adjacent-tagged:
  `{"standard": "Us", "level": "Good"}` (was externally tagged
  `{"Us": "Good"}`).

### Added
- `Serialize`/`Deserialize` on all wire-crossing types: `WeatherData`,
  `AirQualityData`, `AqiCategory`, `UsAqiCategory`, `EuAqiCategory`, `Alert`,
  `AlertSeverity`, `PollenData`, `LocationResult`, `DetectedLocation`.
- `wire` module: `WireError`, `Envelope`, `EnvelopeError` — the frozen
  JSON shapes for the future daemon/CLI surfaces.
- `CONTRACT.md`: the v1 wire contract, enforced by snapshot tests
  (`tests/wire_contract.rs`).
