---
phase: 3
reviewers: [codex]
reviewed_at: 2026-07-02T00:27:59Z
plans_reviewed: [03-01-PLAN.md, 03-02-PLAN.md]
notes: |
  Only Codex was available at review time (gpt-5.4-mini). Claude self-skipped (running inside Claude Code).
  Other reviewers (gemini, coderabbit, opencode, qwen, cursor, antigravity, ollama, lm_studio, llama_cpp) are not installed on this system.
  With a single reviewer, the Consensus Summary reflects Codex's findings only — treat as one grounded opinion, not multi-model convergence.
---

# Cross-AI Plan Review — Phase 3 (Alerts Parser Coverage)

## Codex Review (gpt-5.4-mini)

**Summary**

The two-wave plan is structurally sound: it uses the same extract-for-testability pattern already established in `air_quality_aqicn.rs`, keeps network decode paths intact so existing error conversion stays unchanged, and targets the real parser seams in `alerts.rs` rather than trying to mock live providers. The main risk is not the refactors themselves, but whether the proposed test surface and the ECCC contingency are enough to guarantee the `>=65%` `alerts.rs` coverage bar once the unresolved live-resolver and fetch scaffolding are accounted for.

**Strengths**

- `03-01` is mechanically low risk. It moves only the inline transform bodies out of the async fetch wrappers in `alerts.rs` and preserves the existing decode/error boundaries, which matters because `error.rs` already maps `reqwest::Error` and `quick_xml::DeError` for the current fetch paths. [src/error.rs](=/data/code/weathervane/src/error.rs#L67) [src/error.rs](=/data/code/weathervane/src/error.rs#L91)
- The plan follows an existing in-repo precedent almost exactly: `extract_aqi` in `air_quality_aqicn.rs` exists specifically so fixture-based tests can avoid live network calls. [src/air_quality_aqicn.rs](=/data/code/weathervane/src/air_quality_aqicn.rs#L33)
- The refactor targets the actual inline hotspots in `alerts.rs`: NWS parsing at `fetch_nws_alerts`, MeteoAlarm feed mapping at `fetch_meteoalarm_alerts`, and BOM parsing at `fetch_bom_alerts`. [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L107) [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L258) [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L596)
- `03-02` correctly leans on the already pure parser functions, especially `parse_meteoalarm_entry` and `parse_eccc_cap`, instead of trying to force live HTTP coverage. [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L304) [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L492)
- The region-routing part is grounded in the existing geofence tests in `geo.rs`, and the plan uses the same representative coordinates already proven there. [src/geo.rs](=/data/code/weathervane/src/geo.rs#L399) [src/geo.rs](=/data/code/weathervane/src/geo.rs#L428)
- The wire contract boundary is respected: `Alert` and `AlertSeverity` are already guarded by snapshot tests in `tests/wire_contract.rs`, and the plan does not ask to touch those shapes. [tests/wire_contract.rs](=/data/code/weathervane/tests/wire_contract.rs#L283)

**Concerns**

- **HIGH:** The `03-02` contingency looks too narrow to guarantee the coverage target if the first-pass tests come in low. It only adds pure ECCC HTML-listing helpers from the directory-scraping loops in `alerts.rs`, but the file still contains large untestable live-resolver / fetch scaffolding in `resolve_user_emma_id`, `fetch_eccc_alerts`, `fetch_nws_alerts`, `fetch_meteoalarm_alerts`, and `fetch_bom_alerts`. If `llvm-cov` ends up being more conservative than expected, the contingency may still leave the file below `65%`. [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L203) [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L395) [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L596) [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L419) [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L451)
- **MEDIUM:** The dispatch story is a little overstated. The offline test can only prove `Region::Unknown => Ok(vec![])` in `fetch_alerts`; the US/Europe/Canada/Australia arms still cannot execute without live HTTP, so the plan’s “dispatch proven” language really means “route selection is validated via `detect_region`, and the only offline branch in `fetch_alerts` is covered.” [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L65) [src/geo.rs](=/data/code/weathervane/src/geo.rs#L24)
- **MEDIUM:** The MeteoAlarm namespace check is still a fixture trap until it’s explicitly recorded. `MeteoAlarmEntry` uses plain `#[serde(rename = "...")]` fields with no namespace handling, so the plan should make the actual binding form visible in the summary and not treat “prefixed vs unprefixed” as an afterthought. [src/alerts.rs](=/data/code/weathervane/src/alerts.rs#L180)
- **LOW:** The proposed parser tests leave a few fallback paths unexercised, such as BOM `short_title` / `warning_type` missing behavior and some NWS fallback cases. That’s not fatal for this milestone, but it leaves small regressions unguarded.
- **LOW:** The plan relies on `cargo llvm-cov` reporting the same way it did during research, but the current environment check was blocked by the local `sccache` wrapper, so the threshold math is still an assumption rather than a live confirmation.

**Suggestions**

- Make the `03-02` contingency broader, or measure after `03-01` and again after the XML tests instead of waiting until the very end. If coverage is still short, the next extraction should probably be around the resolver or other pure parsing seams, not just the ECCC listing helpers.
- Reword the dispatch claim in `03-02` to say “proxy validation of routing” unless you add an injectable seam for the live provider branches.
- In `03-02`, explicitly record the working MeteoAlarm namespace form in the summary and keep the test name/assertions tied to that observed behavior.
- If the team cares about production-only coverage, track that separately from the workspace/file total, because in-file `#[cfg(test)]` modules will materially affect the llvm-cov number.
- Add one or two small fallback assertions if you want a bit more protection around `Alert` shape defaults, especially for BOM and NWS.

**Risk Assessment**

**Medium.** The implementation changes themselves are low-risk and the source already has the right test seams, so this is not a fragile refactor. The risk is concentrated in the coverage objective: large parts of `alerts.rs` are inherently network-bound, and the planned contingency may not be enough if the coverage bar depends more heavily on untestable production code than expected.

---

## Consensus Summary

Single-reviewer run (Codex); the items below are Codex's grounded findings, not consensus across reviewers.

### Strengths (Codex-attested, with file:line evidence)

- The extraction pattern in 03-01 mirrors an in-repo precedent — `extract_aqi` in `src/air_quality_aqicn.rs:33` — so the refactor is low-risk.
- The refactor targets real inline hotspots in `src/alerts.rs`: NWS parse at `fetch_nws_alerts`, MeteoAlarm feed mapping at `fetch_meteoalarm_alerts`, BOM parse at `fetch_bom_alerts`.
- 03-02 tests already-pure parser functions (`parse_meteoalarm_entry`, `parse_eccc_cap`) instead of forcing live HTTP coverage.
- Wire contract boundary respected — `Alert` / `AlertSeverity` snapshot-guarded at `tests/wire_contract.rs:283`; plan does not touch those shapes.
- Region-routing grounded in existing geofence tests in `src/geo.rs` with the same representative coordinates.

### Concerns

- **HIGH — Contingency may be too narrow to guarantee ≥65%.** The ECCC HTML-listing extraction is the only fallback, but `src/alerts.rs` still contains large untestable live-resolver / fetch scaffolding: `resolve_user_emma_id`, `fetch_eccc_alerts`, `fetch_nws_alerts`, `fetch_meteoalarm_alerts`, `fetch_bom_alerts`. If llvm-cov comes in more conservative than research suggested, the current contingency may leave `src/alerts.rs` below the 65% bar.
- **MEDIUM — Dispatch claim overstated.** Offline tests can only prove `Region::Unknown => Ok(vec![])` in `fetch_alerts` (`src/alerts.rs:65`) + `detect_region` routing selection (`src/geo.rs:24`). US/Europe/Canada/Australia dispatch arms cannot execute without live HTTP; plan should downgrade "dispatch proven" language to "routing proven; live arms are an accepted, documented gap".
- **MEDIUM — MeteoAlarm namespace: fixture trap risk.** `MeteoAlarmEntry` (`src/alerts.rs:180`) uses plain `#[serde(rename = "...")]` with no namespace handling. Plan 03-02 Task 1 should record the observed working binding form in the summary rather than treating "prefixed vs unprefixed" as an afterthought.
- **LOW — Fallback paths unexercised.** Missing tests around BOM `short_title`/`warning_type` missing behavior and some NWS fallback cases. Not fatal for the milestone.
- **LOW — llvm-cov threshold math still assumed.** Codex could not live-verify llvm-cov output because the local `sccache` wrapper blocked its measurement attempt. Numbers are still research assumption, not measurement.

### Suggestions

- Broaden the 03-02 contingency, OR measure coverage after Wave 1 (post-helper-extraction) and again after XML tests, not only at the end. Earlier reads let you pivot if the target is missed.
- If coverage falls short, extract additional pure parsing seams (e.g. around the resolver) rather than only the ECCC listing helpers.
- Reword 03-02's dispatch claim to "proxy validation of routing" — or add an injectable seam for the live provider branches (out of scope for this milestone).
- Explicitly record the working MeteoAlarm namespace form in 03-02's summary; tie test names/assertions to the observed form.
- Track production-only coverage separately from the workspace/file total — `#[cfg(test)]` in-file modules materially affect the llvm-cov number.
- Add small fallback assertions for `Alert` shape defaults on BOM and NWS if extra regression protection is desired.

### Overall Risk (Codex)

**Medium.** Refactor mechanics are low-risk; risk concentrates in the coverage target itself because much of `src/alerts.rs` is network-bound and the contingency may not be sufficient if the coverage bar depends more heavily on untestable production code than expected.

### Divergent Views

N/A — single reviewer.
