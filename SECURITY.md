# Security

## Reporting a vulnerability

Report it privately through GitHub: open the repository's Security tab and choose "Report a vulnerability", or go straight to <https://github.com/crenshawdev/weathervane/security/advisories/new>. The report stays private until an advisory is published. Please don't open a public issue for a vulnerability.

[THREAT-MODEL.md](THREAT-MODEL.md) describes what weathervane sends, what it parses and what it trusts. It is the quickest way to check whether something is in scope.

## What runs on every pull request

Only collaborators can open pull requests. Each one runs two workflows.

`CI` ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` and a 75 percent line-coverage gate. Any failure fails the pull request.

`sentinel-fast` ([.github/workflows/fast.yml](.github/workflows/fast.yml)) has two jobs.

- `mechanical` runs `.sentinel/mechanical/check.sh` with the gating tools: `cargo audit`, `cargo deny check` against [deny.toml](deny.toml), `cargo clippy` with the pedantic and nursery lints, and `semgrep` with the rulesets in `.sentinel/mechanical/semgrep.yml`. A failing gate fails the pull request. Clippy fails only on errors and deny-level lints; pedantic and nursery lints are warnings. The report-only checks are not installed on the runner and are skipped there.
- `security-review` has Claude (`claude-opus-5-5`, high effort) review the pull request's diff against THREAT-MODEL.md with read-only tools. The repository is public, so the review is encrypted to the maintainer's age key and kept as the `security-review` artifact for 14 days, and the pull request comment shows only the count of findings by severity. Findings do not fail the pull request. The job fails when the review did not run, or when its output holds neither findings nor a "No findings" marker, so a passing check means a review happened. Dependabot pull requests skip this job because GitHub gives them no Actions secrets; the `mechanical` job's dependency checks still run on them.

## The full audit

The full audit runs on the maintainer's machine through `/snt:audit`, not in CI. It needs a Claude Code session, and its findings must stay off public CI until they are fixed. It runs three steps, stopping after each:

1. Mechanical checks. `.sentinel/mechanical/check.sh .` runs every check that applies, including the report-only ones CI skips: `cargo geiger`, and `cargo +nightly fuzz build` over the targets in [fuzz/](fuzz/). Miri runs only when the crate has `unsafe` code, and it has none. `cargo vet` runs only when a `supply-chain/` directory exists, and this repository has none. The output is saved to `audit/check.txt`.
2. A Claude Security codebase scan of the whole repository. Its report is copied to `audit/claude-security.md`. Its suggest-patches job is never run.
3. A findings log, one row per step, in `audit/findings-log.md`.

The audit only finds and records. It fixes nothing, writes no tests and commits nothing; what happens to each finding is decided afterwards. `audit/` and `CLAUDE-SECURITY-*/` are in `.gitignore`, because they hold findings that are not fixed yet.

The workflows, scripts and audit steps come from [sentinel](https://github.com/jcrenshaw-dev/sentinel), pinned as a git submodule at `.sentinel`.
