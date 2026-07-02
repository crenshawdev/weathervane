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

    #[test]
    fn weather_code_maps_remaining_wmo_arms() {
        assert_eq!(
            WeatherCondition::from_code(1),
            WeatherCondition::MainlyClear
        );
        assert_eq!(WeatherCondition::from_code(3), WeatherCondition::Overcast);
        assert_eq!(WeatherCondition::from_code(51), WeatherCondition::Drizzle);
        assert_eq!(WeatherCondition::from_code(53), WeatherCondition::Drizzle);
        assert_eq!(WeatherCondition::from_code(55), WeatherCondition::Drizzle);
        assert_eq!(
            WeatherCondition::from_code(56),
            WeatherCondition::FreezingDrizzle
        );
        assert_eq!(
            WeatherCondition::from_code(57),
            WeatherCondition::FreezingDrizzle
        );
        assert_eq!(WeatherCondition::from_code(63), WeatherCondition::Rain);
        assert_eq!(WeatherCondition::from_code(65), WeatherCondition::Rain);
        assert_eq!(
            WeatherCondition::from_code(66),
            WeatherCondition::FreezingRain
        );
        assert_eq!(
            WeatherCondition::from_code(67),
            WeatherCondition::FreezingRain
        );
        assert_eq!(WeatherCondition::from_code(73), WeatherCondition::Snow);
        assert_eq!(WeatherCondition::from_code(75), WeatherCondition::Snow);
        assert_eq!(
            WeatherCondition::from_code(77),
            WeatherCondition::SnowGrains
        );
        assert_eq!(
            WeatherCondition::from_code(80),
            WeatherCondition::RainShowers
        );
        assert_eq!(
            WeatherCondition::from_code(82),
            WeatherCondition::RainShowers
        );
        assert_eq!(
            WeatherCondition::from_code(85),
            WeatherCondition::SnowShowers
        );
        assert_eq!(
            WeatherCondition::from_code(86),
            WeatherCondition::SnowShowers
        );
        assert_eq!(
            WeatherCondition::from_code(96),
            WeatherCondition::ThunderstormHail
        );
    }

    #[test]
    fn icon_name_clear_sky_day() {
        assert_eq!(
            WeatherCondition::ClearSky.icon_name(false),
            "weather-clear-symbolic"
        );
    }

    #[test]
    fn icon_name_clear_sky_night() {
        assert_eq!(
            WeatherCondition::ClearSky.icon_name(true),
            "weather-clear-night-symbolic"
        );
    }

    #[test]
    fn icon_name_mainly_clear_day() {
        assert_eq!(
            WeatherCondition::MainlyClear.icon_name(false),
            "weather-few-clouds-symbolic"
        );
    }

    #[test]
    fn icon_name_mainly_clear_night() {
        assert_eq!(
            WeatherCondition::MainlyClear.icon_name(true),
            "weather-few-clouds-night-symbolic"
        );
    }

    #[test]
    fn icon_name_partly_cloudy_day() {
        assert_eq!(
            WeatherCondition::PartlyCloudy.icon_name(false),
            "weather-few-clouds-symbolic"
        );
    }

    #[test]
    fn icon_name_partly_cloudy_night() {
        assert_eq!(
            WeatherCondition::PartlyCloudy.icon_name(true),
            "weather-few-clouds-night-symbolic"
        );
    }

    #[test]
    fn icon_name_overcast() {
        assert_eq!(
            WeatherCondition::Overcast.icon_name(false),
            "weather-overcast-symbolic"
        );
    }

    #[test]
    fn icon_name_foggy() {
        assert_eq!(
            WeatherCondition::Foggy.icon_name(false),
            "weather-fog-symbolic"
        );
    }

    #[test]
    fn icon_name_drizzle_and_freezing_drizzle_share_symbol() {
        assert_eq!(
            WeatherCondition::Drizzle.icon_name(false),
            "weather-showers-scattered-symbolic"
        );
        assert_eq!(
            WeatherCondition::FreezingDrizzle.icon_name(false),
            "weather-showers-scattered-symbolic"
        );
    }

    #[test]
    fn icon_name_rain_family_shares_symbol() {
        assert_eq!(
            WeatherCondition::Rain.icon_name(false),
            "weather-showers-symbolic"
        );
        assert_eq!(
            WeatherCondition::FreezingRain.icon_name(false),
            "weather-showers-symbolic"
        );
        assert_eq!(
            WeatherCondition::RainShowers.icon_name(false),
            "weather-showers-symbolic"
        );
    }

    #[test]
    fn icon_name_snow_family_shares_symbol() {
        assert_eq!(
            WeatherCondition::Snow.icon_name(false),
            "weather-snow-symbolic"
        );
        assert_eq!(
            WeatherCondition::SnowGrains.icon_name(false),
            "weather-snow-symbolic"
        );
        assert_eq!(
            WeatherCondition::SnowShowers.icon_name(false),
            "weather-snow-symbolic"
        );
    }

    #[test]
    fn icon_name_thunderstorm_family_shares_symbol() {
        assert_eq!(
            WeatherCondition::Thunderstorm.icon_name(false),
            "weather-storm-symbolic"
        );
        assert_eq!(
            WeatherCondition::ThunderstormHail.icon_name(false),
            "weather-storm-symbolic"
        );
    }

    #[test]
    fn icon_name_unknown_returns_severe_alert() {
        assert_eq!(
            WeatherCondition::Unknown.icon_name(false),
            "weather-severe-alert-symbolic"
        );
    }

    #[test]
    fn compass_direction_as_str_matches_labels() {
        assert_eq!(CompassDirection::N.as_str(), "N");
        assert_eq!(CompassDirection::NE.as_str(), "NE");
        assert_eq!(CompassDirection::E.as_str(), "E");
        assert_eq!(CompassDirection::SE.as_str(), "SE");
        assert_eq!(CompassDirection::S.as_str(), "S");
        assert_eq!(CompassDirection::SW.as_str(), "SW");
        assert_eq!(CompassDirection::W.as_str(), "W");
        assert_eq!(CompassDirection::NW.as_str(), "NW");
    }

    #[test]
    fn compass_direction_from_degrees_interior_arcs() {
        assert_eq!(CompassDirection::from_degrees(0), CompassDirection::N);
        assert_eq!(CompassDirection::from_degrees(45), CompassDirection::NE);
        assert_eq!(CompassDirection::from_degrees(90), CompassDirection::E);
        assert_eq!(CompassDirection::from_degrees(135), CompassDirection::SE);
        assert_eq!(CompassDirection::from_degrees(180), CompassDirection::S);
        assert_eq!(CompassDirection::from_degrees(225), CompassDirection::SW);
        assert_eq!(CompassDirection::from_degrees(270), CompassDirection::W);
        assert_eq!(CompassDirection::from_degrees(315), CompassDirection::NW);
    }
}
