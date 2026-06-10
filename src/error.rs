// SPDX-License-Identifier: MIT OR Apache-2.0

//! Error types for weathervane operations.

use thiserror::Error;

/// Sub-category of a [`Error::Network`] failure, so callers can react to (say)
/// a failed connection differently from a malformed body without string-matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkKind {
    /// Failed to establish a connection to the host.
    Connect,
    /// Failed while transferring the request or response body.
    Body,
    /// The request itself was invalid (builder, redirect policy, etc.).
    Request,
    /// A network failure that didn't fit the other kinds.
    Unknown,
}

impl std::fmt::Display for NetworkKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Connect => "connect",
            Self::Body => "body",
            Self::Request => "request",
            Self::Unknown => "unknown",
        })
    }
}

/// Sub-category of a [`Error::Parse`] failure, identifying the body format that
/// failed to decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseKind {
    /// JSON decoding failed.
    Json,
    /// XML decoding failed.
    Xml,
}

impl std::fmt::Display for ParseKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Json => "json",
            Self::Xml => "xml",
        })
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("request timed out")]
    Timeout,

    #[error("network error: {0}")]
    Network(NetworkKind),

    #[error("http status {0}")]
    HttpStatus(u16),

    #[error("parse error: {0}")]
    Parse(ParseKind),

    #[error("failed to build HTTP client: {0}")]
    HttpClient(String),

    #[error("no results")]
    NoResults { query: String },

    #[error("location detection failed")]
    LocationDetection,

    #[error("D-Bus error: {0}")]
    Dbus(String),
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            return Error::Timeout;
        }
        if let Some(status) = e.status() {
            return Error::HttpStatus(status.as_u16());
        }
        if e.is_decode() {
            return Error::Parse(ParseKind::Json);
        }
        if e.is_connect() {
            return Error::Network(NetworkKind::Connect);
        }
        if e.is_body() {
            return Error::Network(NetworkKind::Body);
        }
        if e.is_request() {
            return Error::Network(NetworkKind::Request);
        }
        Error::Network(NetworkKind::Unknown)
    }
}

impl From<quick_xml::DeError> for Error {
    fn from(_: quick_xml::DeError) -> Self {
        Error::Parse(ParseKind::Xml)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
