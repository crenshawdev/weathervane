# Phase 5: Coverage Tails, ≥70% Gate & CI Enforcement - Context

**Gathered:** 2026-07-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Close the remaining line-coverage tails on `codes.rs`, `geo.rs`, `time.rs`, and
`weather_jma.rs`, verify workspace line coverage is ≥70% (currently 77.87% per
`cargo llvm-cov --workspace` on 2026-07-02), and lock the bar into GitLab CI so
coverage cannot silently regress. Milestone-completion gate for v0.9.1.

**Coverage baseline (2026-07-02, `cargo llvm-cov --workspace --summary-only`):**

| Module          | Lines cov | Uncovered |
|-----------------|-----------|-----------|
| codes.rs        | 62.37%    | 35        |
| geo.rs          | 70.13%    | 92        |
| time.rs         | 74.51%    | 26        |
| weather_jma.rs  | 71.95%    | 69        |
| **Workspace**   | **77.87%**| 648       |

</domain>

<decisions>
## Implementation Decisions

### CI threshold (COV-08)
- **D-01:** CI gate uses `cargo llvm-cov --fail-under-lines 75`. Value chosen for a
  ~3-point buffer below current 77.87% — catches real regressions without blocking
  legitimate PRs that add a small amount of uncovered code. Milestone spec (COV-07)
  is 70%; gate sits above it deliberately.
- **D-02:** Gate metric is **lines** only. `--fail-under-lines` matches COV-07's
  wording ("workspace line coverage ≥70%"). No `--fail-under-regions` or
  `--fail-under-functions` — one metric, one signal.

### Per-module targets (COV-06)
- **D-03:** Workspace total only for the CI gate. No per-module CI floors.
  Per-module lifts are guidance for the planner; `codes.rs` (biggest gap at
  62.37%) gets the most attention. No custom scripting for per-module thresholds.

### CI gate placement (COV-08)
- **D-04:** `--fail-under-lines 75` goes **inline in the existing `coverage` job**
  in `.gitlab-ci.yml` (currently `.gitlab-ci.yml:67-90`). Single job, single
  fail signal, keeps the Cobertura MR-widget behavior. Gate is added **only
  after** the tails are closed and workspace coverage still comfortably clears 75%.
- **D-05:** Job order stays: `cargo llvm-cov --no-report --workspace` → cobertura
  export → `report --summary-only`. Add a final `report --fail-under-lines 75`
  step (or fold the flag into the summary step) so failure is deterministic and
  visible in job logs.

### Tail-closing scope (COV-06)
- **D-06:** Tail closing is **limited to the four modules named in COV-06**:
  `codes.rs`, `geo.rs`, `time.rs`, `weather_jma.rs`. No new tests for
  `network.rs` or `sleep.rs` — the D-Bus streams already carry the CLAUDE.md
  "silent degrade on non-Linux" contract, and driving zbus signals through
  fixtures is out of scope for a coverage-lift phase.
- **D-07:** Test rules match Phase 3/4: tests target **extracted parse/logic
  helpers**, not live HTTP or live D-Bus. `tests/wire_contract.rs` and
  `tests/snapshots/` are **frozen** — any snapshot delta blocks the plan.
- **D-08:** No per-module numeric target is a hard acceptance criterion. Each
  plan states which specific uncovered branches/lines it targets and why they're
  worth covering (branch coverage, PII-scrub, unit conversions, etc.).

### Claude's Discretion
- Which specific helpers to extract from the four target modules (mirroring
  the Phase 3/4 pattern of pulling parse-only sync helpers out of async
  wrappers).
- Whether to split Phase 5 into 2 or 3 plans (Phase 3 used 2, Phase 4 used 4).
- Exact wording of the CI step name and any comment lines added in `.gitlab-ci.yml`.
- Whether COV-07 needs a distinct "verify ≥70%" checkpoint plan or is proven
  transitively when the fail-under-lines gate lands green.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project contracts (frozen behaviors)
- `CLAUDE.md` — no i18n, silent regional fallthrough, no PII in errors/tracing,
  Linux-only streams degrade silently. Tail tests must not violate these.
- `.claude/CLAUDE.md` — reactive stateless design, wire contract stability,
  Linux-only platform notes.
- `CONTRACT.md` — public wire contract (types crossing process boundaries).
- `tests/wire_contract.rs` — frozen JSON shapes (insta snapshots).
- `tests/snapshots/` — snapshot store; must not change in this phase.

### Requirements & roadmap
- `.planning/REQUIREMENTS.md` §Coverage / §CI — COV-06, COV-07, COV-08.
- `.planning/ROADMAP.md` §Phase 5 — goal + Success Criteria list.

### CI target file
- `.gitlab-ci.yml` §coverage (lines 67-90) — the job that gets the
  `--fail-under-lines 75` flag added.

### Prior-phase patterns to mirror
- `.planning/phases/03-alerts-parser-coverage/03-01-PLAN.md` — sync-helper
  extraction pattern (async wrappers → pure parse fns → unit tests on the pure fn).
- `.planning/phases/03-alerts-parser-coverage/03-02-PLAN.md` — XML/JSON fixture
  layout, region dispatch tests, coverage verification step.
- `.planning/phases/04-domain-fetch-parse-coverage/04-*-PLAN.md` — the four
  Phase 4 plans exemplify the pattern for weather/AQI/location/error tail closing.

### Tooling reference
- `cargo llvm-cov --help` — `--fail-under-lines <N>` documented flag. Confirm
  invocation shape at plan time; no research-only lookup needed yet.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Existing coverage job** (`.gitlab-ci.yml:67-90`): already installs
  `cargo-llvm-cov`, runs `--workspace`, emits Cobertura, and prints
  `report --summary-only`. Only missing piece is `--fail-under-lines`.
- **Sync-helper extraction pattern** established in Phases 3 and 4: pull pure
  parse/convert logic out of async fetch functions, unit test the pure fn on
  fixtures. Applies directly to `weather_jma.rs` and any remaining tails.
- **Fixture layout** from Phase 3/4: `tests/fixtures/<provider>/` with `.json`
  or `.xml` snapshots of upstream API responses.

### Established Patterns
- **No live HTTP, no live D-Bus in tests.** All fetches funnel through
  `src/client.rs` and are only invoked in bin-time paths; parse/logic is
  tested directly on fixture strings/JSON values.
- **`Result<T> = std::result::Result<T, Error>`** everywhere; error paths are
  covered via `From<...>` conversions and explicit error-shape tests
  (Phase 4 pattern in `error.rs`).
- **Wire contract stability** enforced by `tests/wire_contract.rs` insta
  snapshots. Coverage tests must not touch these files.

### Integration Points
- New tests live in the existing per-module `#[cfg(test)] mod tests` blocks,
  matching Phase 3/4 style. No new test crates or top-level test files.
- CI change is a one-file edit to `.gitlab-ci.yml`, in the existing
  `coverage` job — no new job, no new stage.

</code_context>

<specifics>
## Specific Ideas

- `codes.rs` at 62.37% is the biggest gap and probably the cheapest lift —
  it's a WMO code → `WeatherCondition` mapping; missing branches are likely
  code ranges that no test currently exercises. Start there.
- `geo.rs` has 92 uncovered lines — probably region-boundary/polygon-lookup
  branches that Phase 3 didn't exercise (Phase 3 covered *alert* dispatch,
  not the underlying `geo.rs` helpers exhaustively).
- `time.rs` and `weather_jma.rs` are already >70%; small targeted tests
  should be enough.
- CI change is intentionally the *last* task in the phase — added only
  after the workspace still clears 75% with the new tests in.

</specifics>

<deferred>
## Deferred Ideas

- Raising `network.rs` / `sleep.rs` coverage (D-Bus stream handlers, 26.74% /
  30.77%). Would need a zbus test harness or extracted parsers around
  `NM_STATE_*` constants; deferred as a future hardening phase, not blocking
  v0.9.1 completion.
- Per-module CI floors (custom scripting on `cargo llvm-cov report --json`
  output). Not needed to meet COV-06/07/08; revisit if regressions land in
  a single module without moving the workspace total.
- Region-coverage gate (`--fail-under-regions`). Currently 74.28%; would
  need extra tests to safely enable. Deferred.

</deferred>

---

*Phase: 05-coverage-tails-70-gate-ci-enforcement*
*Context gathered: 2026-07-02*
