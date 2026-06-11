// SPDX-License-Identifier: MIT OR Apache-2.0

//! Wire-facing types for the frozen JSON contract (CONTRACT.md).
//!
//! These are the shapes the future daemon and CLI emit. They exist in the
//! library so the contract is testable (snapshot suite) before any IPC
//! surface ships.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Error;

/// JSON-facing error shape. Built from [`Error`], never derived on it, so the
/// wire never carries `NoResults { query }` payload data (PII contract: error
/// payloads contain no search text and no coordinates) and the shape stays
/// flat regardless of `Error`'s internal structure.
///
/// The passthrough variants (`HttpClient`, `Dbus`) carry library-generated
/// strings (reqwest builder / zbus errors), never user input or request URLs:
/// URL-bearing reqwest failures map to payload-free variants (`Timeout`,
/// `Network`, `HttpStatus`, `Parse`), so coordinates in query URLs can never
/// reach the wire. Construction sites must keep it that way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireError {
    /// `Error` variant name: "Timeout", "Network", "HttpStatus", "Parse",
    /// "HttpClient", "NoResults", "LocationDetection", "Dbus".
    pub kind: String,
    /// Human-readable detail — the `Display` output, e.g. "http status 429".
    pub message: String,
}

impl From<&Error> for WireError {
    fn from(e: &Error) -> Self {
        let kind = match e {
            Error::Timeout => "Timeout",
            Error::Network(_) => "Network",
            Error::HttpStatus(_) => "HttpStatus",
            Error::Parse(_) => "Parse",
            Error::HttpClient(_) => "HttpClient",
            Error::NoResults { .. } => "NoResults",
            Error::LocationDetection => "LocationDetection",
            Error::Dbus(_) => "Dbus",
        };
        Self {
            kind: kind.to_string(),
            message: e.to_string(),
        }
    }
}

/// A refresh failure as carried inside [`Envelope`]: what went wrong and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeError {
    /// `Error` variant name (same vocabulary as [`WireError::kind`]).
    pub kind: String,
    /// Human-readable detail (`Display` output).
    pub message: String,
    /// When the failed attempt happened (UTC).
    pub at: DateTime<Utc>,
}

impl EnvelopeError {
    /// Builds from a domain error plus the attempt timestamp.
    pub fn new(error: &Error, at: DateTime<Utc>) -> Self {
        let wire = WireError::from(error);
        Self {
            kind: wire.kind,
            message: wire.message,
            at,
        }
    }
}

/// One domain's state as exposed on the wire (CONTRACT.md, state layer):
/// last-good data + freshness + last attempt's failure, all keys always
/// present.
///
/// Invariants the producer (service module / daemon) upholds:
/// - `data` is never wiped by a failed refresh; `None` only before the first
///   successful fetch.
/// - `fetched_at` is `Some` iff `data` is.
/// - `error` is `None` iff the most recent attempt succeeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Last successfully fetched payload.
    pub data: Option<T>,
    /// When `data` was obtained (UTC).
    pub fetched_at: Option<DateTime<Utc>>,
    /// Most recent attempt's failure, if it failed.
    pub error: Option<EnvelopeError>,
}
