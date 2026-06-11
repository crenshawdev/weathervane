// SPDX-License-Identifier: MIT OR Apache-2.0

//! Weather domain logic for the weathervane family of applications.
//!
//! This crate is purely reactive: no polling, no timers, no config storage.
//! It exposes async functions that frontends call when they decide it's time.
//! The frontend owns all scheduling, config persistence, and passes the
//! relevant parameters (coordinates, units, etc.) on each call.

mod air_quality_aqicn;
mod client;
mod geo;
mod weather_jma;

pub mod air_quality;
pub mod alerts;
pub mod codes;
pub mod error;
pub mod location;
pub mod network;
pub mod pollen;
pub mod sleep;
pub mod time;
pub mod units;
pub mod weather;
pub mod wire;

// Re-export the primary types at crate root for convenience.
pub use air_quality::{AirQualityData, AqiCategory, AqiStandard, EuAqiCategory, UsAqiCategory};
pub use alerts::{Alert, AlertSeverity};
pub use codes::{CompassDirection, WeatherCondition};
pub use error::{Error, NetworkKind, ParseKind, Result};
pub use geo::{detect_region, Region};
pub use location::{DetectedLocation, LocationResult, SavedLocation};
pub use network::NetworkEvent;
pub use pollen::PollenData;
pub use sleep::SleepEvent;
pub use time::ParsedDate;
pub use units::{MeasurementSystem, PressureUnit, TemperatureUnit};
pub use weather::{CurrentWeather, DailyForecast, HourlyForecast, WeatherData};
pub use wire::WireError;

// Re-export the async functions at crate root.
pub use air_quality::fetch_air_quality;
pub use alerts::fetch_alerts;
pub use client::reset_http_client;
pub use location::{detect_location, search_city, uses_imperial_units};
pub use network::network_stream;
pub use pollen::fetch_pollen;
pub use sleep::sleep_stream;
pub use time::{format_hour, format_time, is_night_time};
pub use weather::fetch_weather;
