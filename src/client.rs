// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared HTTP client used by every fetch in the crate.
//!
//! Single lazy built client cloned per request. reqwest pools
//! connections and caches DNS internally so a long lived client
//! is the right shape but the same internal state can get stuck
//! in ways that look like network failure and won't recover on
//! their own. `reset_http_client` is the ejection seat.

use crate::error::{Error, Result};
use serde::de::DeserializeOwned;
use std::sync::RwLock;
use std::time::Duration;

const USER_AGENT: &str = "(weathervane, https://gitlab.com/vintagetechie/weathervane)";

/// Per-request timeout applied to all outgoing HTTP calls.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Kernel TCP keepalive interval. Probes detect dead peers without
/// waiting for the full request timeout.
const TCP_KEEPALIVE: Duration = Duration::from_secs(30);

/// How long an idle pooled connection can sit before being dropped.
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

static CLIENT: RwLock<Option<reqwest::Client>> = RwLock::new(None);

/// Returns a clone of the shared HTTP client, building it on first use.
pub(crate) fn http_client() -> Result<reqwest::Client> {
    if let Some(client) = CLIENT.read().unwrap().as_ref() {
        return Ok(client.clone());
    }

    let mut slot = CLIENT.write().unwrap();
    if let Some(client) = slot.as_ref() {
        return Ok(client.clone());
    }

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .tcp_keepalive(Some(TCP_KEEPALIVE))
        .pool_idle_timeout(Some(POOL_IDLE_TIMEOUT))
        .pool_max_idle_per_host(0)
        .build()
        .map_err(|e| Error::HttpClient(e.to_string()))?;

    *slot = Some(client.clone());
    Ok(client)
}

/// Throws away the shared HTTP client. Next request builds a new one.
///
/// For when something inside reqwest is stuck and you'd rather start
/// over than figure out what.
pub fn reset_http_client() {
    *CLIENT.write().unwrap() = None;
}

/// Returns `url` with its query string replaced by `?[redacted]`.
/// Used to keep tokens out of tracing output.
#[allow(dead_code)]
pub(crate) fn sanitize_url(url: &str) -> String {
    if let Some(i) = url.find('?') {
        format!("{}?[redacted]", &url[..i])
    } else {
        url.to_string()
    }
}

/// GETs `url` and deserializes the JSON body, returning `None` on any failure
/// (client build, send, non-2xx status, or decode). Each failure is logged at
/// debug, tagged with `ctx`. This is the "swallow and fall through" shape the
/// optional providers (AMeDAS, aqicn) share; it keeps that convention in one place.
pub(crate) async fn get_json<T: DeserializeOwned>(url: &str, ctx: &str) -> Option<T> {
    http_client()
        .ok()?
        .get(url)
        .send()
        .await
        .map_err(|e| tracing::debug!("{ctx} request failed: {}", e.without_url()))
        .ok()?
        .error_for_status()
        .map_err(|e| tracing::debug!("{ctx} status error: {}", e.without_url()))
        .ok()?
        .json::<T>()
        .await
        .map_err(|e| tracing::debug!("{ctx} parse failed: {}", e.without_url()))
        .ok()
}

/// GETs `url` and returns the raw response body as text, `None` on any failure.
///
/// Like [`get_json`] but for non-JSON endpoints (AMeDAS `latest_time.txt`) and
/// for APIs that carry error detail in a 200 body (aqicn). Deliberately does
/// *not* call `error_for_status`, matching those callers, which inspect the
/// body themselves.
pub(crate) async fn get_text(url: &str, ctx: &str) -> Option<String> {
    http_client()
        .ok()?
        .get(url)
        .send()
        .await
        .map_err(|e| tracing::debug!("{ctx} request failed: {}", e.without_url()))
        .ok()?
        .text()
        .await
        .map_err(|e| tracing::debug!("{ctx} body failed: {}", e.without_url()))
        .ok()
}
