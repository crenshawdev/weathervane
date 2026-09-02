// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::sync::RwLock;

use serde::Deserialize;

use crate::client::http_client;
use crate::error::{Error, ParseKind, Result};

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
pub(crate) fn is_us_bounds(lat: f64, lon: f64) -> bool {
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

/// Checks if coordinates fall within Japan (Honshu, Hokkaido, Kyushu, Shikoku, Ryukyu chain).
/// Used to gate the AMeDAS temperature override in fetch_weather.
pub(crate) fn is_japan_bounds(lat: f64, lon: f64) -> bool {
    // Honshu, Kyushu, Shikoku. West edge at 129.5E keeps South Korea
    // (Busan ~129.08E) out. Costs Tsushima (~129.3E) the override.
    let honshu = (30.5..=41.0).contains(&lat) && (129.5..=142.5).contains(&lon);

    // Hokkaido. West edge at 139.5E keeps Vladivostok (131.87E) out.
    let hokkaido = (41.0..=45.5).contains(&lat) && (139.5..=146.0).contains(&lon);

    // Ryukyu chain plus Yakushima. West edge at 122.5E keeps Taiwan
    // (north tip ~25.3N, 121.5E) out.
    let ryukyu = (24.0..=30.5).contains(&lat) && (122.5..=131.0).contains(&lon);

    honshu || hokkaido || ryukyu
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

/// Maps an ISO 3166-1 alpha-2 country code to that country's MeteoAlarm feed
/// slug and EMMA_ID prefix. Returns None if the country is not covered.
///
/// Keyed on the code rather than the country name because the name arrives from
/// Nominatim in the local language, and asking Nominatim for English instead
/// breaks the EMMA_ID match against MeteoAlarm's own local-language codename
/// list (`PL1465` is "Warszawa", not "Warsaw").
///
/// The prefix is the uppercased code everywhere except the United Kingdom,
/// whose alerts use `UK` rather than ISO `GB`.
pub(crate) fn get_meteoalarm_info(country_code: &str) -> Option<(&'static str, &'static str)> {
    match country_code.to_lowercase().as_str() {
        "at" => Some(("austria", "AT")),
        "ba" => Some(("bosnia-herzegovina", "BA")),
        "be" => Some(("belgium", "BE")),
        "bg" => Some(("bulgaria", "BG")),
        "ch" => Some(("switzerland", "CH")),
        "cy" => Some(("cyprus", "CY")),
        "cz" => Some(("czechia", "CZ")),
        "de" => Some(("germany", "DE")),
        "dk" => Some(("denmark", "DK")),
        "ee" => Some(("estonia", "EE")),
        "es" => Some(("spain", "ES")),
        "fi" => Some(("finland", "FI")),
        "fr" => Some(("france", "FR")),
        "gb" => Some(("united-kingdom", "UK")),
        "gr" => Some(("greece", "GR")),
        "hr" => Some(("croatia", "HR")),
        "hu" => Some(("hungary", "HU")),
        "ie" => Some(("ireland", "IE")),
        "il" => Some(("israel", "IL")),
        "is" => Some(("iceland", "IS")),
        "it" => Some(("italy", "IT")),
        "lt" => Some(("lithuania", "LT")),
        "lu" => Some(("luxembourg", "LU")),
        "lv" => Some(("latvia", "LV")),
        "md" => Some(("moldova", "MD")),
        "me" => Some(("montenegro", "ME")),
        "mk" => Some(("north-macedonia", "MK")),
        "mt" => Some(("malta", "MT")),
        "nl" => Some(("netherlands", "NL")),
        "no" => Some(("norway", "NO")),
        "pl" => Some(("poland", "PL")),
        "pt" => Some(("portugal", "PT")),
        "ro" => Some(("romania", "RO")),
        "rs" => Some(("serbia", "RS")),
        "se" => Some(("sweden", "SE")),
        "si" => Some(("slovenia", "SI")),
        "sk" => Some(("slovakia", "SK")),
        _ => None,
    }
}

/// Nominatim reverse geocoding response.
#[derive(Debug, Deserialize)]
pub(crate) struct NominatimResponse {
    pub address: Option<NominatimAddress>,
}

/// Address details from Nominatim.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct NominatimAddress {
    /// Local-language country name. Display only - `country_code` is what the
    /// MeteoAlarm lookup keys on, because Nominatim is deliberately not asked
    /// for English names.
    pub country: Option<String>,
    /// ISO 3166-1 alpha-2, lowercase. Language-independent, which is why the
    /// feed lookup uses it.
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub town: Option<String>,
    pub village: Option<String>,
    pub municipality: Option<String>,
    pub county: Option<String>,
    pub state: Option<String>,
}

/// MeteoAlarm codenames mapping (EMMA_ID -> region name).
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub(crate) struct MeteoAlarmCodenames {
    pub codes: std::collections::HashMap<String, String>,
}

/// Reverse geocodes, cached per coordinate for the process lifetime.
static REVERSE_GEOCODE_CACHE: RwLock<Option<HashMap<String, NominatimAddress>>> = RwLock::new(None);

/// Reverse-geocodes a coordinate to an address, from cache after the first call.
///
/// Nominatim's usage policy requires results to be cached client-side, and a
/// consumer that refreshes on a timer sends the same coordinates every time.
/// Cached for the process lifetime, in the shape of `STATION_CACHE`: a
/// coordinate's address does not move, and since `fetch_alerts` returns `Err`
/// when the country cannot be determined, being blocked for repeat queries
/// would mean no European alerts at all. Failures are not cached, so a
/// transient miss retries on the next call.
pub(crate) async fn reverse_geocode(latitude: f64, longitude: f64) -> Result<NominatimAddress> {
    let key = cache_key(latitude, longitude);
    if let Some(address) = cached_address(&key) {
        tracing::debug!("Reverse geocode cache hit for {}", key);
        return Ok(address);
    }

    let address = fetch_reverse_geocode(latitude, longitude).await?; // no lock held across .await
    cache_address(&key, &address);
    Ok(address)
}

/// The same strings the request URL carries, so a hit is exactly a repeated
/// query in the sense Nominatim's policy uses.
fn cache_key(latitude: f64, longitude: f64) -> String {
    format!("{},{}", latitude, longitude)
}

fn cached_address(key: &str) -> Option<NominatimAddress> {
    let guard = REVERSE_GEOCODE_CACHE.read().ok()?;
    guard.as_ref()?.get(key).cloned()
}

fn cache_address(key: &str, address: &NominatimAddress) {
    if let Ok(mut guard) = REVERSE_GEOCODE_CACHE.write() {
        guard
            .get_or_insert_with(HashMap::new)
            .insert(key.to_string(), address.clone());
    }
}

/// The network call behind `reverse_geocode`.
///
/// One call serves both the country (MeteoAlarm feed selection) and the place
/// names (EMMA_ID region matching), which were previously two separate lookups
/// of the same point.
///
/// No `accept-language` is requested on purpose. The MeteoAlarm codename list
/// is in local language - `PL1465` is "Warszawa", not "Warsaw" - so asking for
/// English names would break every EMMA_ID match in Europe. The country is
/// identified by `country_code` instead, which carries no language at all.
async fn fetch_reverse_geocode(latitude: f64, longitude: f64) -> Result<NominatimAddress> {
    let url = format!(
        "https://nominatim.openstreetmap.org/reverse?lat={}&lon={}&format=json",
        latitude, longitude
    );
    let body = http_client()?
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    address_from_json(&body)
}

/// Split from the request so the decode is unit-testable without a network.
fn address_from_json(body: &str) -> Result<NominatimAddress> {
    let response: NominatimResponse =
        serde_json::from_str(body).map_err(|_| Error::Parse(ParseKind::Json))?;
    response.address.ok_or(Error::LocationDetection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japan_bounds_includes_main_cities() {
        assert!(is_japan_bounds(35.68, 139.65), "Tokyo");
        assert!(is_japan_bounds(34.69, 135.50), "Osaka");
        assert!(is_japan_bounds(43.07, 141.35), "Sapporo");
        assert!(is_japan_bounds(33.59, 130.40), "Fukuoka");
        assert!(is_japan_bounds(26.21, 127.68), "Okinawa (Naha)");
        assert!(is_japan_bounds(24.34, 124.16), "Ishigaki");
        assert!(is_japan_bounds(30.33, 130.62), "Yakushima");
    }

    #[test]
    fn japan_bounds_excludes_neighbors() {
        assert!(!is_japan_bounds(37.57, 126.98), "Seoul");
        assert!(!is_japan_bounds(35.18, 129.08), "Busan");
        assert!(!is_japan_bounds(43.12, 131.87), "Vladivostok");
        assert!(!is_japan_bounds(46.96, 142.72), "Yuzhno-Sakhalinsk");
        assert!(!is_japan_bounds(25.03, 121.57), "Taipei");
        assert!(!is_japan_bounds(13.44, 144.79), "Guam");
        assert!(!is_japan_bounds(31.23, 121.47), "Shanghai");
    }

    #[test]
    fn detect_region_routes_us_cities() {
        assert_eq!(detect_region(40.71, -74.01), Region::Us, "New York");
        assert_eq!(detect_region(34.05, -118.24), Region::Us, "Los Angeles");
        assert_eq!(detect_region(41.88, -87.63), Region::Us, "Chicago");
        assert_eq!(detect_region(47.61, -122.33), Region::Us, "Seattle");
        assert_eq!(detect_region(61.22, -149.90), Region::Us, "Anchorage (AK)");
        assert_eq!(detect_region(21.31, -157.86), Region::Us, "Honolulu (HI)");
    }

    #[test]
    fn detect_region_routes_canadian_cities() {
        // These sit north of the US border bands, so US is ruled out first.
        assert_eq!(detect_region(43.65, -79.38), Region::Canada, "Toronto");
        assert_eq!(detect_region(45.50, -73.57), Region::Canada, "Montreal");
        assert_eq!(detect_region(49.28, -123.12), Region::Canada, "Vancouver");
        assert_eq!(detect_region(53.55, -113.49), Region::Canada, "Edmonton");
    }

    #[test]
    fn detect_region_routes_europe_and_australia() {
        assert_eq!(detect_region(51.51, -0.13), Region::Europe, "London");
        assert_eq!(detect_region(52.52, 13.40), Region::Europe, "Berlin");
        assert_eq!(detect_region(40.42, -3.70), Region::Europe, "Madrid");
        assert_eq!(detect_region(-33.87, 151.21), Region::Australia, "Sydney");
        assert_eq!(detect_region(-31.95, 115.86), Region::Australia, "Perth");
    }

    #[test]
    fn detect_region_unknown_outside_coverage() {
        assert_eq!(detect_region(35.68, 139.65), Region::Unknown, "Tokyo");
        assert_eq!(detect_region(-23.55, -46.63), Region::Unknown, "Sao Paulo");
        assert_eq!(detect_region(-1.29, 36.82), Region::Unknown, "Nairobi");
    }

    #[test]
    fn point_in_polygon_square() {
        // 10x10 square, vertices as "lat,lon" pairs.
        let square = "0,0 10,0 10,10 0,10";
        assert!(point_in_polygon(5.0, 5.0, square), "center is inside");
        assert!(!point_in_polygon(5.0, 15.0, square), "east of square");
        assert!(
            !point_in_polygon(20.0, 20.0, square),
            "north-east of square"
        );
        assert!(!point_in_polygon(-5.0, 5.0, square), "south of square");
    }

    #[test]
    fn point_in_polygon_rejects_degenerate() {
        assert!(!point_in_polygon(5.0, 5.0, ""), "empty string");
        assert!(
            !point_in_polygon(5.0, 5.0, "0,0 10,10"),
            "only two vertices"
        );
        assert!(!point_in_polygon(5.0, 5.0, "garbage data"), "unparseable");
    }

    #[test]
    fn encode_geohash_known_answer() {
        // Canonical example coordinate (Jutland) → "u4pruydqqvj".
        assert_eq!(encode_geohash(57.64911, 10.40744, 11), "u4pruydqqvj");
        // Lower precision is a prefix of higher precision for the same point.
        assert_eq!(encode_geohash(57.64911, 10.40744, 6), "u4pruy");
    }

    #[test]
    fn encode_geohash_length_matches_precision() {
        for precision in [1, 5, 6, 9, 12] {
            assert_eq!(encode_geohash(35.68, 139.65, precision).len(), precision);
        }
    }

    #[test]
    fn eccc_office_codes_map_provinces() {
        assert!(
            get_eccc_office_codes(43.65, -79.38).contains(&"CWTO"),
            "Toronto -> Ontario"
        );
        assert!(
            get_eccc_office_codes(53.55, -113.49).contains(&"CWNT"),
            "Edmonton -> Alberta"
        );
        assert!(
            get_eccc_office_codes(49.90, -97.14).contains(&"CWWG"),
            "Winnipeg -> Sask/Man"
        );
        assert!(
            get_eccc_office_codes(49.28, -123.12).contains(&"CWVR"),
            "Vancouver -> Pacific"
        );
        assert!(
            get_eccc_office_codes(44.65, -63.57).contains(&"CWHX"),
            "Halifax -> Atlantic"
        );
    }

    #[test]
    fn eccc_office_codes_fallback_to_ontario() {
        // Northern Ungava (58N, 70W) sits in a gap between the province bands
        // (too far north for Quebec's lat<55, west of the Atlantic cutoff),
        // so it exercises the empty-match fallback to CWTO.
        assert_eq!(get_eccc_office_codes(58.0, -70.0), vec!["CWTO"]);
    }

    #[test]
    fn get_meteoalarm_info_maps_every_covered_code() {
        // Every arm of the match, so a dropped country shows up here.
        for (code, slug, prefix) in [
            ("at", "austria", "AT"),
            ("ba", "bosnia-herzegovina", "BA"),
            ("be", "belgium", "BE"),
            ("bg", "bulgaria", "BG"),
            ("ch", "switzerland", "CH"),
            ("cy", "cyprus", "CY"),
            ("cz", "czechia", "CZ"),
            ("de", "germany", "DE"),
            ("dk", "denmark", "DK"),
            ("ee", "estonia", "EE"),
            ("es", "spain", "ES"),
            ("fi", "finland", "FI"),
            ("fr", "france", "FR"),
            ("gb", "united-kingdom", "UK"),
            ("gr", "greece", "GR"),
            ("hr", "croatia", "HR"),
            ("hu", "hungary", "HU"),
            ("ie", "ireland", "IE"),
            ("il", "israel", "IL"),
            ("is", "iceland", "IS"),
            ("it", "italy", "IT"),
            ("lt", "lithuania", "LT"),
            ("lu", "luxembourg", "LU"),
            ("lv", "latvia", "LV"),
            ("md", "moldova", "MD"),
            ("me", "montenegro", "ME"),
            ("mk", "north-macedonia", "MK"),
            ("mt", "malta", "MT"),
            ("nl", "netherlands", "NL"),
            ("no", "norway", "NO"),
            ("pl", "poland", "PL"),
            ("pt", "portugal", "PT"),
            ("ro", "romania", "RO"),
            ("rs", "serbia", "RS"),
            ("se", "sweden", "SE"),
            ("si", "slovenia", "SI"),
            ("sk", "slovakia", "SK"),
        ] {
            assert_eq!(get_meteoalarm_info(code), Some((slug, prefix)), "{code}");
        }
    }

    #[test]
    fn get_meteoalarm_info_case_insensitive() {
        // Proves the .to_lowercase() normalization at the top of the match.
        // Nominatim sends lowercase, but nothing in the type system says so.
        assert_eq!(get_meteoalarm_info("pl"), Some(("poland", "PL")));
        assert_eq!(get_meteoalarm_info("PL"), Some(("poland", "PL")));
        assert_eq!(get_meteoalarm_info("Pl"), Some(("poland", "PL")));
    }

    #[test]
    fn get_meteoalarm_info_uk_prefix_is_not_its_iso_code() {
        // The one country where the EMMA_ID prefix and the ISO code differ, so
        // "uppercase the code" is not a valid shortcut for this table.
        assert_eq!(get_meteoalarm_info("gb"), Some(("united-kingdom", "UK")));
        assert_eq!(get_meteoalarm_info("uk"), None);
    }

    #[test]
    fn get_meteoalarm_info_unknown_returns_none() {
        // Proves the `_ => None` fallback for non-covered countries.
        assert_eq!(get_meteoalarm_info("us"), None);
        assert_eq!(get_meteoalarm_info("jp"), None);
        assert_eq!(get_meteoalarm_info(""), None);
        assert_eq!(get_meteoalarm_info("not-a-code"), None);
    }

    #[test]
    fn detect_region_us_upper_midwest_and_maine_bands() {
        // Upper Midwest band: -95.0..-84.0 lon, lat < 46.5.
        assert_eq!(detect_region(44.98, -93.27), Region::Us, "Minneapolis, MN");
        // Northeast band (-76.0..-67.0), kept for band-parity with existing tests.
        assert_eq!(detect_region(44.30, -69.78), Region::Us, "Augusta, ME");
        // Maine else-arm band: lon -67.0..-66.0.
        assert_eq!(detect_region(45.20, -66.50), Region::Us, "Eastport, ME");
    }

    #[test]
    fn address_from_json_extracts_local_names_and_country_code() {
        // A real Nominatim reply for Warsaw with no accept-language requested.
        let body = r#"{"address":{"country":"Polska","country_code":"pl","city":"Warszawa","state":"wojew\u00f3dztwo mazowieckie"}}"#;
        let address = address_from_json(body).unwrap();
        assert_eq!(address.country_code.as_deref(), Some("pl"));
        assert_eq!(address.country.as_deref(), Some("Polska"));
        assert_eq!(address.city.as_deref(), Some("Warszawa"));
        assert_eq!(
            address.state.as_deref(),
            Some("wojew\u{f3}dztwo mazowieckie")
        );
        assert_eq!(address.town, None);
    }

    #[test]
    fn address_from_json_reads_village_and_municipality() {
        // Lisbon and Athens replies carry these alongside city; small places
        // carry them instead of it.
        let body = r#"{"address":{"country_code":"pt","city":"Lisboa","village":"Arroios","municipality":"Lisboa","county":"Lisboa"}}"#;
        let address = address_from_json(body).unwrap();
        assert_eq!(address.village.as_deref(), Some("Arroios"));
        assert_eq!(address.municipality.as_deref(), Some("Lisboa"));
    }

    #[test]
    fn address_from_json_country_code_matches_meteoalarm_keys() {
        // The decoded country_code must be a key get_meteoalarm_info recognises,
        // whatever language the country name came back in.
        for (body, expected) in [
            (r#"{"address":{"country_code":"pl"}}"#, ("poland", "PL")),
            (r#"{"address":{"country_code":"cz"}}"#, ("czechia", "CZ")),
            (
                r#"{"address":{"country_code":"gb"}}"#,
                ("united-kingdom", "UK"),
            ),
        ] {
            let address = address_from_json(body).unwrap();
            assert_eq!(
                get_meteoalarm_info(address.country_code.as_deref().unwrap()),
                Some(expected)
            );
        }
    }

    #[test]
    fn address_from_json_missing_address_is_location_detection_error() {
        assert!(matches!(
            address_from_json(r#"{"error":"Unable to geocode"}"#),
            Err(Error::LocationDetection)
        ));
    }

    #[test]
    fn address_from_json_malformed_is_parse_error() {
        assert!(matches!(
            address_from_json("not json"),
            Err(Error::Parse(ParseKind::Json))
        ));
    }

    // The cache tests share one process-wide static and run in parallel, so
    // each uses a key no other test touches.

    #[test]
    fn cache_key_matches_url_formatting() {
        // The key must be the exact strings the request URL is built from, so
        // that a cache hit is a repeated query and nothing looser.
        let (lat, lon) = (52.232, 21.0067);
        let url = format!(
            "https://nominatim.openstreetmap.org/reverse?lat={}&lon={}&format=json",
            lat, lon
        );
        assert_eq!(cache_key(lat, lon), "52.232,21.0067");
        assert!(url.contains("lat=52.232&lon=21.0067"));
    }

    #[test]
    fn cached_address_misses_then_hits() {
        let key = "test-miss-then-hit";
        assert!(cached_address(key).is_none());

        let body = r#"{"address":{"country":"Polska","country_code":"pl","city":"Warszawa"}}"#;
        cache_address(key, &address_from_json(body).unwrap());

        let hit = cached_address(key).expect("cached after a successful decode");
        assert_eq!(hit.country_code.as_deref(), Some("pl"));
        assert_eq!(hit.city.as_deref(), Some("Warszawa"));
    }

    #[test]
    fn cache_address_last_write_wins() {
        // No TTL and no first-wins guard: a later successful lookup replaces
        // the earlier one, same as STATION_CACHE.
        let key = "test-last-write-wins";
        cache_address(
            key,
            &address_from_json(r#"{"address":{"country_code":"pl"}}"#).unwrap(),
        );
        cache_address(
            key,
            &address_from_json(r#"{"address":{"country_code":"cz"}}"#).unwrap(),
        );
        assert_eq!(
            cached_address(key).unwrap().country_code.as_deref(),
            Some("cz")
        );
    }
}
