# Wire Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the frozen JSON wire contract from `docs/superpowers/specs/2026-06-11-wire-contract-design.md` — serde derives on all wire-crossing types, the `AqiCategory` restructure, `WireError`/`Envelope` wire types, a CI-enforced snapshot test suite, CONTRACT.md, and the 0.7.0 bump.

**Architecture:** Pure library changes; no daemon/CLI code. Wire shapes are serde-default JSON (snake_case fields, PascalCase variants) with one documented exception (`AqiCategory`, adjacent-tagged). The freeze is enforced by committed insta snapshots — any shape drift fails `cargo test`.

**Tech Stack:** Rust 2021, serde/serde_json (already deps), chrono serde (already enabled), insta + regex (new dev-deps only).

**Ground rules for the executor:**
- A repo hook **blocks file edits on `main`**. Do all work on branch `feat/wire-contract` (Task 1 Step 1 creates it).
- Commit messages: conventional commits, **no AI/Claude attribution of any kind** (hard rule).
- Snapshot workflow: first run of a new snapshot test uses `INSTA_UPDATE=always cargo test --test wire_contract` to write `.snap` files, then **read the generated snapshot and verify it matches the shape shown in the task** before committing. Plain `cargo test` must pass afterward.
- All test code lives in `tests/wire_contract.rs`, built up task by task. Each task appends to it; `use` lines listed in Task 1 cover later tasks too — don't re-add.

---

### Task 1: Test scaffold, baseline snapshots, legacy-compat guards

Covers spec §4.1 (enum spelling canaries, already-serializable types) and §4.4 (tempest config-compat guards).

**Files:**
- Modify: `Cargo.toml` (dev-dependencies)
- Modify: `.gitignore` (ignore pending snapshots)
- Create: `tests/wire_contract.rs`

- [ ] **Step 1: Create the work branch**

```bash
cd /code/weathervane && git checkout -b feat/wire-contract
```

- [ ] **Step 2: Add dev-dependencies**

Append to `Cargo.toml` (file currently ends at the `zbus` line of `[dependencies]`):

```toml

[dev-dependencies]
insta = "1"
regex = "1"
```

- [ ] **Step 3: Ignore insta pending files**

Append to `.gitignore`:

```
tests/snapshots/*.snap.new
```

- [ ] **Step 4: Write the scaffold + first tests**

Create `tests/wire_contract.rs`:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Wire contract enforcement (docs/superpowers/specs/2026-06-11-wire-contract-design.md).
//!
//! The committed snapshots under tests/snapshots/ ARE the frozen JSON contract.
//! A failing snapshot test means a wire-shape change: either revert it, or
//! deliberately accept it as a breaking contract change (INSTA_UPDATE=always /
//! cargo insta review) and treat the release accordingly.

use chrono::{TimeZone, Utc};
use serde::{de::DeserializeOwned, Serialize};
use weathervane::{
    AirQualityData, Alert, AlertSeverity, AqiCategory, CompassDirection, CurrentWeather,
    DailyForecast, DetectedLocation, Envelope, EnvelopeError, Error, EuAqiCategory,
    HourlyForecast, LocationResult, MeasurementSystem, NetworkKind, ParseKind, PollenData,
    PressureUnit, SavedLocation, TemperatureUnit, UsAqiCategory, WeatherCondition, WeatherData,
    WireError,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Serializes pretty (snapshot input) — the canonical wire form.
fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap()
}

/// Serialize → deserialize → re-serialize must be lossless. Returns the JSON
/// so callers can snapshot the same bytes they round-tripped.
fn round_trip<T: Serialize + DeserializeOwned>(value: &T) -> String {
    let first = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&first).unwrap();
    let second = serde_json::to_string(&back).unwrap();
    assert_eq!(first, second, "round-trip changed the wire form");
    first
}

// ---------------------------------------------------------------------------
// Fixtures — fully populated, distinctive values (spec §4.1)
// ---------------------------------------------------------------------------

fn current_weather() -> CurrentWeather {
    CurrentWeather {
        temperature: 71.3,
        weathercode: 2,
        condition: WeatherCondition::PartlyCloudy,
        windspeed: 8.5,
        humidity: 54,
        feels_like: 69.8,
        wind_direction: 305,
        compass_direction: CompassDirection::NW,
        wind_gusts: 12.4,
        uv_index: 6.5,
        visibility: 24135.0,
        pressure: 1015.2,
        cloud_cover: 40,
        dew_point: 52.7,
    }
}

fn hourly_forecast() -> HourlyForecast {
    HourlyForecast {
        time: "2026-06-11T10:00".to_string(),
        temperature: 71.3,
        weathercode: 2,
        condition: WeatherCondition::PartlyCloudy,
        precipitation_probability: 20,
        precipitation: 0.1,
        windspeed: 8.5,
        wind_gusts: 12.4,
    }
}

fn daily_forecast() -> DailyForecast {
    DailyForecast {
        date: "2026-06-11".to_string(),
        temp_max: 78.4,
        temp_min: 58.1,
        weathercode: 3,
        condition: WeatherCondition::Overcast,
        sunrise: "2026-06-11T05:21".to_string(),
        sunset: "2026-06-11T20:29".to_string(),
    }
}

fn saved_location() -> SavedLocation {
    SavedLocation {
        name: "Home".to_string(),
        latitude: 45.5152,
        longitude: -122.6784,
    }
}

// ---------------------------------------------------------------------------
// §4.1 — enum spelling canaries (PascalCase baseline guards)
// ---------------------------------------------------------------------------

#[test]
fn enum_spelling_canaries() {
    insta::assert_snapshot!("canary_weather_condition", json(&WeatherCondition::PartlyCloudy));
    insta::assert_snapshot!("canary_compass_direction", json(&CompassDirection::NW));
    insta::assert_snapshot!("canary_temperature_unit", json(&TemperatureUnit::Fahrenheit));
    insta::assert_snapshot!("canary_pressure_unit", json(&PressureUnit::Hpa));
    insta::assert_snapshot!("canary_measurement_system", json(&MeasurementSystem::Imperial));
}

// ---------------------------------------------------------------------------
// §4.1 — already-serializable struct shapes
// ---------------------------------------------------------------------------

#[test]
fn current_weather_shape() {
    let v = current_weather();
    round_trip(&v);
    insta::assert_snapshot!("current_weather", json(&v));
}

#[test]
fn hourly_forecast_shape() {
    let v = hourly_forecast();
    round_trip(&v);
    insta::assert_snapshot!("hourly_forecast", json(&v));
}

#[test]
fn daily_forecast_shape() {
    let v = daily_forecast();
    round_trip(&v);
    insta::assert_snapshot!("daily_forecast", json(&v));
}

#[test]
fn saved_location_shape() {
    let v = saved_location();
    round_trip(&v);
    insta::assert_snapshot!("saved_location", json(&v));
}

// ---------------------------------------------------------------------------
// §4.4 — tempest config-compat guards: the literal spellings that already
// exist in users' persisted configs MUST keep parsing. These fail if anyone
// ever adds a #[serde(rename_all)] to these types.
// ---------------------------------------------------------------------------

#[test]
fn legacy_persisted_unit_spellings_still_parse() {
    assert_eq!(
        serde_json::from_str::<TemperatureUnit>("\"Fahrenheit\"").unwrap(),
        TemperatureUnit::Fahrenheit
    );
    assert_eq!(
        serde_json::from_str::<TemperatureUnit>("\"Celsius\"").unwrap(),
        TemperatureUnit::Celsius
    );
    assert_eq!(serde_json::from_str::<PressureUnit>("\"Hpa\"").unwrap(), PressureUnit::Hpa);
    assert_eq!(serde_json::from_str::<PressureUnit>("\"InHg\"").unwrap(), PressureUnit::InHg);
    assert_eq!(serde_json::from_str::<PressureUnit>("\"Psi\"").unwrap(), PressureUnit::Psi);
    assert_eq!(
        serde_json::from_str::<MeasurementSystem>("\"Imperial\"").unwrap(),
        MeasurementSystem::Imperial
    );
    assert_eq!(
        serde_json::from_str::<MeasurementSystem>("\"Metric\"").unwrap(),
        MeasurementSystem::Metric
    );
}

#[test]
fn legacy_saved_location_json_still_parses() {
    let parsed: SavedLocation =
        serde_json::from_str(r#"{"name":"Home","latitude":45.5152,"longitude":-122.6784}"#)
            .unwrap();
    assert_eq!(parsed, saved_location());
}
```

**Note:** this file references types that don't exist yet (`Envelope`, `WireError`, `WeatherData` derives…). For THIS task, comment out the not-yet-available imports so it compiles with only what Task 1 tests use:

```rust
use weathervane::{
    CompassDirection, CurrentWeather, DailyForecast, HourlyForecast, MeasurementSystem,
    PressureUnit, SavedLocation, TemperatureUnit, WeatherCondition,
};
```

Later tasks extend this `use` list as their types land. (`chrono`, `Error`, etc. join in Tasks 4/6/7.)

- [ ] **Step 5: Run — expect snapshot creation**

```bash
INSTA_UPDATE=always cargo test --test wire_contract
```

Expected: PASS, and `tests/snapshots/wire_contract__canary_weather_condition.snap` (+8 more `.snap` files) created.

- [ ] **Step 6: Verify the snapshots are the contract**

Read each generated `.snap`. They must show:
- `canary_weather_condition`: `"PartlyCloudy"`
- `canary_compass_direction`: `"NW"`
- `canary_temperature_unit`: `"Fahrenheit"`
- `current_weather`: snake_case keys only (`feels_like`, `wind_gusts`, `compass_direction`…), no camelCase.

- [ ] **Step 7: Plain run passes**

```bash
cargo test --test wire_contract
```

Expected: PASS (all tests, no pending snapshots).

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock .gitignore tests/
git commit -m "test: add wire-contract snapshot scaffold, canaries, config-compat guards"
```

---

### Task 2: `WeatherData` gains derives

**Files:**
- Modify: `src/weather.rs:87-101` (the `WeatherData` struct)
- Test: `tests/wire_contract.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/wire_contract.rs` (and add `WeatherData` to the `use weathervane::{...}` list):

```rust
fn weather_data() -> WeatherData {
    WeatherData {
        current: current_weather(),
        hourly: vec![hourly_forecast()],
        forecast: vec![daily_forecast()],
        utc_offset_seconds: -14400,
    }
}

#[test]
fn weather_data_shape() {
    let v = weather_data();
    round_trip(&v);
    insta::assert_snapshot!("weather_data", json(&v));
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test --test wire_contract
```

Expected: COMPILE ERROR — `WeatherData` doesn't implement `Serialize`.

- [ ] **Step 3: Add the derives**

In `src/weather.rs`, change:

```rust
/// Complete weather data from a single fetch.
#[derive(Debug, Clone)]
pub struct WeatherData {
```

to:

```rust
/// Complete weather data from a single fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherData {
```

(`use serde::{Deserialize, Serialize};` is already at the top of the file.)

- [ ] **Step 4: Create + verify snapshot, then plain pass**

```bash
INSTA_UPDATE=always cargo test --test wire_contract && cargo test --test wire_contract
```

Expected: PASS both times. New `wire_contract__weather_data.snap` must show top-level keys exactly: `current`, `hourly`, `forecast`, `utc_offset_seconds`.

- [ ] **Step 5: Commit**

```bash
git add src/weather.rs tests/
git commit -m "feat: derive Serialize/Deserialize on WeatherData"
```

---

### Task 3: `AqiCategory` restructure + `AirQualityData` derives (breaking)

Spec §2.2. `AirQualityData.standard` field → `standard()` method; `AqiCategory` adjacent-tagged.

**Files:**
- Modify: `src/air_quality.rs` (derives at lines 22, 53, 85; struct at 94-112; fetch at 153-190)
- Test: `tests/wire_contract.rs`

- [ ] **Step 1: Write the failing tests**

Append (extend `use` list with `AirQualityData, AqiCategory, EuAqiCategory, UsAqiCategory`):

```rust
fn air_quality_us() -> AirQualityData {
    AirQualityData {
        aqi: 42,
        category: AqiCategory::Us(UsAqiCategory::Good),
        pm2_5: 8.1,
        pm10: 14.9,
        ozone: 61.3,
        nitrogen_dioxide: 9.4,
        carbon_monoxide: 142.0,
    }
}

fn air_quality_eu() -> AirQualityData {
    AirQualityData {
        aqi: 35,
        category: AqiCategory::Eu(EuAqiCategory::Fair),
        pm2_5: 8.1,
        pm10: 14.9,
        ozone: 61.3,
        nitrogen_dioxide: 9.4,
        carbon_monoxide: 142.0,
    }
}

#[test]
fn air_quality_shapes() {
    round_trip(&air_quality_us());
    round_trip(&air_quality_eu());
    insta::assert_snapshot!("air_quality_us", json(&air_quality_us()));
    insta::assert_snapshot!("air_quality_eu", json(&air_quality_eu()));
}

/// The adjacent tag must round-trip Eu(Good) to Eu(Good) — the exact bug an
/// untagged representation would have shipped ("Good" exists in both scales).
#[test]
fn aqi_category_round_trips_exactly() {
    let eu = AqiCategory::Eu(EuAqiCategory::Good);
    let back: AqiCategory =
        serde_json::from_str(&serde_json::to_string(&eu).unwrap()).unwrap();
    assert_eq!(back, eu);

    let us = AqiCategory::Us(UsAqiCategory::Good);
    let back: AqiCategory =
        serde_json::from_str(&serde_json::to_string(&us).unwrap()).unwrap();
    assert_eq!(back, us);
}

#[test]
fn aqi_standard_method_derives_from_category() {
    use weathervane::AqiStandard;
    assert_eq!(air_quality_us().standard(), AqiStandard::Us);
    assert_eq!(air_quality_eu().standard(), AqiStandard::European);
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test --test wire_contract
```

Expected: COMPILE ERROR — `AirQualityData` has a `standard` field the fixture doesn't set, no `Serialize`, no `standard()` method.

- [ ] **Step 3: Implement**

In `src/air_quality.rs`:

(a) Change the import at the top from `use serde::Deserialize;` to:

```rust
use serde::{Deserialize, Serialize};
```

(b) Add derives to the two category enums (keep existing derive items):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UsAqiCategory {
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EuAqiCategory {
```

(c) Replace the `AqiCategory` definition (line 83-91) with:

```rust
/// AQI category, region-specific.
/// Frontend matches on this to produce translated descriptions.
///
/// Wire form is adjacent-tagged (the one documented exception to the
/// serde-defaults baseline): `{"standard": "Us", "level": "Good"}`.
/// The tag disambiguates levels like `Good`/`Moderate` that exist in both
/// scales, so deserialization is exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "standard", content = "level")]
pub enum AqiCategory {
    /// US EPA category. Returned for all non-European locations.
    Us(UsAqiCategory),
    /// European category. Returned when coordinates fall within Europe.
    Eu(EuAqiCategory),
}
```

(d) Replace the `AirQualityData` struct (lines 93-112) with — note `standard`
field removed and the new method:

```rust
/// Current air quality data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirQualityData {
    /// AQI value. Scale depends on the standard (US 0-500, EU 0-100+).
    pub aqi: i32,
    /// Categorized severity for display. Also carries which standard applies;
    /// see [`AirQualityData::standard`].
    pub category: AqiCategory,
    /// Fine particulate matter (micrograms per cubic meter).
    pub pm2_5: f32,
    /// Coarse particulate matter (micrograms per cubic meter).
    pub pm10: f32,
    /// Ground-level ozone (micrograms per cubic meter).
    pub ozone: f32,
    /// NO2 concentration (micrograms per cubic meter).
    pub nitrogen_dioxide: f32,
    /// CO concentration (micrograms per cubic meter).
    pub carbon_monoxide: f32,
}

impl AirQualityData {
    /// Which AQI standard applies, derived from the category variant.
    pub fn standard(&self) -> AqiStandard {
        match self.category {
            AqiCategory::Us(_) => AqiStandard::Us,
            AqiCategory::Eu(_) => AqiStandard::European,
        }
    }
}
```

(e) In `fetch_air_quality`, replace the `let (aqi, standard, category) = ...`
block and the final `Ok(...)` (lines 153-190) with:

```rust
    let (aqi, category) = if let Some(val) = aqicn_aqi {
        (val, AqiCategory::Us(UsAqiCategory::from_aqi(val)))
    } else if region == Region::Europe {
        let val = data.current.european_aqi.unwrap_or_else(|| {
            tracing::warn!("European AQI missing from API response, defaulting to 0");
            0
        });
        (val, AqiCategory::Eu(EuAqiCategory::from_aqi(val)))
    } else {
        let val = data.current.us_aqi.unwrap_or_else(|| {
            tracing::warn!("US AQI missing from API response, defaulting to 0");
            0
        });
        (val, AqiCategory::Us(UsAqiCategory::from_aqi(val)))
    };

    Ok(AirQualityData {
        aqi,
        category,
        pm2_5: data.current.pm2_5.unwrap_or(0.0),
        pm10: data.current.pm10.unwrap_or(0.0),
        ozone: data.current.ozone.unwrap_or(0.0),
        nitrogen_dioxide: data.current.nitrogen_dioxide.unwrap_or(0.0),
        carbon_monoxide: data.current.carbon_monoxide.unwrap_or(0.0),
    })
```

- [ ] **Step 4: Create + verify snapshots, plain pass, full suite**

```bash
INSTA_UPDATE=always cargo test --test wire_contract && cargo test
```

Expected: PASS. `wire_contract__air_quality_us.snap` must contain exactly:

```json
{
  "aqi": 42,
  "category": {
    "standard": "Us",
    "level": "Good"
  },
  "pm2_5": 8.1,
  ...
}
```

and NO top-level `"standard"` key. `cargo test` (full) confirms no other code in the crate read the removed field.

- [ ] **Step 5: Commit**

```bash
git add src/air_quality.rs tests/
git commit -m "feat!: adjacent-tag AqiCategory, replace AirQualityData.standard field with method"
```

---

### Task 4: `Alert` + `AlertSeverity` derives

**Files:**
- Modify: `src/alerts.rs:17-61`
- Test: `tests/wire_contract.rs`

- [ ] **Step 1: Write the failing test**

Append (extend `use` list with `Alert, AlertSeverity`; `chrono::{TimeZone, Utc}` from Task 1 header is now used):

```rust
fn alert() -> Alert {
    Alert {
        id: "NWS-IDP-PROD-123".to_string(),
        event: "Severe Thunderstorm Warning".to_string(),
        severity: AlertSeverity::Severe,
        headline: "Severe thunderstorm until 10 PM EDT".to_string(),
        description: "Wind gusts to 60 mph expected.".to_string(),
        expires: Utc.with_ymd_and_hms(2026, 6, 11, 22, 0, 0).unwrap(),
    }
}

#[test]
fn alert_shape() {
    let v = alert();
    round_trip(&v);
    let pretty = json(&v);
    // Contract: alert times are UTC instants in RFC3339 with Z.
    assert!(pretty.contains("\"2026-06-11T22:00:00Z\""), "expires must be RFC3339 Z");
    insta::assert_snapshot!("alert", pretty);
    insta::assert_snapshot!("canary_alert_severity", json(&AlertSeverity::Severe));
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test --test wire_contract
```

Expected: COMPILE ERROR — `Alert` doesn't implement `Serialize`.

- [ ] **Step 3: Add the derives**

In `src/alerts.rs`: the file imports serde already for its private response
structs — confirm the import line includes `Serialize` (change
`use serde::Deserialize;` to `use serde::{Deserialize, Serialize};` if not).
Then:

```rust
/// Weather alert severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
```

```rust
/// Weather alert from NWS, MeteoAlarm, ECCC, or BOM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
```

- [ ] **Step 4: Create + verify snapshot, plain pass**

```bash
INSTA_UPDATE=always cargo test --test wire_contract && cargo test --test wire_contract
```

Expected: PASS. `wire_contract__alert.snap` keys: `id`, `event`, `severity`, `headline`, `description`, `expires` — severity value `"Severe"`, expires `"2026-06-11T22:00:00Z"`.

- [ ] **Step 5: Commit**

```bash
git add src/alerts.rs tests/
git commit -m "feat: derive Serialize/Deserialize on Alert and AlertSeverity"
```

---

### Task 5: `PollenData`, `LocationResult`, `DetectedLocation` derives

**Files:**
- Modify: `src/pollen.rs:22`, `src/location.rs:11,60` (and serde imports)
- Test: `tests/wire_contract.rs`

- [ ] **Step 1: Write the failing tests**

Append (extend `use` list with `DetectedLocation, LocationResult, PollenData`):

```rust
fn pollen() -> PollenData {
    PollenData {
        alder: 0.0,
        birch: 12.4,
        grass: 3.1,
        mugwort: 0.0,
        olive: 0.0,
        ragweed: 0.7,
    }
}

#[test]
fn pollen_shapes() {
    let some = Some(pollen());
    round_trip(&some);
    insta::assert_snapshot!("pollen_some", json(&some));
    // Contract (§1, Option policy): outside CAMS coverage pollen is null —
    // a present key with null, never absent, never zero-filled.
    insta::assert_snapshot!("pollen_none", json(&Option::<PollenData>::None));
}

#[test]
fn location_shapes() {
    let result = LocationResult {
        latitude: 45.5152,
        longitude: -122.6784,
        display_name: "Portland, Oregon, United States".to_string(),
        country: "United States".to_string(),
    };
    round_trip(&result);
    insta::assert_snapshot!("location_result", json(&result));

    let detected = DetectedLocation {
        latitude: 45.52,
        longitude: -122.68,
        display_name: "Portland, United States".to_string(),
        country: "United States".to_string(),
    };
    round_trip(&detected);
    insta::assert_snapshot!("detected_location", json(&detected));
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test --test wire_contract
```

Expected: COMPILE ERROR — missing `Serialize` impls.

- [ ] **Step 3: Add the derives**

`src/pollen.rs` — add the serde import (file has none for public types) and the derive:

```rust
use serde::{Deserialize, Serialize};
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollenData {
```

`src/location.rs` — `use serde::{Deserialize, Serialize};` already present (SavedLocation uses it). Add to both structs:

```rust
/// Location search result from geocoding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationResult {
```

```rust
/// Result of automatic IP-based location detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedLocation {
```

- [ ] **Step 4: Create + verify snapshots, plain pass**

```bash
INSTA_UPDATE=always cargo test --test wire_contract && cargo test --test wire_contract
```

Expected: PASS. `pollen_none.snap` content is exactly `null`. `pollen_some.snap` has all six species keys.

- [ ] **Step 5: Commit**

```bash
git add src/pollen.rs src/location.rs tests/
git commit -m "feat: derive Serialize/Deserialize on PollenData, LocationResult, DetectedLocation"
```

---

### Task 6: `wire` module — `WireError` + PII guards

Spec §2.4. New module: wire-facing types live together in `src/wire.rs`.

**Files:**
- Create: `src/wire.rs`
- Modify: `src/lib.rs` (module + re-exports)
- Test: `tests/wire_contract.rs`

- [ ] **Step 1: Write the failing tests**

Append (extend `use` list with `Error, NetworkKind, ParseKind, WireError`):

```rust
/// One WireError exemplar per Error variant — the `kind` strings are contract.
fn wire_error_exemplars() -> Vec<(&'static str, Error)> {
    vec![
        ("Timeout", Error::Timeout),
        ("Network", Error::Network(NetworkKind::Connect)),
        ("HttpStatus", Error::HttpStatus(429)),
        ("Parse", Error::Parse(ParseKind::Xml)),
        ("HttpClient", Error::HttpClient("tls backend not initialized".to_string())),
        ("NoResults", Error::NoResults { query: "Portlandia".to_string() }),
        ("LocationDetection", Error::LocationDetection),
        ("Dbus", Error::Dbus("name lost".to_string())),
    ]
}

#[test]
fn wire_error_shapes() {
    for (expected_kind, err) in wire_error_exemplars() {
        let wire = WireError::from(&err);
        assert_eq!(wire.kind, expected_kind);
        round_trip(&wire);
        insta::assert_snapshot!(format!("wire_error_{expected_kind}"), json(&wire));
    }
}

/// PII contract: error payloads never carry the user's search text. The
/// Display impl for NoResults deliberately omits the query; this pins that.
#[test]
fn wire_error_never_leaks_query() {
    let err = Error::NoResults { query: "SENTINEL_QUERY_55x".to_string() };
    let wire = WireError::from(&err);
    let serialized = serde_json::to_string(&wire).unwrap();
    assert!(!serialized.contains("SENTINEL_QUERY_55x"), "query text leaked: {serialized}");
}

/// PII contract: fixture coordinates must not appear in any error payload.
#[test]
fn wire_error_never_leaks_coordinates() {
    for (_, err) in wire_error_exemplars() {
        let serialized = serde_json::to_string(&WireError::from(&err)).unwrap();
        assert!(!serialized.contains("45.5152"), "latitude leaked: {serialized}");
        assert!(!serialized.contains("-122.6784"), "longitude leaked: {serialized}");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test --test wire_contract
```

Expected: COMPILE ERROR — `WireError` not found in `weathervane`.

- [ ] **Step 3: Create `src/wire.rs`**

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Wire-facing types for the frozen JSON contract (CONTRACT.md).
//!
//! These are the shapes the future daemon and CLI emit. They exist in the
//! library so the contract is testable (snapshot suite) before any IPC
//! surface ships.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Error;

/// JSON-facing error shape. Built from [`Error`], never derived on it, so the
/// wire never carries `NoResults { query }` payload data (PII contract: error
/// payloads contain no search text and no coordinates) and the shape stays
/// flat regardless of `Error`'s internal structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireError {
    /// `Error` variant name: "Timeout", "Network", "HttpStatus", "Parse",
    /// "HttpClient", "NoResults", "LocationDetection", "Dbus".
    pub kind: String,
    /// Human-readable detail — the `Display` output, e.g. "http status 429".
    pub message: String,
}

impl From<&Error> for WireError {
    fn from(e: &Error) -> Self {
        let kind = match e {
            Error::Timeout => "Timeout",
            Error::Network(_) => "Network",
            Error::HttpStatus(_) => "HttpStatus",
            Error::Parse(_) => "Parse",
            Error::HttpClient(_) => "HttpClient",
            Error::NoResults { .. } => "NoResults",
            Error::LocationDetection => "LocationDetection",
            Error::Dbus(_) => "Dbus",
        };
        Self { kind: kind.to_string(), message: e.to_string() }
    }
}
```

- [ ] **Step 4: Wire into `src/lib.rs`**

Add to the `pub mod` block (alphabetical, after `pub mod weather;`):

```rust
pub mod wire;
```

Add to the re-export block (after the `pub use weather::...` line):

```rust
pub use wire::WireError;
```

- [ ] **Step 5: Create + verify snapshots, plain pass**

```bash
INSTA_UPDATE=always cargo test --test wire_contract && cargo test --test wire_contract
```

Expected: PASS. 8 new `wire_error_*.snap` files. `wire_error_NoResults.snap` is exactly:

```json
{
  "kind": "NoResults",
  "message": "no results"
}
```

(no query text). `wire_error_HttpStatus.snap` message: `"http status 429"`.

- [ ] **Step 6: Commit**

```bash
git add src/wire.rs src/lib.rs tests/
git commit -m "feat: add wire module with WireError and PII guarantees"
```

---

### Task 7: `Envelope` + `EnvelopeError` (state-layer shape)

Spec §3.1. The per-domain daemon property shape, frozen now so the service module inherits it.

**Files:**
- Modify: `src/wire.rs`, `src/lib.rs`
- Test: `tests/wire_contract.rs`

- [ ] **Step 1: Write the failing tests**

Append (extend `use` list with `Envelope, EnvelopeError`):

```rust
fn fetched_at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 11, 13, 50, 0).unwrap()
}

fn failed_at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 11, 14, 20, 0).unwrap()
}

#[test]
fn envelope_states() {
    // (a) healthy: data + fetched_at, error null
    let healthy = Envelope {
        data: Some(weather_data()),
        fetched_at: Some(fetched_at()),
        error: None,
    };
    round_trip(&healthy);
    insta::assert_snapshot!("envelope_healthy", json(&healthy));

    // (b) stale: last-good data retained, error reflects the failed attempt
    let stale = Envelope {
        data: Some(weather_data()),
        fetched_at: Some(fetched_at()),
        error: Some(EnvelopeError::new(&Error::Timeout, failed_at())),
    };
    round_trip(&stale);
    insta::assert_snapshot!("envelope_stale", json(&stale));

    // (c) never-fetched: all nulls except the failure
    let never = Envelope::<WeatherData> {
        data: None,
        fetched_at: None,
        error: Some(EnvelopeError::new(&Error::Network(NetworkKind::Connect), failed_at())),
    };
    round_trip(&never);
    insta::assert_snapshot!("envelope_never_fetched", json(&never));
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test --test wire_contract
```

Expected: COMPILE ERROR — `Envelope`/`EnvelopeError` not found.

- [ ] **Step 3: Implement in `src/wire.rs`**

Append:

```rust
/// A refresh failure as carried inside [`Envelope`]: what went wrong and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeError {
    /// `Error` variant name (same vocabulary as [`WireError::kind`]).
    pub kind: String,
    /// Human-readable detail (`Display` output).
    pub message: String,
    /// When the failed attempt happened (UTC).
    pub at: DateTime<Utc>,
}

impl EnvelopeError {
    /// Builds from a domain error plus the attempt timestamp.
    pub fn new(error: &Error, at: DateTime<Utc>) -> Self {
        let wire = WireError::from(error);
        Self { kind: wire.kind, message: wire.message, at }
    }
}

/// One domain's state as exposed on the wire (CONTRACT.md, state layer):
/// last-good data + freshness + last attempt's failure, all keys always
/// present.
///
/// Invariants the producer (service module / daemon) upholds:
/// - `data` is never wiped by a failed refresh; `None` only before the first
///   successful fetch.
/// - `fetched_at` is `Some` iff `data` is.
/// - `error` is `None` iff the most recent attempt succeeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Last successfully fetched payload.
    pub data: Option<T>,
    /// When `data` was obtained (UTC).
    pub fetched_at: Option<DateTime<Utc>>,
    /// Most recent attempt's failure, if it failed.
    pub error: Option<EnvelopeError>,
}
```

- [ ] **Step 4: Export from `src/lib.rs`**

Change the wire re-export to:

```rust
pub use wire::{Envelope, EnvelopeError, WireError};
```

- [ ] **Step 5: Create + verify snapshots, plain pass**

```bash
INSTA_UPDATE=always cargo test --test wire_contract && cargo test --test wire_contract
```

Expected: PASS. `envelope_stale.snap` top-level keys exactly `data`, `fetched_at`, `error`; error object keys `kind`, `message`, `at`. `envelope_never_fetched.snap` has `"data": null` and `"fetched_at": null` (present, not absent).

- [ ] **Step 6: Commit**

```bash
git add src/wire.rs src/lib.rs tests/
git commit -m "feat: add Envelope/EnvelopeError state-layer wire types"
```

---

### Task 8: Snapshot conformance walker

Spec §4.5 — structural guard: no uppercase JSON keys may ever appear in any committed snapshot.

**Files:**
- Test: `tests/wire_contract.rs`

- [ ] **Step 1: Write the test**

Append:

```rust
/// Baseline guard: JSON keys are snake_case everywhere. PascalCase strings
/// (enum variants, AqiCategory tag values) are legal only in VALUE position,
/// never as keys. Walks every committed snapshot.
#[test]
fn no_uppercase_json_keys_in_snapshots() {
    let key_with_uppercase =
        regex::Regex::new(r#""[a-z0-9_]*[A-Z][A-Za-z0-9_]*"\s*:"#).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("snapshots dir exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("snap") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap();
        if let Some(found) = key_with_uppercase.find(&content) {
            panic!("uppercase JSON key {:?} in {}", found.as_str(), path.display());
        }
        checked += 1;
    }
    assert!(checked >= 25, "expected the full snapshot suite, found {checked} files");
}
```

- [ ] **Step 2: Run — expect PASS (it's a guard over already-correct snapshots)**

```bash
cargo test --test wire_contract no_uppercase_json_keys_in_snapshots
```

Expected: PASS with `checked >= 25`. (If the count assert fails, earlier tasks were skipped — fix that first, don't lower the number below the real count.)

- [ ] **Step 3: Prove it can fail (verify the guard works)**

Temporarily add a line `"camelKey": 1,` inside any `.snap` file, rerun — expected: FAIL naming that file. Revert the edit (`git checkout -- tests/snapshots/`), rerun — PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/
git commit -m "test: guard snapshots against uppercase JSON keys"
```

---

### Task 9: CONTRACT.md

Spec §5. Examples are lifted from the committed snapshots — copy the JSON bodies **from the actual `.snap` files**, not from this plan, so doc and tests cannot diverge. The skeleton below shows the exact document structure and all normative text; `<snap:NAME>` markers show which snapshot body to paste.

**Files:**
- Create: `CONTRACT.md`

- [ ] **Step 1: Write CONTRACT.md**

````markdown
# Weathervane Wire Contract (v1)

This document freezes the JSON shapes that cross process boundaries: future
`weathervaned` D-Bus payloads, CLI output, and anything else that serializes
the public types. From the commit that introduces this file, **changing any
shape documented here is a breaking change.**

Enforcement: `tests/wire_contract.rs` — the committed snapshots under
`tests/snapshots/` are the canonical examples; this document quotes them.
Rust API reference: `API.md`.

## Conventions

- **Style:** serde defaults. snake_case field names, PascalCase enum variant
  strings. There are no `rename_all` attributes; new types conform by doing
  nothing.
- **Exception (the only one):** `AqiCategory` is adjacent-tagged:
  `{"standard": "Us", "level": "Good"}`. Standards vocabulary: `"Us"`, `"Eu"`.
- **Option policy:** documented keys are always present; `null` means no data.
  Absent keys never carry meaning.
- **Missing strings** are `""` (e.g. `Alert.description` for providers that
  send none, `LocationResult.country` when geocoding omits it) — not null.
- **Timestamps:** alert times are UTC instants, RFC3339 with `Z`
  (`"2026-06-11T22:00:00Z"`). Forecast/hourly times are location-local
  wall-clock naive ISO strings (`"2026-06-11T10:00"`); convert with the
  top-level `utc_offset_seconds`. Rule of thumb: *alerts = instants,
  forecasts = what a wall clock at the location reads.*
- **Versioning:** D-Bus payload versioning is the interface name
  (`dev.jcrenshaw.Weathervane1`); a breaking change ships `Weathervane2`
  alongside. CLI output carries `"format_version": 1`. JSON shapes are frozen
  as of this document; the `Weathervane1` interface itself freezes after the
  dogfood milestone.
- **PII rule:** error payloads (`kind`, `message`) never contain search query
  text or coordinates. Enforced by tests.

## Payloads

### WeatherData
<snap:weather_data>

### AirQualityData
US standard:
<snap:air_quality_us>
EU standard:
<snap:air_quality_eu>

### Alert (array element of the alerts payload)
<snap:alert>

### PollenData
Inside CAMS European coverage:
<snap:pollen_some>
Outside coverage (US, Asia, …) the value is `null` — present, never absent,
never zero-filled.

### LocationResult (array element of search results)
<snap:location_result>

### DetectedLocation
<snap:detected_location>

### SavedLocation
<snap:saved_location>

## Errors

### WireError

`{"kind": ..., "message": ...}` — `kind` is the machine-matchable field, one of:
`Timeout`, `Network`, `HttpStatus`, `Parse`, `HttpClient`, `NoResults`,
`LocationDetection`, `Dbus`. Sub-detail (network kind, HTTP status code) rides
in `message`. Example:
<snap:wire_error_HttpStatus>

### D-Bus method errors (request layer)

Methods fail with native D-Bus errors named
`dev.jcrenshaw.Weathervane1.Error.<Kind>` using the same kind vocabulary:

```
dev.jcrenshaw.Weathervane1.Error.Timeout
dev.jcrenshaw.Weathervane1.Error.Network
dev.jcrenshaw.Weathervane1.Error.HttpStatus
dev.jcrenshaw.Weathervane1.Error.Parse
dev.jcrenshaw.Weathervane1.Error.HttpClient
dev.jcrenshaw.Weathervane1.Error.NoResults
dev.jcrenshaw.Weathervane1.Error.LocationDetection
dev.jcrenshaw.Weathervane1.Error.Dbus
```

The error message is the `WireError.message` text.

## State layer (daemon properties)

Each domain (weather, air quality, alerts, pollen) is one property holding an
`Envelope`:

- `data` — last-good payload; never wiped by a failed refresh; `null` only
  before the first successful fetch.
- `fetched_at` — when `data` was obtained (UTC RFC3339); `null` iff `data` is.
- `error` — most recent attempt's failure (`kind`/`message`/`at`); `null` when
  the last attempt succeeded.

Healthy:
<snap:envelope_healthy>
Stale (upstream failing, last-good retained):
<snap:envelope_stale>
Never fetched:
<snap:envelope_never_fetched>

## CLI output

stdout on success:

```json
{ "format_version": 1, "data": { ... }, "fetched_at": "2026-06-11T13:50:00Z" }
```

stderr on failure (nonzero exit): a `WireError` object.
````

For every `<snap:NAME>` marker: open `tests/snapshots/wire_contract__NAME.snap`, copy the JSON body (everything below the `---` insta header) into a fenced ```json block.

- [ ] **Step 2: Verify no markers remain**

```bash
grep -c "<snap:" CONTRACT.md
```

Expected: `0`.

- [ ] **Step 3: Commit**

```bash
git add CONTRACT.md
git commit -m "docs: add CONTRACT.md wire contract v1"
```

---

### Task 10: API.md update, CHANGELOG, 0.7.0 bump, final verification

**Files:**
- Modify: `API.md:62, 81-96`
- Create: `CHANGELOG.md`
- Modify: `Cargo.toml:3`

- [ ] **Step 1: Update API.md air-quality section**

Replace (line 62):

```markdown
The headline `aqi` and its `standard`/`category` come from the World Air
```

with:

```markdown
The headline `aqi` and its `category` come from the World Air
```

Replace (lines 81-85):

```markdown
Returns `AirQualityData`:
- `aqi: i32` -- the raw index value
- `standard: AqiStandard` -- `Us` or `European`
- `category: AqiCategory` -- `Us(UsAqiCategory)` or `Eu(EuAqiCategory)`, computed during fetch
- Pollutant readings: `pm2_5`, `pm10`, `ozone`, `nitrogen_dioxide`, `carbon_monoxide` (µg/m³, Open-Meteo)
```

with:

```markdown
Returns `AirQualityData`:
- `aqi: i32` -- the raw index value
- `category: AqiCategory` -- `Us(UsAqiCategory)` or `Eu(EuAqiCategory)`, computed during fetch
- `standard()` -- method returning `AqiStandard` (`Us` or `European`), derived from `category`
- Pollutant readings: `pm2_5`, `pm10`, `ozone`, `nitrogen_dioxide`, `carbon_monoxide` (µg/m³, Open-Meteo)
```

After the AQI Categories paragraph ending `...categorize values yourself.` (line 96), append:

```markdown

`AqiCategory` serializes adjacent-tagged — `{"standard": "Us", "level": "Good"}`.
JSON shapes for all public types are frozen in [CONTRACT.md](CONTRACT.md).
```

- [ ] **Step 2: Create CHANGELOG.md**

```markdown
# Changelog

## 0.7.0 — unreleased

### Breaking
- `AirQualityData`: the `standard` field is removed. Use the new
  `standard()` method (derived from `category`).
- `AqiCategory` now serializes adjacent-tagged:
  `{"standard": "Us", "level": "Good"}` (was externally tagged
  `{"Us": "Good"}`).

### Added
- `Serialize`/`Deserialize` on all wire-crossing types: `WeatherData`,
  `AirQualityData`, `AqiCategory`, `UsAqiCategory`, `EuAqiCategory`, `Alert`,
  `AlertSeverity`, `PollenData`, `LocationResult`, `DetectedLocation`.
- `wire` module: `WireError`, `Envelope`, `EnvelopeError` — the frozen
  JSON shapes for the future daemon/CLI surfaces.
- `CONTRACT.md`: the v1 wire contract, enforced by snapshot tests
  (`tests/wire_contract.rs`).
```

- [ ] **Step 3: Bump version**

In `Cargo.toml` change `version = "0.6.0"` to `version = "0.7.0"`.

- [ ] **Step 4: Full verification**

```bash
cargo fmt && git diff --stat   # expect: no formatting churn (or commit it)
cargo clippy --all-targets     # expect: no NEW warnings vs main
cargo test                     # expect: full suite PASS
```

- [ ] **Step 5: Commit**

```bash
git add API.md CHANGELOG.md Cargo.toml Cargo.lock
git commit -m "chore: bump to 0.7.0, document wire contract in API.md and CHANGELOG"
```

- [ ] **Step 6: Merge and push (hard rule: before advancing)**

```bash
git checkout main && git merge --ff-only feat/wire-contract && git push origin main
git branch -d feat/wire-contract
git status -sb   # expect: clean, in sync with origin/main
```

Do NOT publish to crates.io — release/AUR flow is separate and not part of this plan.

---

## Self-review record

- **Spec coverage:** §1 conventions → Tasks 1/5 (canaries, null policy) ;
  §2.1 derives → Tasks 2/3/4/5 ; §2.2 restructure → Task 3 ; §2.3 (no derives
  on Region/events) → no task touches them, correct ; §2.4 WireError → Task 6 ;
  §3.1 envelope → Task 7 ; §3.2 D-Bus names → CONTRACT.md (Task 9; code lands
  with the daemon) ; §3.3 CLI → CONTRACT.md (Task 9; code lands with the CLI) ;
  §4.1-4.5 tests → Tasks 1-8 ; §5 CONTRACT.md → Task 9 ; §7 bump → Task 10.
- **Type consistency:** `WireError { kind, message }`, `EnvelopeError { kind,
  message, at }`, `Envelope { data, fetched_at, error }` used identically in
  Tasks 6, 7, 9. `standard()` method defined Task 3, referenced Tasks 9, 10.
- **Known judgment call:** snapshot-count floor in Task 8 (`>= 25`) counts:
  5 canaries + 4 Task-1 shapes + 1 weather_data + 2 air_quality + 2 alert
  (incl. severity canary) + 4 pollen/location + 8 wire_error + 3 envelope
  = 29 ≥ 25. If a future task legitimately removes snapshots, adjust with care.
```
