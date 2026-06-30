# Requirements: weathervane — v0.9 TechDebt

**Defined:** 2026-06-30
**Core Value:** Public types crossing process boundaries are wire-stable, and the four CLAUDE.md contracts (no i18n, silent regional fallthrough, no PII in errors/tracing, Linux-only streams degrade silently) hold across every code path.

## v0.9 Requirements

Two categories. Each maps to a roadmap phase. Scope is deliberately narrow — security first, then a general faults pass. Anything not listed here is out of scope for this milestone.

### Security (SEC)

- [ ] **SEC-01**: API tokens and `api_key` values do not appear in any tracing/error output. URLs are sanitized before logging in `client.rs` (lines 72, 75, 95, 99) and `air_quality_aqicn.rs` (lines 19-30).
- [ ] **SEC-02**: A wire-contract test asserts that no aqicn token substring appears in any error log produced by the crate.
- [ ] **SEC-03**: `ip-api.com` response coordinates are validated to lat ∈ [-90, 90] and lon ∈ [-180, 180] before being returned from `detect_location()` (`location.rs:100-127`).
- [ ] **SEC-04**: D-Bus message deserialization in `network.rs:67` and `sleep.rs:65` does not panic on malformed input. Failures are observable at debug log level.
- [ ] **SEC-05**: A PII-leak assertion covering URLs (not just payloads) extends `tests/wire_contract.rs` so debug-trace URL leakage is caught in CI.

### General Faults (FAULT)

- [ ] **FAULT-01**: `partial_cmp().unwrap()` in `weather_jma.rs:276` is replaced with explicit `unwrap_or(Ordering::Equal)` or `total_cmp()`. NaN inputs no longer panic.
- [ ] **FAULT-02**: `s.lat` and `s.lon` array-length checks in `weather_jma.rs:103-129` emit debug-level logs when stations are dropped, and lat/lon ranges are validated before use.
- [ ] **FAULT-03**: Test-helper `.unwrap()` calls in `pollen.rs:108,124,142,158,173` and `time.rs:141` are replaced with `.expect("…")` carrying fixture context.

## Deferred to Future Milestones

Originally drafted under the previous (pre-restart) tech-debt plan; pulled out of v0.9 to keep this milestone narrow. Surface again in a later milestone if needed.

### Dependency hygiene (DEPS)

- **DEPS-01**: `cargo-audit` runs in CI on every push, failing the build on RUSTSEC advisories above a configured severity threshold.
- **DEPS-02**: `cargo-deny` runs in CI with a license allowlist and advisory check. Config is committed at `deny.toml`.
- **DEPS-03**: MeteoAlarm codenames fetched from `raw.githubusercontent.com` are cached for the process lifetime with an in-crate fallback if the CDN is unreachable (`alerts.rs:234`).
- **DEPS-04**: Nominatim reverse-geocode results are cached keyed by coordinate so MeteoAlarm area filtering doesn't re-hit the API on every fetch (`alerts.rs:206`).

### Coverage gaps (TEST)

- **TEST-01**: `point_in_polygon` (`geo.rs`) has fixture tests covering polygon boundaries, holes, and self-intersecting paths.
- **TEST-02**: Region-detection boundary coordinates (e.g. lat 49.0 at US-Canada, EU edges) are covered by tests in `geo.rs`.
- **TEST-03**: Silent-default fallback paths in `air_quality.rs:192-211` and `alerts.rs` `unwrap_or` patterns have tests verifying the default produces sensible downstream behavior.
- **TEST-04**: BOM `warning_group_type` mapping in `alerts.rs:595-658` has tests for all known mapped variants plus an explicit unmapped-variant case.

### Public-surface hardening (API)

- **API-01**: Every `pub` item in `src/` is either documented in `API.md` or made non-public. CI gate prevents undocumented public items.
- **API-02**: `CONTRACT.md` is marked v1; every wire-crossing type listed in CONTRACT.md is covered by an `insta` snapshot in `tests/wire_contract.rs`.
- **API-03**: `API.md` documents the Linux-only D-Bus stream graceful-degradation contract.
- **API-04**: `CHANGELOG.md` has a "v0.8.0 → v1.0 migration" section.

### Performance (PERF)

- **PERF-01**: AMeDAS station haversine pre-sorted or bounding-box filtered before O(n) distance computation (`weather_jma.rs:53-57`).
- **PERF-02**: MeteoAlarm polygons parsed and cached once per process (`alerts.rs:519`).
- **PERF-03**: ECCC cascading HTML fetches parallelized or deduplicated within rate limits (`alerts.rs:395-488`).

### Resilience (RES)

- **RES-01**: Optional retry-with-backoff wrapper for transient API failures.
- **RES-02**: Connection pool tuning surfaced as a configurable knob.
- **RES-03**: HTTP caching with `If-Modified-Since` / `ETag` where providers support it.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Internationalization (i18n) | Crate scope per CLAUDE.md contract; consumers own localization |
| Windows/macOS first-class support | Linux-only crate; degraded streams are explicitly allowed |
| South Korea (KMA) alerts | National-ID-gated key — not viable for an open-source crate |
| Generic rate-limit-aware HTTP queue | Caller owns concurrency policy |
| Built-in HTTP response caching | Caller owns persistence and cache invalidation |
| Refactoring the global `reqwest::Client` RwLock | Profile first; not a documented bottleneck under realistic load |

## Traceability

Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| SEC-01 | Phase 1 | Pending |
| SEC-02 | Phase 1 | Pending |
| SEC-03 | Phase 1 | Pending |
| SEC-04 | Phase 1 | Pending |
| SEC-05 | Phase 1 | Pending |
| FAULT-01 | Phase 2 | Pending |
| FAULT-02 | Phase 2 | Pending |
| FAULT-03 | Phase 2 | Pending |

**Coverage:**
- v0.9 requirements: 8 total
- Mapped to phases: 8
- Unmapped: 0

---
*Requirements defined: 2026-06-30 — narrow restart of TechDebt milestone*
