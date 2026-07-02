---
phase: 05-coverage-tails-70-gate-ci-enforcement
plan: 04
subsystem: testing
tags: [rust, cargo-llvm-cov, unit-tests, sync-helper-extraction, jma-amedas]

# Dependency graph
requires: []
provides:
  - "src/weather_jma.rs line coverage 71.95% -> 87.70% (+15.75pp) via select_temp_from_map extraction + 9 new #[test] fns"
  - "Measured pre/post cargo llvm-cov datum for Plan 05-05's workspace >=75% gate decision"
affects: [05-05-verify-ci-gate]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Mechanical sync-helper extraction (select_temp_from_map) mirroring Phase 3/4's nws_alerts_from_response / resolve_current_temp idiom: pull the decision loop out of an async wrapper into a private sync fn testable against fixture HashMaps"

key-files:
  created: []
  modified:
    - src/weather_jma.rs

key-decisions:
  - "select_temp_from_map extraction is a byte-for-byte cut-and-relocate of override_current_temp's lines 72-83; the tracing::debug! fallthrough log moved intact into the helper"
  - "Deviation (Rule 3 - blocking issue): station_non_temp_capable_is_dropped_silently used assert_eq!(..., None) against Option<Station> as originally planned, but Station does not derive PartialEq and the plan explicitly prohibits editing Station. Switched to .is_none()/.is_some() assertions, matching the existing test style already used elsewhere in this file (e.g. iso_parser_rejects_malformed)."

patterns-established: []

requirements-completed: [COV-06]

coverage:
  - id: D1
    description: "select_temp_from_map extracted as a private sync fn from override_current_temp lines 72-83 with zero behavior change (pre-existing 8 weather_jma tests still pass after extraction)"
    requirement: "COV-06"
    verification:
      - kind: unit
        ref: "src/weather_jma.rs#weather_jma::tests (pre-existing 8 tests, full suite green)"
        status: pass
    human_judgment: false
  - id: D2
    description: "select_temp_from_map proven for first-valid-temp return, invalid-flag skip, missing-temp skip, wrong-length skip, no-valid-temp fallthrough, MAX_HOPS window limit, Fahrenheit conversion, and empty-candidates edge case"
    requirement: "COV-06"
    verification:
      - kind: unit
        ref: "src/weather_jma.rs#weather_jma::tests::select_temp_from_map_*"
        status: pass
    human_judgment: false
  - id: D3
    description: "parse_station_entry elems[0] != '1' (non-temperature-capable AMeDAS station) drop arm proven with a control-vs-non-temp pair"
    requirement: "COV-06"
    verification:
      - kind: unit
        ref: "src/weather_jma.rs#weather_jma::tests::station_non_temp_capable_is_dropped_silently"
        status: pass
    human_judgment: false

duration: 3min
completed: 2026-07-02
status: complete
---

# Phase 5 Plan 4: weather_jma.rs Coverage Tail Summary

**src/weather_jma.rs line coverage lifted 71.95% -> 87.70% (+15.75pp) by extracting select_temp_from_map as a private sync fn from override_current_temp and covering its 8 decision arms plus the parse_station_entry non-temp-station drop arm**

## Performance

- **Duration:** 3 min (baseline at 16:28:37Z, post-plan measurement at 16:31:xxZ)
- **Started:** 2026-07-02T16:28:37Z
- **Tasks:** 3
- **Files modified:** 1 (`src/weather_jma.rs`)

## Coverage baseline (pre-plan)

**Invocation:** `cargo llvm-cov --workspace --summary-only` (plain invocation worked without an sccache workaround, consistent with Phase 3/4/05-01/05-02/05-03 findings).

| Metric | Value |
|---|---|
| `src/weather_jma.rs` line coverage | 71.95% (246 lines, 69 missed) |
| Workspace `TOTAL` line coverage | 83.80% (3203 lines, 519 missed) |

This is the post-05-03 endpoint (83.80%), not the phase-start figure (77.87%), since Plans 05-01, 05-02, and 05-03 completed first and their deltas are already banked. Matches the `src/weather_jma.rs` number recorded in `05-CONTEXT.md` exactly (71.95%).

## Coverage after Plan 05-04

**Invocation:** same as baseline: `cargo llvm-cov --workspace --summary-only`.

| Metric | Value | Delta |
|---|---|---|
| `src/weather_jma.rs` line coverage | 87.70% (488 lines, 60 missed) | **+15.75pp** |
| Workspace `TOTAL` line coverage | 85.20% (3445 lines, 510 missed) | **+1.40pp** |

Workspace already clears the 75% CI gate target set by D-04/COV-07 with 10.20pp of headroom.

No anomaly: delta is strongly positive on both metrics.

## Extracted-helper decision

- **Name:** `select_temp_from_map`
- **Signature:** `fn select_temp_from_map(candidates: &[(f64, Station)], map: &HashMap<String, RawObservation>, unit: TemperatureUnit) -> Option<f32>`
- **Behavior-preservation rationale:** mechanical cut-and-relocate of `override_current_temp`'s lines 72-83 (the `for (_, station) in candidates.iter().take(MAX_HOPS) { ... }` loop plus the final `tracing::debug!` fallthrough and `None`). The MAX_HOPS constant, the `temp.len() == 2 && temp[1] == 0.0` validity flag, and the `to_unit(temp[0] as f32, unit)` conversion are byte-identical to the original. `override_current_temp` now ends with `select_temp_from_map(&candidates, &map, unit)` in place of the inline loop. The pre-existing 8 `weather_jma` tests all still pass after the extraction, and the full 185-test suite stayed green throughout.

## Targeted branches (Groups A-B from Task 2)

- **Group A** — `select_temp_from_map` decision arms (8 tests): `select_temp_from_map_returns_first_valid_temp` proves the happy-path Some(temp) return on the first candidate. `select_temp_from_map_skips_invalid_flag_and_tries_next` proves the `temp[1] != 0.0` skip continues to the next candidate. `select_temp_from_map_skips_missing_temp_and_tries_next` proves the `obs.temp == None` guard skip. `select_temp_from_map_skips_temp_wrong_length_and_tries_next` proves the `temp.len() != 2` guard skip. `select_temp_from_map_returns_none_when_no_valid_temp` proves the terminal fallthrough `None` return. `select_temp_from_map_respects_max_hops` proves the `.take(MAX_HOPS)` (MAX_HOPS = 3) window using four candidates `[s0, s1, s2, s3]` where only s3 (index 3, beyond the window) has a valid temp — asserts `None`, catching a hypothetical off-by-one `.take(MAX_HOPS + 1)` regression. `select_temp_from_map_converts_to_fahrenheit` proves the `to_unit` conversion (0.0 Celsius -> 32.0 Fahrenheit within 0.001 tolerance). `select_temp_from_map_returns_none_on_empty_candidates` proves the defensive empty-slice edge case.
- **Group B** — `parse_station_entry` non-temp-station drop (1 test): `station_non_temp_capable_is_dropped_silently` asserts `parse_station_entry` returns `None` for `elems="00000000"` (first char != '1') and `Some(station)` for a control case with `elems="10000000"` and identical valid lat/lon, proving the arm is specifically the elems filter and not any other drop cause. No debug! log assertion, per the doc comment at line 128-129 ("dropped silently").

## Accepted-gap note (async surface)

The remaining uncovered surface of `src/weather_jma.rs` — `override_current_temp`'s sort + nearest-station-distance-check portion, `cached_stations`, `fetch_stations`, `latest_observation_time`, and `fetch_map` — is inherently coupled to live JMA HTTP endpoints via `crate::client::{get_json, get_text}`. Per D-06/D-07 (tests target parse/logic helpers, not live HTTP), this async surface remains an accepted, documented coverage gap. Any future coverage of this surface would require a zbus/HTTP mock harness, which is out of scope for the v0.9.1 coverage milestone.

## Task Commits

1. **Task 1: Baseline coverage measurement + extract select_temp_from_map** - `6e91d47` (refactor)
2. **Task 2: Add tests for select_temp_from_map decision arms and parse_station_entry non-temp-station drop** - `3025e3f` (test)
3. **Task 3: Post-plan coverage measurement** - no commit (measurement only; numbers recorded above)

**Plan metadata:** (this SUMMARY + STATE.md + ROADMAP.md commit follows separately)

## Files Created/Modified

- `src/weather_jma.rs` - Extracted `select_temp_from_map` as a private sync fn (Task 1, `6e91d47`); added 9 new `#[test]` fns to the existing `#[cfg(test)] mod tests` block covering the 8 decision arms + the non-temp-station drop arm (Task 2, `3025e3f`). No changes to `Station`, `RawStation`, `RawObservation`, or any `#[serde(...)]` attribute.

## Decisions Made

- The extraction places `select_temp_from_map` directly after `override_current_temp` (before `cached_stations`), matching the plan's specified placement.
- `select_temp_from_map`'s doc comment mirrors the `extract_aqi` "pulled out for testability" precedent in `air_quality_aqicn.rs`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking issue] `station_non_temp_capable_is_dropped_silently` could not use `assert_eq!(..., None)` against `Option<Station>`**
- **Found during:** Task 2, first `cargo test` compile attempt
- **Issue:** The plan's Task 2 action text says "Assert result == None" for the non-temp-station case, implying `assert_eq!`. `Station` does not derive `PartialEq` (confirmed by compiler error E0369), and the plan's prohibitions explicitly forbid editing `Station`, `RawStation`, or `RawObservation`.
- **Fix:** Used `.is_none()` / `.is_some()` assertions instead of `assert_eq!`, which is the exact style already used by the pre-existing `iso_parser_rejects_malformed` test in the same file (`parse_iso_to_compact(...).is_none()`). No behavioral difference in what's being proven — the non-temp station is still asserted to be dropped, and the control case is still asserted to succeed with the expected code.
- **Files modified:** `src/weather_jma.rs`
- **Commit:** `3025e3f`

## Issues Encountered

None beyond the deviation above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `src/weather_jma.rs` coverage tail is closed for the deterministically-testable portion (87.70%, +15.75pp from baseline). The remaining 60 missed lines are the accepted async-HTTP gap documented above.
- Workspace `TOTAL` moved 83.80% -> 85.20%, 10.20pp above the 75% CI gate target — Plan 05-05 (COV-07 workspace verification + COV-08 CI gate) has strong measured headroom from all four Wave 1 plans (05-01 through 05-04).
- Wire contract (`tests/wire_contract.rs`, `tests/snapshots/`) confirmed byte-identical throughout (`git status --short` empty at every verify step).
- No blockers for Plan 05-05 (CI gate, Wave 2).

---
*Phase: 05-coverage-tails-70-gate-ci-enforcement*
*Completed: 2026-07-02*

## Self-Check: PASSED

- FOUND: src/weather_jma.rs
- FOUND commit: 6e91d47
- FOUND commit: 3025e3f
