---
phase: 3
slug: alerts-parser-coverage
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-01
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` (in-file `#[cfg(test)] mod tests`); `insta` reserved for `tests/wire_contract.rs` only |
| **Config file** | none — `Cargo.toml` dev-dependencies already present |
| **Quick run command** | `cargo test --lib alerts` |
| **Full suite command** | `cargo test --workspace` |
| **Coverage command** | `cargo llvm-cov --workspace --summary-only` (per-file for `src/alerts.rs`) |
| **Estimated runtime** | ~10–30 seconds (no network; fixtures only) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib alerts`
- **After every plan wave:** Run `cargo test --workspace` + `cargo clippy --workspace -- -D warnings`
- **Before `/gsd-verify-work`:** Full suite green AND wire contract unchanged (`cargo test --test wire_contract`)
- **Max feedback latency:** ~30 seconds

---

## Per-Task Verification Map

> Populated by the planner once task IDs exist. Every parser/expiry/dispatch task maps to a `cargo test` assertion.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 3-01-01 | 01 | 1 | COV-01 | T-03-01 | N/A (test-only; extraction preserves wire contract) | build/regression | `cargo build --workspace && cargo test --workspace` | ✅ existing suite | ⬜ pending |
| 3-01-02 | 01 | 1 | COV-01 | T-03-01 | N/A (test-only phase) | unit | `cargo test --lib alerts::tests::nws_` + `alerts::tests::from_cap_string_maps_all_classes` | ❌ W0 | ⬜ pending |
| 3-01-03 | 01 | 1 | COV-01 | T-03-01 | N/A (test-only phase) | unit | `cargo test --lib alerts::tests::bom_` | ❌ W0 | ⬜ pending |
| 3-02-01 | 02 | 2 | COV-01 | T-03-03 | N/A (test-only phase) | unit | `cargo test --lib alerts::tests::meteoalarm_` | ❌ W0 | ⬜ pending |
| 3-02-02 | 02 | 2 | COV-01 | T-03-03 | N/A (test-only phase) | unit | `cargo test --lib alerts::tests::eccc_` | ❌ W0 | ⬜ pending |
| 3-02-03 | 02 | 2 | COV-01 | — | N/A (test-only phase) | unit + coverage | `cargo test --lib alerts::` + `cargo llvm-cov --workspace --summary-only \| grep alerts.rs` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/alerts.rs` gains a `#[cfg(test)] mod tests` block — new test scaffolding for COV-01
- [ ] Fixtures for NWS JSON, MeteoAlarm XML, ECCC, BOM (inline string literals per research)
- [ ] No new framework install — `cargo test` + `cargo-llvm-cov` already available

*Existing infrastructure covers all phase requirements — no test-runner install needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live regional dispatch for US/EU/CA/AU match arms | COV-01 | Requires live HTTP (excluded by phase constraint); only the `Region::Unknown` branch is provable offline | Accepted, documented coverage gap — verify via code read, not test |

*All other phase behaviors have automated verification via fixtures.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
