// SPDX-License-Identifier: MIT OR Apache-2.0

//! Weather data types and fetching from the Open-Meteo API.

use serde::{Deserialize, Serialize};

use crate::client::http_client;
use crate::codes::{CompassDirection, WeatherCondition};
use crate::error::Result;
use crate::geo::is_japan_bounds;
use crate::units::{MeasurementSystem, TemperatureUnit};
use crate::weather_jma::override_current_temp;

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
#[derive(Debug, Clone)]
pub struct WeatherData {
    /// Current conditions at the requested location.
    pub current: CurrentWeather,
    /// Next 24 hours, one entry per hour.
    pub hourly: Vec<HourlyForecast>,
    /// 7-day forecast, one entry per day.
    pub forecast: Vec<DailyForecast>,
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
    let current_temperature = if is_japan_bounds(latitude, longitude) {
        match override_current_temp(latitude, longitude, temperature_unit).await {
            Some(t) => {
                tracing::debug!(
                    "AMeDAS override: {} -> {} ({:?})",
                    data.current.temperature_2m,
                    t,
                    temperature_unit
                );
                t
            }
            None => data.current.temperature_2m,
        }
    } else {
        data.current.temperature_2m
    };

    let hourly: Vec<_> = (0..data.hourly.time.len().min(24))
        .map(|i| HourlyForecast {
            time: data.hourly.time[i].clone(),
            temperature: data.hourly.temperature_2m[i],
            weathercode: data.hourly.weathercode[i],
            condition: WeatherCondition::from_code(data.hourly.weathercode[i]),
            precipitation_probability: data.hourly.precipitation_probability[i],
            precipitation: data.hourly.precipitation[i],
            windspeed: data.hourly.windspeed_10m[i],
            wind_gusts: data.hourly.wind_gusts_10m[i],
        })
        .collect();

    let forecast: Vec<_> = (0..data.daily.time.len())
        .map(|i| DailyForecast {
            date: data.daily.time[i].clone(),
            temp_max: data.daily.temperature_2m_max[i],
            temp_min: data.daily.temperature_2m_min[i],
            weathercode: data.daily.weathercode[i],
            condition: WeatherCondition::from_code(data.daily.weathercode[i]),
            sunrise: data.daily.sunrise[i].clone(),
            sunset: data.daily.sunset[i].clone(),
        })
        .collect();

    Ok(WeatherData {
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
    })
}

/// Open-Meteo API response structure.
#[derive(Debug, Deserialize)]
struct OpenMeteoResponse {
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
