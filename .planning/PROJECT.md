# weathervane

## What This Is

Reactive Rust domain crate for weather, air quality, alerts, and pollen data from
public APIs. Async functions in, clean Rust types out — no UI, no polling, no timers,
no config storage. Consumed by atmos and cosmic-ext-applet-tempest; published to crates.io.

## Core Value

Public types crossing process boundaries are wire-stable: serialized shapes don't
silently change between releases. Tests (wire_contract.rs insta snapshots) prove it
each commit, and the four CLAUDE.md contracts — no i18n, silent regional fallthrough,
no PII in errors/tracing, Linux-only streams degrade silently — hold across every code
path.

## Current State

**Shipped:** v0.9.1 Test Coverage Lift (2026-07-02) — workspace line coverage
lifted 48.72% → 85.20% (measured via `cargo llvm-cov`), with a `--fail-under-lines 75`
CI gate now enforcing the floor. 3 phases, 11 plans, 8 requirements (COV-01..08),
all satisfied and verified (8/8). Wire contract byte-identical across the whole
milestone. Only source-level change: `select_temp_from_map` extracted from
`override_current_temp` in `src/weather_jma.rs` to make it testable without an
async runtime.

**Prior:** v0.9 TechDebt (2026-07-01) — eliminated token/URL leakage from logs
and removed reachable panics from production code paths.

## Next Milestone: TBD

v0.9.1 shipped 2026-07-02. Next milestone not yet defined. Run
`/gsd-new-milestone` to scope and plan it. Immediate release-facing follow-up
before the next milestone: publish the crate to crates.io (README/CHANGELOG
refresh + `cargo publish` via the `release-prep` skill).

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

- ✓ Fetch current/hourly/daily weather from Open-Meteo — shipped (v0.x)
- ✓ JMA AMeDAS override for Japan current temperature — shipped (v0.x)
- ✓ Air quality with aqicn token (non-Europe) + Open-Meteo pollutants/EU AQI — shipped
- ✓ Pollen from Open-Meteo CAMS (Europe only, 6 species) — shipped
- ✓ Region-routed alerts: NWS / ECCC / MeteoAlarm / BOM — shipped
- ✓ City search and reverse geocoding via Open-Meteo — shipped
- ✓ Imperial/Metric measurement system with display labels — shipped (v0.8.0)
- ✓ AqiSource enum recording which provider supplied AQI — shipped (v0.8.0)
- ✓ Wire contract frozen in CONTRACT.md, enforced by insta snapshots — shipped
- ✓ PII guard: error payloads omit query text and coordinates — enforced by tests
- ✓ CI pipeline: fmt, clippy -D warnings, build, test, llvm-cov coverage — shipped
- ✓ Security audit: aqicn-token/URL leakage stripped from logs, ip-api.com coordinate
      validation, panic-safe D-Bus deserialization, URL PII-leak assertion in CI — v0.9
- ✓ General faults pass: reachable JMA panics replaced with NaN-safe/observable paths;
      test-helper unwraps carry fixture context — v0.9
- ✓ Test coverage lifted to production-grade with a CI floor — v0.9.1:
    - `alerts.rs` covered (COV-01) — NWS/MeteoAlarm/ECCC/BOM parsers, expiry filter, region dispatch
    - `weather.rs` covered (COV-02) — Open-Meteo parse → WeatherData, JMA override path
    - `air_quality.rs` covered (COV-03) — AQI parse, AQICN headline merge, source tracking
    - `location.rs` covered (COV-04) — geocoding, IP geolocation, saved-location matching
    - `error.rs` covered (COV-05) — From impls, Display output, WireError PII-scrub
    - Coverage tails lifted (COV-06) — `codes.rs` 62.37→99.52%, `geo.rs` 70.13→94.49%,
      `time.rs` 74.51→99.41%, `weather_jma.rs` 71.95→87.70%
    - Workspace line coverage ≥70% verified (COV-07) — measured 85.20% via `cargo llvm-cov`
    - CI coverage gate at 75% (COV-08) — `.gitlab-ci.yml` `coverage` job enforces
      `--fail-under-lines 75`

### Active

<!-- Current scope. Building toward these. -->

_(No active requirements — v0.9.1 milestone complete. Next milestone TBD.)_

### Out of Scope

<!-- Explicit boundaries. Includes reasoning to prevent re-adding. -->

- UI components or rendering — frontends own all presentation
- Polling, timers, or background fetch schedules — caller owns scheduling
- Config storage or secret management — caller owns persistence
- Internationalization (i18n) — strings remain English per CLAUDE.md contract
- Non-Linux platform support — Linux-only crate; other platforms may degrade silently
- South Korea (KMA) alerts — national-ID-gated API not viable for open source

## Context

- **Consumers**: atmos (TUI/CLI tool) and cosmic-ext-applet-tempest (COSMIC desktop applet).
- **Identifier namespace**: new app surfaces use `dev.jcrenshaw.*` reverse-DNS;
  tempest keeps its existing identifiers (baked into AUR installs).
- **CI gates** (must match): `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`,
  `cargo build --workspace`, `cargo test --workspace`.
- **Wire contract enforcement**: changes to public serde types must be intentional —
  `cargo insta review` accepts them; otherwise `tests/wire_contract.rs` fails.
- **Codebase map**: `.planning/codebase/` (7 docs, 1770 lines, generated 2026-06-30).

## Constraints

- **Tech stack**: Rust async (tokio runtime in consumers; crate stays runtime-agnostic).
  serde for wire types. reqwest for HTTP. insta for snapshot tests.
- **Compatibility**: serialized payloads from older crate versions must continue to
  deserialize (additive evolution; `#[serde(default)]` on new fields).
- **Privacy**: no PII (coordinates, search queries) in error payloads or tracing output.
- **Platform**: Linux-only; Windows/macOS streams degrade silently per CLAUDE.md contract.

## Key Decisions

<!-- Decisions that constrain future work. Add throughout project lifecycle. -->

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| No UI / no polling / no config in crate | Frontends differ; crate stays pure domain logic | ✓ Good |
| Wire contract frozen with insta snapshots | Catch accidental breakage in CI | ✓ Good |
| Japan: JMA AMeDAS current-temp override | Open-Meteo runs cold against Japan ground truth | ✓ Good |
| Europe stays on Open-Meteo AQI scale | Preserves EU AQI categories users expect | ✓ Good |
| aqicn only outside Europe with explicit token | License terms (free-software/non-commercial) | ✓ Good |
| English-only / no i18n | Crate scope; consumers own localization | ✓ Good |
| `dev.jcrenshaw` namespace for new surfaces | Reverse-DNS of personal domain; retires vintagetechie | ✓ Good |
| v0.9 narrowed to Security + General Faults (2 phases) | Prior 5-phase tech-debt draft too broad; ship a tight security/panic pass first | ✓ Good |
| No wire-crossing serde shape changes in v0.9 | Keep `tests/wire_contract.rs` green without `cargo insta review` | ✓ Good |
| SEC-02 log-line assertions in `src/client.rs` not `wire_contract.rs` | Security goal met at the leak site; accepted override (01-VERIFICATION.md) | ✓ Good |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-07-02 after v0.9.1 milestone close*
