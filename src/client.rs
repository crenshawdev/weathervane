// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared HTTP client used by every fetch in the crate.
//!
//! Single lazy built client cloned per request. reqwest pools
//! connections and caches DNS internally so a long lived client
//! is the right shape but the same internal state can get stuck
//! in ways that look like network failure and won't recover on
//! their own. `reset_http_client` is the ejection seat.

use crate::error::{Error, Result};
use std::sync::RwLock;
use std::time::Duration;

const USER_AGENT: &str =
    "(weathervane, https://gitlab.com/vintagetechie/weathervane)";

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
