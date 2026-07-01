# External Integrations

**Analysis Date:** 2026-06-30

## APIs & External Services

**Weather & AQI (Primary):**
- **Open-Meteo** - Weather, air quality (pollutants), pollen forecasts
  - SDK/Client: `reqwest` HTTP GET
  - URL: `https://api.open-meteo.com/v1/forecast`
  - Auth: None (free API, no key required)
  - Response: JSON arrays of hourly/daily forecasts
  - Used by: `weather::fetch_weather()`, `air_quality::fetch_air_quality()`, `pollen::fetch_pollen()`
  - Files: `src/weather.rs`, `src/air_quality.rs`, `src/pollen.rs`

- **Open-Meteo Geocoding** - City name to coordinates lookup
  - SDK/Client: `reqwest` HTTP GET
  - URL: `https://geocoding-api.open-meteo.com/v1/search`
  - Auth: None
  - Response: JSON with lat/lon and admin boundaries
  - Used by: `location::search_city()`
  - Files: `src/location.rs`

**Air Quality (Optional Upgrade):**
- **aqicn.org (World Air Quality Index Project)** - Ground-station AQI for non-Europe regions
  - SDK/Client: `reqwest` HTTP GET
  - URL: `https://api.waqi.info/feed/geo:{lat};{lon}/?token={TOKEN}`
  - Auth: Free token required (issued at aqicn.org/data-platform/token/)
  - Response: JSON with `data.aqi` (US EPA scale)
  - Used by: `air_quality::fetch_air_quality()` when token provided + outside Europe
  - Files: `src/air_quality_aqicn.rs`, `src/air_quality.rs`
  - Special: Fallback-safe; returns `None` on any error, caller reverts to Open-Meteo
  - Attribution: Users must comply with aqicn data platform terms (free-software/non-commercial only)

**Weather Alerts (Regional):**
- **NWS (National Weather Service)** - United States alerts
  - SDK/Client: `reqwest` HTTP GET
  - URL: `https://api.weather.gov/alerts/active`
  - Auth: None
  - Format: GeoJSON feature collection with CAP severity levels
  - Used by: `alerts::fetch_nws_alerts()` when `detect_region()` returns `Region::Us`
  - Files: `src/alerts.rs`

- **MeteoAlarm** - European weather alerts
  - SDK/Client: `reqwest` HTTP GET
  - URL: `https://www.meteoalarm.org/` (via geohash-indexed JSON endpoints)
  - Auth: None
  - Format: GeoJSON; requires country detection and geohash encoding
  - Used by: `alerts::fetch_meteoalarm_alerts()` when Europe region detected
  - Files: `src/alerts.rs`, `src/geo.rs` (geohash encoding, polygon point-in-polygon)

- **ECCC (Environment and Climate Change Canada)** - Canadian weather alerts
  - SDK/Client: `reqwest` HTTP GET
  - URL: Office-specific URLs based on geocoding (via Nominatim)
  - Auth: None
  - Format: GeoJSON
  - Used by: `alerts::fetch_eccc_alerts()` when Canada region detected
  - Files: `src/alerts.rs`, `src/geo.rs` (ECCC office code lookup)

- **BOM (Bureau of Meteorology)** - Australian weather alerts
  - SDK/Client: `reqwest` HTTP GET
  - URL: WPS GetCapabilities + GML download per warning type
  - Auth: None
  - Format: GML/XML
  - Used by: `alerts::fetch_bom_alerts()` when Australia region detected
  - Files: `src/alerts.rs`, `src/geo.rs` (BOM metadata caching)

**Location Detection:**
- **ip-api.com** - IP-based geolocation
  - SDK/Client: `reqwest` HTTP GET
  - URL: `http://ip-api.com/json/?fields=status,lat,lon,city,regionName,country`
  - Auth: None (rate-limited for free tier)
  - Response: JSON with latitude, longitude, city, country
  - Used by: `location::detect_location()`
  - Files: `src/location.rs`

**Japan-Specific Weather:**
- **JMA AMeDAS** - Japan Meteorological Agency automatic weather stations
  - SDK/Client: `reqwest` (two endpoints: `get_text()` for timestamps, `get_json()` for station data)
  - URLs:
    - `https://www.jma.go.jp/bosai/amedas/data/latest_time.txt` (latest observation timestamp)
    - `https://www.jma.go.jp/bosai/amedas/const/amedastable.json` (station list)
    - `https://www.jma.go.jp/bosai/amedas/data/map/{timestamp}.json` (current temperatures)
  - Auth: None
  - Response: JSON station data, plain-text timestamp
  - Used by: `weather_jma::override_current_temp()` to swap Open-Meteo's current temp with ground truth for coordinates inside Japan
  - Files: `src/weather.rs` (calls override), `src/weather_jma.rs`, `src/geo.rs` (Japan bounding box detection)
  - Special: Cached station list (RwLock) survives process lifetime; fallback-safe (any failure reverts to Open-Meteo)

## Data Storage

**Databases:**
- None - Stateless library; no persistence

**File Storage:**
- None - No file I/O (caller manages persistence)

**Caching:**
- **In-process only:**
  - `client::http_client()` - Lazy-built `reqwest::Client` (static RwLock) with pooled connections, 30s idle timeout per connection
  - `weather_jma::STATIONS` - Cached JMA station list (static RwLock), persists for process lifetime
  - Both are per-process; no inter-process or disk cache

**Resetting Cache:**
- `client::reset_http_client()` - Public function to clear HTTP client (e.g., on network issues)
- No manual reset for JMA station cache (retried on every call if stale)

## Authentication & Identity

**Auth Provider:**
- None for weather/AQI/alerts/location APIs (all public endpoints)
- Optional token: aqicn.org requires a free token (caller supplies)

**Secrets/Token Handling:**
- aqicn token passed as URL query parameter by caller; never stored by crate
- No credential management; no .env support

## Monitoring & Observability

**Logging:**
- Framework: `tracing` 0.1 (structured logging)
- Debug-level: API errors, network failures, optional provider fallback attempts
- Info-level: D-Bus connections established, network/sleep event stream listening
- Warn-level: D-Bus failures (NetworkManager/systemd-logind unavailable)
- No error-tracking service integration

## D-Bus Services (Linux Only)

**NetworkManager Connectivity Monitoring:**
- Service: `org.freedesktop.NetworkManager`
- Interface: `org.freedesktop.NetworkManager`
- Signal: `StateChanged`
- Used by: `network::network_stream()` - yields `NetworkEvent::Connected` on full connectivity restoration
- Files: `src/network.rs`
- Fallback: Returns idle stream if D-Bus unavailable

**systemd-logind Sleep Monitoring:**
- Service: `org.freedesktop.login1`
- Interface: `org.freedesktop.login1.Manager`
- Signal: `PrepareForSleep`
- Used by: `sleep::sleep_stream()` - yields `SleepEvent::Resumed` when system wakes from suspend
- Files: `src/sleep.rs`
- Fallback: Returns idle stream if D-Bus unavailable

## Environment Configuration

**Required env vars:**
- None - All configuration passed as function parameters (coordinates, units, aqicn token)

**Secrets location:**
- aqicn token passed at call time, not from environment

**Platform-specific behavior:**
- Non-Linux: `network_stream()` and `sleep_stream()` return idle (no events yield)
- All weather/AQI/alerts functions work on any platform

## Webhooks & Callbacks

**Incoming:**
- None - Library does not expose endpoints or listeners

**Outgoing:**
- None - Pure synchronous async functions; no callbacks or event publishing outside the crate

## Network Behavior

**HTTP Client Configuration:**
- Per-request timeout: 15 seconds
- TCP keepalive: 30 seconds (detect dead peers early)
- Connection pool idle timeout: 30 seconds
- Pool max idle per host: 0 (no persistent idle connections per host after timeout)
- User-Agent: `(weathervane, https://gitlab.com/vintagetechie/weathervane)`

**Resilience:**
- All optional provider integrations (AMeDAS, aqicn) swallow errors and return `None`; caller handles fallback
- Required providers (Open-Meteo, NWS/MeteoAlarm/ECCC/BOM alerts) propagate errors via `Result<T>` enum
- D-Bus failures degrade gracefully: streams become idle, do not panic

---

*Integration audit: 2026-06-30*
