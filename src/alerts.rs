// SPDX-License-Identifier: MIT OR Apache-2.0

//! Weather alerts from regional providers (NWS, MeteoAlarm, ECCC, BOM).

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::client::http_client;
use crate::error::Result;
use crate::geo::{
    detect_country_from_coords, detect_region, encode_geohash, get_eccc_office_codes,
    get_meteoalarm_info, point_in_polygon, MeteoAlarmCodenames, NominatimResponse, Region,
};

/// Weather alert severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSeverity {
    /// Low-impact advisory. Frost warnings, wind advisories, etc.
    Minor,
    /// Moderate impact. Weather watches, flood advisories.
    Moderate,
    /// High impact. Severe thunderstorm warnings, winter storm warnings.
    Severe,
    /// Life-threatening. Tornado warnings, hurricane warnings.
    Extreme,
    /// Provider didn't include a severity or used an unrecognized value.
    Unknown,
}

impl AlertSeverity {
    /// Parses CAP severity string into enum variant.
    /// Handles variations across providers (NWS, MeteoAlarm, ECCC).
    fn from_cap_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "minor" => Self::Minor,
            "moderate" => Self::Moderate,
            "severe" | "major" => Self::Severe,
            "extreme" => Self::Extreme,
            _ => Self::Unknown,
        }
    }
}

/// Weather alert from NWS, MeteoAlarm, ECCC, or BOM.
#[derive(Debug, Clone)]
pub struct Alert {
    /// Provider-specific identifier for deduplication.
    pub id: String,
    /// Short event type (e.g. "Tornado Warning", "Heat Advisory").
    pub event: String,
    /// How bad it is.
    pub severity: AlertSeverity,
    /// One-line summary from the provider.
    pub headline: String,
    /// Full alert text. May be empty for some providers (MeteoAlarm, BOM).
    pub description: String,
    /// When this alert stops being relevant.
    pub expires: DateTime<Utc>,
}

/// Fetches active weather alerts based on location.
/// Dispatches to the appropriate regional API.
pub async fn fetch_alerts(latitude: f64, longitude: f64) -> Result<Vec<Alert>> {
    match detect_region(latitude, longitude) {
        Region::Us => fetch_nws_alerts(latitude, longitude).await,
        Region::Europe => {
            let country = detect_country_from_coords(latitude, longitude)
                .await
                .unwrap_or_default();
            fetch_meteoalarm_alerts(latitude, longitude, &country).await
        }
        Region::Canada => fetch_eccc_alerts(latitude, longitude).await,
        Region::Australia => fetch_bom_alerts(latitude, longitude).await,
        Region::Unknown => Ok(vec![]),
    }
}

// ---------------------------------------------------------------------------
// NWS (United States)
// ---------------------------------------------------------------------------

/// NWS API GeoJSON response.
#[derive(Debug, Deserialize)]
struct NwsAlertsResponse {
    features: Vec<NwsAlertFeature>,
}

#[derive(Debug, Deserialize)]
struct NwsAlertFeature {
    properties: NwsAlertProperties,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NwsAlertProperties {
    id: String,
    event: String,
    severity: Option<String>,
    headline: Option<String>,
    description: Option<String>,
    sent: String,
    expires: Option<String>,
}

/// Fetches active weather alerts from the NWS API for US locations.
async fn fetch_nws_alerts(latitude: f64, longitude: f64) -> Result<Vec<Alert>> {
    let url = format!(
        "https://api.weather.gov/alerts/active?point={},{}",
        latitude, longitude
    );

    let response = http_client()?
        .get(&url)
        .header("Accept", "application/geo+json")
        .send()
        .await?;

    if !response.status().is_success() {
        tracing::warn!("NWS API returned status: {}", response.status());
        return Ok(vec![]);
    }

    let data: NwsAlertsResponse = response.json().await?;

    let alerts: Vec<Alert> = data
        .features
        .into_iter()
        .filter_map(|feature| {
            let props = feature.properties;

            let sent = DateTime::parse_from_rfc3339(&props.sent)
                .ok()?
                .with_timezone(&Utc);

            let expires = props
                .expires
                .as_ref()
                .and_then(|e| DateTime::parse_from_rfc3339(e).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|| sent + chrono::Duration::hours(24));

            if expires < Utc::now() {
                return None;
            }

            Some(Alert {
                id: props.id,
                event: props.event,
                severity: props
                    .severity
                    .as_deref()
                    .map(AlertSeverity::from_cap_string)
                    .unwrap_or(AlertSeverity::Unknown),
                headline: props.headline.unwrap_or_default(),
                description: props.description.unwrap_or_default(),
                expires,
            })
        })
        .collect();

    tracing::debug!("Fetched {} alert(s) from NWS", alerts.len());
    Ok(alerts)
}

// ---------------------------------------------------------------------------
// MeteoAlarm (Europe)
// ---------------------------------------------------------------------------

/// MeteoAlarm Atom feed response.
#[derive(Debug, Deserialize)]
struct MeteoAlarmFeed {
    #[serde(rename = "entry", default)]
    entries: Vec<MeteoAlarmEntry>,
}

/// Single alert entry from MeteoAlarm Atom feed.
#[derive(Debug, Deserialize)]
struct MeteoAlarmEntry {
    id: String,
    title: Option<String>,
    #[serde(rename = "identifier")]
    cap_identifier: Option<String>,
    #[serde(rename = "event")]
    cap_event: Option<String>,
    #[serde(rename = "severity")]
    cap_severity: Option<String>,
    #[serde(rename = "sent")]
    cap_sent: Option<String>,
    #[serde(rename = "expires")]
    cap_expires: Option<String>,
    #[serde(rename = "geocode")]
    cap_geocode: Option<MeteoAlarmGeocode>,
}

/// Geocode element containing EMMA_ID area identifier.
#[derive(Debug, Deserialize)]
struct MeteoAlarmGeocode {
    value: Option<String>,
}

/// Resolves the user's EMMA_ID by looking up their location and matching against codenames.
async fn resolve_user_emma_id(latitude: f64, longitude: f64, country_code: &str) -> Option<String> {
    let nominatim_url = format!(
        "https://nominatim.openstreetmap.org/reverse?lat={}&lon={}&format=json",
        latitude, longitude
    );

    let response = http_client().ok()?.get(&nominatim_url).send().await.ok()?;
    let nominatim: NominatimResponse = response.json().await.ok()?;
    let address = nominatim.address?;

    // Build list of location names to search for (most specific to least)
    let mut search_terms: Vec<String> = Vec::new();

    if let Some(city) = &address.city {
        search_terms.push(city.clone());
        search_terms.push(format!("Stadt {}", city));
    }
    if let Some(town) = &address.town {
        search_terms.push(town.clone());
    }
    if let Some(county) = &address.county {
        search_terms.push(county.clone());
        search_terms.push(format!("Kreis {}", county));
    }
    if let Some(state) = &address.state {
        search_terms.push(state.clone());
    }

    // Fetch MeteoAlarm codenames
    let codenames_url =
        "https://raw.githubusercontent.com/ktrue/Meteoalarm-warning/master/meteoalarm-codenames.json";
    let codenames_response = http_client().ok()?.get(codenames_url).send().await.ok()?;
    let codenames: MeteoAlarmCodenames = codenames_response.json().await.ok()?;

    // Find matching EMMA_ID for this country
    let country_prefix = country_code.to_uppercase();
    for search_term in &search_terms {
        let search_lower = search_term.to_lowercase();
        for (emma_id, name) in &codenames.codes {
            if !emma_id.starts_with(&country_prefix) {
                continue;
            }
            if name.to_lowercase().contains(&search_lower)
                || search_lower.contains(&name.to_lowercase())
            {
                tracing::debug!(
                    "Resolved EMMA_ID: {} ({}) for search term '{}'",
                    emma_id,
                    name,
                    search_term
                );
                return Some(emma_id.clone());
            }
        }
    }

    tracing::debug!(
        "Could not resolve EMMA_ID for location: {:?}",
        search_terms.first()
    );
    None
}

/// Fetches active weather alerts from MeteoAlarm for European locations.
async fn fetch_meteoalarm_alerts(
    latitude: f64,
    longitude: f64,
    country: &str,
) -> Result<Vec<Alert>> {
    let (slug, country_code) = match get_meteoalarm_info(country) {
        Some(info) => info,
        None => {
            tracing::debug!("Country '{}' not covered by MeteoAlarm", country);
            return Ok(vec![]);
        }
    };

    let user_emma_id = resolve_user_emma_id(latitude, longitude, country_code).await;

    let url = format!(
        "https://feeds.meteoalarm.org/feeds/meteoalarm-legacy-atom-{}",
        slug
    );

    let response = http_client()?.get(&url).send().await?;
    if !response.status().is_success() {
        tracing::warn!("MeteoAlarm returned status: {}", response.status());
        return Ok(vec![]);
    }

    let xml_text = response.text().await?;
    let feed: MeteoAlarmFeed = quick_xml::de::from_str(&xml_text)?;

    let alerts: Vec<Alert> = feed
        .entries
        .into_iter()
        .filter_map(|entry| parse_meteoalarm_entry(entry, &user_emma_id))
        .collect();

    tracing::debug!(
        "Fetched {} alert(s) from MeteoAlarm ({})",
        alerts.len(),
        country
    );
    Ok(alerts)
}

/// Parses a MeteoAlarm entry into an Alert.
/// Returns None if it doesn't match the user's EMMA_ID or is expired.
fn parse_meteoalarm_entry(entry: MeteoAlarmEntry, user_emma_id: &Option<String>) -> Option<Alert> {
    let now = Utc::now();

    // Filter by EMMA_ID if we resolved one for the user
    if let Some(user_id) = user_emma_id {
        let entry_emma_id = entry.cap_geocode.as_ref().and_then(|gc| gc.value.as_ref());

        match entry_emma_id {
            Some(entry_id) if entry_id != user_id => return None,
            _ => {}
        }
    }

    let sent = entry
        .cap_sent
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(now);

    let expires = entry
        .cap_expires
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| sent + chrono::Duration::hours(24));

    if expires < now {
        return None;
    }

    let event = entry
        .cap_event
        .unwrap_or_else(|| "Weather Alert".to_string());

    let headline = entry.title.unwrap_or_else(|| event.clone());

    let severity = entry
        .cap_severity
        .as_deref()
        .map(AlertSeverity::from_cap_string)
        .unwrap_or(AlertSeverity::Unknown);

    Some(Alert {
        id: entry.cap_identifier.unwrap_or(entry.id),
        event,
        severity,
        headline,
        description: String::new(),
        expires,
    })
}

// ---------------------------------------------------------------------------
// ECCC (Canada)
// ---------------------------------------------------------------------------

/// ECCC CAP alert response structure.
#[derive(Debug, Deserialize)]
struct EcccCapAlert {
    identifier: String,
    status: String,
    #[serde(rename = "msgType")]
    msg_type: String,
    sent: String,
    #[serde(rename = "info", default)]
    info_blocks: Vec<EcccCapInfo>,
}

/// Info block from ECCC CAP alert (one per language).
#[derive(Debug, Deserialize)]
struct EcccCapInfo {
    language: Option<String>,
    event: Option<String>,
    severity: Option<String>,
    expires: Option<String>,
    headline: Option<String>,
    description: Option<String>,
    #[serde(rename = "area", default)]
    areas: Vec<EcccCapArea>,
}

/// Area element from ECCC CAP alert.
#[derive(Debug, Deserialize)]
struct EcccCapArea {
    #[serde(rename = "areaDesc")]
    area_desc: Option<String>,
    polygon: Option<String>,
}

/// Fetches active weather alerts from ECCC (Environment and Climate Change Canada).
async fn fetch_eccc_alerts(latitude: f64, longitude: f64) -> Result<Vec<Alert>> {
    let offices = get_eccc_office_codes(latitude, longitude);
    let today = chrono::Utc::now().format("%Y%m%d").to_string();
    let client = http_client()?;

    let mut all_alerts: Vec<Alert> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for office in offices {
        let dir_url = format!(
            "https://dd.weather.gc.ca/today/alerts/cap/{}/{}/",
            today, office
        );

        let dir_response = match client.get(&dir_url).send().await {
            Ok(resp) if resp.status().is_success() => resp,
            _ => continue,
        };

        let dir_html = match dir_response.text().await {
            Ok(text) => text,
            Err(_) => continue,
        };

        // Parse hour directories from HTML listing
        let hour_dirs: Vec<String> = dir_html
            .lines()
            .filter_map(|line| {
                if line.contains("href=\"") && line.contains("/\"") {
                    let start = line.find("href=\"")? + 6;
                    let end = line[start..].find('"')? + start;
                    let href = &line[start..end];
                    if href.len() == 3 && href.ends_with('/') {
                        let hour = &href[..2];
                        if hour.chars().all(|c| c.is_ascii_digit()) {
                            return Some(hour.to_string());
                        }
                    }
                }
                None
            })
            .collect();

        for hour in hour_dirs {
            let hour_url = format!("{}{}/", dir_url, hour);

            let hour_response = match client.get(&hour_url).send().await {
                Ok(resp) if resp.status().is_success() => resp,
                _ => continue,
            };

            let hour_html = match hour_response.text().await {
                Ok(text) => text,
                Err(_) => continue,
            };

            let cap_files: Vec<String> = hour_html
                .lines()
                .filter_map(|line| {
                    if line.contains(".cap\"") {
                        let start = line.find("href=\"")? + 6;
                        let end = line[start..].find('"')? + start;
                        let href = &line[start..end];
                        if href.ends_with(".cap") {
                            return Some(href.to_string());
                        }
                    }
                    None
                })
                .collect();

            for cap_file in cap_files {
                let cap_url = format!("{}{}", hour_url, cap_file);

                let cap_response = match client.get(&cap_url).send().await {
                    Ok(resp) if resp.status().is_success() => resp,
                    _ => continue,
                };

                let cap_xml = match cap_response.text().await {
                    Ok(text) => text,
                    Err(_) => continue,
                };

                if let Some(alert) = parse_eccc_cap(&cap_xml, latitude, longitude, &mut seen_ids) {
                    all_alerts.push(alert);
                }
            }
        }
    }

    tracing::debug!("Fetched {} alert(s) from ECCC", all_alerts.len());
    Ok(all_alerts)
}

/// Parses an ECCC CAP XML document into an Alert.
/// Filters by location using polygon containment and deduplicates by identifier.
fn parse_eccc_cap(
    xml: &str,
    lat: f64,
    lon: f64,
    seen_ids: &mut std::collections::HashSet<String>,
) -> Option<Alert> {
    let cap: EcccCapAlert = quick_xml::de::from_str(xml).ok()?;

    if cap.status != "Actual" {
        return None;
    }

    if cap.msg_type == "Cancel" {
        return None;
    }

    // Find English info block (prefer en-CA)
    let info = cap
        .info_blocks
        .iter()
        .find(|i| {
            i.language
                .as_ref()
                .map(|l| l.starts_with("en"))
                .unwrap_or(false)
        })
        .or_else(|| cap.info_blocks.first())?;

    // Check if user's location is within any of the alert areas
    let mut location_matches = false;
    let mut area_desc = String::new();

    for area in &info.areas {
        if let Some(ref polygon) = area.polygon {
            if point_in_polygon(lat, lon, polygon) {
                location_matches = true;
                area_desc = area.area_desc.clone().unwrap_or_default();
                break;
            }
        }
    }

    if !location_matches {
        return None;
    }

    let event = info
        .event
        .clone()
        .unwrap_or_else(|| "Weather Alert".to_string());

    // Deduplicate by event type + area (ECCC issues updates with new identifiers)
    let dedup_key = format!("{}|{}", event, area_desc);
    if seen_ids.contains(&dedup_key) {
        return None;
    }
    seen_ids.insert(dedup_key);

    let now = Utc::now();

    let sent = cap
        .sent
        .parse::<DateTime<chrono::FixedOffset>>()
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(now);

    let expires = info
        .expires
        .as_ref()
        .and_then(|s| s.parse::<DateTime<chrono::FixedOffset>>().ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| sent + chrono::Duration::hours(24));

    if expires < now {
        return None;
    }

    let headline = info.headline.clone().unwrap_or_else(|| event.clone());

    Some(Alert {
        id: cap.identifier,
        event,
        severity: info
            .severity
            .as_deref()
            .map(AlertSeverity::from_cap_string)
            .unwrap_or(AlertSeverity::Unknown),
        headline,
        description: info.description.clone().unwrap_or_default(),
        expires,
    })
}

// ---------------------------------------------------------------------------
// BOM (Australia)
// ---------------------------------------------------------------------------

/// BOM API response wrapper.
#[derive(Debug, Deserialize)]
struct BomWarningsResponse {
    data: Vec<BomWarning>,
}

/// BOM API warning structure.
#[derive(Debug, Deserialize)]
struct BomWarning {
    id: String,
    #[serde(rename = "type")]
    warning_type: Option<String>,
    short_title: Option<String>,
    warning_group_type: Option<String>,
    phase: Option<String>,
    expiry_time: Option<String>,
}

/// Fetches weather alerts from the Australian Bureau of Meteorology.
async fn fetch_bom_alerts(latitude: f64, longitude: f64) -> Result<Vec<Alert>> {
    let geohash = encode_geohash(latitude, longitude, 6);
    let url = format!(
        "https://api.weather.bom.gov.au/v1/locations/{}/warnings",
        geohash
    );

    let response = http_client()?.get(&url).send().await?;

    if !response.status().is_success() {
        return Ok(vec![]);
    }

    let response_body: BomWarningsResponse = response.json().await?;
    let now = Utc::now();

    let alerts = response_body
        .data
        .into_iter()
        .filter(|w| w.phase.as_deref() != Some("cancelled"))
        .filter_map(|w| {
            let severity = match w.warning_group_type.as_deref() {
                Some("minor") => AlertSeverity::Minor,
                Some("moderate") => AlertSeverity::Moderate,
                Some("major") | Some("severe") => AlertSeverity::Severe,
                Some("extreme") => AlertSeverity::Extreme,
                _ => AlertSeverity::Unknown,
            };

            let expires = w
                .expiry_time
                .as_ref()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(now + chrono::Duration::hours(24));

            if expires < now {
                return None;
            }

            let headline = w
                .short_title
                .clone()
                .unwrap_or_else(|| "Weather Warning".to_string());
            let event = w
                .warning_type
                .as_ref()
                .map(|t| t.replace('_', " "))
                .unwrap_or_else(|| headline.clone());

            Some(Alert {
                id: w.id.clone(),
                event,
                severity,
                headline,
                description: String::new(),
                expires,
            })
        })
        .collect();

    Ok(alerts)
}
