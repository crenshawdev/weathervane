// SPDX-License-Identifier: MIT OR Apache-2.0

//! WMO weather codes and compass direction mappings.

use serde::{Deserialize, Serialize};

/// WMO weather condition mapped from numeric weathercode.
///
/// Core returns the enum variant, frontends match it to produce translated strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeatherCondition {
    /// WMO code 0.
    ClearSky,
    /// WMO code 1.
    MainlyClear,
    /// WMO code 2.
    PartlyCloudy,
    /// WMO code 3.
    Overcast,
    /// WMO codes 45, 48.
    Foggy,
    /// WMO codes 51, 53, 55.
    Drizzle,
    /// WMO codes 56, 57.
    FreezingDrizzle,
    /// WMO codes 61, 63, 65.
    Rain,
    /// WMO codes 66, 67.
    FreezingRain,
    /// WMO codes 71, 73, 75.
    Snow,
    /// WMO code 77.
    SnowGrains,
    /// WMO codes 80-82.
    RainShowers,
    /// WMO codes 85, 86.
    SnowShowers,
    /// WMO code 95.
    Thunderstorm,
    /// WMO codes 96, 99.
    ThunderstormHail,
    /// Anything outside the WMO range.
    Unknown,
}

impl WeatherCondition {
    /// Maps a WMO weather code to a condition variant.
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Self::ClearSky,
            1 => Self::MainlyClear,
            2 => Self::PartlyCloudy,
            3 => Self::Overcast,
            45 | 48 => Self::Foggy,
            51 | 53 | 55 => Self::Drizzle,
            56 | 57 => Self::FreezingDrizzle,
            61 | 63 | 65 => Self::Rain,
            66 | 67 => Self::FreezingRain,
            71 | 73 | 75 => Self::Snow,
            77 => Self::SnowGrains,
            80..=82 => Self::RainShowers,
            85 | 86 => Self::SnowShowers,
            95 => Self::Thunderstorm,
            96 | 99 => Self::ThunderstormHail,
            _ => Self::Unknown,
        }
    }

    /// Returns the freedesktop icon name for this condition.
    /// Uses the -symbolic suffix for proper icon lookup across themes.
    pub fn icon_name(&self, is_night: bool) -> &'static str {
        match self {
            Self::ClearSky => {
                if is_night {
                    "weather-clear-night-symbolic"
                } else {
                    "weather-clear-symbolic"
                }
            }
            Self::MainlyClear | Self::PartlyCloudy => {
                if is_night {
                    "weather-few-clouds-night-symbolic"
                } else {
                    "weather-few-clouds-symbolic"
                }
            }
            Self::Overcast => "weather-overcast-symbolic",
            Self::Foggy => "weather-fog-symbolic",
            Self::Drizzle | Self::FreezingDrizzle => "weather-showers-scattered-symbolic",
            Self::Rain | Self::FreezingRain | Self::RainShowers => "weather-showers-symbolic",
            Self::Snow | Self::SnowGrains | Self::SnowShowers => "weather-snow-symbolic",
            Self::Thunderstorm | Self::ThunderstormHail => "weather-storm-symbolic",
            Self::Unknown => "weather-severe-alert-symbolic",
        }
    }
}

/// Cardinal/intercardinal compass direction from wind bearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompassDirection {
    /// North (338-360, 0-22 degrees).
    N,
    /// Northeast (23-67 degrees).
    NE,
    /// East (68-112 degrees).
    E,
    /// Southeast (113-157 degrees).
    SE,
    /// South (158-202 degrees).
    S,
    /// Southwest (203-247 degrees).
    SW,
    /// West (248-292 degrees).
    W,
    /// Northwest (293-337 degrees).
    NW,
}

impl CompassDirection {
    /// Converts degrees to the nearest compass direction.
    /// Normalizes to 0-359 first, so negative values and >360 are handled.
    pub fn from_degrees(degrees: i32) -> Self {
        match degrees.rem_euclid(360) {
            0..=22 | 338..=359 => Self::N,
            23..=67 => Self::NE,
            68..=112 => Self::E,
            113..=157 => Self::SE,
            158..=202 => Self::S,
            203..=247 => Self::SW,
            248..=292 => Self::W,
            293..=337 => Self::NW,
            _ => unreachable!("rem_euclid(360) always produces 0..=359"),
        }
    }

    /// Returns the short compass label (e.g. "NE", "SW").
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::N => "N",
            Self::NE => "NE",
            Self::E => "E",
            Self::SE => "SE",
            Self::S => "S",
            Self::SW => "SW",
            Self::W => "W",
            Self::NW => "NW",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass_segment_boundaries() {
        assert_eq!(CompassDirection::from_degrees(0), CompassDirection::N);
        assert_eq!(CompassDirection::from_degrees(22), CompassDirection::N);
        assert_eq!(CompassDirection::from_degrees(23), CompassDirection::NE);
        assert_eq!(CompassDirection::from_degrees(90), CompassDirection::E);
        assert_eq!(CompassDirection::from_degrees(180), CompassDirection::S);
        assert_eq!(CompassDirection::from_degrees(270), CompassDirection::W);
        assert_eq!(CompassDirection::from_degrees(337), CompassDirection::NW);
        assert_eq!(CompassDirection::from_degrees(338), CompassDirection::N);
    }

    #[test]
    fn compass_normalizes_out_of_range_degrees() {
        // rem_euclid handles negatives and values past 360 without panicking.
        assert_eq!(CompassDirection::from_degrees(360), CompassDirection::N);
        assert_eq!(CompassDirection::from_degrees(720), CompassDirection::N);
        assert_eq!(CompassDirection::from_degrees(-45), CompassDirection::NW);
        assert_eq!(CompassDirection::from_degrees(-90), CompassDirection::W);
    }

    #[test]
    fn weather_code_maps_known_conditions() {
        assert_eq!(WeatherCondition::from_code(0), WeatherCondition::ClearSky);
        assert_eq!(
            WeatherCondition::from_code(2),
            WeatherCondition::PartlyCloudy
        );
        assert_eq!(WeatherCondition::from_code(45), WeatherCondition::Foggy);
        assert_eq!(WeatherCondition::from_code(48), WeatherCondition::Foggy);
        assert_eq!(WeatherCondition::from_code(61), WeatherCondition::Rain);
        assert_eq!(WeatherCondition::from_code(71), WeatherCondition::Snow);
        assert_eq!(
            WeatherCondition::from_code(81),
            WeatherCondition::RainShowers
        );
        assert_eq!(
            WeatherCondition::from_code(95),
            WeatherCondition::Thunderstorm
        );
        assert_eq!(
            WeatherCondition::from_code(99),
            WeatherCondition::ThunderstormHail
        );
    }

    #[test]
    fn weather_code_unknown_outside_wmo_range() {
        assert_eq!(WeatherCondition::from_code(100), WeatherCondition::Unknown);
        assert_eq!(WeatherCondition::from_code(-1), WeatherCondition::Unknown);
    }
}
