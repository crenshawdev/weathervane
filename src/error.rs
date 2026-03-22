// SPDX-License-Identifier: MIT OR Apache-2.0

//! Error types for weathervane operations.

use thiserror::Error;

/// Errors produced by weathervane operations.
#[derive(Debug, Error)]
pub enum TempestError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("failed to build HTTP client: {0}")]
    HttpClient(String),

    #[error("no results found for '{query}'")]
    NoResults { query: String },

    #[error("location detection failed")]
    LocationDetection,

    #[error("failed to parse XML: {0}")]
    Xml(#[from] quick_xml::DeError),

    #[error("D-Bus connection failed: {0}")]
    Dbus(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, TempestError>;
