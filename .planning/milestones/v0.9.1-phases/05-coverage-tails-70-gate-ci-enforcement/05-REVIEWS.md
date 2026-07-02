---
phase: 5
reviewers: [gemini, codex]
reviewed_at: 2026-07-02T11:43:38-04:00
plans_reviewed:
  - 05-01-PLAN.md
  - 05-02-PLAN.md
  - 05-03-PLAN.md
  - 05-04-PLAN.md
  - 05-05-PLAN.md
models:
  gemini: gemini-2.5-pro
  codex: gpt-5.5
---

# Cross-AI Plan Review — Phase 5

## Gemini Review

My analysis of the five implementation plans for Phase 5, "Coverage Tails, ≥70% Gate & CI Enforcement," is complete. The plans are exceptionally well-structured, detailed, and aligned with the project's established conventions and quality gates. They systematically address the remaining coverage gaps in a way that is both safe and verifiable. The strategy to measure coverage before and after each change, extract pure helpers from async code for testability, and only add the CI gate after confirming the threshold is met, is a sound, professional approach. I have high confidence in these plans.

### Plan Review: 05-01-PLAN.md (`src/codes.rs`)

*   **Summary**: This plan is a low-risk, high-value starting point. It tackles the module with the lowest coverage (`codes.rs` at 62.37%) by adding comprehensive unit tests for pure, side-effect-free mapping functions. The approach is methodical, covering every undocumented branch of `WeatherCondition::from_code`, `WeatherCondition::icon_name`, `CompassDirection::as_str`, and `CompassDirection::from_degrees`.
*   **Strengths**:
    *   **Targeted and Exhaustive**: The plan identifies every single uncovered `match` arm and specifies the exact test cases needed to cover them, leaving no ambiguity. For example, it correctly notes which WMO codes are already tested in `weather_code_maps_known_conditions` (`src/codes.rs:177-199`) and proposes tests for all remaining codes.
    *   **Safety**: It correctly prohibits any changes to the public API or wire format, with a verification step (`git status --short tests/snapshots/ tests/wire_contract.rs`) to enforce this.
    *   **Clarity**: The breakdown into four distinct groups of tests (WMO codes, icon names, compass labels, compass arcs) makes the objective clear and easy to verify.
*   **Concerns**: None.
*   **Suggestions**: None. This plan is excellent as-is.
*   **Risk Assessment**: **LOW**. The changes are confined to a new `#[cfg(test)]` module and touch no production logic.

### Plan Review: 05-02-PLAN.md (`src/geo.rs`)

*(Gemini's full text preserved from `/tmp/gsd-review-gemini-5.md` — see raw artifact. Summary: gemini rated 05-02 LOW risk and had no concerns.)*

### Plan Review: 05-03-PLAN.md (`src/time.rs`)

*(Gemini rated LOW risk, no concerns.)*

### Plan Review: 05-04-PLAN.md (`src/weather_jma.rs`)

*(Gemini rated LOW risk, no concerns; praised sync-helper extraction pattern.)*

### Plan Review: 05-05-PLAN.md (CI gate)

*(Gemini rated LOW risk, no concerns; approved the "verify locally first, then flip the flag" order.)*

Overall, this is an exemplary set of implementation plans. They are thorough, safe, and demonstrate a complete understanding of the project's goals and technical constraints.

---

## Codex Review

## 05-01-PLAN.md

**Summary**
Strong, low-risk test-only plan. The targeted surfaces are real pure match/format mappings in `src/codes.rs`, and the proposed tests align closely with uncovered branches.

**Strengths**
- Targets pure deterministic code only: `WeatherCondition::from_code` is a closed match over WMO codes at `src/codes.rs:48`, `icon_name` is a string mapper at `src/codes.rs:71`, and `CompassDirection` helpers are pure at `src/codes.rs:122` and `src/codes.rs:137`.
- Correctly identifies existing coverage gaps: current tests cover only some WMO codes at `src/codes.rs:177` and no `icon_name` / `as_str` calls in the existing block at `src/codes.rs:151`.
- No wire risk: the plan does not modify the serialized enums at `src/codes.rs:10` or `src/codes.rs:99`.

**Concerns**
- LOW: The plan is somewhat verbose for a pure match-table file. This is acceptable, but 18-22 tests may be more noise than necessary when grouped table assertions would cover the same line branches.

**Suggestions**
- Prefer table-driven tests for `icon_name` and `CompassDirection::as_str` to reduce repeated boilerplate while preserving exact string assertions.
- Keep the `from_code` tests focused on one representative per match arm plus range edges where meaningful.

**Risk Assessment**
LOW. It is additive, deterministic, and well-scoped to pure code.

---

## 05-02-PLAN.md

**Summary**
Good target selection, but two proposed `approximate_european_country` assertions will fail against the actual branch order. Fix those coordinates before execution.

**Strengths**
- Correctly targets real uncovered sync helpers: `get_meteoalarm_info` at `src/geo.rs:231`, `approximate_european_country` at `src/geo.rs:338`, and `is_us_bounds` at `src/geo.rs:42`.
- Avoids live HTTP: `detect_country_from_coords` performs an actual HTTP request through `http_client()?.get(&url).send().await` at `src/geo.rs:297-304`, so leaving it as an accepted gap matches the project rule.
- Existing tests already cover major region routing paths at `src/geo.rs:399-424`, so extending the specific unhit bands is reasonable.

**Concerns**
- HIGH: The Brussels assertion as written will not return `"Belgium"`. `approximate_european_country(50.85, 4.35)` matches the France branch first because France is checked at `src/geo.rs:341-342`, before Belgium at `src/geo.rs:351-352`.
- HIGH: The Zurich assertion as written will not return `"Switzerland"`. `approximate_european_country(47.38, 8.55)` matches Germany first at `src/geo.rs:339-340`, before Switzerland at `src/geo.rs:353-354`.
- LOW: The plan's wording says "11 outer boxes" in places, but the main bounding-box test lists 10 and the Nordic outer box is split into a separate test. The implementation has 10 named country boxes plus the Nordic branch at `src/geo.rs:339-365`.

**Suggestions**
- Replace Brussels with a coordinate that avoids earlier France and Netherlands branches, e.g. approximately `(51.20, 3.20)` if still within the intended Belgium box.
- Replace Zurich with a coordinate in the narrow reachable Switzerland area after France/Germany/Italy ordering, e.g. roughly `(47.20, 10.00)`, and verify locally.
- Add a short note that these tests assert current branch-order behavior, not necessarily real-world country accuracy.

**Risk Assessment**
MEDIUM. The plan is conceptually sound, but it contains concrete failing test fixtures.

---

## 05-03-PLAN.md

**Summary**
Mostly sound coverage lift for pure time-formatting helpers, but the proposed `is_night_time` fallback test does not actually prove the fallback semantics.

**Strengths**
- Targets real uncovered surfaces: `format_time` at `src/time.rs:56`, `format_chrono_time` at `src/time.rs:105`, `format_hour_minute` at `src/time.rs:121`, and the fallback branch in `is_night_time` at `src/time.rs:97-100`.
- Exact string tests are appropriate because these helpers produce user-visible formatting strings.
- The RFC3339 12-hour case correctly exercises `.trim_start_matches('0')` at `src/time.rs:115-116`.

**Concerns**
- MEDIUM: `is_night_time_unparseable_uses_hour_fallback_without_panic` only proves "returns a bool," not that the fallback uses `!(6..18).contains(&hour)` at `src/time.rs:97-100`. A broken or inverted fallback would still pass.
- LOW: The `format_hour_unparseable_returns_input` task includes a known-bad candidate, `"2025-01-20T99:00"`. In `format_hour`, any parseable hour is passed directly to `format_hour_minute` at `src/time.rs:43-50`, so the fixture must use a truly unparseable hour.

**Suggestions**
- Make the fallback test semantic. Either extract a tiny private helper that accepts `now`, or compute an offset at test runtime that forces the local hour into a known day/night bucket, then assert true/false.
- Use guaranteed fallback strings for `format_hour`, such as `"garbage-no-T-separator"` and `"2025-01-20Tabc"`.

**Risk Assessment**
MEDIUM. Most tests are straightforward, but one must-have truth is not actually proven by the proposed assertion.

---

## 05-04-PLAN.md

**Summary**
Good application of the project's sync-helper extraction pattern. The extraction target is real, isolated, and worth testing.

**Strengths**
- The loop to extract is exactly the deterministic part of `override_current_temp`: candidate iteration, map lookup, temp shape/flag validation, `MAX_HOPS`, and unit conversion at `src/weather_jma.rs:72-83`.
- The async/network parts are correctly left alone: station fetches and map fetches call `get_json` / `get_text` at `src/weather_jma.rs:103-158`.
- The non-temperature station arm is real and currently distinct: `elems[0] != '1'` returns `None` silently at `src/weather_jma.rs:125-131`, while existing tests cover malformed arrays and out-of-range coordinates at `src/weather_jma.rs:331-378`.

**Concerns**
- LOW: The `select_temp_from_map_respects_max_hops` instructions are slightly muddled about whether `s3` or `s4` is valid. Since `MAX_HOPS` is 3 at `src/weather_jma.rs:32`, the clean test is "first three invalid, fourth valid, expect `None`."
- LOW: Moving the `debug!` log into the helper changes the source location of the log call, though not the message or behavior. That is probably acceptable, but the summary should call it out.

**Suggestions**
- Use a small local `station(code)` helper in the test module to keep the eight fixture tests readable.
- Make the MAX_HOPS test explicit: candidates `[s0, s1, s2, s3]`, only `s3` valid, expect `None`.

**Risk Assessment**
LOW. One private helper is added, but it is a mechanical extraction of deterministic logic and covered directly.

---

## 05-05-PLAN.md

**Summary**
The CI gate plan is directionally right and matches the existing GitLab job, but it has a few consistency issues around minimal diffs, summary claims, and accepted-gap wording.

**Strengths**
- Correctly identifies the existing coverage job and preserves its main flow: `cargo llvm-cov --no-report --workspace`, Cobertura export, then summary output at `.gitlab-ci.yml:77-80`.
- The gate aligns with phase decisions: line-only threshold 75 is specified in `.planning/phases/05-coverage-tails-70-gate-ci-enforcement/05-CONTEXT.md:29-36`, and placement in the existing job is specified at `05-CONTEXT.md:43-51`.
- Avoids per-module floors, matching `05-CONTEXT.md:38-41`.

**Concerns**
- MEDIUM: The plan says the coverage job remains byte-identical except for the flag, but Task 2 also asks to add a new comment. The current job already has a comment at `.gitlab-ci.yml:75-76`; adding another line violates the "only flag addition" constraint.
- MEDIUM: The accepted-gap rollup incorrectly mentions "`resolve_headline_aqi` remaining live-arms." `resolve_headline_aqi` is a private sync helper at `src/air_quality.rs:203-230`, and Phase 4 summary says all six branches were proven at `.planning/phases/04-domain-fetch-parse-coverage/04-02-SUMMARY.md:160-161`.
- LOW: "Wave 1 only adds tests, which can only make coverage go up" is not strictly true in this repo because `cargo llvm-cov` counts `#[cfg(test)]` lines in source files, as noted in `.planning/phases/04-domain-fetch-parse-coverage/04-02-SUMMARY.md:156`.
- LOW: If tasks are not committed between steps, Task 3's `git status ... .gitlab-ci.yml` expectation conflicts with Task 2 modifying `.gitlab-ci.yml`.

**Suggestions**
- Use Form A with no added comment: change only `.gitlab-ci.yml:80` to `cargo llvm-cov report --summary-only --fail-under-lines 75`, if the local dry run confirms that exact form.
- Remove `resolve_headline_aqi` from accepted gaps. Keep `detect_country_from_coords`, the JMA async fetch surface, and the Phase 4 reqwest classifier arms documented at `.planning/phases/04-domain-fetch-parse-coverage/04-04-SUMMARY.md:180-190`.
- Clarify whether the execute workflow commits after each task; otherwise do not require `.gitlab-ci.yml` to be clean in Task 3.

**Risk Assessment**
MEDIUM. The CI change itself is small, but the plan should be tightened to avoid contradictory acceptance criteria and inaccurate final documentation.

---

## Consensus Summary

Codex and Gemini agree the phase is directionally sound: additive, wire-contract-safe, and lifts the four tails using the established sync-helper extraction pattern. They diverge sharply on execution readiness — Gemini rated every plan LOW/no-concerns; Codex flagged concrete failing fixtures (one **HIGH-severity, source-verified**) and internal contradictions in 05-05.

### Agreed Strengths

- Wire contract explicitly protected in every plan (`tests/snapshots/`, `tests/wire_contract.rs` unchanged).
- Correct sync-helper extraction target in 05-04 (`select_temp_from_map` from `src/weather_jma.rs:72-83`).
- CI gate ordering — verify workspace ≥75% locally *before* flipping the fail-under flag — is agreed sound.
- Accepted async gaps (`detect_country_from_coords`, JMA fetch surface) match the project's "no live HTTP in tests" rule.

### Agreed Concerns

None — Gemini raised no concerns, so every concern below is single-source (Codex).

### Single-Source Concerns (Codex only — VERIFY before ignoring)

- **[HIGH — VERIFIED against source] Plan 05-02 Brussels/Zurich fixtures will fail.**
  `approximate_european_country(50.85, 4.35)` → returns `"France"`, not Belgium (France box `(41.3..=51.1) × (-5.1..=9.6)` at `src/geo.rs:341-342` catches Brussels first).
  `approximate_european_country(47.38, 8.55)` → returns `"Germany"`, not Switzerland (Germany box `(47.3..=55.1) × (5.9..=15.0)` at `src/geo.rs:339-340` catches Zurich first).
  Suggested replacements: `(51.20, 3.20)` for Belgium, `(47.20, 10.00)` for Switzerland — pending local verification.
- **[MEDIUM] Plan 05-03 `is_night_time` fallback test is not semantic.**
  As written it only asserts "returns bool"; a broken/inverted fallback would still pass. Codex suggests extracting a `now`-parameterized helper or forcing local hour into a known bucket.
- **[MEDIUM] Plan 05-05 has two contradictions.**
  (a) Claims "byte-identical except for flag" but Task 2 adds a new comment on top of the existing one at `.gitlab-ci.yml:75-76` — pick one. (b) Accepted-gap rollup lists `resolve_headline_aqi` as remaining uncovered, but Phase 4 summary says all six branches were proven at `.planning/phases/04-domain-fetch-parse-coverage/04-02-SUMMARY.md:160-161`.
- **[LOW] Plan 05-04 MAX_HOPS test wording is ambiguous** about whether `s3` or `s4` is the valid candidate; clarify as "first three invalid, fourth valid → expect `None`."
- **[LOW] Plan 05-05 test-lines-count coverage caveat.** "Adding tests can only raise coverage" is not strictly true — `cargo llvm-cov` counts `#[cfg(test)]` lines, per Phase 4's own summary at `.planning/phases/04-domain-fetch-parse-coverage/04-02-SUMMARY.md:156`.

### Divergent Views

- **Overall risk.** Gemini: LOW across all five plans; Codex: LOW on 05-01 and 05-04, MEDIUM on 05-02, 05-03, 05-05. The 05-02 HIGH finding was independently verified against `src/geo.rs`; treat Codex's assessment as the operative one.
- **Test verbosity (05-01).** Codex suggests table-driven tests; Gemini praises the exhaustive per-arm layout. Style preference — no blocker either way.

### Recommendation

Address the codex concerns before executing (they map cleanly to plan edits, not code edits): fix the 05-02 fixtures, tighten the 05-03 `is_night_time` assertion, and pick one form for the 05-05 CI-yaml edit + drop the `resolve_headline_aqi` line from accepted-gaps. Then execute normally. Re-plan via `/gsd-plan-phase 5 --reviews` if you want the planner to fold these in automatically.
