---
created: 2026-07-01T18:18:58.321Z
title: Lift workspace test coverage to ≥70%
area: testing
files:
  - src/alerts.rs
  - src/weather.rs
  - src/air_quality.rs
  - src/location.rs
  - src/error.rs
  - src/weather_jma.rs
  - src/time.rs
  - src/codes.rs
  - src/geo.rs
---

## Problem

Workspace line coverage is **48.72%** (49.85% region), measured 2026-07-01 via
`cargo llvm-cov --workspace --summary-only`. cargo-llvm-cov 0.8.7 is now installed
(binary had missing execute bit; fixed with chmod +x on ~/.cargo/bin/cargo-llvm-cov).
61 unit tests + 22 wire-contract tests pass, but large surfaces are untested.

Coverage by module (regions / lines), worst first:

| Module            | Regions | Lines  | Uncovered lines |
|-------------------|---------|--------|-----------------|
| alerts.rs         | 0.00%   | 0.00%  | 394             |
| weather.rs        | 0.00%   | 0.00%  | 78              |
| air_quality.rs    | 8.11%   | 10.26% | 70              |
| location.rs       | 23.46%  | 27.96% | 67              |
| error.rs          | 26.09%  | 22.22% | 28              |
| geo.rs            | 65.13%  | 70.13% | 92              |
| time.rs           | 66.67%  | 74.51% | 26              |
| weather_jma.rs    | 68.31%  | 71.95% | 69              |
| codes.rs          | 69.67%  | 62.37% | 35              |

`alerts.rs` (394 uncovered lines) is the single largest gap and the highest-value
target: regional dispatch (NWS/MeteoAlarm/ECCC/BOM), XML/JSON parsing, expiry filtering.

## Solution

Target ≥70% workspace line coverage. Priority order by ROI:

1. **alerts.rs** — parse fixtures for each regional provider (NWS JSON, MeteoAlarm XML,
   ECCC, BOM), expired-alert filtering, region dispatch. Biggest single win.
2. **weather.rs** — Open-Meteo response parsing → WeatherData, JMA override path.
3. **air_quality.rs** — Open-Meteo AQI parse, AQICN headline merge, source tracking.
4. **location.rs** — geocoding parse, IP geolocation parse, saved-location matching.
5. **error.rs** — From impls, Display output, WireError PII-scrub paths.
6. Fill tails on geo.rs / time.rs / weather_jma.rs / codes.rs.

Constraints:
- Wire contract must stay unchanged — `tests/wire_contract.rs` + snapshots frozen.
- Follow project test conventions (in-module `#[cfg(test)]`, descriptive snake_case names).
- Network calls: test the parse/logic helpers (already extracted, e.g. `from_current`),
  not live HTTP. Add fixtures where response parsing isn't yet separable.

Follow-up (separate todo): add a CI coverage gate via cargo-llvm-cov once the repo has
a CI workflow (none exists today — no .github/workflows/).
