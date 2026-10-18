# weathervane security rules

## Threat model in short

weathervane is a Rust library, not a program. The cosmic-ext-applet-tempest panel applet links it and calls async functions with coordinates, units, a city name or an aqicn token; the crate makes HTTPS GETs to public weather, air-quality, geocoding and alert providers, parses the responses and returns Rust types. It reads and writes no files, runs no programs, has no `unsafe`, owns no D-Bus name, and only listens for two system-bus signals (NetworkManager `StateChanged`, logind `PrepareForSleep`). Everything runs in the applet's process as the desktop user.

Every network response is untrusted input. The assets are the user's location, the aqicn token, the applet's uptime, and correct alert state. The worst outcome is a real alert for the user's area dropped or a fake one shown while the result claims to be filtered to their area. Full model: `THREAT-MODEL.md`.

## Review checklist

### Things the crate must never do
- Touch the filesystem in `src/`. Flag any new `std::fs`, temp file, cache directory or path handling.
- Run a process (`std::process`, `Command`) or add `unsafe`.
- Call D-Bus methods other than `AddMatch`, own a bus name, or export an interface.
- Send coordinates to a provider that doesn't need them. ECCC, MeteoAlarm, JMA and the codename table get none today; BOM gets only a 6-character geohash.

### Location and the aqicn token
- Error payloads never carry search text or coordinates (CONTRACT.md PII rule). `Error::NoResults { query }` keeps the query out of `Display` and `WireError`; a new variant or `Display` impl must keep that. `From<reqwest::Error>` must keep mapping to payload-free variants.
- Logs are coordinate-free. Flag any new `tracing` call that formats latitude, longitude, a Nominatim place name, a full request URL, or a station ID. Known existing sites: `src/geo.rs:331`, `src/alerts.rs:510`, `:515`, `:709`, `:712`, `:926`, `:971`, `src/weather_nws.rs:144`.
- Any reqwest error logged or stringified must go through `.without_url()`. A bare `{}` or `{:?}` of a `reqwest::Error` whose request URL carries coordinates or the token is a leak.
- The aqicn token goes only to `api.waqi.info`, URL-encoded, and nowhere else: not logs, errors, caches, or another host. Keep the `src/client.rs` sentinel leak tests passing and extend them when a new path handles the token.

### Where untrusted data enters
- Response data must never pick the host of a later request. Known exception: `src/weather_nws.rs:162` fetches the station-list URL from the NWS `/points/` response as given. Flag any new place that does the same, and any change there that widens it.
- Response data put into a URL path must be validated or encoded first. Existing patterns: the AMeDAS timestamp must be 14 digits; ECCC hour hrefs must be two digits and a slash; NWS station IDs and ECCC `.cap` hrefs are appended unencoded under a fixed host prefix.
- `detect_location` uses plain HTTP to ip-api.com. Its reply can be forged by anyone on the path; keep the range and finite checks on lat/lon, and do not trust its strings for anything but display.
- No response body is size-limited; only the 15 second client timeout bounds it. Flag new full-body reads on paths that loop over many requests (the ECCC CAP walk has no cap on file count).
- XML (MeteoAlarm atom, ECCC CAP) goes through `quick_xml::de::from_str`. Flag any change that enables DTD or entity handling, or that adds manual recursion over XML events.
- Parsers must not panic on hostile input: no indexing, slicing or `unwrap` on response-derived data without a bounds check; use `.get()`. Byte-index slicing of strings must land on char boundaries. Float parsing must tolerate NaN and infinity (`(-90.0..=90.0).contains(&x)` rejects both).
- Alert text (`event`, `headline`, `description`, `area_desc`, `id`) is passed through unescaped and unbounded. That is the contract; the frontend escapes. Flag any change that formats it into markup, a URL or a command inside the crate.
- D-Bus loops check member and interface but not sender. A spoofed signal must stay harmless: it may only trigger an early refresh.
- In-process caches (NWS stations, AMeDAS table, Nominatim address, MeteoAlarm codenames) keep a successful response for the process lifetime. A change that caches failures, or caches data keyed by something an attacker can vary without limit, needs a reason.

### Alert correctness
- `region_filtered: true` must mean the entries were narrowed to the caller's area. Flag any path that returns unfiltered or wrongly filtered entries with it set.
- An alert filter with an active EMMA_ID must drop untagged entries, never pass them through.
- Changes to region bounding boxes in `src/geo.rs` decide which provider a point uses; check the borders they name (US-Canada, Japan's west edge against Korea and Taiwan).
- The MeteoAlarm codename table comes from a third party's GitHub `master` branch. Flag any change that makes more decisions depend on it.

### Wire contract
- Serialized public types are frozen (`tests/wire_contract.rs`, `tests/snapshots/`). A security fix that changes a serialized shape is a breaking change and must say so.
