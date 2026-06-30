// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pollen concentrations from Open-Meteo's air-quality endpoint.
//!
//! Coverage is Europe only, sourced from the CAMS European Air Quality
//! Forecast model. Calls outside Europe return `Ok(None)` rather than an
//! error so callers can treat pollen as a region-optional feature the same
//! way they already do for alerts.

use serde::{Deserialize, Serialize};

use crate::client::http_client;
use crate::error::Result;

/// Current pollen concentrations in grains per cubic meter.
///
/// Values are zero or positive. A value of `0.0` in a covered location
/// means the species is not actively producing pollen there right now
/// (off-season, or not regionally present), which is distinct from "no
/// data." When the API has no coverage at all, [`fetch_pollen`] returns
/// `Ok(None)` instead of zero-filled data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollenData {
    /// Alder (Alnus). Early-spring tree pollen.
    pub alder: f32,
    /// Birch (Betula). Spring tree pollen.
    pub birch: f32,
    /// Grass. Late spring through summer.
    pub grass: f32,
    /// Mugwort (Artemisia). Late-summer weed pollen.
    pub mugwort: f32,
    /// Olive (Olea). Mediterranean tree pollen.
    pub olive: f32,
    /// Ragweed (Ambrosia). Late summer through autumn.
    pub ragweed: f32,
}

/// Fetches current pollen concentrations.
///
/// Open-Meteo's pollen variables are sourced from the CAMS European Air
/// Quality Forecast model and only cover Europe. Calls outside that
/// coverage area return `Ok(None)` rather than an error. Inside coverage
/// you always get back `Ok(Some(_))` with all six species populated; any
/// species not in season locally reads as `0.0`.
///
/// As of the 0.4.0 release Open-Meteo is the only free, no-key public
/// pollen API. The US and Asia have no free coverage from any provider,
/// which is why this function returns `Option` rather than always
/// returning a struct.
pub async fn fetch_pollen(latitude: f64, longitude: f64) -> Result<Option<PollenData>> {
    let url = format!(
        "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=alder_pollen,birch_pollen,grass_pollen,mugwort_pollen,olive_pollen,ragweed_pollen&timezone=auto",
        latitude, longitude
    );

    let response = http_client()?.get(&url).send().await?.error_for_status()?;
    let data: PollenResponse = response.json().await?;

    Ok(from_current(&data.current))
}

/// Builds a `PollenData` from a deserialized current block. Returns `None`
/// when all six species come back null, which is how Open-Meteo signals
/// "this coordinate is outside CAMS European coverage." Lives in its own
/// function so it can be unit-tested against fixtures without a live
/// network.
fn from_current(current: &PollenCurrent) -> Option<PollenData> {
    let any_value = current.alder_pollen.is_some()
        || current.birch_pollen.is_some()
        || current.grass_pollen.is_some()
        || current.mugwort_pollen.is_some()
        || current.olive_pollen.is_some()
        || current.ragweed_pollen.is_some();
    if !any_value {
        return None;
    }
    Some(PollenData {
        alder: current.alder_pollen.unwrap_or(0.0),
        birch: current.birch_pollen.unwrap_or(0.0),
        grass: current.grass_pollen.unwrap_or(0.0),
        mugwort: current.mugwort_pollen.unwrap_or(0.0),
        olive: current.olive_pollen.unwrap_or(0.0),
        ragweed: current.ragweed_pollen.unwrap_or(0.0),
    })
}

/// Open-Meteo Air Quality API response, pollen subset.
#[derive(Debug, Deserialize)]
struct PollenResponse {
    current: PollenCurrent,
}

#[derive(Debug, Deserialize)]
struct PollenCurrent {
    alder_pollen: Option<f32>,
    birch_pollen: Option<f32>,
    grass_pollen: Option<f32>,
    mugwort_pollen: Option<f32>,
    olive_pollen: Option<f32>,
    ragweed_pollen: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json_text: &str) -> Option<PollenData> {
        let resp: PollenResponse = serde_json::from_str(json_text)
            .expect("pollen test helper: inline JSON did not deserialize as PollenResponse");
        from_current(&resp.current)
    }

    #[test]
    #[rustfmt::skip]
    fn returns_some_when_covered_with_active_species() {
        let json = r#"{
            "current": {
                "alder_pollen": 0.0,
                "birch_pollen": 0.1,
                "grass_pollen": 3.5,
                "mugwort_pollen": 0.0,
                "olive_pollen": 0.0,
                "ragweed_pollen": 0.0
            }
        }"#;
        let data = parse(json).expect("returns_some_when_covered_with_active_species: parse() returned None for an active-species fixture");
        assert_eq!(data.birch, 0.1);
        assert_eq!(data.grass, 3.5);
        assert_eq!(data.alder, 0.0);
    }

    #[test]
    #[rustfmt::skip]
    fn returns_some_when_covered_with_all_zero_species() {
        let json = r#"{
            "current": {
                "alder_pollen": 0.0,
                "birch_pollen": 0.0,
                "grass_pollen": 0.0,
                "mugwort_pollen": 0.0,
                "olive_pollen": 0.0,
                "ragweed_pollen": 0.0
            }
        }"#;
        let data = parse(json).expect("returns_some_when_covered_with_all_zero_species: parse() returned None for an all-zero-but-present fixture");
        assert_eq!(data.grass, 0.0);
    }

    #[test]
    fn returns_none_when_all_six_null() {
        let json = r#"{
            "current": {
                "alder_pollen": null,
                "birch_pollen": null,
                "grass_pollen": null,
                "mugwort_pollen": null,
                "olive_pollen": null,
                "ragweed_pollen": null
            }
        }"#;
        assert!(parse(json).is_none());
    }

    #[test]
    #[rustfmt::skip]
    fn zero_fills_when_some_species_null_but_others_present() {
        let json = r#"{
            "current": {
                "alder_pollen": null,
                "birch_pollen": null,
                "grass_pollen": 5.0,
                "mugwort_pollen": null,
                "olive_pollen": null,
                "ragweed_pollen": null
            }
        }"#;
        let data = parse(json).expect("zero_fills_when_some_species_null_but_others_present: parse() returned None when at least one species is present");
        assert_eq!(data.grass, 5.0);
        assert_eq!(data.alder, 0.0);
    }
}
