// SPDX-License-Identifier: MIT OR Apache-2.0

//! Wire contract enforcement (docs/superpowers/specs/2026-06-11-wire-contract-design.md).
//!
//! The committed snapshots under tests/snapshots/ ARE the frozen JSON contract.
//! A failing snapshot test means a wire-shape change: either revert it, or
//! deliberately accept it as a breaking contract change (INSTA_UPDATE=always /
//! cargo insta review) and treat the release accordingly.

use serde::{de::DeserializeOwned, Serialize};
use weathervane::{
    CompassDirection, CurrentWeather, DailyForecast, HourlyForecast, MeasurementSystem,
    PressureUnit, SavedLocation, TemperatureUnit, WeatherCondition, WeatherData,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Serializes pretty (snapshot input) — the canonical wire form.
fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap()
}

/// Serialize → deserialize → re-serialize must be lossless. Returns the
/// PRETTY form of the round-tripped value, so snapshots capture the bytes
/// that survived the trip. Compares JSON strings (not values) deliberately:
/// several wire types have no PartialEq, and serde-derived structs serialize
/// with deterministic field order, so string equality is exact here.
fn round_trip<T: Serialize + DeserializeOwned>(value: &T) -> String {
    let first = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&first).unwrap();
    let second = serde_json::to_string(&back).unwrap();
    assert_eq!(first, second, "round-trip changed the wire form");
    serde_json::to_string_pretty(&back).unwrap()
}

// ---------------------------------------------------------------------------
// Fixtures — fully populated, distinctive values
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

fn weather_data() -> WeatherData {
    WeatherData {
        current: current_weather(),
        hourly: vec![hourly_forecast()],
        forecast: vec![daily_forecast()],
        utc_offset_seconds: -14400,
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
// Enum spelling canaries (PascalCase baseline guards)
// ---------------------------------------------------------------------------

#[test]
fn enum_spelling_canaries() {
    insta::assert_snapshot!(
        "canary_weather_condition",
        json(&WeatherCondition::PartlyCloudy)
    );
    insta::assert_snapshot!("canary_compass_direction", json(&CompassDirection::NW));
    insta::assert_snapshot!(
        "canary_temperature_unit",
        json(&TemperatureUnit::Fahrenheit)
    );
    insta::assert_snapshot!("canary_pressure_unit", json(&PressureUnit::Hpa));
    insta::assert_snapshot!(
        "canary_measurement_system",
        json(&MeasurementSystem::Imperial)
    );
}

// ---------------------------------------------------------------------------
// Already-serializable struct shapes
// ---------------------------------------------------------------------------

#[test]
fn current_weather_shape() {
    insta::assert_snapshot!("current_weather", round_trip(&current_weather()));
}

#[test]
fn hourly_forecast_shape() {
    insta::assert_snapshot!("hourly_forecast", round_trip(&hourly_forecast()));
}

#[test]
fn daily_forecast_shape() {
    insta::assert_snapshot!("daily_forecast", round_trip(&daily_forecast()));
}

#[test]
fn saved_location_shape() {
    insta::assert_snapshot!("saved_location", round_trip(&saved_location()));
}

// ---------------------------------------------------------------------------
// Tempest config-compat guards: the literal spellings that already exist in
// users' persisted configs MUST keep parsing. These fail if anyone ever adds
// a #[serde(rename_all)] to these types.
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
    assert_eq!(
        serde_json::from_str::<PressureUnit>("\"Hpa\"").unwrap(),
        PressureUnit::Hpa
    );
    assert_eq!(
        serde_json::from_str::<PressureUnit>("\"InHg\"").unwrap(),
        PressureUnit::InHg
    );
    assert_eq!(
        serde_json::from_str::<PressureUnit>("\"Psi\"").unwrap(),
        PressureUnit::Psi
    );
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

#[test]
fn weather_data_shape() {
    insta::assert_snapshot!("weather_data", round_trip(&weather_data()));
}
