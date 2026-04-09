// SPDX-License-Identifier: MIT OR Apache-2.0

//! Error types for weathervane operations.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("request timed out")]
    Timeout,

    #[error("network error: {0}")]
    Network(String),

    #[error("http status {0}")]
    HttpStatus(u16),

    #[error("parse error: {0}")]
    Parse(String),

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
            return Error::Parse("json".to_string());
        }
        if e.is_connect() {
            return Error::Network("connect".to_string());
        }
        if e.is_body() {
            return Error::Network("body".to_string());
        }
        if e.is_request() {
            return Error::Network("request".to_string());
        }
        Error::Network("unknown".to_string())
    }
}

impl From<quick_xml::DeError> for Error {
    fn from(_: quick_xml::DeError) -> Self {
        Error::Parse("xml".to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
