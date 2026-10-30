# Changelog

## 0.14.0 — 2026-10-30

### Changed
- `fetch_alerts` returns `Err(Error::HttpStatus(code))` when NWS, MeteoAlarm or
  BOM answers with a non-2xx status, where it returned an empty list, so an
  outage no longer reads as a quiet day. ECCC is read from several
  directories, and one that is not there (404) is an office that issued
  nothing that day; the ECCC walk is an `Err` only when no file could be read
  and some request failed. Partial reads keep the alerts they got. The
  signature is unchanged. (#62)
- `CurrentWeather.pressure` is sea-level pressure, read from Open-Meteo's
  `pressure_msl` instead of `surface_pressure`. The NWS station overlay
  already gave sea-level values, so the two sources now agree. Near sea level
  the number barely moves; at altitude it is much higher than before. The
  field name, type and wire shape are unchanged. (#63)
- For NWS alerts, `Alert.expires` is when the event ends (`ends`), not when
  the message lapses and NWS reissues it, and falls back to the message's
  `expires` when NWS gives no end. An alert whose event has ended is dropped
  even while its message is still current. MeteoAlarm, ECCC and BOM are
  unchanged. (#54)
- `detect_region` decides `Region::Us` from the US outline instead of latitude
  bands and an Alaska box: the 50 states, DC, Puerto Rico, the US Virgin
  Islands, Guam, the Northern Mariana Islands and American Samoa, from the
  Census TIGER/Line 2023 state file, with the Canadian and Mexican borders at
  about 500 m. Canadian border cities such as Windsor, Sarnia and Victoria now
  route to ECCC, northern Mexico and the Bahamas no longer route to NWS, and
  the territories get NWS alerts and the NWS current-conditions overlay.
  Hawaii keeps its box. Open water more than about 33 km off the Atlantic or
  Alaskan coast now falls through to the Canada box. (#56)
- The shared HTTP client keeps two idle connections per host instead of none,
  so sequential requests to one host reuse a connection. Frontends should call
  `reset_http_client` on resume from suspend, so a connection that died across
  the suspend is not reused. (#67)

### Fixed
- North Macedonia gets MeteoAlarm alerts. Its feed slug is
  `republic-of-north-macedonia`; the old `north-macedonia` slug returned 404
  on both feeds. (#44)
- Ukraine is covered by MeteoAlarm through the `ukraine` feed. The feed tags
  no entry with an EMMA_ID, so a Ukrainian location gets the national feed
  with `region_filtered: false`. (#60)
- EMMA_IDs are matched by whole words, so Angers no longer resolves to Gers or
  Saint-Étienne to Ain, and hyphenated names such as `Maine-et-Loire` match
  `Maine et Loire`. Only a county or state may hold a codename, so a town named
  for a river, such as Saint-Laurent-du-Var, no longer resolves to the
  département named for the same river. (#49)
- French locations filter to their own département. France tags each entry's
  `cap:geocode` as NUTS3, so the EMMA_ID is read from the entry's area link
  instead. An entry with no EMMA_ID, such as France's coastal warnings, is
  kept only when its `areaDesc` is exactly the user's area. (#46)
- MeteoAlarm entries that repeat the same alert for the same area, once per
  polygon or once per geocode, are dropped, and the first is kept. (#50)
- A place name found inside a longer codename or area name counts only beside
  a label word ("Linz (Stadt)", "County Dublin") or as one of the places a
  zone is named for ("Brighton and Hove", "Bern-Mühleberg"), and a city label
  outranks a district label. Madrid no longer resolves to `Sierra de Madrid`,
  Linz picks `Linz (Stadt)` over `Linz (Land)`, and Ulm and Venezia no longer
  land in `Kreis Neu-Ulm` and `Friuli Venezia Giulia`. Some places that used
  to resolve by codename length now miss and get the national feed with
  `region_filtered: false`. (#58)
- The NWS current-conditions overlay skips a station reading older than 90
  minutes, or one with no timestamp, and tries the next station, so a station
  that stopped reporting no longer overwrites Open-Meteo's current conditions
  with its last reading. (#64)
- ECCC alerts are read from the office that issues them: Alberta from `CWWG`,
  everything at or north of 60N from `CWNT`, and Nunavik from `CWUL`. A point
  on 60N no longer falls back to `CWTO`. (#68)
- A poisoned HTTP client lock is treated as a cache miss instead of a panic,
  as the crate's other caches already do. (#65)

## 0.13.1 — 2026-10-20

### Fixed
- `detect_location` asks ipwho.is over HTTPS instead of ip-api.com over plain
  HTTP, whose free tier has no HTTPS. Anyone on the network path could rewrite
  the old reply and pick the user's detected location. There is no fallback to
  a plain-HTTP provider. `DetectedLocation` and the function's signature are
  unchanged. (#29)
- An ECCC CAP file with a far-future `<sent>` date and no `<expires>` no
  longer panics `fetch_alerts`. ECCC `<sent>` and `<expires>` are parsed as
  RFC 3339, as the NWS and MeteoAlarm paths already are, which limits years to
  four digits, so the 24-hour expiry fallback can't overflow. (#26)
- `network_stream` and `sleep_stream` drop signals that weren't sent by the
  connection that owns `org.freedesktop.NetworkManager` or
  `org.freedesktop.login1`. Before, any local process could send a
  `StateChanged` or `PrepareForSleep` straight to the connection and trigger a
  refresh. Each signal now costs one `GetNameOwner` call to the bus. (#30)
- The MeteoAlarm codename table is fetched from a pinned commit of
  `ktrue/Meteoalarm-warning` instead of its `master` branch, so a later push
  there can't change which region a European location filters to. The file is
  the same as `master` today. (#30)
- Logs no longer carry the user's location outside debug-level region
  matching. The reverse-geocode cache hit no longer logs coordinates, and the
  MeteoAlarm warnings for an EMMA_ID miss, an unfiltered feed and a
  local-name miss log how many place names were tried instead of the names,
  and no longer log the user's EMMA_ID. (#30)
- ECCC alerts show their latest version. Files were read oldest first and the
  first version claimed the alert, so a later update's text, severity and
  expiry were ignored and the alert vanished at the first version's expiry.
  (#26)
- ECCC alerts that have ended are no longer shown. ECCC ends an alert with an
  update marked `ended` or `transitioned_out`, not a `Cancel`, and those
  updates were shown as active alerts until they expired. (#26)
- Every English info block in an ECCC file is read, not only the first, so a
  user in an area listed in a later block gets that alert. (#26)
- Yesterday's ECCC alerts no longer disappear at 00:00 UTC. They're read from
  the dated archive alongside today's. (#26)

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
