# Roadmap: weathervane — Tech Debt Security Audit (v0.8.0 → v1.0)

## Overview

Five phases harden the crate for a v1.0 crates.io release. High-risk security
and panic-elimination work ships first; dependency hygiene and test coverage
fill the middle; public-surface documentation and the CHANGELOG migration guide
close the milestone. Every phase produces a clean CI run and a discrete
CHANGELOG entry.

## Phases

- [ ] **Phase 1: Security Hardening** - Eliminate token/URL leakage from logs and add input validation for external API responses
- [ ] **Phase 2: Panic-path Elimination** - Replace all reachable panics in production code with safe alternatives
- [ ] **Phase 3: Dependency Hygiene** - Add cargo-audit and cargo-deny to CI; cache external CDN and geocoding calls
- [ ] **Phase 4: Test Coverage** - Fill correctness gaps in geographic logic, fallback paths, and provider-specific parsing
- [ ] **Phase 5: API Surface and v1.0 Cutover** - Freeze public surface, complete wire-contract snapshots, and publish the migration guide

## Phase Details

### Phase 1: Security Hardening
**Goal**: No API token, coordinate, or raw URL can appear in any log line or error payload produced by the crate
**Depends on**: Nothing (first phase)
**Requirements**: SEC-01, SEC-02, SEC-03, SEC-04, SEC-05
**Success Criteria** (what must be TRUE):
  1. `cargo test` passes new assertions in `tests/wire_contract.rs` that confirm no aqicn token substring and no un-sanitized URL appear in any debug-level log output during error conditions
  2. `get_json` and `get_text` in `client.rs` log only sanitized URLs (query params stripped); the aqicn token never appears in any emitted log line during normal or error operation
  3. `detect_location()` returns `Err(LocationDetection)` when ip-api.com responds with lat/lon outside `[-90, 90]` / `[-180, 180]`; a unit test covers this reject path
  4. A malformed D-Bus message delivered to `network_stream()` or `sleep_stream()` produces a `debug!` log entry instead of a panic; the stream continues
  5. All four CI gates pass: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo build --workspace`, `cargo test --workspace`
**Plans**: TBD

### Phase 2: Panic-path Elimination
**Goal**: No reachable panic exists in production code paths; test helpers carry descriptive context on failure
**Depends on**: Phase 1
**Requirements**: PANIC-01, PANIC-02, PANIC-03
**Success Criteria** (what must be TRUE):
  1. `weather_jma.rs` station sort no longer calls `.unwrap()` on `partial_cmp()`; a unit test feeding NaN station coordinates completes without panicking
  2. JMA station parsing emits a `debug!` log and skips stations whose `lat`/`lon` arrays are the wrong length or contain out-of-range values; no panic is reachable from malformed API data
  3. Every `.unwrap()` in `pollen.rs` and `time.rs` test helpers carries a `.expect("…")` message that names the fixture being parsed; a failing test prints an actionable message
  4. All four CI gates pass
**Plans**: TBD

### Phase 3: Dependency Hygiene
**Goal**: CI rejects known-vulnerable or non-compliant dependencies; MeteoAlarm and Nominatim calls do not repeat within a process lifetime for the same input
**Depends on**: Phase 2
**Requirements**: DEPS-01, DEPS-02, DEPS-03, DEPS-04
**Success Criteria** (what must be TRUE):
  1. `cargo audit` runs on every CI push; a RUSTSEC advisory at or above the configured severity threshold fails the build and blocks merge
  2. `cargo deny check` runs on every CI push with a committed `deny.toml`; an unlicensed or advisoried dependency fails the build
  3. MeteoAlarm codenames are fetched once and cached for the process lifetime; when raw.githubusercontent.com is unreachable the in-crate fallback is used and alerts still emit
  4. Nominatim reverse-geocode results are cached keyed by coordinate; a second `fetch_meteoalarm_alerts()` call for the same location makes zero additional Nominatim requests
  5. All four CI gates pass
**Plans**: TBD

### Phase 4: Test Coverage
**Goal**: Critical geographic containment logic, silent fallback paths, and provider-specific parsing are all covered by deterministic fixture tests
**Depends on**: Phase 3
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04
**Success Criteria** (what must be TRUE):
  1. `cargo test geo` passes fixture tests for `point_in_polygon` covering at minimum: a point inside a convex polygon, a point on a boundary edge, a point outside, a concave polygon with a hole, and a self-intersecting path
  2. Region-detection functions in `geo.rs` have tests for exact boundary coordinates (49.0°N at the US-Canada border, EU western and eastern edges); each returns the expected provider region without relying on runtime heuristics
  3. Tests assert that the silent-default fallback paths in `air_quality.rs` and `alerts.rs` produce values that are distinguishable from real zero data — or emit a debug log — so a missing AQI cannot silently read as "Good"
  4. BOM `warning_group_type` mapping has a test for every hardcoded variant and an explicit test confirming unrecognized types map to `AlertSeverity::Unknown` without panicking
  5. All four CI gates pass
**Plans**: TBD

### Phase 5: API Surface and v1.0 Cutover
**Goal**: Every public item is documented, the wire contract is exhaustively snapshot-tested, and consumers have a complete migration guide from v0.8.0
**Depends on**: Phase 4
**Requirements**: API-01, API-02, API-03, API-04
**Success Criteria** (what must be TRUE):
  1. Every `pub` item in `src/` is documented in `API.md` or demoted to private; `cargo doc --no-deps` builds without missing-docs warnings
  2. `tests/wire_contract.rs` has at least one `insta` snapshot covering each wire-crossing type listed in CONTRACT.md; `cargo insta test` passes with zero pending reviews
  3. `API.md` documents that `network_stream()` and `sleep_stream()` return an infinite pending future when D-Bus is unavailable; this contract is stated as a stable guarantee for v1.0
  4. `CHANGELOG.md` contains a "v0.8.0 → v1.0 Migration" section enumerating every behavioral change, deprecation, and wire-contract addition since v0.8.0; `cargo publish --dry-run` succeeds
  5. All four CI gates pass
**Plans**: TBD

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Security Hardening | 0/TBD | Not started | - |
| 2. Panic-path Elimination | 0/TBD | Not started | - |
| 3. Dependency Hygiene | 0/TBD | Not started | - |
| 4. Test Coverage | 0/TBD | Not started | - |
| 5. API Surface and v1.0 Cutover | 0/TBD | Not started | - |
