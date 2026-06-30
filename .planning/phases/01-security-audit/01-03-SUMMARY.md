---
phase: 01-security-audit
plan: "03"
subsystem: dbus-streams
tags: [security, dbus, observability, sec-04]
status: complete

dependency_graph:
  requires: []
  provides: [decode_state_changed, decode_prepare_for_sleep]
  affects: [src/network.rs, src/sleep.rs]

tech_stack:
  added: []
  patterns:
    - explicit-match stream loop (while let Some(item) with Err/Ok arms)
    - module-private decode helper returning Option<T>
    - zbus::Message::signal().build() for sessionless test message construction

key_files:
  created: []
  modified:
    - src/network.rs
    - src/sleep.rs

decisions:
  - "decode helpers are module-private (no pub, no pub(crate)) per PATTERNS.md"
  - "zbus::Message::signal(path, iface, member)?.build(&body)? used in tests -- no D-Bus session required"
  - "no new dependencies added; Message builder is part of the core zbus 5 crate (tokio feature already present)"
  - "wrong-body-shape test uses (u32,) for sleep.rs and (String,) for network.rs to get a mismatched D-Bus signature"

metrics:
  duration: "~20 minutes"
  completed: "2026-06-30T19:22:08Z"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
---

# Phase 01 Plan 03: D-Bus Stream Resilience Summary

Hardened the two D-Bus stream files against silent stream termination and
unobservable malformed messages (SEC-04). Both files now log at debug level on
stream-level errors and body decode errors, and continue streaming rather than
silently exiting.

## Tasks Completed

| Task | Name | Commit | Key files |
|------|------|--------|-----------|
| 1 | Harden src/network.rs -- extract decode_state_changed, convert stream loop, add tests | `3797d4f` | src/network.rs |
| 2 | Mirror transformation in src/sleep.rs -- extract decode_prepare_for_sleep, convert loop, add tests | `e3b89a7` | src/sleep.rs |

## zbus Message Builder API

Tests construct `zbus::Message` values directly without a live D-Bus session:

```rust
let msg = zbus::Message::signal(path, iface, member)?
    .build(&body)?;
```

- `Message::signal(path, iface, member)` returns `Result<Builder>` (sync, no connection).
- `Builder::build(&body)` serializes the body with its D-Bus type signature and returns `Result<Message>`.
- No extra features or dev-dependencies required. `zbus` is already in `[dependencies]` with the `tokio` feature, and the `Message` builder is not feature-gated.

Wrong-body-shape tests:
- `network.rs`: body `&("oops".to_string(),)` -- D-Bus sig `(s)` vs expected `(u)`.
- `sleep.rs`: body `&(42u32,)` -- D-Bus sig `(u)` vs expected `(b)`.

Well-formed tests:
- `network.rs`: body `&(70u32,)` -- asserts `Some(70)`.
- `sleep.rs`: body `&(true,)` and `&(false,)` -- asserts `Some(true)` and `Some(false)`.

## Final Log Message Wording

### src/network.rs

| Site | Level | Message |
|------|-------|---------|
| `network_stream` stream Err arm | `debug` | `NetworkManager stream error: {}` |
| `decode_state_changed` Err arm | `debug` | `NetworkManager message decode error: {}` |
| `decode_state_changed` Ok arm (preserved) | `debug` | `NetworkManager state changed: {}` |

### src/sleep.rs

| Site | Level | Message |
|------|-------|---------|
| `sleep_stream` stream Err arm | `debug` | `logind stream error: {}` |
| `decode_prepare_for_sleep` Err arm | `debug` | `logind message decode error: {}` |
| `decode_prepare_for_sleep` Ok arm (preserved) | `debug` | `logind PrepareForSleep: {}` |

## Linux-Only Fallback Paths: Unchanged

Both files preserve the two `std::future::pending::<()>().await` fallback points:

1. D-Bus system connection guard: `let Ok(connection) = zbus::Connection::system().await else { tracing::warn!(...); std::future::pending::<()>().await; return; }`
2. AddMatch failure guard: `if let Err(e) = connection.call_method(...) { tracing::warn!(...); std::future::pending::<()>().await; return; }`

Non-Linux callers continue to hit the idle stream path exactly as before (CLAUDE.md contract: "Linux-only streams degrade silently").

## Deviations from Plan

None -- plan executed exactly as written.

## CI Gate Results

All four gates passed after each task commit:

```
cargo fmt --check      OK
cargo clippy -- -D warnings   OK (no new warnings)
cargo build --workspace       OK
cargo test --workspace        OK -- 58 unit tests + 22 integration tests
```

New tests: 4 decode-helper pin tests (2 per file), all passing.

## Self-Check: PASSED

- `src/network.rs` exists and contains decode_state_changed + tests.
- `src/sleep.rs` exists and contains decode_prepare_for_sleep + tests.
- Commit `3797d4f` present in git log.
- Commit `e3b89a7` present in git log.
