# weathervane API

Everything here is async. No polling, no timers, no background threads. The crate
does the work when you ask it to and gives you back the result. Your frontend decides
when to call, how often to refresh, and where to store preferences.

The crate never touches i18n. Where the old applet returned translated strings,
this returns enums. You match on the enum and produce whatever localized string your
UI needs.

## Weather

### `fetch_weather`

```rust
pub async fn fetch_weather(
    latitude: f64,
    longitude: f64,
    temperature_unit: TemperatureUnit,
    measurement_system: MeasurementSystem,
) -> Result<WeatherData>
```

Hits the Open-Meteo API and returns current conditions, 7-day forecast, and 24
hours of hourly data. Pass real types, not strings. The crate handles the wire
format internally via `api_param()`.

For coordinates inside Japan, the current temperature is replaced with the
nearest JMA AMeDAS station's reading (converted to the requested unit). The
override is a quality upgrade, not a dependency: if the station table, latest
observation timestamp, or map fetch fails, or the nearest station is further
than 50km, the call falls through to Open-Meteo's value without surfacing an
error. Forecast and hourly data are unaffected.

Returns `WeatherData`:
- `current: CurrentWeather` -- temperature, humidity, feels-like, wind speed/direction/gusts, UV index, visibility, pressure, cloud cover, dew point, weathercode, and pre-computed `condition` and `compass_direction`
- `hourly: Vec<HourlyForecast>` -- up to 12 entries with time, temperature, weathercode, condition, precipitation probability
- `forecast: Vec<DailyForecast>` -- 7 entries with date, high/low temps, weathercode, condition, sunrise/sunset times

Every struct that has a `weathercode: i32` also has a `condition: WeatherCondition`
computed from it. The raw code stays for anyone who wants it.

`CurrentWeather` also carries `compass_direction: CompassDirection` pre-computed
from `wind_direction`.

## Air Quality

### `fetch_air_quality`

```rust
pub async fn fetch_air_quality(
    latitude: f64,
    longitude: f64,
    aqicn_token: Option<&str>,
) -> Result<AirQualityData>
```

Pollutant concentrations always come from Open-Meteo's Air Quality API, so
`pm2_5`, `pm10`, `ozone`, `nitrogen_dioxide`, and `carbon_monoxide` stay in
µg/m³ regardless of which source provided the headline AQI.

The headline `aqi` and its `category` come from the World Air
Quality Index Project (aqicn.org) when a token is supplied and the coordinates
are outside Europe. aqicn reports on the US EPA scale globally and sources
from ground monitoring stations, which reads closer to truth in East Asia
than Open-Meteo's satellite-derived numbers. Pass `None` to skip aqicn and use
Open-Meteo everywhere.

Europe stays on Open-Meteo for the headline AQI even when a token is passed,
so `standard()` still reports `AqiStandard::European`. Users in Paris
or Berlin get the European scale, not a US-scale number dressed up as
European.

If aqicn is selected but the request fails, the token is rejected, or the
response is missing `data.aqi`, the call falls back to Open-Meteo's value
without surfacing an error.

Using aqicn requires attribution and is restricted to free-software and
non-commercial use. See their [data platform terms](https://aqicn.org/data-platform/token/).

Returns `AirQualityData`:
- `aqi: i32` -- the raw index value
- `category: AqiCategory` -- `Us(UsAqiCategory)` or `Eu(EuAqiCategory)`, computed during fetch
- `standard()` -- method returning `AqiStandard` (`Us` or `European`), derived from `category`
- Pollutant readings: `pm2_5`, `pm10`, `ozone`, `nitrogen_dioxide`, `carbon_monoxide` (µg/m³, Open-Meteo)
- `aqi_source: AqiSource` -- which provider supplied the headline `aqi`/`category` for this fetch

### AqiSource

```rust
pub enum AqiSource { Aqicn, OpenMeteo }
```

Records which provider actually supplied the headline AQI:

- `Aqicn` -- aqicn.org (World Air Quality Index Project), US EPA scale; selected
  when a token is passed and the coordinates are outside Europe and the request
  succeeded.
- `OpenMeteo` -- the default. Used for Europe, when no token is passed, or when
  aqicn was selected but failed and the call fell back to Open-Meteo.

`OpenMeteo` is the `Default`. The field is `#[serde(default)]`, so payloads
serialized before this field existed deserialize as `OpenMeteo`.

### AQI Categories

```rust
pub enum UsAqiCategory { Good, Moderate, UnhealthySensitive, Unhealthy, VeryUnhealthy, Hazardous }
pub enum EuAqiCategory { Good, Fair, Moderate, Poor, VeryPoor, ExtremelyPoor }
pub enum AqiCategory { Us(UsAqiCategory), Eu(EuAqiCategory) }
```

Match on these for translations. `UsAqiCategory::from_aqi(i32)` and
`EuAqiCategory::from_aqi(i32)` are public if you need to categorize values yourself.

`AqiCategory` serializes adjacent-tagged — `{"standard": "Us", "level": "Good"}`.
JSON shapes for all public types are frozen in [CONTRACT.md](CONTRACT.md).

## Weather Alerts

### `fetch_alerts_detailed`

```rust
pub async fn fetch_alerts_detailed(latitude: f64, longitude: f64) -> Result<AlertReport>

pub struct AlertReport {
    pub alerts: Vec<AlertEntry>,
    pub region_filtered: bool,
}

pub struct AlertEntry {
    pub alert: Alert,
    pub area_desc: String,
}
```

The same dispatch as `fetch_alerts` below, plus two things a consumer cannot
otherwise know:

- `area_desc` -- the provider's name for the area each entry covers. MeteoAlarm
  `cap:areaDesc`, NWS `areaDesc` (a `;`-separated county list), the containing
  ECCC polygon's `areaDesc`. BOM sends none, so it is `""`.
- `region_filtered` -- `false` only when a MeteoAlarm national feed was returned
  unfiltered because no region could be matched for the location, by EMMA_ID,
  by area name, or by local-language area name, so the entries are national,
  not local. NWS, ECCC and BOM filter by point, polygon and geohash, and an
  empty result is trivially filtered, so all of those are `true`.

MeteoAlarm filtering runs in up to three stages. First the location's EMMA_ID, resolved
from the codename list, filters a feed whose entries are EMMA_ID-tagged; this is
exact, and since the feed only lists regions that are alerting it is the only
stage that can tell a quiet day from a miss. When that produces no usable filter
(no EMMA_ID resolved, or the feed tags entries under another scheme, or not at
all), the location's place names are matched against the feed's own `areaDesc`
values: diacritics folded, administrative affixes such as "Grad" and "region"
dropped, exact match preferred, and a place name that fits more than one region
is treated as a miss rather than a guess. Third, only when that missed and a
place name is in non-Latin script (Greek, Cyrillic, Hebrew), the country's
MeteoAlarm JSON feed is read once for its local-language area names, each paired
with the English name the atom feed uses; the same matching runs over the local
names, with a small allowance for inflection (Nominatim's genitive "Αττικής"
meets the feed's "Αττική"), and a hit keeps the entries whose area is the paired
English name. Latin-script locations never make that request. When an EMMA_ID
did resolve against an EMMA_ID-tagged feed, entries carrying no geocode are
dropped rather than shown, since they cannot be placed in any region.

### `fetch_alerts`

```rust
pub async fn fetch_alerts(latitude: f64, longitude: f64) -> Result<Vec<Alert>>
```

Thin wrapper over `fetch_alerts_detailed` that drops `area_desc` and the
filtering flag. Dispatches to the right provider based on location:

| Region    | Provider   | Notes                                          |
|-----------|------------|-------------------------------------------------|
| US        | NWS API    | GeoJSON point query                             |
| Europe    | MeteoAlarm | Atom feeds, EMMA_ID matching via Nominatim      |
| Canada    | ECCC       | CAP XML from `dd.weather.gc.ca`, polygon filter  |
| Australia | BOM API    | Geohash lookup                                  |
| Other     | --         | Returns empty vec                                |

An empty vec means there are no active alerts, or the location has no covering
provider. Failure to determine the location's country is an `Err`
(`LocationDetection`), never an empty vec, so a consumer can tell a quiet day
from a failed lookup.

The Nominatim reverse geocode behind the European path is cached per coordinate
pair for the process lifetime, so only the first call for a location reaches
Nominatim; a consumer refreshing on a timer sends one request per location, not
one per refresh. Failed lookups are not cached and retry on the next call.

Each `Alert` has:
- `id: String` -- provider-specific identifier
- `event: String` -- event type name (e.g. "Winter Storm Warning")
- `severity: AlertSeverity` -- `Minor`, `Moderate`, `Severe`, `Extreme`, or `Unknown`
- `headline: String` -- short summary
- `description: String` -- full text (empty for MeteoAlarm and BOM, which don't provide it)
- `expires: DateTime<Utc>` -- when the alert expires

Expired alerts are filtered out before returning. ECCC alerts are deduplicated
by event type and area.

## Location

### `detect_location`

```rust
pub async fn detect_location() -> Result<DetectedLocation>
```

IP-based geolocation via ip-api.com. Returns a struct with `latitude`, `longitude`,
`display_name`, and `country`. Errors with `Error::LocationDetection` on failure.

### `search_city`

```rust
pub async fn search_city(city_name: &str) -> Result<Vec<LocationResult>>
```

Geocoding search via Open-Meteo. Returns up to 10 results, each with coordinates,
display name, and country. Errors with `Error::NoResults` if nothing comes back.

### `uses_imperial_units`

```rust
pub fn uses_imperial_units(country: &str) -> bool
```

Returns true for "United States", "Liberia", and "Myanmar". That's it. Those are
the three countries that officially use imperial.

### `SavedLocation`

```rust
pub struct SavedLocation {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}
```

A bookmarked location. Has `matches_coords(lat, lon)` for checking if a saved
location matches given coordinates within 0.01 degree tolerance.

## Units

### `TemperatureUnit`

Variants: `Fahrenheit` (default), `Celsius`.

Methods:
- `symbol()` -- returns `"°F"` or `"°C"`
- `api_param()` -- returns the Open-Meteo API parameter value
- `format(temp: f32)` -- formats like `"72°F"`

### `MeasurementSystem`

Variants: `Imperial` (default), `Metric`.

Methods:
- `wind_speed_unit()` -- `"mph"` or `"km/h"`
- `visibility_unit()` -- `"mi"` or `"km"`
- `wind_speed_api_param()` -- API parameter value
- `convert_visibility(meters: f32)` -- converts from meters to miles or km

### `PressureUnit`

Variants: `Hpa` (default), `InHg`, `Psi`.

Methods:
- `symbol()` -- unit label
- `convert(hpa: f32)` -- converts from hPa to target unit
- `format(hpa: f32)` -- converts and formats with unit label

## Condition and Direction Enums

### `WeatherCondition`

```rust
pub enum WeatherCondition {
    ClearSky, MainlyClear, PartlyCloudy, Overcast, Foggy,
    Drizzle, FreezingDrizzle, Rain, FreezingRain,
    Snow, SnowGrains, RainShowers, SnowShowers,
    Thunderstorm, ThunderstormHail, Unknown,
}
```

- `from_code(code: i32)` -- maps WMO weather codes to variants
- `icon_name(is_night: bool)` -- returns freedesktop icon name with `-symbolic` suffix

This replaces the old `weathercode_to_description` function. Your frontend matches
the variant against its own translation keys.

### `CompassDirection`

```rust
pub enum CompassDirection { N, NE, E, SE, S, SW, W, NW }
```

- `from_degrees(degrees: i32)` -- 8-point compass from bearing
- `as_str()` -- short label like `"NE"`

Replaces the old `wind_direction_to_compass` function.

## Time

### `format_hour`

```rust
pub fn format_hour(time_str: &str, military_time: bool) -> String
```

Parses an ISO timestamp and returns just the hour. `"14:00"` in military time,
`"2:00 PM"` otherwise. Falls back to the raw string if parsing fails.

### `format_time`

```rust
pub fn format_time(time_str: &str, military_time: bool) -> String
```

Same as `format_hour` but preserves minutes. `"14:30"` or `"2:30 PM"`.

### `is_night_time`

```rust
pub fn is_night_time(sunrise: &str, sunset: &str) -> bool
```

Returns true if the current local time is before sunrise or after sunset.
Falls back to 6pm-6am if parsing the times fails.

### `ParsedDate`

```rust
pub struct ParsedDate {
    pub weekday: chrono::Weekday,
    pub month: u32,
    pub day: u32,
    pub year: i32,
}
```

- `from_iso(date_str: &str)` -- parses `"2025-11-25"` into parts, returns `Option`

Frontend takes the weekday and month and maps them to translated names. Core
doesn't care what language Tuesday is in.

## Network

### `network_stream`

```rust
pub fn network_stream() -> Pin<Box<dyn Stream<Item = NetworkEvent> + Send>>
```

Returns an async stream that yields `NetworkEvent::Connected` when NetworkManager
reports full connectivity via D-Bus. If D-Bus or NetworkManager isn't available,
the stream just sits there forever without yielding.

Your frontend wraps this in whatever subscription model it uses. For iced, that
means converting it to a `Subscription`. For something else, consume it however
you want.

## Sleep

### `sleep_stream`

```rust
pub fn sleep_stream() -> Pin<Box<dyn Stream<Item = SleepEvent> + Send>>
```

Returns an async stream that yields `SleepEvent::Resumed` when the system
wakes from suspend, watching the `PrepareForSleep` signal on
`org.freedesktop.login1.Manager`. If the system bus or logind isn't
available, the stream just sits there forever without yielding.

Same wrapping pattern as `network_stream`. Your frontend converts it to
whatever subscription model it uses.

### `SleepEvent`

```rust
pub enum SleepEvent {
    Resumed,
}
```

## HTTP Client

### `reset_http_client`

```rust
pub fn reset_http_client()
```

Drops and rebuilds the internal `reqwest::Client`. Use this when you have
reason to believe the connection pool is poisoned — for example, after
the system wakes from suspend, when pooled TCP connections may be tied
to dead sockets the kernel hasn't reaped yet. The next request after
this call gets a fresh client with no carryover state.

The hardened defaults (TCP keepalive, zero idle pool, short idle
timeout) are preserved on rebuild.

## Geographic Detection

### `detect_region`

```rust
pub fn detect_region(lat: f64, lon: f64) -> Region
```

Returns `Region::Us`, `Region::Europe`, `Region::Canada`, `Region::Australia`, or
`Region::Unknown` based on bounding boxes. Used internally for alert provider
selection and AQI standard, but public if you need it.

The US bounding boxes respect the US-Canada border with regional specificity
(different lat cutoffs for the Pacific Northwest vs Great Lakes vs Northeast).

## Errors

```rust
pub enum Error {
    Timeout,
    Network(String),
    HttpStatus(u16),
    Parse(String),
    HttpClient(String),
    NoResults { query: String },
    LocationDetection,
    Dbus(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

All public functions return `Result<T>`. `From<reqwest::Error>` and `From<quick_xml::DeError>` route through `?` and bucket the failure into the right variant. Display impls are category-only, no URLs or query strings, safe to log.

`0.2.0` renamed from `TempestError` and split `Http`/`Xml` into the categories above.
