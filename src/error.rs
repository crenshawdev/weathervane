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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::WireError;

    // Group A — NetworkKind::Display

    #[test]
    fn network_kind_display_connect() {
        assert_eq!(format!("{}", NetworkKind::Connect), "connect");
    }

    #[test]
    fn network_kind_display_body() {
        assert_eq!(format!("{}", NetworkKind::Body), "body");
    }

    #[test]
    fn network_kind_display_request() {
        assert_eq!(format!("{}", NetworkKind::Request), "request");
    }

    #[test]
    fn network_kind_display_unknown() {
        assert_eq!(format!("{}", NetworkKind::Unknown), "unknown");
    }

    // Group B — ParseKind::Display

    #[test]
    fn parse_kind_display_json() {
        assert_eq!(format!("{}", ParseKind::Json), "json");
    }

    #[test]
    fn parse_kind_display_xml() {
        assert_eq!(format!("{}", ParseKind::Xml), "xml");
    }

    // Group C — Error::Display for all 8 variants

    #[test]
    fn error_display_timeout() {
        assert_eq!(format!("{}", Error::Timeout), "request timed out");
    }

    #[test]
    fn error_display_network_connect() {
        assert_eq!(
            format!("{}", Error::Network(NetworkKind::Connect)),
            "network error: connect"
        );
    }

    #[test]
    fn error_display_http_status() {
        assert_eq!(format!("{}", Error::HttpStatus(429)), "http status 429");
    }

    #[test]
    fn error_display_parse_xml() {
        assert_eq!(
            format!("{}", Error::Parse(ParseKind::Xml)),
            "parse error: xml"
        );
    }

    #[test]
    fn error_display_http_client() {
        assert_eq!(
            format!(
                "{}",
                Error::HttpClient("tls backend not initialized".to_string())
            ),
            "failed to build HTTP client: tls backend not initialized"
        );
    }

    #[test]
    fn error_display_no_results() {
        assert_eq!(
            format!(
                "{}",
                Error::NoResults {
                    query: "Portlandia".to_string()
                }
            ),
            "no results"
        );
    }

    #[test]
    fn error_display_location_detection() {
        assert_eq!(
            format!("{}", Error::LocationDetection),
            "location detection failed"
        );
    }

    #[test]
    fn error_display_dbus() {
        assert_eq!(
            format!("{}", Error::Dbus("name lost".to_string())),
            "D-Bus error: name lost"
        );
    }

    // Group D — PII scrub at the Error layer (source-layer sibling to
    // tests/wire_contract.rs's WireError-only assertions)

    #[test]
    fn error_no_results_display_and_wire_error_never_leak_query() {
        let query = "SENTINEL_QUERY_04_04_DO_NOT_LEAK";
        let e = Error::NoResults {
            query: query.to_string(),
        };
        assert!(!format!("{}", e).contains(query));
        assert!(!WireError::from(&e).message.contains(query));
    }

    // Group E — From<quick_xml::DeError>

    #[test]
    fn from_quick_xml_de_error_maps_to_parse_xml() {
        let de_err = quick_xml::de::from_str::<i32>("<not valid xml").unwrap_err();
        let e: Error = de_err.into();
        assert!(matches!(e, Error::Parse(ParseKind::Xml)));
    }

    // Group F — From<reqwest::Error> is_request arm (or version-specific
    // Unknown fallback) for an empty-URL builder error

    #[test]
    fn from_reqwest_error_empty_url_maps_to_network_request() {
        let req_err = reqwest::Client::new().get("").build().unwrap_err();
        let e: Error = req_err.into();
        assert!(matches!(
            e,
            Error::Network(NetworkKind::Request) | Error::Network(NetworkKind::Unknown)
        ));
    }
}
