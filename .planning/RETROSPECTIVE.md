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

## Cross-Milestone Trends

### Process Evolution

| Milestone | Sessions | Phases | Key Change |
|-----------|----------|--------|------------|
| v0.9 | ~3 | 2 | First GSD-managed milestone; narrow-restart scoping proved out |

### Cumulative Quality

| Milestone | Tests | Coverage | Zero-Dep Additions |
|-----------|-------|----------|-------------------|
| v0.9 | 83 (61 unit + 22 wire) | tracked via llvm-cov | 0 (test-only deps: wiremock, tracing-test) |

### Top Lessons (Verified Across Milestones)

1. (pending second milestone to cross-validate)
