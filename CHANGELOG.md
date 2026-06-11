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
