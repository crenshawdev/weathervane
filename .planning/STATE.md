---
gsd_state_version: 1.0
milestone: v0.9
milestone_name: TechDebt
status: planning
last_updated: "2026-06-30T17:17:18.605Z"
last_activity: 2026-06-30
progress:
  total_phases: 2
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-06-30)

**Core value:** Public types crossing process boundaries are wire-stable, and the four CLAUDE.md contracts (no i18n, silent regional fallthrough, no PII in errors/tracing, Linux-only streams degrade silently) hold across every code path.
**Current focus:** Phase 1 — Security Audit

## Current Position

Phase: Not started (roadmap drafted)
Plan: —
Status: Awaiting Phase 1 planning
Last activity: 2026-06-30 — Narrow two-phase TechDebt roadmap written

## Performance Metrics

**Velocity:**

- Total plans completed: 0
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**

- Last 5 plans: none yet
- Trend: -

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Milestone narrowed to two phases: Security Audit, then General Faults Pass. Previous 5-phase tech-debt draft (dependency hygiene, test coverage, public surface) is abandoned, not shipped; surface items individually in a future milestone if needed.
- No wire-crossing serde shape changes planned in this milestone — `tests/wire_contract.rs` stays green without `cargo insta review` unless a new PII-leak assertion intentionally extends the snapshot set.

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Deferred Items

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| DEPS | cargo-audit in CI (DEPS-01) | Future milestone | 2026-06-30 |
| DEPS | cargo-deny in CI (DEPS-02) | Future milestone | 2026-06-30 |
| DEPS | MeteoAlarm codename CDN cache (DEPS-03) | Future milestone | 2026-06-30 |
| DEPS | Nominatim reverse-geocode cache (DEPS-04) | Future milestone | 2026-06-30 |
| TEST | point_in_polygon fixtures (TEST-01) | Future milestone | 2026-06-30 |
| TEST | Region-detection boundary tests (TEST-02) | Future milestone | 2026-06-30 |
| TEST | Silent-default fallback tests (TEST-03) | Future milestone | 2026-06-30 |
| TEST | BOM warning_group_type tests (TEST-04) | Future milestone | 2026-06-30 |
| API | Public-item documentation gate (API-01) | Future milestone | 2026-06-30 |
| API | CONTRACT.md v1 snapshot coverage (API-02) | Future milestone | 2026-06-30 |
| API | D-Bus degradation contract docs (API-03) | Future milestone | 2026-06-30 |
| API | v0.8.0 → v1.0 migration changelog (API-04) | Future milestone | 2026-06-30 |
| PERF | AMeDAS haversine pre-sort (PERF-01) | Future milestone | 2026-06-30 |
| PERF | MeteoAlarm polygon parse+cache (PERF-02) | Future milestone | 2026-06-30 |
| PERF | ECCC parallel HTML fetches (PERF-03) | Future milestone | 2026-06-30 |
| RES | Retry-with-backoff (RES-01) | Future milestone | 2026-06-30 |
| RES | Connection pool tuning (RES-02) | Future milestone | 2026-06-30 |
| RES | HTTP ETag/If-Modified-Since (RES-03) | Future milestone | 2026-06-30 |

## Session Continuity

Last session: 2026-06-30
Stopped at: Narrow two-phase ROADMAP.md written; STATE.md and REQUIREMENTS.md traceability updated. Ready to plan Phase 1 — Security Audit.
Resume file: None
