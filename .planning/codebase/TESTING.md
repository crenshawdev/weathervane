# Testing Patterns

**Analysis Date:** 2026-06-30

## Test Framework

**Test Runner:**
- Rust built-in test framework via `cargo test`
- Config: No custom test configuration (uses defaults)
- Edition: 2021

**Assertion Library:**
- Standard Rust: `assert!()`, `assert_eq!()`, `assert_ne!()`
- Snapshot testing: `insta` 1.0 for wire contract freezing
- No custom assertion libraries

**Run Commands:**
```bash
cargo test --workspace              # Run all tests (unit + integration + doc)
cargo test --workspace -- --test-threads=1  # Sequential run (if needed)
cargo test --workspace -- --nocapture       # Show println!/tracing output
cargo insta review                  # Accept/reject snapshot changes interactively
cargo llvm-cov --workspace          # Generate coverage (with llvm-tools-preview)
cargo llvm-cov report --summary-only    # Print coverage summary
cargo llvm-cov report --cobertura   # Cobertura XML for CI
```

## Test File Organization

**Location:**
- Integration tests: `tests/` directory at repo root
- Unit tests: `#[cfg(test)] mod tests { ... }` within each module
- Test data fixtures: Helper functions within test modules (no separate data files)

**Naming:**
- Integration test file: `tests/wire_contract.rs`
- Test function names are descriptive: `pressure_hpa_passthrough()`, `no_uppercase_json_keys_in_snapshots()`
- Snapshot names match test function names: test `current_weather_shape()` → `wire_contract__current_weather.snap`
- Double underscore separates test name from snapshot: `wire_contract__` prefix for all snapshots from `wire_contract.rs`

**Structure:**
```
tests/
├── wire_contract.rs            # All wire contract tests (JSON serialization)
└── snapshots/
    ├── wire_contract__current_weather.snap
    ├── wire_contract__alert.snap
    ├── wire_contract__pollen_none.snap
    └── ... (29+ snapshots total, one per test)

src/
├── units.rs                     # Contains inline #[cfg(test)] mod tests
├── time.rs                      # Contains inline #[cfg(test)] mod tests
└── ... (other modules with inline tests)
```

## Test Structure

**Integration Test Suite Pattern (wire_contract.rs):**
```rust
// Module documentation
//! Wire contract enforcement (docs/superpowers/specs/2026-06-11-wire-contract-design.md).

use chrono::{TimeZone, Utc};
use serde::{de::DeserializeOwned, Serialize};
use weathervane::{ /* types */ };

// Helper functions (not tests, but test utilities)
fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap()
}

fn round_trip<T: Serialize + DeserializeOwned>(value: &T) -> String {
    // Serialize → deserialize → re-serialize must be lossless
    // ...
}

// Fixtures (fully populated test data)
fn current_weather() -> CurrentWeather {
    CurrentWeather { /* all fields */ }
}

// Test functions
#[test]
fn current_weather_shape() {
    insta::assert_snapshot!("current_weather", round_trip(&current_weather()));
}
```

**Unit Test Pattern (inline modules):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_hpa_passthrough() {
        assert_eq!(PressureUnit::Hpa.convert(1013.25), 1013.25);
    }

    #[test]
    fn format_hour_handles_rfc3339_and_naive() {
        assert_eq!(format_hour("2025-01-20T14:00:00+09:00", true), "14:00");
        assert_eq!(format_hour("2025-01-20T14:00", true), "14:00");
    }
}
```

**Sections:**
- Helpers (10-40 lines): Reusable test utilities with explanatory comments
- Fixtures (20-60 lines): Factory functions that return fully populated test data
- Test cases (5-20 lines each): Single focused assertion per test

## Test Types

### Wire Contract Tests (integration)
**Location:** `tests/wire_contract.rs`

**Purpose:** Freeze JSON shapes for public API surfaces; catch serialization changes before they ship

**Pattern:**
```rust
#[test]
fn current_weather_shape() {
    insta::assert_snapshot!("current_weather", round_trip(&current_weather()));
}
```

**Coverage:**
- Struct shapes: `current_weather_shape()`, `weather_data_shape()`, `alert_shape()`
- Enum spellings: `enum_spelling_canaries()` (guards PascalCase: "PartlyCloudy", "NW", "Fahrenheit")
- Round-trip serialization: deserialize what serialize produces, verify it re-serializes identically
- Legacy compatibility: `legacy_persisted_unit_spellings_still_parse()` ensures old configs still load
- Error payloads: `wire_error_shapes()` for each Error variant; `wire_error_never_leaks_query()`, `wire_error_never_leaks_coordinates()` for PII
- Envelope state: `envelope_states()` tests healthy/stale/never-fetched scenarios
- Structural guards: `no_uppercase_json_keys_in_snapshots()` scans all snapshots via regex for contract violations

**Snapshot Acceptance:**
```bash
# Review and accept/reject snapshot changes interactively
cargo insta review

# Accept all changes at once (use with caution)
INSTA_UPDATE=always cargo test wire_contract
```

### Unit Tests (inline modules)

**Location:** Within each module file in a `#[cfg(test)]` block at the end

**Examples:**
- `units.rs:143-197`: 6 tests covering pressure/visibility conversions, formatting, API params
- `time.rs:135-183`: 4 tests for date parsing, time formatting, day/night detection with timezone offsets

**Pattern:**
```rust
#[test]
fn test_name_describes_expected_behavior() {
    // Arrange: Set up fixtures
    let pressure = 1013.25;
    
    // Act: Call the function
    let result = PressureUnit::InHg.convert(pressure);
    
    // Assert: Verify result
    assert!((result - 29.92).abs() < 0.01);
}
```

### No E2E or UI Tests

Reason: This is a domain library (no UI, no daemon, no polling). The consuming applications (atmos, cosmic-ext-applet-tempest) own integration testing.

## Mocking

**Framework:** No mocking framework used (don't mock)

**Why No Mocks:**
- Network calls (fetch_weather, search_city) are real but hit public test/staging APIs
- Test data uses fixtures (helper functions returning populated structs)
- Serde/JSON round-tripping is tested directly, not mocked
- D-Bus tests (network_stream) fall back gracefully when D-Bus unavailable

**What to Mock:** (Not done in this crate, but guidance for consumers)
- At application level, wrap fetch functions behind a trait so tests can provide stubs
- In this crate: we test happy path + error conversion; callers mock at the application boundary

**What NOT to Mock:**
- JSON serialization (test real round-trips)
- Error conversions (test via real Error variants)
- Date/time parsing (test with real chrono)

## Fixtures and Factories

**Test Data Location:**
- All in `tests/wire_contract.rs` as helper functions
- Fully populated with distinctive values (71.3 for temperature, not 72.0)
- Example fixture from `wire_contract.rs:45-62`:
  ```rust
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
  ```

**No Builder Patterns:** Fixtures are direct struct construction; simple and readable

**Reuse:** Fixtures are composed (e.g., `weather_data()` calls `current_weather()`, `hourly_forecast()`, `daily_forecast()`)

## Async Testing

**Pattern:**
Standard `#[tokio::test]` is NOT used; no special async test harness. Tests calling async functions run them to completion:
```rust
#[test]
fn legacy_persisted_unit_spellings_still_parse() {
    // serde_json::from_str works synchronously
    let parsed = serde_json::from_str::<TemperatureUnit>("\"Fahrenheit\"").unwrap();
    assert_eq!(parsed, TemperatureUnit::Fahrenheit);
}
```

**Why:** Wire contract tests are synchronous (no I/O). Async integration tests would belong in applications using this crate, not here.

## Error Testing

**Pattern:**
```rust
#[test]
fn wire_error_never_leaks_query() {
    let err = Error::NoResults {
        query: "SENTINEL_QUERY_55x".to_string(),
    };
    let wire = WireError::from(&err);
    let serialized = serde_json::to_string(&wire).unwrap();
    assert!(
        !serialized.contains("SENTINEL_QUERY_55x"),
        "query text leaked: {serialized}"
    );
}
```

**Error Conversion Tests:**
- Every Error variant has an exemplar test
- `wire_error_exemplars()` in `wire_contract.rs:342-361` provides test data for each variant
- Loop over exemplars to test serialization consistency

## Coverage

**Requirements:** None enforced (coverage is measured but not gated)

**Tool:** `cargo-llvm-cov` with LLVM code coverage

**Setup (CI):**
```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
```

**View Coverage:**
```bash
# Text summary to console
cargo llvm-cov report --summary-only

# HTML report (opens in browser)
cargo llvm-cov report --html

# Cobertura XML for GitLab MR widget
cargo llvm-cov report --cobertura --output-path target/llvm-cov-target/cobertura.xml
```

**CI Integration:** `.gitlab-ci.yml:67-90` runs coverage and publishes Cobertura to MR widget (coverage-badge in interface)

## Common Patterns

### Round-trip Serialization (JSON Contract)

**Pattern:**
```rust
fn round_trip<T: Serialize + DeserializeOwned>(value: &T) -> String {
    let first = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&first).unwrap();
    let second = serde_json::to_string(&back).unwrap();
    assert_eq!(first, second, "round-trip changed the wire form");
    serde_json::to_string_pretty(&back).unwrap()
}
```

**Why:** Ensures deserialization is exact (no field reordering, no value loss). The pretty-printed version becomes the snapshot.

**Usage in tests:**
```rust
#[test]
fn air_quality_shapes() {
    insta::assert_snapshot!("air_quality_us", round_trip(&air_quality_us()));
    insta::assert_snapshot!("air_quality_eu", round_trip(&air_quality_eu()));
}
```

### Backward Compatibility (Legacy Spelling)

**Pattern:**
```rust
#[test]
fn legacy_persisted_unit_spellings_still_parse() {
    assert_eq!(
        serde_json::from_str::<TemperatureUnit>("\"Fahrenheit\"").unwrap(),
        TemperatureUnit::Fahrenheit
    );
    // ... more variants
}
```

**Why:** Applications have saved config JSON with old enum spellings. Any #[serde(rename)] would break deserialization; this test pins the contract.

### Default Field Behavior (Backward Addition)

**Pattern (from wire_contract.rs:261-266):**
```rust
#[test]
fn air_quality_without_aqi_source_defaults_to_open_meteo() {
    let legacy = r#"{"aqi":42,"category":{...},...}"#;  // 7 keys, no aqi_source
    let parsed: AirQualityData = serde_json::from_str(legacy).unwrap();
    assert_eq!(parsed.aqi_source, AqiSource::OpenMeteo);
}
```

**Why:** New fields must have `#[serde(default)]` so old JSON (before the field existed) still parses, defaulting the missing field.

### Enum Tagging (Ambiguous Variants)

**Test from wire_contract.rs:241-250:**
```rust
#[test]
fn aqi_category_round_trips_exactly() {
    let eu = AqiCategory::Eu(EuAqiCategory::Good);
    let back: AqiCategory = serde_json::from_str(&serde_json::to_string(&eu).unwrap()).unwrap();
    assert_eq!(back, eu);

    let us = AqiCategory::Us(UsAqiCategory::Good);
    // ... same check
}
```

**Why:** Both US and EU scales have a "Good" level. Adjacent tagging (`#[serde(tag = "standard", content = "level")]`) disambiguates on deserialization. This test guards against untagged serialization which would lose the standard.

### PII Auditing (Error Payloads)

**Pattern:**
```rust
#[test]
fn wire_error_passthrough_is_verbatim_but_library_generated() {
    let wire = WireError::from(&Error::Dbus("name lost".to_string()));
    assert_eq!(wire.message, "D-Bus error: name lost");
    // ...
}
```

**Coverage:**
- `wire_error_never_leaks_query()`: Searches serialized error for sentinel query string
- `wire_error_never_leaks_coordinates()`: Searches serialized error for fixture lat/lon
- `wire_error_passthrough_is_verbatim_but_library_generated()`: Verifies passthrough fields contain only library-generated text (not URLs with coordinates)

## Test Execution (CI)

**Order (from .gitlab-ci.yml):**
1. **check stage:** `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`
2. **build stage:** `cargo build --workspace`
3. **test stage:** `cargo test --workspace`, `cargo llvm-cov ...`

**Run Conditions:**
- Merge requests (triggered automatically)
- Pushes to main branch (triggered automatically)

**Failure Handling:**
- Any test failure fails the pipeline
- Snapshot changes (test output mismatch) fail the test; use `cargo insta review` to accept
- Clippy warnings are errors; fix or annotate

---

*Testing analysis: 2026-06-30*
