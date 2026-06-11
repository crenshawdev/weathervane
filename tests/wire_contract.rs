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
    DailyForecast, DetectedLocation, Error, EuAqiCategory, HourlyForecast, LocationResult,
    MeasurementSystem, NetworkKind, ParseKind, PollenData, PressureUnit, SavedLocation,
    TemperatureUnit, UsAqiCategory, WeatherCondition, WeatherData, WireError,
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

// ---------------------------------------------------------------------------
// Air quality shapes
// ---------------------------------------------------------------------------

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
    insta::assert_snapshot!("air_quality_us", round_trip(&air_quality_us()));
    insta::assert_snapshot!("air_quality_eu", round_trip(&air_quality_eu()));
}

/// The adjacent tag must round-trip Eu(Good) to Eu(Good) — the exact bug an
/// untagged representation would have shipped ("Good" exists in both scales).
#[test]
fn aqi_category_round_trips_exactly() {
    let eu = AqiCategory::Eu(EuAqiCategory::Good);
    let back: AqiCategory = serde_json::from_str(&serde_json::to_string(&eu).unwrap()).unwrap();
    assert_eq!(back, eu);

    let us = AqiCategory::Us(UsAqiCategory::Good);
    let back: AqiCategory = serde_json::from_str(&serde_json::to_string(&us).unwrap()).unwrap();
    assert_eq!(back, us);
}

#[test]
fn aqi_standard_method_derives_from_category() {
    use weathervane::AqiStandard;
    assert_eq!(air_quality_us().standard(), AqiStandard::Us);
    assert_eq!(air_quality_eu().standard(), AqiStandard::European);
}

// ---------------------------------------------------------------------------
// Alerts
// ---------------------------------------------------------------------------

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
    let pretty = round_trip(&alert());
    // Contract: alert times are UTC instants in RFC3339 with Z.
    assert!(
        pretty.contains("\"2026-06-11T22:00:00Z\""),
        "expires must be RFC3339 Z"
    );
    insta::assert_snapshot!("alert", pretty);
    insta::assert_snapshot!("canary_alert_severity", json(&AlertSeverity::Severe));
}

// ---------------------------------------------------------------------------
// Pollen and location shapes
// ---------------------------------------------------------------------------

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
    insta::assert_snapshot!("pollen_some", round_trip(&Some(pollen())));
    // Contract (Option policy): outside CAMS coverage pollen is null —
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
    insta::assert_snapshot!("location_result", round_trip(&result));

    let detected = DetectedLocation {
        latitude: 45.52,
        longitude: -122.68,
        display_name: "Portland, United States".to_string(),
        country: "United States".to_string(),
    };
    insta::assert_snapshot!("detected_location", round_trip(&detected));
}

// ---------------------------------------------------------------------------
// WireError shapes and PII contracts
// ---------------------------------------------------------------------------

/// One WireError exemplar per Error variant — the `kind` strings are contract.
fn wire_error_exemplars() -> Vec<(&'static str, Error)> {
    vec![
        ("Timeout", Error::Timeout),
        ("Network", Error::Network(NetworkKind::Connect)),
        ("HttpStatus", Error::HttpStatus(429)),
        ("Parse", Error::Parse(ParseKind::Xml)),
        (
            "HttpClient",
            Error::HttpClient("tls backend not initialized".to_string()),
        ),
        (
            "NoResults",
            Error::NoResults {
                query: "Portlandia".to_string(),
            },
        ),
        ("LocationDetection", Error::LocationDetection),
        ("Dbus", Error::Dbus("name lost".to_string())),
    ]
}

#[test]
fn wire_error_shapes() {
    for (expected_kind, err) in wire_error_exemplars() {
        let wire = WireError::from(&err);
        assert_eq!(wire.kind, expected_kind);
        insta::assert_snapshot!(format!("wire_error_{expected_kind}"), round_trip(&wire));
    }
}

/// PII contract: error payloads never carry the user's search text. The
/// Display impl for NoResults deliberately omits the query; this pins that.
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

/// PII contract: fixture coordinates must not appear in any error payload.
#[test]
fn wire_error_never_leaks_coordinates() {
    for (_, err) in wire_error_exemplars() {
        let serialized = serde_json::to_string(&WireError::from(&err)).unwrap();
        assert!(
            !serialized.contains("45.5152"),
            "latitude leaked: {serialized}"
        );
        assert!(
            !serialized.contains("-122.6784"),
            "longitude leaked: {serialized}"
        );
    }
}

/// Documents that Dbus/HttpClient messages ARE passed through verbatim —
/// they carry library-generated strings only; construction sites must never
/// embed user input or request URLs (which contain coordinates).
#[test]
fn wire_error_passthrough_is_verbatim_but_library_generated() {
    let wire = WireError::from(&Error::Dbus("name lost".to_string()));
    assert_eq!(wire.message, "D-Bus error: name lost");

    let wire = WireError::from(&Error::HttpClient(
        "tls backend not initialized".to_string(),
    ));
    assert_eq!(
        wire.message,
        "failed to build HTTP client: tls backend not initialized"
    );
}
