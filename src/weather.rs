// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

use crate::client::http_client;
use crate::codes::{CompassDirection, WeatherCondition};
use crate::error::Result;
use crate::units::{MeasurementSystem, TemperatureUnit};

/// Current weather conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentWeather {
    pub temperature: f32,
    pub weathercode: i32,
    pub condition: WeatherCondition,
    pub windspeed: f32,
    pub humidity: i32,
    pub feels_like: f32,
    pub wind_direction: i32,
    pub compass_direction: CompassDirection,
    pub wind_gusts: f32,
    pub uv_index: f32,
    pub visibility: f32,
    pub pressure: f32,
    pub cloud_cover: i32,
    pub dew_point: f32,
}

/// Daily forecast data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyForecast {
    pub date: String,
    pub temp_max: f32,
    pub temp_min: f32,
    pub weathercode: i32,
    pub condition: WeatherCondition,
    pub sunrise: String,
    pub sunset: String,
}

/// Hourly forecast data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyForecast {
    pub time: String,
    pub temperature: f32,
    pub weathercode: i32,
    pub condition: WeatherCondition,
    pub precipitation_probability: i32,
}

/// Complete weather data from a single fetch.
#[derive(Debug, Clone)]
pub struct WeatherData {
    pub current: CurrentWeather,
    pub hourly: Vec<HourlyForecast>,
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
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weathercode,windspeed_10m,relative_humidity_2m,apparent_temperature,wind_direction_10m,wind_gusts_10m,uv_index,visibility,surface_pressure,cloud_cover,dewpoint_2m&hourly=temperature_2m,weathercode,precipitation_probability&daily=temperature_2m_max,temperature_2m_min,weathercode,sunrise,sunset&temperature_unit={}&windspeed_unit={}&timezone=auto&forecast_days=7&forecast_hours=24",
        latitude,
        longitude,
        temperature_unit.api_param(),
        measurement_system.wind_speed_api_param(),
    );

    let response = http_client()?.get(&url).send().await?;
    let data: OpenMeteoResponse = response.json().await?;

    let hourly: Vec<_> = (0..data.hourly.time.len().min(12))
        .map(|i| HourlyForecast {
            time: data.hourly.time[i].clone(),
            temperature: data.hourly.temperature_2m[i],
            weathercode: data.hourly.weathercode[i],
            condition: WeatherCondition::from_code(data.hourly.weathercode[i]),
            precipitation_probability: data.hourly.precipitation_probability[i],
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
            temperature: data.current.temperature_2m,
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
