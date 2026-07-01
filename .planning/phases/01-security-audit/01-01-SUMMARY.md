---
phase: 01-security-audit
plan: 01
subsystem: testing
tags: [security, tracing, reqwest, wiremock, tracing-test, token-leak]

requires: []
provides:
  - pub(crate) sanitize_url() in src/client.rs strips query strings for safe URL logging
  - e.without_url() applied at all five tracing::debug! sites in get_json and get_text
  - Five inline #[tokio::test] leak-assertion tests in src/client.rs covering all HTTP error arms
  - wire_error_never_leaks_aqicn_sentinel in tests/wire_contract.rs pins the wire boundary
affects:
  - 01-02-PLAN
  - 01-03-PLAN

tech-stack:
  added:
    - wiremock = "0.6" (dev-dependency)
    - tracing-test = "0.2" (dev-dependency)
    - tokio = "1" with rt/macros/net/io-util features (dev-dependency)
  patterns:
    - e.without_url() at every reqwest::Error tracing site (strips embedded URL from Display)
    - sanitize_url(url) helper for future log sites that format crate-built URLs
    - AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG pattern for token-leak regression tests
    - Raw TcpListener technique for simulating body-read failure without wiremock hacks

key-files:
  created: []
  modified:
    - src/client.rs
    - Cargo.toml
    - tests/wire_contract.rs

key-decisions:
  - "e.without_url() chosen over sanitize_url at tracing sites -- reqwest::Error Display embeds the URL; without_url() is the zero-cost correct fix"
  - "sanitize_url kept pub(crate) with #[allow(dead_code)] -- no production log site needs it yet; test module exercises it"
  - "body-failure test uses raw TcpListener (claims Content-Length 1000, delivers 5 bytes then drops) -- wiremock cannot express connection-drop mid-body"
  - "tokio added as explicit dev-dep (rt/macros/net/io-util) -- transitive via wiremock but better to be explicit for TcpListener test infra"

patterns-established:
  - "Sentinel-based leak assertions: hard-coded unique constant as fake token, assert !logs_contain(SENTINEL) and !logs_contain('?token=')"
  - "Raw TCP partial-body server pattern for body-read failure tests in hyper-based HTTP clients"

requirements-completed:
  - SEC-01
  - SEC-02
  - SEC-05

coverage:
  - id: D1
    description: "sanitize_url helper strips query string -- URL with ?token=SENTINEL returns base?[redacted], sentinel absent from output"
    requirement: SEC-01
    verification:
      - kind: unit
        ref: "src/client.rs#sanitize_url_strips_sentinel_from_query_string"
        status: pass
    human_judgment: false
  - id: D2
    description: "e.without_url() at get_json request-failed site (line 83) -- token never reaches tracing output on connect failure"
    requirement: SEC-01
    verification:
      - kind: unit
        ref: "src/client.rs#aqicn_token_does_not_leak_on_connect_failure"
        status: pass
    human_judgment: false
  - id: D3
    description: "e.without_url() at get_json status-error site (line 86) -- token never reaches tracing output on HTTP 500"
    requirement: SEC-01
    verification:
      - kind: unit
        ref: "src/client.rs#aqicn_token_does_not_leak_on_status_error"
        status: pass
    human_judgment: false
  - id: D4
    description: "e.without_url() at get_text body-failed site (line 110) -- token never reaches tracing output on incomplete body"
    requirement: SEC-02
    verification:
      - kind: unit
        ref: "src/client.rs#aqicn_token_does_not_leak_on_body_failure"
        status: pass
    human_judgment: false
  - id: D5
    description: "e.without_url() at get_json parse-failed site (line 90) -- token never reaches tracing output on JSON parse failure"
    requirement: SEC-01
    verification:
      - kind: unit
        ref: "src/client.rs#aqicn_token_does_not_leak_on_parse_failure"
        status: pass
    human_judgment: false
  - id: D6
    description: "Happy-path leak guard -- token absent from tracing output on 200 success (catches future info-level 'fetched {url}' regression)"
    requirement: SEC-02
    verification:
      - kind: unit
        ref: "src/client.rs#aqicn_token_does_not_leak_on_success"
        status: pass
    human_judgment: false
  - id: D7
    description: "WireError serialization never contains aqicn sentinel -- SEC-05 wire boundary defense-in-depth"
    requirement: SEC-05
    verification:
      - kind: integration
        ref: "tests/wire_contract.rs#wire_error_never_leaks_aqicn_sentinel"
        status: pass
    human_judgment: false

duration: 18min
completed: 2026-06-30
status: complete
---

# Phase 01 Plan 01: URL/Token Leak Hardening Summary

**aqicn token stripped from all five tracing::debug! sites in HTTP client using e.without_url(); five wiremock+tracing-test leak-assertion tests added inline; wire boundary pinned via wire_contract.rs sentinel assertion.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-06-30T19:00:00Z
- **Completed:** 2026-06-30T19:18:00Z
- **Tasks:** 2
- **Files modified:** 3 (src/client.rs, Cargo.toml, tests/wire_contract.rs)

## Accomplishments

- Five `tracing::debug!` sites in `get_json` and `get_text` (lines 83, 86, 90, 106, 110) converted from bare `{e}` to `e.without_url()` -- reqwest::Error Display previously echoed the full request URL including the aqicn `?token=` query parameter
- `pub(crate) fn sanitize_url()` added at line 64 between `http_client()` and `get_json()` -- strips entire query string to `?[redacted]` for future log sites that format crate-built URLs
- Five `#[tokio::test] #[tracing_test::traced_test]` inline tests assert `!logs_contain(AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG)` and `!logs_contain("?token=")` across all five client failure+success paths
- `wire_error_never_leaks_aqicn_sentinel` added to `tests/wire_contract.rs` -- walks all `wire_error_exemplars()` and asserts the sentinel is absent from each serialized WireError
- Inspection confirmed `src/air_quality_aqicn.rs` tracing sites (lines 40, 45) format a parse error and a status string respectively -- neither formats `url` or `token` directly; no edit needed

## Converted Leak Sites (post-insertion line numbers)

| Site | Function | Original | Fixed |
|------|----------|----------|-------|
| 83 | `get_json` | `{ctx} request failed: {e}` | `{ctx} request failed: {}` via `e.without_url()` |
| 86 | `get_json` | `{ctx} status error: {e}` | `{ctx} status error: {}` via `e.without_url()` |
| 90 | `get_json` | `{ctx} parse failed: {e}` | `{ctx} parse failed: {}` via `e.without_url()` |
| 106 | `get_text` | `{ctx} request failed: {e}` | `{ctx} request failed: {}` via `e.without_url()` |
| 110 | `get_text` | `{ctx} body failed: {e}` | `{ctx} body failed: {}` via `e.without_url()` |

## Body-Failure Test Approach

wiremock cannot simulate a connection drop mid-body. The plan offered a fallback ("content-type mismatch") but `.text()` does not fail on mismatched content types. The chosen technique: a raw `tokio::net::TcpListener` that accepts the connection, writes HTTP headers claiming `Content-Length: 1000`, writes 5 bytes, then drops the TCP stream. hyper detects the incomplete body and returns an error, triggering the `body failed` tracing site. This reliably exercises the path on every run.

## Smoke Check Result

Temporarily reverted line 86 (`get_json` status-error site) from `e.without_url()` to bare `{e}`. The test `aqicn_token_does_not_leak_on_status_error` **FAILED** with the sentinel visible in captured log output:

```
DEBUG aqicn_token_does_not_leak_on_status_error: weathervane::client:
  aqicn status error: HTTP status server error (500 Internal Server Error)
  for url (http://127.0.0.1:37815/feed?token=AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG)
```

Restored `e.without_url()` before committing. All five tests pass on the final build.

## Task Commits

1. **Task 1: sanitize_url + e.without_url() at all tracing sites** - `e0aa2eb` (fix)
2. **Task 2: wiremock harness, inline tests, wire_contract sentinel** - `016800b` (feat)

## Files Created/Modified

- `src/client.rs` - sanitize_url helper (line 64), five e.without_url() conversions, #[cfg(test)] mod tests with sentinel constant and five leak-assertion tests
- `Cargo.toml` - wiremock = "0.6", tracing-test = "0.2", tokio dev-deps added
- `tests/wire_contract.rs` - wire_error_never_leaks_aqicn_sentinel test added after line 401

## Decisions Made

- `e.without_url()` is the correct primitive at tracing sites that format `reqwest::Error` -- it strips the embedded URL from `Display` at zero allocation cost. `sanitize_url()` is for the separate case where the crate formats a URL string it built itself.
- Body-failure test uses a raw `TcpListener` rather than wiremock -- content-type mismatch does not cause `.text()` to fail; the only reliable trigger is a truncated body read.
- `tokio` added as an explicit dev-dep (not just transitively via wiremock) for `TcpListener`, `AsyncWriteExt`, and `#[tokio::test]` -- explicit is more robust than relying on transitive feature flags.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Added #[allow(dead_code)] to sanitize_url**
- **Found during:** Task 1 (cargo clippy gate)
- **Issue:** `pub(crate) fn sanitize_url` has no production call sites yet (intentional -- it exists for future log sites). `cargo clippy --workspace -- -D warnings` does not compile `#[cfg(test)]` code, so clippy flags the function as unused even though Task 2 tests call it.
- **Fix:** Added `#[allow(dead_code)]` to the function. The test module's reference to it is sufficient behavioral coverage; the allow annotation communicates the intent explicitly.
- **Files modified:** src/client.rs
- **Verification:** `cargo clippy --workspace -- -D warnings` exits 0
- **Committed in:** e0aa2eb (Task 1 commit)

**2. [Rule 3 - Blocking] Added tokio to dev-dependencies**
- **Found during:** Task 2 (raw TcpListener body-failure test)
- **Issue:** `tokio::net::TcpListener` and `tokio::io::AsyncWriteExt` needed for the body-failure test. tokio is transitively available via wiremock but using transitive deps is fragile and Cargo warns about it. Plan spec listed only wiremock and tracing-test.
- **Fix:** Added `tokio = { version = "1", features = ["rt", "macros", "net", "io-util"] }` to `[dev-dependencies]`.
- **Files modified:** Cargo.toml
- **Verification:** Build succeeds, all tests pass
- **Committed in:** 016800b (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 Rule 1 clippy dead-code, 1 Rule 3 blocking dep)
**Impact on plan:** Both necessary for CI cleanliness and test infrastructure. No scope creep.

## Issues Encountered

None beyond the deviations documented above.

## Next Phase Readiness

- SEC-01, SEC-02, SEC-05 closed. The tracing and wire boundaries are hardened and regression-tested.
- 01-02-PLAN (coordinate validation in detect_location) and 01-03-PLAN (D-Bus stream hardening) are independent and ready to execute.
- The `sanitize_url` helper is available for any future log site that formats a crate-built URL.

---

## Self-Check

Checking created files exist and commits are present.

## Threat Flags

None. No new network endpoints, auth paths, file access patterns, or schema changes introduced. All changes are internal to the logging layer and dev-only test infrastructure.

---

*Phase: 01-security-audit*
*Completed: 2026-06-30*
