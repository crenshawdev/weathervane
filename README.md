# weathervane

[![crates.io](https://img.shields.io/crates/v/weathervane)](https://crates.io/crates/weathervane)
[![docs.rs](https://docs.rs/weathervane/badge.svg)](https://docs.rs/weathervane)

Weather data, air quality, and alerts from public APIs. You call the async functions, you get back clean Rust types. That's it.

No UI dependencies. No polling. No timers. No config storage. The frontend owns all of that. This crate fetches data, parses responses, handles unit conversions, and gets out of the way.

## Data sources

**Weather and air quality** come from [Open-Meteo](https://open-meteo.com/), which is free and doesn't require an API key.

**Alerts** are pulled from whichever provider covers the user's location:

| Region | Provider |
|--------|----------|
| United States | NWS (National Weather Service) |
| Canada | ECCC (Environment and Climate Change Canada) |
| Europe | MeteoAlarm |
| Australia | BOM (Bureau of Meteorology) |

Region detection is automatic based on coordinates. Everywhere else gets weather and air quality but no alerts.

## Usage

```toml
[dependencies]
weathervane = "0.2"
```

```rust
use weathervane::{fetch_weather, TemperatureUnit, MeasurementSystem};

let weather = fetch_weather(
    40.7128, -74.0060,
    TemperatureUnit::Fahrenheit,
    MeasurementSystem::Imperial,
).await?;

println!("{:?}", weather.current.condition);
```

Location detection works off the user's IP. Not precise, but good enough for weather.

```rust
use weathervane::detect_location;

let location = detect_location().await?;
println!("{} ({}, {})", location.display_name, location.latitude, location.longitude);
```

Air quality automatically picks the right AQI standard (US EPA or European) based on where the coordinates land.

```rust
use weathervane::fetch_air_quality;

let aqi = fetch_air_quality(48.8566, 2.3522).await?;
println!("AQI: {} ({:?})", aqi.aqi, aqi.category);
```

## What's in the box

**Weather**: current conditions, 12-hour hourly forecast, 7-day daily forecast. Temperature, humidity, wind, pressure, UV index, visibility, cloud cover, dew point.

**Air quality**: AQI with region-appropriate categorization, PM2.5, PM10, ozone, NO2, CO.

**Alerts**: severity, headline, description, expiry. Normalized across all four providers into a single `Alert` type.

**Units**: temperature (F/C), pressure (hPa/inHg/PSI), measurement system (imperial/metric). Pass the types in, get formatted values out.

**Location**: city search via geocoding, IP-based auto-detection, saved location bookmarks.

**Network**: monitors NetworkManager over D-Bus, yields an event when connectivity comes back. Linux only, degrades gracefully elsewhere.

**Sleep**: monitors systemd-logind for resume from suspend, yields an event when the system wakes. Linux only, degrades gracefully elsewhere.

## Building

```sh
cargo build
cargo clippy
cargo doc --open
```

## License

MIT or Apache-2.0, your choice.
