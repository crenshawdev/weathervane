# Security

## Reporting a vulnerability

Report it privately through GitHub: open the repository's Security tab and choose "Report a vulnerability", or go straight to <https://github.com/crenshawdev/weathervane/security/advisories/new>. The report stays private until an advisory is published. Please don't open a public issue for a vulnerability.

## What runs on every pull request

Only collaborators can open pull requests. Each one runs the `CI` workflow.

`CI` ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` and a 75 percent line-coverage gate. Any failure fails the pull request.
