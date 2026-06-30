# Phase 1: Security Audit - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-30
**Phase:** 1-Security Audit
**Areas discussed:** URL/token sanitization mechanism, Coord-validation error surface (SEC-03), D-Bus malformed-message observability (SEC-04), Leak-assertion test architecture (SEC-02, SEC-05)

---

## URL/token sanitization mechanism

### Q1: Where does URL/token scrubbing happen?

| Option | Description | Selected |
|--------|-------------|----------|
| `e.without_url()` + shared `sanitize_url()` helper | Use reqwest::Error::without_url() at every log site in client.rs. Add `pub(crate) fn sanitize_url(&str) -> String` for any URL the crate formats itself. Two complementary primitives, both unit-testable. | ✓ |
| Centralized in client.rs: log only ctx + error kind name, never `{e}` Display | Per-site logs drop the reqwest::Error Display entirely. Bulletproof but loses diagnostic value. | |
| Custom `SanitizedReqwestError` Display wrapper | Wrapper type owns sanitization in its own Display impl. One implementation locus, adds a new helper type. | |

### Q2: Which query parameters does `sanitize_url()` redact?

| Option | Description | Selected |
|--------|-------------|----------|
| Strip the entire query string | Replace `?...` with `?[redacted]`. Simplest, no allowlist drift, no surprise leak from a new provider adding a param we forgot. | ✓ |
| Case-insensitive name allowlist: token, api_key, apikey, key, secret, password | Walk query, mask matching keys, pass through others. Risk: missing a provider-specific name. | |
| Strip query AND scrub `geo:lat;lon` path segments | Also masks the aqicn coord path segment. Larger blast radius; needs regex/path parser. | |

### Q3: Where does `sanitize_url()` live and what's its visibility?

| Option | Description | Selected |
|--------|-------------|----------|
| `pub(crate) fn` in `client.rs` | One module owns HTTP concerns; helper sits next to the call sites. Public surface unchanged. | ✓ |
| New private module `src/redact.rs` | Dedicated module for log-safety helpers. Costs one extra module. | |
| `pub fn` exported from crate root | Consumers could reuse it. Adds wire-stable public surface; not required by the audit. | |

---

## Coord-validation error surface (SEC-03)

### Q1: How does the out-of-range / NaN reject path surface as an Error?

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse unit `Error::LocationDetection` | Both reject paths share the same variant. Wire kind unchanged. No breaking change for atmos/tempest. | ✓ |
| Add sub-kind: `LocationDetection(LocationKind { Failed, InvalidCoordinates })` | Mirrors NetworkKind/ParseKind convention. Breaking change to Error enum, needs new wire-kind strings and snapshot update. | |
| Add separate variant: `Error::Validation(ValidationKind)` | Reserves `LocationDetection` for upstream failure. Two new public surfaces, new wire kind/snapshot. | |

### Q2: Where does the lat/lon validation happen?

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit post-parse check inside `detect_location()` | Validate after `response.json()`, inside the existing `(Some(lat), Some(lon))` arm. One obvious place to read. | ✓ |
| Custom serde Deserialize on `IpApiResponse` | Validation at parse time; spreads logic into deserialize impl. | |
| Validator helper `fn valid_coords(lat, lon) -> bool` used from detect_location | Same call site as option 1, but extracts the predicate as `pub(crate) fn` for future JMA reuse. | |

### Q3: Is NaN/infinity covered by a dedicated test, or implicit in the range check?

| Option | Description | Selected |
|--------|-------------|----------|
| Add an explicit NaN/infinity test case | Range predicate rejects NaN by f64 semantics, but a future "simplification" could silently break the contract. Pin behaviour with a dedicated test. | ✓ |
| Implicit — the range-out test covers NaN by transitivity | One test exercises an out-of-range value; trust f64 ordering. Minimal surface; regression risk. | |

---

## D-Bus malformed-message observability (SEC-04)

### Q1: Code shape for handling the deserialize Err arm in both streams?

| Option | Description | Selected |
|--------|-------------|----------|
| `match` with explicit Err arm logging at debug | Replace `if let Ok` with `match ... { Ok => ..., Err(e) => tracing::debug!(...) }`. Forces the Err arm to be visible. | ✓ |
| Extract `decode_state_changed`/`decode_prepare_for_sleep` helpers | Pure functions return `None` on malformed body, log inside. Loop body shrinks. Easier to test. | (synthesised with Q1 — see Q3) |
| Keep `if let Ok` and add an `else` branch | Smallest diff; equivalent to option 1 minus the readability win. | |

### Q2: Does `while let Some(Ok(msg)) = stream.next().await` also need to log on `Some(Err)`?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — convert to `while let Some(item) = ...` and log on Err, continue | Currently silently exits the loop on first stream Err — stream stops yielding for the rest of the session, no log. Worst kind of silent fault. | ✓ |
| Out of scope — only deserialize Err matters | Defers the stream-Err handling to a future phase. Keeps the diff narrower. | |

### Q3: How does the test for malformed-message handling exercise both code paths?

| Option | Description | Selected |
|--------|-------------|----------|
| Extract `decode_state_changed`/`decode_prepare_for_sleep` helpers, unit-test them | Pure helpers, no live D-Bus needed. Construct `zbus::Message` with wrong body signature. Lightest CI dependency. | ✓ |
| Integration test driving a mocked D-Bus session bus | Spin up a private bus, publish a wrong-shape signal, assert via tracing capture. Highest fidelity, biggest infra. | |
| Inline test on existing functions via `#[cfg(test)] pub(crate) fn` decode shim | Less refactoring, but bleeds test concerns into prod code. | |

**Notes:** Q1 and Q3 selections synthesised — helpers from Q3 internally use the explicit `match` from Q1 and log on Err; loop body becomes `if let Some(state) = decode_state_changed(&msg)`.

---

## Leak-assertion test architecture (SEC-02, SEC-05)

### Q1: How does the test capture tracing events?

| Option | Description | Selected |
|--------|-------------|----------|
| Custom `tracing_subscriber::Layer` that records events into a `Vec<String>` | Hand-rolled ~30 lines, no new dep, scope-controlled install. | |
| Add `tracing-test` as a dev-dependency | Provides `#[traced_test]` macro and `logs_contain`/`logs_assert_no_match`. ~5 lines per test. Global subscriber; verify behaviour with parallel test execution. | ✓ |
| Redirect `tracing_subscriber::fmt` Writer to a buffer | Captures actual on-the-wire format; trickier setup. | |

### Q2: What drives the HTTP error paths the leak test inspects?

| Option | Description | Selected |
|--------|-------------|----------|
| `wiremock` dev-dep — stand up a local server, return 4xx/5xx + invalid JSON | Crate unchanged; exercises actual get_text/get_json paths and their tracing!. One dev-dep. | ✓ |
| `mockito` dev-dep — same idea, older API | Similar coverage; less actively maintained. | |
| No live server — point client at `http://127.0.0.1:1/` (closed port), connect-fail only | Zero new dep but covers only the connect arm. Narrowest. | |

### Q3: What sentinel token value goes into the test URL?

| Option | Description | Selected |
|--------|-------------|----------|
| Hard-coded constant `AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG` | Deterministic, easy to grep, unique. No env coupling. | ✓ |
| Random per-test UUID generated at setup | Re-rolled each run; marginal benefit; adds dep. | |
| Env-var driven: `WEATHERVANE_TEST_TOKEN` | Risk of pointing CI at production. Rejected on safety grounds. | |

### Q4: Which code paths does the leak assertion exercise?

| Option | Description | Selected |
|--------|-------------|----------|
| All four `get_json`/`get_text` failure arms (connect, status, body, decode) + happy path | Five sub-tests; matches the four `tracing::debug!` lines in client.rs + aqicn happy path. | ✓ |
| Just the connect+status+parse arms | Skip body-failed and happy. Three sub-tests; leaves a regression hole. | |
| One end-to-end test through `fetch_headline_aqi` with a wiremock 4xx | Single test; low granularity for pointing at which leak site failed. | |

---

## Claude's Discretion

- Exact debug-log message wording for D-Bus decode failures and stream errors (D-07, D-08).
- Whether to add the `pub(crate) fn valid_coords()` helper now (D-05 inlines) or wait for Phase 2 reuse.
- Whether leak-assertion tests extend `tests/wire_contract.rs` or live in a new `tests/leak_contract.rs`.

## Deferred Ideas

- Path-segment scrubbing of the aqicn `geo:LAT;LON` URL segment.
- Rate-limiting or aggregating repeated D-Bus stream errors.
- AddMatch-failure observability beyond the existing `tracing::warn!`.
- CI grep / clippy lint preventing future direct `{}` formatting of `reqwest::Error`.
- Extracting `pub(crate) fn valid_coords()` for FAULT-02 reuse.
