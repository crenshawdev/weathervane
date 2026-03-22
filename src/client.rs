// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::{Result, TempestError};
use std::sync::OnceLock;
use std::time::Duration;

const USER_AGENT: &str =
    "(weathervane, https://gitlab.com/vintagetechie/weathervane)";

/// Per-request timeout applied to all outgoing HTTP calls.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Shared HTTP client for connection pooling and consistent headers.
pub(crate) fn http_client() -> Result<&'static reqwest::Client> {
    static CLIENT: OnceLock<std::result::Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| TempestError::HttpClient(e.clone()))
}
