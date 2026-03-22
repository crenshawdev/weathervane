// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Deserialize;

use crate::client::http_client;
use crate::error::Result;

/// Geographic region for alert provider and AQI standard selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// Continental US, Alaska, Hawaii. Uses NWS alerts and US EPA AQI.
    Us,
    /// European countries covered by MeteoAlarm. Uses European AQI.
    Europe,
    /// Canada. Uses ECCC (Environment and Climate Change Canada) alerts.
    Canada,
    /// Australia. Uses BOM (Bureau of Meteorology) alerts.
    Australia,
    /// Anywhere else. No alert provider available.
    Unknown,
}

/// Detects geographic region from coordinates for alert provider selection.
pub fn detect_region(lat: f64, lon: f64) -> Region {
    if is_us_bounds(lat, lon) {
        return Region::Us;
    }
    if is_canada_bounds(lat, lon) {
        return Region::Canada;
    }
    if is_europe_bounds(lat, lon) {
        return Region::Europe;
    }
    if is_australia_bounds(lat, lon) {
        return Region::Australia;
    }
    Region::Unknown
}

/// Checks if coordinates fall within US territory (continental US, Alaska, Hawaii).
/// Excludes Canadian territory by respecting the US-Canada border.
fn is_us_bounds(lat: f64, lon: f64) -> bool {
    let alaska = (51.0..=72.0).contains(&lat) && (-180.0..=-129.0).contains(&lon);
    let hawaii = (18.0..=23.0).contains(&lat) && (-161.0..=-154.0).contains(&lon);

    // Continental US with proper northern border respecting Canada.
    // The US-Canada border varies by region.
    let continental = if lon < -95.0 {
        // Western US: border at 49N
        (24.0..=49.0).contains(&lat) && (-125.0..=-95.0).contains(&lon)
    } else if lon < -84.0 {
        // Upper Midwest (MN, WI, MI upper): border near 49N for MN,
        // drops to ~46N for Lake Superior region
        (24.0..=46.5).contains(&lat) && (-95.0..=-84.0).contains(&lon)
    } else if lon < -76.0 {
        // Great Lakes / Southern Ontario overlap zone (MI, OH, NY, PA).
        // Use 43N to exclude Toronto and everything north of the lakes.
        (24.0..=43.0).contains(&lat) && (-84.0..=-76.0).contains(&lon)
    } else if lon < -67.0 {
        // Northeast US (NY, VT, NH, MA, CT, RI): St. Lawrence border ~45N
        (24.0..=45.0).contains(&lat) && (-76.0..=-67.0).contains(&lon)
    } else {
        // Maine: border goes up to ~47N
        (24.0..=47.0).contains(&lat) && (-67.0..=-66.0).contains(&lon)
    };

    continental || alaska || hawaii
}

/// Checks if coordinates fall within Canada.
fn is_canada_bounds(lat: f64, lon: f64) -> bool {
    (41.0..=84.0).contains(&lat) && (-141.0..=-52.0).contains(&lon)
}

/// Checks if coordinates fall within Europe.
fn is_europe_bounds(lat: f64, lon: f64) -> bool {
    (35.0..=71.0).contains(&lat) && (-25.0..=40.0).contains(&lon)
}

/// Checks if coordinates fall within Australia.
fn is_australia_bounds(lat: f64, lon: f64) -> bool {
    (-44.0..=-10.0).contains(&lat) && (112.0..=154.0).contains(&lon)
}

/// Checks if a point is inside a polygon using the ray casting algorithm.
/// Polygon format: "lat1,lon1 lat2,lon2 lat3,lon3 ..."
pub(crate) fn point_in_polygon(lat: f64, lon: f64, polygon_str: &str) -> bool {
    let vertices: Vec<(f64, f64)> = polygon_str
        .split_whitespace()
        .filter_map(|coord| {
            let parts: Vec<&str> = coord.split(',').collect();
            if parts.len() == 2 {
                if let (Ok(lat), Ok(lon)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                    return Some((lat, lon));
                }
            }
            None
        })
        .collect();

    if vertices.len() < 3 {
        return false;
    }

    let mut inside = false;
    let n = vertices.len();
    let mut j = n - 1;

    for i in 0..n {
        let (yi, xi) = vertices[i];
        let (yj, xj) = vertices[j];

        if ((yi > lat) != (yj > lat)) && (lon < (xj - xi) * (lat - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }

    inside
}

/// Encodes latitude/longitude into a geohash string.
/// Uses base32 encoding with the given precision (6 is city-level).
pub(crate) fn encode_geohash(lat: f64, lon: f64, precision: usize) -> String {
    const BASE32: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

    let mut lat_range = (-90.0, 90.0);
    let mut lon_range = (-180.0, 180.0);
    let mut hash = String::with_capacity(precision);
    let mut bits = 0u8;
    let mut bit_count = 0;
    let mut is_lon = true;

    while hash.len() < precision {
        if is_lon {
            let mid = (lon_range.0 + lon_range.1) / 2.0;
            if lon >= mid {
                bits = (bits << 1) | 1;
                lon_range.0 = mid;
            } else {
                bits <<= 1;
                lon_range.1 = mid;
            }
        } else {
            let mid = (lat_range.0 + lat_range.1) / 2.0;
            if lat >= mid {
                bits = (bits << 1) | 1;
                lat_range.0 = mid;
            } else {
                bits <<= 1;
                lat_range.1 = mid;
            }
        }
        is_lon = !is_lon;
        bit_count += 1;

        if bit_count == 5 {
            hash.push(BASE32[bits as usize] as char);
            bits = 0;
            bit_count = 0;
        }
    }
    hash
}

/// Maps Canadian province/territory to ECCC weather office codes.
/// Returns the primary office and optionally secondary offices for border regions.
pub(crate) fn get_eccc_office_codes(lat: f64, lon: f64) -> Vec<&'static str> {
    let mut offices = Vec::new();

    // British Columbia: roughly west of -120
    if lon < -114.0 && lat < 60.0 {
        offices.push("CWVR");
    }
    // Yukon: northwest corner
    if lon < -124.0 && lat > 60.0 {
        offices.push("CWVR");
    }
    // Alberta: -120 to -110, south of 60
    if (-120.0..=-110.0).contains(&lon) && lat < 60.0 {
        offices.push("CWNT");
    }
    // Northwest Territories and Nunavut: north of 60
    if lat > 60.0 && lon > -124.0 {
        offices.push("CWNT");
    }
    // Saskatchewan and Manitoba: -110 to -89
    if (-110.0..=-89.0).contains(&lon) && lat < 60.0 {
        offices.push("CWWG");
    }
    // Ontario: -95 to -74
    if (-95.0..=-74.0).contains(&lon) && lat < 56.0 {
        offices.push("CWTO");
    }
    // Quebec: east of -79
    if lon > -79.0 && lat < 55.0 && lon < -57.0 {
        offices.push("CWUL");
    }
    // Atlantic provinces: east of -67 or specific lat/lon ranges
    if lon > -67.0 || (lon > -64.0 && lat < 48.0) {
        offices.push("CWHX");
    }

    // Fallback: if no office matched, return Ontario
    if offices.is_empty() {
        offices.push("CWTO");
    }

    offices
}

/// Maps country name to (MeteoAlarm feed slug, ISO country code).
/// Returns None if country is not covered by MeteoAlarm.
pub(crate) fn get_meteoalarm_info(country: &str) -> Option<(&'static str, &'static str)> {
    match country.to_lowercase().as_str() {
        "austria" => Some(("austria", "AT")),
        "belgium" => Some(("belgium", "BE")),
        "bosnia and herzegovina" => Some(("bosnia-herzegovina", "BA")),
        "bulgaria" => Some(("bulgaria", "BG")),
        "croatia" => Some(("croatia", "HR")),
        "cyprus" => Some(("cyprus", "CY")),
        "czechia" | "czech republic" => Some(("czechia", "CZ")),
        "denmark" => Some(("denmark", "DK")),
        "estonia" => Some(("estonia", "EE")),
        "finland" => Some(("finland", "FI")),
        "france" => Some(("france", "FR")),
        "germany" => Some(("germany", "DE")),
        "greece" => Some(("greece", "GR")),
        "hungary" => Some(("hungary", "HU")),
        "iceland" => Some(("iceland", "IS")),
        "ireland" => Some(("ireland", "IE")),
        "israel" => Some(("israel", "IL")),
        "italy" => Some(("italy", "IT")),
        "latvia" => Some(("latvia", "LV")),
        "lithuania" => Some(("lithuania", "LT")),
        "luxembourg" => Some(("luxembourg", "LU")),
        "malta" => Some(("malta", "MT")),
        "moldova" => Some(("moldova", "MD")),
        "montenegro" => Some(("montenegro", "ME")),
        "netherlands" => Some(("netherlands", "NL")),
        "north macedonia" | "macedonia" => Some(("north-macedonia", "MK")),
        "norway" => Some(("norway", "NO")),
        "poland" => Some(("poland", "PL")),
        "portugal" => Some(("portugal", "PT")),
        "romania" => Some(("romania", "RO")),
        "serbia" => Some(("serbia", "RS")),
        "slovakia" => Some(("slovakia", "SK")),
        "slovenia" => Some(("slovenia", "SI")),
        "spain" => Some(("spain", "ES")),
        "sweden" => Some(("sweden", "SE")),
        "switzerland" => Some(("switzerland", "CH")),
        "united kingdom" | "uk" => Some(("united-kingdom", "UK")),
        _ => None,
    }
}

/// Nominatim reverse geocoding response.
#[derive(Debug, Deserialize)]
pub(crate) struct NominatimResponse {
    pub address: Option<NominatimAddress>,
}

/// Address details from Nominatim.
#[derive(Debug, Deserialize)]
pub(crate) struct NominatimAddress {
    pub city: Option<String>,
    pub town: Option<String>,
    pub county: Option<String>,
    pub state: Option<String>,
}

/// MeteoAlarm codenames mapping (EMMA_ID -> region name).
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub(crate) struct MeteoAlarmCodenames {
    pub codes: std::collections::HashMap<String, String>,
}

/// Detects country from coordinates using reverse geocoding.
pub(crate) async fn detect_country_from_coords(latitude: f64, longitude: f64) -> Result<String> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name=&latitude={}&longitude={}&count=1",
        latitude, longitude
    );

    let response = http_client()?.get(&url).send().await;
    if let Ok(resp) = response {
        if let Ok(data) = resp.json::<GeocodingResponseMinimal>().await {
            if let Some(results) = data.results {
                if let Some(first) = results.first() {
                    if let Some(country) = &first.country {
                        return Ok(country.clone());
                    }
                }
            }
        }
    }

    // Fallback: approximate from European bounding boxes
    let country = approximate_european_country(latitude, longitude);
    tracing::debug!(
        "Reverse geocoding failed, approximated country as '{}' from bounding boxes",
        country
    );
    Ok(country.to_string())
}

/// Minimal geocoding response for country detection only.
#[derive(Debug, Deserialize)]
struct GeocodingResponseMinimal {
    results: Option<Vec<GeocodingResultMinimal>>,
}

#[derive(Debug, Deserialize)]
struct GeocodingResultMinimal {
    country: Option<String>,
}

/// Approximates country from coordinates using bounding boxes.
/// Used as fallback when reverse geocoding fails.
fn approximate_european_country(lat: f64, lon: f64) -> &'static str {
    if (47.3..=55.1).contains(&lat) && (5.9..=15.0).contains(&lon) {
        "Germany"
    } else if (41.3..=51.1).contains(&lat) && (-5.1..=9.6).contains(&lon) {
        "France"
    } else if (36.0..=43.8).contains(&lat) && (-9.5..=3.3).contains(&lon) {
        "Spain"
    } else if (36.6..=47.1).contains(&lat) && (6.6..=18.5).contains(&lon) {
        "Italy"
    } else if (49.9..=61.0).contains(&lat) && (-8.6..=1.8).contains(&lon) {
        "United Kingdom"
    } else if (50.8..=53.5).contains(&lat) && (3.4..=7.2).contains(&lon) {
        "Netherlands"
    } else if (49.5..=51.5).contains(&lat) && (2.5..=6.4).contains(&lon) {
        "Belgium"
    } else if (46.4..=49.0).contains(&lat) && (5.9..=10.5).contains(&lon) {
        "Switzerland"
    } else if (46.4..=49.0).contains(&lat) && (9.5..=17.2).contains(&lon) {
        "Austria"
    } else if (49.0..=54.9).contains(&lat) && (14.1..=24.2).contains(&lon) {
        "Poland"
    } else if (55.0..=69.1).contains(&lat) && (4.5..=31.1).contains(&lon) {
        if lon < 10.0 {
            "Norway"
        } else if lon < 24.2 {
            "Sweden"
        } else {
            "Finland"
        }
    } else {
        "Unknown"
    }
}
