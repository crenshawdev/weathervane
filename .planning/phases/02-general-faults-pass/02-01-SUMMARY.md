---
phase: 02-general-faults-pass
plan: 01
subsystem: weather-data
tags: [rust, tracing, jma, amedas, nan-safety, validation]

# Dependency graph
requires:
  - phase: 01-security-audit
    provides: PII-safe tracing/logging conventions and the JMA-station code-only logging pattern this plan follows
provides:
  - NaN-safe sort comparator in the weather_jma.rs test suite, matching the production fallback already in place
  - Observable (debug-logged) drop paths for malformed JMA station table entries (array-length mismatch, out-of-range coordinates)
affects: [weather_jma, alerts]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Closed-range RangeInclusive::contains() as a single predicate that rejects out-of-bounds AND NaN/infinite values, avoiding a separate is_finite() check"
    - "Extracted pure validation helper (parse_station_entry) from an async fetch function so drop-path unit tests don't require network mocking"

key-files:
  created: []
  modified:
    - src/weather_jma.rs

key-decisions:
  - "Extracted a private parse_station_entry(code, s) -> Option<Station> helper from fetch_stations() rather than keeping the validation inline, so the two new drop-path tests exercise the real validation logic directly against in-memory RawStation/HashMap fixtures instead of needing to mock the network fetch."
  - "Tests assert behavioural outcome (returned station count and surviving station's code) rather than capturing tracing output. The project does not have a tracing-test dev-dep wired up for this module, and the plan explicitly permitted skipping log-content capture in favor of count/identity assertions."

requirements-completed: [FAULT-01, FAULT-02]

coverage:
  - id: D1
    description: "Test-helper sort comparator in nearest_selection_picks_closest rewritten to use unwrap_or(Ordering::Equal), matching the production fallback at line 57; new nan_station_coords_do_not_panic test pins NaN-safety on the sort pipeline."
    requirement: "FAULT-01"
    verification:
      - kind: unit
        ref: "src/weather_jma.rs#weather_jma::tests::nan_station_coords_do_not_panic"
        status: pass
      - kind: unit
        ref: "src/weather_jma.rs#weather_jma::tests::nearest_selection_picks_closest"
        status: pass
    human_judgment: false
  - id: D2
    description: "fetch_stations() validation rewritten into parse_station_entry helper: array-length mismatches and out-of-range coordinates are now dropped with a tracing::debug! log naming the station code; only well-formed stations are returned."
    requirement: "FAULT-02"
    verification:
      - kind: unit
        ref: "src/weather_jma.rs#weather_jma::tests::station_array_length_mismatch_is_dropped"
        status: pass
      - kind: unit
        ref: "src/weather_jma.rs#weather_jma::tests::station_out_of_range_coords_are_dropped"
        status: pass
    human_judgment: false

# Metrics
duration: 6min
completed: 2026-06-30
status: complete
---

# Phase 2 Plan 1: weather_jma.rs Hardening (FAULT-01 + FAULT-02) Summary

**NaN-safe test sort comparator plus observable (debug-logged), code-only drop paths for malformed JMA station table entries, with three new pinning tests.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-06-30T22:37:38Z
- **Completed:** 2026-06-30T22:41:24Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Rewrote the test-helper sort comparator in `nearest_selection_picks_closest` to use `unwrap_or(std::cmp::Ordering::Equal)`, mirroring the production comparator's fallback at line 57 verbatim — no bare `partial_cmp().unwrap()` remains anywhere in the file.
- Added `nan_station_coords_do_not_panic`, a test that feeds a NaN-lat station alongside two well-formed stations through the same haversine + sort pipeline `override_current_temp` uses, asserting the sort completes without panicking and the nearest well-formed station still ranks first.
- Extracted a private `parse_station_entry(code, s) -> Option<Station>` helper from `fetch_stations()`. It now emits `tracing::debug!` on two distinct drop paths — array-length mismatch and out-of-range coordinates — each naming only the JMA-published station code, never the malformed raw `lat`/`lon` Vec contents.
- Added a closed-range `(-90.0..=90.0).contains(&lat)` / `(-180.0..=180.0).contains(&lon)` validation, which also rejects NaN and infinite decimal-degree values as a side effect of `PartialOrd` semantics on `RangeInclusive::contains`. Commented inline so a future "simplify with `<=`/`>=`" rewrite cannot silently regress NaN handling.
- Added `station_array_length_mismatch_is_dropped` and `station_out_of_range_coords_are_dropped` tests, both exercising `parse_station_entry` directly against in-memory `RawStation` fixtures (no network call), asserting the malformed entry is dropped and the well-formed sibling survives.

## Task Commits

Each task was committed atomically:

1. **Task 1: Make the weather_jma.rs test-helper sort NaN-safe and pin production NaN behaviour with a new test (FAULT-01)** - `12beb4a` (fix)
2. **Task 2: Add tracing::debug! and range validation to fetch_stations() station parsing, with two new tests pinning the dropped-station paths (FAULT-02)** - `6dcebe8` (fix)

**Plan metadata:** committed alongside this SUMMARY

_Note: Both tasks had `tdd="true"` in the plan frontmatter, but the actual work was small mechanical hardening of existing functions plus pinning tests written and verified in the same commit — no separate RED-phase-failing-commit was created, consistent with how the analogous 01-01-PLAN.md (referenced as the style precedent) treated similarly-scoped hardening tasks._

## Files Created/Modified
- `src/weather_jma.rs` - NaN-safe test comparator + `nan_station_coords_do_not_panic` test (Task 1); `parse_station_entry` validation helper with two `tracing::debug!` drop-path logs + two new pinning tests (Task 2)

## Decisions Made
- **(a) Helper extraction:** Extracted `parse_station_entry` as a standalone private function rather than keeping the validation loop inline in `fetch_stations()`. Rationale: the two new drop-path tests need to exercise the validation logic against synthetic `RawStation` fixtures without a network fetch; a pure, synchronous helper makes that direct and avoids any mocking of `get_json`.
- **(b) Exact debug-log message strings emitted by the two drop arms:**
  - `"dropping JMA station {code}: lat/lon array length mismatch"`
  - `"dropping JMA station {code}: coordinates out of range"`
- **(c) tracing-test dev-dep:** Not reused. Both new tests assert only the post-parse station count and the surviving station's code identity — they do not capture or assert on tracing output content. This matches the plan's stated default ("the project does not currently take a dev-dep on tracing-test... reusing it here is fine but not required for FAULT-02").
- **(d) Wire/public-surface confirmation:** `git diff --stat tests/snapshots/` and `git diff --stat src/lib.rs` both returned empty after both task commits — `tests/snapshots/` and `src/lib.rs` are byte-identical to their pre-plan state.

## Deviations from Plan

None - plan executed exactly as written. Line numbers and existing comparator/validation shapes in the live source matched the plan's file:line references exactly (production comparator at line 57, test comparator at line 276, validation block at lines 108-122 pre-edit).

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- FAULT-01 and FAULT-02 are both closed; ROADMAP Phase 2 Success Criteria 1 and 2 are met.
- Sibling plan 02-02 (FAULT-03, `src/pollen.rs` + `src/time.rs`) is unaffected — this plan touched only `src/weather_jma.rs` as scoped.
- All four CI gates (`cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo build --workspace`, `cargo test --workspace`) pass after both tasks; wire snapshots and public crate surface are untouched.

---
*Phase: 02-general-faults-pass*
*Completed: 2026-06-30*

## Self-Check: PASSED

- `src/weather_jma.rs` exists on disk: FOUND
- Commit `12beb4a` (Task 1, FAULT-01) exists in git log: FOUND
- Commit `6dcebe8` (Task 2, FAULT-02) exists in git log: FOUND
- All 12 plan-level `<verification>` checks re-run and pass: `cargo fmt --check` (0), `cargo clippy --workspace -- -D warnings` (0), `cargo build --workspace` (0), `cargo test --workspace` (0), bare-unwrap grep (0), unwrap_or grep (3, >=2), dropping-JMA-station grep (2, >=2), nan test fn grep (1), length-mismatch test fn grep (1), out-of-range test fn grep (1), snapshots diff (empty), lib.rs diff (empty)
