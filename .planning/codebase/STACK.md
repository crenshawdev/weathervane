# Technology Stack

**Analysis Date:** 2026-06-30

## Languages

**Primary:**
- Rust 2021 edition - Entire codebase (async reactive library)

## Runtime

**Environment:**
- Tokio async runtime (via zbus dependency)
- No polling loops, no timers - reactive only

**Package Manager:**
- Cargo - Rust package manager
- Lockfile: Present (`Cargo.lock` in repo)

## Frameworks

**Core:**
- No traditional framework; pure async library design
- `async_stream` 0.3 - Async generator macros for streams

**Testing:**
- `insta` 1.x - Snapshot testing (for wire contract validation)
- Built-in `#[cfg(test)]` modules throughout codebase

**Build/Dev:**
- `cargo fmt` - Code formatting
- `cargo clippy` - Linting with strict warnings-as-errors
- `cargo-llvm-cov` - Code coverage reporting

## Key Dependencies

**Critical:**
- `reqwest` 0.12 (with `json` feature) - HTTP client for all external API calls; pooled connection management with 15s timeout per request
- `serde` 1.0 + `serde_json` 1.0 (with derive) - Serialization/deserialization for all wire types; frozen by `tests/wire_contract.rs` (insta snapshots)
- `zbus` 5 (tokio feature, no default-features) - D-Bus communication for NetworkManager and systemd-logind event streams
- `thiserror` 2 - Error type derivation and context propagation

**Data & Time:**
- `chrono` 0.4 (with serde feature) - ISO 8601 timestamp parsing and timezone handling
- `quick-xml` 0.37 (with serialize feature) - XML parsing for alert feeds (MeteoAlarm, BOM)

**Utilities:**
- `futures` 0.3 - Stream and combinator traits
- `urlencoding` 2.1 - Query parameter encoding for API URLs (aqicn token, geocoding queries)
- `tracing` 0.1 - Structured logging (debug-level for optional provider fallbacks, info/warn for failures)

## Configuration

**Environment:**
- No `.env` support; caller (frontend) passes all parameters (coordinates, temperature unit, measurement system, aqicn token)
- No persistent config; stateless per-call design

**Build:**
- Single `Cargo.toml` workspace
- No feature gates or optional compilation (all integrations built in)
- Edition: 2021
- Minimum platform: Linux (D-Bus services), degrades gracefully on non-Linux

## Platform Requirements

**Development:**
- Rust 1.70+ (exact MSRV not locked; inferred from Edition 2021 and dependency versions)
- Build deps: libssl-dev, pkg-config (for OpenSSL via reqwest, shown in CI)
- Linux host for NetworkManager/systemd-logind development (streams fail gracefully elsewhere)

**Production:**
- Linux for full feature set (network monitoring via D-Bus, suspend/resume events)
- Non-Linux systems: weather/AQI/alerts/pollen functions work, D-Bus streams return idle (no events)
- No runtime config files required

---

*Stack analysis: 2026-06-30*
