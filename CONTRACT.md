# Weathervane Wire Contract (v1)

This document freezes the JSON shapes that cross process boundaries: future
`weathervaned` D-Bus payloads, CLI output, and anything else that serializes
the public types. From the commit that introduces this file, **changing any
shape documented here is a breaking change.**

Enforcement: `tests/wire_contract.rs` — the committed snapshots under
`tests/snapshots/` are the canonical examples; this document quotes them.
Rust API reference: `API.md`.

## Conventions

- **Style:** serde defaults. snake_case field names, PascalCase enum variant
  strings. There are no `rename_all` attributes; new types conform by doing
  nothing.
- **Exception (the only one):** `AqiCategory` is adjacent-tagged:
  `{"standard": "Us", "level": "Good"}`. Standards vocabulary: `"Us"`, `"Eu"`.
- **Option policy:** documented keys are always present; `null` means no data.
  Absent keys never carry meaning.
- **Missing strings** are `""` (e.g. `Alert.description` for providers that
  send none, `LocationResult.country` when geocoding omits it) — not null.
- **Timestamps:** alert times are UTC instants, RFC3339 with `Z`
  (`"2026-06-11T22:00:00Z"`). Forecast/hourly times are location-local
  wall-clock naive ISO strings (`"2026-06-11T10:00"`); convert with the
  top-level `utc_offset_seconds`. Rule of thumb: *alerts = instants,
  forecasts = what a wall clock at the location reads.*
- **Versioning:** D-Bus payload versioning is the interface name
  (`dev.jcrenshaw.Weathervane1`); a breaking change ships `Weathervane2`
  alongside. CLI output carries `"format_version": 1`. JSON shapes are frozen
  as of this document; the `Weathervane1` interface itself freezes after the
  dogfood milestone.
- **PII rule:** error payloads (`kind`, `message`) never contain search query
  text or coordinates. Enforced by tests.

## Payloads

### WeatherData
```json
{
  "current": {
    "temperature": 71.3,
    "weathercode": 2,
    "condition": "PartlyCloudy",
    "windspeed": 8.5,
    "humidity": 54,
    "feels_like": 69.8,
    "wind_direction": 305,
    "compass_direction": "NW",
    "wind_gusts": 12.4,
    "uv_index": 6.5,
    "visibility": 24135.0,
    "pressure": 1015.2,
    "cloud_cover": 40,
    "dew_point": 52.7
  },
  "hourly": [
    {
      "time": "2026-06-11T10:00",
      "temperature": 71.3,
      "weathercode": 2,
      "condition": "PartlyCloudy",
      "precipitation_probability": 20,
      "precipitation": 0.1,
      "windspeed": 8.5,
      "wind_gusts": 12.4
    }
  ],
  "forecast": [
    {
      "date": "2026-06-11",
      "temp_max": 78.4,
      "temp_min": 58.1,
      "weathercode": 3,
      "condition": "Overcast",
      "sunrise": "2026-06-11T05:21",
      "sunset": "2026-06-11T20:29"
    }
  ],
  "utc_offset_seconds": -14400
}
```

### AirQualityData
US standard:
```json
{
  "aqi": 42,
  "category": {
    "standard": "Us",
    "level": "Good"
  },
  "pm2_5": 8.1,
  "pm10": 14.9,
  "ozone": 61.3,
  "nitrogen_dioxide": 9.4,
  "carbon_monoxide": 142.0
}
```

EU standard:
```json
{
  "aqi": 35,
  "category": {
    "standard": "Eu",
    "level": "Fair"
  },
  "pm2_5": 8.1,
  "pm10": 14.9,
  "ozone": 61.3,
  "nitrogen_dioxide": 9.4,
  "carbon_monoxide": 142.0
}
```

### Alert (array element of the alerts payload)
```json
{
  "id": "NWS-IDP-PROD-123",
  "event": "Severe Thunderstorm Warning",
  "severity": "Severe",
  "headline": "Severe thunderstorm until 10 PM EDT",
  "description": "Wind gusts to 60 mph expected.",
  "expires": "2026-06-11T22:00:00Z"
}
```

### PollenData
Inside CAMS European coverage:
```json
{
  "alder": 0.0,
  "birch": 12.4,
  "grass": 3.1,
  "mugwort": 0.0,
  "olive": 0.0,
  "ragweed": 0.7
}
```

Outside coverage (US, Asia, …) the value is `null` — present, never absent,
never zero-filled.

### LocationResult (array element of search results)
```json
{
  "latitude": 45.5152,
  "longitude": -122.6784,
  "display_name": "Portland, Oregon, United States",
  "country": "United States"
}
```

### DetectedLocation
```json
{
  "latitude": 45.52,
  "longitude": -122.68,
  "display_name": "Portland, United States",
  "country": "United States"
}
```

### SavedLocation
```json
{
  "name": "Home",
  "latitude": 45.5152,
  "longitude": -122.6784
}
```

## Errors

### WireError

`{"kind": ..., "message": ...}` — `kind` is the machine-matchable field, one of:
`Timeout`, `Network`, `HttpStatus`, `Parse`, `HttpClient`, `NoResults`,
`LocationDetection`, `Dbus`. Sub-detail (network kind, HTTP status code) rides
in `message`. Example:
```json
{
  "kind": "HttpStatus",
  "message": "http status 429"
}
```

### D-Bus method errors (request layer)

Methods fail with native D-Bus errors named
`dev.jcrenshaw.Weathervane1.Error.<Kind>` using the same kind vocabulary:

```
dev.jcrenshaw.Weathervane1.Error.Timeout
dev.jcrenshaw.Weathervane1.Error.Network
dev.jcrenshaw.Weathervane1.Error.HttpStatus
dev.jcrenshaw.Weathervane1.Error.Parse
dev.jcrenshaw.Weathervane1.Error.HttpClient
dev.jcrenshaw.Weathervane1.Error.NoResults
dev.jcrenshaw.Weathervane1.Error.LocationDetection
dev.jcrenshaw.Weathervane1.Error.Dbus
```

The error message is the `WireError.message` text.

## State layer (daemon properties)

Each domain (weather, air quality, alerts, pollen) is one property holding an
`Envelope`:

- `data` — last-good payload; never wiped by a failed refresh; `null` only
  before the first successful fetch.
- `fetched_at` — when `data` was obtained (UTC RFC3339); `null` iff `data` is.
- `error` — most recent attempt's failure (`kind`/`message`/`at`); `null` when
  the last attempt succeeded.

Healthy:
```json
{
  "data": {
    "current": {
      "temperature": 71.3,
      "weathercode": 2,
      "condition": "PartlyCloudy",
      "windspeed": 8.5,
      "humidity": 54,
      "feels_like": 69.8,
      "wind_direction": 305,
      "compass_direction": "NW",
      "wind_gusts": 12.4,
      "uv_index": 6.5,
      "visibility": 24135.0,
      "pressure": 1015.2,
      "cloud_cover": 40,
      "dew_point": 52.7
    },
    "hourly": [
      {
        "time": "2026-06-11T10:00",
        "temperature": 71.3,
        "weathercode": 2,
        "condition": "PartlyCloudy",
        "precipitation_probability": 20,
        "precipitation": 0.1,
        "windspeed": 8.5,
        "wind_gusts": 12.4
      }
    ],
    "forecast": [
      {
        "date": "2026-06-11",
        "temp_max": 78.4,
        "temp_min": 58.1,
        "weathercode": 3,
        "condition": "Overcast",
        "sunrise": "2026-06-11T05:21",
        "sunset": "2026-06-11T20:29"
      }
    ],
    "utc_offset_seconds": -14400
  },
  "fetched_at": "2026-06-11T13:50:00Z",
  "error": null
}
```

Stale (upstream failing, last-good retained):
```json
{
  "data": {
    "current": {
      "temperature": 71.3,
      "weathercode": 2,
      "condition": "PartlyCloudy",
      "windspeed": 8.5,
      "humidity": 54,
      "feels_like": 69.8,
      "wind_direction": 305,
      "compass_direction": "NW",
      "wind_gusts": 12.4,
      "uv_index": 6.5,
      "visibility": 24135.0,
      "pressure": 1015.2,
      "cloud_cover": 40,
      "dew_point": 52.7
    },
    "hourly": [
      {
        "time": "2026-06-11T10:00",
        "temperature": 71.3,
        "weathercode": 2,
        "condition": "PartlyCloudy",
        "precipitation_probability": 20,
        "precipitation": 0.1,
        "windspeed": 8.5,
        "wind_gusts": 12.4
      }
    ],
    "forecast": [
      {
        "date": "2026-06-11",
        "temp_max": 78.4,
        "temp_min": 58.1,
        "weathercode": 3,
        "condition": "Overcast",
        "sunrise": "2026-06-11T05:21",
        "sunset": "2026-06-11T20:29"
      }
    ],
    "utc_offset_seconds": -14400
  },
  "fetched_at": "2026-06-11T13:50:00Z",
  "error": {
    "kind": "Timeout",
    "message": "request timed out",
    "at": "2026-06-11T14:20:00Z"
  }
}
```

Never fetched:
```json
{
  "data": null,
  "fetched_at": null,
  "error": {
    "kind": "Network",
    "message": "network error: connect",
    "at": "2026-06-11T14:20:00Z"
  }
}
```

## CLI output

stdout on success:

```json
{ "format_version": 1, "data": { ... }, "fetched_at": "2026-06-11T13:50:00Z" }
```

stderr on failure (nonzero exit): a `WireError` object.
