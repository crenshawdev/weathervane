---
phase: 04-domain-fetch-parse-coverage
plan: 03
subsystem: testing
tags: [rust, serde, coverage, llvm-cov, location, geocoding]

# Dependency graph
requires:
  - phase: 04-domain-fetch-parse-coverage
    provides: "Plan 04-01 (weather.rs) and 04-02 (air_quality.rs) established the sync-helper-extraction + fixture-test pattern for this milestone"
provides:
  - "One private sync helper extracted from src/location.rs (detected_from_ip_api)"
  - "LocationResult::from_geocoding_result parser test suite (4 tests)"
  - "detected_from_ip_api branch test suite (7 tests)"
  - "SavedLocation::matches_coords boundary test suite (3 tests)"
  - "uses_imperial_units test suite (3 tests)"
  - "Measured src/location.rs + workspace TOTAL line-coverage baseline and post-plan delta"
affects: [05-coverage-gate-and-wrapup]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Sync-helper extraction for testability (mirrors air_quality.rs::resolve_headline_aqi, alerts.rs parser extractions) — move the IP-API success-branch body out of async detect_location into a private sync fn callable from both production and #[test] fixtures"

key-files:
  created: []
  modified:
    - src/location.rs

key-decisions:
  - "Plain cargo llvm-cov --workspace --summary-only invocation worked without the env -u RUSTC_WRAPPER sccache workaround, consistent with Plans 04-01 and 04-02 in this same environment"
  - "Coverage measured before and after the plan: location.rs 27.96% -> 88.61% (+60.65pp), workspace TOTAL 73.83% -> 76.55% (+2.72pp) — a large single-file delta since location.rs was near-zero covered on its behavior branches beforehand"

patterns-established: []

requirements-completed: [COV-04]

coverage:
  - id: D1
    description: "Extract the IP-API success-branch body from detect_location into a private sync helper (detected_from_ip_api) with zero behavior change"
    verification:
      - kind: unit
        ref: "cargo test --workspace (133 lib tests + 22 wire_contract tests, pre-existing suite green after extraction)"
        status: pass
      - kind: other
        ref: "cargo clippy --workspace -- -D warnings"
        status: pass
    human_judgment: false
  - id: D2
    description: "LocationResult::from_geocoding_result proven for all three display_name arms (three-part, two-part, name-only) plus country pass-through"
    requirement: COV-04
    verification:
      - kind: unit
        ref: "src/location.rs#location::tests::location_result_from_geocoding_with_admin_and_country"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::location_result_from_geocoding_without_admin_uses_two_part_display"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::location_result_from_geocoding_without_country_uses_name_only"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::location_result_from_geocoding_without_admin_or_country_uses_name_only"
        status: pass
    human_judgment: false
  - id: D3
    description: "detected_from_ip_api proven for all four display_name arms (city+country, region+country, country-only, Unknown fallback) plus three error paths (status fail, lat missing, lat out-of-range)"
    requirement: COV-04
    verification:
      - kind: unit
        ref: "src/location.rs#location::tests::detected_from_ip_api_success_with_city_and_country"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::detected_from_ip_api_success_without_city_uses_region_and_country"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::detected_from_ip_api_success_with_only_country_uses_country_alone"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::detected_from_ip_api_success_with_no_names_falls_back_to_unknown"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::detected_from_ip_api_returns_location_detection_error_when_status_fail"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::detected_from_ip_api_returns_location_detection_error_when_lat_missing"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::detected_from_ip_api_returns_location_detection_error_when_lat_out_of_range"
        status: pass
    human_judgment: false
  - id: D4
    description: "SavedLocation::matches_coords proven at the +/-0.01 window boundary in both lat and lon"
    requirement: COV-04
    verification:
      - kind: unit
        ref: "src/location.rs#location::tests::saved_location_matches_within_window"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::saved_location_does_not_match_outside_window_lat"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::saved_location_does_not_match_outside_window_lon"
        status: pass
    human_judgment: false
  - id: D5
    description: "uses_imperial_units proven for all three imperial countries plus non-imperial (including non-substring-match on United Kingdom) and empty-string cases"
    requirement: COV-04
    verification:
      - kind: unit
        ref: "src/location.rs#location::tests::uses_imperial_units_returns_true_for_imperial_countries"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::uses_imperial_units_returns_false_for_metric_countries"
        status: pass
      - kind: unit
        ref: "src/location.rs#location::tests::uses_imperial_units_returns_false_for_empty_string"
        status: pass
    human_judgment: false
  - id: D6
    description: "Measured src/location.rs + workspace TOTAL line-coverage baseline (pre-extraction) and post-plan numbers recorded for Phase 5's COV-07 gate"
    verification:
      - kind: other
        ref: "cargo llvm-cov --workspace --summary-only (baseline: location.rs 27.96%/93 lines, TOTAL 73.83%; post-plan: location.rs 88.61%/237 lines, TOTAL 76.55%)"
        status: pass
    human_judgment: false

duration: ~10min
completed: 2026-07-02
status: complete
---

# Phase 4 Plan 3: Location Geocoding/IP-Detection Coverage Summary

**Extracted the IP-API success branch from detect_location into a testable private sync helper and proved all four named location surfaces (geocoding parse, IP-geolocation parse, saved-location coord matching, imperial-units lookup) with 17 fixture-based tests, lifting `src/location.rs` line coverage from 27.96% to 88.61%.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-07-02T02:04:06Z
- **Tasks:** 3/3
- **Files modified:** 1 (`src/location.rs`)

## Coverage Baseline (pre-extraction)

Measured before any source change, via:

```
cargo llvm-cov --workspace --summary-only
```

(Plain invocation — no `env -u RUSTC_WRAPPER -u CARGO_BUILD_RUSTC_WRAPPER` workaround was needed, consistent with Plans 04-01 and 04-02 in this environment.)

| Metric | Value |
|---|---|
| `src/location.rs` line coverage | 27.96% (26/93 lines) |
| Workspace `TOTAL` line coverage | 73.83% (2003/2713 lines) |

## Coverage after Plan 04-03

Re-measured after Task 2 (same invocation), via:

```
cargo llvm-cov --workspace --summary-only
```

| Metric | Value | Delta from baseline |
|---|---|---|
| `src/location.rs` line coverage | 88.61% (210/237 lines) | +60.65 pp |
| Workspace `TOTAL` line coverage | 76.55% (2187/2857 lines) | +2.72 pp |

`src/location.rs`'s total line count grew from 93 to 237 because the extended `#[cfg(test)] mod tests` block (fixtures + assertions) is itself counted in the file's line total by `cargo llvm-cov` — expected, and consistent with the same caveat noted in Plan 03-01's SUMMARY. The delta is positive as required by Task 3's acceptance criteria; no anomaly to arbitrate.

## Accomplishments

- Extracted `detected_from_ip_api(data: IpApiResponse) -> Result<DetectedLocation>` as a private sync helper with zero behavior change: same debug logs, same coord-range guard (IEEE-754-safe range-contains form preserved verbatim), same display_name arm selection, same `Error::LocationDetection` error paths. `detect_location`'s trailing unreachable `Err(Error::LocationDetection)` line was removed since all error paths now live inside the helper.
- Proved `LocationResult::from_geocoding_result` across all three display_name arms (name+admin+country three-part, name+country two-part, name-only fallback) plus the `.unwrap_or_default()` empty-string country pass-through when country is absent.
- Proved `detected_from_ip_api` across all four display_name arms (city+country, region+country, country-only, Unknown fallback) plus three error paths: status != "success", lat/lon missing, and lat out-of-range (91.0), the last of which proves the coord-range guard fires correctly through the extracted call site.
- Proved `SavedLocation::matches_coords` at the ±0.01 window boundary: within-window (exact match and a small delta under 0.01 on both axes), outside-window on lat alone, and outside-window on lon alone.
- Proved `uses_imperial_units` for all three officially-imperial countries (United States, Liberia, Myanmar), non-imperial countries including "United Kingdom" (confirming the exact `matches!` form does not substring-match "United"), and the empty string.
- Measured and recorded both the pre-extraction baseline and post-plan `src/location.rs` + workspace `TOTAL` line-coverage numbers, giving Phase 5's COV-07 gate a real measured trajectory.

## Task Commits

Each task was committed atomically:

1. **Task 1: Baseline coverage measurement + extract detected_from_ip_api from detect_location** - `46be5ff` (refactor)
2. **Task 2: LocationResult geocoding parse tests + detected_from_ip_api branch tests + SavedLocation::matches_coords tests + uses_imperial_units tests** - `b208466` (test)
3. **Task 3: Post-plan coverage measurement** - no code change (measurement-only task); captured in this SUMMARY and the plan-metadata commit.

_Baseline coverage measurement (Task 1 Step 0) was run and recorded before the first commit; it produced no code change so has no commit of its own._

## Files Created/Modified

- `src/location.rs` - Extracted `detected_from_ip_api` private sync helper from `detect_location`; extended the existing `#[cfg(test)] mod tests` block (line 168, now with `use super::*;`) with 17 new `#[test]` fns (4 LocationResult geocoding + 7 detected_from_ip_api branches + 3 SavedLocation boundary + 3 uses_imperial_units), for 20 tests total in the module. The 3 pre-existing D-05/D-06 NaN/Inf/OOR guard tests are unchanged.

## Decisions Made

- Used the plain `cargo llvm-cov --workspace --summary-only` invocation for both coverage measurements; no sccache workaround needed, matching Plans 04-01/04-02.
- Kept the `if data.status == "success" { if let (Some(lat), Some(lon)) = ... { ... } } Err(Error::LocationDetection)` control-flow shape inside the extracted helper exactly as it existed in the original `detect_location`, rather than rewriting to a flatter early-return form — this minimizes the diff and keeps behavior provably identical (Task 1's zero-behavior-change constraint).

## Deviations from Plan

None - plan executed exactly as written (all `must_haves`, `acceptance_criteria`, and `done` conditions across all 3 tasks met without needing Rule 1-4 auto-fixes).

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `src/location.rs` is now well-covered on its behavior branches (88.61% line coverage, up from 27.96%), joining `alerts.rs`, `weather.rs`, and `air_quality.rs` from earlier phase plans.
- Workspace `TOTAL` line coverage now sits at 76.55%, already above the Phase 5 COV-07 ≥70% hard gate — Plan 04-04 (the remaining Phase 4 plan) can proceed without pressure to hit a specific number, though it should still measure its own delta per the established pattern.
- Wire contract confirmed byte-identical throughout (`cargo test --test wire_contract` green, `git status --short tests/snapshots/` empty after every task).
- No blockers for Plan 04-04 or Phase 5.

---
*Phase: 04-domain-fetch-parse-coverage*
*Completed: 2026-07-02*

## Self-Check: PASSED

- FOUND: src/location.rs
- FOUND: .planning/phases/04-domain-fetch-parse-coverage/04-03-SUMMARY.md
- FOUND: 46be5ff (Task 1 commit)
- FOUND: b208466 (Task 2 commit)
