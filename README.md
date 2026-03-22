# weathervane

Weather data, air quality, and alerts from public APIs. Fetches, parses, and returns clean Rust types.

A standalone Rust library crate that handles data fetching, response parsing, unit conversions, geographic detection, and weather domain types. No UI dependencies, no polling, no timers, no config storage. Frontends call the async functions when they're ready and pass in whatever parameters they need.

## What it does

- Fetches weather data, air quality, and alerts from Open-Meteo, NWS, MeteoAlarm, ECCC, and BOM
- IP-based location detection and city search via geocoding
- Unit types (temperature, pressure, measurement system) with conversions
- Weather condition and compass direction enums for frontend translation mapping
- Network connectivity monitoring via NetworkManager D-Bus signals
- Time formatting and date parsing utilities

## Usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
weathervane = "0.1"
```

```rust
use weathervane::{fetch_weather, TemperatureUnit, MeasurementSystem};

let weather = fetch_weather(40.7128, -74.0060, TemperatureUnit::Fahrenheit, MeasurementSystem::Imperial).await?;
println!("{:?}", weather.current.condition);
```

## Building

```sh
cargo build
cargo clippy
cargo test
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
