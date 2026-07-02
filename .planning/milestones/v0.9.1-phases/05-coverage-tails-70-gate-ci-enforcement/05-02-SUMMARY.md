---
phase: 05-coverage-tails-70-gate-ci-enforcement
plan: 02
subsystem: testing
tags: [rust, cargo-llvm-cov, unit-tests, geo, meteoalarm, bounding-boxes]

# Dependency graph
requires: []
provides:
  - "src/geo.rs line coverage 70.13% -> 94.49% (+24.36pp) via 8 new #[test] fns"
  - "Measured pre/post cargo llvm-cov datum for Plan 05-05's workspace >=75% gate decision"
affects: [05-05-verify-ci-gate]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Branch-order-safe fixture selection: coordinates chosen to reach the intended arm under the CURRENT branch cascade, not real-world capital-city coordinates (Belgium (51.20,3.20) / Switzerland (47.20,10.00) instead of Brussels/Zurich)"

key-files:
  created: []
  modified:
    - src/geo.rs

key-decisions:
  - "Branch-order-safe fixtures for Belgium and Switzerland: real-world Brussels (50.85,4.35) hits France first, Zurich (47.38,8.55) hits Germany first — replaced with in-box points that survive the current cascade per plan D-08 requirement"

patterns-established: []

requirements-completed: [COV-06]

coverage:
  - id: D1
    description: "get_meteoalarm_info proven for 34 explicit country arms + case-insensitive lookup + 3 alias pairs (both sides) + Unknown fallback"
    requirement: "COV-06"
    verification:
      - kind: unit
        ref: "src/geo.rs#geo::tests::get_meteoalarm_info_*"
        status: pass
    human_judgment: false
  - id: D2
    description: "approximate_european_country proven for 11 outer bounding boxes (Germany/France/Spain/Italy/UK/Netherlands/Belgium/Switzerland/Austria/Poland + Nordic outer) + 3 Nordic sub-branches (Norway/Sweden/Finland) + Unknown fallback"
    requirement: "COV-06"
    verification:
      - kind: unit
        ref: "src/geo.rs#geo::tests::approximate_european_country_*"
        status: pass
    human_judgment: false
  - id: D3
    description: "is_us_bounds Upper Midwest (-95..-84) and Maine else-arm (-67..-66) continental bands proven via detect_region returning Region::Us"
    requirement: "COV-06"
    verification:
      - kind: unit
        ref: "src/geo.rs#geo::tests::detect_region_us_upper_midwest_and_maine_bands"
        status: pass
    human_judgment: false

duration: 1min
completed: 2026-07-02
status: complete
---

# Phase 5 Plan 2: geo.rs Coverage Tail Summary

**src/geo.rs line coverage lifted 70.13% -> 94.49% (+24.36pp) via 8 new fixture-based unit tests covering get_meteoalarm_info country/alias/case arms, approximate_european_country bounding boxes, and is_us_bounds continental bands**

## Performance

- **Duration:** ~1 min (executor Task 2 commit at 12:11:40; Task 3 measurement at 12:12)
- **Tasks:** 3
- **Files modified:** 1 (`src/geo.rs`, +141 lines, all under `#[cfg(test)] mod tests`)

## Coverage baseline (pre-plan)

**Invocation:** `cargo llvm-cov --workspace --summary-only` (plain invocation worked, matching Phase 3/4/05-01 findings).

| Metric | Value |
|---|---|
| `src/geo.rs` line coverage | 70.13% (~92 uncovered lines per 05-CONTEXT.md) |
| Workspace `TOTAL` line coverage | 79.83% (05-01 endpoint) |

Baseline for workspace is the post-05-01 endpoint (79.83%), not the phase-start figure (77.87%), since Plan 05-01 completed first and its delta is already banked.

## Coverage after Plan 05-02

**Invocation:** same as baseline: `cargo llvm-cov --workspace --summary-only`.

| Metric | Value | Delta |
|---|---|---|
| `src/geo.rs` line coverage | 94.49% (399 lines, 22 missed) | **+24.36pp** |
| `src/geo.rs` function coverage | 94.12% (34 functions, 2 missed) | +N/A (helpers now reached) |
| Workspace `TOTAL` line coverage | 82.65% (3135 lines, 544 missed) | **+2.82pp** |

Workspace already clears the 75% CI gate target set by D-04 / COV-07 with 7.65pp of headroom.

No anomaly: delta is strongly positive on both metrics.

## Targeted branches (Groups A-C from Task 2)

- **Group A** — `get_meteoalarm_info` (was 0% covered): 4 tests. `get_meteoalarm_info_maps_representative_countries` asserts 34 explicit country → (slug, ISO) tuple mappings covering every non-alias arm. `get_meteoalarm_info_case_insensitive` proves the `.to_lowercase()` normalization at line 232 (france/FRANCE/FrAnCe all resolve). `get_meteoalarm_info_alias_arms` asserts both sides of all three alias pairs (czechia/czech republic, north macedonia/macedonia, united kingdom/uk) return the same tuple. `get_meteoalarm_info_unknown_returns_none` proves the `_ => None` fallback for 4 non-covered inputs including the empty string.
- **Group B** — `approximate_european_country` (was 0% covered): 3 tests. `approximate_european_country_maps_bounding_boxes` covers all 10 non-Nordic outer branches (Germany/France/Spain/Italy/UK/Netherlands/Belgium/Switzerland/Austria/Poland) with interior coordinates. Belgium `(51.20, 3.20)` and Switzerland `(47.20, 10.00)` deliberately DIVERGE from real-world Brussels/Zurich coordinates because the actual cascade at src/geo.rs:338-370 evaluates Germany → France → … in order, and Brussels (50.85, 4.35) falls into France first while Zurich (47.38, 8.55) falls into Germany first. The chosen fixtures are branch-order-safe points inside each target's box that survive earlier branches. `approximate_european_country_nordic_sub_branches` covers the Nordic outer box's three lon sub-branches (Norway lon<10, Sweden 10≤lon<24.2, Finland lon≥24.2). `approximate_european_country_unknown_outside_boxes` covers the outer `else "Unknown"` fallback for 3 non-European coordinates.
- **Group C** — `is_us_bounds` continental bands: 1 test (`detect_region_us_upper_midwest_and_maine_bands`) covers Upper Midwest band (Minneapolis, -93.27 in -95..-84) and Maine else-arm band (Eastport, -66.99 in -67..-66) via `detect_region` end-to-end. Combined with the pre-existing `detect_region_routes_us_cities` test hitting Seattle/LA (Western), Chicago (Great Lakes), NYC (Northeast), Alaska, and Hawaii, all five continental bands + Alaska + Hawaii are now proven.

## Accepted-gap note

`detect_country_from_coords` at src/geo.rs:297-323 is async and calls `http_client()`. Per D-06 / D-07, tests target parse/logic helpers and do NOT execute live HTTP. Its fallback path (`approximate_european_country`) IS covered by Group B above, so the CLAUDE.md "silent regional fallthrough" contract is directly proven. Any future coverage of the reverse-geocoding HTTP path itself is out of scope for v0.9.1 and would require mocking `http_client()` or a fixture-based reqwest interceptor — deferred as a future-milestone item.

## Task Commits

1. **Task 1: Baseline coverage measurement** — no commit (measurement only, no source change; numbers recorded above)
2. **Task 2: Add tests for get_meteoalarm_info, approximate_european_country, and is_us_bounds continental bands** — `688af76` (test; +141 lines to src/geo.rs)
3. **Task 3: Post-plan coverage measurement** — no commit (measurement only; numbers recorded above)

## Files Created/Modified

- `src/geo.rs` — Added 8 new `#[test]` fns to the existing `#[cfg(test)] mod tests` block (Groups A-C above); no changes to `Region`, `NominatimResponse`, `NominatimAddress`, `MeteoAlarmCodenames`, `get_meteoalarm_info`, `approximate_european_country`, `is_us_bounds`, `detect_region`, `detect_country_from_coords`, or any `#[serde(...)]` attribute.

## Decisions Made

- Branch-order-safe fixture coordinates for Belgium `(51.20, 3.20)` and Switzerland `(47.20, 10.00)` instead of the real-world capital-city coordinates for Brussels and Zurich, because the cascade order at src/geo.rs:338-370 evaluates Germany → France → Spain → Italy → UK → Netherlands → Belgium → Switzerland → Austria → Poland → Nordic → Unknown, and Brussels (50.85, 4.35) actually falls into France first, while Zurich (47.38, 8.55) falls into Germany first. This decision was pre-recorded in the revised plan per the Codex adversarial-review feedback (REVIEWS.md) and applied verbatim during execution.

## Deviations from Plan

None — plan executed exactly as written. All 8 new tests match the plan's Group A-C specification exactly (test names, assertions, exact (slug, ISO code) tuples, branch-order-safe coordinates).

## Issues Encountered

None.

## User Setup Required

None.

## Next Phase Readiness

- `src/geo.rs` coverage tail is closed (94.49%, +24.36pp from baseline).
- Workspace `TOTAL` moved 79.83% -> 82.65%, already 7.65pp above the 75% CI gate target — Plan 05-05 (COV-07 workspace verification + COV-08 CI gate) has strong measured headroom.
- Wire contract (`tests/wire_contract.rs`, `tests/snapshots/`) confirmed byte-identical throughout (`git status --short` empty at every verify step).
- No blockers for Plans 05-03 (time.rs) and 05-04 (weather_jma.rs) — Wave 1 remaining.

---
*Phase: 05-coverage-tails-70-gate-ci-enforcement*
*Completed: 2026-07-02*

## Self-Check: PASSED
