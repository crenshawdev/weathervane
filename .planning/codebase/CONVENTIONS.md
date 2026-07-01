# Coding Conventions

**Analysis Date:** 2026-06-30

## Naming Patterns

**Files:**
- snake_case for all module files: `weather.rs`, `air_quality.rs`, `location.rs`
- Derived modules use suffixes for their purpose: `air_quality_aqicn.rs` (provider-specific), `weather_jma.rs` (region-specific)

**Structs and Types:**
- PascalCase: `CurrentWeather`, `WeatherData`, `LocationResult`, `AirQualityData`, `SavedLocation`, `DetectedLocation`
- Newtypes follow their inner type: `Result<T> = std::result::Result<T, Error>`

**Enums:**
- Type name in PascalCase: `WeatherCondition`, `AlertSeverity`, `TemperatureUnit`, `MeasurementSystem`, `AqiCategory`
- Variants in PascalCase: `ClearSky`, `PartlyCloudy`, `Good`, `Moderate`, `Fahrenheit`, `Celsius`
- No suffix on variant names even when context is clear in type

**Functions:**
- snake_case: `search_city()`, `detect_location()`, `fetch_weather()`, `fetch_air_quality()`, `is_night_time()`
- Public async functions at crate root re-export from modules: `pub use weather::fetch_weather;` in `lib.rs`

**Constants:**
- UPPER_SNAKE_CASE: `NM_STATE_CONNECTED_GLOBAL` in `network.rs:16`

**Fields and Variables:**
- snake_case: `temperature`, `wind_speed`, `humidity`, `feels_like`, `cloud_cover`, `weather_code`
- Abbreviations expanded except where API-standard: `pm2_5` (matches AQI standard), `pm10`
- Boolean getters use `is_`/`uses_` prefix: `is_night_time()`, `uses_imperial_units()`, `matches_coords()`

**Test Functions:**
- descriptive snake_case with assertions clear from name: `pressure_hpa_passthrough()`, `pressure_converts_to_inhg_and_psi()`, `no_uppercase_json_keys_in_snapshots()`

## Code Style

**Formatting:**
- Tool: rustfmt (default configuration, no custom rustfmt.toml)
- CI gate: `cargo fmt --check` enforced on all branches
- Run locally: `cargo fmt`

**Linting:**
- Tool: clippy with strict mode
- CI gate: `cargo clippy --workspace -- -D warnings` (all warnings are errors)
- Idiomatic Rust patterns required; no unsafe code without explicit justification

**Documentation:**
- Every module has a doc comment block (`//!`) explaining its purpose and content
- Example from `weather.rs:3`: `//! Weather data types and fetching from the Open-Meteo API.`
- Every public struct/enum has doc comments above it describing its purpose
- Public fields have doc comments (///) explaining units, ranges, or contract details
- Example from `weather.rs:16-19`:
  ```rust
  /// Current weather conditions.
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct CurrentWeather {
      /// Temperature in the requested unit (Fahrenheit or Celsius).
      pub temperature: f32,
  ```
- Methods have doc comments explaining behavior and edge cases
- No doc examples in this crate (pure data transformation library)

**SPDX Headers:**
- Every file starts with: `// SPDX-License-Identifier: MIT OR Apache-2.0`
- No exception — applied to all `.rs` files

## Import Organization

**Order (as seen in files):**
1. Internal crate imports (`use crate::...`)
2. External crate imports (`use serde::..., use chrono::...`)
3. Standard library imports (`use std::...`)
4. Module-scoped items imported in functions where needed

**Patterns:**
- Serde derives always: `#[derive(Debug, Clone, Serialize, Deserialize)]` (in that order)
- Debug comes first for debugging convenience
- Clone follows Debug for flexibility
- Serde derives last for wire contract awareness
- No glob imports at module level
- Private module imports at top of `lib.rs` (`mod air_quality_aqicn;`)
- Public modules re-exported at crate root for convenience: `pub use weather::{CurrentWeather, WeatherData};`

**Crate Dependencies:**
- `thiserror` for error types (all error enums derive `#[derive(Debug, Error)]`)
- `serde`/`serde_json` for wire contracts
- `reqwest` for HTTP (async with JSON feature)
- `chrono` with serde feature for timestamps
- `zbus` with tokio feature for D-Bus (network monitoring)
- `tracing` for structured logging
- `quick-xml` for XML parsing (air quality provider)

## Error Handling

**Pattern:**
- All functions return `Result<T>` (aliased as `pub type Result<T> = std::result::Result<T, Error>;` in `error.rs:108`)
- Custom `Error` enum in `error.rs` with thiserror derives
- Example error variants:
  ```rust
  #[error("request timed out")]
  Timeout,
  
  #[error("http status {0}")]
  HttpStatus(u16),
  
  #[error("no results")]
  NoResults { query: String },
  ```

**Error Conversion:**
- Automatic From<reqwest::Error> converts network errors to crate errors (`error.rs:78-100`)
- From<quick_xml::DeError> converts XML parse errors (`error.rs:102-106`)
- Error display is deterministic; matches `Display` impl
- Error variants have named sub-types (NetworkKind, ParseKind) for pattern matching without string parsing
- Example: `Error::Network(NetworkKind::Connect)` not `Error::Network("connect")`

**PII Contract:**
- Error payloads never carry user search text or coordinates
- Tests verify this explicitly (`wire_contract.rs:374-401`)
- `NoResults { query }` field is in enum but NOT serialized to wire format (via WireError conversion)
- Passthrough fields (HttpClient, Dbus messages) are library-generated only, never user input

## Logging

**Framework:** tracing 0.1
- Not console.log-style; structured/leveled
- Example from `network.rs:26`: `tracing::warn!("Could not connect to system D-Bus, network monitoring disabled");`
- Info level for significant state changes: `tracing::info!("Listening for NetworkManager connectivity changes");`
- Debug level for detailed flow: `tracing::debug!("Found {} location(s)", locations.len());`
- No println! or dbg! macros in library code

## Comments

**When to Comment:**
- Every public function has a doc comment
- Every public struct/enum/field has a doc comment
- Complex logic (like round-trip serialization in tests) gets an explanatory comment above the block
- Contract details get comments: "UTC instants in RFC3339 with Z" in wire types
- Implementation notes explaining *why* a decision was made, not *what* the code does

**Doc Comments Examples:**
- Module level: `//! Weather domain logic for the weathervane family of applications.`
- Struct level: `/// Current weather conditions.`
- Field level: `/// Temperature in the requested unit (Fahrenheit or Celsius).`
- Function level: `/// Formats an ISO timestamp to hour display (e.g. "14:00" or "2:00 PM").`

**JSDoc/TSDoc:** Not applicable (Rust)

## Function Design

**Size:** Generally 10-40 lines; keep a single responsibility
- `search_city()` (`location.rs:73-97`): search, parse, return — 25 lines
- `fetch_weather()` in weather.rs follows Open-Meteo fetch → parse → return pattern
- Helper functions extracted for testable units: `from_current()` in `pollen.rs:67` (logic separated from API call)

**Parameters:**
- Positional for required inputs: `pub async fn search_city(city_name: &str) -> Result<Vec<LocationResult>>`
- No builder patterns; straightforward params
- Coordinates always as (latitude, longitude): `fetch_weather(latitude: f64, longitude: f64, ...)`
- Units passed explicitly: `pub async fn fetch_weather(..., temp_unit: TemperatureUnit, measurement_system: MeasurementSystem) -> Result<WeatherData>`

**Return Values:**
- Async functions return `Result<T>` (error is always Error from error.rs)
- Some operations return `Option<T>` for optional data: `fetch_pollen() -> Result<Option<PollenData>>` (no coverage outside Europe)
- Struct fields are public; no getters/setters

## Module Design

**Exports:**
- Private modules listed in `lib.rs` (`mod air_quality_aqicn;`, `mod client;`) — internal-only
- Public modules with `pub mod` are re-exported at crate root for user convenience
- Types re-exported at root: `pub use weather::{CurrentWeather, DailyForecast, HourlyForecast, WeatherData};`
- Functions re-exported at root: `pub use weather::fetch_weather;`
- Prevents users needing to know the module structure; can call `weathervane::fetch_weather()` not `weathervane::weather::fetch_weather()`

**Barrel Files:** Yes, but minimal
- `lib.rs` is the barrel file
- Lists all public APIs at crate root for one-stop reference
- Imports organized: types first, then async functions

**Visibility:**
- Public: Type definitions, domain functions (fetch_*, search_*, detect_*), error types
- Private: Parsing helpers, API response types (Serde intermediate structs), provider-specific logic
- Helper functions in modules (e.g., `from_current()` in pollen.rs) are private or scoped within tests

## Wire Contract

**Serde Rules:**
- All serializable types derive: `#[derive(Serialize, Deserialize)]`
- JSON keys are always snake_case: `"wind_gusts"`, `"feels_like"`, `"utc_offset_seconds"`
- Enum variants serialize as PascalCase strings: `"PartlyCloudy"`, `"Good"`, `"Fahrenheit"`
- No `#[serde(rename)]` without docstring explaining backward-compatibility reason
- AqiCategory uses adjacent tagging to disambiguate (Eu(Good) vs Us(Good)):
  ```rust
  #[serde(tag = "standard", content = "level")]
  pub enum AqiCategory {
      Us(UsAqiCategory),
      Eu(EuAqiCategory),
  }
  ```
- Snapshots in `tests/snapshots/` are the frozen contract; changes require explicit review via `cargo insta review`
- Snapshot filenames match test names: test `current_weather_shape()` → snapshot `wire_contract__current_weather.snap`

## Async Patterns

**Streams:**
- `async-stream` crate for async generator syntax
- Example from `network.rs:24`: `Box::pin(async_stream::stream! { ... })`
- Returns `Pin<Box<dyn Stream<Item = T> + Send>>`
- Frontends wrap in their own subscription type (iced Subscription, etc.)

**Futures:**
- No manual Future impl; use async/await
- Composition via `.await` on async calls
- Futures used for composition: `.and_then()`, `.map()` for chaining results

## Reactive Philosophy

The crate is reactive: no polling, no timers, no config storage
- Functions are stateless transformations
- Callers control when to call (frontend decides timing, storage, scheduling)
- Each call is independent; parameters passed in, result returned
- Network calls are explicit (fetch_weather, search_city, detect_location)

---

*Convention analysis: 2026-06-30*
