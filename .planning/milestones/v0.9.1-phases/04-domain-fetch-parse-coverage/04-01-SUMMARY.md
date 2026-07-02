---
phase: 04-domain-fetch-parse-coverage
plan: 01
subsystem: testing
tags: [rust, serde, coverage, llvm-cov, weather, open-meteo, jma]

# Dependency graph
requires:
  - phase: 03-alerts-parser-coverage
    provides: proven sync-helper-extraction + coverage-measurement idiom (nws_alerts_from_response/bom_alerts_from_response pattern, cargo llvm-cov invocation)
provides:
  - Two private sync helpers extracted from src/weather.rs (resolve_current_temp, weather_from_open_meteo)
  - Open-Meteo response parse test suite (6 tests: current fields, resolved-temperature passthrough, hourly full, hourly degraded/unequal-length, daily forecast, utc_offset_seconds default)
  - JMA override decision test suite (3 tests: japan+Some, japan+None, non-japan+Some)
  - Measured src/weather.rs + workspace TOTAL line-coverage baseline and post-plan delta
affects: [04-02, 04-03, 04-04, 05-workspace-coverage-gate]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Sync-helper extraction for testability (mirrors air_quality_aqicn.rs::extract_aqi and Phase 3's nws_alerts_from_response idiom) — move the filter_map transform body and the override decision out of async fetch_weather into two private sync fns callable from both production and #[test] fixtures"
    - "Helper-fn JSON fixtures via serde_json::json! (minimal_current_json/minimal_hourly_json/minimal_daily_json) reused across tests instead of hand-duplicating full literals for every case, while still inline in the test module (no fixtures/ directory)"

key-files:
  created: []
  modified:
    - src/weather.rs

key-decisions:
  - "cargo llvm-cov ran successfully with the plain invocation (no env -u RUSTC_WRAPPER workaround needed), consistent with Phase 3's finding"
  - "Preserved the original debug!() call site behavior exactly: it now fires from an `if let Some(t) = jma_override` block between the override await and the resolve_current_temp call, rather than inside a match arm — same message text, same argument order, same Some-only trigger condition"

patterns-established:
  - "Pattern: extract-for-testability sync helper + #[cfg(test)] mod tests with inline r#\"...\"# and serde_json::json!-built JSON fixtures, no insta, no live HTTP"

requirements-completed: [COV-02]

coverage:
  - id: D1
    description: "Extract the Open-Meteo response transform and JMA override decision from fetch_weather into two private sync helpers with zero behavior change"
    requirement: COV-02
    verification:
      - kind: unit
        ref: "cargo test --workspace (86 lib tests + 22 wire_contract tests, pre-existing suite green after extraction)"
        status: pass
      - kind: other
        ref: "cargo clippy --workspace -- -D warnings"
        status: pass
    human_judgment: false
  - id: D2
    description: "Open-Meteo response -> WeatherData parse proven: current fields (exact enum values), resolved-temperature passthrough seam, hourly full-length decode, hourly unequal-length filter drops mismatched rows, daily forecast decode, utc_offset_seconds #[serde(default)] fallback"
    requirement: COV-02
    verification:
      - kind: unit
        ref: "src/weather.rs#weather::tests::open_meteo_current_fields_decode"
        status: pass
      - kind: unit
        ref: "src/weather.rs#weather::tests::open_meteo_current_temperature_uses_resolved_value"
        status: pass
      - kind: unit
        ref: "src/weather.rs#weather::tests::open_meteo_hourly_forecast_decodes_24_rows"
        status: pass
      - kind: unit
        ref: "src/weather.rs#weather::tests::open_meteo_hourly_drops_rows_when_parallel_array_shorter"
        status: pass
      - kind: unit
        ref: "src/weather.rs#weather::tests::open_meteo_daily_forecast_decodes_multiple_days"
        status: pass
      - kind: unit
        ref: "src/weather.rs#weather::tests::open_meteo_default_utc_offset_when_missing"
        status: pass
    human_judgment: false
  - id: D3
    description: "JMA current-temp override decision proven across all three branches: japan+Some(override) wins, japan+None falls through to raw, non-japan+Some(override) still uses raw (is_japan_bounds gate)"
    requirement: COV-02
    verification:
      - kind: unit
        ref: "src/weather.rs#weather::tests::resolve_current_temp_japan_uses_override_when_some"
        status: pass
      - kind: unit
        ref: "src/weather.rs#weather::tests::resolve_current_temp_japan_uses_raw_when_none"
        status: pass
      - kind: unit
        ref: "src/weather.rs#weather::tests::resolve_current_temp_non_japan_always_uses_raw"
        status: pass
    human_judgment: false
  - id: D4
    description: "Measured src/weather.rs + workspace TOTAL line-coverage baseline (pre-extraction) and post-plan numbers recorded for Plan 04-04 / Phase 5's coverage gate"
    verification:
      - kind: other
        ref: "cargo llvm-cov --workspace --summary-only (baseline: weather.rs 0.00%/78 lines, TOTAL 66.51%; post-plan: weather.rs 89.08%/238 lines, TOTAL 70.63%)"
        status: pass
    human_judgment: false

duration: ~5min
completed: 2026-07-02
status: complete
---

# Phase 4 Plan 1: Open-Meteo Response Parse + JMA Override Decision Coverage Summary

**Extracted the Open-Meteo response builder and JMA current-temp override decision out of `fetch_weather` into two private sync helpers and proved both with 9 fixture-based tests, lifting `src/weather.rs` line coverage from 0.00% to 89.08%.**

## Performance

- **Duration:** ~5 min
- **Completed:** 2026-07-02
- **Tasks:** 3/3
- **Files modified:** 1 (`src/weather.rs`)

## Coverage baseline (pre-extraction)

Measured before any source change, via:

```
cargo llvm-cov --workspace --summary-only
```

(Plain invocation — no `env -u RUSTC_WRAPPER -u CARGO_BUILD_RUSTC_WRAPPER` workaround was needed in this environment, consistent with Phase 3's finding.)

| Metric | Value |
|---|---|
| `src/weather.rs` line coverage | 0.00% (0/78 lines) |
| Workspace `TOTAL` line coverage | 66.51% (1601/2407 lines) |

## Coverage after Plan 04-01

Re-measured after Task 3 (same invocation), via:

```
cargo llvm-cov --workspace --summary-only
```

| Metric | Value | Delta from baseline |
|---|---|---|
| `src/weather.rs` line coverage | 89.08% (212/238 lines) | +89.08 pp |
| Workspace `TOTAL` line coverage | 70.63% (1813/2567 lines) | +4.12 pp |

`src/weather.rs`'s total line count grew from 78 to 238 because the new `#[cfg(test)] mod tests` block (fixtures, helper fns, and assertions) is itself counted in the file's line total by `cargo llvm-cov` — expected, matching Phase 3's noted caveat on in-file test-module coverage math.

## Accomplishments

- Extracted `resolve_current_temp(latitude, longitude, raw_open_meteo_temp, jma_override) -> f32` and `weather_from_open_meteo(data, current_temperature) -> WeatherData` as private sync helpers with zero behavior change, mirroring the existing `air_quality_aqicn.rs::extract_aqi` / Phase 3 alerts.rs idiom.
- Proved the Open-Meteo response parse across 6 fixture tests: current-fields exact-value decode (including `WeatherCondition::ClearSky` and `CompassDirection::E`), resolved-temperature passthrough (proving the helper takes its temperature from the parameter, not the raw response — the seam the JMA override relies on), hourly full 24-row decode, hourly unequal-length-array row-dropping, multi-day daily forecast decode, and `utc_offset_seconds` `#[serde(default)]` fallback to 0.
- Proved the JMA override decision across all three branches: Japan coordinates with `Some(override)` return the override, Japan coordinates with `None` fall through to the raw Open-Meteo value, and non-Japan coordinates always use the raw value even when a (deliberately absurd) `Some` override is passed — confirming the `is_japan_bounds` gate blocks the override outside Japan.
- Measured and recorded both the pre-extraction baseline and post-plan `src/weather.rs` + workspace `TOTAL` line-coverage numbers.

## Task Commits

Each task was committed atomically:

1. **Task 1: Baseline coverage measurement + extract Open-Meteo response builder and JMA override decision into private sync helpers** - `ac691ba` (refactor)
2. **Task 2: Open-Meteo response -> WeatherData parse tests** - `5426b1f` (test)
3. **Task 3: JMA override decision tests + post-plan coverage measurement** - `c2ca4fb` (test)

_Baseline coverage measurement (Task 1 Step 0) was run and recorded before the first commit; it produced no code change so has no commit of its own._

## Files Created/Modified

- `src/weather.rs` - Extracted 2 private sync helpers (`resolve_current_temp`, `weather_from_open_meteo`); added `#[cfg(test)] mod tests` with 9 new `#[test]` fns (6 Open-Meteo parse + 3 JMA override decision), inline `r#"..."#` and `serde_json::json!`-built fixtures, and 3 fixture-helper fns (`minimal_current_json`, `minimal_hourly_json`, `minimal_daily_json`).

## Decisions Made

- Used the plain `cargo llvm-cov --workspace --summary-only` invocation for both coverage measurements, matching Phase 3's finding that the sccache workaround was unnecessary in this environment.
- Preserved the original `tracing::debug!("AMeDAS override: ...")` call site exactly (message text, argument order, Some-only trigger) by moving it into an `if let Some(t) = jma_override` block placed between the `override_current_temp` await and the `resolve_current_temp` call, rather than trying to fold it inside `resolve_current_temp` itself (which needed to stay a pure sync fn testable without side effects).
- Reused `serde_json::json!` to build helper JSON fixtures (`minimal_current_json`, `minimal_hourly_json`, `minimal_daily_json`) programmatically instead of hand-typing every full literal, keeping the 24-row hourly fixture and the unequal-length-array fixture concise while staying inline in the test module (no `tests/fixtures/` directory, satisfying the plan's prohibition).

## Deviations from Plan

None - plan executed exactly as written (all `must_haves`, `acceptance_criteria`, and `done` conditions across all 3 tasks met without needing Rule 1-4 auto-fixes).

## Issues Encountered

None. `cargo fmt` (via the harness's post-edit formatting hook) reformatted the Task 1 extraction slightly (multi-line `resolve_current_temp` call args); this was expected auto-formatting, not a deviation, and verified clean by the subsequent `cargo fmt --check`.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 04-02, 04-03, and 04-04 (Wave 1/2 siblings and dependents in Phase 4) are unaffected by this plan's scope (`src/weather.rs` only) and can proceed independently.
- Phase 5's workspace coverage gate now has a real trajectory to reason from: `src/weather.rs` baseline 0.00% -> post-plan 89.08% (+89.08pp), workspace `TOTAL` 66.51% -> 70.63% (+4.12pp) in one plan.
- Wire contract confirmed byte-identical throughout (`cargo test --test wire_contract` green, `git status --short tests/snapshots/` empty after every task).
- No blockers for subsequent Phase 4 plans.

---
*Phase: 04-domain-fetch-parse-coverage*
*Completed: 2026-07-02*

## Self-Check: PASSED

- FOUND: src/weather.rs
- FOUND: .planning/phases/04-domain-fetch-parse-coverage/04-01-SUMMARY.md
- FOUND: ac691ba (Task 1 commit)
- FOUND: 5426b1f (Task 2 commit)
- FOUND: c2ca4fb (Task 3 commit)
