# Phase 3: Alerts Parser Coverage - Research

**Researched:** 2026-07-01
**Domain:** Rust unit testing of existing parse logic (no new features); JSON/XML deserialization fixtures; `cargo-llvm-cov` line coverage
**Confidence:** HIGH — every finding below is grounded in direct reads of `src/alerts.rs`, `src/geo.rs`, `src/error.rs`, `src/client.rs`, sibling modules' existing test conventions, and a live `cargo llvm-cov` run against this exact tree.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-------------------|
| COV-01 | `alerts.rs` is exercised by tests covering each regional parser (NWS JSON, MeteoAlarm XML, ECCC, BOM), expired-alert filtering, and region dispatch | Testability Shape (per-parser extraction plan), Fixture Strategy table, Expiry Filter + Dispatch section, Code Examples — covers all four parsers, the shared expiry logic, and the dispatch decision (`detect_region` + `fetch_alerts`'s `Unknown` branch) |
</phase_requirements>

## Summary

`src/alerts.rs` is 0.00% covered (394/394 lines, 50/50 functions missed per the live `cargo llvm-cov` run below). The file is NOT uniformly untestable: two of the four regional parsers already have their core transform logic in standalone, synchronous, pure functions (`parse_meteoalarm_entry`, `parse_eccc_cap`) that can be unit-tested today with zero code changes. The other two (NWS, BOM) have their transform logic inlined inside `async fn fetch_*` closures alongside the live HTTP call — these need a small, mechanical extraction (pull the existing `.filter_map(...).collect()` block into a new private sync function) to become testable without HTTP. No async runtime is needed for any of the new tests: every parser's actual decode-and-filter logic is synchronous string/struct transformation: only the outer `fetch_*` wrappers are `async` (because they call `http_client()`), and none of those wrappers need to run in tests.

The codebase already has an established idiom for exactly this situation: `air_quality_aqicn.rs::extract_aqi(json_text: &str) -> Option<i32>` is doc-commented "Lives in its own function so it can be unit-tested against fixtures without a live network," with fixtures as inline `r#"..."#` string literals in a `#[cfg(test)] mod tests` block at the bottom of the same file. Follow that same shape for alerts.rs — do not introduce `tests/fixtures/*.json` files (no such directory exists anywhere in this repo) or `insta` snapshots (insta is reserved for `tests/wire_contract.rs` only; no other module in the crate uses it for unit tests).

Region dispatch (`fetch_alerts`, lines 65–78) is a straight 1:1 match on `detect_region()` (already implemented and unit-tested in `geo.rs`). Three of its four live-provider arms cannot be exercised without live HTTP or an injectable base URL — both out of scope per `REQUIREMENTS.md`. Proving "region dispatch" for COV-01 realistically means: (a) the `Region::Unknown` arm of `fetch_alerts` IS directly callable and synchronously returns `Ok(vec![])` without touching the network — test it directly; (b) the coordinate→Region mapping that *drives* the dispatch decision is proven via `detect_region()` assertions for one representative coordinate per supported region, mirroring the existing test style in `geo.rs`. The four match arms that invoke live-network `fetch_*` functions (lines 67, 72, 74, 75) will remain uncovered lines — a small (~5 line), deliberate, and acceptable gap given the "no live HTTP" constraint and the "substantial majority, not 100%" bar.

**Primary recommendation:** Extract two small sync helpers (`nws_alerts_from_response`, `bom_alerts_from_response`) mirroring the codebase's existing `extract_aqi`-style testability pattern; write fixture-based `#[test]` functions (not `#[tokio::test]`) directly against `parse_meteoalarm_entry`, a new `parse_meteoalarm_feed` wrapper, `parse_eccc_cap`, and the two new NWS/BOM helpers, using inline XML/JSON string literals in a `#[cfg(test)] mod tests` block appended to `src/alerts.rs`. This is projected to lift `alerts.rs` from 0% to roughly 65–75% line coverage without touching `Alert`, any wire type, or any HTTP-calling code path.

## Architectural Responsibility Map

This is a single-crate Rust library (no browser/frontend/API/DB tiers). Adapted tiers for this domain crate:

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Alert format parsing (JSON/XML decode + expiry filter) | Domain crate (`weathervane::alerts`) | — | Pure transform logic, no I/O; this phase's entire target |
| Region → provider dispatch | Domain crate (`weathervane::geo` + `alerts::fetch_alerts`) | — | Pure function (`detect_region`) plus a straight match; no I/O in the decision itself |
| Live HTTP fetch (NWS/MeteoAlarm/ECCC/BOM/Nominatim/codenames-CDN) | External provider (out of process) | Domain crate (caller, `client.rs`) | Explicitly out of scope for this phase's tests (`REQUIREMENTS.md` Out of Scope) |
| Coverage measurement | Rust toolchain (`cargo-llvm-cov`) | GitLab CI (`coverage` job, Phase 5) | Local measurement this phase; CI gate added in Phase 5 (COV-08) |

## Testability Shape of alerts.rs (per parser)

### 1. NWS (United States) — needs extraction

- `fetch_nws_alerts` (lines 108–165) is `async`, does the HTTP GET (lines 114–118), then deserializes (line 125: `let data: NwsAlertsResponse = response.json().await?;`), then the actual parse/filter/expiry logic is an inline `.filter_map` closure (lines 127–161).
- **Extraction needed:** pull lines 127–161 (the `let alerts: Vec<Alert> = data.features.into_iter().filter_map(...).collect();` block) into a new private sync function:
  ```rust
  fn nws_alerts_from_response(data: NwsAlertsResponse) -> Vec<Alert> { /* unchanged body */ }
  ```
  `fetch_nws_alerts` becomes `let alerts = nws_alerts_from_response(data);` — a one-line change, zero behavior change.
- **Why not extract further (raw `&str` in, `Result` out)?** `error.rs` has no `From<serde_json::Error> for Error` impl (confirmed by reading `error.rs` fully — only `From<reqwest::Error>` and `From<quick_xml::DeError>` exist). `fetch_nws_alerts` currently relies on `reqwest`'s `.json::<T>().await` to surface decode failures through the existing `reqwest::Error` → `Error::Parse(ParseKind::Json)` path (via `is_decode()`). Keeping the deserialize call (`response.json()`) inside the async fetch function and only extracting the *post-deserialize* transform avoids touching error-conversion plumbing at all — the lowest-risk cut. Test-side, construct `NwsAlertsResponse` via `serde_json::from_str::<NwsAlertsResponse>(FIXTURE).unwrap()` (fine to `unwrap()` in test code).
- `NwsAlertsResponse` / `NwsAlertFeature` / `NwsAlertProperties` (lines 85–105) are private structs — accessible from `mod tests` nested inside the same file via `use super::*;` (standard Rust visibility: descendant modules see private items of ancestors).

### 2. MeteoAlarm (Europe) — mostly already testable

- `parse_meteoalarm_entry(entry: MeteoAlarmEntry, user_emma_id: &Option<String>) -> Option<Alert>` (lines 304–355) is **already a standalone, pure, synchronous function**. No extraction needed — call it directly from tests with a hand-built or `quick_xml`-deserialized `MeteoAlarmEntry`.
- The feed-level XML decode + per-entry mapping (lines 286–292, inside `fetch_meteoalarm_alerts`) is inline:
  ```rust
  let feed: MeteoAlarmFeed = quick_xml::de::from_str(&xml_text)?;
  let alerts: Vec<Alert> = feed.entries.into_iter()
      .filter_map(|entry| parse_meteoalarm_entry(entry, &user_emma_id))
      .collect();
  ```
  **Recommended extraction** (small, optional but worthwhile — it is the only thing standing between "parser logic tested" and "XML→Vec<Alert> feed-level decode tested"): pull this into
  ```rust
  fn meteoalarm_alerts_from_feed(feed: MeteoAlarmFeed, user_emma_id: &Option<String>) -> Vec<Alert> { ... }
  ```
  Test-side: `quick_xml::de::from_str::<MeteoAlarmFeed>(FIXTURE_XML).unwrap()` then call the helper. This exercises the real XML-to-struct deserialization path (catching `#[serde(rename=...)]` mistakes) in addition to the pure mapping.
- `resolve_user_emma_id` (lines 204–256) does two live HTTP calls (Nominatim reverse geocode + a GitHub-raw codenames JSON fetch) — out of scope. Tests supply `user_emma_id` directly as `&Some("DE723".to_string())` / `&None`, bypassing HTTP entirely (this is exactly what the function signature already allows).

### 3. ECCC (Canada) — parser already fully testable; outer fetch has extra opportunity

- `parse_eccc_cap(xml: &str, lat: f64, lon: f64, seen_ids: &mut HashSet<String>) -> Option<Alert>` (lines 492–571) is **already a standalone, pure, synchronous function** taking raw XML text. Directly testable today with zero extraction. This is the strongest existing precedent in the file for "parse logic separated from fetch."
- `fetch_eccc_alerts` (lines 395–488) does live HTTP (directory listing crawl across `today/hour` paths, then per-`.cap`-file fetch) — out of scope, cannot run in tests.
- **Optional secondary extraction (recommended, not required by success criteria):** the HTML `href="..."` link-scraping logic used to discover hour-directories (lines 419–436) and `.cap` files (lines 451–464) is pure string parsing with no I/O, just currently inlined inside the HTTP loop. Extracting:
  ```rust
  fn extract_hour_dirs(html: &str) -> Vec<String> { ... }   // lines ~420-436 body
  fn extract_cap_files(html: &str) -> Vec<String> { ... }   // lines ~451-464 body
  ```
  adds ~25–30 more testable lines toward the "substantial majority" target, using tiny inline HTML fixtures (e.g. `r#"<a href="14/">14/</a>"#`). This is discretionary — success criterion 1 only requires the four *parsers* to decode fixtures; this extraction is extra credit toward the overall line-coverage number, not a hard requirement.

### 4. BOM (Australia) — needs extraction (same shape as NWS)

- `fetch_bom_alerts` (lines 596–658) is `async`, does the HTTP GET (line 603), deserializes (line 609: `let response_body: BomWarningsResponse = response.json().await?;`), then the parse/filter/expiry logic is an inline `.filter(...).filter_map(...)` chain (lines 612–655).
- **Extraction needed**, same pattern as NWS:
  ```rust
  fn bom_alerts_from_response(data: Vec<BomWarning>) -> Vec<Alert> { /* body of lines 612-655, operating on response_body.data */ }
  ```
  `fetch_bom_alerts` becomes `let alerts = bom_alerts_from_response(response_body.data);`.
- `BomWarningsResponse` / `BomWarning` (lines 578–593) are private — same descendant-module visibility rule applies for test fixtures.

### 5. Region dispatch (`fetch_alerts`, lines 65–78)

- `match detect_region(latitude, longitude) { Region::Us => ..., Region::Europe => ..., Region::Canada => ..., Region::Australia => ..., Region::Unknown => Ok(vec![]) }`.
- The `Region::Unknown => Ok(vec![])` arm (line 76) is the only branch that does **zero** network I/O — call `fetch_alerts(lat, lon).await` directly in a `#[tokio::test]` with a coordinate that `detect_region` maps to `Unknown` (e.g. Tokyo, 35.68/139.65 — already proven `Unknown` in `geo.rs`'s `detect_region_unknown_outside_coverage` test) and assert `Ok(vec![])`.
- The other four match arms cannot be exercised without either live HTTP (excluded) or an injectable base-URL refactor (out of scope — no such mechanism exists in `client.rs`, and adding one is a feature change, not a test change). **Accept these ~5 lines as an uncoverable gap** for this phase; document it explicitly so the planner doesn't chase 100%.
- **Proof of the actual routing decision** (which coordinates select which provider) is `detect_region()` — already implemented and already unit-tested in `geo.rs` (`detect_region_routes_us_cities`, `detect_region_routes_canadian_cities`, `detect_region_routes_europe_and_australia`, `detect_region_unknown_outside_coverage`). Nothing new is strictly required here for COV-01, but adding one `alerts.rs`-local test that documents the Region→parser-function mapping (e.g. a table-driven assertion pairing `Region` variants with which `fetch_*` function they dispatch to, by name/comment) makes success criterion 3 self-evidently satisfied from within `alerts.rs` itself rather than relying on cross-file inference.

## Fixture Strategy Per Format

| Parser | Input type | Smallest representative fixture | Where it lives |
|--------|-----------|----------------------------------|----------------|
| NWS | `NwsAlertsResponse` (deserialize from JSON `&str` in test) | GeoJSON `FeatureCollection`-shaped object: `{"features":[{"properties":{"id":"...","event":"Tornado Warning","severity":"Severe","headline":"...","description":"...","sent":"2026-06-01T12:00:00Z","expires":"2099-01-01T00:00:00Z"}}]}` — one expired variant (`expires` in the past, e.g. `2020-01-01...`), one with `expires: null` (exercise the `sent + 24h` fallback, line 142), one with `severity: null` (exercise `Unknown` fallback) | Inline `r#"..."#` in `#[cfg(test)] mod tests` at bottom of `alerts.rs` |
| MeteoAlarm | Raw XML `&str` deserialized via `quick_xml::de::from_str::<MeteoAlarmFeed>` | Atom-feed-shaped XML with one `<entry>` containing `id`, `title`, `identifier`, `event`, `severity`, `sent`, `expires`, and a `geocode` child with a `value`. **Landmine:** real MeteoAlarm feeds mix Atom-namespace elements (`id`, `title`) with CAP-namespace elements (typically prefixed, e.g. `cap:identifier`); `quick-xml`'s serde integration matches on local (unprefixed) tag name by default, so `#[serde(rename = "identifier")]` should match `<cap:identifier>` — but this is `[ASSUMED]`, not verified against a real captured MeteoAlarm payload in this session. **Recommend the planner run one throwaway/scratch deserialization check against a real (or namespace-prefixed) sample before finalizing the fixture**, to confirm quick-xml's actual namespace-matching behavior with this crate's `quick-xml 0.37` version, rather than trusting an un-prefixed fixture that might pass for the wrong reason. | Inline XML string literal |
| ECCC | Raw CAP XML `&str` via `quick_xml::de::from_str::<EcccCapAlert>` (called inside `parse_eccc_cap`) | CAP-shaped XML: `identifier`, `status` ("Actual" and "Cancel"/other variants for the reject-path tests), `msgType`, `sent`, one `<info>` block with `language` ("en-CA"), `event`, `severity`, `expires`, `headline`, `description`, and one `<area>` with `areaDesc` + a **simple** polygon (e.g. a small square `"0,0 10,0 10,10 0,10"`, matching `geo.rs`'s own `point_in_polygon_square` test style — do NOT use a real multi-hundred-vertex Canadian province polygon, it's unnecessary complexity) | Inline XML string literal |
| BOM | `Vec<BomWarning>` (deserialize the wrapper `BomWarningsResponse` from JSON `&str` in test, then pass `.data`) | `{"data":[{"id":"...","type":"severe_thunderstorm","short_title":"Severe Thunderstorm Warning","warning_group_type":"severe","phase":"active","expiry_time":"2099-01-01T00:00:00Z"}]}` plus one `phase: "cancelled"` variant (filtered at line 615) and one with `warning_group_type: "major"` (also maps to `Severe`, line 620 — landmine, test both strings map the same) | Inline `r#"..."#` JSON string literal |

No `tests/fixtures/` directory exists anywhere in this repo (`find . -iname "*fixture*"` returned nothing, no `include_str!` usage anywhere) — inline string literals inside the test module is the established and only pattern. Do not introduce a fixtures directory for this phase.

## Expiry Filter + Dispatch

- Expiry is computed identically (duplicated) in all four parsers: `if expires < Utc::now() { return None; }` — NWS line 144, MeteoAlarm line 331, ECCC line 553, BOM line 632. `Utc::now()` is called directly; it is **not** injectable in the current code, and this phase's "no live HTTP, extracted helpers and fixtures" mandate does not require making it injectable.
- **Recommended test strategy: do not refactor `Utc::now()` into an injectable parameter.** Instead, since real "now" is 2026-07-01, use fixture `expires` values that are unambiguously in the past (e.g. `2020-01-01T00:00:00Z`) or unambiguously in the future (e.g. `2099-01-01T00:00:00Z`). This proves the filter with zero production-code risk and remains valid for the practical lifetime of this test suite. `[ASSUMED: acceptable given no explicit CONTEXT.md constraint requiring deterministic time injection — flagged in Assumptions Log]`.
- Region dispatch: see "Region dispatch" subsection above. `detect_region()` (pure, `geo.rs:24`) is the actual decision function; representative coordinates already proven per-region in `geo.rs`'s existing test suite (reuse those coordinates for any `alerts.rs`-local dispatch test, don't invent new ones).

## Test Harness Conventions

- **Every** module in this crate (`client.rs`, `units.rs`, `geo.rs`, `time.rs`, `air_quality.rs`, `location.rs`, `network.rs`, `codes.rs`, `air_quality_aqicn.rs`, `weather_jma.rs`, `pollen.rs`, `sleep.rs`) uses an in-file `#[cfg(test)] mod tests { use super::*; ... }` block at the bottom of the source file. **None** use a separate `tests/*.rs` integration file for unit-level testing — those private structs (`NwsAlertsResponse`, `BomWarning`, etc.) couldn't be reached from an integration test anyway (integration tests only see the crate's public API). `alerts.rs` should follow this exact convention: add `#[cfg(test)] mod tests` at the end of `src/alerts.rs`.
- Plain `#[test]` fns are used throughout for sync logic (`geo.rs`, `air_quality_aqicn.rs`). `#[tokio::test]` is used only where the function under test is genuinely `async` (`client.rs`'s `get_json`/`get_text` tests, using `wiremock::MockServer`). **None of the new alerts.rs parser tests need `#[tokio::test]`** — `parse_meteoalarm_entry`, `parse_eccc_cap`, and the two new extracted helpers (`nws_alerts_from_response`, `bom_alerts_from_response`) are all synchronous. Only a direct test of `fetch_alerts(...)` with `Region::Unknown` coordinates (see above) needs `#[tokio::test]` (it's `async fn`, but doesn't touch the network on that branch).
- `insta` snapshot testing is used **exclusively** in `tests/wire_contract.rs` / `tests/snapshots/` for the frozen wire contract. No other module uses `insta` for its unit tests. Do not add `insta::assert_snapshot!` calls to the new `alerts.rs` tests — use plain `assert_eq!`/`assert!`, matching every other module's convention.
- `wiremock` (dev-dep, already in `Cargo.toml`) is used in `client.rs` to test the shared HTTP-error-swallowing helpers (`get_json`/`get_text`) against a local mock server. It is not needed for this phase — none of the extracted alerts.rs helpers do their own HTTP calls, so there's nothing to mock at this layer. (If the planner chooses to also cover `fetch_nws_alerts`/`fetch_bom_alerts`/`fetch_eccc_alerts`/`fetch_meteoalarm_alerts` themselves via `wiremock` pointed at hardcoded production hostnames — that is **not possible**; `wiremock` only intercepts requests to its own local server URI, and those functions have hardcoded `https://api.weather.gov`/etc. URLs with no injection point. This is exactly why the requirements correctly scope this phase to "extracted helpers and fixtures, no live HTTP.")
- The existing precedent most directly analogous to this phase's task is `air_quality_aqicn.rs::extract_aqi` (lines 38–50, tests at 52–91): a pure fn pulled out of an async fetch, doc-commented explicitly for testability, tested via 5 inline JSON string-literal fixtures covering: happy path, error status, missing field, malformed JSON, absent field. Mirror this shape and this level of fixture granularity per parser.
- Confirm the wire contract stays byte-identical: this phase must not touch `Alert`, `AlertSeverity`, or any other type re-exported in `tests/wire_contract.rs` (`alert_shape` test at line 284, `canary_alert_severity` at line 292). The extractions above only move code around inside private helper functions — they do not touch `Alert`'s field list, derive list, or any `#[serde(...)]` attribute. Run `cargo test --workspace` after the refactor and confirm `wire_contract::alert_shape` and all 22 existing tests still pass unchanged (baseline: 22 passed, 0 failed, confirmed by the coverage run below).

## Coverage Measurement

Confirmed working in this environment:
```bash
cargo llvm-cov --version   # cargo-llvm-cov 0.8.7 [VERIFIED: ran locally]
cargo llvm-cov --workspace --summary-only
```
Live baseline (2026-07-01, this session):
```
Filename         Regions  Missed Regions  Cover    Functions  Missed Functions  Executed   Lines  Missed Lines  Cover
alerts.rs            697             697   0.00%          50                50    0.00%       394           394  0.00%
TOTAL                                     48.72%                                                              48.72%
```
[VERIFIED: `cargo llvm-cov` executed in this session against the current tree]

Per-file numbers are already broken out in the default `--summary-only` table (no extra flag needed); to isolate just this file post-implementation: `cargo llvm-cov --workspace --summary-only | grep alerts.rs`, or for a line-by-line annotated view: `cargo llvm-cov --workspace --html` then open `target/llvm-cov/html/src/alerts.rs.html`.

CI already runs `cargo llvm-cov --no-report --workspace` → `cargo llvm-cov report --cobertura` → `cargo llvm-cov report --summary-only` in `.gitlab-ci.yml`'s `coverage` job (stage `test`, scrapes `TOTAL` via regex). This phase does not touch CI config (that's COV-08 / Phase 5) — just make sure the new tests run cleanly under this same invocation locally before considering the phase done.

## Landmines

### Pitfall 1: quick-xml namespace matching for MeteoAlarm fixture
**What goes wrong:** A hand-written fixture that omits CAP namespace prefixes (`cap:identifier` vs `identifier`) might deserialize "successfully" for the wrong reason (empty/optional fields silently `None`), giving false confidence.
**Why it happens:** `quick-xml`'s serde integration matches on local tag name, ignoring the namespace prefix, by default — but this crate's exact `quick-xml 0.37` config (`features = ["serialize"]`, no explicit namespace handling) hasn't been empirically re-verified against a real MeteoAlarm payload in this research session.
**How to avoid:** Before finalizing the MeteoAlarm fixture, write one throwaway test asserting every field is `Some(...)` with the exact expected value (not just "doesn't panic") using a fixture that includes namespace prefixes as MeteoAlarm's real feed does (`cap:identifier`, `cap:event`, etc.), confirming the prefix is stripped as expected.
**Warning signs:** A test that passes even when a field name is deliberately misspelled in the fixture — that means the field is silently defaulting to `None`/absent rather than actually being matched.

### Pitfall 2: BOM `phase` vs ECCC `status`/`msgType` casing differs
**What goes wrong:** ECCC filters `msgType == "Cancel"` (PascalCase) at line 499; BOM filters `phase.as_deref() != Some("cancelled")` (all-lowercase) at line 615. A test author who copies one fixture's casing convention into the other provider's fixture will get a silent false-pass or false-fail.
**Why it happens:** Two independent upstream APIs use different string conventions for conceptually the same "cancelled" state.
**How to avoid:** Test each provider's cancel/non-cancel filtering against its own exact literal string, verified by reading lines 495–501 (ECCC) and 615 (BOM) directly rather than assuming symmetry.

### Pitfall 3: BOM `warning_group_type` overlap
**What goes wrong:** Both `"major"` and `"severe"` map to `AlertSeverity::Severe` (line 620, `Some("major") | Some("severe") => AlertSeverity::Severe`). A test that only exercises one of the two strings leaves half the match arm's branch coverage on the table and risks missing a future regression if someone "simplifies" the match to only one string.
**How to avoid:** Include both `"major"` and `"severe"` fixture variants.

### Pitfall 4: ECCC dedup state is a shared, mutable `HashSet` across multiple `parse_eccc_cap` calls
**What goes wrong:** `parse_eccc_cap`'s `seen_ids: &mut HashSet<String>` parameter is populated across a loop of many CAP files in production (`fetch_eccc_alerts`'s nested loop, lines 403–484). A test that only calls `parse_eccc_cap` once per fresh `HashSet` never exercises the dedup path (lines 530–535) at all.
**How to avoid:** Write one test that calls `parse_eccc_cap` twice with two fixtures sharing the same `event`+`area_desc` combination against the same `seen_ids` set, asserting the second call returns `None`.

### Pitfall 5: `sent` field parse differs between ECCC and the other three providers
**What goes wrong:** ECCC parses `sent` via `.parse::<DateTime<chrono::FixedOffset>>()` (line 541) while NWS/MeteoAlarm/BOM use `DateTime::parse_from_rfc3339(...)` (lines 133, 320, 628 respectively). Both are RFC3339-compatible parses in `chrono`, but a fixture author might copy an NWS-style `Z`-suffixed timestamp into an ECCC fixture without confirming the offset-based (`-04:00` style, matching how ECCC's real CAP feed formats `sent`) form also round-trips through the different parse call.
**How to avoid:** Use an explicit-offset timestamp (e.g. `2026-06-01T08:00:00-04:00`) in the ECCC `sent` fixture at least once, not just a `Z`-suffixed one, to confirm both forms work through `FixedOffset::parse`.

### Pitfall 6: `AlertSeverity::from_cap_string` has zero direct tests today
**What goes wrong:** This helper (lines 35–43) is shared by NWS, MeteoAlarm, and ECCC (not BOM, which has its own inline mapping) but currently has no test coverage of its own, at any confidence level, in the 0%-baseline file. If only exercised indirectly through one parser's fixtures, the `"major"` → `Severe` alias and the `Unknown` fallback for garbage input may go untested.
**How to avoid:** Add a small dedicated test for `AlertSeverity::from_cap_string` covering all five input classes (`"minor"`, `"moderate"`, `"severe"`, `"major"`, `"extreme"`) plus an unrecognized string, independent of any single provider's fixture.

## Code Examples

### Established extraction-for-testability pattern in this codebase
```rust
// Source: src/air_quality_aqicn.rs:33-50 (existing code, cite as the pattern to follow)
/// Pulls the headline AQI out of an aqicn feed response. Lives in its own
/// function so it can be unit-tested against fixtures without a live network.
fn extract_aqi(json_text: &str) -> Option<i32> {
    let value: serde_json::Value = serde_json::from_str(json_text)
        .map_err(|e| tracing::debug!("aqicn response parse failed: {e}"))
        .ok()?;
    let status = value.get("status")?.as_str()?;
    if status != "ok" {
        tracing::debug!("aqicn returned non-ok status: {status}");
        return None;
    }
    value.get("data")?.get("aqi")?.as_i64().map(|v| v as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_aqi_from_ok_response() {
        let json = r#"{"status":"ok","data":{"aqi":52}}"#;
        assert_eq!(extract_aqi(json), Some(52));
    }
}
```

### Recommended shape for the NWS extraction (mechanical, not yet applied)
```rust
// Current (lines 125-161, abbreviated) — inline in async fetch_nws_alerts:
let data: NwsAlertsResponse = response.json().await?;
let alerts: Vec<Alert> = data.features.into_iter().filter_map(|feature| { /* ... */ }).collect();

// Recommended split:
async fn fetch_nws_alerts(latitude: f64, longitude: f64) -> Result<Vec<Alert>> {
    // ...unchanged HTTP call...
    let data: NwsAlertsResponse = response.json().await?;
    let alerts = nws_alerts_from_response(data);
    tracing::debug!("Fetched {} alert(s) from NWS", alerts.len());
    Ok(alerts)
}

/// Extracted so the decode+filter logic is unit-testable without a live network.
fn nws_alerts_from_response(data: NwsAlertsResponse) -> Vec<Alert> {
    data.features.into_iter().filter_map(|feature| { /* unchanged body */ }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nws_response_decodes_active_alert() {
        let json = r#"{"features":[{"properties":{
            "id":"NWS-IDP-PROD-123","event":"Tornado Warning","severity":"Severe",
            "headline":"Tornado Warning until 8 PM","description":"Take cover now.",
            "sent":"2026-06-01T12:00:00Z","expires":"2099-01-01T00:00:00Z"
        }}]}"#;
        let data: NwsAlertsResponse = serde_json::from_str(json).unwrap();
        let alerts = nws_alerts_from_response(data);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Severe);
    }

    #[test]
    fn nws_response_drops_expired_alert() {
        let json = r#"{"features":[{"properties":{
            "id":"x","event":"Test","severity":null,"headline":null,"description":null,
            "sent":"2020-01-01T00:00:00Z","expires":"2020-01-01T01:00:00Z"
        }}]}"#;
        let data: NwsAlertsResponse = serde_json::from_str(json).unwrap();
        assert!(nws_alerts_from_response(data).is_empty());
    }
}
```

### MeteoAlarm — already-extracted pure function, no code change needed
```rust
// Source: src/alerts.rs:304 (existing code)
fn parse_meteoalarm_entry(entry: MeteoAlarmEntry, user_emma_id: &Option<String>) -> Option<Alert> {
    // ... existing body, unchanged ...
}
// Test directly: construct MeteoAlarmEntry via quick_xml::de::from_str::<MeteoAlarmEntry>(xml_fragment)
// or via serde_json is NOT applicable here (MeteoAlarmEntry only derives Deserialize for XML use,
// but quick_xml::de::from_str works on any Deserialize impl — use it, not serde_json).
```

### ECCC — already-extracted pure function, no code change needed
```rust
// Source: src/alerts.rs:492 (existing code)
fn parse_eccc_cap(xml: &str, lat: f64, lon: f64, seen_ids: &mut HashSet<String>) -> Option<Alert> {
    // ... existing body, unchanged ...
}
// Test directly with a full CAP XML string fixture; call twice with the same seen_ids
// to prove dedup (Pitfall 4).
```

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | quick-xml 0.37's serde integration matches CAP-namespace-prefixed elements (`cap:identifier`) by local tag name, ignoring the namespace prefix, so an un-prefixed test fixture will still deserialize the way a real prefixed MeteoAlarm feed does | Fixture Strategy / Pitfall 1 | If wrong, the MeteoAlarm feed-level fixture test could pass for the wrong reason (fields silently `None`) and give false confidence; planner should verify with one throwaway assertion-heavy test before trusting the fixture, per Pitfall 1 |
| A2 | Using fixed calendar dates (`2020-01-01` / `2099-01-01`) instead of injecting a controllable "now" is an acceptable test strategy for the expiry filter, rather than refactoring `Utc::now()` into an injectable parameter | Expiry Filter + Dispatch | Low risk — only fails if this test suite is still running unmodified in the year 2099; acceptable given no explicit user constraint demanding deterministic time injection |
| A3 | ~65-75% is a realistic projected `alerts.rs` line-coverage outcome from the recommended extractions, without the optional ECCC HTML-scraping extraction | Summary | If the actual post-implementation number falls short of "substantial majority," the planner may need to add the optional ECCC `extract_hour_dirs`/`extract_cap_files` extraction (Testability Shape §3) to close the gap |

**If this table is empty:** N/A — see above.

## Open Questions (RESOLVED)

1. **Should the optional ECCC HTML-scraping extraction (`extract_hour_dirs`/`extract_cap_files`) be included in this phase's plan, or deferred?** — RESOLVED: Included as a conditional contingency in `03-02-PLAN.md` Task 3 STEP 1b (applied only if mid-Wave-2 coverage on `src/alerts.rs` is < 65%). Not a mandatory stretch task.
   - What we know: it is pure string parsing, low risk, and would meaningfully increase `alerts.rs` line coverage beyond the four core parsers.
   - What's unclear: whether "substantial majority" is reached without it (see A3).
   - Recommendation: plan it as a stretch task within Phase 3 (not a separate phase) — cheap to add, and directly reduces risk of falling short on success criterion 5.

2. **Exact numeric target for "substantial majority" of alerts.rs coverage.** — RESOLVED: 65% set as the working bar in `03-02-PLAN.md` Task 3 STEP 0; measured mid-Wave-2 (early pivot point) and at end of Wave 2 (final). Baseline captured pre-extraction in `03-01-PLAN.md` Task 1 STEP 0 with a pre-planned sccache-bypass invocation so the delta is measured, not assumed.
   - What we know: the roadmap's Phase 3 success criterion 5 says "rises from 0% to a substantial majority," with no specific percentage. The milestone-level bar (COV-07, Phase 5) is workspace-wide ≥70%.
   - What's unclear: whether the planner should set a specific per-file target (e.g. ≥65%) as a phase-level checkpoint.
   - Recommendation: track the actual number after implementation via `cargo llvm-cov --workspace --summary-only | grep alerts.rs` and treat ≥65% as the working bar (informed by the line-budget estimate above), but don't block phase completion on an exact percentage the roadmap didn't specify — the four success criteria (1-4) plus "measured via cargo llvm-cov" (5) are the actual gate.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All test/build | ✓ | rustc 1.96.1 | — |
| cargo-llvm-cov | Coverage measurement | ✓ | 0.8.7 | — |
| cargo fmt | CI gate | ✓ | 1.9.0-stable | — |
| cargo clippy | CI gate | ✓ | 0.1.96 | — |
| insta (dev-dep) | Wire contract tests only, not needed this phase | ✓ (already in Cargo.toml) | 1.x | — |
| wiremock (dev-dep) | Not needed for this phase's tests (see Test Harness Conventions) | ✓ (already in Cargo.toml) | 0.6 | N/A |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** none — everything required is already installed and already a dependency of the crate; this phase adds zero new packages.

## Package Legitimacy Audit

**Not applicable.** This phase adds no new dependencies. All tooling used (`serde_json`, `quick-xml`, `chrono`) is already a direct dependency in `Cargo.toml`; all test tooling (`insta`, `regex`, `tokio`, `tracing-test`, `wiremock`) is already a dev-dependency. No `npm view` / `pip index` / `cargo search` verification is needed because no package-legitimacy gate applies to a phase that installs nothing.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` harness via `cargo test` (no external test framework) |
| Config file | none — tests are `#[cfg(test)] mod tests` blocks compiled in-crate; `Cargo.toml` already declares needed dev-deps |
| Quick run command | `cargo test --lib alerts::` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| COV-01 | NWS JSON decodes active alert with all fields, drops expired, handles null `severity`/`expires` fallback | unit | `cargo test --lib alerts::tests::nws_ -- --nocapture` | ❌ Wave 0 — write in `src/alerts.rs` |
| COV-01 | MeteoAlarm XML decodes entry, filters by EMMA_ID, drops expired | unit | `cargo test --lib alerts::tests::meteoalarm_ -- --nocapture` | ❌ Wave 0 |
| COV-01 | ECCC CAP XML decodes alert, filters non-"Actual"/"Cancel" status, dedups by event+area, filters by polygon containment | unit | `cargo test --lib alerts::tests::eccc_ -- --nocapture` | ❌ Wave 0 |
| COV-01 | BOM JSON decodes warning, filters "cancelled" phase, maps "major"/"severe" both to Severe | unit | `cargo test --lib alerts::tests::bom_ -- --nocapture` | ❌ Wave 0 |
| COV-01 | Expired alerts dropped across all four providers; current alerts retained | unit | `cargo test --lib alerts::tests:: -- --nocapture` (covered by each provider's own expiry-case test) | ❌ Wave 0 |
| COV-01 | Region dispatch: representative coordinates per region select the correct parser path | unit | `cargo test --lib alerts::tests::dispatch_ -- --nocapture` + existing `cargo test --lib geo::tests::detect_region_` | ✓ (geo.rs existing) / ❌ (alerts.rs-local, Wave 0) |
| COV-01 | Wire contract unchanged | integration | `cargo test --test wire_contract` | ✓ existing |

### Sampling Rate
- **Per task commit:** `cargo test --lib alerts::` (fast, no HTTP, no network)
- **Per wave merge:** `cargo test --workspace` (all 22+ existing tests plus new ones) followed by `cargo llvm-cov --workspace --summary-only | grep alerts.rs` to confirm the coverage delta
- **Phase gate:** `cargo test --workspace` green, `cargo test --test wire_contract` unchanged (22 tests still passing), `cargo fmt --check` and `cargo clippy --workspace -- -D warnings` clean (both extraction refactors must pass clippy strict mode), `cargo llvm-cov --workspace --summary-only` shows `alerts.rs` risen substantially above 0%

### Wave 0 Gaps
- [ ] `src/alerts.rs` — add `#[cfg(test)] mod tests` block (does not exist yet; file currently has zero tests)
- [ ] Two extraction refactors needed before tests can be written: `nws_alerts_from_response` (NWS), `bom_alerts_from_response` (BOM) — both mechanical, ~35 and ~44 lines respectively, no behavior change
- [ ] Optional: `meteoalarm_alerts_from_feed` extraction (feed-level XML decode + mapping) — recommended, not strictly required (parse_meteoalarm_entry alone already covers the core parser logic requirement)
- [ ] Optional: `extract_hour_dirs`/`extract_cap_files` extraction from `fetch_eccc_alerts` — stretch task to push total file coverage higher (see Open Question 1)
- Framework install: none — `cargo test`, `cargo llvm-cov` already present and working

## Security Domain

`security_enforcement` is not explicitly disabled in `.planning/config.json`, so this section is included per policy. This phase is test-only (no new production behavior beyond mechanical logic extraction); it does not introduce new input surface, auth, sessions, or cryptography.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V2 Authentication | No | Crate has no auth; not touched by this phase |
| V3 Session Management | No | Stateless crate; not touched by this phase |
| V4 Access Control | No | N/A |
| V5 Input Validation | Yes (pre-existing, not newly introduced) | Already handled by `serde`/`quick-xml` typed deserialization (`Option<T>` fields tolerate missing/malformed data, `filter_map` drops unparseable entries) — this phase only adds tests proving that existing validation behavior, it does not add new parsing surface |
| V6 Cryptography | No | N/A |

### Known Threat Patterns for this stack
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| Malformed/adversarial XML or JSON from a regional alert provider | Denial of Service / Tampering | Already mitigated by typed `serde`/`quick-xml` deserialization returning `Result`/`Option` rather than panicking; this phase's fixtures should include at least one deliberately malformed/partial payload per format to prove the existing `filter_map`/`Option` handling doesn't panic (already implicitly covered by the "missing field" fixture variants listed above) |

## Sources

### Primary (HIGH confidence — direct code reads, this session)
- `src/alerts.rs` (full file, 659 lines) — all four parsers, dispatch, expiry logic
- `src/geo.rs` (full file) — `detect_region`, region bounding boxes, existing test conventions
- `src/error.rs` (full file) — confirmed no `From<serde_json::Error>` impl exists
- `src/client.rs` (full file, incl. tests) — `http_client()`/`get_json`/`get_text`, `wiremock` usage pattern
- `src/air_quality_aqicn.rs` (full file) — the established "extract for testability" idiom (`extract_aqi`)
- `src/weather_jma.rs` (tests section) — confirms sync `#[test]` convention, no fixture-file usage
- `src/location.rs` (tests section) — confirms in-file test convention
- `tests/wire_contract.rs` (relevant sections) — `Alert`/`AlertSeverity` wire-shape fixture, confirms frozen fields
- `Cargo.toml` — confirmed exact dev-dependency set (`insta`, `regex`, `tokio`, `tracing-test`, `wiremock`), no new deps needed
- `.gitlab-ci.yml` — confirmed exact `cargo-llvm-cov` CI invocation and `coverage` regex
- Live `cargo llvm-cov --workspace --summary-only` run in this session — confirmed 0.00% baseline for `alerts.rs` (394/394 lines, 50/50 functions), 48.72% workspace total, 22/22 tests passing
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/STATE.md`, `.planning/config.json` — phase scope, requirement IDs, milestone decisions, workflow flags

### Secondary / Tertiary
- None used — this research required no external web search or documentation lookup; everything needed was already present in the codebase and verifiable by direct execution.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; every tool/library already in use and verified present
- Architecture / extraction plan: HIGH — every line reference verified by direct file read; extraction pattern matches an existing, working precedent in the same codebase (`air_quality_aqicn.rs`)
- Pitfalls: HIGH for casing/dedup/mapping-overlap findings (verified by direct code read); MEDIUM/LOW (flagged `[ASSUMED]`, A1) for the quick-xml namespace-matching behavior specifically, since no real captured MeteoAlarm payload was fetched or tested against in this research session

**Research date:** 2026-07-01
**Valid until:** stable — this is internal-code research with no external API surface to go stale; re-verify only if `alerts.rs`, `error.rs`, or the `quick-xml`/`serde`/`chrono` dependency versions change before this phase is planned
