## App Identifiers

All new app/service identifiers use the `dev.jcrenshaw` namespace (reverse-DNS of
jcrenshaw.dev) — the older vintagetechie branding is retired for new surfaces:

- Flatpak app ID / D-Bus bus name: `dev.jcrenshaw.Weathervane`
- D-Bus interface: `dev.jcrenshaw.Weathervane1`
- Future apps: `dev.jcrenshaw.<AppName>`

Exception: cosmic-ext-applet-tempest keeps its existing identifiers (baked into
installs/AUR); it links this crate directly and doesn't touch the new namespace.

## Subagent Model Routing

When dispatching subagents, always set `model` explicitly — never rely on inherit:

- `haiku` — mechanical impl (1-2 files, complete spec)
- `sonnet` — integration, multi-file, judgment calls
- `opus` — architecture, design, final review

## What this is

Reactive weather/AQI/alerts/pollen domain crate (no UI, no polling, no timers,
no config storage). Async fns in, clean Rust types out; the frontend owns all
scheduling and persistence. Consumed by atmos and cosmic-ext-applet-tempest.

## Wire contract — read before touching serde types

Public types crossing process boundaries are frozen. Changing a serialized
shape breaks `tests/wire_contract.rs` (insta snapshots in `tests/snapshots/`)
and is a breaking change. See CONTRACT.md; Rust API ref in API.md.
Accept intended changes with `cargo insta review`.

## Build & gates (match CI order)

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo build --workspace
cargo test --workspace
```
