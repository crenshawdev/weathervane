//! NWS current condition overrides for US coordinates.
//!
//! NWS current conditions provides real instrument readings (ASOS/AWOS
//! ground stations, METAR; 900+ continuous US sites).
//! Open-Meteo interpolates model output to the location but is not an observation.
//! For *right now*, NWS reports reality; Open-Meteo reports a model's best guess.
//! When the caller's coordinate falls inside the US we overlay the nearest
//! station's measured readings onto Open-Meteo's current conditions, field by
//! field - keeping Open-Meteo wherever the station is silent, and for the
//! modeled sky fields (weather code, cloud cover, UV).
//!
//! All failures are swallowed and logged at debug. NWS is a quality upgrade,
//! not a dependency: any failure falls through to whatever Open-Meteo returned.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::Deserialize;

use crate::client::get_json;
use crate::codes::CompassDirection;
use crate::units::{MeasurementSystem, TemperatureUnit};
use crate::weather::CurrentWeather;

const POINTS_URL_PREFIX: &str = "https://api.weather.gov/points/";
const STATIONS_URL_PREFIX: &str = "https://api.weather.gov/stations/";

/// How many of the grid's ordered stations to try before giving up. The list is
/// distance-ordered (nearest first); extra hops cover a nearest station that's
/// offline or reporting a null temperature. Mirrors JMA's`MAX_HOPS`.
const MAX_HOPS: usize = 3;

/// `api.weather.gov/points/{lat,lon}` response (a GeoJSON Feature).
#[derive(Debug, Deserialize)]
struct PointsResponse {
    properties: PointsProperties,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PointsProperties {
    /// URL of this grid's observation-station collection.
    observation_stations: String,
}

/// The station collection (a GeoJSON Feature collection).
#[derive(Debug, Deserialize)]
struct StationsResponse {
    features: Vec<StationFeature>,
}

#[derive(Debug, Deserialize)]
struct StationFeature {
    properties: StationProperties,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StationProperties {
    station_identifier: String,
}

static STATION_CACHE: RwLock<Option<HashMap<String, Vec<String>>>> = RwLock::new(None);

/// One NWS measured quantity: `{ "value: <number|null>, "unitCode": ..., ... }`.
/// We read only `value`; serde ignores `unitCode`/`qualityControl`.
#[derive(Debug, Default, Deserialize)]
struct Measurement {
    value: Option<f64>,
}

/// The measured fields we read. Each NWS value is raw SI and independently
/// nullable, so each is a `Measurement`; The container-level `#[serde(default)]` also tolerates
/// NWS omitting a key entirely (-> an empty `Measurement`) instead of failing the
/// whole decode and losing an otherwise-good observation.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ObservationProperties {
    temperature: Measurement,
    dewpoint: Measurement,
    relative_humidity: Measurement,
    wind_direction: Measurement,
    wind_speed: Measurement,
    wind_gust: Measurement,
    barometric_pressure: Measurement,
    sea_level_pressure: Measurement,
    visibility: Measurement,
    heat_index: Measurement,
    wind_chill: Measurement,
}

/// `.../stations/{id}/observations/latest` response (a GeoJSON feature).
#[derive(Debug, Deserialize)]
struct ObservationResponse {
    properties: ObservationProperties,
}

/// Current-condition fields from an NWS station observation, each present
/// only when the station reported it. The caller applies each `Some` over Open-Meteo.
#[derive(Debug)]
pub(crate) struct NwsObservation {
    /// Temperature in the requested unit (Fahrenheit or Celsius).
    temperature: Option<f32>,
    /// Wind speed in the requested unit (mph or km/h).
    windspeed: Option<f32>,
    /// Relative humidity as a percentage (0-100).
    humidity: Option<i32>,
    /// Apparent temperature (heat index / wind chill), requested unit.
    feels_like: Option<f32>,
    /// Wind bearing in degrees (0-360). Caller derives the compass label.
    wind_direction: Option<i32>,
    /// Wind gust speed in the requested unit.
    wind_gusts: Option<f32>,
    /// Visibility in meters.
    visibility: Option<f32>,
    /// Surface pressure in hPa.
    pressure: Option<f32>,
    /// Dew point in the requested temperature unit.
    dew_point: Option<f32>,
}

/// Returns the nearest NWS station's current observations in the caller's
/// requested unit. Returns `None` on any failure.
pub(crate) async fn override_current_observations(
    latitude: f64,
    longitude: f64,
    temperature_unit: TemperatureUnit,
    measurement_system: MeasurementSystem,
) -> Option<NwsObservation> {
    let point = format!("{latitude:.4},{longitude:.4}");
    let station_ids = cached_station_ids(&point).await?;

    // Stations are distance-ordered; walk the nearest few until one returns a
    // usable obs. A failed fetch OR a null-temp obs falls through to the next
    // hop rather than aborting the override - hence `else {continue }`, not `?`.
    for id in station_ids.iter().take(MAX_HOPS) {
        let url = format!("{STATIONS_URL_PREFIX}{id}/observations/latest");
        let Some(obs) = get_json::<ObservationResponse>(&url, "NWS observation").await else {
            continue;
        };
        if let Some(parsed) =
            parse_observation(obs.properties, temperature_unit, measurement_system)
        {
            tracing::debug!("NWS override from station {id}");
            return Some(parsed);
        }
    }

    tracing::debug!("no usable NWS observation within {MAX_HOPS}");
    None
}

/// Resolves a `lat,lon` point to its grid's ordered observation-station IDs.
/// `point` is the pre-formatted `"{:.4},{:.4}"` string to avoid NWS bounce
async fn fetch_station_ids(point: &str) -> Option<Vec<String>> {
    // point -> the URL of this grid's station collection
    let points_url = format!("{POINTS_URL_PREFIX}{point}");
    let points: PointsResponse = get_json(&points_url, "NWS points").await?;

    // fetch the ordered list of station identifiers
    let stations: StationsResponse =
        get_json(&points.properties.observation_stations, "NWS stations").await?;

    let ids: Vec<String> = stations
        .features
        .into_iter()
        .map(|f| f.properties.station_identifier)
        .collect();

    tracing::debug!("NWS grid resolved {} station(s)", ids.len());
    Some(ids)
}

/// Returns the grid's ordered station IDs, fetching and caching on first call.
/// Cache misses retry on subsequent calls so a transient failure doesn't
/// poison the override for the process lifetime.
async fn cached_station_ids(point: &str) -> Option<Vec<String>> {
    if let Ok(guard) = STATION_CACHE.read() {
        if let Some(ids) = guard.as_ref().and_then(|m| m.get(point)) {
            return Some(ids.clone());
        }
    } // read guard dropped here - before the wait

    let ids = fetch_station_ids(point).await?; // no lock held across .await

    if !ids.is_empty() {
        if let Ok(mut guard) = STATION_CACHE.write() {
            guard
                .get_or_insert_with(HashMap::new)
                .insert(point.to_string(), ids.clone());
        }
    }
    Some(ids)
}

/// Convert Celsius to caller's temperature unit.
fn c_to_unit(celsius: f64, unit: TemperatureUnit) -> f32 {
    match unit {
        TemperatureUnit::Celsius => celsius as f32,
        TemperatureUnit::Fahrenheit => (celsius * 9.0 / 5.0 + 32.0) as f32,
    }
}

/// Convert km/h to the caller's measurement system (mph for Imperial, km/h for Metric).
fn kmh_to_system(kmh: f64, system: MeasurementSystem) -> f32 {
    match system {
        MeasurementSystem::Metric => kmh as f32,
        MeasurementSystem::Imperial => (kmh / 1.609_344) as f32,
    }
}

/// Converts a raw NWS observation into an `NwsObservation` in the caller's units
/// Returns `None` when the station reports no temperature: a null-temp obs means
/// a stale/offline station, so the caller should try the next hop
fn parse_observation(
    props: ObservationProperties,
    unit: TemperatureUnit,
    system: MeasurementSystem,
) -> Option<NwsObservation> {
    // Temperature is the usability gate. `?` here doubles as "try next hop"
    // signal: no temp -> None -> the loop moves on.
    let temperature = props.temperature.value?;

    // feels_like precedence: heatIndex then windChill, then the station's
    // own temperature - all from this station, never Open-Meteo's apparent temp
    // `Temperature is known present, so this is always `Some`.
    let feels_like = props
        .heat_index
        .value
        .or(props.wind_chill.value)
        .or(Some(temperature))
        .map(|c| c_to_unit(c, unit));

    Some(NwsObservation {
        temperature: Some(c_to_unit(temperature, unit)),
        windspeed: props.wind_speed.value.map(|k| kmh_to_system(k, system)),
        humidity: props.relative_humidity.value.map(|v| v.round() as i32),
        feels_like,
        wind_direction: props.wind_direction.value.map(|v| v.round() as i32),
        wind_gusts: props.wind_gust.value.map(|k| kmh_to_system(k, system)),
        visibility: props.visibility.value.map(|m| m as f32), //meters, stored direct
        // barometric preferred, sea-level fallback; both Pa -> hPa
        pressure: props
            .barometric_pressure
            .value
            .or(props.sea_level_pressure.value)
            .map(|pa| (pa / 100.0) as f32),
        dew_point: props.dewpoint.value.map(|c| c_to_unit(c, unit)),
    })
}

impl NwsObservation {
    /// overlays each present reading onto 'cw', leaving absent fields at
    /// Open-Meteo's value. When a wind bearing is present it also refreshes the
    /// compass label, so the two never drift apart.
    pub(crate) fn apply_to(&self, cw: &mut CurrentWeather) {
        if let Some(v) = self.temperature {
            cw.temperature = v;
        }
        if let Some(v) = self.windspeed {
            cw.windspeed = v;
        }
        if let Some(v) = self.humidity {
            cw.humidity = v;
        }
        if let Some(v) = self.feels_like {
            cw.feels_like = v;
        }
        if let Some(v) = self.wind_gusts {
            cw.wind_gusts = v;
        }
        if let Some(v) = self.visibility {
            cw.visibility = v;
        }
        if let Some(v) = self.pressure {
            cw.pressure = v;
        }
        if let Some(v) = self.dew_point {
            cw.dew_point = v;
        }
        if let Some(v) = self.wind_direction {
            cw.wind_direction = v;
            cw.compass_direction = CompassDirection::from_degrees(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codes::WeatherCondition;

    fn meas(v: f64) -> Measurement {
        Measurement { value: Some(v) }
    }
    fn empty() -> Measurement {
        Measurement { value: None }
    }

    /// Fully-populated obs; tests null out fields as needed.
    fn sample() -> ObservationProperties {
        ObservationProperties {
            temperature: meas(20.0),       // 68°F
            dewpoint: meas(10.0),          // 50°F
            relative_humidity: meas(55.4), // → 55
            wind_direction: meas(180.0),
            wind_speed: meas(16.093_44),          // 10 mph
            wind_gust: meas(32.186_88),           // 20 mph
            barometric_pressure: meas(101_325.0), // 1013.25 hPa
            sea_level_pressure: empty(),
            visibility: meas(16_093.4), // meters, stored direct
            heat_index: empty(),
            wind_chill: empty(),
        }
    }

    /// Baseline Open-Meteo current conditions to overlay onto.
    fn om_current() -> CurrentWeather {
        CurrentWeather {
            temperature: 60.0,
            weathercode: 3,
            condition: WeatherCondition::from_code(3),
            windspeed: 5.0,
            humidity: 40,
            feels_like: 58.0,
            wind_direction: 90,
            compass_direction: CompassDirection::from_degrees(90),
            wind_gusts: 8.0,
            uv_index: 4.0,
            visibility: 10_000.0,
            pressure: 1_000.0,
            cloud_cover: 75,
            dew_point: 45.0,
        }
    }

    #[test]
    fn converters_round_trip() {
        assert!((c_to_unit(0.0, TemperatureUnit::Fahrenheit) - 32.0).abs() < 0.001);
        assert!((c_to_unit(100.0, TemperatureUnit::Fahrenheit) - 212.0).abs() < 0.001);
        assert_eq!(c_to_unit(18.5, TemperatureUnit::Celsius), 18.5);
        assert!((kmh_to_system(1.609_344, MeasurementSystem::Imperial) - 1.0).abs() < 0.001);
        assert_eq!(kmh_to_system(50.0, MeasurementSystem::Metric), 50.0);
    }

    #[test]
    fn parse_maps_and_converts_every_field() {
        let o = parse_observation(
            sample(),
            TemperatureUnit::Fahrenheit,
            MeasurementSystem::Imperial,
        )
        .unwrap();
        assert!((o.temperature.unwrap() - 68.0).abs() < 0.01);
        assert!((o.dew_point.unwrap() - 50.0).abs() < 0.01);
        assert_eq!(o.humidity.unwrap(), 55);
        assert_eq!(o.wind_direction.unwrap(), 180);
        assert!((o.windspeed.unwrap() - 10.0).abs() < 0.01);
        assert!((o.wind_gusts.unwrap() - 20.0).abs() < 0.01);
        assert!((o.pressure.unwrap() - 1013.25).abs() < 0.01);
        assert!((o.visibility.unwrap() - 16_093.4).abs() < 0.5);
    }

    #[test]
    fn none_without_temperature() {
        let mut p = sample();
        p.temperature = empty();
        assert!(
            parse_observation(p, TemperatureUnit::Celsius, MeasurementSystem::Metric).is_none()
        );
    }

    #[test]
    fn feels_like_precedence() {
        let mut p = sample(); // heatIndex wins
        p.heat_index = meas(30.0);
        p.wind_chill = meas(5.0);
        assert_eq!(parse(p).feels_like.unwrap(), 30.0);

        let mut p = sample(); // windChill next
        p.wind_chill = meas(5.0);
        assert_eq!(parse(p).feels_like.unwrap(), 5.0);

        assert_eq!(parse(sample()).feels_like.unwrap(), 20.0); // falls back to temp
    }

    #[test]
    fn absent_optionals_stay_none() {
        let mut p = sample();
        p.visibility = empty();
        p.barometric_pressure = empty(); // sea_level also empty in sample()
        let o = parse(p);
        assert!(o.visibility.is_none());
        assert!(o.pressure.is_none());
    }

    // Celsius/Metric so feels_like/temps read as raw °C in assertions.
    fn parse(p: ObservationProperties) -> NwsObservation {
        parse_observation(p, TemperatureUnit::Celsius, MeasurementSystem::Metric).unwrap()
    }

    #[test]
    fn apply_to_overlays_present_preserves_absent_and_pairs_compass() {
        let nws = NwsObservation {
            temperature: Some(68.0),
            windspeed: Some(10.0),
            humidity: None, // absent → keep Open-Meteo 40
            feels_like: None,
            wind_direction: Some(180), // must refresh compass to S
            wind_gusts: None,
            visibility: None,
            pressure: Some(1013.25),
            dew_point: Some(50.0),
        };
        let mut cw = om_current();
        nws.apply_to(&mut cw);

        // present → overlaid
        assert!((cw.temperature - 68.0).abs() < 0.01);
        assert!((cw.windspeed - 10.0).abs() < 0.01);
        assert!((cw.pressure - 1013.25).abs() < 0.01);
        assert!((cw.dew_point - 50.0).abs() < 0.01);
        // wind bearing + compass move together
        assert_eq!(cw.wind_direction, 180);
        assert_eq!(cw.compass_direction, CompassDirection::from_degrees(180));
        // absent → Open-Meteo preserved
        assert_eq!(cw.humidity, 40);
        assert!((cw.wind_gusts - 8.0).abs() < 0.01);
        assert!((cw.feels_like - 58.0).abs() < 0.01);
        // modeled sky fields NWS never touches
        assert_eq!(cw.weathercode, 3);
        assert_eq!(cw.condition, WeatherCondition::from_code(3));
        assert_eq!(cw.cloud_cover, 75);
        assert!((cw.uv_index - 4.0).abs() < 0.01);
    }

    #[test]
    fn apply_to_all_absent_is_a_noop() {
        let nws = NwsObservation {
            temperature: None,
            windspeed: None,
            humidity: None,
            feels_like: None,
            wind_direction: None,
            wind_gusts: None,
            visibility: None,
            pressure: None,
            dew_point: None,
        };
        let before = om_current();
        let mut cw = before.clone(); // Clone, not PartialEq → compare fields
        nws.apply_to(&mut cw);
        assert!((cw.temperature - before.temperature).abs() < f32::EPSILON);
        assert_eq!(cw.wind_direction, before.wind_direction);
        assert_eq!(cw.compass_direction, before.compass_direction); // untouched when bearing absent
        assert_eq!(cw.humidity, before.humidity);
        assert!((cw.pressure - before.pressure).abs() < f32::EPSILON);
    }
}
