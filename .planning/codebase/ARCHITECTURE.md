<!-- refreshed: 2026-06-30 -->
# Architecture

**Analysis Date:** 2026-06-30

## System Overview

Weathervane is a purely reactive Rust crate for weather, air quality, alerts, and pollen data. It exposes four independent async fetch functions and supporting utilities. No polling, no timers, no config storage — frontends call functions when they decide to, pass all parameters (coordinates, units), and get back typed results. Frontends own scheduling and persistence.

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                      Public API Layer (lib.rs)                          │
│  ┌──────────────┬──────────────┬──────────────┬──────────────┐          │
│  │ fetch_weather│fetch_air_    │fetch_alerts  │fetch_pollen │          │
│  │              │quality       │              │              │          │
│  └──────────────┴──────────────┴──────────────┴──────────────┘          │
│                   + Location, Units, Time, Network, Sleep APIs          │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────────────────────┐
│                         Domain Layer                                     │
│  ┌─────────────────────┐  ┌──────────────────┐  ┌─────────────────┐    │
│  │ Weather Domain      │  │ Air Quality      │  │ Alerts Domain   │    │
│  │ `weather.rs` (258)  │  │ `air_quality.rs` │  │ `alerts.rs` (658)   │
│  │ Open-Meteo API      │  │ Open-Meteo +     │  │ Regional dispatch   │
│  │ + JMA AMeDAS        │  │ optional AQICN   │  │ (NWS, MeteoAlarm,   │
│  │ override            │  │                  │  │ ECCC, BOM)          │
│  └─────────────────────┘  └──────────────────┘  └─────────────────┘    │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │ Pollen Domain              Location Domain      Support APIs    │    │
│  │ `pollen.rs` (177)          `location.rs` (160)  `units.rs` (197)    │
│  │ Open-Meteo (EU only)       Geocoding + detect   `codes.rs` (206)    │
│  │ Returns Option<>           `time.rs` (183)                      │    │
│  │                            Format, parse, detect night          │    │
│  └─────────────────────────────────────────────────────────────────┘    │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────────────────────┐
│                    Integration Layer                                    │
│  ┌──────────────────────┐  ┌──────────────────────────────────────┐    │
│  │ Geographic Detection │  │ Provider Adapters                    │    │
│  │ `geo.rs` (502)       │  │ `weather_jma.rs` (279) - JMA AMeDAS  │    │
│  │ Region + polygon     │  │ `air_quality_aqicn.rs` (91) - AQICN  │    │
│  │ queries for alerts   │  │                                      │    │
│  └──────────────────────┘  └──────────────────────────────────────┘    │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────────────────────┐
│                       Wire Contract Layer                               │
│                        `wire.rs` (92)                                   │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │ Envelope<T>: data + fetched_at + error                          │   │
│  │ WireError: kind + message (PII-safe, no URLs/queries)           │   │
│  │ EnvelopeError: kind + message + timestamp                       │   │
│  │ Frozen by tests/wire_contract.rs (insta snapshots)              │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────────────────────┐
│                  Foundation Layer                                       │
│  ┌──────────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │ HTTP Client      │  │ Error Handling   │ D-Bus Streams            │  │
│  │ `client.rs`      │  │ `error.rs` (108) │ `network.rs` (78)       │  │
│  │ (101)            │  │ Typed errors,    │ `sleep.rs` (76)         │  │
│  │ reqwest with     │  │ wire mapping     │ NetworkManager, logind  │  │
│  │ hardened         │  │ PII filtering    │ yields Connected/Resumed│  │
│  │ defaults         │  │                  │                         │  │
│  └──────────────────┘  └──────────────────┘  └──────────────────────┘  │
│                                                                          │
│  reqwest (HTTP pooling, TCP keepalive, connection timeout)             │
│  zbus (D-Bus message streams)                                          │
│  serde/serde_json (JSON serialization for wire contract)               │
│  quick-xml (XML parsing for ECCC alerts)                               │
└──────────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **fetch_weather** | Open-Meteo weather + JMA override (Japan) | `src/weather.rs` |
| **fetch_air_quality** | Open-Meteo AQI + optional AQICN headline | `src/air_quality.rs` |
| **fetch_alerts** | Regional dispatch (NWS/MeteoAlarm/ECCC/BOM) | `src/alerts.rs` |
| **fetch_pollen** | European pollen concentrations | `src/pollen.rs` |
| **Geographic detection** | Region/country/polygon lookup for routing | `src/geo.rs` |
| **JMA AMeDAS** | Japan temperature quality upgrade | `src/weather_jma.rs` |
| **AQICN fallback** | Optional AQI headline provider | `src/air_quality_aqicn.rs` |
| **Location services** | City search, IP geolocation, saved locations | `src/location.rs` |
| **Units/Codes** | Temperature, pressure, weather conditions, compass | `src/units.rs`, `src/codes.rs` |
| **Time utilities** | Format, parse, detect sunrise/sunset | `src/time.rs` |
| **HTTP client** | Shared reqwest client with hardened defaults | `src/client.rs` |
| **Error handling** | Categorized errors, wire mapping, PII filtering | `src/error.rs` |
| **Wire contract** | Frozen JSON shapes for daemon/CLI | `src/wire.rs` |
| **D-Bus streams** | Network connectivity, sleep/resume detection | `src/network.rs`, `src/sleep.rs` |

## Pattern Overview

**Overall:** Message-passing reactive crate with regional dispatch and optional quality upgrades.

**Key Characteristics:**
- **Stateless:** All functions are pure async callables; no internal state or persistence
- **Typed errors:** Failures categorized for caller reaction (timeout vs network vs parse)
- **Wire frozen:** Public types crossing process boundaries locked by `tests/wire_contract.rs` (insta snapshots)
- **Regional dispatch:** Alerts and AQI standards selected by geographic region detection
- **Graceful fallback:** Optional providers (JMA, AQICN) silently fail over on any error
- **D-Bus integration:** NetworkManager and logind signal streams for frontend scheduling hints
- **PII-safe:** Error wire forms never contain URLs, queries, or coordinates

## Layers

**Public API Layer (lib.rs):**
- Purpose: Re-export async functions and types for frontend consumption
- Location: `src/lib.rs` (54 lines)
- Contains: Four async fetch functions, type re-exports, documentation examples
- Depends on: All domain and support modules
- Used by: Frontend applications (atmos, cosmic-ext-applet-tempest)

**Domain Layer (Weather, Air Quality, Alerts, Pollen):**
- Purpose: Implement the four main weather/AQI/alert/pollen queries
- Location: `src/weather.rs`, `src/air_quality.rs`, `src/alerts.rs`, `src/pollen.rs`
- Contains: Async fetch functions, typed result structs (CurrentWeather, AirQualityData, Alert, PollenData)
- Depends on: client.rs (HTTP), geo.rs (region/country detection), error.rs (Result<T>)
- Used by: Public API layer, wire contract tests

**Provider Adapters (internal):**
- Purpose: Provider-specific parsing and fallbacks (JMA AMeDAS, AQICN)
- Location: `src/weather_jma.rs`, `src/air_quality_aqicn.rs`
- Contains: API-specific response deserialization, optional quality upgrades
- Depends on: client.rs (HTTP), units.rs (unit conversion)
- Used by: weather.rs, air_quality.rs (only on success, silent fallback on fail)

**Integration Layer (Location, Units, Geographic Detection):**
- Purpose: Cross-cutting services used by multiple domains
- Location: `src/location.rs`, `src/units.rs`, `src/geo.rs`
- Contains: City geocoding, saved locations, temperature/pressure conversions, region/country/polygon lookups
- Depends on: client.rs (HTTP), error.rs (Result<T>), serde (JSON/XML parsing)
- Used by: alerts.rs (region dispatch), air_quality.rs (standard selection), weather.rs (Japan detection)

**Wire Contract Layer:**
- Purpose: Freeze JSON shapes for D-Bus daemon and CLI output
- Location: `src/wire.rs` (92 lines)
- Contains: Envelope<T> wrapper (data + fetched_at + error), WireError, EnvelopeError
- Depends on: serde, error.rs (for mapping)
- Used by: Future daemon/CLI, tests/wire_contract.rs (insta snapshots)

**Foundation Layer:**
- HTTP Client: `src/client.rs` — shared reqwest with hardened defaults (TCP keepalive, connection timeout, zero idle pool)
- Error Handling: `src/error.rs` — categorized Error enum, From<reqwest::Error>, From<quick_xml::DeError>, wire mapping
- D-Bus Streams: `src/network.rs`, `src/sleep.rs` — async streams via zbus for system events
- Utilities: `src/codes.rs`, `src/time.rs` — WMO weather code mapping, ISO timestamp parsing

## Data Flow

### Primary Request Path (fetch_weather)

1. Frontend calls `fetch_weather(lat, lon, temp_unit, system)` (`src/lib.rs:54`)
2. weather.rs validates inputs, constructs Open-Meteo API URL with unit params
3. Hits Open-Meteo `/v1/forecast` endpoint via `http_client()` (`src/client.rs:31`)
4. Parses JSON response into OpenMeteo internal struct
5. Converts to typed WeatherData (CurrentWeather + hourly + forecast)
6. If coords inside Japan: attempts JMA AMeDAS override via `override_current_temp` (`src/weather_jma.rs:46`)
   - Fetches station table from JMA, finds nearest, retrieves latest observation
   - On any failure: silently uses Open-Meteo value (no error surfaced)
7. Returns `Result<WeatherData>` with all fields in requested units
8. On error: maps to categorized Error (Timeout, Network, HttpStatus, Parse)

### Air Quality Path (fetch_air_quality)

1. Frontend calls `fetch_air_quality(lat, lon, aqicn_token: Option)` (`src/lib.rs:46`)
2. Detects region via `detect_region(lat, lon)` in `geo.rs` (determines AQI standard)
3. Fetches pollutant data from Open-Meteo (always, regardless of headline AQI source)
4. If token provided AND region is not Europe AND request succeeds: attempts AQICN headline via `fetch_headline_aqi` (`src/air_quality_aqicn.rs`)
   - On failure: falls back to Open-Meteo headline AQI silently
5. Wraps pollutants + headline AQI into AirQualityData with source tracking (AqiSource::Aqicn or OpenMeteo)
6. Computes AqiCategory (Us or Eu) from numeric AQI
7. Returns `Result<AirQualityData>`

### Alerts Path (fetch_alerts)

1. Frontend calls `fetch_alerts(lat, lon)` (`src/lib.rs:47`)
2. Detects region via `detect_region(lat, lon)` (`src/geo.rs:24`)
3. Dispatches to provider:
   - US: NWS API GeoJSON point query (`src/alerts.rs:fetch_nws_alerts`)
   - Europe: MeteoAlarm Atom feeds + Nominatim geocoding (`src/alerts.rs:fetch_meteoalarm_alerts`)
   - Canada: ECCC CAP XML with polygon filter (`src/alerts.rs:fetch_eccc_alerts`)
   - Australia: BOM geohash lookup (`src/alerts.rs:fetch_bom_alerts`)
   - Unknown: Returns empty Vec
4. Each provider: parse XML/JSON, filter expired, map to Alert struct
5. Returns `Result<Vec<Alert>>` sorted by severity

### State Layer (Wire Envelope)

Daemon/CLI wraps any domain result in `Envelope<T>`:
```
Envelope {
  data: Some(WeatherData { ... })          // last-good, never wiped
  fetched_at: Some("2026-06-11T13:50Z")   // when data was fetched
  error: None                               // most recent attempt; None if success
}
```
On failure with stale data:
```
Envelope {
  data: Some(old_data)                      // kept across failed refresh
  fetched_at: Some(timestamp_of_last_good) // unchanged
  error: Some(EnvelopeError {
    kind: "Timeout",                        // machine-readable
    message: "request timed out",           // human-readable
    at: "2026-06-11T14:20Z"                // when attempt failed
  })
}
```

**State Management:**
- No state inside weathervane itself
- Each fetch is independent, idempotent, stateless
- Envelope invariants upheld by daemon/service layer (not this crate)
- Frontend controls caching, retry logic, refresh timing

## Key Abstractions

**Error Categorization (error.rs):**
- Purpose: Distinguish failure modes for caller reaction
- Examples: Timeout vs Network(Connect) vs HttpStatus(429) vs Parse(Json)
- Pattern: From<reqwest::Error> implements fallible mapping; Display output is PII-safe

**Region Detection (geo.rs):**
- Purpose: Route alerts/AQI to regional providers and standards
- Examples: `detect_region(45.5, -122.7)` → Region::Us
- Pattern: Bounding-box checks (continental US/Alaska/Hawaii with Canada border awareness, Europe, Canada, Australia)

**Typed Units (units.rs):**
- Purpose: Frontend-driven unit selection without string encoding
- Examples: TemperatureUnit::Fahrenheit.symbol() → "°F", .api_param() → "fahrenheit"
- Pattern: Enums with conversion methods, symmetric Serialize/Deserialize

**D-Bus Streams:**
- Purpose: Let frontends subscribe to system events for scheduling hints
- Examples: network_stream() yields NetworkEvent::Connected on NM state change, sleep_stream() yields SleepEvent::Resumed
- Pattern: async_stream via zbus, falls back to infinite pending if D-Bus unavailable

**Wire Contract Freezing:**
- Purpose: Lock JSON serialization shape for daemon/CLI compatibility
- Examples: `AqiCategory` adjacent-tagged as `{"standard": "Us", "level": "Good"}`; alerts always present, null means no data
- Pattern: insta snapshots in tests/snapshots/; test failure = breaking change

## Entry Points

**fetch_weather(latitude, longitude, temperature_unit, measurement_system):**
- Location: `src/weather.rs:102`
- Triggers: Frontend on user request or refresh timer
- Responsibilities: Query Open-Meteo, optionally override Japan temp via JMA, return typed WeatherData

**fetch_air_quality(latitude, longitude, aqicn_token):**
- Location: `src/air_quality.rs:135`
- Triggers: Frontend on user request or refresh timer
- Responsibilities: Query Open-Meteo, optionally headline via AQICN, track source, return AirQualityData

**fetch_alerts(latitude, longitude):**
- Location: `src/alerts.rs:65`
- Triggers: Frontend on user request or refresh timer
- Responsibilities: Dispatch to regional provider, parse, filter expired, return Vec<Alert>

**fetch_pollen(latitude, longitude):**
- Location: `src/pollen.rs:50`
- Triggers: Frontend on user request or refresh timer (Europe only)
- Responsibilities: Query Open-Meteo pollen, return Option<PollenData> (None outside coverage)

**detect_location():**
- Location: `src/location.rs:113`
- Triggers: Frontend first-run setup or user location reset
- Responsibilities: IP-based geolocation via ip-api.com, return DetectedLocation

**search_city(city_name):**
- Location: `src/location.rs:123`
- Triggers: Frontend location search UI
- Responsibilities: Geocoding via Open-Meteo, return Vec<LocationResult>

**network_stream():**
- Location: `src/network.rs:23`
- Triggers: Frontend during startup or subscription setup
- Responsibilities: D-Bus signal stream from NetworkManager, yield NetworkEvent::Connected

**sleep_stream():**
- Location: `src/sleep.rs:21`
- Triggers: Frontend during startup or subscription setup
- Responsibilities: D-Bus signal stream from logind, yield SleepEvent::Resumed on wake

## Architectural Constraints

- **Threading:** Fully async with tokio runtime. All functions are async callables; no blocking calls in the hot path. HTTP client uses tokio's runtime for connection pooling.
- **Global state:** Single static RwLock<Option<reqwest::Client>> in `client.rs:28` shared across all requests. JMA station cache in `weather_jma.rs:42` (immutable after first fetch). No mutable shared state otherwise.
- **Circular imports:** None; dependency graph is acyclic. lib.rs → domains → client/geo/error/wire.
- **Error safety:** All From trait impls filter PII (URLs, queries, coordinates) before wire conversion. WireError never contains request details.
- **Wire stability:** JSON shapes frozen by insta snapshots; wire format change is breaking and requires deliberate `cargo insta review` acceptance.

## Anti-Patterns

### Mutation Without Guard

**What happens:** Optional providers (JMA, AQICN) silently fall back to default on any error, including network timeouts during optional fetch.

**Why it's wrong:** Frontend has no signal that the upgrade was attempted but failed. If JMA goes down, China gets Open-Meteo temperatures without visibility.

**Do this instead:** Consume `aqi_source: AqiSource` field in AirQualityData (`src/air_quality.rs:157`) and `Envelope.error` for transparency. Frontend can log or display "using fallback" when error is present.

### Envelope Invariant Violation

**What happens:** Client code wrapping in Envelope<T> could set `data: None` while `error: None` (contradictory state).

**Why it's wrong:** Breaks the contract: "error is None iff the most recent attempt succeeded" (wire.rs:79). Daemon couldn't distinguish "never fetched" from "fetched but failed then retried successfully."

**Do this instead:** Enforce invariants in service layer (outside this crate). Per CONTRACT.md: data wipe only on explicit reset, not on failure. Keep last-good until explicitly cleared.

### Region Detection Boundary Misses

**What happens:** Coordinates very close to US-Canada or Europe-Russia borders could miss the correct region due to bounding box rounding.

**Why it's wrong:** User in Niagara Falls might get Canadian alerts instead of NWS, or vice versa.

**Do this instead:** Use `detect_region`'s output as primary router, but for ambiguous coords (within 1 degree of known borders), consider falling back to `detect_country_from_coords` geocoding if available. Currently JMA bounding box has explicit regional awareness (weather_jma.rs:47-50); replicate for alerts.

## Error Handling

**Strategy:** Categorized errors with wire mapping.

**Patterns:**
- Internal: `Result<T> = std::result::Result<T, Error>` with 8 variants (Timeout, Network(kind), HttpStatus(code), Parse(kind), HttpClient, NoResults, LocationDetection, Dbus)
- Wire: WireError struct with `kind: String` (machine-readable) + `message: String` (human-readable, PII-scrubbed)
- Mapping: `From<&Error> for WireError` strips query detail (error.rs:33)
- D-Bus: Methods fail with native D-Bus errors named `dev.jcrenshaw.Weathervane1.Error.<Kind>` (CONTRACT.md:194)
- Fallback: Optional providers (JMA, AQICN) log at debug and return None on any failure, silently using default

## Cross-Cutting Concerns

**Logging:** Uses `tracing` crate. Debug level for optional provider failures (doesn't surface error), info for system events (NM connected, logind resumed), warn for D-Bus unavailability.

**Validation:** Each async function validates inputs (lat/lon in [-90, 90] and [-180, 180]) implicitly via provider APIs; no explicit bounds checks in weathervane (APIs reject invalid coords).

**Authentication:** API keys (aqicn_token) passed as parameters, never stored. D-Bus access via zbus; no credential handling inside crate.

---

*Architecture analysis: 2026-06-30*
