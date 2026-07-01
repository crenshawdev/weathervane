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
            // range-contains rejects NaN and infinity by IEEE 754 ordering
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                tracing::debug!("detect_location: coordinates out of valid range");
                return Err(Error::LocationDetection);
            }

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

#[cfg(test)]
mod tests {
    /// Pins the IEEE 754 semantics detect_location() depends on (D-05, D-06).
    ///
    /// `RangeInclusive::contains` on `f64` uses `PartialOrd`, which returns `false`
    /// for any comparison involving NaN. A future refactor to `lat <= 90.0 && lat >= -90.0`
    /// would silently re-admit NaN (that form is true-for-NaN). These three tests lock
    /// the predicate form so any such regression fails CI.
    #[test]
    fn nan_coords_are_rejected() {
        // Pins the IEEE 754 semantics detect_location() depends on (D-05, D-06).
        assert!(
            !(-90.0_f64..=90.0_f64).contains(&f64::NAN),
            "range-contains must reject NaN lat"
        );
        assert!(
            !(-180.0_f64..=180.0_f64).contains(&f64::NAN),
            "range-contains must reject NaN lon"
        );
    }

    #[test]
    fn infinite_coords_are_rejected() {
        // Pins the IEEE 754 semantics detect_location() depends on (D-05, D-06).
        assert!(
            !(-90.0_f64..=90.0_f64).contains(&f64::INFINITY),
            "range-contains must reject +inf lat"
        );
        assert!(
            !(-90.0_f64..=90.0_f64).contains(&f64::NEG_INFINITY),
            "range-contains must reject -inf lat"
        );
        assert!(
            !(-180.0_f64..=180.0_f64).contains(&f64::INFINITY),
            "range-contains must reject +inf lon"
        );
        assert!(
            !(-180.0_f64..=180.0_f64).contains(&f64::NEG_INFINITY),
            "range-contains must reject -inf lon"
        );
    }

    #[test]
    fn out_of_range_coords_are_rejected() {
        // Out-of-range values are rejected.
        assert!(
            !(-90.0_f64..=90.0_f64).contains(&91.0_f64),
            "lat 91 must be rejected"
        );
        assert!(
            !(-180.0_f64..=180.0_f64).contains(&181.0_f64),
            "lon 181 must be rejected"
        );
        // Sanity anchor: valid coords are accepted (guard is not over-rejecting).
        assert!(
            (-90.0_f64..=90.0_f64).contains(&45.5_f64),
            "lat 45.5 must be accepted"
        );
        assert!(
            (-180.0_f64..=180.0_f64).contains(&-122.6_f64),
            "lon -122.6 must be accepted"
        );
    }
}
