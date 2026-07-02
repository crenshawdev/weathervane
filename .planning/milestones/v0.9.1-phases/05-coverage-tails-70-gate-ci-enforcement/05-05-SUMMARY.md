---
phase: 05-coverage-tails-70-gate-ci-enforcement
plan: 05
subsystem: testing
tags: [rust, cargo-llvm-cov, gitlab-ci, coverage-gate]

# Dependency graph
requires:
  - phase: 05-coverage-tails-70-gate-ci-enforcement
    provides: "Wave 1 tail-lift tests (05-01 codes.rs, 05-02 geo.rs, 05-03 time.rs, 05-04 weather_jma.rs) that raised workspace TOTAL to 85.20% before this plan's CI gate was added"
provides:
  - "Workspace line coverage measured and recorded at 85.20% post-Wave-1 (COV-07, transitively >=70%)"
  - ".gitlab-ci.yml coverage job gated with --fail-under-lines 75 (COV-08)"
  - "Phase-level rollup: per-module tail-lift table, workspace TOTAL trajectory, accepted-gap summary, milestone completion checkpoint"
affects: [v0.9.1-milestone-completion]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CI gate landed only after empirical local dry-run of the exact CI command sequence exits 0 (D-04 sequencing invariant): measure -> compare against 75% -> dry-run the two-step CI form -> only then commit the flag"

key-files:
  created: []
  modified:
    - .gitlab-ci.yml

key-decisions:
  - "Form A (single-flag append to line 80) used exactly as specified by the plan; Form B (separate step) explicitly rejected to keep the diff to 1 line added / 1 line removed"

patterns-established: []

requirements-completed: [COV-07, COV-08]

coverage:
  - id: D1
    description: "Post-Wave-1 workspace TOTAL line coverage measured via cargo llvm-cov --workspace --summary-only and recorded at 85.20%, >= 75% D-01 gate value and >= 70% COV-07 milestone bar"
    requirement: "COV-07"
    verification:
      - kind: unit
        ref: "cargo llvm-cov --workspace --summary-only (TOTAL line 3445/510 missed = 85.20%)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Local two-step CI dry-run (cargo llvm-cov --no-report --workspace + cargo llvm-cov report --summary-only --fail-under-lines 75) exits 0 against the post-Wave-1 tree, proving the exact CI command form passes before the .gitlab-ci.yml edit was committed"
    requirement: "COV-07"
    verification:
      - kind: other
        ref: "cargo llvm-cov --no-report --workspace && cargo llvm-cov report --summary-only --fail-under-lines 75 (exit 0)"
        status: pass
    human_judgment: false
  - id: D3
    description: ".gitlab-ci.yml coverage job line 80 has --fail-under-lines 75 appended (Form A single-line edit); git diff --numstat reports 1 added + 1 removed; grep for the literal flag yields exactly 1 hit; coverage: regex, Cobertura export, before_script, and all other jobs unchanged"
    requirement: "COV-08"
    verification:
      - kind: other
        ref: "git diff --numstat .gitlab-ci.yml (1 1); grep -c -- '--fail-under-lines 75' .gitlab-ci.yml (1)"
        status: pass
    human_judgment: false

duration: 12min
completed: 2026-07-02
status: complete
---

# Phase 5 Plan 5: Post-Wave-1 Coverage Verification & CI Gate Summary

**Workspace line coverage verified at 85.20% post-Wave-1 (10.20pp above the 75% gate), then `--fail-under-lines 75` landed in `.gitlab-ci.yml`'s `coverage` job as a single-line Form A edit, closing out the v0.9.1 Test Coverage Lift milestone (COV-06, COV-07, COV-08)**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-07-02T20:24:00Z
- **Completed:** 2026-07-02T20:36:00Z
- **Tasks:** 3
- **Files modified:** 2 (`.gitlab-ci.yml`, `.planning/phases/05-coverage-tails-70-gate-ci-enforcement/05-05-SUMMARY.md`)

## Post-Wave-1 coverage measurement (Task 1)

**Invocation:** `cargo llvm-cov --workspace --summary-only` (plain invocation worked without an sccache workaround, consistent with every prior Phase 3/4/05-01..04 finding).

| Metric | Value |
|---|---|
| Workspace `TOTAL` line coverage | **85.20%** (3445 lines, 510 missed) |
| Workspace `TOTAL` region coverage | 83.02% (4942 regions, 839 missed) |
| Workspace `TOTAL` function coverage | 85.16% (384 functions, 57 missed) |

**Gate comparison:** 85.20% >= 75.0% (D-01 gate value) — **PASS**, 10.20pp of headroom. Not blocked; proceeded to Task 2.

**Local gate-command dry-run (STEP 3):** ran the exact two-step form CI will run —
`cargo llvm-cov --no-report --workspace` then `cargo llvm-cov report --summary-only --fail-under-lines 75` — and it **exited 0**. `cargo-llvm-cov 0.8.7` (local version) recognizes `--fail-under-lines`; no version-mismatch note needed.

`cargo test --workspace` was green (194 unit tests + 22 wire-contract tests, all passing) and `git status --short .gitlab-ci.yml src/ tests/snapshots/ tests/wire_contract.rs` was empty at the end of Task 1 — measurement only, no file changes yet.

## Per-module tail-lift table

| Module | Pre-Phase-5 baseline | Post-Phase-5 (this plan's tree) | Delta | Source |
|---|---|---|---|---|
| `codes.rs` | 62.37% | 99.52% | **+37.15pp** | `.planning/phases/05-coverage-tails-70-gate-ci-enforcement/05-01-SUMMARY.md` |
| `geo.rs` | 70.13% | 94.49% | **+24.36pp** | `.planning/phases/05-coverage-tails-70-gate-ci-enforcement/05-02-SUMMARY.md` |
| `time.rs` | 74.51% | 99.41% | **+24.90pp** | `.planning/phases/05-coverage-tails-70-gate-ci-enforcement/05-03-SUMMARY.md` |
| `weather_jma.rs` | 71.95% | 87.70% | **+15.75pp** | `.planning/phases/05-coverage-tails-70-gate-ci-enforcement/05-04-SUMMARY.md` |

All four COV-06 target modules moved well above their pre-Phase-5 baselines. `codes.rs` and `time.rs` are effectively saturated (99%+); the one residual missed line in each is an intentionally-untestable `unreachable!()` fallback (codes.rs) or a residual partial-branch line with all functions at 100% (time.rs). `geo.rs`'s remaining gap is the async `detect_country_from_coords` HTTP path (accepted, see below). `weather_jma.rs`'s remaining gap is its live-JMA-HTTP async surface (accepted, see below).

## Workspace TOTAL trajectory

| Point | Workspace `TOTAL` | Delta from prior |
|---|---|---|
| Pre-Phase-5 baseline (`05-CONTEXT.md`, 2026-07-02) | 77.87% | — |
| Post-05-01 (codes.rs) | 79.83% | +1.96pp |
| Post-05-02 (geo.rs) | 82.65% | +2.82pp |
| Post-05-03 (time.rs) | 83.80% | +1.15pp |
| Post-05-04 (weather_jma.rs) | 85.20% | +1.40pp |
| **Post-Wave-1 (this plan, Task 1 measurement)** | **85.20%** | **+7.33pp total from pre-Phase-5** |

Both the D-01 gate value (>=75%) and the COV-07 milestone bar (>=70%) are proven transitively: the measured post-Wave-1 `TOTAL` of 85.20% clears both with comfortable margin (10.20pp and 15.20pp respectively).

**Caveat (per Phase 4's `04-02-SUMMARY.md:156`):** `cargo llvm-cov` counts `#[cfg(test)] mod tests` blocks as lines of source in the same file. A large new test module on a small source file can dilute per-module percentages even while covered-line count rises. Per-module deltas above are therefore not strictly monotonic proof of "more coverage" in isolation — the workspace `TOTAL` (which is the number the CI gate reads via the `coverage:` regex) is the single load-bearing number, and it rose at every step of Wave 1.

## `.gitlab-ci.yml` diff (Task 2)

**Before (line 80):**
```
    - cargo llvm-cov report --summary-only
```

**After (line 80):**
```
    - cargo llvm-cov report --summary-only --fail-under-lines 75
```

Form A only — single-line replacement, no new step, no new comment. `git diff --numstat .gitlab-ci.yml` reported exactly `1 1` (1 line added, 1 line removed). `grep -c -- '--fail-under-lines 75' .gitlab-ci.yml` returned exactly `1`. The `coverage:` regex (`'/TOTAL\s+(\d+\s+)+(\d+\.\d+\%)/'`), the Cobertura export step, the `before_script` (llvm-tools-preview install + cargo-llvm-cov install-if-missing), the `artifacts` block, and the `rules` block are all byte-identical to before. No other CI job (`fmt`, `clippy`, `build`, `test`) was touched.

**Working local command sequence that passed green (mirrors the CI job exactly):**
```bash
mkdir -p target/llvm-cov-target
cargo llvm-cov --no-report --workspace
cargo llvm-cov report --cobertura --output-path target/llvm-cov-target/cobertura.xml
cargo llvm-cov report --summary-only --fail-under-lines 75
```
All four commands exited 0 against the post-edit tree; `cargo test --workspace` also passed (194 + 22 tests) immediately after.

## Accepted-gap phase-level summary

The following coverage gaps remain open at the end of Phase 5, by design (D-06/D-07 — out of scope for v0.9.1):

- **`detect_country_from_coords`** (`src/geo.rs:297-323`) — async, calls `http_client()` for live reverse-geocoding. Its fallback path (`approximate_european_country`) IS fully covered (05-02). Per D-06/D-07, tests target parse/logic helpers, not live HTTP.
- **The `weather_jma.rs` async surface** — `override_current_temp`'s sort + nearest-station-distance-check portion (`src/weather_jma.rs:53-67`), `cached_stations`, `fetch_stations`, `latest_observation_time`, `fetch_map` — all coupled to live JMA HTTP endpoints via `crate::client::{get_json, get_text}`. The deterministically-testable portion (`select_temp_from_map`, `parse_station_entry`'s drop arm) was closed in 05-04.
- **Phase 4's remaining reqwest error classifier arms**, documented at `.planning/phases/04-domain-fetch-parse-coverage/04-04-SUMMARY.md:180-190` — still an accepted-open gap from the prior phase, not re-opened or re-scoped by Phase 5.

**Explicitly NOT listed:** `resolve_headline_aqi` (`src/air_quality.rs`) — Phase 4 Plan 02 proved all six of its branches (`.planning/phases/04-domain-fetch-parse-coverage/04-02-SUMMARY.md:160-161`). This is a closed gap, not an accepted-open one.

Phase 5 does not close any of the still-open gaps above; per D-06/D-07 they remain explicitly out of scope for v0.9.1 and are candidates for a future hardening milestone (already tracked in STATE.md's Deferred Items table for `network.rs`/`sleep.rs`, and noted here for `geo.rs`/`weather_jma.rs`/error-classifier arms).

## Milestone completion checkpoint

Phase 5 completes the **v0.9.1 Test Coverage Lift** milestone:

- **COV-06** (coverage tails lifted on `codes.rs`, `geo.rs`, `time.rs`, `weather_jma.rs`) — **satisfied**, per 05-01..05-04's per-module lifts (table above).
- **COV-07** (workspace line coverage >=70% verified via `cargo llvm-cov`) — **satisfied**, measured at 85.20% post-Wave-1, proven transitively via the stricter 75% D-01 gate value.
- **COV-08** (CI coverage gate — `--fail-under-lines` added to the GitLab `coverage` job) — **satisfied**, `.gitlab-ci.yml:80` now enforces `--fail-under-lines 75`.

All eight v0.9.1 requirements (COV-01 through COV-08) are now complete across Phases 3, 4, and 5.

## Task Commits

1. **Task 1: Measure post-Wave-1 workspace line coverage and verify it clears 75% locally** — no commit (measurement only, no source/CI change; numbers recorded above)
2. **Task 2: Add --fail-under-lines 75 flag to the coverage job in .gitlab-ci.yml** — `0af68f5` (feat)
3. **Task 3: Record final coverage numbers, CI diff, and phase-level accepted-gap summary** — this SUMMARY.md (no separate task commit; part of final docs commit)

**Plan metadata:** (this SUMMARY + STATE.md + ROADMAP.md + REQUIREMENTS.md commit follows separately)

## Files Created/Modified

- `.gitlab-ci.yml` — `coverage` job line 80: appended ` --fail-under-lines 75` to the existing `cargo llvm-cov report --summary-only` step (Form A, single-line edit). No other line changed.
- `.planning/phases/05-coverage-tails-70-gate-ci-enforcement/05-05-SUMMARY.md` — this file.

## Decisions Made

- Form A (single-flag append) used exactly as the plan specified; Form B (a separate `report --fail-under-lines 75` step) was explicitly rejected to keep the CI diff to exactly 1 line added / 1 line removed, preserving the `coverage:` regex scrape point and the Cobertura MR-widget behavior untouched.

## Deviations from Plan

None — plan executed exactly as written. The gate value (75), the flag form (Form A), and the sequencing (measure → compare → dry-run → commit) all matched the plan's must_haves and prohibitions exactly. No architectural changes, no bug fixes needed, no blocking issues encountered.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required. The CI gate takes effect automatically on the next pipeline run (merge request or default-branch push) once this branch merges.

## Next Phase Readiness

- v0.9.1 Test Coverage Lift milestone is complete: COV-01 through COV-08 all satisfied.
- Wire contract (`tests/wire_contract.rs`, `tests/snapshots/`) confirmed byte-identical throughout Phase 5 — `git status --short` was empty at every verify step across all five plans.
- `.gitlab-ci.yml` now enforces `--fail-under-lines 75` on every merge-request and default-branch pipeline; a future workspace line-coverage drop below 75% will fail the `coverage` job deterministically with a visible message in job logs.
- No blockers. Next step is milestone completion (`/gsd-complete-milestone`) to archive v0.9.1 and prepare for the next milestone cycle.

---
*Phase: 05-coverage-tails-70-gate-ci-enforcement*
*Completed: 2026-07-02*

## Self-Check: PASSED

- FOUND: .gitlab-ci.yml
- FOUND: 05-05-SUMMARY.md
- FOUND commit: 0af68f5
- grep -c -- '--fail-under-lines 75' .gitlab-ci.yml == 1
