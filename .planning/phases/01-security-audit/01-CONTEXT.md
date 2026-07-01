# Phase 1: Security Audit - Context

**Gathered:** 2026-06-30
**Status:** Ready for planning

<domain>
## Phase Boundary

Strip the aqicn token and full URLs from every log line and error payload the crate emits, validate `ip-api.com` response coordinates before returning them from `detect_location()`, and make D-Bus stream deserialization observably error-safe (debug-log on malformed input, stream continues without panicking). Extend `tests/wire_contract.rs` with a leak-assertion harness so any regression is caught in CI.

In scope: `src/client.rs`, `src/air_quality_aqicn.rs`, `src/location.rs`, `src/network.rs`, `src/sleep.rs`, `tests/wire_contract.rs` (or a new sibling test file). Requirements SEC-01..05.

Out of scope (deferred per `.planning/REQUIREMENTS.md`): cargo-audit/deny CI gates (DEPS), Nominatim/MeteoAlarm caching (DEPS), additional coverage gaps (TEST), public-surface hardening (API), performance work (PERF), retry/backoff (RES). FAULT-01..03 belong to Phase 2.

</domain>

<decisions>
## Implementation Decisions

### URL/token sanitization mechanism (SEC-01)
- **D-01:** Two complementary primitives. At every `tracing::*` site in `client.rs` that formats a `reqwest::Error`, call `e.without_url()` before formatting. For URLs the crate builds itself (currently only the aqicn URL in `air_quality_aqicn.rs:23-27`), pass them through a shared `sanitize_url()` helper before they reach any log line or error context.
- **D-02:** `sanitize_url()` strips the entire query string. If the input contains `?`, replace everything from the first `?` onward with `?[redacted]`. Host and path are preserved for diagnostic value. The aqicn `geo:lat;lon` path segment is intentionally NOT scrubbed (coords are already excluded from wire errors by a separate contract; scrubbing them from log URLs would widen scope past SEC-01).
- **D-03:** `sanitize_url()` lives as `pub(crate) fn` in `src/client.rs` next to `get_json`/`get_text`. No new module. Public surface unchanged.

### Coord-validation error surface (SEC-03)
- **D-04:** Reuse the existing unit-typed `Error::LocationDetection` for both the "ip-api.com returned non-success" path (current behaviour at `location.rs:126`) and the new "lat/lon out of range or NaN" reject path. Wire kind stays `"LocationDetection"`; `tests/snapshots/wire_contract__wire_error_LocationDetection.snap` is unchanged. No breaking change for atmos/tempest pattern matches.
- **D-05:** Validation is an explicit post-parse check inside `detect_location()`, immediately after `response.json::<IpApiResponse>().await?` and inside the existing `if let (Some(lat), Some(lon))` arm. Predicate: reject if `!(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon)` (which also rejects NaN and infinity by f64 ordering semantics). No new helper function; no validator extracted yet.
- **D-06:** Add an explicit `nan_coords_are_rejected` (and `infinite_coords_are_rejected`) unit test in `location.rs`, separate from the out-of-range test. Reason: NaN rejection is implicit in the range predicate but a future "simplification" rewrite (e.g. `lat <= 90.0 && lat >= -90.0`, which is true-for-NaN) would silently break SEC-03 without a dedicated test pinning it.

### D-Bus malformed-message observability (SEC-04)
- **D-07:** Extract pure decode helpers: `decode_state_changed(&zbus::Message) -> Option<u32>` and `decode_prepare_for_sleep(&zbus::Message) -> Option<bool>`. Each uses an explicit `match msg.body().deserialize::<(T,)>()` with the `Err(e) => { tracing::debug!(...); None }` arm written out (not `if let Ok`). Loop bodies in `network_stream`/`sleep_stream` become `if let Some(state) = decode_state_changed(&msg) { ... }`.
- **D-08:** Convert both stream loops from `while let Some(Ok(msg)) = stream.next().await` to `while let Some(item) = stream.next().await { match item { Ok(msg) => ..., Err(e) => { tracing::debug!(...); continue; } } }`. The current shape silently exits the loop on the first `Some(Err)` from `MessageStream::next`, which means the stream stops yielding for the rest of the session with no log; that's within the spirit of SEC-04 ("stream continues without panicking").
- **D-09:** Unit tests target the decode helpers directly with wrong-shape `zbus::Message` bodies (e.g. `(String,)` where `(u32,)` is expected). No live or mocked D-Bus session bus; helpers are the test boundary. Test names assert both the None-on-malformed and Some-on-well-formed paths.

### Leak-assertion test architecture (SEC-02, SEC-05)
- **D-10:** Add `tracing-test` as a dev-dependency for the leak-assertion tests; use its `#[traced_test]` macro for log capture. (User-chosen alternative considered: hand-rolled `tracing_subscriber::Layer`. Planner should confirm `tracing-test` plays well with `nextest`-style parallel test execution; if it doesn't, fall back to the hand-rolled Layer.)
- **D-11:** Drive HTTP error paths with a `wiremock` dev-dependency. Tests stand up a local wiremock server, point the crate at it (via `fetch_headline_aqi`-style code paths or directly via `get_json`/`get_text` with a wiremock URL containing the sentinel token), and inspect captured tracing output for any sentinel/URL substring.
- **D-12:** Sentinel token is a hard-coded constant `AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG` (or equivalent) used as the `?token=` query value in test URLs. Deterministic, unique enough that false matches are implausible. No env-var dependency, no random per-run UUID.
- **D-13:** Coverage is the four `tracing::debug!` failure arms in `client.rs` (connect-failed, status-error, body-failed, parse-failed) plus one happy-path 200 — five sub-tests in total. Each asserts `!captured.contains(SENTINEL)` and `!captured.contains("/?token=")` (or equivalent URL leak pattern).

### Claude's Discretion
- Exact debug-log message text for D-Bus decode failures and stream errors (D-07, D-08): planner/executor picks wording that is greppable but PII-free.
- Whether to add a `pub(crate) fn valid_coords(lat, lon) -> bool` helper (D-05 currently inlines): if Phase 2's FAULT-02 work reuses the predicate for JMA station validation, planner may extract; this phase does not require it.
- Where the leak-assertion tests live: extending `tests/wire_contract.rs` versus a new `tests/leak_contract.rs`. SEC-05 says "extends `tests/wire_contract.rs`"; planner may stay in that file or split if the file size becomes unwieldy.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Source files (in-scope edits)
- `src/client.rs` — leak sites at the four `tracing::debug!` lines (request failed / status error / body failed / parse failed); home of the new `sanitize_url()` helper.
- `src/air_quality_aqicn.rs` — URL builder at lines 19-30 (the `?token=` formatter); only call site that constructs a tokenised URL.
- `src/location.rs` — `detect_location()` at lines 100-127; coord validation goes inside the existing `Some(lat), Some(lon)` arm.
- `src/network.rs` — `network_stream()`; deserialize path around line 67, plus the `while let Some(Ok(msg))` loop header.
- `src/sleep.rs` — `sleep_stream()`; deserialize path around line 65, plus the `while let Some(Ok(msg))` loop header.
- `src/error.rs` — `Error::LocationDetection` and the wire mapping; do NOT change the variant shape (per D-04).
- `src/wire.rs` — `From<&Error> for WireError` mapping; unchanged this phase.

### Tests
- `tests/wire_contract.rs` — existing PII assertions at lines 374-401; new leak-assertion sub-tests extend (or accompany) this file.
- `tests/snapshots/wire_contract__wire_error_LocationDetection.snap` — wire snapshot that MUST stay unchanged after the SEC-03 work.

### Contracts and project state
- `CLAUDE.md` (root) — "no PII in errors/tracing" and "Linux-only streams degrade silently" contracts that shape this phase.
- `CONTRACT.md` — wire contract reference (mention only; this phase does not change wire types).
- `.planning/REQUIREMENTS.md` §Security (SEC-01..05) — the locked requirements list.
- `.planning/ROADMAP.md` §Phase 1 — goal statement and success criteria the executor verifies against.
- `.planning/PROJECT.md` §Key Decisions — sub-typed error kinds convention (NetworkKind/ParseKind) referenced in D-04.
- `.planning/codebase/ARCHITECTURE.md`, `CONCERNS.md`, `CONVENTIONS.md` — codebase maps generated 2026-06-30; useful background for the planner.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `reqwest::Error::without_url()` — used by D-01 at every `tracing::debug!("{ctx}: {e}", e = ...)` site in `client.rs`. No allocation; returns a reference whose Display omits the URL.
- Existing `IpApiResponse` struct in `location.rs` — keep as-is; coord validation runs on the deserialized struct, not at serde-deserialize time (D-05).
- Existing `Error::LocationDetection` unit variant and wire mapping — reused as-is (D-04).
- `if let Ok(body) = msg.body().deserialize::<T>()` pattern in network/sleep streams — replaced by the explicit-match shape inside the new decode helpers (D-07).

### Established Patterns
- Sub-typed error kinds (`NetworkKind`, `ParseKind`) — informs how a future SEC-03 expansion could go, but D-04 chose to NOT introduce a `LocationKind` here.
- `pub(crate)` helpers in `client.rs` (e.g. `get_json`, `get_text`) — `sanitize_url()` follows the same visibility and location (D-03).
- `tracing::debug!` for optional-provider fallback and observability-only events — leak-assertion tests rely on this level being where the sensitive data flows.

### Integration Points
- `air_quality_aqicn.rs::fetch_headline_aqi` is the only caller that builds a tokenised URL → `client::get_text()`. After D-01, the URL going into `get_text` would still carry the token (so the underlying HTTP request works); only the log/error formatting layer scrubs it.
- `wire_contract.rs` already has a PII-leak test for error payloads (lines 374-401); the new leak assertions extend that style to tracing capture.

</code_context>

<specifics>
## Specific Ideas

- SEC-04 phrasing "does not panic on malformed input" — scout confirmed the current `if let Ok` pattern does not panic; the gap is observability, not panic safety. D-07/D-08 deliver the observability and convert the `while let Some(Ok)` shape that silently kills the stream.
- The aqicn URL `https://api.waqi.info/feed/geo:LAT;LON/?token=...` is the only known token-bearing URL the crate constructs. Path-segment coord scrubbing was explicitly deferred (D-02 commentary).
- Sentinel string `AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG` is a recommendation; planner may bikeshed the exact identifier as long as it is unique and obvious in failure output.

</specifics>

<deferred>
## Deferred Ideas

- Path-segment scrubbing of the aqicn `geo:LAT;LON` URL segment — would tighten log hygiene further; out of scope for SEC-01..05 which target query-string scrubbing.
- Rate-limiting or aggregating repeated D-Bus stream errors — outside SEC-04 scope; raise if production logs ever show flapping.
- AddMatch-failure observability beyond the existing `tracing::warn!` — current behaviour suffices for SEC-04.
- CI grep / clippy lint preventing future direct `{}` formatting of `reqwest::Error` — the leak-assertion test catches it; an extra static check is belt-and-braces and not requested.
- Extracting `pub(crate) fn valid_coords()` for reuse — defer until Phase 2 (FAULT-02) work confirms a second call site.

</deferred>

---

*Phase: 1-Security Audit*
*Context gathered: 2026-06-30*
