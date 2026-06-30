# Phase 1: Security Audit - Pattern Map

**Mapped:** 2026-06-30
**Files analyzed:** 7 (6 source + 1 test)
**Analogs found:** 7 / 7

---

## File Classification

| File | Role | Data Flow | Closest Analog | Match Quality |
|------|------|-----------|----------------|---------------|
| `src/client.rs` | HTTP client / utility | request-response | `src/client.rs` itself (existing `get_json`/`get_text` log sites) | exact — adding helper + patching existing sites |
| `src/air_quality_aqicn.rs` | API provider (optional) | request-response | `src/client.rs:89-101` (`get_text` call chain) | exact — caller of `get_text`; URL construction is the only change |
| `src/location.rs` | domain fetch | request-response | `src/location.rs:106-126` (`detect_location` existing arm) | exact — inline guard inside existing `if let` arm |
| `src/error.rs` | error types | — | `src/error.rs` (no change this phase; analog pattern only) | exact — read-only reference |
| `src/network.rs` | D-Bus stream | event-driven | `src/sleep.rs:55-75` (identical loop shape) | exact — mirror of sleep.rs |
| `src/sleep.rs` | D-Bus stream | event-driven | `src/network.rs:57-76` (identical loop shape) | exact — mirror of network.rs |
| `tests/wire_contract.rs` | integration test / leak contract | — | `tests/wire_contract.rs:374-401` (sentinel/PII assertions) | exact — extend same style |

---

## Pattern Assignments

### `src/client.rs` — `sanitize_url()` + `e.without_url()`

**Role:** HTTP client utility
**Changes:** (1) add `pub(crate) fn sanitize_url(url: &str) -> String`; (2) replace bare `{e}` in every `tracing::debug!` with `{e}` via `e.without_url()`.

**Analog — existing `pub(crate)` helper shape** (`src/client.rs:66-81`):
```rust
pub(crate) async fn get_json<T: DeserializeOwned>(url: &str, ctx: &str) -> Option<T> {
    http_client()
        .ok()?
        .get(url)
        .send()
        .await
        .map_err(|e| tracing::debug!("{ctx} request failed: {e}"))   // <-- LEAK SITE
        .ok()?
        .error_for_status()
        .map_err(|e| tracing::debug!("{ctx} status error: {e}"))      // <-- LEAK SITE
        .ok()?
        .json::<T>()
        .await
        .map_err(|e| tracing::debug!("{ctx} parse failed: {e}"))      // <-- LEAK SITE
        .ok()
}
```

**New differentiator:** Replace `{e}` with `{e}` using `e.without_url()` (a zero-cost method on `reqwest::Error` returning a Display wrapper that strips the embedded URL):
```rust
.map_err(|e| tracing::debug!("{ctx} request failed: {}", e.without_url()))
```
Apply the same substitution to all four `tracing::debug!` sites in `get_json` and `get_text` (lines 72, 75, 79, 95, 99).

**New helper — `sanitize_url()`, placed after `http_client()` / before `get_json`:**
Signature: `pub(crate) fn sanitize_url(url: &str) -> String`
Logic: `if let Some(i) = url.find('?') { format!("{}?[redacted]", &url[..i]) } else { url.to_string() }`
No external dependencies; pure string operation.

---

### `src/air_quality_aqicn.rs` — pass sanitized URL to log

**Role:** Optional AQI provider
**Change:** The URL built at lines 24-27 goes directly into `get_text`. The token is in the query string. After D-01/D-03, the `get_text` log sites will already call `e.without_url()`, so the token never surfaces through the error path. No change is needed to `air_quality_aqicn.rs` for SEC-01 itself — but confirm by inspection.

**Analog — URL construction pattern** (`src/air_quality_aqicn.rs:24-29`):
```rust
let url = format!(
    "{FEED_URL_PREFIX}{latitude};{longitude}/?token={}",
    urlencoding::encode(token)
);

let body = get_text(&url, "aqicn").await?;
```

**Differentiator:** No structural change to this file. The `sanitize_url()` helper is available if a future log site ever formats this URL directly, but the current code only passes `url` to `get_text` (which owns all logging). Verification: confirm no `tracing::*!` macro in this file formats `url` or `token` directly. (Current code: only `tracing::debug!("aqicn returned non-ok status: {status}")` at line 45 — `status` is the response body field, not a URL or token. Clean.)

---

### `src/location.rs` — coordinate validation in `detect_location()`

**Role:** Domain fetch
**Change:** After `if let (Some(lat), Some(lon)) = (data.lat, data.lon)` (line 107), add a range guard before constructing `DetectedLocation`.

**Analog — existing guard shape in `detect_location()`** (`src/location.rs:106-126`):
```rust
if data.status == "success" {
    if let (Some(lat), Some(lon)) = (data.lat, data.lon) {
        let country = data.country.clone().unwrap_or_default();
        // ... build display_name ...
        tracing::debug!("Auto-detected location");
        return Ok(DetectedLocation { latitude: lat, longitude: lon, display_name, country });
    }
}

Err(Error::LocationDetection)
```

**Differentiator:** Insert immediately after `if let (Some(lat), Some(lon)) = ...` opens:
```rust
if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
    tracing::debug!("detect_location: coordinates out of valid range");
    return Err(Error::LocationDetection);
}
```
`f64` range-contains returns false for NaN and infinity (by IEEE 754 ordering), so this one predicate covers NaN, infinite, and out-of-range cases. Error variant stays `Error::LocationDetection` (D-04); wire snapshot unchanged.

**Analog for the error variant** (`src/error.rs:71-72`):
```rust
#[error("location detection failed")]
LocationDetection,
```
Unit variant — no fields, no new sub-kind needed.

**New unit tests** (inline `#[cfg(test)]` block in `location.rs`):
- `nan_coords_are_rejected` — constructs a fake `IpApiResponse`-shaped check with `lat = f64::NAN`
- `infinite_coords_are_rejected` — same with `lat = f64::INFINITY`
- `out_of_range_coords_are_rejected` — e.g. lat = 91.0, lon = 0.0
These target the validation predicate directly, not the full async function, so no mock HTTP is needed. Extract the predicate as a private `fn valid_coords(lat: f64, lon: f64) -> bool` only if the test can't reach it inline; D-05 allows inlining.

---

### `src/network.rs` — stream loop hardening + decode helper

**Role:** D-Bus stream
**Change:** (1) convert `while let Some(Ok(msg))` → explicit `match item`; (2) extract `decode_state_changed()` helper; (3) replace `if let Ok(body)` with `match ... { Err(e) => { tracing::debug!(...); None } }` inside helper.

**Analog — current loop and decode shape** (`src/network.rs:57-76`):
```rust
use futures::StreamExt;
while let Some(Ok(msg)) = stream.next().await {    // <-- silent-exit on Err
    let header = msg.header();
    if header.member().is_none_or(|m| m != "StateChanged")
        || header.interface().is_none_or(|i| i != "org.freedesktop.NetworkManager")
    {
        continue;
    }

    if let Ok(body) = msg.body().deserialize::<(u32,)>() {  // <-- silent-skip on Err
        let state = body.0;
        tracing::debug!("NetworkManager state changed: {}", state);
        if state == NM_STATE_CONNECTED_GLOBAL {
            tracing::info!("Network connectivity restored");
            yield NetworkEvent::Connected;
        }
    }
}
```

**Differentiator — new loop shape (D-08):**
```rust
while let Some(item) = stream.next().await {
    match item {
        Err(e) => { tracing::debug!("NetworkManager stream decode error: {e}"); continue; }
        Ok(msg) => {
            // header filter ... same as before ...
            if let Some(state) = decode_state_changed(&msg) {
                if state == NM_STATE_CONNECTED_GLOBAL {
                    tracing::info!("Network connectivity restored");
                    yield NetworkEvent::Connected;
                }
            }
        }
    }
}
```

**New decode helper (D-07):**
```rust
fn decode_state_changed(msg: &zbus::Message) -> Option<u32> {
    match msg.body().deserialize::<(u32,)>() {
        Ok(body) => {
            tracing::debug!("NetworkManager state changed: {}", body.0);
            Some(body.0)
        }
        Err(e) => {
            tracing::debug!("NetworkManager message decode error: {e}");
            None
        }
    }
}
```

**Analog for the `tracing::debug!` on continuation** (`src/network.rs:47-50`):
```rust
tracing::warn!("Failed to subscribe to NetworkManager signals: {}", e);
std::future::pending::<()>().await;
```
This is the `warn`-and-abort pattern. The new case uses `debug`-and-`continue` (recoverable; stream is still live).

---

### `src/sleep.rs` — stream loop hardening + decode helper

**Role:** D-Bus stream
**Change:** Mirror of the `network.rs` changes above. Identical structural transformation.

**Analog — current loop and decode shape** (`src/sleep.rs:55-74`):
```rust
while let Some(Ok(msg)) = stream.next().await {    // <-- silent-exit on Err
    let header = msg.header();
    if header.member().is_none_or(|m| m != "PrepareForSleep")
        || header.interface().is_none_or(|i| i != "org.freedesktop.login1.Manager")
    {
        continue;
    }

    if let Ok(body) = msg.body().deserialize::<(bool,)>() {  // <-- silent-skip on Err
        let going_to_sleep = body.0;
        tracing::debug!("logind PrepareForSleep: {}", going_to_sleep);
        if !going_to_sleep {
            tracing::info!("System resumed from suspend");
            yield SleepEvent::Resumed;
        }
    }
}
```

**Differentiator — new decode helper:**
```rust
fn decode_prepare_for_sleep(msg: &zbus::Message) -> Option<bool> {
    match msg.body().deserialize::<(bool,)>() {
        Ok(body) => {
            tracing::debug!("logind PrepareForSleep: {}", body.0);
            Some(body.0)
        }
        Err(e) => {
            tracing::debug!("logind message decode error: {e}");
            None
        }
    }
}
```
Loop head becomes identical to the `network.rs` pattern: `while let Some(item) = stream.next().await { match item { Err(e) => { tracing::debug!(...); continue; } Ok(msg) => { ... } } }`.

**Unit tests (D-09):** Both `decode_state_changed` and `decode_prepare_for_sleep` are private fns testable in inline `#[cfg(test)]` blocks. Test axes: wrong body type → None, correct body type → Some(value). No live D-Bus needed; construct a minimal `zbus::Message` with the wrong tuple type in the test.

---

### `tests/wire_contract.rs` — leak-assertion tests (SEC-02, SEC-05)

**Role:** Integration / leak contract test
**Change:** Add a new test block (extend existing file or new `tests/leak_contract.rs`) using `wiremock` for HTTP control and `tracing-test` for log capture.

**Analog — existing PII sentinel tests** (`tests/wire_contract.rs:374-401`):
```rust
#[test]
fn wire_error_never_leaks_query() {
    let err = Error::NoResults { query: "SENTINEL_QUERY_55x".to_string() };
    let wire = WireError::from(&err);
    let serialized = serde_json::to_string(&wire).unwrap();
    assert!(
        !serialized.contains("SENTINEL_QUERY_55x"),
        "query text leaked: {serialized}"
    );
}

#[test]
fn wire_error_never_leaks_coordinates() {
    for (_, err) in wire_error_exemplars() {
        let serialized = serde_json::to_string(&WireError::from(&err)).unwrap();
        assert!(!serialized.contains("45.5152"), "latitude leaked: {serialized}");
        assert!(!serialized.contains("-122.6784"), "longitude leaked: {serialized}");
    }
}
```

**Differentiator — new leak tests target tracing output, not serialized wire:**
- Add `wiremock` and `tracing-test` as dev-dependencies in `Cargo.toml`.
- Use `#[traced_test]` on each async test to capture log output.
- Stand up a `wiremock::MockServer`, mount a mock returning the target status/body.
- Construct a URL with the sentinel token in the query string: `format!("{}/feed?token={}", server.uri(), SENTINEL)`.
- Call `get_text(&url, "aqicn")` or equivalent directly.
- Assert `!logs_contain(SENTINEL)` and `!logs_contain("?token=")`.

**Sentinel constant (D-12):**
```rust
const AQICN_LEAK_SENTINEL: &str = "AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG";
```

**Coverage axes (D-13) — five sub-tests:**
1. wiremock returns connection refused (no server) → request-failed log path
2. wiremock returns HTTP 500 → status-error log path
3. wiremock returns 200 with body read error (drop connection mid-body) → body-failed log path
4. wiremock returns 200 with non-JSON body; caller parses → parse-failed log path (via `get_json`)
5. wiremock returns 200 with well-formed body → happy path, no log emitted

Each asserts `!logs_contain(AQICN_LEAK_SENTINEL)`.

**Import pattern to copy from existing test file** (`tests/wire_contract.rs:1-17`):
```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
use weathervane::{...};
```
New file or section adds:
```rust
use weathervane::client::{get_json, get_text};  // pub(crate) — only reachable from tests/ if re-exported or via #[cfg(test)] pub
```
If `get_json`/`get_text` are `pub(crate)`, they are not reachable from `tests/`. Planner must decide: either make them `pub` (widens surface), or test through a thin `pub` shim, or use `#[cfg(test)] pub(crate)` and move tests into `src/client.rs` inline. Recommend inline `#[cfg(test)]` in `src/client.rs` for the leak tests since `get_json`/`get_text` are the call sites.

---

## Shared Patterns

### SPDX header
**Source:** Every `.rs` file in the crate, line 1.
**Apply to:** All modified files (no new files this phase, but the pattern must be present).
```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
```

### `tracing::debug!` for non-fatal observability
**Source:** `src/network.rs:69`, `src/sleep.rs:67`, `src/client.rs:72,75,79,95,99`
**Apply to:** All new debug log sites (D-Bus decode errors, sanitize_url trace, coord-validation rejection).
Pattern: single-line, lowercase message, no PII in the format string, structured where possible.

### `pub(crate)` for internal helpers
**Source:** `src/client.rs:31` (`http_client`), `src/client.rs:66` (`get_json`), `src/client.rs:89` (`get_text`)
**Apply to:** `sanitize_url()` in `src/client.rs`; decode helpers in `network.rs`/`sleep.rs` are private (crate-internal, not `pub(crate)`).

### Sentinel-based assertion style
**Source:** `tests/wire_contract.rs:376-384`
**Apply to:** All new leak-assertion tests. Hard-code a unique string constant; pass it as the sensitive value; assert the serialized/logged output does not contain it. Failure message shows the actual output (`"token leaked: {output}"`).

---

## No Analog Found

None. Every pattern this phase introduces has a close existing analog in the codebase.

---

## Metadata

**Analog search scope:** `src/`, `tests/`
**Files read:** `src/client.rs`, `src/air_quality_aqicn.rs`, `src/location.rs`, `src/network.rs`, `src/sleep.rs`, `src/error.rs`, `tests/wire_contract.rs`
**Pattern extraction date:** 2026-06-30
