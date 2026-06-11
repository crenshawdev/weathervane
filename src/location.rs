// SPDX-License-Identifier: MIT OR Apache-2.0

//! Location search, detection, and bookmarking.

use serde::{Deserialize, Serialize};

use crate::client::http_client;
use crate::error::{Error, Result};

/// Location search result from geocoding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationResult {
    /// Decimal latitude.
    pub latitude: f64,
    /// Decimal longitude.
    pub longitude: f64,
    /// Human-readable name (e.g. "Portland, Oregon, United States").
    pub display_name: String,
    /// Country name as returned by the geocoding API.
    pub country: String,
}

impl LocationResult {
    fn from_geocoding_result(result: &GeocodingResult) -> Self {
        let country = result.country.clone().unwrap_or_default();
        let display_name = match (&result.admin1, &result.country) {
            (Some(admin), Some(c)) => format!("{}, {}, {}", result.name, admin, c),
            (None, Some(c)) => format!("{}, {}", result.name, c),
            _ => result.name.clone(),
        };

        Self {
            latitude: result.latitude,
            longitude: result.longitude,
            display_name,
            country,
        }
    }
}

/// A bookmarked location for quick switching.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedLocation {
    /// User-facing label for this bookmark.
    pub name: String,
    /// Decimal latitude.
    pub latitude: f64,
    /// Decimal longitude.
    pub longitude: f64,
}

impl SavedLocation {
    /// Checks if this saved location matches the given coordinates.
    pub fn matches_coords(&self, lat: f64, lon: f64) -> bool {
        (self.latitude - lat).abs() < 0.01 && (self.longitude - lon).abs() < 0.01
    }
}

/// Result of automatic IP-based location detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedLocation {
    /// Decimal latitude from IP geolocation.
    pub latitude: f64,
    /// Decimal longitude from IP geolocation.
    pub longitude: f64,
    /// Best-effort city/country name from the IP lookup.
    pub display_name: String,
    /// Country name, used to determine default units.
    pub country: String,
}

/// Searches for a location by city name using Open-Meteo Geocoding API.
pub async fn search_city(city_name: &str) -> Result<Vec<LocationResult>> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=10&language=en&format=json",
        urlencoding::encode(city_name)
    );

    let response = http_client()?.get(&url).send().await?.error_for_status()?;
    let data: GeocodingResponse = response.json().await?;

    if let Some(results) = data.results {
        if !results.is_empty() {
            let locations: Vec<LocationResult> = results
                .iter()
                .map(LocationResult::from_geocoding_result)
                .collect();

            tracing::debug!("Found {} location(s)", locations.len());
            return Ok(locations);
        }
    }

    Err(Error::NoResults {
        query: city_name.to_string(),
    })
}

/// Detects user location automatically using IP-based geolocation.
pub async fn detect_location() -> Result<DetectedLocation> {
    let url = "http://ip-api.com/json/?fields=status,lat,lon,city,regionName,country";

    let response = http_client()?.get(url).send().await?.error_for_status()?;
    let data: IpApiResponse = response.json().await?;

    if data.status == "success" {
        if let (Some(lat), Some(lon)) = (data.lat, data.lon) {
            let country = data.country.clone().unwrap_or_default();
            let display_name = match (data.city, data.region_name, data.country) {
                (Some(city), _, Some(c)) => format!("{}, {}", city, c),
                (_, Some(region), Some(c)) => format!("{}, {}", region, c),
                (_, _, Some(c)) => c,
                _ => "Unknown".to_string(),
            };

            tracing::debug!("Auto-detected location");
            return Ok(DetectedLocation {
                latitude: lat,
                longitude: lon,
                display_name,
                country,
            });
        }
    }

    Err(Error::LocationDetection)
}

/// Returns true if the country uses imperial units (Fahrenheit, mph, miles).
/// Only US, Liberia, and Myanmar officially use imperial.
pub fn uses_imperial_units(country: &str) -> bool {
    matches!(country, "United States" | "Liberia" | "Myanmar")
}

/// Open-Meteo Geocoding API response.
#[derive(Debug, Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Debug, Deserialize)]
struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    admin1: Option<String>,
}

/// IP-API.com response for geolocation.
#[derive(Debug, Deserialize)]
struct IpApiResponse {
    status: String,
    lat: Option<f64>,
    lon: Option<f64>,
    city: Option<String>,
    #[serde(rename = "regionName")]
    region_name: Option<String>,
    country: Option<String>,
}
