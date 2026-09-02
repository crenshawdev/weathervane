// SPDX-License-Identifier: MIT OR Apache-2.0

//! Weather alerts from regional providers (NWS, MeteoAlarm, ECCC, BOM).

use std::collections::HashSet;
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::http_client;
use crate::error::{Error, Result};
use crate::geo::{
    detect_region, encode_geohash, get_eccc_office_codes, get_meteoalarm_info, point_in_polygon,
    reverse_geocode, MeteoAlarmCodenames, NominatimAddress, Region,
};

/// Weather alert severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// One alert with the provider's name for the area it covers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEntry {
    pub alert: Alert,
    /// Provider's area name for this entry: MeteoAlarm `cap:areaDesc`, NWS
    /// `areaDesc`, the containing ECCC polygon's `areaDesc`. Empty where the
    /// provider sends none (BOM).
    pub area_desc: String,
}

/// What `fetch_alerts_detailed` returns: the alerts, and whether they were
/// narrowed to the caller's area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertReport {
    pub alerts: Vec<AlertEntry>,
    /// `false` only when a MeteoAlarm national feed was returned unfiltered
    /// because no region could be matched for the location, by EMMA_ID or by
    /// area name, so the entries are national, not local. NWS, ECCC and BOM
    /// filter by point, polygon and geohash respectively, and an empty result
    /// is trivially filtered, so all of those are `true`.
    pub region_filtered: bool,
}

/// Fetches active weather alerts based on location, with each entry's area
/// name and whether the list was narrowed to the caller's area.
/// Dispatches to the appropriate regional API.
pub async fn fetch_alerts_detailed(latitude: f64, longitude: f64) -> Result<AlertReport> {
    match detect_region(latitude, longitude) {
        Region::Us => fetch_nws_alerts(latitude, longitude).await,
        Region::Europe => fetch_meteoalarm_alerts(latitude, longitude).await,
        Region::Canada => fetch_eccc_alerts(latitude, longitude).await,
        Region::Australia => fetch_bom_alerts(latitude, longitude).await,
        Region::Unknown => Ok(AlertReport {
            alerts: vec![],
            region_filtered: true,
        }),
    }
}

/// Fetches active weather alerts based on location.
/// Thin wrapper over `fetch_alerts_detailed` that drops the area names and
/// the filtering flag.
pub async fn fetch_alerts(latitude: f64, longitude: f64) -> Result<Vec<Alert>> {
    let report = fetch_alerts_detailed(latitude, longitude).await?;
    Ok(report.alerts.into_iter().map(|entry| entry.alert).collect())
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
    #[serde(rename = "areaDesc")]
    area_desc: Option<String>,
}

/// Fetches active weather alerts from the NWS API for US locations.
async fn fetch_nws_alerts(latitude: f64, longitude: f64) -> Result<AlertReport> {
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
        return Ok(AlertReport {
            alerts: vec![],
            region_filtered: true,
        });
    }

    let data: NwsAlertsResponse = response.json().await?;

    let alerts = nws_alerts_from_response(data);

    tracing::debug!("Fetched {} alert(s) from NWS", alerts.len());
    // The point query already narrowed the list to the caller's location.
    Ok(AlertReport {
        alerts,
        region_filtered: true,
    })
}

/// Lives in its own function so it can be unit-tested against fixtures without a live network.
fn nws_alerts_from_response(data: NwsAlertsResponse) -> Vec<AlertEntry> {
    data.features
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

            Some(AlertEntry {
                alert: Alert {
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
                },
                area_desc: props.area_desc.unwrap_or_default(),
            })
        })
        .collect()
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
    #[serde(rename = "areaDesc")]
    cap_area_desc: Option<String>,
}

/// Geocode element containing EMMA_ID area identifier.
#[derive(Debug, Deserialize)]
struct MeteoAlarmGeocode {
    /// Which scheme `value` belongs to. Most feeds say `EMMA_ID`; France says
    /// `NUTS3`, whose codes are not EMMA_IDs and must not be filtered as such.
    #[serde(rename = "valueName")]
    value_name: Option<String>,
    value: Option<String>,
}

/// Resolves the user's EMMA_ID by matching their place names against the
/// MeteoAlarm codename list.
async fn resolve_user_emma_id(address: &NominatimAddress, country_code: &str) -> Option<String> {
    let codenames = fetch_meteoalarm_codenames().await?;
    match_emma_id(address, country_code, &codenames)
}

/// The codename list, fetched once per process. It is 72 KB from a third
/// party's `master` branch and changes rarely, so refetching it on every
/// call was the largest request in the whole alerts path.
static CODENAMES_CACHE: RwLock<Option<MeteoAlarmCodenames>> = RwLock::new(None);

fn cached_codenames() -> Option<MeteoAlarmCodenames> {
    CODENAMES_CACHE.read().ok()?.clone()
}

fn cache_codenames(codenames: &MeteoAlarmCodenames) {
    if let Ok(mut guard) = CODENAMES_CACHE.write() {
        *guard = Some(codenames.clone());
    }
}

/// Cached front for the network fetch, in the shape of `STATION_CACHE` and
/// the geocode cache: filled on the first success, failures not cached so a
/// transient miss retries on the next call.
async fn fetch_meteoalarm_codenames() -> Option<MeteoAlarmCodenames> {
    if let Some(codenames) = cached_codenames() {
        return Some(codenames);
    }
    let codenames = fetch_meteoalarm_codenames_uncached().await?; // no lock held across .await
    cache_codenames(&codenames);
    Some(codenames)
}

/// Split from the matching so its three failure points get distinct messages
/// rather than collapsing into one silent `None`.
async fn fetch_meteoalarm_codenames_uncached() -> Option<MeteoAlarmCodenames> {
    const CODENAMES_URL: &str =
        "https://raw.githubusercontent.com/ktrue/Meteoalarm-warning/master/meteoalarm-codenames.json";

    let client = match http_client() {
        Ok(client) => client,
        Err(e) => {
            tracing::warn!(
                "No HTTP client for MeteoAlarm codenames ({}); region filter cannot be applied",
                e
            );
            return None;
        }
    };

    let response = match client.get(CODENAMES_URL).send().await {
        Ok(response) => response,
        Err(e) => {
            tracing::warn!(
                "MeteoAlarm codenames fetch failed ({}); region filter cannot be applied",
                e
            );
            return None;
        }
    };

    match response.json::<MeteoAlarmCodenames>().await {
        Ok(codenames) => Some(codenames),
        Err(e) => {
            tracing::warn!(
                "MeteoAlarm codenames decode failed ({}); region filter cannot be applied",
                e
            );
            None
        }
    }
}

/// Place names to try, most specific first.
fn emma_search_terms(address: &NominatimAddress) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();

    if let Some(city) = &address.city {
        terms.push(city.clone());
        terms.push(format!("Stadt {}", city));
    }
    if let Some(town) = &address.town {
        terms.push(town.clone());
    }
    if let Some(village) = &address.village {
        terms.push(village.clone());
    }
    if let Some(municipality) = &address.municipality {
        terms.push(municipality.clone());
    }
    if let Some(county) = &address.county {
        terms.push(county.clone());
        terms.push(format!("Kreis {}", county));
    }
    if let Some(state) = &address.state {
        terms.push(state.clone());
    }

    terms
}

/// The matching itself, split from the fetch so it is testable without a network.
fn match_emma_id(
    address: &NominatimAddress,
    country_code: &str,
    codenames: &MeteoAlarmCodenames,
) -> Option<String> {
    let country_prefix = country_code.to_uppercase();
    let search_terms = emma_search_terms(address);

    for search_term in &search_terms {
        let search_lower = search_term.to_lowercase();

        // Rank every candidate rather than taking the first hit. `codes` is a
        // HashMap, so "first hit" means whichever one iteration happened to
        // reach, and Vienna's "Wien" matches 26 Austrian codenames.
        let best = codenames
            .codes
            .iter()
            .filter(|(emma_id, _)| emma_id.starts_with(&country_prefix))
            .filter_map(|(emma_id, name)| {
                rank_emma_match(&search_lower, &name.to_lowercase()).map(|rank| (rank, emma_id))
            })
            // Equal ranks fall back to the EMMA_ID, which is unique, so the
            // winner is total and identical on every run. Reversed because the
            // lower ID is the one to keep.
            .max_by(|(a_rank, a_id), (b_rank, b_id)| {
                a_rank.cmp(b_rank).then_with(|| b_id.cmp(a_id))
            });

        if let Some((_, emma_id)) = best {
            tracing::debug!("Resolved EMMA_ID: {}", emma_id);
            return Some(emma_id.clone());
        }
    }

    tracing::warn!(
        "No EMMA_ID matched {:?} in {}; will try the feed's own area names",
        search_terms,
        country_prefix
    );
    None
}

/// How well one codename fits one search term, worst variant first so `max_by`
/// picks the strongest fit.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EmmaMatch {
    /// The codename is the longer string, as "Wiener Neustadt" is for a search
    /// of "Wien". The weakest kind of hit, and the shorter codename is the
    /// closer one, hence the `Reverse`.
    CodenameContainsTerm(std::cmp::Reverse<usize>),
    /// The search term is the longer string, as "Warsaw County" is for a
    /// codename of "Warsaw". A real hit on a coarser region, and here the
    /// longer codename is the more specific one.
    TermContainsCodename(usize),
    /// Same name on both sides.
    Exact,
}

/// Returns None when the two do not relate at all. Empty strings never match,
/// since `contains("")` is true for everything and would hand back an arbitrary
/// region for a location Nominatim gave a blank name.
fn rank_emma_match(search_lower: &str, name_lower: &str) -> Option<EmmaMatch> {
    if search_lower.is_empty() || name_lower.is_empty() {
        return None;
    }

    if name_lower == search_lower {
        Some(EmmaMatch::Exact)
    } else if search_lower.contains(name_lower) {
        Some(EmmaMatch::TermContainsCodename(name_lower.len()))
    } else if name_lower.contains(search_lower) {
        Some(EmmaMatch::CodenameContainsTerm(std::cmp::Reverse(
            name_lower.len(),
        )))
    } else {
        None
    }
}

/// Administrative words that name a level, not a place. Dropped from both
/// sides of an area-name compare so "Grad Zagreb" and "Zagreb region" meet.
const AREA_AFFIXES: &[&str] = &[
    "grad",
    "stadt",
    "kreis",
    "landkreis",
    "region",
    "county",
    "district",
    "city",
    "municipality",
    "powiat",
    "gmina",
    "okres",
    "kraj",
    "oblast",
];

/// Lowercase, diacritics folded, split into tokens, administrative affixes
/// dropped. "Grad Zagreb" and "Zagreb region" both become ["zagreb"].
fn area_tokens(name: &str) -> Vec<String> {
    use unicode_normalization::UnicodeNormalization;

    let folded: String = name
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase();
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty() && !AREA_AFFIXES.contains(t))
        .map(str::to_string)
        .collect()
}

/// How well one place name fits one area name, weakest first so `max` picks
/// the strongest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum AreaMatch {
    /// Every significant token of the shorter name appears in the longer one.
    Tokens,
    /// Same tokens in the same order.
    Exact,
}

/// Returns None when the two do not relate. No substring or prefix compare on
/// tokens: "seine" does not fit "seinemaritime". The shorter side must carry
/// at least one token of three or more characters, so a stray "i" or "de"
/// cannot anchor a match on its own.
fn rank_area_match(term_tokens: &[String], area_tokens: &[String]) -> Option<AreaMatch> {
    if term_tokens.is_empty() || area_tokens.is_empty() {
        return None;
    }
    if term_tokens == area_tokens {
        return Some(AreaMatch::Exact);
    }
    let (shorter, longer) = if term_tokens.len() <= area_tokens.len() {
        (term_tokens, area_tokens)
    } else {
        (area_tokens, term_tokens)
    };
    let anchored = shorter.iter().any(|t| t.chars().count() >= 3);
    if anchored && shorter.iter().all(|t| longer.contains(t)) {
        Some(AreaMatch::Tokens)
    } else {
        None
    }
}

/// The one area name the user's place names pick out of the feed, or None.
///
/// Terms run most specific first. Within a term the best rank wins, and it
/// counts only when every area at that rank is the same area, so a place name
/// that fits several regions ("Seine" against three départements) is a miss
/// for that term rather than a guess, and the next term is tried.
fn match_area(search_terms: &[String], area_names: &[String]) -> Option<String> {
    for term in search_terms {
        let term_tokens = area_tokens(term);
        let mut ranked: Vec<(AreaMatch, &str)> = area_names
            .iter()
            .filter_map(|area| {
                rank_area_match(&term_tokens, &area_tokens(area)).map(|rank| (rank, area.as_str()))
            })
            .collect();
        let Some(best) = ranked.iter().map(|(rank, _)| rank.clone()).max() else {
            continue;
        };
        ranked.retain(|(rank, _)| *rank == best);
        let mut distinct: Vec<&str> = ranked.iter().map(|(_, area)| *area).collect();
        distinct.sort_unstable();
        distinct.dedup();
        match distinct.as_slice() {
            [area] => {
                tracing::debug!("Matched area {:?} by place name {:?}", area, term);
                return Some(area.to_string());
            }
            many => tracing::debug!(
                "Place name {:?} is ambiguous across {:?}; trying the next",
                term,
                many
            ),
        }
    }
    None
}

/// Fetches active weather alerts from MeteoAlarm for European locations.
async fn fetch_meteoalarm_alerts(latitude: f64, longitude: f64) -> Result<AlertReport> {
    // Failing to determine the country is an error, not an absence of alerts.
    // Returning Ok(vec![]) here would be indistinguishable from a quiet day,
    // which is the silent-failure pattern this whole path is being fixed for.
    // The warn! stays because consumers do swallow errors.
    let address = match reverse_geocode(latitude, longitude).await {
        Ok(address) => address,
        Err(e) => {
            tracing::warn!(
                "Reverse geocoding failed ({}); cannot determine country for MeteoAlarm",
                e
            );
            return Err(e);
        }
    };

    let iso_code = match address.country_code.as_deref() {
        Some(iso_code) => iso_code,
        None => {
            tracing::warn!("Reverse geocode returned no country code; cannot select a feed");
            return Err(Error::LocationDetection);
        }
    };

    // Local-language name, for logs only; every lookup below keys on the code.
    let country = address.country.as_deref().unwrap_or(iso_code);

    // Not covered is a real absence: the country exists, MeteoAlarm has no feed
    // for it. That stays Ok(vec![]).
    let (slug, country_code) = match get_meteoalarm_info(iso_code) {
        Some(info) => info,
        None => {
            tracing::debug!("{} ({}) is not covered by MeteoAlarm", country, iso_code);
            return Ok(AlertReport {
                alerts: vec![],
                region_filtered: true,
            });
        }
    };

    let user_emma_id = resolve_user_emma_id(&address, country_code).await;

    let url = format!(
        "https://feeds.meteoalarm.org/feeds/meteoalarm-legacy-atom-{}",
        slug
    );

    let response = http_client()?.get(&url).send().await?;
    if !response.status().is_success() {
        tracing::warn!("MeteoAlarm returned status: {}", response.status());
        return Ok(AlertReport {
            alerts: vec![],
            region_filtered: user_emma_id.is_some(),
        });
    }

    let xml_text = response.text().await?;
    let feed: MeteoAlarmFeed = quick_xml::de::from_str(&xml_text)?;

    let search_terms = emma_search_terms(&address);
    Ok(meteoalarm_alerts_from_feed(
        feed,
        &user_emma_id,
        &search_terms,
        country,
    ))
}

/// The entry's EMMA_ID, if the feed tagged it with one. A geocode under any
/// other scheme (`NUTS3` in France) is not an EMMA_ID and yields `None`.
fn entry_emma_id(entry: &MeteoAlarmEntry) -> Option<&str> {
    entry
        .cap_geocode
        .as_ref()
        .filter(|gc| gc.value_name.as_deref() == Some("EMMA_ID"))
        .and_then(|gc| gc.value.as_deref())
}

/// Lives in its own function so it can be unit-tested against fixtures without a live network.
///
/// Two stages. Stage 1: a resolved EMMA_ID filters a feed that tags its
/// entries with EMMA_IDs; this is exact and, since the feed only lists regions
/// that are alerting, it is the only stage that can tell a quiet day from a
/// miss. Stage 2, when stage 1 has no usable filter: the user's place names
/// are matched against the feed's own `areaDesc` values. A hit filters to that
/// area; a miss keeps the national feed and says so.
fn meteoalarm_alerts_from_feed(
    feed: MeteoAlarmFeed,
    user_emma_id: &Option<String>,
    search_terms: &[String],
    country: &str,
) -> AlertReport {
    if feed.entries.is_empty() {
        tracing::debug!("Fetched 0 alert(s) from MeteoAlarm ({})", country);
        return AlertReport {
            alerts: vec![],
            region_filtered: true,
        };
    }

    let feed_has_emma_ids = feed
        .entries
        .iter()
        .any(|entry| entry_emma_id(entry).is_some());

    // Stage 1: EMMA_ID against an EMMA_ID-tagged feed.
    if let Some(user_id) = user_emma_id {
        if feed_has_emma_ids {
            let untagged = feed
                .entries
                .iter()
                .filter(|entry| entry_emma_id(entry).is_none())
                .count();
            if untagged > 0 {
                tracing::debug!(
                    "Dropped {} untagged MeteoAlarm entr(y/ies) while filtering to {}",
                    untagged,
                    user_id
                );
            }
            let filter = Some(user_id.clone());
            let alerts: Vec<AlertEntry> = feed
                .entries
                .into_iter()
                .filter_map(|entry| parse_meteoalarm_entry(entry, &filter))
                .collect();
            tracing::debug!(
                "Fetched {} alert(s) from MeteoAlarm ({}), filtered to {}",
                alerts.len(),
                country,
                user_id
            );
            return AlertReport {
                alerts,
                region_filtered: true,
            };
        }

        let mut schemes: Vec<&str> = feed
            .entries
            .iter()
            .filter_map(|entry| entry.cap_geocode.as_ref())
            .filter_map(|gc| gc.value_name.as_deref())
            .collect();
        schemes.sort_unstable();
        schemes.dedup();
        tracing::warn!(
            "MeteoAlarm feed ({}) carries no EMMA_ID geocodes (found {:?}); the filter to {} \
            cannot apply, matching by area name instead",
            country,
            schemes,
            user_id
        );
    }

    // Stage 2: place names against the feed's own area names.
    let mut area_names: Vec<String> = feed
        .entries
        .iter()
        .filter_map(|entry| entry.cap_area_desc.clone())
        .filter(|name| !name.is_empty())
        .collect();
    area_names.sort_unstable();
    area_names.dedup();

    match match_area(search_terms, &area_names) {
        Some(area) => {
            let alerts: Vec<AlertEntry> = feed
                .entries
                .into_iter()
                .filter(|entry| entry.cap_area_desc.as_deref() == Some(area.as_str()))
                .filter_map(|entry| parse_meteoalarm_entry(entry, &None))
                .collect();
            tracing::debug!(
                "Fetched {} alert(s) from MeteoAlarm ({}), filtered to area {:?}",
                alerts.len(),
                country,
                area
            );
            AlertReport {
                alerts,
                region_filtered: true,
            }
        }
        None => {
            let alerts: Vec<AlertEntry> = feed
                .entries
                .into_iter()
                .filter_map(|entry| parse_meteoalarm_entry(entry, &None))
                .collect();
            let shown: Vec<&str> = area_names.iter().take(10).map(String::as_str).collect();
            tracing::warn!(
                "Fetched {} alert(s) from MeteoAlarm ({}), UNFILTERED - no area name matched {:?} \
                among {} area(s) ({:?}{}); these are national alerts, not local ones",
                alerts.len(),
                country,
                search_terms,
                area_names.len(),
                shown,
                if area_names.len() > shown.len() {
                    ", ..."
                } else {
                    ""
                }
            );
            AlertReport {
                alerts,
                region_filtered: false,
            }
        }
    }
}

/// Parses a MeteoAlarm entry into an AlertEntry.
/// Returns None if it doesn't match the user's EMMA_ID or is expired.
fn parse_meteoalarm_entry(
    entry: MeteoAlarmEntry,
    user_emma_id: &Option<String>,
) -> Option<AlertEntry> {
    let now = Utc::now();

    // Filter by EMMA_ID if we resolved one for the user
    if let Some(user_id) = user_emma_id {
        match entry_emma_id(&entry) {
            Some(entry_id) if entry_id != user_id => return None,
            // An untagged entry cannot be shown to belong to the user's region;
            // when a filter is active it is dropped, not leaked past it.
            None => return None,
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

    Some(AlertEntry {
        alert: Alert {
            id: entry.cap_identifier.unwrap_or(entry.id),
            event,
            severity,
            headline,
            description: String::new(),
            expires,
        },
        area_desc: entry.cap_area_desc.unwrap_or_default(),
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
async fn fetch_eccc_alerts(latitude: f64, longitude: f64) -> Result<AlertReport> {
    let offices = get_eccc_office_codes(latitude, longitude);
    let today = chrono::Utc::now().format("%Y%m%d").to_string();
    let client = http_client()?;

    let mut all_alerts: Vec<AlertEntry> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

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
    // Every entry passed the polygon check for the caller's point.
    Ok(AlertReport {
        alerts: all_alerts,
        region_filtered: true,
    })
}

/// Parses an ECCC CAP XML document into an Alert.
/// Filters by location using polygon containment and deduplicates by identifier.
fn parse_eccc_cap(
    xml: &str,
    lat: f64,
    lon: f64,
    seen_ids: &mut HashSet<String>,
) -> Option<AlertEntry> {
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
    let area_desc = info.areas.iter().find_map(|area| {
        area.polygon
            .as_ref()
            .filter(|poly| point_in_polygon(lat, lon, poly))
            .map(|_| area.area_desc.clone().unwrap_or_default())
    });

    let area_desc = area_desc?;

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

    Some(AlertEntry {
        alert: Alert {
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
        },
        area_desc,
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
async fn fetch_bom_alerts(latitude: f64, longitude: f64) -> Result<AlertReport> {
    let geohash = encode_geohash(latitude, longitude, 6);
    let url = format!(
        "https://api.weather.bom.gov.au/v1/locations/{}/warnings",
        geohash
    );

    let response = http_client()?.get(&url).send().await?;

    if !response.status().is_success() {
        return Ok(AlertReport {
            alerts: vec![],
            region_filtered: true,
        });
    }

    let response_body: BomWarningsResponse = response.json().await?;

    let alerts = bom_alerts_from_response(response_body.data);

    // The geohash lookup already narrowed the list to the caller's location.
    Ok(AlertReport {
        alerts,
        region_filtered: true,
    })
}

/// Lives in its own function so it can be unit-tested against fixtures without a live network.
fn bom_alerts_from_response(data: Vec<BomWarning>) -> Vec<AlertEntry> {
    let now = Utc::now();

    data.into_iter()
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

            Some(AlertEntry {
                alert: Alert {
                    id: w.id.clone(),
                    event,
                    severity,
                    headline,
                    description: String::new(),
                    expires,
                },
                // BOM's warnings endpoint carries no area name.
                area_desc: String::new(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------
    // NWS
    // -----------------------------------------------------------------

    #[test]
    fn nws_decodes_active_alert() {
        let json = r#"{"features":[{"properties":{
            "id":"NWS-IDP-PROD-123",
            "event":"Tornado Warning",
            "severity":"Severe",
            "headline":"Tornado Warning until 8 PM",
            "description":"Take cover now.",
            "sent":"2026-06-01T12:00:00Z",
            "expires":"2099-01-01T00:00:00Z"
        }}]}"#;
        let data: NwsAlertsResponse = serde_json::from_str(json).unwrap();
        let alerts = nws_alerts_from_response(data);

        assert_eq!(alerts.len(), 1);
        let alert = &alerts[0].alert;
        assert_eq!(alert.id, "NWS-IDP-PROD-123");
        assert_eq!(alert.event, "Tornado Warning");
        assert_eq!(alert.severity, AlertSeverity::Severe);
        assert_eq!(alert.headline, "Tornado Warning until 8 PM");
    }

    #[test]
    fn nws_drops_expired_alert() {
        let json = r#"{"features":[{"properties":{
            "id":"NWS-IDP-PROD-124",
            "event":"Winter Storm Warning",
            "severity":"Severe",
            "headline":"Winter Storm Warning",
            "description":"Snow expected.",
            "sent":"2026-06-01T12:00:00Z",
            "expires":"2020-01-01T01:00:00Z"
        }}]}"#;
        let data: NwsAlertsResponse = serde_json::from_str(json).unwrap();
        let alerts = nws_alerts_from_response(data);

        assert!(alerts.is_empty());
    }

    #[test]
    fn nws_null_severity_falls_back_to_unknown() {
        let json = r#"{"features":[{"properties":{
            "id":"NWS-IDP-PROD-125",
            "event":"Special Weather Statement",
            "severity":null,
            "headline":"Special Weather Statement",
            "description":"Details.",
            "sent":"2026-06-01T12:00:00Z",
            "expires":"2099-01-01T00:00:00Z"
        }}]}"#;
        let data: NwsAlertsResponse = serde_json::from_str(json).unwrap();
        let alerts = nws_alerts_from_response(data);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.severity, AlertSeverity::Unknown);
    }

    #[test]
    fn nws_null_expires_uses_sent_plus_24h() {
        let json = r#"{"features":[{"properties":{
            "id":"NWS-IDP-PROD-126",
            "event":"Flood Watch",
            "severity":"Moderate",
            "headline":"Flood Watch",
            "description":"Details.",
            "sent":"2099-01-01T00:00:00Z",
            "expires":null
        }}]}"#;
        let data: NwsAlertsResponse = serde_json::from_str(json).unwrap();
        let alerts = nws_alerts_from_response(data);

        assert_eq!(alerts.len(), 1);
    }

    #[test]
    fn nws_null_headline_and_description_default_to_empty() {
        let json = r#"{"features":[{"properties":{
            "id":"NWS-IDP-PROD-127",
            "event":"Wind Advisory",
            "severity":"Minor",
            "headline":null,
            "description":null,
            "sent":"2026-06-01T12:00:00Z",
            "expires":"2099-01-01T00:00:00Z"
        }}]}"#;
        let data: NwsAlertsResponse = serde_json::from_str(json).unwrap();
        let alerts = nws_alerts_from_response(data);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.headline, "");
        assert_eq!(alerts[0].alert.description, "");
    }

    // -----------------------------------------------------------------
    // AlertSeverity::from_cap_string
    // -----------------------------------------------------------------

    #[test]
    fn from_cap_string_maps_all_classes() {
        assert_eq!(
            AlertSeverity::from_cap_string("minor"),
            AlertSeverity::Minor
        );
        assert_eq!(
            AlertSeverity::from_cap_string("moderate"),
            AlertSeverity::Moderate
        );
        assert_eq!(
            AlertSeverity::from_cap_string("severe"),
            AlertSeverity::Severe
        );
        assert_eq!(
            AlertSeverity::from_cap_string("major"),
            AlertSeverity::Severe
        );
        assert_eq!(
            AlertSeverity::from_cap_string("extreme"),
            AlertSeverity::Extreme
        );
        assert_eq!(
            AlertSeverity::from_cap_string("not-a-severity"),
            AlertSeverity::Unknown
        );
    }

    // -----------------------------------------------------------------
    // BOM
    // -----------------------------------------------------------------

    #[test]
    fn bom_decodes_active_severe_warning() {
        let json = r#"{"data":[{
            "id":"bom-1",
            "type":"severe_thunderstorm",
            "short_title":"Severe Thunderstorm Warning",
            "warning_group_type":"severe",
            "phase":"active",
            "expiry_time":"2099-01-01T00:00:00Z"
        }]}"#;
        let resp: BomWarningsResponse = serde_json::from_str(json).unwrap();
        let alerts = bom_alerts_from_response(resp.data);

        assert_eq!(alerts.len(), 1);
        let alert = &alerts[0].alert;
        assert_eq!(alert.severity, AlertSeverity::Severe);
        assert_eq!(alert.event, "severe thunderstorm");
        assert_eq!(alert.headline, "Severe Thunderstorm Warning");
    }

    #[test]
    fn bom_major_group_type_maps_to_severe() {
        let json = r#"{"data":[{
            "id":"bom-1b",
            "type":"severe_thunderstorm",
            "short_title":"Severe Thunderstorm Warning",
            "warning_group_type":"major",
            "phase":"active",
            "expiry_time":"2099-01-01T00:00:00Z"
        }]}"#;
        let resp: BomWarningsResponse = serde_json::from_str(json).unwrap();
        let alerts = bom_alerts_from_response(resp.data);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.severity, AlertSeverity::Severe);
    }

    #[test]
    fn bom_drops_cancelled_phase() {
        let json = r#"{"data":[{
            "id":"bom-1c",
            "type":"severe_thunderstorm",
            "short_title":"Severe Thunderstorm Warning",
            "warning_group_type":"severe",
            "phase":"cancelled",
            "expiry_time":"2099-01-01T00:00:00Z"
        }]}"#;
        let resp: BomWarningsResponse = serde_json::from_str(json).unwrap();
        let alerts = bom_alerts_from_response(resp.data);

        assert!(alerts.is_empty());
    }

    #[test]
    fn bom_drops_expired_warning() {
        let json = r#"{"data":[{
            "id":"bom-1d",
            "type":"severe_thunderstorm",
            "short_title":"Severe Thunderstorm Warning",
            "warning_group_type":"severe",
            "phase":"active",
            "expiry_time":"2020-01-01T00:00:00Z"
        }]}"#;
        let resp: BomWarningsResponse = serde_json::from_str(json).unwrap();
        let alerts = bom_alerts_from_response(resp.data);

        assert!(alerts.is_empty());
    }

    #[test]
    fn bom_missing_short_title_uses_default_headline() {
        let json = r#"{"data":[{
            "id":"bom-2",
            "type":"flood",
            "short_title":null,
            "warning_group_type":"moderate",
            "phase":"active",
            "expiry_time":"2099-01-01T00:00:00Z"
        }]}"#;
        let resp: BomWarningsResponse = serde_json::from_str(json).unwrap();
        let alerts = bom_alerts_from_response(resp.data);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.headline, "Weather Warning");
        assert_eq!(alerts[0].alert.event, "flood");
    }

    #[test]
    fn bom_missing_warning_type_uses_headline_as_event() {
        let json = r#"{"data":[{
            "id":"bom-3",
            "type":null,
            "short_title":"Severe Weather Alert",
            "warning_group_type":"severe",
            "phase":"active",
            "expiry_time":"2099-01-01T00:00:00Z"
        }]}"#;
        let resp: BomWarningsResponse = serde_json::from_str(json).unwrap();
        let alerts = bom_alerts_from_response(resp.data);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.event, "Severe Weather Alert");
        assert_eq!(alerts[0].alert.headline, "Severe Weather Alert");
    }

    // -----------------------------------------------------------------
    // MeteoAlarm
    // -----------------------------------------------------------------
    //
    // MeteoAlarm namespace binding (Assumptions Log A1, REVIEWS.md finding 3):
    // this crate's quick-xml 0.37 (features = ["serialize"]) matches serde
    // `rename` targets by LOCAL tag name, ignoring namespace prefixes. A
    // fixture using the real feed's `cap:identifier`, `cap:event`, etc.
    // prefixes decodes identically to an unprefixed fixture -- confirmed by
    // `meteoalarm_entry_decodes_all_fields` below, which uses the prefixed
    // form and asserts every field against its exact expected value (not
    // merely "did not panic"). This is the observed working form; the
    // fixture and test names in this module are written against it.

    #[test]
    fn meteoalarm_entry_decodes_all_fields() {
        let xml = r#"<entry>
            <id>https://feeds.meteoalarm.org/feed/example-entry-1</id>
            <title>Wind Warning for Test Region</title>
            <cap:identifier>2-717000-DE723</cap:identifier>
            <cap:areaDesc>Test Region</cap:areaDesc>
            <cap:event>Wind</cap:event>
            <cap:severity>Severe</cap:severity>
            <cap:sent>2026-06-01T08:00:00Z</cap:sent>
            <cap:expires>2099-01-01T00:00:00Z</cap:expires>
            <cap:geocode>
                <valueName>EMMA_ID</valueName>
                <cap:value>DE723</cap:value>
            </cap:geocode>
        </entry>"#;
        let entry: MeteoAlarmEntry = quick_xml::de::from_str(xml).unwrap();
        let entry = parse_meteoalarm_entry(entry, &None).expect("entry should decode to an alert");
        assert_eq!(entry.area_desc, "Test Region");
        let alert = entry.alert;

        assert_eq!(alert.id, "2-717000-DE723");
        assert_eq!(alert.event, "Wind");
        assert_eq!(alert.severity, AlertSeverity::Severe);
        assert_eq!(alert.headline, "Wind Warning for Test Region");
    }

    #[test]
    fn meteoalarm_entry_matches_user_emma_id() {
        let xml = r#"<entry>
            <id>https://feeds.meteoalarm.org/feed/example-entry-2</id>
            <title>Wind Warning for Test Region</title>
            <cap:identifier>2-717000-DE723</cap:identifier>
            <cap:event>Wind</cap:event>
            <cap:severity>Severe</cap:severity>
            <cap:sent>2026-06-01T08:00:00Z</cap:sent>
            <cap:expires>2099-01-01T00:00:00Z</cap:expires>
            <cap:geocode>
                <valueName>EMMA_ID</valueName>
                <cap:value>DE723</cap:value>
            </cap:geocode>
        </entry>"#;
        let entry: MeteoAlarmEntry = quick_xml::de::from_str(xml).unwrap();
        let alert = parse_meteoalarm_entry(entry, &Some("DE723".to_string()));

        assert!(alert.is_some());
        let alert = alert.unwrap().alert;
        assert_eq!(alert.event, "Wind");
        assert_eq!(alert.severity, AlertSeverity::Severe);
    }

    #[test]
    fn meteoalarm_entry_filters_wrong_emma_id() {
        let xml = r#"<entry>
            <id>https://feeds.meteoalarm.org/feed/example-entry-3</id>
            <title>Wind Warning for Test Region</title>
            <cap:identifier>2-717000-DE723</cap:identifier>
            <cap:event>Wind</cap:event>
            <cap:severity>Severe</cap:severity>
            <cap:sent>2026-06-01T08:00:00Z</cap:sent>
            <cap:expires>2099-01-01T00:00:00Z</cap:expires>
            <cap:geocode>
                <valueName>EMMA_ID</valueName>
                <cap:value>DE723</cap:value>
            </cap:geocode>
        </entry>"#;
        let entry: MeteoAlarmEntry = quick_xml::de::from_str(xml).unwrap();
        let alert = parse_meteoalarm_entry(entry, &Some("DE999".to_string()));

        assert!(alert.is_none());
    }

    #[test]
    fn meteoalarm_entry_drops_expired() {
        let xml = r#"<entry>
            <id>https://feeds.meteoalarm.org/feed/example-entry-4</id>
            <title>Wind Warning for Test Region</title>
            <cap:identifier>2-717000-DE723</cap:identifier>
            <cap:event>Wind</cap:event>
            <cap:severity>Severe</cap:severity>
            <cap:sent>2020-01-01T00:00:00Z</cap:sent>
            <cap:expires>2020-01-01T00:00:00Z</cap:expires>
            <cap:geocode>
                <valueName>EMMA_ID</valueName>
                <cap:value>DE723</cap:value>
            </cap:geocode>
        </entry>"#;
        let entry: MeteoAlarmEntry = quick_xml::de::from_str(xml).unwrap();
        let alert = parse_meteoalarm_entry(entry, &None);

        assert!(alert.is_none());
    }

    #[test]
    fn meteoalarm_feed_decodes_and_maps() {
        let xml = r#"<feed>
            <entry>
                <id>https://feeds.meteoalarm.org/feed/example-entry-future</id>
                <title>Wind Warning for Test Region (future)</title>
                <cap:identifier>2-717000-DE723-future</cap:identifier>
                <cap:event>Wind</cap:event>
                <cap:severity>Severe</cap:severity>
                <cap:sent>2026-06-01T08:00:00Z</cap:sent>
                <cap:expires>2099-01-01T00:00:00Z</cap:expires>
                <cap:geocode>
                    <valueName>EMMA_ID</valueName>
                    <cap:value>DE723</cap:value>
                </cap:geocode>
            </entry>
            <entry>
                <id>https://feeds.meteoalarm.org/feed/example-entry-past</id>
                <title>Wind Warning for Test Region (past)</title>
                <cap:identifier>2-717000-DE723-past</cap:identifier>
                <cap:event>Wind</cap:event>
                <cap:severity>Severe</cap:severity>
                <cap:sent>2020-01-01T00:00:00Z</cap:sent>
                <cap:expires>2020-01-01T00:00:00Z</cap:expires>
                <cap:geocode>
                    <valueName>EMMA_ID</valueName>
                    <cap:value>DE723</cap:value>
                </cap:geocode>
            </entry>
        </feed>"#;
        let feed: MeteoAlarmFeed = quick_xml::de::from_str(xml).unwrap();
        let alerts = meteoalarm_alerts_from_feed(feed, &None, &[], "test").alerts;

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.id, "2-717000-DE723-future");
    }

    // -----------------------------------------------------------------
    // ECCC
    // -----------------------------------------------------------------
    //
    // Fixtures reuse the exact test square from geo.rs's point_in_polygon_square
    // ("0,0 10,0 10,10 0,10") and its inside point (5.0, 5.0).

    fn eccc_fixture(status: &str, msg_type: &str, sent: &str, identifier: &str) -> String {
        format!(
            r#"<alert>
                <identifier>{identifier}</identifier>
                <status>{status}</status>
                <msgType>{msg_type}</msgType>
                <sent>{sent}</sent>
                <info>
                    <language>en-CA</language>
                    <event>Thunderstorm Warning</event>
                    <severity>Severe</severity>
                    <expires>2099-01-01T00:00:00Z</expires>
                    <headline>Severe Thunderstorm Warning</headline>
                    <description>Severe thunderstorm expected.</description>
                    <area>
                        <areaDesc>Test Region</areaDesc>
                        <polygon>0,0 10,0 10,10 0,10</polygon>
                    </area>
                </info>
            </alert>"#
        )
    }

    #[test]
    fn eccc_decodes_active_alert_in_polygon() {
        let xml = eccc_fixture(
            "Actual",
            "Alert",
            "2026-06-01T08:00:00-04:00",
            "CA-ON-2026-001",
        );
        let mut seen_ids = HashSet::new();
        let alert = parse_eccc_cap(&xml, 5.0, 5.0, &mut seen_ids);

        assert!(alert.is_some());
        let alert = alert.unwrap().alert;
        assert_eq!(alert.event, "Thunderstorm Warning");
        assert_eq!(alert.severity, AlertSeverity::Severe);
        assert_eq!(alert.id, "CA-ON-2026-001");
    }

    #[test]
    fn eccc_rejects_point_outside_polygon() {
        let xml = eccc_fixture(
            "Actual",
            "Alert",
            "2026-06-01T08:00:00-04:00",
            "CA-ON-2026-002",
        );
        let mut seen_ids = HashSet::new();
        let alert = parse_eccc_cap(&xml, 50.0, 50.0, &mut seen_ids);

        assert!(alert.is_none());
    }

    #[test]
    fn eccc_rejects_non_actual_status() {
        let xml = eccc_fixture(
            "Test",
            "Alert",
            "2026-06-01T08:00:00-04:00",
            "CA-ON-2026-003",
        );
        let mut seen_ids = HashSet::new();
        let alert = parse_eccc_cap(&xml, 5.0, 5.0, &mut seen_ids);

        assert!(alert.is_none());
    }

    #[test]
    fn eccc_rejects_cancel_msgtype() {
        let xml = eccc_fixture(
            "Actual",
            "Cancel",
            "2026-06-01T08:00:00-04:00",
            "CA-ON-2026-004",
        );
        let mut seen_ids = HashSet::new();
        let alert = parse_eccc_cap(&xml, 5.0, 5.0, &mut seen_ids);

        assert!(alert.is_none());
    }

    #[test]
    fn eccc_dedups_same_event_and_area() {
        let xml_first = eccc_fixture(
            "Actual",
            "Alert",
            "2026-06-01T08:00:00-04:00",
            "CA-ON-2026-005",
        );
        let xml_second = eccc_fixture(
            "Actual",
            "Alert",
            "2026-06-01T09:00:00-04:00",
            "CA-ON-2026-006",
        );
        let mut seen_ids = HashSet::new();

        let first = parse_eccc_cap(&xml_first, 5.0, 5.0, &mut seen_ids);
        assert!(first.is_some());

        let second = parse_eccc_cap(&xml_second, 5.0, 5.0, &mut seen_ids);
        assert!(second.is_none());
    }

    #[test]
    fn eccc_drops_expired_alert() {
        let xml = r#"<alert>
                <identifier>CA-ON-2026-007</identifier>
                <status>Actual</status>
                <msgType>Alert</msgType>
                <sent>2020-06-01T08:00:00-04:00</sent>
                <info>
                    <language>en-CA</language>
                    <event>Thunderstorm Warning</event>
                    <severity>Severe</severity>
                    <expires>2020-01-01T00:00:00Z</expires>
                    <headline>Severe Thunderstorm Warning</headline>
                    <description>Severe thunderstorm expected.</description>
                    <area>
                        <areaDesc>Test Region</areaDesc>
                        <polygon>0,0 10,0 10,10 0,10</polygon>
                    </area>
                </info>
            </alert>"#
            .to_string();
        let mut seen_ids = HashSet::new();
        let alert = parse_eccc_cap(&xml, 5.0, 5.0, &mut seen_ids);

        assert!(alert.is_none());
    }

    // -----------------------------------------------------------------
    // Region dispatch / routing
    // -----------------------------------------------------------------
    //
    // Only the Region::Unknown arm of fetch_alerts is offline-provable (it
    // returns Ok(vec![]) without touching the network). The Us/Europe/Canada/
    // Australia arms invoke live-HTTP fetch_* functions and remain an
    // accepted, documented coverage gap: no injectable base URL exists in
    // client.rs, and adding one is out of scope per REQUIREMENTS.md. The
    // coordinate->Region routing DECISION that feeds those arms is fully
    // proven below via detect_region -- this proves ROUTING, not live-arm
    // DISPATCH (per REVIEWS.md finding 2).

    #[tokio::test]
    async fn dispatch_unknown_region_returns_empty() {
        // Tokyo -> Region::Unknown (proven in geo.rs's
        // detect_region_unknown_outside_coverage test). This branch touches
        // no network.
        let result = fetch_alerts(35.68, 139.65).await;
        assert!(matches!(result, Ok(alerts) if alerts.is_empty()));
    }

    #[test]
    fn dispatch_routes_coordinates_to_expected_region() {
        // One representative coordinate per provider, reusing geo.rs's
        // existing test coordinates. Each Region maps 1:1 to a fetch_* arm
        // in fetch_alerts (Us -> fetch_nws_alerts, Europe -> fetch_meteoalarm_alerts,
        // Canada -> fetch_eccc_alerts, Australia -> fetch_bom_alerts). The
        // Us/Europe/Canada/Australia arms invoke live-HTTP fetch_* and REMAIN
        // UNCOVERED by this test suite -- this is the accepted, documented
        // gap referenced above and in REVIEWS.md finding 2.
        assert_eq!(detect_region(40.71, -74.01), Region::Us, "New York");
        assert_eq!(detect_region(43.65, -79.38), Region::Canada, "Toronto");
        assert_eq!(detect_region(51.51, -0.13), Region::Europe, "London");
        assert_eq!(detect_region(-33.87, 151.21), Region::Australia, "Sydney");
        assert_eq!(detect_region(35.68, 139.65), Region::Unknown, "Tokyo");
    }

    fn address(city: Option<&str>, county: Option<&str>, state: Option<&str>) -> NominatimAddress {
        NominatimAddress {
            country: Some("Polska".to_string()),
            country_code: Some("pl".to_string()),
            city: city.map(str::to_string),
            town: None,
            village: None,
            municipality: None,
            county: county.map(str::to_string),
            state: state.map(str::to_string),
        }
    }

    fn codenames(pairs: &[(&str, &str)]) -> MeteoAlarmCodenames {
        MeteoAlarmCodenames {
            codes: pairs
                .iter()
                .map(|(id, name)| (id.to_string(), name.to_string()))
                .collect(),
        }
    }

    #[test]
    fn emma_search_terms_are_most_specific_first() {
        let terms = emma_search_terms(&address(
            Some("Warsaw"),
            Some("Warsaw County"),
            Some("Masovian"),
        ));
        assert_eq!(
            terms,
            vec![
                "Warsaw",
                "Stadt Warsaw",
                "Warsaw County",
                "Kreis Warsaw County",
                "Masovian",
            ]
        );
    }

    #[test]
    fn match_emma_id_ignores_other_countries() {
        // A German codename must not match a Polish query, which is the prefix
        // check that the old bounding-box country guess kept getting wrong.
        let codes = codenames(&[("DE123", "Warsaw")]);
        assert_eq!(
            match_emma_id(&address(Some("Warsaw"), None, None), "PL", &codes),
            None
        );
    }

    #[test]
    fn match_emma_id_matches_on_city() {
        let codes = codenames(&[("PL1465", "Warsaw"), ("DE123", "Berlin")]);
        assert_eq!(
            match_emma_id(&address(Some("Warsaw"), None, None), "PL", &codes),
            Some("PL1465".to_string())
        );
    }

    #[test]
    fn match_emma_id_falls_back_to_state() {
        let codes = codenames(&[("PL0100", "Masovian")]);
        assert_eq!(
            match_emma_id(
                &address(Some("Nowhere"), None, Some("Masovian")),
                "PL",
                &codes
            ),
            Some("PL0100".to_string())
        );
    }

    #[test]
    fn match_emma_id_returns_none_when_nothing_matches() {
        let codes = codenames(&[("PL1465", "Warsaw")]);
        assert_eq!(
            match_emma_id(&address(Some("Nowhere"), None, None), "PL", &codes),
            None
        );
    }

    #[test]
    fn emma_search_terms_includes_town() {
        // Nominatim returns `town` instead of `city` for smaller places, so the
        // city-less path has to produce terms too.
        let address = NominatimAddress {
            country: Some("Polska".to_string()),
            country_code: Some("pl".to_string()),
            city: None,
            town: Some("Sopot".to_string()),
            village: None,
            municipality: None,
            county: None,
            state: None,
        };
        assert_eq!(emma_search_terms(&address), vec!["Sopot"]);
    }

    #[test]
    fn match_emma_id_matches_real_local_language_pair() {
        // Regression guard. MeteoAlarm's PL1465 is "Warszawa"; if Nominatim is
        // ever asked for English names it answers "Warsaw", this stops matching,
        // and the whole Polish feed renders unfiltered. Every other fixture here
        // is English on both sides, which is what let that slip through green.
        let codes = codenames(&[("PL1465", "Warszawa")]);
        assert_eq!(
            match_emma_id(&address(Some("Warszawa"), None, None), "PL", &codes),
            Some("PL1465".to_string())
        );
    }

    /// Vienna's real shape in the MeteoAlarm list: the city itself, a
    /// same-prefix neighbour, and the numbered districts. "Wien" matches all of
    /// them, and only AT010 is right.
    fn vienna_codenames() -> MeteoAlarmCodenames {
        let mut pairs = vec![
            ("AT010", "Wien"),
            ("AT304", "Wiener Neustadt (Stadt)"),
            ("AT323", "Wiener Neustadt (Land)"),
        ];
        let districts: Vec<String> = (901..=923).map(|n| format!("AT{n}")).collect();
        for id in &districts {
            pairs.push((id.as_str(), "Wien Innere Stadt"));
        }
        codenames(&pairs)
    }

    fn vienna_address() -> NominatimAddress {
        NominatimAddress {
            country: Some("Österreich".to_string()),
            country_code: Some("at".to_string()),
            city: Some("Wien".to_string()),
            town: None,
            village: None,
            municipality: None,
            county: None,
            state: Some("Wien".to_string()),
        }
    }

    #[test]
    fn match_emma_id_prefers_the_exact_codename_over_longer_ones() {
        assert_eq!(
            match_emma_id(&vienna_address(), "AT", &vienna_codenames()),
            Some("AT010".to_string())
        );
    }

    #[test]
    fn match_emma_id_is_stable_across_hashmap_instances() {
        // Regression guard for the ambiguity itself. `codes` is a HashMap, so a
        // first-hit-wins loop returned a different one of these 26 Austrian
        // codenames per run. A fresh map each round is what a fresh fetch
        // builds, and RandomState reseeds every instance.
        let resolved: std::collections::BTreeSet<String> = (0..200)
            .filter_map(|_| match_emma_id(&vienna_address(), "AT", &vienna_codenames()))
            .collect();
        assert_eq!(
            resolved,
            ["AT010".to_string()].into_iter().collect(),
            "one input must resolve to exactly one EMMA_ID"
        );
    }

    #[test]
    fn match_emma_id_prefers_the_longest_codename_the_term_contains() {
        // Both are real regions containing the search term. "Rhone-Alpes" is the
        // more specific of the two, so a search for "Auvergne-Rhone-Alpes"
        // should not settle for "Rhone".
        let codes = codenames(&[("FR001", "Rhone"), ("FR002", "Rhone-Alpes")]);
        let address = NominatimAddress {
            country: Some("France".to_string()),
            country_code: Some("fr".to_string()),
            city: None,
            town: None,
            village: None,
            municipality: None,
            county: None,
            state: Some("Auvergne-Rhone-Alpes".to_string()),
        };
        assert_eq!(
            match_emma_id(&address, "FR", &codes),
            Some("FR002".to_string())
        );
    }

    #[test]
    fn match_emma_id_ties_break_on_the_lower_id() {
        // Two codenames, same name, same rank. Nothing distinguishes them but
        // the ID, and the answer still has to be the same every run.
        let codes = codenames(&[("PL2000", "Warszawa"), ("PL1465", "Warszawa")]);
        for _ in 0..50 {
            assert_eq!(
                match_emma_id(&address(Some("Warszawa"), None, None), "PL", &codes),
                Some("PL1465".to_string())
            );
        }
    }

    #[test]
    fn match_emma_id_ignores_blank_place_names() {
        // Nominatim can answer with an empty string, and `contains("")` is true
        // for every codename, which would hand back an arbitrary region.
        let codes = codenames(&[("PL1465", "Warszawa")]);
        assert_eq!(
            match_emma_id(&address(Some(""), None, None), "PL", &codes),
            None
        );
    }

    #[test]
    fn match_emma_id_search_term_order_still_wins_over_rank() {
        // The city is checked before the state, so a weak city hit beats a
        // perfect state hit. Ranking is only a tie-break inside one term.
        let codes = codenames(&[("PL1465", "Warszawa Centrum"), ("PL0100", "Masovian")]);
        assert_eq!(
            match_emma_id(
                &address(Some("Warszawa"), None, Some("Masovian")),
                "PL",
                &codes
            ),
            Some("PL1465".to_string())
        );
    }

    #[test]
    fn match_emma_id_matches_when_search_term_contains_codename() {
        // Nominatim's county ("Warsaw County") is longer than the codename
        // ("Warsaw"), so the containment runs the other direction.
        let codes = codenames(&[("PL1465", "Warsaw")]);
        assert_eq!(
            match_emma_id(&address(None, Some("Warsaw County"), None), "PL", &codes),
            Some("PL1465".to_string())
        );
    }

    // -----------------------------------------------------------------
    // area_desc and AlertReport
    // -----------------------------------------------------------------

    #[test]
    fn meteoalarm_entry_without_area_desc_is_empty_string() {
        let xml = r#"<entry>
            <id>https://feeds.meteoalarm.org/feed/example-entry-5</id>
            <title>Wind Warning</title>
            <cap:event>Wind</cap:event>
            <cap:sent>2026-06-01T08:00:00Z</cap:sent>
            <cap:expires>2099-01-01T00:00:00Z</cap:expires>
        </entry>"#;
        let entry: MeteoAlarmEntry = quick_xml::de::from_str(xml).unwrap();
        let entry = parse_meteoalarm_entry(entry, &None).unwrap();

        assert_eq!(entry.area_desc, "");
    }

    /// Two entries: one tagged with a region that is not the user's, one with
    /// no geocode at all. With a filter active both must be dropped; before
    /// the `None` arm existed, the untagged one leaked through.
    fn untagged_feed() -> MeteoAlarmFeed {
        let xml = r#"<feed>
            <entry>
                <id>https://feeds.meteoalarm.org/feed/tagged-elsewhere</id>
                <title>Wind Warning for elsewhere</title>
                <cap:event>Wind</cap:event>
                <cap:severity>Moderate</cap:severity>
                <cap:sent>2026-06-01T08:00:00Z</cap:sent>
                <cap:expires>2099-01-01T00:00:00Z</cap:expires>
                <cap:geocode>
                    <valueName>EMMA_ID</valueName>
                    <cap:value>PL999</cap:value>
                </cap:geocode>
            </entry>
            <entry>
                <id>https://feeds.meteoalarm.org/feed/untagged</id>
                <title>Wind Warning with no geocode</title>
                <cap:event>Wind</cap:event>
                <cap:severity>Moderate</cap:severity>
                <cap:sent>2026-06-01T08:00:00Z</cap:sent>
                <cap:expires>2099-01-01T00:00:00Z</cap:expires>
            </entry>
        </feed>"#;
        quick_xml::de::from_str(xml).unwrap()
    }

    #[test]
    fn meteoalarm_untagged_entry_dropped_when_filter_active() {
        let alerts =
            meteoalarm_alerts_from_feed(untagged_feed(), &Some("PL1465".to_string()), &[], "test")
                .alerts;

        assert!(alerts.is_empty(), "untagged entry leaked past the filter");
    }

    #[test]
    fn meteoalarm_untagged_entry_kept_without_filter() {
        let alerts = meteoalarm_alerts_from_feed(untagged_feed(), &None, &[], "test").alerts;

        assert_eq!(alerts.len(), 2);
    }

    #[test]
    fn nws_decodes_area_desc() {
        let json = r#"{"features":[{"properties":{
            "id":"NWS-IDP-PROD-128",
            "event":"Heat Advisory",
            "severity":"Moderate",
            "headline":"Heat Advisory until 8 PM",
            "description":"Hot.",
            "sent":"2026-06-01T12:00:00Z",
            "expires":"2099-01-01T00:00:00Z",
            "areaDesc":"Coastal Los Angeles County; Los Angeles County Beaches"
        }}]}"#;
        let data: NwsAlertsResponse = serde_json::from_str(json).unwrap();
        let alerts = nws_alerts_from_response(data);

        assert_eq!(
            alerts[0].area_desc,
            "Coastal Los Angeles County; Los Angeles County Beaches"
        );
    }

    #[test]
    fn nws_missing_area_desc_is_empty_string() {
        let json = r#"{"features":[{"properties":{
            "id":"NWS-IDP-PROD-129",
            "event":"Heat Advisory",
            "severity":"Moderate",
            "headline":"Heat Advisory",
            "description":"Hot.",
            "sent":"2026-06-01T12:00:00Z",
            "expires":"2099-01-01T00:00:00Z"
        }}]}"#;
        let data: NwsAlertsResponse = serde_json::from_str(json).unwrap();
        let alerts = nws_alerts_from_response(data);

        assert_eq!(alerts[0].area_desc, "");
    }

    #[test]
    fn eccc_area_desc_is_the_containing_polygon() {
        let xml = eccc_fixture(
            "Actual",
            "Alert",
            "2026-06-01T08:00:00-04:00",
            "CA-ON-2026-008",
        );
        let mut seen_ids = HashSet::new();
        let entry = parse_eccc_cap(&xml, 5.0, 5.0, &mut seen_ids).unwrap();

        assert_eq!(entry.area_desc, "Test Region");
    }

    #[test]
    fn bom_area_desc_is_empty_string() {
        let json = r#"{"data":[{
            "id":"bom-4",
            "type":"flood",
            "short_title":"Flood Warning",
            "warning_group_type":"moderate",
            "phase":"active",
            "expiry_time":"2099-01-01T00:00:00Z"
        }]}"#;
        let resp: BomWarningsResponse = serde_json::from_str(json).unwrap();
        let alerts = bom_alerts_from_response(resp.data);

        assert_eq!(alerts[0].area_desc, "");
    }

    #[tokio::test]
    async fn dispatch_unknown_region_detailed_is_empty_and_filtered() {
        // Same Tokyo coordinate as dispatch_unknown_region_returns_empty.
        let report = fetch_alerts_detailed(35.68, 139.65).await.unwrap();

        assert!(report.alerts.is_empty());
        assert!(report.region_filtered);
    }

    // -----------------------------------------------------------------
    // Feeds that are not EMMA_ID-tagged (France uses NUTS3)
    // -----------------------------------------------------------------

    /// The shape of every entry in the live French feed on 2026-09-02: a NUTS3
    /// geocode, with the EMMA_ID only in a link href that is not parsed.
    fn nuts3_entry(id: &str, nuts3: &str, area: &str) -> String {
        format!(
            r#"<entry>
                <id>https://feeds.meteoalarm.org/feed/{id}</id>
                <cap:geocode>
                    <valueName>NUTS3</valueName>
                    <value>{nuts3}</value>
                </cap:geocode>
                <link title="{area}" href="https://meteoalarm.org?geocode=EMMA_ID:FR031" hreflang="en"/>
                <cap:areaDesc>{area}</cap:areaDesc>
                <cap:event>Yellow Wind Warning</cap:event>
                <cap:severity>Moderate</cap:severity>
                <cap:sent>2026-06-01T08:00:00Z</cap:sent>
                <cap:expires>2099-01-01T00:00:00Z</cap:expires>
            </entry>"#
        )
    }

    #[test]
    fn meteoalarm_nuts3_geocode_is_not_an_emma_id() {
        let entry: MeteoAlarmEntry =
            quick_xml::de::from_str(&nuts3_entry("fr-1", "FR713", "Drôme")).unwrap();

        assert_eq!(entry_emma_id(&entry), None);
        assert_eq!(entry.cap_geocode.unwrap().value.as_deref(), Some("FR713"));
    }

    #[test]
    fn meteoalarm_feed_without_emma_ids_renders_unfiltered() {
        let xml = format!(
            "<feed>{}{}</feed>",
            nuts3_entry("fr-1", "FR713", "Drôme"),
            nuts3_entry("fr-2", "FR813", "Hérault")
        );
        let feed: MeteoAlarmFeed = quick_xml::de::from_str(&xml).unwrap();
        let report = meteoalarm_alerts_from_feed(feed, &Some("FR101".to_string()), &[], "test");

        // The filter cannot apply to a feed that never carries an EMMA_ID, so
        // nothing is dropped and the report says the list is national.
        assert_eq!(report.alerts.len(), 2);
        assert!(!report.region_filtered);
        assert_eq!(report.alerts[0].area_desc, "Drôme");
    }

    #[test]
    fn meteoalarm_mixed_feed_drops_untagged_and_stays_filtered() {
        let xml = format!(
            r#"<feed>
                <entry>
                    <id>https://feeds.meteoalarm.org/feed/tagged-here</id>
                    <cap:event>Wind</cap:event>
                    <cap:sent>2026-06-01T08:00:00Z</cap:sent>
                    <cap:expires>2099-01-01T00:00:00Z</cap:expires>
                    <cap:geocode>
                        <valueName>EMMA_ID</valueName>
                        <value>FR101</value>
                    </cap:geocode>
                </entry>
                {}
            </feed>"#,
            nuts3_entry("fr-3", "FR713", "Drôme")
        );
        let feed: MeteoAlarmFeed = quick_xml::de::from_str(&xml).unwrap();
        let report = meteoalarm_alerts_from_feed(feed, &Some("FR101".to_string()), &[], "test");

        // One entry carries an EMMA_ID, so the feed is filterable: the NUTS3
        // entry is untagged for this purpose and is dropped.
        assert_eq!(report.alerts.len(), 1);
        assert_eq!(
            report.alerts[0].alert.id,
            "https://feeds.meteoalarm.org/feed/tagged-here"
        );
        assert!(report.region_filtered);
    }

    #[test]
    fn meteoalarm_empty_feed_with_filter_is_filtered() {
        let feed: MeteoAlarmFeed = quick_xml::de::from_str("<feed></feed>").unwrap();
        let report = meteoalarm_alerts_from_feed(feed, &Some("PL1465".to_string()), &[], "test");

        assert!(report.alerts.is_empty());
        assert!(report.region_filtered);
    }

    // -----------------------------------------------------------------
    // Stage 2: area names
    // -----------------------------------------------------------------

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// Live Portuguese area names, 2026-09-02.
    fn portugal_areas() -> Vec<String> {
        strings(&[
            "Beja",
            "Bragança",
            "Castelo Branco",
            "Coimbra",
            "Faro",
            "Guarda",
            "Leiria",
            "Lisboa",
            "Portalegre",
            "Santarém",
            "Setúbal",
            "Vila Real",
            "Viseu",
            "Évora",
        ])
    }

    /// Live Croatian area names, 2026-09-02.
    fn croatia_areas() -> Vec<String> {
        strings(&[
            "Dubrovnik region",
            "Kvarner i Kvarneric region",
            "Middle Dalmatia region",
            "North Dalmatia region",
            "Osijek region",
            "Rijeka region",
            "South Dalmatia region",
            "Split region",
            "Velebit channel region",
            "West Istrian coast region",
            "Zagreb region",
        ])
    }

    #[test]
    fn area_tokens_folds_diacritics_and_affixes() {
        assert_eq!(area_tokens("Évora"), strings(&["evora"]));
        assert_eq!(area_tokens("Setúbal"), strings(&["setubal"]));
        assert_eq!(area_tokens("Grad Zagreb"), strings(&["zagreb"]));
        assert_eq!(area_tokens("Zagreb region"), strings(&["zagreb"]));
        assert_eq!(
            area_tokens("Brussel-Hoofdstad - Bruxelles-Capitale"),
            strings(&["brussel", "hoofdstad", "bruxelles", "capitale"])
        );
        assert!(area_tokens("Kreis").is_empty());
    }

    #[test]
    fn rank_area_match_exact_beats_tokens() {
        let paris = area_tokens("Paris");
        assert_eq!(
            rank_area_match(&paris, &area_tokens("Paris")),
            Some(AreaMatch::Exact)
        );
        assert_eq!(
            rank_area_match(&paris, &area_tokens("Paris et Petite Ceinture")),
            Some(AreaMatch::Tokens)
        );
        assert!(AreaMatch::Exact > AreaMatch::Tokens);
    }

    #[test]
    fn rank_area_match_requires_an_anchor_token() {
        // "i" appears in "Kvarner i Kvarneric region" but is too short to
        // anchor a match on its own.
        assert_eq!(
            rank_area_match(
                &area_tokens("i"),
                &area_tokens("Kvarner i Kvarneric region")
            ),
            None
        );
        assert_eq!(rank_area_match(&[], &area_tokens("Faro")), None);
    }

    #[test]
    fn rank_area_match_rejects_substrings() {
        assert_eq!(
            rank_area_match(&area_tokens("Seine"), &area_tokens("Seinemaritime")),
            None
        );
    }

    #[test]
    fn match_area_zagreb() {
        let terms = strings(&["Grad Zagreb", "Stadt Grad Zagreb"]);
        assert_eq!(
            match_area(&terms, &croatia_areas()).as_deref(),
            Some("Zagreb region")
        );
    }

    #[test]
    fn match_area_lisboa_exact() {
        let terms = strings(&[
            "Lisboa",
            "Stadt Lisboa",
            "Arroios",
            "Lisboa",
            "Kreis Lisboa",
        ]);
        assert_eq!(
            match_area(&terms, &portugal_areas()).as_deref(),
            Some("Lisboa")
        );
    }

    #[test]
    fn match_area_paris_prefers_exact() {
        let areas = strings(&["Paris", "Paris et Petite Ceinture"]);
        assert_eq!(
            match_area(&strings(&["Paris"]), &areas).as_deref(),
            Some("Paris")
        );
    }

    #[test]
    fn match_area_ambiguous_term_is_a_miss() {
        let areas = strings(&["Seine-Maritime", "Seine-et-Marne", "Hauts-de-Seine"]);
        assert_eq!(match_area(&strings(&["Seine"]), &areas), None);
    }

    #[test]
    fn match_area_greek_script_misses_english_block() {
        // The legacy atom feed's areaDesc is the English transliteration;
        // Nominatim returns Greek script. Same-script matching needs the JSON
        // API's local-language block, which is a separate change.
        let areas = strings(&["Attiki", "Kriti", "Thessalia"]);
        assert_eq!(
            match_area(&strings(&["Αθήνα", "Περιφέρεια Αττικής"]), &areas),
            None
        );
    }

    #[test]
    fn match_area_no_terms_is_a_miss() {
        assert_eq!(match_area(&[], &portugal_areas()), None);
    }

    fn emma_entry(id: &str, emma_id: &str, area: &str) -> String {
        format!(
            r#"<entry>
                <id>https://feeds.meteoalarm.org/feed/{id}</id>
                <cap:geocode>
                    <valueName>EMMA_ID</valueName>
                    <value>{emma_id}</value>
                </cap:geocode>
                <cap:areaDesc>{area}</cap:areaDesc>
                <cap:event>Yellow High Temperature Warning</cap:event>
                <cap:severity>Moderate</cap:severity>
                <cap:sent>2026-06-01T08:00:00Z</cap:sent>
                <cap:expires>2099-01-01T00:00:00Z</cap:expires>
            </entry>"#
        )
    }

    fn portugal_feed() -> MeteoAlarmFeed {
        let xml = format!(
            "<feed>{}{}{}</feed>",
            emma_entry("pt-1", "PT021", "Faro"),
            emma_entry("pt-2", "PT013", "Lisboa"),
            emma_entry("pt-3", "PT015", "Setúbal")
        );
        quick_xml::de::from_str(&xml).unwrap()
    }

    #[test]
    fn portugal_user_in_lisboa_is_filtered_by_area() {
        // Stage 1 cannot resolve a Portuguese EMMA_ID (every codename is
        // "Portugal"), so the feed's own area names decide.
        let terms = strings(&[
            "Lisboa",
            "Stadt Lisboa",
            "Arroios",
            "Lisboa",
            "Kreis Lisboa",
        ]);
        let report = meteoalarm_alerts_from_feed(portugal_feed(), &None, &terms, "Portugal");

        assert_eq!(report.alerts.len(), 1);
        assert_eq!(report.alerts[0].area_desc, "Lisboa");
        assert!(report.region_filtered);
    }

    #[test]
    fn portugal_user_in_porto_renders_national() {
        // Porto is not alerting, so nothing in the feed can be matched to it;
        // the national feed renders and the report says so.
        let report =
            meteoalarm_alerts_from_feed(portugal_feed(), &None, &strings(&["Porto"]), "Portugal");

        assert_eq!(report.alerts.len(), 3);
        assert!(!report.region_filtered);
    }

    #[test]
    fn france_nuts3_feed_filters_by_area() {
        // An EMMA_ID resolved but the feed is NUTS3-tagged, so stage 1 cannot
        // apply; the area name can.
        let xml = format!(
            "<feed>{}{}</feed>",
            nuts3_entry("fr-1", "FR713", "Drôme"),
            nuts3_entry("fr-2", "FR813", "Hérault")
        );
        let feed: MeteoAlarmFeed = quick_xml::de::from_str(&xml).unwrap();
        let terms = strings(&["Valence", "Drôme"]);
        let report =
            meteoalarm_alerts_from_feed(feed, &Some("FR031".to_string()), &terms, "France");

        assert_eq!(report.alerts.len(), 1);
        assert_eq!(report.alerts[0].area_desc, "Drôme");
        assert!(report.region_filtered);
    }

    #[test]
    fn emma_id_quiet_day_stays_filtered_without_stage_two() {
        // Warsaw resolves PL1465 and nothing in the feed is for it: that is a
        // quiet day, not a miss, and stage 2 must not turn it into the
        // national feed.
        let xml = format!(
            "<feed>{}{}</feed>",
            emma_entry("pl-1", "PL999", "Kraków"),
            emma_entry("pl-2", "PL998", "Gdańsk")
        );
        let feed: MeteoAlarmFeed = quick_xml::de::from_str(&xml).unwrap();
        let report = meteoalarm_alerts_from_feed(
            feed,
            &Some("PL1465".to_string()),
            &strings(&["Warszawa"]),
            "Polska",
        );

        assert!(report.alerts.is_empty());
        assert!(report.region_filtered);
    }

    #[test]
    fn emma_search_terms_includes_village_and_municipality() {
        let mut address = address(Some("Lisboa"), Some("Lisboa"), None);
        address.village = Some("Arroios".to_string());
        address.municipality = Some("Lisboa".to_string());
        let terms = emma_search_terms(&address);

        assert_eq!(
            terms,
            vec![
                "Lisboa",
                "Stadt Lisboa",
                "Arroios",
                "Lisboa",
                "Lisboa",
                "Kreis Lisboa",
            ]
        );
    }

    #[test]
    fn cached_codenames_fills_then_reads() {
        // One process-wide value; the network fetch is not exercised here.
        cache_codenames(&codenames(&[("PT021", "Faro")]));
        let hit = cached_codenames().expect("cached after a successful fetch");
        assert_eq!(hit.codes.get("PT021").map(String::as_str), Some("Faro"));
    }
}
