# Roadmap: weathervane

## Milestones

- ✅ **v0.9 TechDebt** — Phases 1-2 (shipped 2026-07-01)
- ✅ **v0.9.1 Test Coverage Lift** — Phases 3-5 (shipped 2026-07-02)
- 📋 **vNext** — TBD

## Phases

<details>
<summary>✅ v0.9 TechDebt (Phases 1-2) — SHIPPED 2026-07-01</summary>

Narrow two-phase milestone: eliminate token/URL leakage and reachable panics
from production code paths. Full detail archived at
`milestones/v0.9-ROADMAP.md`.

- [x] Phase 1: Security Audit (3/3 plans) — completed 2026-06-30 — SEC-01..05
- [x] Phase 2: General Faults Pass (2/2 plans) — completed 2026-06-30 — FAULT-01..03

</details>

<details>
<summary>✅ v0.9.1 Test Coverage Lift (Phases 3-5) — SHIPPED 2026-07-02</summary>

Workspace line coverage lifted 48.72% → 85.20% via extracted sync helpers and
fixture-based tests; `--fail-under-lines 75` CI gate landed in `.gitlab-ci.yml`.
Wire contract byte-identical across the milestone. Full detail archived at
`milestones/v0.9.1-ROADMAP.md`.

- [x] Phase 3: Alerts Parser Coverage (2/2 plans) — completed 2026-07-01 — COV-01
- [x] Phase 4: Domain Fetch/Parse Coverage (4/4 plans) — completed 2026-07-02 — COV-02..05
- [x] Phase 5: Coverage Tails, ≥70% Gate & CI Enforcement (5/5 plans) — completed 2026-07-02 — COV-06..08

</details>

### 📋 vNext (Planned)

_(No phases planned yet — run `/gsd-new-milestone` to start the next milestone.)_

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Security Audit | v0.9 | 3/3 | Complete | 2026-06-30 |
| 2. General Faults Pass | v0.9 | 2/2 | Complete | 2026-06-30 |
| 3. Alerts Parser Coverage | v0.9.1 | 2/2 | Complete | 2026-07-01 |
| 4. Domain Fetch/Parse Coverage | v0.9.1 | 4/4 | Complete | 2026-07-02 |
| 5. Coverage Tails, ≥70% Gate & CI Enforcement | v0.9.1 | 5/5 | Complete | 2026-07-02 |
