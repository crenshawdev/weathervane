# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v0.9 — TechDebt

**Shipped:** 2026-07-01
**Phases:** 2 | **Plans:** 5 | **Sessions:** ~3

### What Was Built
- Security audit (Phase 1): aqicn token/URL leakage stripped from all five `tracing::debug!` sites via `e.without_url()`; ip-api.com coordinate range/NaN validation in `detect_location()`; panic-safe D-Bus deserialization in `network.rs`/`sleep.rs`; URL PII-leak sentinel assertion added to `tests/wire_contract.rs` (SEC-01..05).
- General faults pass (Phase 2): NaN-safe JMA station sort comparator plus observable debug-logged drop paths for malformed station entries; test-helper `.unwrap()` calls in `pollen.rs`/`time.rs` upgraded to `.expect()` with fixture context (FAULT-01..03).
- 8 source/test files changed (+544/−55); wire snapshot and public API surface unchanged.

### What Worked
- Narrow-restart scoping: cutting the original 5-phase tech-debt draft down to 2 tightly-scoped phases (security + panics) kept the milestone shippable in ~1 day.
- Every requirement had automated verification (`#[test]` fns) plus source-level grep checks — made the audit fast and unambiguous (8/8 clean).
- Wire-contract freeze held: no `cargo insta review` needed, CI stayed green throughout.

### What Was Inefficient
- Two SUMMARY.md files (01-02 SEC-03, 01-03 SEC-04) shipped with empty `one_liner` frontmatter, so the auto-generated MILESTONES.md missed those accomplishments and needed a manual backfill.
- REQUIREMENTS.md traceability checkboxes were never flipped during phase completion — all 8 stayed `[ ]` Pending until milestone close caught it.
- Nyquist VALIDATION.md files were reconstructed post-hoc (State B) rather than written during execution, forcing a re-audit to lift status from `tech_debt` → `passed`.

### Patterns Established
- **PII-safe logging convention** (`e.without_url()` at every leak site) replicated across `client.rs`, `location.rs`, `weather_jma.rs` — code-only debug logging, never lat/lon/token/URL values.
- **Shared closed-range coordinate predicate** `(-90.0..=90.0).contains(&lat) || (-180.0..=180.0).contains(&lon)` reused identically in `weather_jma.rs` and `location.rs`.
- **Fixture-context `.expect()` messages** naming the test + parsing step instead of opaque panics.

### Key Lessons
1. Fill SUMMARY `one_liner` frontmatter at plan close — the milestone accomplishment list is generated from it, and empty fields silently drop deliverables.
2. Flip REQUIREMENTS.md traceability checkboxes at phase verification, not milestone close, so mid-milestone status reflects reality.
3. Write Nyquist VALIDATION.md during execution, not as a post-hoc reconstruction — it avoids a second audit pass.

### Cost Observations
- Model mix: predominantly opus/sonnet for the hardening work; haiku for the integration-checker pass (~152s).
- Sessions: ~3 (execution, then a validate-phase run, then this resume + close).
- Notable: source untouched between the two audits — the re-audit was pure docs reconciliation, cheap to run.

---

## Milestone: v0.9.1 — Test Coverage Lift

**Shipped:** 2026-07-02
**Phases:** 3 | **Plans:** 11 | **Sessions:** ~4

### What Was Built
- Phase 3 (Alerts Parser Coverage): 3 private sync helpers extracted from `alerts.rs`
  (`nws_alerts_from_response`, `bom_alerts_from_response`, `meteoalarm_alerts_from_feed`);
  NWS/BOM/MeteoAlarm/ECCC fixture parser tests plus region dispatch and expiry-filter
  tests; `alerts.rs` line coverage 0% → 76.53%.
- Phase 4 (Domain Fetch/Parse Coverage): sync-helper extractions repeated across four
  modules — `resolve_current_temp`/`weather_from_open_meteo` (weather),
  `resolve_headline_aqi` (air_quality), `detected_from_ip_api` (location); `error.rs`
  covered zero-extraction via 17-test Display/From/PII-scrub module.
  Deltas: weather 0→89.08%, air_quality 10.26→88.39%, location 27.96→88.61%,
  error 22.22→94.39%.
- Phase 5 (Coverage Tails, ≥70% Gate & CI Enforcement): four parallel Wave-1 tail-lift
  plans (codes 62.37→99.52%, geo 70.13→94.49%, time 74.51→99.41%,
  weather_jma 71.95→87.70% via `select_temp_from_map` extraction); Wave-2 Plan 05-05
  measured workspace TOTAL at 85.20% then added `--fail-under-lines 75` to the GitLab
  `coverage` job as a single-line Form A edit.
- Milestone totals: workspace line coverage 48.72% → 85.20% (+36.48pp); CI gate at 75%;
  wire contract byte-identical throughout (`tests/wire_contract.rs` + snapshots
  unchanged); one source-level refactor (`select_temp_from_map` extraction in
  `weather_jma.rs`).

### What Worked
- **Sync-helper-extraction idiom scaled.** The Phase 3 pattern — extract an inner
  private sync `fn` from an async fetcher so it's coverable without a runtime — was
  reused verbatim in 04-01, 04-02, 04-03, and 05-04. Same shape, no new judgment
  calls, no wire impact.
- **Fixture-based tests, no mocks.** Every plan drove real serde/quick_xml decoders
  against inline JSON/XML strings — deterministic, fast, and immune to reqwest/mock
  library churn. No `wiremock` beyond what v0.9 already had.
- **Cross-AI plan review actually landed changes.** Phase 5 plans went through
  Claude/Codex/Gemini review before execution — surfaced the per-arm test style
  decision (05-01) and the branch-order coordinate correction (05-02) before writing
  any code.
- **Milestone bar met with 10.20pp margin.** ≥70% requirement, 75% gate landed —
  85.20% actual means normal drift can't accidentally trip CI.

### What Was Inefficient
- Original `--fail-under-lines 70` gate was raised to 75 mid-milestone (per D-04)
  because Wave-1 tails alone overshot 70% by a wide margin. Reasonable but caused a
  small plan revision loop.
- Some phase-summary `one_liner` frontmatter fields were still missing at plan close
  and had to be filled during milestone.complete extraction — same v0.9 lesson,
  didn't fully land as process.
- `weather_jma.rs` async HTTP surface (`fetch_stations`/`fetch_map`) and `geo.rs`
  `detect_country_from_coords` async path remain uncovered — accepted gaps per
  D-06/D-07 (out of scope for extract-and-test approach), but they persist as
  known coverage tails for a future milestone if the async testing surface
  changes.

### Patterns Established
- **Extract-then-test sync helpers as a milestone-standard technique.** Any async
  fetcher whose parse/decision logic is not trivially separable gets a private sync
  fn extraction; the async wrapper stays as thin glue. Idiom applied to 6 modules
  this milestone.
- **Debug-log preservation at extraction sites.** Every extracted helper preserved
  the original `tracing::debug!` call-site behavior exactly — required by the
  no-behavior-change constraint on wire-frozen code.
- **CI coverage gate ordered last, after margin proven.** Add the failing threshold
  only after coverage crosses the bar with headroom, so the same PR that raises the
  bar doesn't also break `main` if tails come in short.
- **Cross-AI plan review before Phase execution.** Phase 5 was the first phase to
  use `/gsd-review` + `/gsd-plan-review-convergence` before executing; caught real
  issues (per-arm vs table-driven test style, incorrect Brussels/Zurich coordinate
  claim) that inline plan-check would have missed.

### Key Lessons
1. **Sync-helper extraction is the coverage lever for async-fetch domains.** Don't
   try to test the async fetcher; test the pure fn inside it. Reused six times
   this milestone with identical shape.
2. **Fill SUMMARY `one_liner` at plan close, not milestone close.** Same v0.9
   lesson, still not fully landed as habit; a plan-completion checklist item would
   close it.
3. **Cross-AI plan review pays off on multi-plan phases.** Codex/Gemini caught real
   bugs in Phase 5 plans before execution. Consider running it on any phase with
   ≥3 parallel plans by default.
4. **Set the CI gate below your measured margin.** With 85.20% actual, gating at
   75% leaves headroom for normal drift while still catching real regressions.

### Cost Observations
- Model mix: predominantly opus/sonnet for plan authoring and cross-AI review;
  haiku for mechanical fixture-parse tests where the spec was complete.
- Sessions: ~4 (planning + Phase 3, Phase 4, Phase 5 execution, milestone close).
- Notable: Wave-1 of Phase 5 ran 4 plans in parallel (disjoint `files_modified`
  contract); wall-clock savings vs sequential was significant.

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Sessions | Phases | Key Change |
|-----------|----------|--------|------------|
| v0.9 | ~3 | 2 | First GSD-managed milestone; narrow-restart scoping proved out |
| v0.9.1 | ~4 | 3 | Cross-AI plan review adopted; Wave-based parallel plan execution proved out |

### Cumulative Quality

| Milestone | Tests | Coverage | Zero-Dep Additions |
|-----------|-------|----------|-------------------|
| v0.9 | 83 (61 unit + 22 wire) | tracked via llvm-cov | 0 (test-only deps: wiremock, tracing-test) |
| v0.9.1 | +~140 fixture tests across 10 modules | 48.72% → 85.20% workspace, CI gated at 75% | 0 |

### Top Lessons (Verified Across Milestones)

1. **Fill SUMMARY `one_liner` at plan close.** Flagged in v0.9, recurred in v0.9.1.
   Milestone accomplishment lists are generated from these fields — empty frontmatter
   silently drops deliverables. Two-milestone pattern.
2. **Narrow-restart scoping.** v0.9 cut a 5-phase draft to 2; v0.9.1 shipped 3 tightly
   scoped phases in 2 days. Small-milestone rhythm is the default that works here.
3. **Sync-helper extraction for async-fetch coverage.** Proven on 6 modules in v0.9.1.
   Any coverage lift in the async-domain layer should default to this pattern.
