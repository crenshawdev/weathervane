// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Deserialize;

use crate::client::http_client;
use crate::error::Result;
use crate::geo::{detect_region, Region};

/// AQI standard based on geographic region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AqiStandard {
    Us,
    European,
}

/// US EPA AQI category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UsAqiCategory {
    Good,
    Moderate,
    UnhealthySensitive,
    Unhealthy,
    VeryUnhealthy,
    Hazardous,
}

impl UsAqiCategory {
    /// Maps a US AQI value to its category.
    pub fn from_aqi(aqi: i32) -> Self {
        match aqi {
            0..=50 => Self::Good,
            51..=100 => Self::Moderate,
            101..=150 => Self::UnhealthySensitive,
            151..=200 => Self::Unhealthy,
            201..=300 => Self::VeryUnhealthy,
            _ => Self::Hazardous,
        }
    }
}

/// European AQI category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EuAqiCategory {
    Good,
    Fair,
    Moderate,
    Poor,
    VeryPoor,
    ExtremelyPoor,
}

impl EuAqiCategory {
    /// Maps a European AQI value to its category.
    pub fn from_aqi(aqi: i32) -> Self {
        match aqi {
            0..=20 => Self::Good,
            21..=40 => Self::Fair,
            41..=60 => Self::Moderate,
            61..=80 => Self::Poor,
            81..=100 => Self::VeryPoor,
            _ => Self::ExtremelyPoor,
        }
    }
}

/// AQI category, region-specific.
/// Frontend matches on this to produce translated descriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AqiCategory {
    Us(UsAqiCategory),
    Eu(EuAqiCategory),
}

/// Current air quality data.
#[derive(Debug, Clone)]
pub struct AirQualityData {
    pub aqi: i32,
    pub standard: AqiStandard,
    pub category: AqiCategory,
    pub pm2_5: f32,
    pub pm10: f32,
    pub ozone: f32,
    pub nitrogen_dioxide: f32,
    pub carbon_monoxide: f32,
}

/// Fetches air quality data from the Open-Meteo Air Quality API.
pub async fn fetch_air_quality(latitude: f64, longitude: f64) -> Result<AirQualityData> {
    let url = format!(
        "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=us_aqi,european_aqi,pm2_5,pm10,ozone,nitrogen_dioxide,carbon_monoxide&timezone=auto",
        latitude, longitude
    );

    let response = http_client()?.get(&url).send().await?;
    let data: AirQualityResponse = response.json().await?;

    let (aqi, standard, category) = match detect_region(latitude, longitude) {
        Region::Europe => {
            let val = data.current.european_aqi.unwrap_or(0);
            (
                val,
                AqiStandard::European,
                AqiCategory::Eu(EuAqiCategory::from_aqi(val)),
            )
        }
        _ => {
            let val = data.current.us_aqi.unwrap_or(0);
            (
                val,
                AqiStandard::Us,
                AqiCategory::Us(UsAqiCategory::from_aqi(val)),
            )
        }
    };

    Ok(AirQualityData {
        aqi,
        standard,
        category,
        pm2_5: data.current.pm2_5.unwrap_or(0.0),
        pm10: data.current.pm10.unwrap_or(0.0),
        ozone: data.current.ozone.unwrap_or(0.0),
        nitrogen_dioxide: data.current.nitrogen_dioxide.unwrap_or(0.0),
        carbon_monoxide: data.current.carbon_monoxide.unwrap_or(0.0),
    })
}

/// Open-Meteo Air Quality API response.
#[derive(Debug, Deserialize)]
struct AirQualityResponse {
    current: AirQualityCurrentData,
}

#[derive(Debug, Deserialize)]
struct AirQualityCurrentData {
    us_aqi: Option<i32>,
    european_aqi: Option<i32>,
    pm2_5: Option<f32>,
    pm10: Option<f32>,
    ozone: Option<f32>,
    nitrogen_dioxide: Option<f32>,
    carbon_monoxide: Option<f32>,
}
