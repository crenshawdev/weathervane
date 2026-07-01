---
phase: 01
slug: security-audit
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-01
---

# Phase 01 — Validation Strategy

> Retroactive validation reconstruction. Phase already executed and verified;
> this file records the automated coverage that closes each SEC-* requirement.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (built-in Rust test runner) + `insta` snapshots + `wiremock` + `tracing-test` (dev-deps) |
| **Config file** | Cargo.toml `[dev-dependencies]` (no separate test config) |
| **Quick run command** | `cargo test --workspace --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~5 seconds (61 unit + 22 wire contract tests, per Phase 02 VERIFICATION run) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --workspace --lib` (unit tests only)
- **After every plan wave:** Run `cargo test --workspace` (adds wire contract)
- **Before `/gsd-verify-work`:** Full suite + all 4 CI gates (`cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo build --workspace`, `cargo test --workspace`) must exit 0
- **Max feedback latency:** ~5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | SEC-01 | T-01-01 | `sanitize_url()` strips query strings; URL replaced by `?[redacted]` in output | unit | `cargo test --workspace --lib sanitize_url_strips_sentinel_from_query_string` | ✅ src/client.rs:131 | ✅ green |
| 01-01-02a | 01 | 1 | SEC-01, SEC-02 | T-01-01 | aqicn token absent from tracing output on `get_json` request-failed path | integration | `cargo test --workspace --lib aqicn_token_does_not_leak_on_connect_failure` | ✅ src/client.rs:149 | ✅ green |
| 01-01-02b | 01 | 1 | SEC-01, SEC-02 | T-01-01 | aqicn token absent from tracing output on `get_json` status-error path | integration | `cargo test --workspace --lib aqicn_token_does_not_leak_on_status_error` | ✅ src/client.rs:165 | ✅ green |
| 01-01-02c | 01 | 1 | SEC-01, SEC-02 | T-01-01 | aqicn token absent from tracing output on `get_text` body-failed path | integration | `cargo test --workspace --lib aqicn_token_does_not_leak_on_body_failure` | ✅ src/client.rs:189 | ✅ green |
| 01-01-02d | 01 | 1 | SEC-01, SEC-02 | T-01-01 | aqicn token absent from tracing output on `get_json` parse-failed path | integration | `cargo test --workspace --lib aqicn_token_does_not_leak_on_parse_failure` | ✅ src/client.rs:221 | ✅ green |
| 01-01-02e | 01 | 1 | SEC-01, SEC-02 | T-01-01 | aqicn token absent from tracing output on 200 happy path (regression guard for future info-level "fetched {url}" additions) | integration | `cargo test --workspace --lib aqicn_token_does_not_leak_on_success` | ✅ src/client.rs:247 | ✅ green |
| 01-01-03 | 01 | 1 | SEC-05 | T-01-02 | WireError serialization contains no aqicn token sentinel across `wire_error_exemplars()` | wire-contract | `cargo test --workspace --test wire_contract wire_error_never_leaks_aqicn_sentinel` | ✅ tests/wire_contract.rs:407 | ✅ green |
| 01-02-01a | 02 | 1 | SEC-03 | T-02-01, T-02-02 | `(-90.0..=90.0).contains(&f64::NAN)` returns `false` (NaN pin — future flipped predicate `lat <= 90.0 && lat >= -90.0` would fail) | unit | `cargo test --workspace --lib nan_coords_are_rejected` | ✅ src/location.rs:177 | ✅ green |
| 01-02-01b | 02 | 1 | SEC-03 | T-02-01 | Range-contains returns `false` for `f64::INFINITY` and `f64::NEG_INFINITY` on lat and lon | unit | `cargo test --workspace --lib infinite_coords_are_rejected` | ✅ src/location.rs:190 | ✅ green |
| 01-02-01c | 02 | 1 | SEC-03 | T-02-01 | Out-of-range values (91.0, 181.0) rejected; valid anchor (45.5) accepted | unit | `cargo test --workspace --lib out_of_range_coords_are_rejected` | ✅ src/location.rs:211 | ✅ green |
| 01-03-01a | 03 | 1 | SEC-04 | T-03-02 | `decode_state_changed()` returns `None` when D-Bus body shape mismatches `(u32,)` | unit | `cargo test --workspace --lib decode_state_changed_returns_none_on_wrong_body_shape` | ✅ src/network.rs:109 | ✅ green |
| 01-03-01b | 03 | 1 | SEC-04 | T-03-02 | `decode_state_changed()` returns `Some(value)` on well-formed `(u32,)` body | unit | `cargo test --workspace --lib decode_state_changed_returns_some_on_well_formed_body` | ✅ src/network.rs:125 | ✅ green |
| 01-03-02a | 03 | 1 | SEC-04 | T-03-02 | `decode_prepare_for_sleep()` returns `None` on wrong body shape | unit | `cargo test --workspace --lib decode_prepare_for_sleep_returns_none_on_wrong_body_shape` | ✅ src/sleep.rs:107 | ✅ green |
| 01-03-02b | 03 | 1 | SEC-04 | T-03-02 | `decode_prepare_for_sleep()` returns `Some(bool)` on well-formed `(bool,)` body | unit | `cargo test --workspace --lib decode_prepare_for_sleep_returns_some_on_well_formed_body` | ✅ src/sleep.rs:123 | ✅ green |

*Status legend: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. Phase 01 added the
following dev-deps to Cargo.toml as its own Wave 0 (no separate wave was
declared; the plan authored the deps and the tests in a single execution
wave):

- [x] `wiremock = "0.6"` in `[dev-dependencies]` (mock HTTP server for the 5 leak assertions)
- [x] `tracing-test = "0.2"` in `[dev-dependencies]` (per-test tracing subscriber + `logs_contain()`)
- [x] `tokio` with `rt/macros/net/io-util` features in `[dev-dependencies]` (async test runtime for the wiremock tests)

All three verified present via `awk '/\[dev-dependencies\]/,/^\[/' Cargo.toml`.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Stream-loop-level Err handling (transport-level D-Bus error while iterating `MessageStream::next()` — the `match item { Err(e) => { debug!; continue; } }` arm) | SEC-04 | Simulating a mid-stream D-Bus transport error without a live D-Bus session is out of scope for the decode-helper test seam chosen in Plan 01-03 D-09. Threat T-03-01 (transient Err kills stream) is mitigated by inspection: `grep -c 'match item' src/network.rs src/sleep.rs` both return ≥1, and the `continue` arm is visible at src/network.rs:78-81 and src/sleep.rs:76-79. | On any Linux host with a live system D-Bus: run a consumer of `network_stream()` / `sleep_stream()`; inject a stream Err (e.g. kill NetworkManager mid-subscription); confirm the stream continues yielding subsequent events instead of terminating silently, and that a `tracing::debug!("{NetworkManager,logind} stream error: {}", e)` line appears. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies — 14 automated commands mapped, 1 manual (SEC-04 stream-level Err handling, documented rationale)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify — every task has an automated command
- [x] Wave 0 covers all MISSING references — none missing; existing infra + 3 dev-deps added in-phase cover all requirements
- [x] No watch-mode flags — all commands use `cargo test --workspace` (single-run)
- [x] Feedback latency < 10s — full suite runs in ~5s per Phase 02 VERIFICATION.md CI Gates table
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-07-01 (retroactive reconstruction after `/gsd-audit-milestone v0.9` flagged missing VALIDATION.md; all requirements found already COVERED by tests committed during phase execution)
