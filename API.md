# tempest-core API

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
pub async fn fetch_air_quality(latitude: f64, longitude: f64) -> Result<AirQualityData>
```

Fetches from Open-Meteo Air Quality API. Automatically picks US or European AQI
based on `detect_region()`.

Returns `AirQualityData`:
- `aqi: i32` -- the raw index value
- `standard: AqiStandard` -- `Us` or `European`
- `category: AqiCategory` -- `Us(UsAqiCategory)` or `Eu(EuAqiCategory)`, computed during fetch
- Pollutant readings: `pm2_5`, `pm10`, `ozone`, `nitrogen_dioxide`, `carbon_monoxide`

### AQI Categories

```rust
pub enum UsAqiCategory { Good, Moderate, UnhealthySensitive, Unhealthy, VeryUnhealthy, Hazardous }
pub enum EuAqiCategory { Good, Fair, Moderate, Poor, VeryPoor, ExtremelyPoor }
pub enum AqiCategory { Us(UsAqiCategory), Eu(EuAqiCategory) }
```

Match on these for translations. `UsAqiCategory::from_aqi(i32)` and
`EuAqiCategory::from_aqi(i32)` are public if you need to categorize values yourself.

## Weather Alerts

### `fetch_alerts`

```rust
pub async fn fetch_alerts(latitude: f64, longitude: f64) -> Result<Vec<Alert>>
```

Dispatches to the right provider based on location:

| Region    | Provider   | Notes                                          |
|-----------|------------|-------------------------------------------------|
| US        | NWS API    | GeoJSON point query                             |
| Europe    | MeteoAlarm | Atom feeds, EMMA_ID matching via Nominatim      |
| Canada    | ECCC       | CAP XML from `dd.weather.gc.ca`, polygon filter  |
| Australia | BOM API    | Geohash lookup                                  |
| Other     | --         | Returns empty vec                                |

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
`display_name`, and `country`. Errors with `TempestError::LocationDetection` on failure.

### `search_city`

```rust
pub async fn search_city(city_name: &str) -> Result<Vec<LocationResult>>
```

Geocoding search via Open-Meteo. Returns up to 10 results, each with coordinates,
display name, and country. Errors with `TempestError::NoResults` if nothing comes back.

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
pub enum TempestError {
    Http(reqwest::Error),       // network or HTTP failure
    HttpClient(String),         // couldn't build the HTTP client
    NoResults { query: String },// geocoding search returned nothing
    LocationDetection,          // IP geolocation failed
    Xml(quick_xml::DeError),    // XML parsing failure (MeteoAlarm/ECCC feeds)
    Dbus(String),               // D-Bus connection failure
}

pub type Result<T> = std::result::Result<T, TempestError>;
```

All public functions return `Result<T>` from this crate. `Http` and `Xml` have
`#[from]` impls so `?` works naturally with reqwest and quick-xml errors.
