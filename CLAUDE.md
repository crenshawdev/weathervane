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
