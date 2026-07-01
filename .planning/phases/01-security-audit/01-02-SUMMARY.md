---
phase: 01-security-audit
plan: 02
subsystem: location
tags: [security, input-validation, coordinate-range, ieee754, sec-03]
status: complete

dependency_graph:
  requires: []
  provides: [coordinate-range-guard-detect-location]
  affects: [src/location.rs]

tech_stack:
  added: []
  patterns:
    - RangeInclusive::contains for f64 range checks (rejects NaN/infinity by IEEE 754 ordering)
    - tracing::debug! static string (no PII interpolation)

key_files:
  created: []
  modified:
    - src/location.rs

decisions:
  - Predicate inlined per D-05 (no pub(crate) valid_coords() helper extracted; extraction deferred to Phase 2 FAULT-02 when a second call site appears in weather_jma.rs)
  - Used !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) not lat <= 90.0 && lat >= -90.0 (the flipped form is true-for-NaN; D-06 motivated the pinning tests)
  - Error::LocationDetection unit variant reused unchanged (D-04); wire snapshot bytes are identical

metrics:
  duration: "~8 minutes"
  completed: "2026-06-30"
  tasks_total: 1
  tasks_completed: 1
  files_changed: 1
  lines_added: 71
---

# Phase 01 Plan 02: Coordinate validation in detect_location() Summary

Inline range guard in `detect_location()` rejects ip-api.com responses with out-of-range, NaN, or infinite lat/lon using `RangeInclusive::contains` (IEEE 754 ordering); three unit tests pin the predicate semantics so a future "simplification" rewrite fails CI.

## What Was Built

Added a five-line range guard to `src/location.rs::detect_location()` immediately after the `if let (Some(lat), Some(lon)) = (data.lat, data.lon)` arm opens. The guard emits a static `tracing::debug!` line on rejection (no coord value, no PII) and returns `Err(Error::LocationDetection)`.

Added three unit tests in a new `#[cfg(test)] mod tests` block at the end of `src/location.rs`. The tests assert the `RangeInclusive::contains` semantics the guard relies on rather than calling the async function (no mock HTTP required per D-06).

## Guard Insertion Location

| Item | Line (after insertion) |
|------|----------------------|
| `if !(-90.0..=90.0).contains(&lat)` guard | 109 |
| `tracing::debug!("detect_location: coordinates out of valid range")` | 110 |
| `return Err(Error::LocationDetection)` | 111 |
| `nan_coords_are_rejected` test function | 177 |
| `infinite_coords_are_rejected` test function | 190 |
| `out_of_range_coords_are_rejected` test function | 211 |

The `if let (Some(lat), Some(lon))` arm was at line 107 pre-insertion; the guard occupies lines 109-112 in the modified file (shifted by 0 additional lines since the guard was inserted at line 108).

## Predicate Form

Inlined per D-05. Predicate:

```rust
if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
    // range-contains rejects NaN and infinity by IEEE 754 ordering
    tracing::debug!("detect_location: coordinates out of valid range");
    return Err(Error::LocationDetection);
}
```

Not extracted to a `pub(crate) fn valid_coords()` helper. That extraction is deferred to Phase 2 (FAULT-02), which will introduce a second call site in `weather_jma.rs` and make a shared helper worthwhile.

## CI Gates

All four gates passed on the first run:

| Gate | Result |
|------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy --workspace -- -D warnings` | pass (0 warnings) |
| `cargo build --workspace` | pass |
| `cargo test --workspace` | pass (54 unit + 22 wire contract = 76 total) |

No clippy or fmt nudges were needed. The `RangeInclusive::contains(&x)` form is the idiomatic form clippy prefers (`manual_range_contains` lint approves it).

## Negative Verifications

- `git diff src/error.rs` is empty: `Error::LocationDetection` unit variant untouched.
- `git diff tests/snapshots/wire_contract__wire_error_LocationDetection.snap` is empty: snapshot bytes identical to pre-plan state.

## Deviations from Plan

None. Plan executed exactly as written.

## Threat Register Coverage

| Threat | Mitigated? |
|--------|-----------|
| T-02-01: Malformed lat/lon from ip-api.com propagating to downstream URL construction | Yes — inline guard returns Err(LocationDetection) before DetectedLocation is built |
| T-02-02: Future refactor re-admitting NaN via flipped predicate | Yes — three pin tests catch `lat <= 90.0 && lat >= -90.0` regression in CI |
| T-02-03: Rejection log embedding the coord value | Accepted/mitigated — static format string, no coord interpolation |

## Known Stubs

None.

## Threat Flags

None. No new public symbols, no new network endpoints, no schema changes.

## Commits

- `6146dfa`: fix(01-02): reject out-of-range, NaN, and infinite coords in detect_location()

## Self-Check: PASSED

- `src/location.rs` exists and contains all three test function names
- `git diff src/error.rs` empty
- `git diff tests/snapshots/wire_contract__wire_error_LocationDetection.snap` empty
- Commit `6146dfa` exists in git log
- All 76 tests pass
