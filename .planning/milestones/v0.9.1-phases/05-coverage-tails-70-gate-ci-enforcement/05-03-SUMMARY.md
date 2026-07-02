---
phase: 05-coverage-tails-70-gate-ci-enforcement
plan: 03
subsystem: testing
tags: [rust, cargo-llvm-cov, unit-tests, time-formatting, day-night-detection]

# Dependency graph
requires: []
provides:
  - "src/time.rs line coverage 74.51% -> 99.41% (+24.90pp) via 11 new #[test] fns"
  - "Measured pre/post cargo llvm-cov datum for Plan 05-05's workspace >=75% gate decision"
affects: [05-05-verify-ci-gate]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Semantic day/night fallback proof via computed utc_offset_seconds (no wall-clock mocking): offset_hours = (target_hour - now_utc_hour).rem_euclid(24) shifts the local-frame hour into a known bucket regardless of when the test runs"

key-files:
  created: []
  modified:
    - src/time.rs

key-decisions:
  - "is_night_time fallback tests compute offsets at test-time rather than freezing the clock, per plan D-08/prohibitions — no new dependency, no mock-time crate"

patterns-established: []

requirements-completed: [COV-06]

coverage:
  - id: D1
    description: "format_time proven for RFC3339 (military + 12h with trim-leading-zero), naive fallback (military + 12h), and unparseable-returns-input branches"
    requirement: "COV-06"
    verification:
      - kind: unit
        ref: "src/time.rs#time::tests::format_time_*"
        status: pass
    human_judgment: false
  - id: D2
    description: "format_hour_minute AM/PM boundary arms proven: 0->12 AM (non-zero minute), 12->12 PM, 13/23->(hour-12) PM, plus military zero-padding"
    requirement: "COV-06"
    verification:
      - kind: unit
        ref: "src/time.rs#time::tests::format_hour_minute_*"
        status: pass
    human_judgment: false
  - id: D3
    description: "is_night_time unparseable-input fallback SEMANTICALLY proven (day-bucket false at target_hour=12, night-bucket true at target_hour=22) plus (Some, None)/(None, Some) partial-parse fall-through"
    requirement: "COV-06"
    verification:
      - kind: unit
        ref: "src/time.rs#time::tests::is_night_time_unparseable_fallback_*, time::tests::is_night_time_partial_parse_falls_through_to_fallback"
        status: pass
    human_judgment: false
  - id: D4
    description: "format_hour terminal unparseable-input fallback proven with three truly-unparseable fixtures"
    requirement: "COV-06"
    verification:
      - kind: unit
        ref: "src/time.rs#time::tests::format_hour_unparseable_returns_input"
        status: pass
    human_judgment: false

duration: 2min
completed: 2026-07-02
status: complete
---

# Phase 5 Plan 3: time.rs Coverage Tail Summary

**src/time.rs line coverage lifted 74.51% -> 99.41% (+24.90pp) via 11 new fixture-based unit tests covering format_time branches, format_hour_minute AM/PM boundaries, format_chrono_time's 12-hour trim-zero path, is_night_time's unparseable fallback, and format_hour's terminal fallback**

## Performance

- **Duration:** ~2 min (baseline at 20:24:59Z, post-plan measurement at 20:26:22Z)
- **Tasks:** 3
- **Files modified:** 1 (`src/time.rs`, +116 lines, all under `#[cfg(test)] mod tests`)

## Coverage baseline (pre-plan)

**Invocation:** `cargo llvm-cov --workspace --summary-only` (plain invocation worked without an sccache workaround, consistent with Phase 3/4/05-01/05-02 findings).

| Metric | Value |
|---|---|
| `src/time.rs` line coverage | 74.51% (102 lines, 26 missed) |
| Workspace `TOTAL` line coverage | 82.65% (3135 lines, 544 missed) |

Baseline for workspace is the post-05-02 endpoint (82.65%), not the phase-start figure (77.87%), since Plans 05-01 and 05-02 completed first and their deltas are already banked. Matches the expected `src/time.rs` number recorded in `05-CONTEXT.md` exactly.

## Coverage after Plan 05-03

**Invocation:** same as baseline: `cargo llvm-cov --workspace --summary-only`.

| Metric | Value | Delta |
|---|---|---|
| `src/time.rs` line coverage | 99.41% (170 lines, 1 missed) | **+24.90pp** |
| `src/time.rs` function coverage | 100.00% (26 functions, 0 missed) | +26.67pp |
| Workspace `TOTAL` line coverage | 83.80% (3203 lines, 519 missed) | **+1.15pp** |

Workspace already clears the 75% CI gate target set by D-04/COV-07 with 8.80pp of headroom.

No anomaly: delta is strongly positive on both metrics. The one remaining missed `src/time.rs` line is unattributed by `cargo llvm-cov`'s summary output (no per-line detail requested); given all four target fns (`format_time`, `format_hour`, `format_hour_minute`, `is_night_time`) plus `format_chrono_time` now show 100% function coverage, it is a residual partial-branch line (e.g. an unreachable arm combination) rather than an untested function.

## Targeted branches (Groups A-D from Task 2)

- **Group A** — `format_time` (was 0% covered per baseline; sibling `format_hour` was already tested): 5 tests. `format_time_rfc3339_military` proves the RFC3339-success -> military branch. `format_time_rfc3339_12h_trims_leading_zero` is the sole test hitting `format_chrono_time`'s `.trim_start_matches('0')` path at src/time.rs:116 (`"06:30 AM"` -> `"6:30 AM"`). `format_time_naive_military` and `format_time_naive_12h` prove the RFC3339-fails -> manual `T`/`:` split fallback in both time-format modes. `format_time_unparseable_returns_input` proves the terminal `time_str.to_string()` at line 72 for both a no-`T` string and a date-only string.
- **Group B** — `format_hour_minute` AM/PM boundary arms (0 -> 12 AM was already covered via `format_hour`, but 12 -> 12 PM and 13+ -> (hour-12) PM were not): 2 tests. `format_hour_minute_boundary_12_pm_and_pm_arm` reaches all four arms via `format_time` end-to-end (12:00 -> "12:00 PM", 13:15 -> "1:15 PM", 23:59 -> "11:59 PM", 00:15 -> "12:15 AM"). `format_hour_minute_military_passthrough_pads_zeros` proves the `{:02}:{:02}` zero-padding for single-digit hour/minute (05:07 -> "05:07").
- **Group C** — `is_night_time` unparseable-input fallback (the `_ => !(6..18).contains(&hour)` arm at src/time.rs:97-100 was not hit by any existing test): 3 tests. `is_night_time_unparseable_fallback_returns_false_in_day_bucket` and `is_night_time_unparseable_fallback_returns_true_in_night_bucket` compute `offset_hours = (target_hour - now_utc_hour).rem_euclid(24)` at test time (target_hour 12 for day, 22 for night) and pass unparseable sunrise/sunset strings, asserting the fallback boolean matches the target bucket. This is a SEMANTIC proof — an inverted fallback (`(6..18).contains(&hour)`) would fail both tests. `is_night_time_partial_parse_falls_through_to_fallback` proves the `(Some, None)` and `(None, Some)` match arms (one parseable timestamp, one garbage string) both fall into the same fallback branch as `(None, None)`.
- **Group D** — `format_hour` terminal unparseable fallback (line 52, not previously hit): 1 test with three fixtures. `"garbage-no-T-separator"` (no `T` -> `split('T').nth(1)` is `None`), `"2025-01-20Tabc"` (post-`T` segment `"abc"` fails `parse::<u32>()`), and `"2025-01-20T:00"` (post-`T` segment before `:` is empty, also fails `parse::<u32>()`). Deliberately does NOT use `"2025-01-20T99:00"` since `"99"` parses as a valid `u32` and would route to `format_hour_minute` instead of the terminal fallback.

## Task Commits

1. **Task 1: Baseline coverage measurement** - no commit (measurement only, no source change; numbers recorded above)
2. **Task 2: Add tests for format_time branches, format_hour_minute AM/PM boundaries, format_chrono_time 12h trim, is_night_time unparseable fallback, and format_hour unparseable fallback** - `606fce4` (test)
3. **Task 3: Post-plan coverage measurement** - no commit (measurement only; numbers recorded above)

**Plan metadata:** (this SUMMARY + STATE.md + ROADMAP.md commit follows separately)

## Files Created/Modified

- `src/time.rs` - Added 11 new `#[test]` fns to the existing `#[cfg(test)] mod tests` block (Groups A-D above); no changes to `ParsedDate`, `format_time`, `format_hour`, `format_chrono_time`, `format_hour_minute`, `is_night_time`, or any `#[serde(...)]` attribute.

## Decisions Made

- `is_night_time` fallback tests compute a deterministic `utc_offset_seconds` at test-time via `(target_hour - now_utc_hour).rem_euclid(24)` rather than freezing/mocking the wall clock, matching the plan's explicit prohibition against any freezetime/mock-time crate. The tests are robust to whatever hour the CI runner executes at.

## Deviations from Plan

None - plan executed exactly as written. All 11 new tests match the plan's Group A-D specification exactly (test names, assertions, exact fixture strings).

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `src/time.rs` coverage tail is closed (99.41%, +24.90pp from baseline; function coverage 100%).
- Workspace `TOTAL` moved 82.65% -> 83.80%, already 8.80pp above the 75% CI gate target — Plan 05-05 (COV-07 workspace verification + COV-08 CI gate) has strong measured headroom.
- Wire contract (`tests/wire_contract.rs`, `tests/snapshots/`) confirmed byte-identical throughout (`git status --short` empty at every verify step).
- No blockers for Plan 05-04 (weather_jma.rs) — Wave 1 remaining.

---
*Phase: 05-coverage-tails-70-gate-ci-enforcement*
*Completed: 2026-07-02*

## Self-Check: PASSED
