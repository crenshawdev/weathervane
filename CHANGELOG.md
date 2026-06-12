# Changelog

## 0.8.0

### Added
- `MeasurementSystem::precipitation_unit()` — returns the display label for
  precipitation depth: `"in"` (Imperial) or `"mm"` (Metric). Distinct from
  `precipitation_api_param()` (`"inch"`/`"mm"`), which is unchanged.
- `AirQualityData.aqi_source: AqiSource` — records which provider supplied the
  headline AQI for a given fetch: `Aqicn` when aqicn.org/US EPA data was used,
  `OpenMeteo` otherwise (Europe, no token, or aqicn fallback).
- `AqiSource { Aqicn, OpenMeteo }` enum (re-exported at the crate root as
  `weathervane::AqiSource`).

### Back-compat
- `aqi_source` is `#[serde(default)]` and defaults to `OpenMeteo`, so existing
  serialized `AirQualityData` payloads without the field still deserialize
  cleanly.

## 0.7.0

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
