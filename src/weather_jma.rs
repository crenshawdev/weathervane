// SPDX-License-Identifier: MIT OR Apache-2.0

//! AMeDAS temperature override for Japanese coordinates.
//!
//! JMA publishes AMeDAS station observations every ten minutes. Open-Meteo's
//! `best_match` model runs a few degrees cold for Japan versus ground truth,
//! so when the caller's coordinate falls inside Japan we swap the current
//! temperature for the nearest station's reading.
//!
//! All failures are swallowed and logged at debug. JMA is a quality upgrade,
//! not a dependency: any failure falls through to whatever Open-Meteo returned.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::Deserialize;

use crate::client::{get_json, get_text};
use crate::units::TemperatureUnit;

const LATEST_TIME_URL: &str = "https://www.jma.go.jp/bosai/amedas/data/latest_time.txt";
const STATION_TABLE_URL: &str = "https://www.jma.go.jp/bosai/amedas/const/amedastable.json";
const MAP_URL_PREFIX: &str = "https://www.jma.go.jp/bosai/amedas/data/map/";

// Skip the override if the nearest AMeDAS station is further than this.
// AMeDAS coverage is dense inside Japan, so a large gap means we caught
// an outlier that slipped through the bounding box.
const MAX_STATION_DISTANCE_KM: f64 = 50.0;

// How many nearest stations to try before giving up, in case the first
// has an invalid temperature flag.
const MAX_HOPS: usize = 3;

/// A temperature-reporting AMeDAS station.
#[derive(Debug, Clone)]
struct Station {
    code: String,
    lat: f64,
    lon: f64,
}

static STATIONS: RwLock<Option<Vec<Station>>> = RwLock::new(None);

/// Returns the nearest AMeDAS station's current temperature in the caller's
/// requested unit. Returns `None` on any failure.
pub(crate) async fn override_current_temp(
    latitude: f64,
    longitude: f64,
    unit: TemperatureUnit,
) -> Option<f32> {
    let stations = cached_stations().await?;

    let mut candidates: Vec<(f64, Station)> = stations
        .iter()
        .map(|s| (haversine_km(latitude, longitude, s.lat, s.lon), s.clone()))
        .collect();
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    match candidates.first() {
        Some((d, _)) if *d <= MAX_STATION_DISTANCE_KM => {}
        _ => {
            tracing::debug!(
                "nearest AMeDAS station further than {MAX_STATION_DISTANCE_KM}km, skipping override"
            );
            return None;
        }
    }

    let timestamp = latest_observation_time().await?;
    let map = fetch_map(&timestamp).await?;

    for (_, station) in candidates.iter().take(MAX_HOPS) {
        if let Some(obs) = map.get(&station.code) {
            if let Some(temp) = obs.temp.as_ref() {
                if temp.len() == 2 && temp[1] == 0.0 {
                    return Some(to_unit(temp[0] as f32, unit));
                }
            }
        }
    }

    tracing::debug!("no AMeDAS station within {MAX_HOPS} hops returned a valid temperature");
    None
}

/// Returns the cached station list, fetching and caching on first call.
/// Cache misses retry on subsequent calls so a transient failure doesn't
/// poison the override for the process lifetime.
async fn cached_stations() -> Option<Vec<Station>> {
    if let Ok(guard) = STATIONS.read() {
        if let Some(stations) = guard.as_ref() {
            return Some(stations.clone());
        }
    }

    let fetched = fetch_stations().await?;
    if let Ok(mut guard) = STATIONS.write() {
        *guard = Some(fetched.clone());
    }
    Some(fetched)
}

async fn fetch_stations() -> Option<Vec<Station>> {
    let raw: HashMap<String, RawStation> =
        get_json(STATION_TABLE_URL, "AMeDAS station table").await?;

    let mut stations = Vec::with_capacity(raw.len());
    for (code, s) in raw {
        // elems is an 8-char flag string. First char == '1' means this
        // station reports temperature. See JMA AMeDAS docs.
        if s.elems.as_bytes().first() != Some(&b'1') {
            continue;
        }
        if s.lat.len() != 2 || s.lon.len() != 2 {
            continue;
        }
        stations.push(Station {
            code,
            lat: deg_min_to_decimal(s.lat[0], s.lat[1]),
            lon: deg_min_to_decimal(s.lon[0], s.lon[1]),
        });
    }

    tracing::debug!(
        "AMeDAS station table loaded, {} temp-capable stations",
        stations.len()
    );
    Some(stations)
}

async fn latest_observation_time() -> Option<String> {
    let text = get_text(LATEST_TIME_URL, "AMeDAS latest_time").await?;
    parse_iso_to_compact(text.trim())
}

async fn fetch_map(timestamp: &str) -> Option<HashMap<String, RawObservation>> {
    let url = format!("{MAP_URL_PREFIX}{timestamp}.json");
    get_json(&url, "AMeDAS map").await
}

/// Reformats `2026-04-21T02:30:00+09:00` to `20260421023000`.
fn parse_iso_to_compact(iso: &str) -> Option<String> {
    if iso.len() < 19 {
        return None;
    }
    let b = iso.as_bytes();
    // Bail if the separators aren't where we expect.
    if b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let mut out = String::with_capacity(14);
    for i in [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18] {
        let c = b[i];
        if !c.is_ascii_digit() {
            return None;
        }
        out.push(c as char);
    }
    Some(out)
}

/// Converts AMeDAS `[degrees, decimal_minutes]` coordinate to decimal degrees.
fn deg_min_to_decimal(deg: f64, min: f64) -> f64 {
    deg + min / 60.0
}

/// Haversine distance between two coordinates in kilometers.
fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();

    let a =
        (d_lat / 2.0).sin().powi(2) + lat1_rad.cos() * lat2_rad.cos() * (d_lon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().asin()
}

fn to_unit(celsius: f32, unit: TemperatureUnit) -> f32 {
    match unit {
        TemperatureUnit::Celsius => celsius,
        TemperatureUnit::Fahrenheit => celsius * 9.0 / 5.0 + 32.0,
    }
}

#[derive(Debug, Deserialize)]
struct RawStation {
    lat: Vec<f64>,
    lon: Vec<f64>,
    elems: String,
}

#[derive(Debug, Deserialize)]
struct RawObservation {
    #[serde(default)]
    temp: Option<Vec<f64>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deg_min_converts() {
        // Tokyo station is roughly 35°41'N, 139°45'E.
        assert!((deg_min_to_decimal(35.0, 41.0) - 35.683).abs() < 0.01);
        assert!((deg_min_to_decimal(139.0, 45.0) - 139.75).abs() < 0.01);
    }

    #[test]
    fn haversine_tokyo_to_osaka() {
        // Great circle distance is about 400km.
        let d = haversine_km(35.68, 139.65, 34.69, 135.50);
        assert!((d - 400.0).abs() < 25.0, "expected ~400km, got {d}");
    }

    #[test]
    fn haversine_same_point_is_zero() {
        assert!(haversine_km(35.0, 139.0, 35.0, 139.0) < 0.001);
    }

    #[test]
    fn celsius_passthrough() {
        assert_eq!(to_unit(18.5, TemperatureUnit::Celsius), 18.5);
    }

    #[test]
    fn celsius_to_fahrenheit() {
        assert!((to_unit(0.0, TemperatureUnit::Fahrenheit) - 32.0).abs() < 0.001);
        assert!((to_unit(100.0, TemperatureUnit::Fahrenheit) - 212.0).abs() < 0.001);
        assert!((to_unit(18.0, TemperatureUnit::Fahrenheit) - 64.4).abs() < 0.01);
    }

    #[test]
    fn iso_parser_strips_separators() {
        assert_eq!(
            parse_iso_to_compact("2026-04-21T02:30:00+09:00"),
            Some("20260421023000".to_string())
        );
    }

    #[test]
    fn iso_parser_rejects_malformed() {
        assert!(parse_iso_to_compact("not-an-iso").is_none());
        assert!(parse_iso_to_compact("2026/04/21 02:30:00").is_none());
        assert!(parse_iso_to_compact("").is_none());
    }

    #[test]
    fn nearest_selection_picks_closest() {
        // Sanity check the sort used in override_current_temp: given a
        // caller coord and several stations, the closest should come first.
        let caller = (35.68_f64, 139.65_f64);
        let stations = [
            Station {
                code: "osaka".into(),
                lat: 34.69,
                lon: 135.50,
            },
            Station {
                code: "tokyo".into(),
                lat: 35.69,
                lon: 139.70,
            },
            Station {
                code: "sapporo".into(),
                lat: 43.07,
                lon: 141.35,
            },
        ];
        let mut ranked: Vec<(f64, &Station)> = stations
            .iter()
            .map(|s| (haversine_km(caller.0, caller.1, s.lat, s.lon), s))
            .collect();
        ranked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(ranked[0].1.code, "tokyo");
    }

    #[test]
    fn nan_station_coords_do_not_panic() {
        // A NaN-lat station must not panic the comparator and must not
        // displace a well-formed, nearer station from rank 0.
        let caller = (35.68_f64, 139.65_f64);
        let stations = [
            Station {
                code: "tokyo".into(),
                lat: 35.69,
                lon: 139.70,
            },
            Station {
                code: "sapporo".into(),
                lat: 43.07,
                lon: 141.35,
            },
            Station {
                code: "nan_station".into(),
                lat: f64::NAN,
                lon: 139.0,
            },
        ];
        let mut ranked: Vec<(f64, &Station)> = stations
            .iter()
            .map(|s| (haversine_km(caller.0, caller.1, s.lat, s.lon), s))
            .collect();
        ranked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(ranked[0].1.code, "tokyo");
    }
}
