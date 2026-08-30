# weathervane

[![CI](https://github.com/crenshawdev/weathervane/actions/workflows/ci.yml/badge.svg)](https://github.com/crenshawdev/weathervane/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/weathervane)](https://crates.io/crates/weathervane)
[![Crates.io downloads](https://img.shields.io/crates/d/weathervane?logo=rust)](https://crates.io/crates/weathervane)
[![docs.rs](https://docs.rs/weathervane/badge.svg)](https://docs.rs/weathervane)
[![Latest release](https://img.shields.io/github/v/release/crenshawdev/weathervane?label=release)](https://github.com/crenshawdev/weathervane/releases/latest)
[![License](https://img.shields.io/crates/l/weathervane)](LICENSE-APACHE)

Weather data, air quality, and alerts from public APIs. You call the async functions, you get back clean Rust types. That's it.

No UI dependencies. No polling. No timers. No config storage. The frontend owns all of that. This crate fetches data, parses responses, handles unit conversions, and gets out of the way.

## Data sources

**Weather and air quality** come from [Open-Meteo](https://open-meteo.com/) by default, which is free and doesn't require an API key.

**Japan** gets a targeted upgrade. When coordinates fall inside Japan, the current temperature is swapped for the nearest JMA [AMeDAS](https://www.jma.go.jp/bosai/amedas/) station reading. Open-Meteo's blended model runs a few degrees cold against ground truth in Japan, so the surgical override brings the current-condition number in line without disturbing anything else.

**Air quality** uses [aqicn.org](https://aqicn.org/) (World Air Quality Index Project) for the headline AQI when a token is provided and the coordinates fall outside Europe. Pollutant concentrations (PM2.5, PM10, ozone, etc) always come from Open-Meteo so the µg/m³ contract stays honest. Europe stays on Open-Meteo so the European AQI scale and its categories are preserved. Free aqicn tokens are issued at [aqicn.org/data-platform/token/](https://aqicn.org/data-platform/token/). Pass `None` to skip aqicn entirely.

Using aqicn requires attribution and is restricted to free-software and non-commercial use. Read their [data platform terms](https://aqicn.org/data-platform/token/) before building on top of it. This crate is not affiliated with JMA or with the World Air Quality Index Project.

**Pollen** comes from Open-Meteo's CAMS European model. Coverage is Europe only, six species in grains per cubic meter: alder, birch, grass, mugwort, olive, ragweed. Coordinates outside Europe return `None`. No free public pollen API covers the US or Asia at the moment, which is why there's no token-gated worldwide upgrade the way air quality has aqicn.

**Alerts** are pulled from whichever provider covers the user's location:

| Region | Provider |
|--------|----------|
| United States | NWS (National Weather Service) |
| Canada | ECCC (Environment and Climate Change Canada) |
| Europe | MeteoAlarm |
| Australia | BOM (Bureau of Meteorology) |

Region detection is automatic based on coordinates. Everywhere else gets weather and air quality but no alerts. South Korea stays on Open-Meteo for both weather and air quality. KMA requires a national-ID-gated key, which is not a useful integration path for an open source crate.

## Usage

```toml
[dependencies]
weathervane = "0.10"
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

Air quality automatically picks the right AQI standard (US EPA or European) based on where the coordinates land. Pass an aqicn token to get ground-station data worldwide outside Europe, or `None` to stay on Open-Meteo everywhere.

```rust
use weathervane::fetch_air_quality;

let aqi = fetch_air_quality(48.8566, 2.3522, None).await?;
println!("AQI: {} ({:?})", aqi.aqi, aqi.category);

// With a user-supplied token for ground-station AQI:
let aqi = fetch_air_quality(35.68, 139.69, Some("your-token-here")).await?;
```

Pollen returns `None` outside Europe. Inside coverage, all six species are populated and species that aren't in season read as zero.

```rust
use weathervane::fetch_pollen;

let pollen = fetch_pollen(48.2082, 16.3738).await?;
if let Some(p) = pollen {
    println!("Grass: {} grains/m³", p.grass);
}
```

## What's in the box

**Weather**: current conditions, 12-hour hourly forecast, 7-day daily forecast. Temperature, humidity, wind, pressure, UV index, visibility, cloud cover, dew point.

**Air quality**: AQI with region-appropriate categorization, PM2.5, PM10, ozone, NO2, CO.

**Pollen**: current grains per cubic meter for alder, birch, grass, mugwort, olive, and ragweed. Europe only.

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
