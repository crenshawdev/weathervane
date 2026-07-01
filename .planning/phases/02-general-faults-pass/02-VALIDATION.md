---
phase: 02
slug: general-faults-pass
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-30
reconstructed_from: SUMMARY.md (post-execution audit)
---

# Phase 02 — Validation Strategy

> Reconstructed post-execution from `02-01-SUMMARY.md`, `02-02-SUMMARY.md`, and `VERIFICATION.md`. All requirements were already covered by pinning tests written during execution; no gaps were introduced by this audit.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (built-in `#[cfg(test)]` modules) + `insta` 1.x for wire snapshots |
| **Config file** | `Cargo.toml` (workspace root) |
| **Quick run command** | `cargo test --workspace --lib` |
| **Full suite command** | `cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo build --workspace && cargo test --workspace` |
| **Estimated runtime** | ~10 seconds (lib tests) / ~30 seconds (full four-gate suite) |

---

## Sampling Rate

- **After every task commit:** `cargo test --workspace --lib`
- **After every plan wave:** Full four-gate suite (fmt + clippy + build + test)
- **Before `/gsd-verify-work`:** Full suite must be green (all 61 lib + 22 wire_contract tests)
- **Max feedback latency:** ~30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | FAULT-01 | T-02-01 | NaN comparator inputs do not panic; well-formed stations still rank first in the sort | unit | `cargo test --workspace --lib nan_station_coords_do_not_panic` | ✅ src/weather_jma.rs:301 | ✅ green |
| 02-01-01 | 01 | 1 | FAULT-01 | T-02-01 | Pre-existing nearest-selection behaviour preserved after comparator rewrite | unit (regression) | `cargo test --workspace --lib nearest_selection_picks_closest` | ✅ src/weather_jma.rs:271 | ✅ green |
| 02-01-01 | 01 | 1 | FAULT-01 | T-02-01 | No bare `partial_cmp().unwrap()` remains in the file | static (grep) | `test $(grep -cE 'partial_cmp\([^)]*\)\.unwrap\(\)' src/weather_jma.rs) -eq 0` | ✅ src/weather_jma.rs | ✅ green |
| 02-01-02 | 01 | 1 | FAULT-02 | T-02-02 | Length-mismatched JMA stations are dropped and the drop is observable at debug level | unit | `cargo test --workspace --lib station_array_length_mismatch_is_dropped` | ✅ src/weather_jma.rs:331 | ✅ green |
| 02-01-02 | 01 | 1 | FAULT-02 | T-02-02 | Out-of-range JMA stations (including NaN/infinity via closed-range predicate) are dropped and observable at debug level | unit | `cargo test --workspace --lib station_out_of_range_coords_are_dropped` | ✅ src/weather_jma.rs:356 | ✅ green |
| 02-01-02 | 01 | 1 | FAULT-02 | T-02-03 | Debug-log format string interpolates the JMA station `{code}` only; no raw `s.lat`/`s.lon` array contents leak into tracing | static (grep) | `! grep -n 'tracing::' src/weather_jma.rs \| grep -E 's\.lat\|s\.lon'` | ✅ src/weather_jma.rs | ✅ green |
| 02-02-01 | 02 | 1 | FAULT-03 | T-02-05 | `parse()` helper `.expect()` fires with fixture context when inline JSON fails to deserialize | unit (indirect — helper exercised by all pollen tests) | `cargo test --workspace --lib pollen::tests` | ✅ src/pollen.rs:108 | ✅ green |
| 02-02-01 | 02 | 1 | FAULT-03 | T-02-05 | Active-species fixture path returns `Some(PollenData)`; `.expect()` message names the test | unit | `cargo test --workspace --lib returns_some_when_covered_with_active_species` | ✅ src/pollen.rs:115 | ✅ green |
| 02-02-01 | 02 | 1 | FAULT-03 | T-02-05 | All-zero-but-present fixture path returns `Some(PollenData)`; `.expect()` message names the test | unit | `cargo test --workspace --lib returns_some_when_covered_with_all_zero_species` | ✅ src/pollen.rs:134 | ✅ green |
| 02-02-01 | 02 | 1 | FAULT-03 | T-02-05 | Mixed-null fixture zero-fills missing species; `.expect()` message names the test | unit | `cargo test --workspace --lib zero_fills_when_some_species_null_but_others_present` | ✅ src/pollen.rs:166 | ✅ green |
| 02-02-01 | 02 | 1 | FAULT-03 | T-02-05 | `time.rs:141` `.expect(...)` embeds the literal ISO date being parsed | unit | `cargo test --workspace --lib parsed_date_from_iso` | ✅ src/time.rs:140 | ✅ green |
| 02-02-01 | 02 | 1 | FAULT-03 | T-02-05 | No bare `.unwrap()` remains in `src/pollen.rs`; old thin `expect("valid ISO date")` is gone | static (grep) | `test $(grep -cE '\.unwrap\(\)' src/pollen.rs) -eq 0 && test $(grep -cE '\.expect\("valid ISO date"\)' src/time.rs) -eq 0` | ✅ src/pollen.rs, src/time.rs | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covered all phase requirements. No new test files or dev-dependencies were required.

- Rust workspace `cargo test` was already in place before Phase 02.
- `insta` snapshots at `tests/snapshots/` were untouched (wire contract preserved).
- No new dev-dependencies added; the plan-permitted optional `tracing-test` reuse from Phase 01 was declined by the executor in favour of behavioural (station-count / identity) assertions.

---

## Manual-Only Verifications

All phase behaviors have automated verification. Every FAULT requirement is bound to at least one passing `#[test]` fn plus a source-level grep check; the four CI gates (fmt, clippy, build, test) re-run the whole thing on every commit.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify (no Wave 0 dependencies were needed)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify — every task in both plans has behavioural + static verification
- [x] Wave 0 covers all MISSING references — no MISSING references identified
- [x] No watch-mode flags — `cargo test --workspace` is a one-shot run
- [x] Feedback latency < 30s (lib tests ~1s; full four-gate suite ~30s)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-06-30 (reconstructed post-execution; zero gaps found in audit)

---

## Reconstruction Audit Notes

This VALIDATION.md was created post-execution during `/gsd-validate-phase 02`. Because the phase already shipped with per-task pinning tests plus a `VERIFICATION.md` that mechanically re-ran every acceptance-criteria grep and test, the audit found no gaps and did not need to spawn `gsd-nyquist-auditor` for gap-filling.

Static checks re-run at audit time (2026-06-30):

- `grep -cE 'partial_cmp\([^)]*\)\.unwrap\(\)' src/weather_jma.rs` → `0` (FAULT-01)
- `grep -cE 'unwrap_or\(std::cmp::Ordering::Equal\)' src/weather_jma.rs` → `3` (FAULT-01; 1 production + 2 test sites)
- `grep -c 'dropping JMA station' src/weather_jma.rs` → `2` (FAULT-02)
- `grep -cE '\.unwrap\(\)' src/pollen.rs` → `0` (FAULT-03)
- `grep -cE '\.expect\("' src/pollen.rs` → `4` (FAULT-03)
- `grep -cE '\.expect\("valid ISO date"\)' src/time.rs` → `0` (FAULT-03)
- `grep -c '2025-11-25' src/time.rs` → `3` (FAULT-03; literal + echoed twice)
- `cargo test --workspace --lib` → 61 passed, 0 failed
