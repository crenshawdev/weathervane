# Roadmap: weathervane — TechDebt (v0.9)

## Overview

Narrow two-phase milestone. Phase 1 eliminates token/URL leakage and the
reachable D-Bus panics flagged by the security audit. Phase 2 sweeps the
remaining reachable `.unwrap()` / `partial_cmp` panics in production code
paths and gives test-helper unwraps fixture context. Anything outside these
two buckets is deferred to a future milestone per REQUIREMENTS.md.

## Phases

- [x] **Phase 1: Security Audit** - Strip tokens and URLs from logs, validate external inputs, and make D-Bus deserialization panic-safe (completed 2026-06-30)
- [ ] **Phase 2: General Faults Pass** - Replace reachable production panics with safe alternatives and give test-helper unwraps fixture context

## Phase Details

### Phase 1: Security Audit

**Goal**: No API token, raw URL, or unvalidated external coordinate reaches any log line, error payload, or D-Bus stream output produced by the crate
**Depends on**: Nothing (first phase)
**Requirements**: SEC-01, SEC-02, SEC-03, SEC-04, SEC-05
**Success Criteria** (what must be TRUE):

  1. URLs emitted from `client.rs` (lines 72, 75, 95, 99) and `air_quality_aqicn.rs` (lines 19-30) are sanitized before reaching tracing or error output; the aqicn token and any `api_key`-style query parameter never appear in a debug or error log line during normal or error operation (SEC-01)
  2. `tests/wire_contract.rs` gains assertions that fail CI if either an aqicn token substring or an unsanitized URL with query parameters appears in any log line produced by the crate under tested error and success paths (SEC-02, SEC-05)
  3. `detect_location()` (`location.rs:100-127`) returns a typed `Error::LocationDetection` when an ip-api.com response carries `lat` outside `[-90, 90]` or `lon` outside `[-180, 180]`; a unit test covers the reject path and confirms no out-of-range coordinate is returned
  4. A malformed D-Bus message delivered to `network_stream()` (`network.rs:67`) or `sleep_stream()` (`sleep.rs:65`) produces a `debug!` log entry and the stream continues without panicking; a test feeding a malformed signal exercises both paths
  5. All four CI gates pass: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo build --workspace`, `cargo test --workspace`

**Plans**: 3/3 plans complete

- [x] 01-01-PLAN.md — URL/token leak hardening: sanitize_url + e.without_url() in client.rs, wiremock+tracing-test leak harness, sentinel assertion in wire_contract.rs (SEC-01, SEC-02, SEC-05)
- [x] 01-02-PLAN.md — Coord validation in detect_location() rejecting out-of-range/NaN/infinite ip-api.com responses with Error::LocationDetection (SEC-03)
- [x] 01-03-PLAN.md — D-Bus stream resilience: decode helpers + explicit-match loops in network.rs and sleep.rs, observable at debug level (SEC-04)

**Cross-cutting constraints:**

- All four CI gates pass (cargo fmt --check, cargo clippy --workspace -- -D warnings, cargo build --workspace, cargo test --workspace)

### Phase 2: General Faults Pass

**Goal**: No reachable panic remains in production code paths flagged by the audit, and every test-helper `.unwrap()` carries fixture context on failure
**Depends on**: Phase 1
**Requirements**: FAULT-01, FAULT-02, FAULT-03
**Success Criteria** (what must be TRUE):

  1. The `partial_cmp().unwrap()` at `weather_jma.rs:276` is replaced with `unwrap_or(Ordering::Equal)` or `total_cmp()`; a unit test feeding NaN station coordinates completes the sort without panicking (FAULT-01)
  2. JMA station parsing in `weather_jma.rs:103-129` emits a `debug!` log naming the dropped station when `s.lat` / `s.lon` array lengths are wrong or values fall outside `[-90, 90]` / `[-180, 180]`, and only well-formed stations reach the haversine pass (FAULT-02)
  3. Every `.unwrap()` in the `pollen.rs:108,124,142,158,173` and `time.rs:141` test helpers is replaced with `.expect("…")` carrying the fixture name; a deliberately corrupted fixture produces an actionable failure message rather than an opaque panic (FAULT-03)
  4. All four CI gates pass: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo build --workspace`, `cargo test --workspace`

**Plans**: 2 plans

- [ ] 02-01-PLAN.md — weather_jma.rs NaN-safe sort comparator + tracing::debug! + range validation in fetch_stations() (FAULT-01, FAULT-02)
- [ ] 02-02-PLAN.md — Test-helper .expect() fixture context in pollen.rs and time.rs (FAULT-03)

**Cross-cutting constraints:**

- All four CI gates pass (cargo fmt --check, cargo clippy --workspace -- -D warnings, cargo build --workspace, cargo test --workspace)

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Security Audit | 3/3 | Complete   | 2026-06-30 |
| 2. General Faults Pass | 0/2 | Not started | - |
