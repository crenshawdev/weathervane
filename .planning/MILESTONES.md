# Milestones

## v0.9 TechDebt (Shipped: 2026-07-01)

**Phases completed:** 2 phases, 5 plans, 5 tasks

**Key accomplishments:**

- aqicn token stripped from all five tracing::debug! sites in HTTP client using e.without_url(); five wiremock+tracing-test leak-assertion tests added inline; wire boundary pinned via wire_contract.rs sentinel assertion.
- ip-api.com coordinates range/NaN-validated in detect_location() before return, rejecting out-of-range/non-finite lat/lon with Error::LocationDetection (SEC-03).
- D-Bus message deserialization in network.rs and sleep.rs made panic-safe via decode helpers and explicit-match loops, malformed signals dropped observably at debug level without breaking the stream (SEC-04).
- NaN-safe test sort comparator plus observable (debug-logged), code-only drop paths for malformed JMA station table entries, with three new pinning tests.
- Replaced 4 bare `.unwrap()` calls in `src/pollen.rs` tests and upgraded the thin `.expect("valid ISO date")` in `src/time.rs` so a corrupted inline-JSON fixture or a regressed date literal now names the failing test and parsing step instead of an opaque panic.

---
