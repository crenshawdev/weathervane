---
phase: 04-domain-fetch-parse-coverage
plan: 02
subsystem: testing
tags: [rust, serde, coverage, llvm-cov, air-quality]

# Dependency graph
requires:
  - phase: 03-alerts-parser-coverage
    provides: proven sync-helper-extraction + fixture-test method (mirrored here)
provides:
  - One private sync helper extracted from src/air_quality.rs (resolve_headline_aqi)
  - resolve_headline_aqi branch test suite (6 tests covering all US/EU/AQICN/missing-scale paths)
  - UsAqiCategory::from_aqi + EuAqiCategory::from_aqi boundary test suites (12 tests)
  - AirQualityData::standard() test suite (2 tests)
  - Pollutant Option-default test (1 test)
  - Measured src/air_quality.rs + workspace TOTAL line-coverage baseline and post-plan delta
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Sync-helper extraction for testability (mirrors air_quality_aqicn.rs::extract_aqi and Phase 3's alerts.rs pattern) — move the headline-AQI selection ternary out of async fetch_air_quality into a private sync fn callable from both production and #[test] fixtures"

key-files:
  created: []
  modified:
    - src/air_quality.rs

key-decisions:
  - "cargo llvm-cov ran successfully with the plain invocation (no env -u RUSTC_WRAPPER workaround needed), consistent with Phase 3 Plan 01's finding — Codex's sccache-blocked concern did not reproduce here either"
  - "Coverage measured before and after the plan: air_quality.rs 10.26% -> 88.39% (line coverage), workspace TOTAL 70.63% -> 73.83%, giving Phase 5's COV-07 gate a measured trajectory instead of an assumption"

patterns-established:
  - "Pattern confirmed a second time: extract-for-testability sync helper + inline r#\"...\"# JSON fixtures fed through serde_json::from_str, no insta, no live HTTP, no #[tokio::test]"

requirements-completed: [COV-03]

coverage:
  - id: D1
    description: "Extract the aqicn/EU/US AQI-selection if/else-if/else chain into a private sync helper with zero behavior change"
    verification:
      - kind: unit
        ref: "cargo test --workspace (95 lib tests pre-extraction baseline, all pass after extraction before new tests added)"
        status: pass
      - kind: other
        ref: "cargo clippy --workspace -- -D warnings"
        status: pass
    human_judgment: false
  - id: D2
    description: "resolve_headline_aqi proven across all 6 truth-table branches: US+aqicn, US+none+val, US+none+null, EU+none+val, EU+none+null, Unknown falls through as US"
    requirement: COV-03
    verification:
      - kind: unit
        ref: "src/air_quality.rs#tests::resolve_headline_aqi_us_region_with_aqicn_returns_aqicn"
        status: pass
      - kind: unit
        ref: "src/air_quality.rs#tests::resolve_headline_aqi_us_region_without_aqicn_returns_open_meteo_us"
        status: pass
      - kind: unit
        ref: "src/air_quality.rs#tests::resolve_headline_aqi_us_region_with_missing_us_aqi_defaults_to_zero"
        status: pass
      - kind: unit
        ref: "src/air_quality.rs#tests::resolve_headline_aqi_europe_region_without_aqicn_returns_open_meteo_eu"
        status: pass
      - kind: unit
        ref: "src/air_quality.rs#tests::resolve_headline_aqi_europe_region_with_missing_european_aqi_defaults_to_zero"
        status: pass
      - kind: unit
        ref: "src/air_quality.rs#tests::resolve_headline_aqi_unknown_region_behaves_as_us"
        status: pass
    human_judgment: false
  - id: D3
    description: "UsAqiCategory::from_aqi and EuAqiCategory::from_aqi proven at all 12 range boundaries"
    requirement: COV-03
    verification:
      - kind: unit
        ref: "src/air_quality.rs#tests::us_aqi_category_* (6 tests: good/moderate/unhealthy_sensitive/unhealthy/very_unhealthy/hazardous)"
        status: pass
      - kind: unit
        ref: "src/air_quality.rs#tests::eu_aqi_category_* (6 tests: good/fair/moderate/poor/very_poor/extremely_poor)"
        status: pass
    human_judgment: false
  - id: D4
    description: "AirQualityData::standard() proven for both AqiCategory::Us(_) and AqiCategory::Eu(_) variants"
    requirement: COV-03
    verification:
      - kind: unit
        ref: "src/air_quality.rs#tests::air_quality_data_standard_returns_us_for_us_variant"
        status: pass
      - kind: unit
        ref: "src/air_quality.rs#tests::air_quality_data_standard_returns_european_for_eu_variant"
        status: pass
    human_judgment: false
  - id: D5
    description: "Pollutant Option-defaults (pm2_5/pm10/ozone/nitrogen_dioxide/carbon_monoxide) proven to fall back to 0.0 when the API returns null"
    requirement: COV-03
    verification:
      - kind: unit
        ref: "src/air_quality.rs#tests::air_quality_response_missing_pollutants_default_to_zero_after_fetch"
        status: pass
    human_judgment: false
  - id: D6
    description: "Measured src/air_quality.rs + workspace TOTAL line-coverage baseline (pre-extraction) and post-plan numbers recorded for Phase 5's COV-07 gate"
    verification:
      - kind: other
        ref: "cargo llvm-cov --workspace --summary-only (baseline: air_quality.rs 10.26%/78 lines, TOTAL 70.63%; post-plan: air_quality.rs 88.39%/224 lines, TOTAL 73.83%)"
        status: pass
    human_judgment: false

duration: ~2min
completed: 2026-07-02
status: complete
---

# Phase 4 Plan 2: Air Quality AQI Resolution Extraction and Test Coverage Summary

**Extracted the headline-AQI selection ternary from `fetch_air_quality` into `resolve_headline_aqi` and proved it plus the US/EU category boundaries, `AirQualityData::standard()`, and pollutant Option-defaults with 21 fixture-based tests, lifting `src/air_quality.rs` line coverage from 10.26% to 88.39%.**

## Performance

- **Duration:** ~2 min
- **Completed:** 2026-07-02
- **Tasks:** 3/3
- **Files modified:** 1 (`src/air_quality.rs`)

## Coverage baseline (pre-extraction)

Measured before any source change, via:

```
cargo llvm-cov --workspace --summary-only
```

(Plain invocation — no `env -u RUSTC_WRAPPER -u CARGO_BUILD_RUSTC_WRAPPER` workaround was needed in this environment, consistent with Phase 3 Plan 01's finding.)

| Metric | Value |
|---|---|
| `src/air_quality.rs` line coverage | 10.26% (8/78 lines) |
| Workspace `TOTAL` line coverage | 70.63% (1813/2567 lines) |

## Coverage after Plan 04-02

Re-measured after Task 3 (same invocation), via:

```
cargo llvm-cov --workspace --summary-only
```

| Metric | Value | Delta from baseline |
|---|---|---|
| `src/air_quality.rs` line coverage | 88.39% (198/224 lines) | +78.13 pp |
| Workspace `TOTAL` line coverage | 73.83% (2003/2713 lines) | +3.20 pp |

`src/air_quality.rs`'s total line count grew from 78 to 224 because the new `#[cfg(test)] mod tests` block (fixtures + assertions) is itself counted in the file's line total by `cargo llvm-cov` — same caveat noted in Phase 3's 03-01-SUMMARY.md. This is telemetry for Phase 5's COV-07 gate, not a gate of its own.

## Accomplishments

- Extracted `resolve_headline_aqi(&AirQualityResponse, Region, Option<i32>) -> (i32, AqiCategory, AqiSource)` as a private sync helper with zero behavior change, mirroring the existing `air_quality_aqicn.rs::extract_aqi` and Phase 3's `alerts.rs` "pulled out for testability" idiom. Both `tracing::warn!` messages ("European AQI missing..." / "US AQI missing...") preserved verbatim inside the helper's `unwrap_or_else` closures.
- Proved `resolve_headline_aqi` across all 6 truth-table branches: US+aqicn returns Aqicn source, US+none+val and US+none+null both resolve on the Open-Meteo US scale (with the null case exercising the zero-default fallback), EU+none+val and EU+none+null both resolve on the Open-Meteo EU scale, and `Region::Unknown` falls through the else arm to behave identically to `Region::Us`.
- Proved `UsAqiCategory::from_aqi` at all 6 US boundaries (0/50, 51/100, 101/150, 151/200, 201/300, 301/999) and `EuAqiCategory::from_aqi` at all 6 EU boundaries (0/20, 21/40, 41/60, 61/80, 81/100, 101/999).
- Proved `AirQualityData::standard()` returns `AqiStandard::Us` for a `Us(_)` category and `AqiStandard::European` for an `Eu(_)` category.
- Proved the pollutant Option-default fallback (pm2_5/pm10/ozone/nitrogen_dioxide/carbon_monoxide all default to 0.0 when the API returns null), built the same way `fetch_air_quality`'s `Ok(AirQualityData { ... })` builder does.
- Measured and recorded both the pre-extraction baseline and post-plan `src/air_quality.rs` + workspace `TOTAL` line-coverage numbers.

## Task Commits

Each task was committed atomically:

1. **Task 1: Baseline coverage measurement + extract resolve_headline_aqi** - `37f7eaf` (refactor)
2. **Task 2: resolve_headline_aqi branch tests + category boundary tests + standard() tests + pollutant-defaults test** - `e399df6` (test)
3. **Task 3: Post-plan coverage measurement** - no source change, measurement recorded here; folded into this SUMMARY's final docs commit.

_Baseline coverage measurement (Task 1 Step 0) was run and recorded before the first commit; it produced no code change so has no commit of its own, consistent with Phase 3 Plan 01's precedent._

## Files Created/Modified

- `src/air_quality.rs` - Extracted `resolve_headline_aqi` private sync helper; added 21 new `#[test]` fns (6 resolve_headline_aqi branches + 6 UsAqiCategory boundaries + 6 EuAqiCategory boundaries + 2 AirQualityData::standard() + 1 pollutant-defaults) to the existing `#[cfg(test)] mod tests` block, alongside the pre-existing `aqi_source_defaults_to_open_meteo`.

## Decisions Made

- Used the plain `cargo llvm-cov --workspace --summary-only` invocation for both coverage measurements; no sccache workaround needed.
- Reported the `Lines` column (not `Regions`) for "line coverage" per the plan's literal wording, matching Phase 3 Plan 01's reporting convention.

## Deviations from Plan

None - plan executed exactly as written (all `must_haves`, `acceptance_criteria`, and `done` conditions across all 3 tasks met without needing Rule 1-4 auto-fixes).

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `src/air_quality.rs` now sits at 88.39% line coverage, up from 10.26% (+78.13pp), giving Phase 5's COV-07 workspace ≥70% gate a strong contribution from this module.
- Wire contract confirmed byte-identical throughout (`cargo test --test wire_contract` green, `git status --short tests/snapshots/` empty after every task) — the adjacent-tagged `AqiCategory` shape (`#[serde(tag = "standard", content = "level")]`) is untouched.
- No blockers for Plan 03 or Plan 04.

---
*Phase: 04-domain-fetch-parse-coverage*
*Completed: 2026-07-02*

## Self-Check: PASSED

- FOUND: src/air_quality.rs
- FOUND: .planning/phases/04-domain-fetch-parse-coverage/04-02-SUMMARY.md
- FOUND: 37f7eaf (Task 1 commit)
- FOUND: e399df6 (Task 2 commit)
