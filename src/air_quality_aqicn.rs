// SPDX-License-Identifier: MIT OR Apache-2.0

//! World Air Quality Index (aqicn.org) headline AQI fetch.
//!
//! aqicn sources from ground monitoring stations worldwide and reports on
//! the US EPA scale. We take only the headline AQI and let the Open-Meteo
//! response carry the µg/m³ pollutant fields, so the `AirQualityData`
//! unit contract stays honest.
//!
//! A token is required. Free tokens are issued at aqicn.org/data-platform/token/.
//! Any failure returns `None` and the caller falls through to Open-Meteo.

use crate::client::get_text;

const FEED_URL_PREFIX: &str = "https://api.waqi.info/feed/geo:";

/// Fetches the headline AQI (US EPA scale) for a coordinate. Returns `None`
/// on any failure: bad token, network error, non-ok status, or missing data.
pub(crate) async fn fetch_headline_aqi(latitude: f64, longitude: f64, token: &str) -> Option<i32> {
    if token.trim().is_empty() {
        return None;
    }

    let url = format!(
        "{FEED_URL_PREFIX}{latitude};{longitude}/?token={}",
        urlencoding::encode(token)
    );

    let body = get_text(&url, "aqicn").await?;
    extract_aqi(&body)
}

/// Pulls the headline AQI out of an aqicn feed response. Lives in its own
/// function so it can be unit-tested against fixtures without a live network.
///
/// aqicn uses `data: { ... }` on success and `data: "reason string"` on
/// errors, so we walk a `serde_json::Value` rather than a typed struct.
fn extract_aqi(json_text: &str) -> Option<i32> {
    let value: serde_json::Value = serde_json::from_str(json_text)
        .map_err(|e| tracing::debug!("aqicn response parse failed: {e}"))
        .ok()?;

    let status = value.get("status")?.as_str()?;
    if status != "ok" {
        tracing::debug!("aqicn returned non-ok status: {status}");
        return None;
    }

    value.get("data")?.get("aqi")?.as_i64().map(|v| v as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_aqi_from_ok_response() {
        let json = r#"{
            "status": "ok",
            "data": {
                "aqi": 52,
                "iaqi": { "pm25": { "v": 22 } },
                "city": { "name": "Seoul" }
            }
        }"#;
        assert_eq!(extract_aqi(json), Some(52));
    }

    #[test]
    fn returns_none_on_error_status() {
        let json = r#"{"status":"error","data":"Invalid key"}"#;
        assert_eq!(extract_aqi(json), None);
    }

    #[test]
    fn returns_none_when_aqi_missing() {
        let json = r#"{"status":"ok","data":{"iaqi":{}}}"#;
        assert_eq!(extract_aqi(json), None);
    }

    #[test]
    fn returns_none_on_malformed_json() {
        assert_eq!(extract_aqi("not json"), None);
        assert_eq!(extract_aqi(""), None);
    }

    #[test]
    fn returns_none_when_status_field_absent() {
        assert_eq!(extract_aqi(r#"{"data":{"aqi":42}}"#), None);
    }
}
