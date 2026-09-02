# Changelog

## 0.13.0 — 2026-09-02

### Changed
- MeteoAlarm filtering runs in up to three stages. The EMMA_ID and area-name
  stages are unchanged. A third stage runs only when the area-name stage missed
  and one of the location's place names is in non-Latin script (Greek, Cyrillic
  or Hebrew). It reads the country's MeteoAlarm JSON feed once for its
  local-language area names, each paired with the English name the atom feed
  uses, runs the same matching over the local names, and keeps the entries whose
  area is the paired English name. Matching allows a little inflection, so
  Nominatim's genitive `Αττικής` meets the feed's `Αττική`. Latin-script
  locations never make that request. (#20)
- `region_filtered` is `false` only when no region could be matched for the
  location by any of the three stages. The wire shape is unchanged. (#20)

## 0.12.0 — 2026-09-02

### Changed
- MeteoAlarm filtering runs in two stages. The EMMA_ID stage is unchanged and,
  because the feed only lists regions that are alerting, remains the only one
  that can tell a quiet day from a miss. When it yields no usable filter — no
  EMMA_ID resolved, or the feed tags its entries under another scheme or none
  at all — the location's place names are matched against the feed's own
  `cap:areaDesc` values instead. Diacritics are folded and administrative
  affixes such as "Grad" and "region" are dropped, so `Grad Zagreb` reaches
  `Zagreb region`. An exact token match beats containment, there is no
  substring or prefix matching, and a place name that fits more than one region
  is a miss rather than a guess. This narrows the feed for Portugal and
  Croatia, whose codenames resolve no usable per-region EMMA_ID, and for the
  feeds the 0.11.0 census found carrying no EMMA_ID geocodes at all: bulgaria
  and france (NUTS3), hungary (NUTS2), and estonia, israel, latvia, norway,
  slovenia and sweden (no geocode). (#18)
- `region_filtered` is `false` only when no region could be matched for the
  location by either stage. The wire shape is unchanged. (#18)
- The MeteoAlarm codename list is cached for the process lifetime, in the shape
  of the station and geocode caches, so its 72 KB fetch happens once per
  process rather than on every alerts call. (#18)
- Reverse geocoding also reads `village` and `municipality`, searched between
  `town` and `county`. (#18)

## 0.11.0 — 2026-09-02

### Added
- `fetch_alerts_detailed(lat, lon) -> Result<AlertReport>` returns each alert
  with the provider's area name and whether the list was narrowed to the
  caller's area. `AlertEntry { alert, area_desc }` and
  `AlertReport { alerts, region_filtered }` are new public types. `area_desc`
  is MeteoAlarm `cap:areaDesc`, NWS `areaDesc`, the containing ECCC polygon's
  `areaDesc`, and `""` for BOM, which sends none. `fetch_alerts` is unchanged,
  now a thin wrapper that drops both. (#16)

### Fixed
- MeteoAlarm country detection reverse geocodes instead of guessing from a
  bounding box, and a failure returns `Err` rather than an empty alert list
  that reads like a quiet day. (#11)
- The user's EMMA_ID is picked by ranked match instead of `HashMap` iteration
  order. 336 of the 2237 codenames match more than one entry inside their own
  country; "Wien" matches 26 Austrian codenames, and the right one, AT010, came
  up about 1 run in 26. (#12)
- A MeteoAlarm feed that tags its entries under a scheme other than `EMMA_ID`
  no longer comes back empty while reporting itself as filtered. The match is
  gated on `valueName == "EMMA_ID"`, and a feed carrying none renders
  unfiltered with `region_filtered: false`. Across all 37 live feeds on
  2026-09-02, bulgaria and france are NUTS3, hungary is NUTS2, and estonia,
  israel, latvia, norway, slovenia and sweden send no geocode at all. (#16)

### Changed
- `reverse_geocode` caches per coordinate for the process lifetime, which
  Nominatim's usage policy requires of clients repeating a query. A consumer
  refreshing every 30 minutes sends one request per location per process
  instead of 48 a day. (#15)

### Wire contract
- Additive only. `AlertEntry` and `AlertReport` are new shapes; `Alert` is
  untouched and the 28 existing snapshots round-trip byte-identical.

### Dependencies
- `reqwest` 0.12 -> 0.13. (#8)

## 0.10.1 — 2026-08-26

### Security
- Update `quick-xml` from 0.37 to 0.41 to address RUSTSEC-2026-0194 and
  RUSTSEC-2026-0195.

## 0.10.0 — 2026-07-15

### Added
- `DailyForecast` gains four fields from the Open-Meteo daily block:
  `windspeed_max: f32`, `wind_direction: i32`,
  `compass_direction: CompassDirection`, and
  `precipitation_probability_max: Option<i32>`. Derived from
  `windspeed_10m_max`, `wind_direction_10m_dominant`, and
  `precipitation_probability_max`; `compass_direction` via
  `CompassDirection::from_degrees`.
- NWS current-observation override for US coordinates: live National Weather
  Service observations overlay the Open-Meteo base (temperature, wind,
  humidity, pressure, dew point, visibility), mirroring the existing AMeDAS
  temperature override for Japan. The two overrides are mutually exclusive by
  geography; both fall through to Open-Meteo on any failure. Modeled sky fields
  (weathercode/condition/cloud cover/UV) stay Open-Meteo. Internal
  `weather_nws` module; no new public type.

### Wire contract
- Additive only: the four new `DailyForecast` fields extend the daily-forecast
  shape. `tests/snapshots/` and `CONTRACT.md` updated accordingly; no field
  removed or retyped. Existing payloads round-trip
  (`precipitation_probability_max` is `Option`).

### Packaging
- `repository` and the outgoing HTTP `User-Agent` now point to
  `github.com/crenshawdev/weathervane` (the crate's home moved to GitHub).

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
