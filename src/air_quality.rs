// SPDX-License-Identifier: MIT OR Apache-2.0

//! Air quality data and AQI categories for US and European standards.

use serde::{Deserialize, Serialize};

use crate::air_quality_aqicn::fetch_headline_aqi;
use crate::client::http_client;
use crate::error::Result;
use crate::geo::{detect_region, Region};

/// AQI standard based on geographic region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AqiStandard {
    /// US EPA AQI. Scale 0-500, used everywhere outside Europe.
    Us,
    /// European AQI. Scale 0-100+, used within Europe.
    European,
}

/// Which provider supplied the headline AQI.
///
/// Recorded at fetch time by [`fetch_air_quality`]; consumers use it for
/// source attribution without re-deriving the selection logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AqiSource {
    /// World Air Quality Index Project (aqicn.org), US EPA scale.
    Aqicn,
    /// Open-Meteo (the default; used for Europe, no token, or aqicn fallback).
    #[default]
    OpenMeteo,
}

/// US EPA AQI category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UsAqiCategory {
    /// AQI 0-50.
    Good,
    /// AQI 51-100.
    Moderate,
    /// AQI 101-150. Active children and adults with respiratory issues should limit outdoor exertion.
    UnhealthySensitive,
    /// AQI 151-200.
    Unhealthy,
    /// AQI 201-300.
    VeryUnhealthy,
    /// AQI 301+.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EuAqiCategory {
    /// AQI 0-20.
    Good,
    /// AQI 21-40.
    Fair,
    /// AQI 41-60.
    Moderate,
    /// AQI 61-80.
    Poor,
    /// AQI 81-100.
    VeryPoor,
    /// AQI 101+.
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
///
/// Wire form is adjacent-tagged (the one documented exception to the
/// serde-defaults baseline): `{"standard": "Us", "level": "Good"}`.
/// The tag disambiguates levels like `Good`/`Moderate` that exist in both
/// scales, so deserialization is exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "standard", content = "level")]
pub enum AqiCategory {
    /// US EPA category. Returned for all non-European locations.
    Us(UsAqiCategory),
    /// European category. Returned when coordinates fall within Europe.
    Eu(EuAqiCategory),
}

/// Current air quality data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirQualityData {
    /// AQI value. Scale depends on the standard (US 0-500, EU 0-100+).
    pub aqi: i32,
    /// Categorized severity for display. Also carries which standard applies;
    /// see [`AirQualityData::standard()`].
    pub category: AqiCategory,
    /// Fine particulate matter (micrograms per cubic meter).
    pub pm2_5: f32,
    /// Coarse particulate matter (micrograms per cubic meter).
    pub pm10: f32,
    /// Ground-level ozone (micrograms per cubic meter).
    pub ozone: f32,
    /// NO2 concentration (micrograms per cubic meter).
    pub nitrogen_dioxide: f32,
    /// CO concentration (micrograms per cubic meter).
    pub carbon_monoxide: f32,
    /// Which provider supplied the headline AQI. Defaults to OpenMeteo for
    /// wire back-compat with payloads serialized before this field existed.
    #[serde(default)]
    pub aqi_source: AqiSource,
}

impl AirQualityData {
    /// Which AQI standard applies, derived from the category variant.
    pub fn standard(&self) -> AqiStandard {
        match self.category {
            AqiCategory::Us(_) => AqiStandard::Us,
            AqiCategory::Eu(_) => AqiStandard::European,
        }
    }
}

/// Fetches air quality data.
///
/// Pollutant concentrations (pm2_5, pm10, etc) always come from Open-Meteo so
/// the µg/m³ contract on [`AirQualityData`] stays honest. The headline `aqi`
/// field uses the World Air Quality Index Project (aqicn.org) when a token is
/// provided and the coordinates are outside Europe. Europe stays on Open-Meteo
/// so the [`AqiStandard::European`] category mapping is preserved.
///
/// Free aqicn tokens are issued at <https://aqicn.org/data-platform/token/>.
/// Pass `None` to skip aqicn entirely.
///
/// If aqicn is selected but unreachable, returns invalid data, or the token
/// is rejected, the call falls back to Open-Meteo's AQI without surfacing an
/// error.
pub async fn fetch_air_quality(
    latitude: f64,
    longitude: f64,
    aqicn_token: Option<&str>,
) -> Result<AirQualityData> {
    let url = format!(
        "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=us_aqi,european_aqi,pm2_5,pm10,ozone,nitrogen_dioxide,carbon_monoxide&timezone=auto",
        latitude, longitude
    );

    let response = http_client()?.get(&url).send().await?.error_for_status()?;
    let data: AirQualityResponse = response.json().await?;

    let region = detect_region(latitude, longitude);

    // Try aqicn for the headline AQI when a token is present and we're not
    // in Europe. Europe keeps Open-Meteo so the EU scale and category mapping
    // are preserved.
    let aqicn_aqi = match (aqicn_token, region) {
        (Some(token), r) if r != Region::Europe => {
            fetch_headline_aqi(latitude, longitude, token).await
        }
        _ => None,
    };

    let (aqi, category, aqi_source) = if let Some(val) = aqicn_aqi {
        (
            val,
            AqiCategory::Us(UsAqiCategory::from_aqi(val)),
            AqiSource::Aqicn,
        )
    } else if region == Region::Europe {
        let val = data.current.european_aqi.unwrap_or_else(|| {
            tracing::warn!("European AQI missing from API response, defaulting to 0");
            0
        });
        (
            val,
            AqiCategory::Eu(EuAqiCategory::from_aqi(val)),
            AqiSource::OpenMeteo,
        )
    } else {
        let val = data.current.us_aqi.unwrap_or_else(|| {
            tracing::warn!("US AQI missing from API response, defaulting to 0");
            0
        });
        (
            val,
            AqiCategory::Us(UsAqiCategory::from_aqi(val)),
            AqiSource::OpenMeteo,
        )
    };

    Ok(AirQualityData {
        aqi,
        category,
        pm2_5: data.current.pm2_5.unwrap_or(0.0),
        pm10: data.current.pm10.unwrap_or(0.0),
        ozone: data.current.ozone.unwrap_or(0.0),
        nitrogen_dioxide: data.current.nitrogen_dioxide.unwrap_or(0.0),
        carbon_monoxide: data.current.carbon_monoxide.unwrap_or(0.0),
        aqi_source,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aqi_source_defaults_to_open_meteo() {
        assert_eq!(AqiSource::default(), AqiSource::OpenMeteo);
    }
}
