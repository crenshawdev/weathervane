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

### Active

<!-- Current scope. Building toward these. -->

- [ ] Tech debt audit pass: clippy lint cleanup, dead-code removal, panic-path review
- [ ] Security audit pass: input parsing, HTTP error handling, PII leak surfaces,
      version-pin audit (cargo-audit / cargo-deny)
- [ ] Hardening for v1.0 release: stabilize public API, lock wire-contract version,
      confirm Linux-only graceful-degradation guarantees

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
*Last updated: 2026-06-30 after initialization*
