# Codebase Concerns

**Analysis Date:** 2026-06-30

## Tech Debt

**Reqwest Error Message Leakage:**
- Issue: When aqicn API calls fail, `client.rs` logs errors with `tracing::debug!("{ctx} request failed: {e}")`. If reqwest's error Display includes the full URL, this leaks the aqicn token from the query string to debug logs.
- Files: `src/client.rs` (lines 72, 75, 95, 99), `src/air_quality_aqicn.rs` (line 19-30)
- Impact: Credentials can be leaked if logs are captured, persisted, or forwarded to external log aggregators. Low risk if logs are ephemeral, critical if persisted.
- Fix approach: Strip sensitive query params (token, api_key) from URLs before logging. Create a sanitized URL wrapper that redacts query strings in error Display or catch reqwest errors before calling get_text/get_json and map to generic error kinds.

**Float Comparison Panics:**
- Issue: `weather_jma.rs` line 57 and 276 use `partial_cmp().unwrap()` on floats. NaN comparisons return None, causing panics. Line 57 uses `unwrap_or()` as mitigation but line 276 uses bare `unwrap()`.
- Files: `src/weather_jma.rs` (lines 57, 276)
- Impact: Malformed or edge-case station coordinates could crash the temperature override on rare inputs.
- Fix approach: Replace with explicit `unwrap_or(std::cmp::Ordering::Equal)` on both sites or use `total_cmp()` if Ord guarantee is needed.

**Test Fixture Fragility:**
- Issue: `pollen.rs` test helper `parse()` (line 108) calls `.unwrap()` on JSON deserialization without context. If test fixtures are corrupted or JSON format changes, the test panics with no useful message.
- Files: `src/pollen.rs` (lines 108, 124, 142, 158, 173), `src/time.rs` (line 141)
- Impact: Silent test failure masking fixture or logic bugs; unhelpful panic message for debugging.
- Fix approach: Use `.expect("parse {spec}")` with description or assert helper that annotates the fixture.

## Known Bugs

**D-Bus Stream Fallback is Silent:**
- Symptoms: If systemd D-Bus is unavailable, network monitoring (`network_stream()`) and suspend monitoring (`sleep_stream()`) fall back to infinite pending futures without yielding. The application runs but never learns about network state changes or suspend events.
- Files: `src/network.rs` (lines 25-29), `src/sleep.rs` (lines 23-27)
- Trigger: On systems without D-Bus (minimal containers, WSL without systemd) or if dbus-daemon is not running.
- Workaround: None — frontend must implement its own fallback polling or network detection if this crate's streams don't emit.

**Meteoalarm EMMA_ID Resolution Cascades:**
- Symptoms: When MeteoAlarm is the alert provider, finding the correct area requires three external calls: nominatim (reverse geocode), GitHub raw content (fetch codenames), then MeteoAlarm feed. If any fail, area filtering is skipped entirely.
- Files: `src/alerts.rs` (lines 204-256, 272)
- Trigger: Any transient failure in nominatim or GitHub CDN during fetch_meteoalarm_alerts.
- Workaround: Alerts still emit but are not area-filtered; caller receives all alerts for the country.

**Missing Test Coverage for Edge Cases:**
- Issue: Several parsing and validation paths have limited test coverage:
  - JMA station data validation (geo.rs) assumes all stations in the API response are well-formed
  - ECCC CAP polygon parsing (alerts.rs line 519) relies on point_in_polygon with no visible tests for edge coordinates
  - No tests for empty API responses or missing required fields in some providers
- Files: `src/weather_jma.rs`, `src/alerts.rs`, `src/geo.rs`
- Impact: Malformed API responses or boundary cases could silently skip data or return incorrect results.
- Priority: Medium — most providers return consistent schemas, but deviations could hide.

## Security Considerations

**API Token in Query String Leakage Risk:**
- Risk: aqicn API token is part of the URL query string (air_quality_aqicn.rs line 25). If error messages, tracing spans, or debug output capture the URL, the token is exposed.
- Files: `src/air_quality_aqicn.rs` (line 25), `src/client.rs` (get_json/get_text functions)
- Current mitigation: Errors are logged at debug level only; No PII contract test covers URL/token leakage.
- Recommendations: (1) Strip query params from URLs in error messages; (2) Add a tracing test that verifies no token substring appears in any error log; (3) Consider passing token as a header instead of query param if aqicn API supports it.

**Coordinates Embedded in External API Calls:**
- Risk: User coordinates are sent to multiple external services (nominatim, ip-api.com, NWS, MeteoAlarm, ECCC, BOM). While this is by design, if error logs or tracing captures request URLs, coordinates leak as PII.
- Files: `src/location.rs` (line 101 uses ip-api.com), `src/alerts.rs` (nominatim call line 206)
- Current mitigation: Wire contract test checks that error payloads don't leak coordinates; URL logging is at debug level.
- Recommendations: Sanitize URLs in debug traces; ensure trace/log forwarding to external systems respects debug-level filtering.

**No Validation on IP-API Response Fields:**
- Risk: ip-api.com response is trusted without validating lat/lon are plausible (e.g., within [-90, 90] and [-180, 180]). Malicious or corrupted responses could feed invalid coordinates to all downstream providers.
- Files: `src/location.rs` (lines 100-127)
- Current mitigation: None.
- Recommendations: Validate coordinate ranges before returning DetectedLocation; optionally verify the country name against ISO 3166 list.

**D-Bus Message Deserialization:**
- Risk: zbus::MessageStream deserialization in network_stream (line 67) and sleep_stream (line 65) deserializes untrusted D-Bus messages. While the filter checks message source and interface, a malformed or adversarial message could panic if deserialization fails.
- Files: `src/network.rs` (line 67), `src/sleep.rs` (line 65)
- Current mitigation: .ok()? silently ignores deserialization failures.
- Recommendations: Add debug logging for unexpected message formats to catch suspicious D-Bus activity; monitor for patterns.

## Performance Bottlenecks

**Haversine Distance Computed for Every Station on Every Override:**
- Problem: `override_current_temp()` computes haversine distance for every AMeDAS station in Japan (hundreds) on every weather fetch. Distance computation is O(n) and happens even if the override fails later.
- Files: `src/weather_jma.rs` (lines 53-57)
- Cause: Station list is cached but not pre-sorted or indexed by spatial partition.
- Improvement path: Pre-sort stations by region or use a simple bounding-box filter before haversine; cache sorted results.

**MeteoAlarm Area Filtering Uses Point-in-Polygon Every Alert:**
- Problem: `parse_meteoalarm_entry()` calls `point_in_polygon(lat, lon, poly)` for every alert area. Polygon strings are parsed and matched without caching.
- Files: `src/alerts.rs` (line 519)
- Cause: No memoization of polygon parsing or spatial indexing.
- Improvement path: Parse polygons once and cache; use spatial indexing if polygon count is high.

**ECCC Cascading HTML Parsing:**
- Problem: Fetching ECCC alerts requires parsing the directory listing (lines 420-436), then looping through hours (lines 438-443), then parsing CAP files. Three levels of HTTP requests + string parsing per alert cycle.
- Files: `src/alerts.rs` (lines 395-488)
- Cause: ECCC API structure requires this traversal; no batch endpoint exists.
- Improvement path: Cache hour directories per day; deduplicate CAP URLs; consider parallel HTTP requests if rate limits allow.

## Fragile Areas

**JMA Station Coordinate Parsing:**
- Files: `src/weather_jma.rs` (lines 103-129)
- Why fragile: The function assumes `s.lat` and `s.lon` are exactly 2-element arrays (line 114 checks length). If the API changes to 1 or 3 elements, the station silently drops. No validation of lat/lon values.
- Safe modification: Add explicit error logging for skipped stations; validate lat/lon ranges.
- Test coverage: Covered by unit test at line 205-208 (deg_min conversion) but not the array-length check.

**MeteoAlarm Country Slug Resolution:**
- Files: `src/alerts.rs` (lines 264-269)
- Why fragile: `get_meteoalarm_info(country)` is called with the result of `detect_country_from_coords()` but if the country name format changes or isn't in the codenames map, alerts silently disappear for that region.
- Safe modification: Add debug logging when a country isn't found; provide fallback or skip gracefully.
- Test coverage: No tests for edge cases (country name variations, unmapped countries).

**Wire Contract Snapshot Tests:**
- Files: `tests/wire_contract.rs`
- Why fragile: The snapshot files (tests/snapshots/*.snap) are the ground truth. Any serde `#[serde(...)]` attribute change or field rename breaks the test. Recent change to add `aqi_source` with `#[serde(default)]` is correct, but future changes must follow the same pattern.
- Safe modification: Use `#[serde(default)]` for any new optional field; use `cargo insta review` to accept deliberate breaking changes; never use `#[serde(rename_all)]` without updating snapshots.
- Test coverage: Comprehensive; 29+ snapshot tests cover shape contracts and PII guards.

## Scaling Limits

**Global HTTP Client RwLock:**
- Current capacity: Single shared `reqwest::Client` behind an RwLock (src/client.rs line 28). Read-heavy workload (many concurrent fetches) is efficient; write (reset) is rare.
- Limit: If reset_http_client() is called during high concurrency, the write lock can cause brief stalls. No queuing or fairness; fairness depends on OS scheduler.
- Scaling path: If contention becomes visible in profiling, consider lock-free or tokio::RwLock; profile before optimizing.

**Station Cache in Memory:**
- Current capacity: All JMA AMeDAS stations (O(100s-1000s) entries) are cached in a Vec<Station> (src/weather_jma.rs line 42). Fresh fetch on cache miss happens once per process, no TTL.
- Limit: Cache is never invalidated; if JMA adds/removes stations, the running process sees stale data until restart.
- Scaling path: Add optional TTL-based cache expiry; consider periodic refresh if the app runs for days.

## Dependencies at Risk

**aqicn API Dependency:**
- Risk: aqicn.org is an optional feature (requires user token). If the API changes, breaks, or imposes rate limits, air quality queries degrade silently (fallback to Open-Meteo). No visibility into why aqicn was skipped.
- Impact: Users with aqicn tokens see US EPA AQI; without token or if aqicn fails, they see Open-Meteo AQI (which may differ in quality).
- Migration plan: AirQualityData now includes aqi_source field; frontend can inform user which provider was used. If aqicn sunsets, remove the optional path and hardcode Open-Meteo.

**MeteoAlarm Codenames GitHub CDN:**
- Risk: Fetches codenames from raw.githubusercontent.com (alerts.rs line 234). If GitHub is down or changes the URL, EMMA_ID resolution fails and MeteoAlarm alerts are unfiltered.
- Impact: Users in Europe see all alerts for their country, not just their region.
- Migration plan: Bake codenames into the crate or fetch once on startup with caching; add fallback to unfiltered alerts if fetch fails.

**IP-API.com Rate Limits:**
- Risk: Auto-location detection calls ip-api.com with no authentication (location.rs line 101). Free tier has rate limits; if user triggers detect_location() frequently, requests may be rate-limited or blocked.
- Impact: detect_location() returns Err(LocationDetection); auto-location is disabled.
- Migration plan: Use a quota-aware wrapper or cache detection results per session; document rate limit expectations; consider authenticated tier if this becomes a blocker.

**Nominatim Rate Limits:**
- Risk: EMMA_ID resolution calls nominatim.openstreetmap.org (alerts.rs line 205) with no authentication. Terms of Service require respectful rate-limiting; high-frequency or concurrent requests may be throttled.
- Impact: MeteoAlarm area filtering fails for that fetch cycle; all alerts are returned unfiltered.
- Migration plan: Cache nominatim results keyed by coordinate; add backoff/retry logic; consider batching or caching coordinates to reduce API load.

**Chrono DateTime Parsing:**
- Risk: Multiple alert providers parse RFC3339 and ISO timestamps. If an API returns non-standard formatting, DateTime::parse_from_rfc3339() fails silently and defaults to "now" or "now + 24h". Expired alerts may not expire correctly.
- Files: `src/alerts.rs` (lines 133-135, 137-142, 317-329, 539-551, 625-630), `src/time.rs` (lines 26-27, 39-40, 57, 90-91)
- Impact: Alert expiry can be incorrect; stale alerts may display longer than intended.
- Mitigation: Chrono is well-tested; APIs are unlikely to change format. Fallback defaults preserve data rather than losing it.

## Contract-Breaking Risks

**Wire Contract Enforcement:**
- Constraint: `tests/wire_contract.rs` snapshot tests are the law. Any change to serde-derived types (field addition, rename, removal, type change) that alters JSON shape breaks the snapshot test and is a breaking change.
- Locked types: `WeatherData`, `AirQualityData`, `AqiCategory`, `Alert`, `PollenData`, `LocationResult`, `DetectedLocation`, `WireError`, `Envelope`.
- Safe changes: Add new fields with `#[serde(default)]` (defers to Default or provided default); remove fields by deprecation + grace period; rename with `#[serde(alias)]` for back-compat.
- Recent safe change: `aqi_source` added with `#[serde(default)]` in v0.8.0; deserializes old payloads without the field, defaulting to OpenMeteo (line 132 in air_quality.rs).
- Anti-pattern: Using `#[serde(rename_all)]` (would break all field names) or removing a field without default.

**Enum Variant Spelling Contract:**
- Constraint: Enum variants serialize as PascalCase (e.g., `AlertSeverity::Severe` → `"Severe"`). Test at line 111-126 pins enum spellings.
- Example: `WeatherCondition::PartlyCloudy` serializes as `"PartlyCloudy"`, not `"partlyCloudy"` or `"Partly_Cloudy"`.
- Safe changes: Add new variants; reorder existing (order doesn't affect JSON). Never rename or swap variant names.

**Region Detection Hardcoded in Logic:**
- Constraint: Region detection (geo.rs) determines alert provider selection. If boundaries change, old configs may point to wrong provider. Not a breaking change but a UX shift.
- Files: `src/geo.rs` (lines 24-37)
- Example: Expanding Europe bounds to include Turkey would change alert provider for Turkey users.
- Safe changes: Tighten or expand bounds as needed; this doesn't break wire contracts. Document boundary changes in CHANGELOG.

## Missing Critical Features

**No Automatic Retry with Exponential Backoff:**
- Problem: API fetch failures (network timeouts, transient 5xx) are returned immediately without retry. Caller must implement retry logic.
- Blocks: Resilience; high packet-loss networks or temporarily unavailable APIs will fail immediately.
- Workaround: Frontend can retry; this crate is stateless.

**No Connection Pool Tuning Beyond Default:**
- Problem: HTTP client pool idle timeout and max-idle-per-host are hardcoded (client.rs lines 23-46). No way to tune for specific network conditions or load patterns.
- Blocks: Custom tuning for high-latency or congested networks.
- Workaround: Fork and rebuild; reset_http_client() and rebuild with custom config (but requires API change).

**No Data Compression or Caching Headers:**
- Problem: Requests don't set ETag or If-Modified-Since; responses aren't cached by the library. Every fetch is a full download.
- Blocks: Bandwidth optimization for mobile or metered networks.
- Workaround: Frontend implements caching; Open-Meteo and other providers support If-Modified-Since (but not used here).

**No Rate-Limit Aware Queuing:**
- Problem: If multiple API clients (this crate + other libraries) call aqicn or nominatim simultaneously, they may trip rate limits without coordination.
- Blocks: Robust operation in multi-client environments.
- Workaround: Caller implements queuing; document rate limits.

## Test Coverage Gaps

**ECCC CAP Polygon Parsing:**
- What's not tested: The point_in_polygon function is called in production but never tested with fixtures. Edge cases: polygon boundaries, holes, self-intersecting paths.
- Files: `src/geo.rs` (point_in_polygon implementation), `src/alerts.rs` (line 519 usage)
- Risk: Incorrect coordinate containment could silently filter wrong alerts.
- Priority: High — geographic logic is critical for alert filtering.

**BOM API Response Variations:**
- What's not tested: BOM warning type mapping (alerts.rs lines 617-622) is hardcoded; no tests for unmapped types or missing fields.
- Files: `src/alerts.rs` (lines 595-658)
- Risk: New or unexpected warning_group_type values default to AlertSeverity::Unknown; no visibility into what was missed.
- Priority: Medium — BOM coverage is smaller than US/EU, changes are less frequent.

**Region Detection Boundary Cases:**
- What's not tested: Coordinates exactly on region boundaries (e.g., 49.0 latitude at US-Canada border) are not tested. Floating-point comparison in is_us_bounds() uses inclusive ranges which should handle this, but edge cases aren't validated.
- Files: `src/geo.rs` (is_us_bounds, is_canada_bounds, etc.)
- Risk: Boundary coordinates may map to wrong region, changing alert provider.
- Priority: Medium — rare in practice (users are rarely exactly on a border).

**Silent Default Fallback Paths:**
- What's not tested: Multiple locations swallow errors and default to fallback values (0.0, empty string, now, 24h future). No tests verify these defaults are sensible for all APIs.
- Files: `src/air_quality.rs` (lines 192-211), `src/alerts.rs` (multiple unwrap_or patterns)
- Risk: Hidden data availability issues (e.g., missing AQI defaulting to 0, which looks like "Good" to some callers).
- Priority: Medium — defaults are conservative but opaque.

---

*Concerns audit: 2026-06-30*
