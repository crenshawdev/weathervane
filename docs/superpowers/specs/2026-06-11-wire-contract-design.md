# Weathervane Wire Contract — Design Spec

Date: 2026-06-11
Status: approved (design discussion 2026-06-11)
Scope: steps 1–2 of the multi-frontend architecture (serialization layer + wire
contract). Service module, daemon, and CLI implementation are out of scope but
every JSON shape they will emit is fixed here.

Cross-refs:
- Vault: `weathervane/weathervane-multi-frontend-architecture-daemon-cli-d-bus-contract.md`
  (architecture decision this spec implements)
- `CLAUDE.md` — app identifier namespace (`dev.jcrenshaw.Weathervane`,
  interface `dev.jcrenshaw.Weathervane1`)

## Goal

Freeze the JSON wire contract that the future daemon (`weathervaned`), CLI, and
all non-Rust frontends (GNOME extensions, KDE widgets, waybar modules) consume.
After this lands, JSON shape changes are breaking changes.

## Decisions (all confirmed 2026-06-11)

| # | Decision | Choice |
|---|----------|--------|
| 1 | Baseline style | serde defaults: snake_case fields, PascalCase variants, no `rename_all` |
| 2 | `AqiCategory` shape | adjacent-tagged `{"standard": "Us", "level": "Good"}`; drop sibling `standard` field |
| 3 | Option policy | keys always present, `null` = no data |
| 4 | Timestamps | keep mixed: alerts UTC RFC3339 `Z`; forecast times local wall-clock + `utc_offset_seconds` |
| 5 | Method errors | native D-Bus errors, one name per `Error` variant |
| 6 | State-layer staleness | envelope `{data, fetched_at, error}` per domain property |
| 7 | Version marker | D-Bus: interface name only; CLI: `"format_version": 1` envelope key |

## 1. Serialization conventions

- **Baseline is serde's default output.** snake_case field names, PascalCase
  enum variant strings (`"condition": "PartlyCloudy"`). No `#[serde(rename_all)]`
  anywhere. Rationale: tempest user configs already persist `TemperatureUnit`,
  `MeasurementSystem`, `PressureUnit`, and `SavedLocation` in exactly these
  spellings; the zero-attribute convention is the only one that enforces itself.
- **Per-type exceptions are documented in CONTRACT.md.** At freeze time there is
  exactly one: `AqiCategory` (§2.2).
- **Option fields/payloads:** always emit the key; `null` means no data.
  Concrete case: pollen outside CAMS European coverage serializes as `null`,
  never as an absent key and never as zero-filled data.
- **Missing string data stays `""`** (existing convention: `Alert.description`,
  `LocationResult.country` are `String`, not `Option<String>`). Documented, not
  changed.
- **Timestamps:**
  - Alert times are instants: `DateTime<Utc>` → RFC3339 with `Z`
    (`"2026-06-11T22:00:00Z"`).
  - Forecast/hourly/sunrise/sunset times are location-local wall-clock naive
    ISO strings (`"2026-06-11T10:00"`), with one top-level
    `utc_offset_seconds` on `WeatherData` for conversion.
  - CONTRACT.md states this as a rule: *alerts = UTC instants; forecasts =
    local wall clock.*

## 2. Type changes

### 2.1 Types gaining `Serialize + Deserialize`

Wire-crossing types only:

| Type | Module | Note |
|------|--------|------|
| `WeatherData` | weather.rs | nested types already have derives |
| `AirQualityData` | air_quality.rs | also loses `standard` field (§2.2) |
| `UsAqiCategory`, `EuAqiCategory` | air_quality.rs | plain unit-variant enums |
| `AqiCategory` | air_quality.rs | adjacent-tagged (§2.2) |
| `Alert`, `AlertSeverity` | alerts.rs | `expires: DateTime<Utc>` → RFC3339 via chrono serde |
| `PollenData` | pollen.rs | wire value is `PollenData | null` |
| `LocationResult` | location.rs | |
| `DetectedLocation` | location.rs | |

### 2.2 `AqiCategory` restructure (breaking → 0.7.0)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "standard", content = "level")]
pub enum AqiCategory {
    Us(UsAqiCategory),
    Eu(EuAqiCategory),
}
```

Wire form:

```json
{ "aqi": 42, "category": { "standard": "Us", "level": "Good" }, "pm2_5": 8.1 }
```

- `AirQualityData.standard` field is **removed**; replaced by
  `AirQualityData::standard(&self) -> AqiStandard` derived from the category
  variant.
- Wire vocabulary for standards is `"Us"` / `"Eu"` (the variant names).
  `AqiStandard::European` remains a Rust-side name only.
- Round-trips exactly (tag disambiguates the `Good`/`Moderate` collisions
  between scales) — safe for Rust D-Bus clients.
- Tempest impact: one line (`data.standard` → `data.standard()`), folded into
  the 0.5 → 0.7 upgrade it already owes.

### 2.3 Types deliberately NOT serializable

`Region`, `NetworkEvent`, `SleepEvent`, `AqiStandard`, `Error` (directly).
None cross the wire: `Region` is internal routing, the event streams are
consumed by the daemon itself, `AqiStandard` no longer appears in payloads
after §2.2. Deriving them would add types to the frozen contract with no
consumer. `Error` crosses the wire only via `WireError` (§2.4).

### 2.4 `WireError` — explicit error mapping, not a derive

```rust
/// JSON-facing error shape. Built from `Error`, never derived on it, so the
/// wire never carries `NoResults { query }` payload data (PII contract) and
/// the shape stays flat regardless of `Error`'s internal structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireError {
    /// `Error` variant name: "Timeout", "Network", "HttpStatus", "Parse",
    /// "HttpClient", "NoResults", "LocationDetection", "Dbus".
    pub kind: String,
    /// The `Display` output (already PII-clean, e.g. "http status 429",
    /// "network error: connect").
    pub message: String,
}

impl From<&Error> for WireError { /* variant name + Display */ }
```

Sub-kinds (`NetworkKind`, `ParseKind`) and HTTP status codes ride inside
`message` via the existing `Display` impls; `kind` is the machine-matchable
field.

**PII rule (contractual):** error `message`/`kind` never contain search query
text or coordinates. The current `Display` impls already satisfy this
(`NoResults` prints `"no results"` without the query); tests enforce it (§4.3).

## 3. Envelopes and transport

### 3.1 State layer (daemon properties; one per domain)

```json
{
  "data": { ... } | null,
  "fetched_at": "2026-06-11T13:50:00Z" | null,
  "error": { "kind": "Timeout", "message": "request timed out",
             "at": "2026-06-11T14:20:00Z" } | null
}
```

- `data` = last-good payload; **never wiped by a failed refresh**; `null` only
  before first successful fetch.
- `fetched_at` = when `data` was obtained (UTC RFC3339); `null` iff `data` is.
- `error` = most recent attempt's failure (`WireError` + `at`); `null` when the
  last attempt succeeded.
- The service module's cache type mirrors this 1:1
  (`Cached<T> { data: Option<T>, fetched_at: Option<DateTime<Utc>>, last_error: Option<(Error, DateTime<Utc>)> }`).

### 3.2 Request layer (daemon methods)

Native D-Bus errors, names frozen as part of the `Weathervane1` contract:

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

Error message = `Display` output. (Implemented later via zbus `DBusError`
derive; the *names* are fixed now.)

### 3.3 CLI

- stdout (success): `{ "format_version": 1, "data": ..., "fetched_at": ... }`
- stderr (failure) + nonzero exit: `WireError` JSON.
- D-Bus payloads carry **no** version field — the interface name is the
  version. A breaking change ships `Weathervane2` alongside `Weathervane1`.

## 4. Test coverage (enforcement of the freeze)

The freeze is enforced by CI, not memory. New `tests/` integration suite +
`insta` as the only new dev-dependency. All tests run under plain `cargo test`.

### 4.1 Snapshot tests — one per wire shape (`tests/wire_contract.rs`)

For **every** type in §2.1 plus `WireError` and the state envelope: construct a
fully-populated exemplar (no defaults, every field set to a distinctive value)
and snapshot `serde_json::to_string_pretty` via `insta::assert_snapshot!`.
Committed `.snap` files ARE the contract artifacts; any shape drift fails CI
with a readable diff and requires a deliberate `cargo insta review`.

Edge exemplars, each its own snapshot:
- `AqiCategory`: one `Us`, one `Eu` (tag/content shape both ways).
- Pollen: `Some(PollenData)` and the `null` case (serialized as
  `Option<PollenData>`).
- State envelope: (a) healthy `{data, fetched_at, error: null}`,
  (b) stale `{data, fetched_at, error: {...}}`, (c) never-fetched
  `{data: null, fetched_at: null, error: {...}}`.
- `WireError`: one exemplar per `Error` variant (8 snapshots — the variant
  names in `kind` are contract).
- `Alert.expires`: known `DateTime<Utc>` → asserts RFC3339 `Z` form.
- Enum spelling canaries: `WeatherCondition::PartlyCloudy`,
  `CompassDirection::NW`, `TemperatureUnit::Fahrenheit`,
  `AlertSeverity::Severe` — guards the PascalCase baseline explicitly.

### 4.2 Round-trip tests

For every `Serialize + Deserialize` type: `value → JSON → value` asserts
equality (`PartialEq` where present; otherwise re-serialize and compare
strings). Specifically proves the `AqiCategory` adjacent tag round-trips
`Eu(Good)` to `Eu(Good)` — the failure mode the untagged option would have had.

### 4.3 PII guard tests

- `WireError::from(&Error::NoResults { query: "SENTINEL_QUERY" })` →
  serialized JSON must not contain `SENTINEL_QUERY`.
- Each `WireError` exemplar's JSON must not match a lat/lon-like pattern for
  the sentinel coordinates used in test fixtures.

### 4.4 Tempest config-compat guards

Deserialization tests pinning the legacy persisted spellings (what tempest
configs already store):
- `"Fahrenheit"`, `"Imperial"`, `"Hpa"` etc. parse into the unit enums.
- A literal `SavedLocation` JSON (`{"name": ..., "latitude": ..., "longitude": ...}`)
  parses. These fail if anyone ever adds a `rename_all` by accident.

### 4.5 Convention conformance

A test walking every committed snapshot file asserting no camelCase JSON keys
(regex `"[a-z0-9_]*[A-Z][a-zA-Z0-9]*"\s*:` finding any key with an uppercase
letter) — cheap structural guard for the baseline rule. The only uppercase
strings in baseline payloads are enum *values* (PascalCase), which never appear
in key position.

## 5. CONTRACT.md (deliverable alongside the code)

In-repo `CONTRACT.md` documenting: conventions (§1), one full example payload
per wire type (lifted from the snapshots so doc and tests can't diverge),
envelope shapes, the D-Bus error name table, the PII rule, and the versioning
policy: *JSON shapes are frozen as of this document; the `Weathervane1`
interface freezes after the dogfood step (architecture doc step 6).*

## 6. Out of scope

Service module internals, refresh scheduling, daemon method signatures and
introspection XML, CLI argument design, packaging. Next design covers steps
3–5; everything it emits reuses the shapes frozen here.

## 7. Implementation order (input to the plan)

1. Add derives (§2.1) + `AqiCategory` restructure (§2.2) + `WireError` (§2.4).
2. Snapshot/round-trip/PII/compat test suite (§4) — green.
3. CONTRACT.md (§5), examples lifted from snapshots.
4. Version bump to 0.7.0; CHANGELOG notes the `standard` field removal.
