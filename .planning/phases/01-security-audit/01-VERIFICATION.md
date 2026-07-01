---
phase: 01-security-audit
verified: 2026-06-30T20:30:00Z
status: passed
score: 5/5 must-haves verified (1 via override)
behavior_unverified: 0
overrides_applied: 1
overrides:
  - must_have: "tests/wire_contract.rs gains assertions that fail CI if aqicn token or unsanitized URL appears in any log line (SEC-02, SEC-05)"
    reason: "Documented architectural split per 01-CONTEXT.md D-12. Log-line assertions (5 #[tracing_test::traced_test] tests) live in src/client.rs because get_text and get_json are pub(crate) and unreachable from an integration test in tests/. Wire serialization assertion (wire_error_never_leaks_aqicn_sentinel) lives in tests/wire_contract.rs (SEC-05). Both run under cargo test --workspace and the smoke-check in 01-01-SUMMARY.md confirmed a leak-assertion fails when e.without_url() is reverted. The security goal of SC2 (CI catches token in log lines) is fully met; only the literal file location differs from the criterion's wording."
    accepted_by: "john"
    accepted_at: "2026-06-30T20:35:00Z"
gaps: []
---

# Phase 01: Security Audit — Verification Report

**Phase Goal:** No API token, raw URL, or unvalidated external coordinate reaches any log line, error payload, or D-Bus stream output produced by the crate.
**Verified:** 2026-06-30T20:30:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | URLs sanitized in client.rs and air_quality_aqicn.rs; aqicn token never in debug/error log (SEC-01) | VERIFIED | 5 `e.without_url()` calls at lines 83, 86, 90, 106, 110 of client.rs; `sanitize_url()` helper at line 65; air_quality_aqicn.rs tracing (lines 40, 45) formats serde_json::Error and status string only — no URL or token interpolated |
| 2 | tests/wire_contract.rs gains assertions failing CI if aqicn token or unsanitized URL appears in any log line (SEC-02, SEC-05) | FAILED | wire_contract.rs has `wire_error_never_leaks_aqicn_sentinel` (checks WireError JSON serialization — SEC-05 wire boundary only); log-line assertions exist but in src/client.rs, not wire_contract.rs; no `tracing_test`/`logs_contain` anywhere in wire_contract.rs |
| 3 | detect_location() returns Error::LocationDetection for out-of-range/NaN/infinite coordinates from ip-api.com; unit test covers reject path (SEC-03) | VERIFIED | Range guard at location.rs:109-112 using `!(-90.0..=90.0).contains(&lat) \|\| !(-180.0..=180.0).contains(&lon)`; static debug message (no coord PII); 3 tests passing: `nan_coords_are_rejected`, `infinite_coords_are_rejected`, `out_of_range_coords_are_rejected` |
| 4 | Malformed D-Bus message to network_stream() or sleep_stream() produces debug! log and stream continues; test covers both paths (SEC-04) | VERIFIED | `decode_state_changed()` and `decode_prepare_for_sleep()` helpers in network.rs/sleep.rs; explicit match loops with Err arms emitting debug! at stream and decode level; 4 tests pass: `decode_state_changed_returns_none_on_wrong_body_shape`, `decode_state_changed_returns_some_on_well_formed_body`, `decode_prepare_for_sleep_returns_none_on_wrong_body_shape`, `decode_prepare_for_sleep_returns_some_on_well_formed_body` |
| 5 | All four CI gates pass: cargo fmt --check, cargo clippy --workspace -- -D warnings, cargo build --workspace, cargo test --workspace | VERIFIED | All 4 gates exit 0; 58 unit tests + 22 wire contract tests = 80 total, all pass |

**Score:** 4/5 truths verified (1 failed — SC2 file-location deviation)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/client.rs` | sanitize_url() + e.without_url() at 5 tracing sites | VERIFIED | sanitize_url at line 65; e.without_url() at lines 83, 86, 90, 106, 110; 5 inline traced_test leak assertions |
| `src/location.rs` | Range guard + Error::LocationDetection return + 3 unit tests | VERIFIED | Guard at lines 109-112; 3 tests at lines 177, 190, 211 |
| `src/network.rs` | decode_state_changed() helper + explicit match loop + 2 tests | VERIFIED | Helper at line 24; match loop at lines 76-99; 2 tests at lines 109, 125 |
| `src/sleep.rs` | decode_prepare_for_sleep() helper + explicit match loop + 2 tests | VERIFIED | Helper at line 21; match loop at lines 74-98; 2 tests at lines 107, 123 |
| `tests/wire_contract.rs` | Assertions failing CI if aqicn token appears in log lines | FAILED | Has `wire_error_never_leaks_aqicn_sentinel` (WireError JSON check only); no log-line/tracing_test assertions |
| `Cargo.toml` | wiremock, tracing-test, tokio dev-deps | VERIFIED | All three added as dev-dependencies |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `air_quality_aqicn.rs::fetch_headline_aqi` | `client.rs::get_text` | URL with token passed to get_text() | WIRED | get_text() uses e.without_url() at all tracing sites; no direct URL logging in aqicn module |
| `location.rs::detect_location` | `error.rs::Error::LocationDetection` | range guard returns Err(Error::LocationDetection) | WIRED | Verified at lines 109-112; error variant unchanged (git log shows no phase commits on error.rs) |
| `network.rs::network_stream` | `network.rs::decode_state_changed` | called on every message with StateChanged member | WIRED | Line 92: `if let Some(state) = decode_state_changed(&msg)` |
| `sleep.rs::sleep_stream` | `sleep.rs::decode_prepare_for_sleep` | called on every message with PrepareForSleep member | WIRED | Line 90: `if let Some(going_to_sleep) = decode_prepare_for_sleep(&msg)` |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| All 80 tests pass (full suite) | `cargo test --workspace` | 58 unit + 22 wire = 80 passed, 0 failed | PASS |
| sanitize_url strips sentinel | `cargo test sanitize_url_strips_sentinel` | ok | PASS |
| NaN coord rejected | `cargo test nan_coords_are_rejected` | ok | PASS |
| Decode on wrong shape returns None | `cargo test decode_state_changed_returns_none` | ok | PASS |
| Token absent from log on HTTP 500 | `cargo test aqicn_token_does_not_leak_on_status_error` | ok | PASS |

### Probe Execution

Step 7c: SKIPPED — no `scripts/*/tests/probe-*.sh` files exist in this repository; no probes declared in any PLAN.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SEC-01 | 01-01-PLAN | No aqicn token or api_key in tracing/error output | SATISFIED | e.without_url() at 5 sites; air_quality_aqicn.rs tracing doesn't format URL |
| SEC-02 | 01-01-PLAN | Wire-contract test asserts no aqicn token in error log | PARTIAL | wire_contract.rs has WireError serialization check; log-line check is in client.rs |
| SEC-03 | 01-02-PLAN | ip-api.com coords validated before returning from detect_location() | SATISFIED | Range guard + 3 IEEE 754 pin tests |
| SEC-04 | 01-03-PLAN | D-Bus deserialization does not panic; failures at debug level | SATISFIED | Decode helpers + explicit match loops + 4 tests |
| SEC-05 | 01-01-PLAN | PII-leak assertion covering URLs extends wire_contract.rs | PARTIAL | wire_contract.rs checks WireError serialization for sentinel; URL query-param log assertions are in client.rs |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | — | — | — |

No TBD/FIXME/XXX markers in any modified file. No stub implementations. No hardcoded empty returns in production paths. The `#[allow(dead_code)]` on `sanitize_url` is intentional and documented — the function has no production call site yet (the plan notes it exists for future log sites) and its test usage is in `#[cfg(test)]` which clippy does not see.

### CLAUDE.md Contract Check

| Contract | Status | Evidence |
|----------|--------|---------|
| No PII in tracing/error lines | VERIFIED | All 12 new tracing calls use static strings or library-generated error display (e.without_url(), serde_json::Error, zbus::Error). No coord values, URLs, or tokens interpolated. |
| error.rs unchanged | VERIFIED | `git log -- src/error.rs` shows no commits from this phase (last commit is `600f504` from a prior session) |
| LocationDetection snapshot intact | VERIFIED | `git log -- tests/snapshots/wire_contract__wire_error_LocationDetection.snap` shows last commit is `25d3af1` (pre-phase); snapshot content: `{"kind":"LocationDetection","message":"location detection failed"}` |

### Gaps Summary

One gap: SC2 file-location deviation.

The ROADMAP success criterion 2 says `tests/wire_contract.rs` gains assertions that fail CI if an aqicn token or an unsanitized URL appears in **any log line**. What wire_contract.rs actually has is `wire_error_never_leaks_aqicn_sentinel`, which checks WireError JSON serialization — a wire-boundary defense, not a log-line assertion.

The actual log-line assertions (5 tests using `#[tracing_test::traced_test]` + `logs_contain()`) are in `src/client.rs` inline tests. They run in CI via `cargo test --workspace` and do catch the sentinel in log output. The smoke-check in the SUMMARY even confirmed the test FAILED when e.without_url() was temporarily reverted — the mechanism is real.

This is a file-location deviation, not a security gap. The security intent of SC2/SEC-02 (leaks caught in CI) is fully achieved. Two resolution paths:

**Option A — Add log-line assertions to wire_contract.rs** (closes the literal wording of SC2): add a `#[cfg(test)]` helper in wire_contract.rs that constructs an AQICN sentinel URL, calls a client path under `tracing_test`, and asserts `!logs_contain(SENTINEL)`.

**Option B — Override SC2** (accept the split): document that SEC-05 (wire boundary) is in wire_contract.rs and SEC-02 (log-line boundary) is in client.rs — both in CI, security goal met, location deviation intentional.

```yaml
# Override to add to this file's frontmatter if accepting Option B:
overrides:
  - must_have: "tests/wire_contract.rs gains assertions that fail CI if aqicn token or unsanitized URL appears in any log line (SEC-02, SEC-05)"
    reason: "Log-line assertions (5 tracing_test tests) are in src/client.rs where the logging happens; wire serialization assertion is in tests/wire_contract.rs (SEC-05). Both run in cargo test --workspace and the security goal is fully achieved."
    accepted_by: "john"
    accepted_at: "2026-06-30T20:30:00Z"
```

---

_Verified: 2026-06-30T20:30:00Z_
_Verifier: Claude (gsd-verifier)_
