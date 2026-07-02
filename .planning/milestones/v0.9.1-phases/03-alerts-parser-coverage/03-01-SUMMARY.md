---
phase: 03-alerts-parser-coverage
plan: 01
subsystem: testing
tags: [rust, serde, coverage, llvm-cov, alerts]

# Dependency graph
requires:
  - phase: 02-general-faults-pass
    provides: panic-safe production code paths (prerequisite for stable coverage baseline)
provides:
  - Three private sync helpers extracted from src/alerts.rs (nws_alerts_from_response, bom_alerts_from_response, meteoalarm_alerts_from_feed)
  - NWS GeoJSON parser test suite (5 tests) + AlertSeverity::from_cap_string test (1 test)
  - BOM JSON parser test suite (6 tests)
  - Measured src/alerts.rs + workspace TOTAL line-coverage baseline and post-Wave-1 delta
affects: [03-02-meteoalarm-eccc-dispatch-coverage]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Sync-helper extraction for testability (mirrors air_quality_aqicn.rs::extract_aqi) — move filter_map transform bodies out of async fetch_* fns into private sync fns callable from both production and #[test] fixtures"

key-files:
  created: []
  modified:
    - src/alerts.rs

key-decisions:
  - "cargo llvm-cov ran successfully with the plain invocation (no env -u RUSTC_WRAPPER workaround needed) despite RUSTC_WRAPPER=sccache being set in this environment — Codex's blocked-by-sccache finding did not reproduce here"
  - "Coverage measured before and after Wave 1 per REVIEWS.md finding 5: alerts.rs 0.00% -> 46.37%, workspace TOTAL 49.85% -> 57.80% (line coverage), giving Plan 02 a measured trajectory instead of an assumption"

patterns-established:
  - "Pattern: extract-for-testability sync helper + #[cfg(test)] mod tests with inline r#\"...\"# JSON fixtures, no insta, no live HTTP"

requirements-completed: []  # COV-01 spans both 03-01 and 03-02; not marked complete until Wave 2 (Plan 02) lands

coverage:
  - id: D1
    description: "Extract NWS/BOM/MeteoAlarm-feed transform logic into private sync helpers with zero behavior change"
    verification:
      - kind: unit
        ref: "cargo test --workspace (61 lib tests + 22 wire_contract tests, pre-existing suite green after extraction)"
        status: pass
      - kind: other
        ref: "cargo clippy --workspace -- -D warnings"
        status: pass
    human_judgment: false
  - id: D2
    description: "NWS GeoJSON parser proven via fixture tests: active alert, expired drop, null-severity fallback, null-expires sent+24h fallback, null-headline/description empty-string fallback"
    requirement: COV-01
    verification:
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::nws_decodes_active_alert"
        status: pass
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::nws_drops_expired_alert"
        status: pass
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::nws_null_severity_falls_back_to_unknown"
        status: pass
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::nws_null_expires_uses_sent_plus_24h"
        status: pass
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::nws_null_headline_and_description_default_to_empty"
        status: pass
    human_judgment: false
  - id: D3
    description: "AlertSeverity::from_cap_string proven across all five classes plus Unknown fallback (including major->Severe alias)"
    requirement: COV-01
    verification:
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::from_cap_string_maps_all_classes"
        status: pass
    human_judgment: false
  - id: D4
    description: "BOM JSON parser proven via fixture tests: active-severe decode, major==severe overlap, cancelled-phase drop, expiry drop, missing short_title/warning_type fallbacks"
    requirement: COV-01
    verification:
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::bom_decodes_active_severe_warning"
        status: pass
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::bom_major_group_type_maps_to_severe"
        status: pass
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::bom_drops_cancelled_phase"
        status: pass
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::bom_drops_expired_warning"
        status: pass
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::bom_missing_short_title_uses_default_headline"
        status: pass
      - kind: unit
        ref: "src/alerts.rs#alerts::tests::bom_missing_warning_type_uses_headline_as_event"
        status: pass
    human_judgment: false
  - id: D5
    description: "Measured src/alerts.rs + workspace TOTAL line-coverage baseline (pre-extraction) and post-Wave-1 numbers recorded for Plan 02's 65% gate"
    verification:
      - kind: other
        ref: "cargo llvm-cov --workspace --summary-only (baseline: alerts.rs 0.00%/394 lines, TOTAL 49.85%; post-Wave-1: alerts.rs 46.37%/578 lines, TOTAL 57.80%)"
        status: pass
    human_judgment: false

duration: ~15min
completed: 2026-07-02
status: complete
---

# Phase 3 Plan 1: NWS/BOM Alert Parser Extraction and Test Coverage Summary

**Extracted NWS/BOM/MeteoAlarm transform logic into testable private sync helpers and proved both regional JSON parsers plus `AlertSeverity::from_cap_string` with 12 fixture-based tests, lifting `src/alerts.rs` line coverage from 0.00% to 46.37%.**

## Performance

- **Duration:** ~15 min
- **Completed:** 2026-07-02
- **Tasks:** 3/3
- **Files modified:** 1 (`src/alerts.rs`)

## Coverage Baseline (pre-extraction)

Measured before any source change, via:

```
cargo llvm-cov --workspace --summary-only
```

(Plain invocation — no `env -u RUSTC_WRAPPER -u CARGO_BUILD_RUSTC_WRAPPER` workaround was needed in this environment, even though `RUSTC_WRAPPER=sccache` was set. Codex's sccache-blocked finding from 03-REVIEWS.md did not reproduce here.)

| Metric | Value |
|---|---|
| `src/alerts.rs` line coverage | 0.00% (0/394 lines) |
| Workspace `TOTAL` line coverage | 49.85% (1014/2034 lines) |

(Note: PROJECT.md records the pre-milestone workspace baseline as 48.72% — that figure is the **region** coverage column from the same tool run, not the line-coverage column. This SUMMARY reports the `Lines`-column percentage per the plan's literal "line-coverage" instruction; both numbers come from the identical `cargo llvm-cov` invocation.)

## Coverage after Wave 1

Re-measured after Task 3 (same invocation), via:

```
cargo llvm-cov --workspace --summary-only
```

| Metric | Value | Delta from baseline |
|---|---|---|
| `src/alerts.rs` line coverage | 46.37% (268/578 lines) | +46.37 pp |
| Workspace `TOTAL` line coverage | 57.80% (1282/2218 lines) | +7.95 pp |

`src/alerts.rs`'s total line count grew from 394 to 578 because the new `#[cfg(test)] mod tests` block (fixtures + assertions) is itself counted in the file's line total by `cargo llvm-cov` — expected, and flagged in 03-REVIEWS.md as a caveat on in-file test-module coverage math. This is telemetry for Plan 02, not a gate (the 65% bar is Plan 02 Task 3's).

## Accomplishments

- Extracted `nws_alerts_from_response`, `bom_alerts_from_response`, and `meteoalarm_alerts_from_feed` as private sync helpers with zero behavior change, mirroring the existing `air_quality_aqicn.rs::extract_aqi` "pulled out for testability" idiom.
- Proved the NWS GeoJSON parser across 5 fixture tests: active-alert decode, expired-alert drop, null-severity-to-Unknown fallback, null-expires-sent+24h fallback, and null-headline/description-to-empty-string fallback.
- Proved `AlertSeverity::from_cap_string` across all six input classes (minor/moderate/severe/major/extreme/unrecognized), including the "major"->Severe alias.
- Proved the BOM JSON parser across 6 fixture tests: active-severe decode (event underscore->space formatting), major==severe group-type overlap, cancelled-phase filter, expiry filter, missing-short_title fallback ("Weather Warning"), and missing-warning_type fallback (headline used as event).
- Measured and recorded both the pre-extraction baseline and post-Wave-1 `src/alerts.rs` + workspace `TOTAL` line-coverage numbers, turning Plan 02's 65% gate into a measured delta.

## Task Commits

Each task was committed atomically:

1. **Task 1: Baseline coverage measurement + extract NWS/BOM/MeteoAlarm transform logic into private sync helpers** - `5899eef` (refactor)
2. **Task 2: NWS JSON parser tests + AlertSeverity::from_cap_string tests** - `4c48a1a` (test)
3. **Task 3: BOM JSON parser tests + post-Wave-1 coverage measurement** - `3cc8bd3` (test)

_Baseline coverage measurement (Task 1 Step 0) was run and recorded before the first commit; it produced no code change so has no commit of its own._

## Files Created/Modified

- `src/alerts.rs` - Extracted 3 private sync helpers (`nws_alerts_from_response`, `bom_alerts_from_response`, `meteoalarm_alerts_from_feed`); added `#[cfg(test)] mod tests` with 12 new `#[test]` fns (5 NWS + 1 severity + 6 BOM) and inline JSON fixtures.

## Decisions Made

- Used the plain `cargo llvm-cov --workspace --summary-only` invocation for both coverage measurements; the pre-planned `env -u RUSTC_WRAPPER -u CARGO_BUILD_RUSTC_WRAPPER` sccache workaround was not needed in this environment (worth noting for Plan 02, which will re-run the same working command).
- Reported the `Lines` column (not `Regions`) for "line coverage" per the plan's literal wording, and noted the discrepancy with PROJECT.md's 48.72% (which is the `Regions` column from the same measurement) so the number isn't misread as a regression.

## Deviations from Plan

**Worktree base was stale (infrastructure, not plan content).** This worktree's branch (`worktree-agent-a91eae9993d42a12f`) was created from a commit (`8828756`) that predates all of Phase 3's planning commits — `03-01-PLAN.md`, `03-RESEARCH.md`, `03-VALIDATION.md`, and `03-REVIEWS.md` did not exist in the worktree's `.planning/` at session start. Confirmed via `git merge-base` that the worktree branch was a strict ancestor of `chore/v0.9.1-coverage-milestone` (the source-of-truth branch, with tip `b986538`), then fast-forwarded (`git merge chore/v0.9.1-coverage-milestone --ff-only`) to bring the plan files in before execution. No rebase, no rewrite, no destructive operation — the worktree branch simply caught up to its own upstream via a fast-forward. Not a Rule 1-4 deviation (nothing in the plan or codebase was wrong); flagging as an infrastructure note for the orchestrator in case other Wave 1 agents hit the same stale-worktree-base condition.

Otherwise: None - plan executed exactly as written (all `must_haves`, `acceptance_criteria`, and `done` conditions across all 3 tasks met without needing Rule 1-4 auto-fixes).

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 02 (Wave 2, depends on 03-01) can proceed: the three sync helpers this plan created are directly usable by Plan 02's XML/dispatch tests, and `meteoalarm_alerts_from_feed` was extracted specifically for Plan 02's MeteoAlarm feed-level test.
- Plan 02 Task 3's 65% coverage gate now has a real trajectory to reason from: baseline 0.00% -> post-Wave-1 46.37% on `src/alerts.rs` (+46.37pp in one wave). Whether the remaining MeteoAlarm/ECCC/dispatch tests close the gap to 65% is Plan 02's open question, not pre-decided here.
- Wire contract confirmed byte-identical throughout (`cargo test --test wire_contract` green, `git status --short tests/snapshots/` empty after every task).
- No blockers for Plan 02.

---
*Phase: 03-alerts-parser-coverage*
*Completed: 2026-07-02*

## Self-Check: PASSED

- FOUND: src/alerts.rs
- FOUND: .planning/phases/03-alerts-parser-coverage/03-01-SUMMARY.md
- FOUND: 5899eef (Task 1 commit)
- FOUND: 4c48a1a (Task 2 commit)
- FOUND: 3cc8bd3 (Task 3 commit)
