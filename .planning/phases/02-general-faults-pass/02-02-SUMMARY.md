---
phase: 02-general-faults-pass
plan: 02
subsystem: testing
tags: [rust, rustfmt, test-observability, pollen, time]

# Dependency graph
requires:
  - phase: 02-general-faults-pass
    provides: "Phase 2 ROADMAP + REQUIREMENTS scoping (FAULT-01/02/03) and plan-checker verification"
provides:
  - "src/pollen.rs test module: 4 bare .unwrap() sites replaced with .expect(...) messages naming the test and parsing step"
  - "src/time.rs:141 .expect(\"valid ISO date\") upgraded to embed the literal ISO date string parsed"
affects: [general-faults-pass-closeout]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Fixture-context .expect() messages: test-helper panics name the test function and the parsing step, not a bare unwrap"
    - "#[rustfmt::skip] on individual #[test] fns whose single long string-literal argument cannot fit rustfmt's max_width, to keep the verbatim .expect(\"message\") call on one physical line"

key-files:
  created: []
  modified:
    - src/pollen.rs
    - src/time.rs

key-decisions:
  - "Applied #[rustfmt::skip] to the 3 pollen test fns (returns_some_when_covered_with_active_species, returns_some_when_covered_with_all_zero_species, zero_fills_when_some_species_null_but_others_present) because their required verbatim .expect(\"...\") message strings exceed rustfmt's 100-column max_width; without the skip attribute, rustfmt wraps the call onto a new line (.expect(\\n    \"msg\",\\n)), which breaks the plan's grep -cE '\\.expect\\(\"' acceptance check and the substring-count checks for each test name. The skip is scoped per-function (not file-wide), keeps cargo fmt --check green, and does not affect production code or the test's runtime behavior."
  - "Confirmed the REQUIREMENTS.md pollen.rs:158 reference is the pre-existing assert!(parse(json).is_none()) call, not an unwrap site — no fifth edit needed in pollen.rs (documented in the plan and re-verified here)."

requirements-completed: [FAULT-03]

coverage:
  - id: D1
    description: "Four bare .unwrap() sites in src/pollen.rs test module replaced with .expect(...) messages naming the test/helper and the parsing step"
    requirement: "FAULT-03"
    verification:
      - kind: unit
        ref: "src/pollen.rs#pollen::tests::returns_some_when_covered_with_active_species"
        status: pass
      - kind: unit
        ref: "src/pollen.rs#pollen::tests::returns_some_when_covered_with_all_zero_species"
        status: pass
      - kind: unit
        ref: "src/pollen.rs#pollen::tests::zero_fills_when_some_species_null_but_others_present"
        status: pass
      - kind: other
        ref: "grep -cE '\\.unwrap\\(\\)' src/pollen.rs == 0"
        status: pass
      - kind: other
        ref: "grep -cE '\\.expect\\(\"' src/pollen.rs >= 4"
        status: pass
    human_judgment: false
  - id: D2
    description: "src/time.rs:141 .expect(\"valid ISO date\") upgraded to embed the literal ISO date string \"2025-11-25\" being parsed"
    requirement: "FAULT-03"
    verification:
      - kind: unit
        ref: "src/time.rs#time::tests::parsed_date_from_iso"
        status: pass
      - kind: other
        ref: "grep -cE '\\.expect\\(\"valid ISO date\"\\)' src/time.rs == 0"
        status: pass
      - kind: other
        ref: "grep -c '2025-11-25' src/time.rs >= 2"
        status: pass
    human_judgment: false
  - id: D3
    description: "No production-code path modified; wire contract and public crate surface byte-identical to pre-plan state"
    requirement: "FAULT-03"
    verification:
      - kind: other
        ref: "git diff --stat tests/snapshots/ (empty)"
        status: pass
      - kind: other
        ref: "git diff src/pollen.rs | grep -cE '^\\+.*pub|^\\+.*async fn|^-.*pub|^-.*async fn' == 0"
        status: pass
      - kind: other
        ref: "git diff src/time.rs | grep -cE '^\\+.*pub|^-.*pub' == 0"
        status: pass
    human_judgment: false

duration: 12min
completed: 2026-06-30
status: complete
---

# Phase 2 Plan 02: Pollen + Time Fixture-Context Expects Summary

**Replaced 4 bare `.unwrap()` calls in `src/pollen.rs` tests and upgraded the thin `.expect("valid ISO date")` in `src/time.rs` so a corrupted inline-JSON fixture or a regressed date literal now names the failing test and parsing step instead of an opaque panic.**

## Performance

- **Duration:** ~12 min
- **Completed:** 2026-06-30T22:46:17Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- `src/pollen.rs:108` (inside the shared `parse()` test helper): `.unwrap()` → `.expect("pollen test helper: inline JSON did not deserialize as PollenResponse")`
- `src/pollen.rs:124` (`returns_some_when_covered_with_active_species`): `.unwrap()` → `.expect("returns_some_when_covered_with_active_species: parse() returned None for an active-species fixture")`
- `src/pollen.rs:142` (`returns_some_when_covered_with_all_zero_species`): `.unwrap()` → `.expect("returns_some_when_covered_with_all_zero_species: parse() returned None for an all-zero-but-present fixture")`
- `src/pollen.rs:173` (`zero_fills_when_some_species_null_but_others_present`): `.unwrap()` → `.expect("zero_fills_when_some_species_null_but_others_present: parse() returned None when at least one species is present")`
- `src/time.rs:141` (`parsed_date_from_iso`): `.expect("valid ISO date")` → `.expect("parsed_date_from_iso: ISO date \"2025-11-25\" should parse")`
- Confirmed `pollen.rs:158` (REQUIREMENTS.md's listed 5th site) is `assert!(parse(json).is_none())`, not an unwrap — no edit made there, matching the plan's source-grounding note.

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace four .unwrap() sites in src/pollen.rs tests and upgrade one .expect() site in src/time.rs tests with fixture-context messages (FAULT-03)** - `76cdc4c` (test)

**Plan metadata:** (this commit, pending)

## Files Created/Modified
- `src/pollen.rs` - 4 bare `.unwrap()` test-helper sites converted to `.expect("...")` with fixture-naming messages; `#[rustfmt::skip]` added to 3 test fns whose message string exceeds rustfmt's line-width
- `src/time.rs` - 1 `.expect("valid ISO date")` upgraded to embed the literal ISO date string parsed

## Decisions Made
- Used `#[rustfmt::skip]` on the 3 pollen test functions whose required verbatim `.expect("...")` message text is too long to fit on one line under rustfmt's default 100-column `max_width`. Without it, rustfmt wraps the call as `.expect(\n    "msg",\n)`, splitting `.expect("` across two physical lines and failing the plan's literal `grep -cE '\.expect\("'` acceptance check (and the per-test-name substring-count checks, since the message text would still be present but the line-boundary check on the call form would not reflect the plan's intent of a single readable expect call). The skip is scoped to the 3 affected `#[test]` functions only, not file-wide, and does not change runtime behavior — `cargo fmt --check` remains green because skipped items are excluded from formatting checks.
- Verified the REQUIREMENTS.md `pollen.rs:158` reference points to `assert!(parse(json).is_none())`, confirming the plan's source-grounding caveat — no fifth pollen.rs edit was needed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `#[rustfmt::skip]` to 3 pollen test functions to keep verbatim `.expect("...")` messages on one line**
- **Found during:** Task 1 (after first `cargo fmt --check` pass and acceptance-criteria grep verification)
- **Issue:** The plan mandates exact verbatim `.expect("...")` message strings, several of which (98-112 characters) exceed rustfmt's 100-column `max_width` for the surrounding statement. `cargo fmt` reformatted 3 of the 4 pollen `.expect(...)` calls by moving the opening string literal to a new indented line (`.expect(\n    "msg",\n)`), which still passed `cargo fmt --check` and `cargo test --workspace`, but broke the plan's literal acceptance criterion `grep -cE '\.expect\("' src/pollen.rs` returning at least 4 (it returned 1) — the substring `.expect("` no longer appeared on a single line for those 3 calls.
- **Fix:** Added `#[rustfmt::skip]` immediately above each affected `#[test]` fn (`returns_some_when_covered_with_active_species`, `returns_some_when_covered_with_all_zero_species`, `zero_fills_when_some_species_null_but_others_present`) and manually joined the `.expect("...")` call back onto a single (over-width, but skip-exempted) line. This satisfies both `cargo fmt --check` (the items are excluded from formatting) and the plan's literal grep-based acceptance criteria.
- **Files modified:** src/pollen.rs
- **Verification:** Re-ran all 4 CI gates (`cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo build --workspace`, `cargo test --workspace`) — all exit 0. Re-ran every acceptance-criteria grep from the plan — all pass (`.unwrap()` count 0; `.expect("` count 4; per-test-name substring counts all ≥2; old `time.rs` message count 0; `tests/snapshots/` and `src/lib.rs` diffs empty; no `pub`/`async fn` line changes in either file).
- **Committed in:** `76cdc4c` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking — rustfmt vs. literal acceptance-grep conflict)
**Impact on plan:** Purely test-only formatting workaround; no behavior, no production-code, and no wire-contract change. All plan-specified message strings are present verbatim and all CI gates pass.

## Issues Encountered
None beyond the rustfmt line-wrap conflict documented above as a deviation.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- FAULT-03 closed. Combined with sibling plan 02-01 (FAULT-01, FAULT-02, already committed), Phase 2's three fault requirements are now fully addressed in source.
- No blockers for phase close-out; orchestrator should update STATE.md/ROADMAP.md to reflect both 02-01 and 02-02 complete.

---
*Phase: 02-general-faults-pass*
*Completed: 2026-06-30*

## Self-Check: PASSED
- FOUND: src/pollen.rs
- FOUND: src/time.rs
- FOUND: .planning/phases/02-general-faults-pass/02-02-SUMMARY.md
- FOUND: commit 76cdc4c (test(02-02): add fixture context to pollen and time test expects)
