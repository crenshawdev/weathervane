# Threat model: weathervane

Version or commit this describes: v0.13.0 plus `a9bd292` (quick-xml 0.42)
Last reviewed: 2026-10-17

weathervane is a library crate, not a program. It has no binary, no `main`, no config file, no data directory and no build script. Its one consumer, the cosmic-ext-applet-tempest COSMIC panel applet, links it, calls its async functions with coordinates, units, a city name or an aqicn token, and gets Rust types back. Everything below happens inside the applet's process, with the applet's privileges. "Frontend" below means tempest or any later consumer.

## What this tool reads

- Files: none. Nothing in `src/` touches the filesystem. The only file reads are in `tests/wire_contract.rs`, which reads its own snapshots under `tests/snapshots/`.
- Arguments from the caller: latitude and longitude (`f64`), `TemperatureUnit`, `MeasurementSystem`, a city name for `search_city`, and an optional aqicn token for `fetch_air_quality`.
- Environment variables: none read by the crate's own code. Its dependencies read some. reqwest is built with its `system-proxy` feature, so it honours `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` and the system proxy settings [VERIFY: exact variables and sources in reqwest 0.13.5]. zbus reads `DBUS_SYSTEM_BUS_ADDRESS` when it connects to the system bus [VERIFY].
- Network, all through one shared reqwest client (`src/client.rs:31`) with a 15 second per-request timeout, rustls with the platform verifier, and a fixed User-Agent:

| Endpoint | Scheme | Called from | What the request carries |
|---|---|---|---|
| `api.open-meteo.com/v1/forecast` | https | `fetch_weather` | coordinates, units |
| `air-quality-api.open-meteo.com/v1/air-quality` | https | `fetch_air_quality`, `fetch_pollen` | coordinates |
| `geocoding-api.open-meteo.com/v1/search` | https | `search_city` | the city name, URL-encoded |
| `ip-api.com/json/` | plain http | `detect_location` | nothing but the caller's public IP |
| `api.waqi.info/feed/geo:` | https | `fetch_air_quality` with a token, outside Europe | coordinates and the aqicn token in the query string |
| `api.weather.gov/points/`, `/stations/.../observations/latest`, `/alerts/active` | https | US weather override, US alerts | coordinates (points rounded to 4 decimals) |
| the station-list URL named in the `/points/` response | whatever the response says | `src/weather_nws.rs:162` | nothing added |
| `www.jma.go.jp/bosai/amedas/...` | https | Japan temperature override | no coordinates; whole tables are fetched |
| `nominatim.openstreetmap.org/reverse` | https | Europe alerts | coordinates |
| `raw.githubusercontent.com/ktrue/Meteoalarm-warning/master/meteoalarm-codenames.json` | https | Europe alerts | nothing |
| `feeds.meteoalarm.org` (atom feed and v1 JSON) | https | Europe alerts | the country slug from a fixed table |
| `dd.weather.gc.ca/today/alerts/cap/` | https | Canada alerts | today's date and a regional office code, no coordinates |
| `api.weather.bom.gov.au/v1/locations/{geohash}/warnings` | https | Australia alerts | a 6-character geohash of the coordinates |

- D-Bus: `network_stream` (`src/network.rs:42`) and `sleep_stream` (`src/sleep.rs:40`) connect to the system bus and subscribe to NetworkManager `StateChanged` and logind `PrepareForSleep` signals. They call nothing else on the bus.
- Nothing owned by another user, project or tool is read.

## What it writes

- Files: none. No temp files, no caches on disk, nothing outside or inside a data directory, because it has none.
- In-memory state, process lifetime, never persisted: the shared HTTP client, the AMeDAS station table, the NWS station list per rounded point, the Nominatim address per exact coordinate, and the MeteoAlarm codename table. The per-point and per-coordinate maps have no size limit.
- Output: return values only. It prints nothing to stdout or stderr itself. It emits `tracing` events at debug, info and warn; where those go is decided by whatever subscriber the frontend installs [VERIFY: what tempest installs, and whether its stderr lands in the journal].

## What it executes

- No external programs, no shell, no `std::process`. No hooks are installed anywhere.
- No `unsafe` in the crate's own code.
- Built strings: every request URL is built with `format!`. Coordinates are `f64` formatted with `{}`, which yields only digits, `-`, `.`, `NaN` or `inf`, so they cannot inject. The city name and the aqicn token are URL-encoded. Response data flows into later URLs in four places, covered under trust boundaries: the NWS station-list URL, NWS station IDs, the AMeDAS timestamp, and ECCC directory listing hrefs.
- No SQL, no FTS, no prompts.
- Build time: no `build.rs`. Dependencies' proc macros (serde, thiserror, zbus, async-stream) run at build time on the developer's or CI's machine.

## What it must never touch

- The filesystem. The crate reads and writes no files, and adding `std::fs`, a temp file or a cache directory to `src/` changes that design.
- Precise location in logs or errors. CONTRACT.md's PII rule: error payloads (`kind`, `message`) never contain search query text or coordinates, enforced by tests. `Error::NoResults` holds the query but its `Display` and `WireError` drop it. The 0.9.0 log posture is coordinate-free.
- The aqicn token anywhere but the aqicn request. It must never reach tracing output, an `Error`, a `WireError` or any other host. `get_json` and `get_text` log reqwest errors with `without_url()` for this reason, and `src/client.rs` tests assert it on connect, status, body, parse and success paths.
- Coordinates going to a provider that does not need them. Today: ECCC, MeteoAlarm, JMA and the codename table get none, and BOM gets a geohash.
- Anything on D-Bus beyond listening. No method calls besides `AddMatch`, no owned bus name, no exported interface. The future daemon's `dev.jcrenshaw.Weathervane` name is not in this crate.
- Actions on the user's behalf: none. It makes only the GET requests listed above, and only when the frontend calls a function.

## Who runs it and with what privileges

- The interactive desktop user, inside the tempest COSMIC panel applet's process. [VERIFY: whether the installed tempest runs in a Flatpak sandbox (AUR builds do not), which would cap the blast radius.]
- Started by the frontend at login or by hand. The crate has no timers; the frontend decides when to call.
- That user can already reach the network, the system bus and their own files. A bug in weathervane is an escalation only if it lets remote data make the frontend do something the user did not ask for: leak location, crash or hang the panel, or show false alert state.

## What the worst outcome looks like

- The worst outcome is wrong alert state shown as right: a tornado or flood warning that applies to the user is dropped while the result still says `region_filtered: true`, or a fake one is shown. This can come from a parsing or filtering bug, from the bounding boxes in `src/geo.rs` putting a point in the wrong region, or from a third-party data source. The MeteoAlarm codename table is fetched from another person's GitHub `master` branch; if it maps the user's place to the wrong EMMA_ID, stage 1 filtering returns that other region's alerts, and a quiet result looks the same as a quiet day.
- Second: the user's location leaking to logs, to an on-path observer, or to a provider that does not need it.
- Third: remote data crashing or hanging the frontend. A panic in a parser takes the panel applet down with it, and a huge response body is read fully into memory.
- What the user would not notice: an alert silently filtered out, a spoofed `detect_location` result over plain HTTP (the forecast and alerts would just be for somewhere else), and location-bearing debug or warn lines in the journal.

## Trust boundaries (where untrusted data enters)

Every network response is untrusted. HTTPS protects the transport for all endpoints but one; it says nothing about whether the provider's data is well formed.

- `detect_location` over plain HTTP (`src/location.rs:101`). Anyone on the path (hotel or café Wi-Fi) can read the reply and forge it. Validation: status must be `success`, latitude and longitude must be in range and finite. Not validated: the display strings, and the body size. [VERIFY: whether ip-api offers HTTPS on the free tier.]
- JSON bodies (Open-Meteo, NWS, BOM, aqicn, Nominatim, the codename table, MeteoAlarm v1 JSON) are decoded with serde into typed structs or `serde_json::Value`. serde_json's default recursion limit applies. No size limit on any body; only the 15 second timeout bounds it.
- XML bodies: the MeteoAlarm atom feed (`src/alerts.rs:780`) and ECCC CAP files (`src/alerts.rs:1212`) go through `quick_xml::de::from_str`. [VERIFY: that quick-xml 0.42's serde deserializer does no DTD or custom entity expansion, and how deep element nesting it tolerates before overflowing the stack.]
- ECCC directory listings are HTML scraped line by line for `href="..."`. Hour hrefs must be two ASCII digits and a slash. CAP file hrefs only need to end in `.cap` and are appended to the hour URL as a string (`src/alerts.rs:1177`), so the host stays `dd.weather.gc.ca` but the path is the listing's choice. The number of CAP files fetched per refresh has no cap.
- NWS: the `/points/` response names the station-list URL, and `src/weather_nws.rs:162` fetches it as given, any host, any scheme reqwest accepts. Station IDs from that list are put into a path under `api.weather.gov/stations/` without encoding (`src/weather_nws.rs:137`); the host cannot change, the path can.
- AMeDAS: the `latest_time.txt` timestamp is checked to be 14 digits before it goes into a URL. Station coordinates are range-checked.
- ECCC polygons are parsed from CAP text in `point_in_polygon` (`src/geo.rs:107`); unparseable vertices are dropped and fewer than three means no match.
- Alert text (`event`, `headline`, `description`, `area_desc`, `id`) is passed through unmodified and with no length limit. The crate does no escaping. The frontend must treat it as plain text; freedesktop notification bodies accept a markup subset, so passing alert text into one unescaped is markup injection from a remote feed. [VERIFY: how tempest renders alert text and notification bodies.]
- D-Bus signals: the match rules name the sender, but the message loop checks only member and interface, not sender or path. A spoofed signal can only cause a spurious `Connected` or `Resumed` event, which makes the frontend refresh early. [VERIFY: whether the system bus policy lets another local process deliver a matching signal to this connection.]
- Caller input: coordinates are not range-checked by the fetch functions; the city name is URL-encoded; the token is URL-encoded and blank tokens skip the request.
- Data trusted because the crate produced it earlier: the in-memory caches. A bad response cached once (an NWS station list, the AMeDAS table, a Nominatim address, the codename table) stays for the life of the process. Failures are not cached; successes are, even if the content was wrong.
- Redirects: reqwest's default policy follows up to 10 redirects. [VERIFY: whether it follows an https to http downgrade, which would turn any endpoint into a plain-HTTP one for that request.]

## Secrets that may pass through

- The aqicn token. The frontend owns storage; the crate holds it only for the duration of `fetch_air_quality`. It travels in the query string of the `api.waqi.info` request, so it is visible to waqi and to anything that logs full URLs (a proxy set through `HTTPS_PROXY` sees only the host, not the query, over TLS). It is kept out of tracing by `without_url()` and out of `Error` because `From<reqwest::Error>` maps to payload-free variants. The tests in `src/client.rs` guard this.
- Location, which is the user's most sensitive data here even though it is not a credential. Where it currently ends up in logs:
  - `src/geo.rs:331` logs the exact coordinates at debug on a reverse-geocode cache hit.
  - `src/alerts.rs:515`, `:926` and `:971` log the Nominatim place names (city, town, village, county, state) at warn when MeteoAlarm matching misses.
  - `src/alerts.rs:709` and `:712` log the matched place name at debug; `:510` logs the resolved EMMA_ID at debug.
  - `src/weather_nws.rs:144` logs the NWS station ID at debug, which places the user within a few kilometres.
  - [VERIFY: whether these are intended exceptions to the coordinate-free log posture or gaps in it.]
- The user's public IP goes to ip-api.com in clear text, and the location it returns is visible to anyone on the path.
- The crate holds no secrets of its own. Releases publish to crates.io from `.github/workflows/release.yml` through crates.io OIDC in the `release` environment, so no long-lived registry token is stored in the repo.
