// SPDX-License-Identifier: MIT OR Apache-2.0

//! Weather alerts from regional providers (NWS, MeteoAlarm, ECCC, BOM).

use std::collections::HashMap;
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
    /// because no region could be matched for the location, by EMMA_ID, by
    /// area name, or by local-language area name, so the entries are
    /// national, not local. NWS, ECCC and BOM
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

/// MeteoAlarm v1 JSON API response, `/api/v1/warnings/feeds-<slug>`. Read
/// only for its per-language `areaDesc` values; the atom feed stays the
/// source of alerts.
#[derive(Debug, Deserialize)]
struct MeteoAlarmJsonFeed {
    #[serde(default)]
    warnings: Vec<MeteoAlarmJsonWarning>,
}

#[derive(Debug, Deserialize)]
struct MeteoAlarmJsonWarning {
    alert: MeteoAlarmJsonAlert,
}

#[derive(Debug, Deserialize)]
struct MeteoAlarmJsonAlert {
    #[serde(default)]
    info: Vec<MeteoAlarmJsonInfo>,
}

/// One `info` block per language (`en-GB`, `el-GR`, `bg`, ...), each with
/// the same areas in the same order.
#[derive(Debug, Deserialize)]
struct MeteoAlarmJsonInfo {
    language: Option<String>,
    #[serde(default)]
    area: Vec<MeteoAlarmJsonArea>,
}

#[derive(Debug, Deserialize)]
struct MeteoAlarmJsonArea {
    #[serde(rename = "areaDesc")]
    area_desc: Option<String>,
}

/// One region as the feed names it in the local language and in English.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LocalArea {
    local: String,
    english: String,
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
    // Pinned to a commit, so a later push to the third party's repo can't
    // change which region a European location filters to. Bump it by hand.
    const CODENAMES_URL: &str = "https://raw.githubusercontent.com/ktrue/Meteoalarm-warning/a357ec93fdaa10e82217304840a2673bc6e14eb2/meteoalarm-codenames.json";

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

/// The feed's own local-language area names, paired with the English names
/// the atom feed uses. Read from the v1 JSON API, whose alerts carry one
/// `info` block per language. Expired warnings are kept: this is a name
/// inventory, not an alert list, and a larger one matches more quiet regions.
/// Not cached: it is fetched only on the non-Latin miss path, once per
/// refresh, and a fresh inventory is worth more than one saved request.
async fn fetch_meteoalarm_local_areas(slug: &str) -> Option<Vec<LocalArea>> {
    let url = format!(
        "https://feeds.meteoalarm.org/api/v1/warnings/feeds-{}",
        slug
    );

    let client = match http_client() {
        Ok(client) => client,
        Err(e) => {
            tracing::warn!(
                "No HTTP client for MeteoAlarm local area names ({}); national feed stays unfiltered",
                e
            );
            return None;
        }
    };

    let response = match client.get(&url).send().await {
        Ok(response) => response,
        Err(e) => {
            tracing::warn!(
                "MeteoAlarm local area names fetch failed ({}); national feed stays unfiltered",
                e
            );
            return None;
        }
    };

    match response.json::<MeteoAlarmJsonFeed>().await {
        Ok(feed) => Some(local_areas_from_json(feed)),
        Err(e) => {
            tracing::warn!(
                "MeteoAlarm local area names decode failed ({}); national feed stays unfiltered",
                e
            );
            None
        }
    }
}

/// Every (local name, English name) pair the feed carries, sorted and
/// deduplicated. Every non-English block pairs with every English block
/// (Serbia sends `sr-Latn` and `sr` beside `en-GB`); within a block, area
/// `i` pairs with area `i`, which is how the sampled feeds are laid out.
fn local_areas_from_json(feed: MeteoAlarmJsonFeed) -> Vec<LocalArea> {
    let mut pairs = Vec::new();
    for warning in feed.warnings {
        let (english, local): (Vec<_>, Vec<_>) = warning.alert.info.into_iter().partition(|info| {
            info.language
                .as_deref()
                .map(|l| l.starts_with("en"))
                .unwrap_or(false)
        });
        for local_info in &local {
            for english_info in &english {
                for (l, e) in local_info.area.iter().zip(english_info.area.iter()) {
                    if let (Some(l), Some(e)) = (l.area_desc.as_deref(), e.area_desc.as_deref()) {
                        if !l.is_empty() && !e.is_empty() {
                            pairs.push(LocalArea {
                                local: l.to_string(),
                                english: e.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
    pairs.sort();
    pairs.dedup();
    pairs
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

    // Counts only: warn is on by default, and the terms are the user's place names.
    tracing::warn!(
        "No EMMA_ID matched {} place name(s) in {}; will try the feed's own area names",
        search_terms.len(),
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
    "περιφερεια",
    "περιφερειακη",
    "ενοτητα",
    "δημος",
    "νομος",
    "област",
    "община",
    "град",
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

fn is_latin(c: char) -> bool {
    c.is_ascii_alphabetic()
        || ('\u{00C0}'..='\u{024F}').contains(&c)
        || ('\u{1E00}'..='\u{1EFF}').contains(&c)
}

/// True when any name carries a letter outside the Latin script, which is
/// when the atom feed's English area names cannot match and the
/// local-language inventory is worth a request.
fn has_non_latin(names: &[String]) -> bool {
    names
        .iter()
        .flat_map(|t| t.chars())
        .any(|c| c.is_alphabetic() && !is_latin(c))
}

/// Token equality with one concession to inflection: two non-Latin tokens of
/// five or more characters match when one is the other plus at most two
/// trailing characters ("αττικη" and "αττικης"). Latin tokens compare exactly,
/// so nothing changes for Latin-script countries.
fn tokens_equal(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (shorter, longer) = if a.chars().count() <= b.chars().count() {
        (a, b)
    } else {
        (b, a)
    };
    let non_latin = |s: &str| s.chars().any(|c| c.is_alphabetic() && !is_latin(c));
    non_latin(shorter)
        && non_latin(longer)
        && shorter.chars().count() >= 5
        && longer.starts_with(shorter)
        && longer.chars().count() - shorter.chars().count() <= 2
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
/// Latin tokens: "seine" does not fit "seinemaritime"; non-Latin tokens get
/// the `tokens_equal` inflection allowance. The shorter side must carry at
/// least one token of three or more characters, so a stray "i" or "de"
/// cannot anchor a match on its own.
fn rank_area_match(term_tokens: &[String], area_tokens: &[String]) -> Option<AreaMatch> {
    if term_tokens.is_empty() || area_tokens.is_empty() {
        return None;
    }
    if term_tokens.len() == area_tokens.len()
        && term_tokens
            .iter()
            .zip(area_tokens)
            .all(|(t, a)| tokens_equal(t, a))
    {
        return Some(AreaMatch::Exact);
    }
    let (shorter, longer) = if term_tokens.len() <= area_tokens.len() {
        (term_tokens, area_tokens)
    } else {
        (area_tokens, term_tokens)
    };
    let anchored = shorter.iter().any(|t| t.chars().count() >= 3);
    if anchored
        && shorter
            .iter()
            .all(|t| longer.iter().any(|l| tokens_equal(t, l)))
    {
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
    let report = meteoalarm_alerts_from_feed(feed, &user_emma_id, &search_terms, country);

    // Stage 3, only after a stage-2 miss for a non-Latin place name: the
    // feed's local-language area names, one request, no cache.
    if report.region_filtered || !has_non_latin(&search_terms) {
        return Ok(report);
    }
    let Some(local_areas) = fetch_meteoalarm_local_areas(slug).await else {
        return Ok(report);
    };
    Ok(apply_local_area_match(
        report,
        &search_terms,
        &local_areas,
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
/// area; a miss keeps the national feed and says so. Stage 3, in
/// `fetch_meteoalarm_alerts`, retries a stage-2 miss for non-Latin place
/// names against the feed's local-language names (`apply_local_area_match`).
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
            "MeteoAlarm feed ({}) carries no EMMA_ID geocodes (found {:?}); the EMMA_ID filter \
            cannot apply, matching by area name instead",
            country,
            schemes
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
                "Fetched {} alert(s) from MeteoAlarm ({}), UNFILTERED - none of {} place name(s) \
                matched among {} area(s) ({:?}{}); these are national alerts, not local ones",
                alerts.len(),
                country,
                search_terms.len(),
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

/// Stage 3: the stage-2 miss retried against the feed's local-language area
/// names. A hit narrows the already-parsed entries to the region whose
/// English name is the paired one; a miss returns the report unchanged.
fn apply_local_area_match(
    report: AlertReport,
    search_terms: &[String],
    local_areas: &[LocalArea],
    country: &str,
) -> AlertReport {
    let local_names: Vec<String> = local_areas.iter().map(|a| a.local.clone()).collect();
    if !has_non_latin(&local_names) {
        // Israel's `he-IL` block repeats the English names; say so rather than
        // report a miss that no place name could ever have avoided.
        tracing::warn!(
            "MeteoAlarm ({}) JSON feed carries no local-language area names ({} English only); \
            national feed stays unfiltered",
            country,
            local_names.len()
        );
        return report;
    }
    let Some(local) = match_area(search_terms, &local_names) else {
        tracing::warn!(
            "None of {} place name(s) matched a local-language area name among {} for MeteoAlarm ({}); \
            national feed stays unfiltered",
            search_terms.len(),
            local_names.len(),
            country
        );
        return report;
    };
    // Raw compare: the parsed atom `area_desc` and the JSON English name are
    // byte-identical (quick_xml has already unescaped the atom's extra level).
    // Token equality would collapse "Sofia-city" and "Sofia-region".
    let english: Vec<&str> = local_areas
        .iter()
        .filter(|a| a.local == local)
        .map(|a| a.english.as_str())
        .collect();
    let alerts: Vec<AlertEntry> = report
        .alerts
        .into_iter()
        .filter(|entry| english.contains(&entry.area_desc.as_str()))
        .collect();
    tracing::debug!(
        "Fetched {} alert(s) from MeteoAlarm ({}), filtered to area {:?} via its local name {:?}",
        alerts.len(),
        country,
        english,
        local
    );
    AlertReport {
        alerts,
        region_filtered: true,
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

/// Info block from ECCC CAP alert. A file carries one per language per
/// status: the same file can hold an `active` block for some areas and an
/// `ended` block for others.
#[derive(Debug, Deserialize)]
struct EcccCapInfo {
    language: Option<String>,
    event: Option<String>,
    #[serde(rename = "responseType")]
    response_type: Option<String>,
    severity: Option<String>,
    expires: Option<String>,
    headline: Option<String>,
    description: Option<String>,
    #[serde(rename = "parameter", default)]
    parameters: Vec<EcccCapParameter>,
    #[serde(rename = "area", default)]
    areas: Vec<EcccCapArea>,
}

/// `<parameter>` from an ECCC CAP info block.
#[derive(Debug, Deserialize)]
struct EcccCapParameter {
    #[serde(rename = "valueName")]
    value_name: Option<String>,
    value: Option<String>,
}

/// Area element from ECCC CAP alert.
#[derive(Debug, Deserialize)]
struct EcccCapArea {
    #[serde(rename = "areaDesc")]
    area_desc: Option<String>,
    polygon: Option<String>,
}

impl EcccCapInfo {
    /// Value of an ECCC `layer:EC-MSC-SMC:<version>:<name>` parameter.
    fn eccc_parameter(&self, name: &str) -> Option<&str> {
        self.parameters.iter().find_map(|p| {
            let value_name = p.value_name.as_deref()?;
            let rest = value_name.strip_prefix("layer:EC-MSC-SMC:")?;
            let (_, param) = rest.split_once(':')?;
            (param == name).then_some(p.value.as_deref()?)
        })
    }
}

/// One version of one alert for one area, from an info block whose area
/// contains the user's point. ECCC ends an alert with an `Update`, not a
/// `Cancel`, so whether the alert is over is part of the version.
#[derive(Debug)]
struct EcccVersion {
    /// Alert name and area. The name ("yellow warning - wind") rather than the
    /// event ("wind"), so an upgrade to another alert for the same hazard is
    /// its own group and can't be hidden behind the one it replaced.
    key: String,
    /// When the version was sent, for ordering. A `sent` that doesn't parse,
    /// or is more than an hour past the server's time, is `DateTime::MIN_UTC`:
    /// it can't override a real update, but still shows when it's the only one.
    sent: DateTime<Utc>,
    ended: bool,
    entry: AlertEntry,
}

/// Fetches active weather alerts from ECCC (Environment and Climate Change Canada).
async fn fetch_eccc_alerts(latitude: f64, longitude: f64) -> Result<AlertReport> {
    let offices = get_eccc_office_codes(latitude, longitude);
    let now = Utc::now();
    let today = now.format("%Y%m%d").to_string();
    // Yesterday's alerts are still current after 00:00 UTC, and they move out
    // of `today/` to the dated tree.
    let yesterday = (now - chrono::Duration::days(1))
        .format("%Y%m%d")
        .to_string();
    let client = http_client()?;

    let mut dir_urls: Vec<String> = Vec::new();
    for office in offices {
        dir_urls.push(format!(
            "https://dd.weather.gc.ca/{yesterday}/WXO-DD/alerts/cap/{yesterday}/{office}/"
        ));
        dir_urls.push(format!(
            "https://dd.weather.gc.ca/today/alerts/cap/{today}/{office}/"
        ));
    }

    let mut versions: Vec<EcccVersion> = Vec::new();

    for dir_url in dir_urls {
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

                let date = cap_response
                    .headers()
                    .get(reqwest::header::DATE)
                    .and_then(|d| d.to_str().ok());
                let sent_limit = eccc_sent_limit(date, now);

                let cap_xml = match cap_response.text().await {
                    Ok(text) => text,
                    Err(_) => continue,
                };

                versions.extend(parse_eccc_cap(
                    &cap_xml, latitude, longitude, now, sent_limit,
                ));
            }
        }
    }

    let alerts = latest_eccc_alerts(versions, now);
    tracing::debug!("Fetched {} alert(s) from ECCC", alerts.len());

    // Every entry passed the polygon check for the caller's point.
    Ok(AlertReport {
        alerts,
        region_filtered: true,
    })
}

/// How far past the server's time a `sent` may be before it isn't trusted
/// for ordering.
const ECCC_SENT_SKEW: chrono::Duration = chrono::Duration::hours(1);

/// How far the server's `Date` may be from the local clock before it's
/// ignored. A dual-boot clock read as UTC is off by a few hours, well inside.
const ECCC_DATE_TRUST: chrono::Duration = chrono::Duration::hours(24);

/// The latest `sent` trusted for ordering: the server's `Date` plus the
/// allowance. The server's clock is the one ECCC's `sent` comes from, so a
/// slow local clock can't make a real update look like it's from the future.
/// A missing, unparseable or implausible `Date` falls back to the local clock.
fn eccc_sent_limit(date_header: Option<&str>, now: DateTime<Utc>) -> DateTime<Utc> {
    let server_now = date_header
        .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
        .map(|d| d.with_timezone(&Utc))
        .filter(|d| (*d - now).abs() <= ECCC_DATE_TRUST)
        .unwrap_or(now);
    // chrono's `+` panics on overflow; `now` is never near the limit.
    server_now
        .checked_add_signed(ECCC_SENT_SKEW)
        .unwrap_or(now + ECCC_SENT_SKEW)
}

/// Parses an ECCC CAP XML document into one version per English info block
/// whose area contains the point. A `sent` later than `sent_limit` ranks
/// oldest; `now` is the local clock, for the 24-hour expiry fallback.
fn parse_eccc_cap(
    xml: &str,
    lat: f64,
    lon: f64,
    now: DateTime<Utc>,
    sent_limit: DateTime<Utc>,
) -> Vec<EcccVersion> {
    let Ok(cap) = quick_xml::de::from_str::<EcccCapAlert>(xml) else {
        return Vec::new();
    };

    if cap.status != "Actual" {
        return Vec::new();
    }

    // RFC 3339 limits years to four digits, so the 24-hour fallback below
    // can't overflow.
    let sent = DateTime::parse_from_rfc3339(&cap.sent)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .filter(|sent| *sent <= sent_limit);

    let is_english = |i: &&EcccCapInfo| {
        i.language
            .as_ref()
            .map(|l| l.starts_with("en"))
            .unwrap_or(false)
    };
    let mut blocks: Vec<&EcccCapInfo> = cap.info_blocks.iter().filter(is_english).collect();
    if blocks.is_empty() {
        blocks.extend(cap.info_blocks.first());
    }

    blocks
        .into_iter()
        .filter_map(|info| {
            let area_desc = info.areas.iter().find_map(|area| {
                area.polygon
                    .as_ref()
                    .filter(|poly| point_in_polygon(lat, lon, poly))
                    .map(|_| area.area_desc.clone().unwrap_or_default())
            })?;

            let event = info
                .event
                .clone()
                .unwrap_or_else(|| "Weather Alert".to_string());
            let name = info.eccc_parameter("Alert_Name").unwrap_or(&event);
            let key = format!("{}|{}", name, area_desc);

            // Unknown location statuses count as active: showing an alert
            // that is over beats hiding one that isn't.
            let ended = cap.msg_type == "Cancel"
                || info.response_type.as_deref() == Some("AllClear")
                || matches!(
                    info.eccc_parameter("Alert_Location_Status"),
                    Some("ended" | "transitioned_out")
                );

            let expires = info
                .expires
                .as_ref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .or_else(|| {
                    sent.unwrap_or(now)
                        .checked_add_signed(chrono::Duration::hours(24))
                })?;

            let headline = info.headline.clone().unwrap_or_else(|| event.clone());

            Some(EcccVersion {
                key,
                sent: sent.unwrap_or(DateTime::<Utc>::MIN_UTC),
                ended,
                entry: AlertEntry {
                    alert: Alert {
                        id: cap.identifier.clone(),
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
                },
            })
        })
        .collect()
}

/// Keeps the latest version of each alert for each area, and drops the alert
/// when that version ended it or has expired. On a tie in `sent`, a version
/// that keeps the alert going wins, then the higher identifier.
fn latest_eccc_alerts(versions: Vec<EcccVersion>, now: DateTime<Utc>) -> Vec<AlertEntry> {
    let mut latest: HashMap<String, EcccVersion> = HashMap::new();
    for version in versions {
        match latest.get(&version.key) {
            Some(kept)
                if (kept.sent, !kept.ended, &kept.entry.alert.id)
                    >= (version.sent, !version.ended, &version.entry.alert.id) => {}
            _ => {
                latest.insert(version.key.clone(), version);
            }
        }
    }

    let mut alerts: Vec<AlertEntry> = latest
        .into_values()
        .filter(|v| !v.ended && v.entry.alert.expires >= now)
        .map(|v| v.entry)
        .collect();
    // HashMap order is random; keep the output stable between refreshes.
    alerts.sort_by(|a, b| {
        (&a.alert.event, &a.area_desc, &a.alert.id).cmp(&(
            &b.alert.event,
            &b.area_desc,
            &b.alert.id,
        ))
    });
    alerts
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

/// Parser entry points for the fuzz targets in `fuzz/`.
#[cfg(feature = "fuzzing")]
pub(crate) mod fuzz {
    use std::collections::HashMap;

    use super::*;

    /// NWS `/alerts/active` GeoJSON.
    pub fn nws_alerts(data: &[u8]) {
        if let Ok(response) = serde_json::from_slice::<NwsAlertsResponse>(data) {
            let _ = nws_alerts_from_response(response);
        }
    }

    /// MeteoAlarm atom feed XML, unfiltered and under an EMMA_ID filter, with
    /// Latin and non-Latin place names for the area-name stage.
    pub fn meteoalarm_atom(xml: &str) {
        let terms = ["Wien".to_string(), "Αττική".to_string()];
        for filter in [None, Some("AT001".to_string())] {
            if let Ok(feed) = quick_xml::de::from_str::<MeteoAlarmFeed>(xml) {
                let _ = meteoalarm_alerts_from_feed(feed, &filter, &terms, "fuzz");
            }
        }
    }

    /// MeteoAlarm v1 JSON, through the local-language area match.
    pub fn meteoalarm_json(data: &[u8]) {
        let Ok(feed) = serde_json::from_slice::<MeteoAlarmJsonFeed>(data) else {
            return;
        };
        let local_areas = local_areas_from_json(feed);
        let report = AlertReport {
            alerts: vec![],
            region_filtered: false,
        };
        let terms = ["Αττική".to_string(), "София".to_string()];
        let _ = apply_local_area_match(report, &terms, &local_areas, "fuzz");
    }

    /// Place-name matching. Every input is third-party text: the place names
    /// from Nominatim, the codename table from GitHub, the area names from
    /// the feed.
    pub fn meteoalarm_match(
        place_names: Vec<String>,
        country_code: &str,
        codenames: Vec<(String, String)>,
        area_names: Vec<String>,
    ) {
        let mut names = place_names.into_iter();
        let address = NominatimAddress {
            country: None,
            country_code: Some(country_code.to_string()),
            city: names.next(),
            town: names.next(),
            village: names.next(),
            municipality: names.next(),
            county: names.next(),
            state: names.next(),
        };
        let codenames = MeteoAlarmCodenames {
            codes: codenames.into_iter().collect::<HashMap<_, _>>(),
        };
        let _ = match_emma_id(&address, country_code, &codenames);
        let terms = emma_search_terms(&address);
        let _ = has_non_latin(&terms);
        let _ = match_area(&terms, &area_names);
    }

    /// ECCC CAP XML checked against a point, and the polygon parser on the
    /// raw text.
    pub fn eccc_cap(latitude: f64, longitude: f64, xml: &str) {
        let now = Utc::now();
        let versions = parse_eccc_cap(xml, latitude, longitude, now, now + ECCC_SENT_SKEW);
        let _ = latest_eccc_alerts(versions, now);
        let _ = point_in_polygon(latitude, longitude, xml);
    }

    /// BOM `/warnings` JSON.
    pub fn bom_alerts(data: &[u8]) {
        if let Ok(response) = serde_json::from_slice::<BomWarningsResponse>(data) {
            let _ = bom_alerts_from_response(response.data);
        }
    }
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

    // Shapes follow real ECCC files from dd.weather.gc.ca, October 2026:
    // every info block comes in en-CA and fr-CA, the alert name and location
    // status are `layer:EC-MSC-SMC` parameters, and an alert ends with an
    // `Update` carrying `AllClear` and `ended`, never a `Cancel`.

    const ECCC_INSIDE: &str = "0,0 10,0 10,10 0,10";
    const ECCC_ELSEWHERE: &str = "20,20 30,20 30,30 20,30";

    /// One info block in each language for one area.
    fn eccc_info(name: &str, status: &str, expires: &str, area: &str, polygon: &str) -> String {
        let response = if status == "ended" {
            "AllClear"
        } else {
            "Monitor"
        };
        let event = name.rsplit(" - ").next().unwrap_or(name);
        ["en-CA", "fr-CA"]
            .iter()
            .map(|lang| {
                format!(
                    r#"<info>
                    <language>{lang}</language>
                    <event>{event}</event>
                    <responseType>{response}</responseType>
                    <severity>Moderate</severity>
                    <expires>{expires}</expires>
                    <headline>{name}</headline>
                    <description>Test.</description>
                    <parameter>
                        <valueName>layer:EC-MSC-SMC:1.0:Alert_Type</valueName>
                        <value>warning</value>
                    </parameter>
                    <parameter>
                        <valueName>layer:EC-MSC-SMC:1.0:Alert_Location_Status</valueName>
                        <value>{status}</value>
                    </parameter>
                    <parameter>
                        <valueName>layer:EC-MSC-SMC:1.0:Alert_Name</valueName>
                        <value>{name}</value>
                    </parameter>
                    <area>
                        <areaDesc>{area}</areaDesc>
                        <polygon>{polygon}</polygon>
                    </area>
                </info>"#
                )
            })
            .collect()
    }

    fn eccc_file(identifier: &str, msg_type: &str, sent: &str, infos: &[String]) -> String {
        format!(
            r#"<alert>
                <identifier>{identifier}</identifier>
                <status>Actual</status>
                <msgType>{msg_type}</msgType>
                <sent>{sent}</sent>
                {}
            </alert>"#,
            infos.concat()
        )
    }

    fn eccc_now() -> DateTime<Utc> {
        "2026-10-13T20:00:00Z".parse().unwrap()
    }

    /// The listing's files, parsed for the inside point and reduced to what
    /// the user sees.
    fn eccc_alerts_at(files: &[String], now: DateTime<Utc>) -> Vec<AlertEntry> {
        eccc_alerts_with_server_time(files, now, now)
    }

    /// `now` is the user's clock; `server_now` is the `Date` the files came with.
    fn eccc_alerts_with_server_time(
        files: &[String],
        now: DateTime<Utc>,
        server_now: DateTime<Utc>,
    ) -> Vec<AlertEntry> {
        let versions = files
            .iter()
            .flat_map(|xml| parse_eccc_cap(xml, 5.0, 5.0, now, server_now + ECCC_SENT_SKEW))
            .collect();
        latest_eccc_alerts(versions, now)
    }

    fn eccc_alerts(files: &[String]) -> Vec<AlertEntry> {
        eccc_alerts_at(files, eccc_now())
    }

    fn wind(status: &str, expires: &str) -> String {
        eccc_info(
            "yellow warning - wind",
            status,
            expires,
            "Belleville",
            ECCC_INSIDE,
        )
    }

    #[test]
    fn eccc_decodes_active_alert_in_polygon() {
        let file = eccc_file(
            "a1",
            "Alert",
            "2026-10-13T12:00:00-00:00",
            &[wind("active", "2026-10-14T04:00:00-00:00")],
        );

        let alerts = eccc_alerts(&[file]);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.event, "wind");
        assert_eq!(alerts[0].alert.headline, "yellow warning - wind");
        assert_eq!(alerts[0].alert.severity, AlertSeverity::Moderate);
        assert_eq!(alerts[0].alert.id, "a1");
        assert_eq!(alerts[0].area_desc, "Belleville");
    }

    #[test]
    fn eccc_rejects_point_outside_polygon() {
        let file = eccc_file(
            "a1",
            "Alert",
            "2026-10-13T12:00:00-00:00",
            &[eccc_info(
                "yellow warning - wind",
                "active",
                "2026-10-14T04:00:00-00:00",
                "Elsewhere",
                ECCC_ELSEWHERE,
            )],
        );

        assert!(eccc_alerts(&[file]).is_empty());
    }

    #[test]
    fn eccc_rejects_non_actual_status() {
        let file = eccc_file(
            "a1",
            "Alert",
            "2026-10-13T12:00:00-00:00",
            &[wind("active", "2026-10-14T04:00:00-00:00")],
        )
        .replace("<status>Actual</status>", "<status>Test</status>");

        assert!(eccc_alerts(&[file]).is_empty());
    }

    /// The Belleville alert from the CWTO feed on 2026-10-13: three versions,
    /// and before the fix the 03Z one claimed the alert and vanished at 19:28Z
    /// while the 12Z one ran to 04:21Z. Listed out of order on purpose; ECCC's
    /// listings don't follow `sent` either.
    #[test]
    fn eccc_latest_version_wins_whatever_the_listing_order() {
        let v12 = eccc_file(
            "v12",
            "Update",
            "2026-10-13T12:21:00-00:00",
            &[wind("active", "2026-10-14T04:21:00-00:00")],
        );
        let v03 = eccc_file(
            "v03",
            "Alert",
            "2026-10-13T03:28:00-00:00",
            &[wind("active", "2026-10-13T19:28:00-00:00")],
        );
        let v08 = eccc_file(
            "v08",
            "Update",
            "2026-10-13T08:44:00-00:00",
            &[wind("active", "2026-10-14T00:44:00-00:00")],
        );

        let alerts = eccc_alerts(&[v12, v03, v08]);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.id, "v12");
        assert_eq!(
            alerts[0].alert.expires,
            "2026-10-14T04:21:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn eccc_ended_update_ends_the_alert() {
        let active = eccc_file(
            "v1",
            "Alert",
            "2026-10-13T12:00:00-00:00",
            &[wind("active", "2026-10-14T04:00:00-00:00")],
        );
        let ended = eccc_file(
            "v2",
            "Update",
            "2026-10-13T18:00:00-00:00",
            &[wind("ended", "2026-10-13T21:00:00-00:00")],
        );

        assert!(eccc_alerts(&[active.clone(), ended.clone()]).is_empty());
        assert!(eccc_alerts(&[ended]).is_empty());
    }

    #[test]
    fn eccc_cancel_ends_the_alert() {
        let active = eccc_file(
            "v1",
            "Alert",
            "2026-10-13T12:00:00-00:00",
            &[wind("active", "2026-10-14T04:00:00-00:00")],
        );
        let cancel = eccc_file(
            "v2",
            "Cancel",
            "2026-10-13T18:00:00-00:00",
            &[wind("active", "2026-10-14T04:00:00-00:00")],
        );

        assert!(eccc_alerts(&[active, cancel]).is_empty());
    }

    /// From a CWUL frost advisory on 2026-10-17: one file, an `active` block
    /// for some areas and a `transitioned_out` block for others.
    #[test]
    fn eccc_transitioned_out_block_ends_the_alert_for_its_areas_only() {
        let name = "yellow advisory - frost";
        let earlier = eccc_file(
            "v1",
            "Alert",
            "2026-10-13T12:00:00-00:00",
            &[eccc_info(
                name,
                "active",
                "2026-10-14T04:00:00-00:00",
                "Soulanges area",
                ECCC_INSIDE,
            )],
        );
        let split = eccc_file(
            "v2",
            "Update",
            "2026-10-13T18:00:00-00:00",
            &[
                eccc_info(
                    name,
                    "active",
                    "2026-10-14T04:00:00-00:00",
                    "Lévis area",
                    ECCC_ELSEWHERE,
                ),
                eccc_info(
                    name,
                    "transitioned_out",
                    "2026-10-14T04:00:00-00:00",
                    "Soulanges area",
                    ECCC_INSIDE,
                ),
            ],
        );

        assert!(eccc_alerts(&[earlier.clone(), split.clone()]).is_empty());

        let versions = parse_eccc_cap(&split, 25.0, 25.0, eccc_now(), eccc_now() + ECCC_SENT_SKEW);
        let elsewhere = latest_eccc_alerts(versions, eccc_now());
        assert_eq!(elsewhere.len(), 1);
        assert_eq!(elsewhere[0].area_desc, "Lévis area");
    }

    #[test]
    fn eccc_active_block_after_the_first_is_read() {
        let file = eccc_file(
            "v1",
            "Update",
            "2026-10-13T18:00:00-00:00",
            &[
                eccc_info(
                    "yellow warning - wind",
                    "ended",
                    "2026-10-13T21:00:00-00:00",
                    "Elsewhere",
                    ECCC_ELSEWHERE,
                ),
                wind("active", "2026-10-14T04:00:00-00:00"),
            ],
        );

        let alerts = eccc_alerts(&[file]);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].area_desc, "Belleville");
    }

    /// An upgrade for the same hazard and area, sent at the same second as the
    /// old alert's `transitioned_out`. Keyed on the event, the two would share
    /// a group and the tie could hide the new warning.
    #[test]
    fn eccc_upgrade_to_another_alert_name_is_kept() {
        let out = eccc_file(
            "v1",
            "Update",
            "2026-10-13T18:00:00-00:00",
            &[wind("transitioned_out", "2026-10-14T04:00:00-00:00")],
        );
        let orange = eccc_file(
            "v2",
            "Alert",
            "2026-10-13T18:00:00-00:00",
            &[eccc_info(
                "orange warning - wind",
                "active",
                "2026-10-14T04:00:00-00:00",
                "Belleville",
                ECCC_INSIDE,
            )],
        );

        let alerts = eccc_alerts(&[orange, out]);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.headline, "orange warning - wind");
    }

    #[test]
    fn eccc_tie_in_sent_keeps_the_alert() {
        let ended = eccc_file(
            "v1",
            "Update",
            "2026-10-13T18:00:00-00:00",
            &[wind("ended", "2026-10-13T21:00:00-00:00")],
        );
        let active = eccc_file(
            "v2",
            "Update",
            "2026-10-13T18:00:00-00:00",
            &[wind("active", "2026-10-14T04:00:00-00:00")],
        );

        assert_eq!(eccc_alerts(&[active.clone(), ended.clone()]).len(), 1);
        assert_eq!(eccc_alerts(&[ended, active]).len(), 1);
    }

    #[test]
    fn eccc_unknown_location_status_counts_as_active() {
        let file = eccc_file(
            "v1",
            "Update",
            "2026-10-13T18:00:00-00:00",
            &[wind("something_new", "2026-10-14T04:00:00-00:00")],
        );

        assert_eq!(eccc_alerts(&[file]).len(), 1);
    }

    /// The latest version decides: an expired update hides an older version
    /// whose own expiry is still ahead.
    #[test]
    fn eccc_drops_alert_whose_latest_version_expired() {
        let older = eccc_file(
            "v1",
            "Alert",
            "2026-10-13T08:00:00-00:00",
            &[wind("active", "2026-10-14T04:00:00-00:00")],
        );
        let newer = eccc_file(
            "v2",
            "Update",
            "2026-10-13T12:00:00-00:00",
            &[wind("active", "2026-10-13T19:00:00-00:00")],
        );

        assert!(eccc_alerts(&[older, newer]).is_empty());
    }

    #[test]
    fn eccc_missing_expires_falls_back_to_a_day_after_sent() {
        let file = eccc_file(
            "v1",
            "Alert",
            "2026-10-13T12:00:00-00:00",
            &[wind("active", "x")],
        )
        .replace("<expires>x</expires>", "");

        let alerts = eccc_alerts(&[file]);

        assert_eq!(alerts.len(), 1);
        assert_eq!(
            alerts[0].alert.expires,
            "2026-10-14T12:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn eccc_far_future_sent_without_expires_does_not_panic() {
        // chrono's FromStr takes years up to 262142, and adding the 24-hour
        // fallback to that used to overflow and panic. RFC 3339 rejects it, so
        // the version ranks oldest and its expiry falls back to a day from now.
        let file = eccc_file(
            "v1",
            "Alert",
            "+262142-12-31T23:59:59Z",
            &[wind("active", "x")],
        )
        .replace("<expires>x</expires>", "");

        let alerts = eccc_alerts(&[file]);

        assert_eq!(alerts.len(), 1);
        assert_eq!(
            alerts[0].alert.expires,
            eccc_now() + chrono::Duration::hours(24)
        );
    }

    /// A small format slip in `sent` must not hide a warning that is only in
    /// that file.
    #[test]
    fn eccc_unparseable_sent_still_shows_when_alone() {
        let file = eccc_file(
            "v1",
            "Alert",
            "2026-10-13T12:21:00",
            &[wind("active", "2026-10-14T04:21:00-00:00")],
        );

        assert_eq!(eccc_alerts(&[file]).len(), 1);
    }

    /// ...and must not override a real update either.
    #[test]
    fn eccc_unparseable_sent_ranks_below_a_real_update() {
        let slipped = eccc_file(
            "v1",
            "Update",
            "2026-10-13T18:00:00",
            &[wind("ended", "2026-10-13T21:00:00-00:00")],
        );
        let real = eccc_file(
            "v2",
            "Update",
            "2026-10-13T12:00:00-00:00",
            &[wind("active", "2026-10-14T04:00:00-00:00")],
        );

        let alerts = eccc_alerts(&[slipped, real]);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.id, "v2");
    }

    /// The second review's case: the user's clock is two hours slow, and the
    /// newest update is newer than that clock plus the allowance. Measured
    /// against the server's time it is still the latest.
    #[test]
    fn eccc_slow_local_clock_still_takes_the_newest_update() {
        let older = eccc_file(
            "v1",
            "Alert",
            "2026-10-13T12:00:00-00:00",
            &[wind("active", "2026-10-13T19:30:00-00:00")],
        );
        let newest = eccc_file(
            "v2",
            "Update",
            "2026-10-13T21:30:00-00:00",
            &[wind("active", "2026-10-14T04:00:00-00:00")],
        );
        let local = eccc_now();
        let server = local + chrono::Duration::hours(2);

        let alerts = eccc_alerts_with_server_time(&[older, newest], local, server);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert.id, "v2");
    }

    #[test]
    fn eccc_sent_limit_uses_a_plausible_server_date() {
        let now = eccc_now();
        let limit = eccc_sent_limit(Some("Tue, 13 Oct 2026 22:00:00 GMT"), now);

        assert_eq!(
            limit,
            "2026-10-13T23:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn eccc_sent_limit_falls_back_to_the_local_clock() {
        let now = eccc_now();
        let local = now + ECCC_SENT_SKEW;

        assert_eq!(eccc_sent_limit(None, now), local);
        assert_eq!(eccc_sent_limit(Some("not a date"), now), local);
        // A week ahead would switch off the future-`sent` check.
        assert_eq!(
            eccc_sent_limit(Some("Tue, 20 Oct 2026 20:00:00 GMT"), now),
            local
        );
    }

    /// The third review's case: chrono's RFC 2822 parser takes years up to
    /// 262142, and adding the allowance to that used to overflow and panic.
    #[test]
    fn eccc_sent_limit_does_not_panic_on_a_far_future_date() {
        let now = eccc_now();

        assert_eq!(
            eccc_sent_limit(Some("31 Dec 262142 23:30:00 +0000"), now),
            now + ECCC_SENT_SKEW
        );
    }

    /// A version sent more than an hour after the fetch can't end a real
    /// alert; one a few minutes ahead, as clock skew would give, still counts.
    #[test]
    fn eccc_future_sent_cannot_override_a_real_update() {
        let real = eccc_file(
            "v1",
            "Update",
            "2026-10-13T12:00:00-00:00",
            &[wind("active", "2026-10-14T04:00:00-00:00")],
        );
        let far_ahead = eccc_file(
            "v2",
            "Update",
            "9999-12-31T23:59:59-00:00",
            &[wind("ended", "2026-10-13T21:00:00-00:00")],
        );
        let skewed = eccc_file(
            "v3",
            "Update",
            "2026-10-13T20:10:00-00:00",
            &[wind("ended", "2026-10-13T21:00:00-00:00")],
        );

        assert_eq!(eccc_alerts(&[real.clone(), far_ahead]).len(), 1);
        assert!(eccc_alerts(&[real, skewed]).is_empty());
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

    fn local_areas(pairs: &[(&str, &str)]) -> Vec<LocalArea> {
        pairs
            .iter()
            .map(|(local, english)| LocalArea {
                local: local.to_string(),
                english: english.to_string(),
            })
            .collect()
    }

    /// Live Greek pairs from the v1 JSON feed, 2026-09-02, including the
    /// source's stray tonos on Δωδεκάνησα and its literal `&amp;`.
    fn greece_local_areas() -> Vec<LocalArea> {
        local_areas(&[
            ("Ήπειρο", "Epirus"),
            ("Ανατολική Μακεδονία", "East Makedonia"),
            ("Ανατολική Πελοπόννησο", "East Peloponnisos"),
            ("Ανατολική Στερεά &amp; Έυβοια", "East Sterea &amp; Evvoia"),
            ("Αττική", "Attiki"),
            ("Δυτική Μακεδονία", "West Makedonia"),
            ("Δυτική Πελοπόννησο", "West Peloponnisos"),
            ("Δυτική Στερεά", "West Sterea"),
            ("Δωδεκάνησα΄", "Dodekanisa Islands"),
            ("Θεσσαλία", "Thessalia"),
            ("Θράκη", "Thraki"),
            ("Κεντρική Μακεδονία", "Central Makedonia"),
            ("Κρήτη", "Kriti"),
            ("Κυκλάδες", "Kyklades"),
            (
                "Νησιά Βορειοανατολικού Αιγαίου",
                "North East Aegean Islands",
            ),
            ("Νησιά Ιονίου", "Ionion Islands"),
        ])
    }

    /// Live Bulgarian pairs from the v1 JSON feed, 2026-09-02.
    fn bulgaria_local_areas() -> Vec<LocalArea> {
        local_areas(&[
            ("Благоевград", "Blagoevgrad"),
            ("Бургас", "Burgas"),
            ("Варна", "Varna"),
            ("Велико Търново", "Veliko Tarnovo"),
            ("Видин", "Vidin"),
            ("Враца", "Vratsa"),
            ("Габрово", "Gabrovo"),
            ("Добрич", "Dobrich"),
            ("Кърджали", "Kardzhali"),
            ("Кюстендил", "Kyustendil"),
            ("Ловеч", "Lovech"),
            ("Монтана", "Montana"),
            ("Пазарджик", "Pazardzhik"),
            ("Перник", "Pernik"),
            ("Плевен", "Pleven"),
            ("Пловдив", "Plovdiv"),
            ("Разград", "Razgrad"),
            ("Русе", "Ruse"),
            ("Силистра", "Silistra"),
            ("Сливен", "Sliven"),
            ("Смолян", "Smolyan"),
            ("Софийска област", "Sofia-region"),
            ("София град", "Sofia-city"),
            ("Стара Загора", "Stara Zagora"),
            ("Търговище", "Targovishte"),
            ("Хасково", "Haskovo"),
            ("Шумен", "Shumen"),
            ("Ямбол", "Yambol"),
        ])
    }

    fn local_names(areas: &[LocalArea]) -> Vec<String> {
        areas.iter().map(|a| a.local.clone()).collect()
    }

    /// What `emma_search_terms` builds from Nominatim's Athens reverse
    /// geocode (city, municipality, county, state), 2026-09-02.
    fn athens_terms() -> Vec<String> {
        strings(&[
            "Αθήνα",
            "Stadt Αθήνα",
            "Δήμος Αθηναίων",
            "Περιφερειακή Ενότητα Κεντρικού Τομέα Αθηνών",
            "Kreis Περιφερειακή Ενότητα Κεντρικού Τομέα Αθηνών",
            "Περιφέρεια Αττικής",
        ])
    }

    /// Nominatim's Sofia reverse geocode (city, county, state), 2026-09-02.
    fn sofia_terms() -> Vec<String> {
        strings(&[
            "София",
            "Stadt София",
            "Средец",
            "Kreis Средец",
            "София-град",
        ])
    }

    /// The Greek atom feed as it reaches stage 2: English area names, and
    /// the double-escaped ampersand the source really sends.
    fn greece_report() -> AlertReport {
        let xml = format!(
            "<feed>{}{}{}</feed>",
            emma_entry("gr-1", "GR001", "Attiki"),
            emma_entry("gr-2", "GR002", "Kriti"),
            emma_entry("gr-3", "GR003", "East Sterea &amp;amp; Evvoia")
        );
        let feed: MeteoAlarmFeed = quick_xml::de::from_str(&xml).unwrap();
        let report = meteoalarm_alerts_from_feed(feed, &None, &athens_terms(), "Ελλάς");
        assert!(
            !report.region_filtered,
            "Greek terms cannot match the English block"
        );
        assert_eq!(report.alerts.len(), 3);
        report
    }

    #[test]
    fn is_latin_and_has_non_latin() {
        assert!(!has_non_latin(&strings(&["Évora", "Łódź", "Setúbal"])));
        assert!(has_non_latin(&strings(&["Αθήνα"])));
        assert!(has_non_latin(&strings(&["София"])));
        assert!(has_non_latin(&strings(&["ירושלים"])));
        assert!(has_non_latin(&strings(&["Lisboa", "Αθήνα"])));
        assert!(!has_non_latin(&[]));
    }

    #[test]
    fn area_tokens_drops_greek_and_bulgarian_affixes() {
        assert_eq!(area_tokens("Περιφέρεια Αττικής"), strings(&["αττικης"]));
        assert_eq!(area_tokens("Δήμος Αθηναίων"), strings(&["αθηναιων"]));
        assert_eq!(area_tokens("София град"), strings(&["софия"]));
        // NFD splits й into и plus a breve and the fold drops the breve; both
        // sides fold the same way, so the compare is unaffected.
        assert_eq!(area_tokens("Софийска област"), strings(&["софииска"]));
    }

    #[test]
    fn tokens_equal_genitive() {
        assert!(tokens_equal("αττικη", "αττικης"));
        assert!(tokens_equal("αττικης", "αττικη"));
        assert!(tokens_equal("κρητη", "κρητης"));
        assert!(!tokens_equal("αθηνα", "αθηναιων"));
        assert!(!tokens_equal("paris", "parise"));
        assert!(!tokens_equal("αττι", "αττικη"));
        assert!(!tokens_equal("софия", "софииска"));
    }

    #[test]
    fn match_area_athens_against_local_names() {
        // The first three terms miss (Athens is not a region); the state
        // "Περιφέρεια Αττικής" meets "Αττική" through the affix drop and the
        // genitive allowance.
        assert_eq!(
            match_area(&athens_terms(), &local_names(&greece_local_areas())),
            Some("Αττική".to_string())
        );
    }

    #[test]
    fn match_area_sofia_against_local_names() {
        // "София" fits "София град" on the first term. "Софийска област" does
        // not compete: "софия" is not a prefix of "софииска".
        assert_eq!(
            match_area(&sofia_terms(), &local_names(&bulgaria_local_areas())),
            Some("София град".to_string())
        );
    }

    #[test]
    fn local_areas_from_json_pairs_blocks() {
        // Two Greek warnings sharing a region, one Serbian-shaped warning with
        // two local blocks, and an area with no name in either language.
        let json = r#"{"warnings": [
            {"alert": {"info": [
                {"language": "en-GB", "area": [{"areaDesc": "Attiki"}]},
                {"language": "el-GR", "area": [{"areaDesc": "Αττική"}]}
            ]}},
            {"alert": {"info": [
                {"language": "el-GR", "area": [{"areaDesc": "Κρήτη"}]},
                {"language": "en-GB", "area": [{"areaDesc": "Kriti"}]}
            ]}},
            {"alert": {"info": [
                {"language": "en-GB", "area": [{"areaDesc": "Attiki"}]},
                {"language": "el-GR", "area": [{"areaDesc": "Αττική"}]}
            ]}},
            {"alert": {"info": [
                {"language": "sr-Latn", "area": [{"areaDesc": "Beograd"}]},
                {"language": "sr", "area": [{"areaDesc": "Београд"}]},
                {"language": "en-GB", "area": [{"areaDesc": "Belgrade"}]}
            ]}},
            {"alert": {"info": [
                {"language": "en-GB", "area": [{}]},
                {"language": "el-GR", "area": [{"areaDesc": ""}]}
            ]}},
            {"alert": {}}
        ]}"#;
        let feed: MeteoAlarmJsonFeed = serde_json::from_str(json).unwrap();

        assert_eq!(
            local_areas_from_json(feed),
            local_areas(&[
                ("Beograd", "Belgrade"),
                ("Αττική", "Attiki"),
                ("Κρήτη", "Kriti"),
                ("Београд", "Belgrade"),
            ])
        );
    }

    #[test]
    fn apply_local_area_match_filters_by_english_name() {
        let report = apply_local_area_match(
            greece_report(),
            &athens_terms(),
            &greece_local_areas(),
            "Ελλάς",
        );

        assert_eq!(report.alerts.len(), 1);
        assert_eq!(report.alerts[0].area_desc, "Attiki");
        assert!(report.region_filtered);
    }

    #[test]
    fn apply_local_area_match_compares_parsed_area_desc_raw() {
        // The atom source double-escapes the ampersand and quick_xml unescapes
        // one level, so the parsed entry equals the JSON English name byte for
        // byte. Terms that pick the Sterea region keep only that entry.
        let terms = strings(&["Ανατολική Στερεά"]);
        let report =
            apply_local_area_match(greece_report(), &terms, &greece_local_areas(), "Ελλάς");

        assert_eq!(report.alerts.len(), 1);
        assert_eq!(report.alerts[0].area_desc, "East Sterea &amp; Evvoia");
        assert!(report.region_filtered);
    }

    #[test]
    fn apply_local_area_match_keeps_city_not_region() {
        // "Sofia-city" and "Sofia-region" fold to the same tokens once the
        // affixes drop; the raw compare keeps them apart.
        let xml = format!(
            "<feed>{}{}{}</feed>",
            nuts3_entry("bg-1", "BG411", "Sofia-city"),
            nuts3_entry("bg-2", "BG412", "Sofia-region"),
            nuts3_entry("bg-3", "BG413", "Blagoevgrad")
        );
        let feed: MeteoAlarmFeed = quick_xml::de::from_str(&xml).unwrap();
        let report = meteoalarm_alerts_from_feed(feed, &None, &sofia_terms(), "България");
        assert!(!report.region_filtered);

        let report =
            apply_local_area_match(report, &sofia_terms(), &bulgaria_local_areas(), "България");

        assert_eq!(report.alerts.len(), 1);
        assert_eq!(report.alerts[0].area_desc, "Sofia-city");
        assert!(report.region_filtered);
    }

    #[test]
    fn apply_local_area_match_miss_leaves_report() {
        let terms = strings(&["Θεσσαλονίκη"]);
        let report =
            apply_local_area_match(greece_report(), &terms, &greece_local_areas(), "Ελλάς");

        assert_eq!(report.alerts.len(), 3);
        assert!(!report.region_filtered);
    }

    #[test]
    fn apply_local_area_match_skips_latin_only_inventory() {
        // Israel's he-IL block repeats the English names, so Hebrew terms can
        // never match; the report passes through untouched.
        let israel = local_areas(&[
            ("Judea Mountains", "Judea Mountains"),
            ("Gush Dan", "Gush Dan"),
        ]);
        let terms = strings(&["ירושלים", "מחוז ירושלים"]);
        let report = apply_local_area_match(greece_report(), &terms, &israel, "ישראל");

        assert_eq!(report.alerts.len(), 3);
        assert!(!report.region_filtered);
    }

    // Warn is on by default, so a matching miss must log counts, not the
    // user's place names or EMMA_ID. "Placeholdville" is in no feed or table.
    #[test]
    #[tracing_test::traced_test]
    fn emma_id_miss_warning_carries_no_place_names() {
        let codenames = codenames(&[("PL101", "Warszawa")]);
        let address = address(Some("Placeholdville"), None, None);

        assert!(match_emma_id(&address, "pl", &codenames).is_none());
        assert!(logs_contain("No EMMA_ID matched"));
        assert!(!logs_contain("Placeholdville"));
    }

    #[test]
    #[tracing_test::traced_test]
    fn unfiltered_feed_warnings_carry_no_place_names_or_emma_id() {
        let xml = format!(
            "<feed>{}{}</feed>",
            nuts3_entry("fr-1", "FR713", "Drôme"),
            nuts3_entry("fr-2", "FR813", "Hérault")
        );
        let feed: MeteoAlarmFeed = quick_xml::de::from_str(&xml).unwrap();
        let terms = vec!["Placeholdville".to_string()];

        let report = meteoalarm_alerts_from_feed(feed, &Some("FR101".to_string()), &terms, "test");

        assert!(!report.region_filtered);
        assert!(logs_contain("UNFILTERED"));
        assert!(!logs_contain("Placeholdville"));
        assert!(!logs_contain("FR101"));
    }

    #[test]
    #[tracing_test::traced_test]
    fn local_name_miss_warning_carries_no_place_names() {
        let report = AlertReport {
            alerts: vec![],
            region_filtered: false,
        };
        let areas = local_areas(&[("Αττική", "Attiki")]);
        let terms = vec!["Πλασεχολντβιλ".to_string()];

        apply_local_area_match(report, &terms, &areas, "test");

        assert!(logs_contain("national feed stays unfiltered"));
        assert!(!logs_contain("Πλασεχολντβιλ"));
    }
}
