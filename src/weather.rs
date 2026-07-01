// SPDX-License-Identifier: MIT OR Apache-2.0

//! Weather data types and fetching from the Open-Meteo API.

use serde::{Deserialize, Serialize};

use crate::client::http_client;
use crate::codes::{CompassDirection, WeatherCondition};
use crate::error::Result;
use crate::geo::{is_japan_bounds, is_us_bounds};
use crate::units::{MeasurementSystem, TemperatureUnit};
use crate::weather_jma::override_current_temp;
use crate::weather_nws::{override_current_observations, NwsObservation};

/// Current weather conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentWeather {
    /// Temperature in the requested unit (Fahrenheit or Celsius).
    pub temperature: f32,
    /// Raw WMO weather code from the API.
    pub weathercode: i32,
    /// Parsed weather condition from the WMO code.
    pub condition: WeatherCondition,
    /// Wind speed in the requested unit (mph or km/h).
    pub windspeed: f32,
    /// Relative humidity as a percentage (0-100).
    pub humidity: i32,
    /// Apparent temperature accounting for wind chill and heat index.
    pub feels_like: f32,
    /// Wind bearing in degrees (0-360).
    pub wind_direction: i32,
    /// Wind bearing as a compass direction.
    pub compass_direction: CompassDirection,
    /// Wind gust speed in the requested unit.
    pub wind_gusts: f32,
    /// UV index (0-11+).
    pub uv_index: f32,
    /// Visibility in meters. Convert with [`MeasurementSystem::convert_visibility`].
    pub visibility: f32,
    /// Surface pressure in hPa. Convert with [`PressureUnit::convert`].
    pub pressure: f32,
    /// Cloud cover as a percentage (0-100).
    pub cloud_cover: i32,
    /// Dew point in the requested temperature unit.
    pub dew_point: f32,
}

/// Daily forecast data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyForecast {
    /// ISO date string (e.g. "2025-11-25").
    pub date: String,
    /// High temperature for the day.
    pub temp_max: f32,
    /// Low temperature for the day.
    pub temp_min: f32,
    /// Raw WMO weather code.
    pub weathercode: i32,
    /// Parsed weather condition.
    pub condition: WeatherCondition,
    /// Sunrise time as an ISO timestamp (local time, no timezone).
    pub sunrise: String,
    /// Sunset time as an ISO timestamp (local time, no timezone).
    pub sunset: String,
}

/// Hourly forecast data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyForecast {
    /// ISO timestamp for this hour (local time, no timezone).
    pub time: String,
    /// Temperature in the requested unit.
    pub temperature: f32,
    /// Raw WMO weather code.
    pub weathercode: i32,
    /// Parsed weather condition.
    pub condition: WeatherCondition,
    /// Chance of precipitation as a percentage (0-100).
    pub precipitation_probability: i32,
    /// Precipitation amount for this hour, in the requested unit (mm or inch).
    pub precipitation: f32,
    /// Wind speed in the requested unit (mph or km/h).
    pub windspeed: f32,
    /// Wind gust speed in the requested unit.
    pub wind_gusts: f32,
}

/// Complete weather data from a single fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherData {
    /// Current conditions at the requested location.
    pub current: CurrentWeather,
    /// Next 24 hours, one entry per hour.
    pub hourly: Vec<HourlyForecast>,
    /// 7-day forecast, one entry per day.
    pub forecast: Vec<DailyForecast>,
    /// Seconds east of UTC for the requested location, from `timezone=auto`.
    /// The `sunrise`/`sunset` and hourly `time` strings are in this offset's
    /// local frame. Pass to [`crate::time::is_night_time`] so day/night is
    /// computed at the location, not on the machine running the code.
    pub utc_offset_seconds: i32,
}

/// Fetches weather data from the Open-Meteo API.
///
/// Core calls `.api_param()` internally so callers pass typed units
/// instead of raw strings.
pub async fn fetch_weather(
    latitude: f64,
    longitude: f64,
    temperature_unit: TemperatureUnit,
    measurement_system: MeasurementSystem,
) -> Result<WeatherData> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weathercode,windspeed_10m,relative_humidity_2m,apparent_temperature,wind_direction_10m,wind_gusts_10m,uv_index,visibility,surface_pressure,cloud_cover,dewpoint_2m&hourly=temperature_2m,weathercode,precipitation_probability,precipitation,windspeed_10m,wind_gusts_10m&daily=temperature_2m_max,temperature_2m_min,weathercode,sunrise,sunset&temperature_unit={}&windspeed_unit={}&precipitation_unit={}&timezone=auto&forecast_days=7&forecast_hours=24",
        latitude,
        longitude,
        temperature_unit.api_param(),
        measurement_system.wind_speed_api_param(),
        measurement_system.precipitation_api_param(),
    );

    let response = http_client()?.get(&url).send().await?.error_for_status()?;
    let data: OpenMeteoResponse = response.json().await?;

    // Japan: swap the current temperature for AMeDAS ground truth. Any
    // failure falls through to Open-Meteo's value.
    let jma_override = if is_japan_bounds(latitude, longitude) {
        override_current_temp(latitude, longitude, temperature_unit).await
    } else {
        None
    };

    // US: NWS full current-condition overlay, applied to the built WeatherData
    // below. Mutually exclusive with the Japan path by geography (when this
    // fires, jma_override is None, and vice-versa). Failures inside
    // override_current_observations fall through to the Open-Meteo values.
    let nws = if is_us_bounds(latitude, longitude) {
        override_current_observations(latitude, longitude, temperature_unit, measurement_system)
            .await
    } else {
        None
    };

    if nws.is_some() {
        tracing::debug!("NWS override applied"); // coord-free (0.9.0 log posture)
    }

    if let Some(t) = jma_override {
        tracing::debug!(
            "AMeDAS override: {} -> {} ({:?})",
            data.current.temperature_2m,
            t,
            temperature_unit
        );
    }
    let current_temperature = resolve_current_temp(
        latitude,
        longitude,
        data.current.temperature_2m,
        jma_override,
    );

    let weather = weather_from_open_meteo(data, current_temperature);
    Ok(apply_nws_override(weather, nws.as_ref()))
}
/// Decides which current temperature to use: the AMeDAS override when the
/// coordinates fall inside Japan and an override value was returned, or the
/// raw Open-Meteo value otherwise. Lives in its own function so the override
/// decision can be unit-tested without a live network.
fn resolve_current_temp(
    latitude: f64,
    longitude: f64,
    raw_open_meteo_temp: f32,
    jma_override: Option<f32>,
) -> f32 {
    if is_japan_bounds(latitude, longitude) {
        jma_override.unwrap_or(raw_open_meteo_temp)
    } else {
        raw_open_meteo_temp
    }
}

/// Builds `WeatherData` from a decoded Open-Meteo response and the already-
/// resolved current temperature. Lives in its own function so the response
/// transform can be unit-tested against fixtures without a live network.
fn weather_from_open_meteo(data: OpenMeteoResponse, current_temperature: f32) -> WeatherData {
    // Open-Meteo returns each hourly field as its own parallel array. They are
    // normally equal length, but a partial/degraded response can return a
    // shorter array for some field — so pull every value with `.get()` and drop
    // any row that's missing one, rather than indexing and risking a panic.
    let hourly: Vec<_> = data
        .hourly
        .time
        .iter()
        .take(24)
        .enumerate()
        .filter_map(|(i, time)| {
            let weathercode = *data.hourly.weathercode.get(i)?;
            Some(HourlyForecast {
                time: time.clone(),
                temperature: *data.hourly.temperature_2m.get(i)?,
                weathercode,
                condition: WeatherCondition::from_code(weathercode),
                precipitation_probability: *data.hourly.precipitation_probability.get(i)?,
                precipitation: *data.hourly.precipitation.get(i)?,
                windspeed: *data.hourly.windspeed_10m.get(i)?,
                wind_gusts: *data.hourly.wind_gusts_10m.get(i)?,
            })
        })
        .collect();

    let forecast: Vec<_> = data
        .daily
        .time
        .iter()
        .enumerate()
        .filter_map(|(i, date)| {
            let weathercode = *data.daily.weathercode.get(i)?;
            Some(DailyForecast {
                date: date.clone(),
                temp_max: *data.daily.temperature_2m_max.get(i)?,
                temp_min: *data.daily.temperature_2m_min.get(i)?,
                weathercode,
                condition: WeatherCondition::from_code(weathercode),
                sunrise: data.daily.sunrise.get(i)?.clone(),
                sunset: data.daily.sunset.get(i)?.clone(),
            })
        })
        .collect();

    WeatherData {
        current: CurrentWeather {
            temperature: current_temperature,
            weathercode: data.current.weathercode,
            condition: WeatherCondition::from_code(data.current.weathercode),
            windspeed: data.current.windspeed_10m,
            humidity: data.current.relative_humidity_2m,
            feels_like: data.current.apparent_temperature,
            wind_direction: data.current.wind_direction_10m,
            compass_direction: CompassDirection::from_degrees(data.current.wind_direction_10m),
            wind_gusts: data.current.wind_gusts_10m,
            uv_index: data.current.uv_index,
            visibility: data.current.visibility,
            pressure: data.current.surface_pressure,
            cloud_cover: data.current.cloud_cover,
            dew_point: data.current.dewpoint_2m,
        },
        hourly,
        forecast,
        utc_offset_seconds: data.utc_offset_seconds,
    }
}

/// Overlays an optional NWS observation onto the already-built weather. Kept
/// pure (no network) so the override step is fixture-testable, mirroring
/// `resolve_current_temp` and `weather_from_open_meteo
fn apply_nws_override(mut weather: WeatherData, nws: Option<&NwsObservation>) -> WeatherData {
    if let Some(nws) = nws {
        nws.apply_to(&mut weather.current);
    }
    weather
}

/// Open-Meteo API response structure.
#[derive(Debug, Deserialize)]
struct OpenMeteoResponse {
    /// Seconds east of UTC for the location, returned because we request
    /// `timezone=auto`. Defaults to 0 (UTC) if the field is ever absent.
    #[serde(default)]
    utc_offset_seconds: i32,
    current: CurrentData,
    hourly: HourlyData,
    daily: DailyData,
}

#[derive(Debug, Deserialize)]
struct CurrentData {
    temperature_2m: f32,
    weathercode: i32,
    windspeed_10m: f32,
    relative_humidity_2m: i32,
    apparent_temperature: f32,
    wind_direction_10m: i32,
    wind_gusts_10m: f32,
    uv_index: f32,
    visibility: f32,
    surface_pressure: f32,
    cloud_cover: i32,
    dewpoint_2m: f32,
}

#[derive(Debug, Deserialize)]
struct HourlyData {
    time: Vec<String>,
    temperature_2m: Vec<f32>,
    weathercode: Vec<i32>,
    precipitation_probability: Vec<i32>,
    precipitation: Vec<f32>,
    windspeed_10m: Vec<f32>,
    wind_gusts_10m: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct DailyData {
    time: Vec<String>,
    temperature_2m_max: Vec<f32>,
    temperature_2m_min: Vec<f32>,
    weathercode: Vec<i32>,
    sunrise: Vec<String>,
    sunset: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full current-block fixture with distinct, unambiguous values for every
    /// field (weathercode 0 -> ClearSky, wind_direction_10m 90 -> E).
    const CURRENT_FIELDS_FIXTURE: &str = r#"{
        "utc_offset_seconds": -25200,
        "current": {
            "temperature_2m": 72.5,
            "weathercode": 0,
            "windspeed_10m": 8.0,
            "relative_humidity_2m": 55,
            "apparent_temperature": 71.0,
            "wind_direction_10m": 90,
            "wind_gusts_10m": 12.0,
            "uv_index": 4.5,
            "visibility": 10000.0,
            "surface_pressure": 1013.25,
            "cloud_cover": 10,
            "dewpoint_2m": 55.0
        },
        "hourly": {
            "time": ["2026-06-01T00:00"],
            "temperature_2m": [70.0],
            "weathercode": [0],
            "precipitation_probability": [10],
            "precipitation": [0.0],
            "windspeed_10m": [5.0],
            "wind_gusts_10m": [8.0]
        },
        "daily": {
            "time": ["2026-06-01"],
            "temperature_2m_max": [80.0],
            "temperature_2m_min": [60.0],
            "weathercode": [0],
            "sunrise": ["2026-06-01T05:30"],
            "sunset": ["2026-06-01T20:45"]
        }
    }"#;

    /// A minimal, valid `current` block used by tests that only care about
    /// `hourly` or `daily` parsing.
    fn minimal_current_json() -> serde_json::Value {
        serde_json::json!({
            "temperature_2m": 60.0,
            "weathercode": 0,
            "windspeed_10m": 5.0,
            "relative_humidity_2m": 50,
            "apparent_temperature": 60.0,
            "wind_direction_10m": 0,
            "wind_gusts_10m": 5.0,
            "uv_index": 1.0,
            "visibility": 10000.0,
            "surface_pressure": 1000.0,
            "cloud_cover": 0,
            "dewpoint_2m": 40.0
        })
    }

    /// A single-row `hourly` block used by tests that only care about
    /// `current` or `daily` parsing.
    fn minimal_hourly_json() -> serde_json::Value {
        serde_json::json!({
            "time": ["2026-06-01T00:00"],
            "temperature_2m": [50.0],
            "weathercode": [0],
            "precipitation_probability": [10],
            "precipitation": [0.0],
            "windspeed_10m": [5.0],
            "wind_gusts_10m": [8.0]
        })
    }

    /// A single-row `daily` block used by tests that only care about
    /// `current` or `hourly` parsing.
    fn minimal_daily_json() -> serde_json::Value {
        serde_json::json!({
            "time": ["2026-06-01"],
            "temperature_2m_max": [80.0],
            "temperature_2m_min": [60.0],
            "weathercode": [0],
            "sunrise": ["2026-06-01T05:30"],
            "sunset": ["2026-06-01T20:45"]
        })
    }

    #[test]
    fn open_meteo_current_fields_decode() {
        let data: OpenMeteoResponse = serde_json::from_str(CURRENT_FIELDS_FIXTURE).unwrap();
        let result = weather_from_open_meteo(data, 72.5);

        assert_eq!(result.current.temperature, 72.5);
        assert_eq!(result.current.weathercode, 0);
        assert_eq!(result.current.condition, WeatherCondition::ClearSky);
        assert_eq!(result.current.windspeed, 8.0);
        assert_eq!(result.current.humidity, 55);
        assert_eq!(result.current.feels_like, 71.0);
        assert_eq!(result.current.wind_direction, 90);
        assert_eq!(result.current.compass_direction, CompassDirection::E);
        assert_eq!(result.current.wind_gusts, 12.0);
        assert_eq!(result.current.uv_index, 4.5);
        assert_eq!(result.current.visibility, 10000.0);
        assert_eq!(result.current.pressure, 1013.25);
        assert_eq!(result.current.cloud_cover, 10);
        assert_eq!(result.current.dew_point, 55.0);
        assert_eq!(result.utc_offset_seconds, -25200);
    }

    #[test]
    fn open_meteo_current_temperature_uses_resolved_value() {
        let data: OpenMeteoResponse = serde_json::from_str(CURRENT_FIELDS_FIXTURE).unwrap();
        // Resolved temperature (60.0) differs from data.current.temperature_2m
        // (72.5) in the fixture, proving weather_from_open_meteo takes the
        // temperature from its parameter, not from the raw response — the
        // seam that lets resolve_current_temp inject the JMA override.
        let result = weather_from_open_meteo(data, 60.0);
        assert_eq!(result.current.temperature, 60.0);
    }

    #[test]
    fn open_meteo_hourly_forecast_decodes_24_rows() {
        let times: Vec<String> = (0..24).map(|h| format!("2026-06-01T{h:02}:00")).collect();
        let temps: Vec<f32> = (0..24).map(|h| 50.0 + h as f32).collect();
        let json = serde_json::json!({
            "utc_offset_seconds": 0,
            "current": minimal_current_json(),
            "hourly": {
                "time": times,
                "temperature_2m": temps,
                "weathercode": vec![0; 24],
                "precipitation_probability": vec![10; 24],
                "precipitation": vec![0.0; 24],
                "windspeed_10m": vec![5.0; 24],
                "wind_gusts_10m": vec![8.0; 24]
            },
            "daily": minimal_daily_json()
        })
        .to_string();

        let data: OpenMeteoResponse = serde_json::from_str(&json).unwrap();
        let result = weather_from_open_meteo(data, 60.0);

        assert_eq!(result.hourly.len(), 24);
        assert_eq!(result.hourly[0].time, "2026-06-01T00:00");
        assert_eq!(result.hourly[0].temperature, 50.0);
    }

    #[test]
    fn open_meteo_hourly_drops_rows_when_parallel_array_shorter() {
        // time has 3 entries but temperature_2m only has 2, so index 2 is
        // unreachable via `.get(i)?` and the row is dropped.
        let json = serde_json::json!({
            "utc_offset_seconds": 0,
            "current": minimal_current_json(),
            "hourly": {
                "time": ["2026-06-01T00:00", "2026-06-01T01:00", "2026-06-01T02:00"],
                "temperature_2m": [50.0, 51.0],
                "weathercode": [0, 0, 0],
                "precipitation_probability": [10, 10, 10],
                "precipitation": [0.0, 0.0, 0.0],
                "windspeed_10m": [5.0, 5.0, 5.0],
                "wind_gusts_10m": [8.0, 8.0, 8.0]
            },
            "daily": minimal_daily_json()
        })
        .to_string();

        let data: OpenMeteoResponse = serde_json::from_str(&json).unwrap();
        let result = weather_from_open_meteo(data, 60.0);

        assert_eq!(result.hourly.len(), 2);
    }

    #[test]
    fn open_meteo_daily_forecast_decodes_multiple_days() {
        let json = serde_json::json!({
            "utc_offset_seconds": 0,
            "current": minimal_current_json(),
            "hourly": minimal_hourly_json(),
            "daily": {
                "time": ["2026-06-01", "2026-06-02", "2026-06-03"],
                "temperature_2m_max": [80.0, 78.0, 82.0],
                "temperature_2m_min": [60.0, 58.0, 61.0],
                "weathercode": [0, 61, 71],
                "sunrise": ["2026-06-01T05:30", "2026-06-02T05:31", "2026-06-03T05:32"],
                "sunset": ["2026-06-01T20:45", "2026-06-02T20:46", "2026-06-03T20:47"]
            }
        })
        .to_string();

        let data: OpenMeteoResponse = serde_json::from_str(&json).unwrap();
        let result = weather_from_open_meteo(data, 60.0);

        assert_eq!(result.forecast.len(), 3);
        assert_eq!(result.forecast[0].date, "2026-06-01");
        assert_eq!(result.forecast[0].sunrise, "2026-06-01T05:30");
        assert_eq!(result.forecast[0].sunset, "2026-06-01T20:45");
        assert_eq!(result.forecast[0].condition, WeatherCondition::ClearSky);
    }

    #[test]
    fn open_meteo_default_utc_offset_when_missing() {
        // utc_offset_seconds is omitted entirely, proving the #[serde(default)]
        // fallback to 0.
        let json = serde_json::json!({
            "current": minimal_current_json(),
            "hourly": minimal_hourly_json(),
            "daily": minimal_daily_json()
        })
        .to_string();

        let data: OpenMeteoResponse = serde_json::from_str(&json).unwrap();
        let result = weather_from_open_meteo(data, 60.0);

        assert_eq!(result.utc_offset_seconds, 0);
    }

    #[test]
    fn apply_nws_override_none_leaves_weather_untouched() {
        let data: OpenMeteoResponse = serde_json::from_str(CURRENT_FIELDS_FIXTURE).unwrap();
        let weather = weather_from_open_meteo(data, 60.0);
        let before = weather.current.temperature;
        let out = apply_nws_override(weather, None);
        assert_eq!(out.current.temperature, before); // None -> nothing mutated
    }

    #[test]
    fn resolve_current_temp_japan_uses_override_when_some() {
        // Tokyo coords (inside Japan bounds) with a Some override present.
        let result = resolve_current_temp(35.68, 139.65, 60.0, Some(72.5));
        assert_eq!(result, 72.5);
    }

    #[test]
    fn resolve_current_temp_japan_uses_raw_when_none() {
        // Tokyo coords with no override (JMA fetch failed or was skipped):
        // silently falls through to the raw Open-Meteo value.
        let result = resolve_current_temp(35.68, 139.65, 60.0, None);
        assert_eq!(result, 60.0);
    }

    #[test]
    fn resolve_current_temp_non_japan_always_uses_raw() {
        // Portland, OR (outside Japan bounds) with a Some override present
        // anyway: the is_japan_bounds gate must block it regardless.
        let result = resolve_current_temp(45.5152, -122.6784, 60.0, Some(999.0));
        assert_eq!(result, 60.0);
    }
}
