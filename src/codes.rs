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
    /// Converts degrees (0-360) to the nearest compass direction.
    pub fn from_degrees(degrees: i32) -> Self {
        match degrees {
            0..=22 | 338..=360 => Self::N,
            23..=67 => Self::NE,
            68..=112 => Self::E,
            113..=157 => Self::SE,
            158..=202 => Self::S,
            203..=247 => Self::SW,
            248..=292 => Self::W,
            293..=337 => Self::NW,
            _ => Self::N,
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
