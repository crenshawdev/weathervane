// SPDX-License-Identifier: MIT OR Apache-2.0

//! Unit types and conversions for temperature, pressure, and measurement systems.

use serde::{Deserialize, Serialize};

/// Temperature scale for weather data requests and display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemperatureUnit {
    /// Fahrenheit. Default because the US uses it and Open-Meteo defaults to Celsius anyway.
    #[default]
    Fahrenheit,
    /// Celsius.
    Celsius,
}

impl TemperatureUnit {
    /// Returns the display symbol (e.g. "°F").
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Fahrenheit => "°F",
            Self::Celsius => "°C",
        }
    }

    /// Returns the Open-Meteo API parameter value.
    pub fn api_param(&self) -> &'static str {
        match self {
            Self::Fahrenheit => "fahrenheit",
            Self::Celsius => "celsius",
        }
    }

    /// Formats a temperature value with the unit symbol.
    pub fn format(&self, temp: f32) -> String {
        format!("{:.0}{}", temp, self.symbol())
    }
}

/// Pressure display unit. The API always returns hPa, so we convert client-side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PressureUnit {
    /// Hectopascals (millibars). The SI default.
    #[default]
    Hpa,
    /// Inches of mercury. Common in US weather reporting.
    InHg,
    /// Pounds per square inch. Rarely used for weather but some people want it.
    Psi,
}

impl PressureUnit {
    /// Returns the unit label for display.
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Hpa => "hPa",
            Self::InHg => "inHg",
            Self::Psi => "PSI",
        }
    }

    /// Converts a pressure value from hPa to the target unit.
    pub fn convert(&self, hpa: f32) -> f32 {
        match self {
            Self::Hpa => hpa,
            Self::InHg => hpa / 33.8639,
            Self::Psi => hpa / 68.9476,
        }
    }

    /// Formats a pressure value (in hPa) with conversion and unit symbol.
    pub fn format(&self, hpa: f32) -> String {
        let value = self.convert(hpa);
        match self {
            Self::Hpa => format!("{:.0} {}", value, self.symbol()),
            Self::InHg => format!("{:.2} {}", value, self.symbol()),
            Self::Psi => format!("{:.1} {}", value, self.symbol()),
        }
    }
}

/// Measurement system for non-temperature units (wind speed, visibility, etc.)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MeasurementSystem {
    /// Miles, mph, etc.
    #[default]
    Imperial,
    /// Kilometers, km/h, etc.
    Metric,
}

impl MeasurementSystem {
    /// Returns the wind speed unit label.
    pub fn wind_speed_unit(&self) -> &'static str {
        match self {
            Self::Imperial => "mph",
            Self::Metric => "km/h",
        }
    }

    /// Returns the visibility unit label.
    pub fn visibility_unit(&self) -> &'static str {
        match self {
            Self::Imperial => "mi",
            Self::Metric => "km",
        }
    }

    /// Returns the Open-Meteo API parameter for wind speed unit.
    pub fn wind_speed_api_param(&self) -> &'static str {
        match self {
            Self::Imperial => "mph",
            Self::Metric => "kmh",
        }
    }

    /// Returns the Open-Meteo API parameter for precipitation unit.
    pub fn precipitation_api_param(&self) -> &'static str {
        match self {
            Self::Imperial => "inch",
            Self::Metric => "mm",
        }
    }

    /// Converts visibility from meters to the appropriate unit.
    pub fn convert_visibility(&self, meters: f32) -> f32 {
        match self {
            Self::Imperial => meters / 1609.34,
            Self::Metric => meters / 1000.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_hpa_passthrough() {
        assert_eq!(PressureUnit::Hpa.convert(1013.25), 1013.25);
    }

    #[test]
    fn pressure_converts_to_inhg_and_psi() {
        // Standard sea-level pressure: 1013.25 hPa ≈ 29.92 inHg ≈ 14.70 PSI.
        assert!((PressureUnit::InHg.convert(1013.25) - 29.92).abs() < 0.01);
        assert!((PressureUnit::Psi.convert(1013.25) - 14.70).abs() < 0.01);
    }

    #[test]
    fn pressure_format_uses_unit_precision() {
        assert_eq!(PressureUnit::Hpa.format(1013.25), "1013 hPa");
        assert_eq!(PressureUnit::InHg.format(1013.25), "29.92 inHg");
        assert_eq!(PressureUnit::Psi.format(1013.25), "14.7 PSI");
    }

    #[test]
    fn visibility_converts_from_meters() {
        // One mile and one kilometer should round-trip to ~1.0.
        assert!((MeasurementSystem::Imperial.convert_visibility(1609.34) - 1.0).abs() < 0.001);
        assert!((MeasurementSystem::Metric.convert_visibility(1000.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn temperature_format_and_symbol() {
        assert_eq!(TemperatureUnit::Fahrenheit.format(72.4), "72°F");
        assert_eq!(TemperatureUnit::Celsius.format(22.6), "23°C");
        assert_eq!(TemperatureUnit::Fahrenheit.symbol(), "°F");
    }

    #[test]
    fn api_params_match_open_meteo() {
        assert_eq!(TemperatureUnit::Fahrenheit.api_param(), "fahrenheit");
        assert_eq!(TemperatureUnit::Celsius.api_param(), "celsius");
        assert_eq!(MeasurementSystem::Imperial.wind_speed_api_param(), "mph");
        assert_eq!(MeasurementSystem::Metric.wind_speed_api_param(), "kmh");
        assert_eq!(
            MeasurementSystem::Imperial.precipitation_api_param(),
            "inch"
        );
        assert_eq!(MeasurementSystem::Metric.precipitation_api_param(), "mm");
    }
}
