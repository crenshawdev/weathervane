// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared HTTP client used by every fetch in the crate.
//!
//! Single lazy built client cloned per request. reqwest pools
//! connections and caches DNS internally so a long lived client
//! is the right shape but the same internal state can get stuck
//! in ways that look like network failure and won't recover on
//! their own. `reset_http_client` is the ejection seat; frontends
//! call it on resume ([`crate::sleep_stream`]) so a pooled
//! connection that died across a suspend is never resued.

use crate::error::{Error, Result};
use serde::de::DeserializeOwned;
use std::sync::RwLock;
use std::time::Duration;

const USER_AGENT: &str = "(weathervane, https://github.com/crenshawdev/weathervane)";

/// Per-request timeout applied to all outgoing HTTP calls.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Kernel TCP keepalive interval. Probes detect dead peers without
/// waiting for the full request timeout.
const TCP_KEEPALIVE: Duration = Duration::from_secs(30);

/// How long an idle pooled connection can sit before being dropped.
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Idle connections kept per host. The crate's own walks (ECCC listings
/// and CAP files, NWS station hops) are sequential and need one; two
/// covers a frontend fetching from the same host concurrently.
const POOL_MAX_IDLE_PER_HOST: usize = 2;

static CLIENT: RwLock<Option<reqwest::Client>> = RwLock::new(None);

/// Returns a clone of the shared HTTP client, building it on first use.
///
/// A poisoned lock is a cache miss, as in the crate's other caches: the
/// client is built fresh and, if the slot can't be written, not stored.
pub(crate) fn http_client() -> Result<reqwest::Client> {
    if let Some(client) = CLIENT.read().ok().and_then(|slot| slot.clone()) {
        return Ok(client);
    }

    let mut slot = CLIENT.write().ok();
    if let Some(client) = slot.as_deref().and_then(Option::as_ref) {
        return Ok(client.clone());
    }

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .tcp_keepalive(Some(TCP_KEEPALIVE))
        .pool_idle_timeout(Some(POOL_IDLE_TIMEOUT))
        .pool_max_idle_per_host(POOL_MAX_IDLE_PER_HOST)
        .build()
        .map_err(|e| Error::HttpClient(e.to_string()))?;

    if let Some(slot) = slot.as_deref_mut() {
        *slot = Some(client.clone());
    }
    Ok(client)
}

/// Throws away the shared HTTP client. Next request builds a new one.
///
/// For when something inside reqwest is stuck and you'd rather start
/// over than figure out what. On a poisoned lock this does nothing, since
/// `http_client` already builds a fresh client for every request then.
pub fn reset_http_client() {
    if let Ok(mut slot) = CLIENT.write() {
        *slot = None;
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

#[cfg(test)]
mod tests {
    use super::{get_json, get_text};
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Unique sentinel used as a fake aqicn token in test URLs. If this string
    /// appears anywhere in tracing output, a token-leak regression exists.
    const AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG: &str = "AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG";

    /// Builds a URL that carries the sentinel token in the query string.
    fn build_url_with_sentinel(server_uri: &str) -> String {
        format!("{server_uri}/feed?token={AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG}")
    }

    /// The aqicn token must not appear in tracing output when the TCP connection fails.
    /// Port 1 is reserved and immediately ECONNREFUSED on Linux -- no MockServer needed.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn aqicn_token_does_not_leak_on_connect_failure() {
        let url = format!("http://127.0.0.1:1/feed?token={AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG}");
        let _ = get_json::<serde_json::Value>(&url, "aqicn").await;
        assert!(
            !logs_contain(AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG),
            "aqicn token leaked into tracing output on connect failure"
        );
        assert!(
            !logs_contain("?token="),
            "raw token query string leaked into tracing output on connect failure"
        );
    }

    /// The aqicn token must not appear in tracing output when the server returns HTTP 500.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn aqicn_token_does_not_leak_on_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let url = build_url_with_sentinel(&server.uri());
        let _ = get_json::<serde_json::Value>(&url, "aqicn").await;
        assert!(
            !logs_contain(AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG),
            "aqicn token leaked into tracing output on HTTP 500"
        );
        assert!(
            !logs_contain("?token="),
            "raw token query string leaked into tracing output on HTTP 500"
        );
    }

    /// The aqicn token must not appear in tracing output when the response body read fails.
    ///
    /// A raw TcpListener sends HTTP headers claiming Content-Length: 1000 but drops
    /// the connection after 5 bytes, causing hyper to return an incomplete-body error.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn aqicn_token_does_not_leak_on_body_failure() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let headers =
                    "HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nContent-Type: text/plain\r\n\r\n";
                let _ = stream.write_all(headers.as_bytes()).await;
                let _ = stream.write_all(b"hello").await;
                // dropping stream closes the connection mid-body
            }
        });

        let url = format!("http://{addr}/feed?token={AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG}");
        let _ = get_text(&url, "aqicn").await;
        assert!(
            !logs_contain(AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG),
            "aqicn token leaked into tracing output on body failure"
        );
        assert!(
            !logs_contain("?token="),
            "raw token query string leaked into tracing output on body failure"
        );
    }

    /// Two sequential requests to one host share a connection: the pool keeps
    /// it idle between them instead of opening a new TCP connection each time.
    #[tokio::test]
    async fn sequential_requests_reuse_one_connection() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));

        let counter = Arc::clone(&accepts);
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                counter.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    // One read per request is enough for these small GETs;
                    // the connection stays open for the next one.
                    while let Ok(n) = stream.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        let reply = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
                        if stream.write_all(reply.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });

        let url = format!("http://{addr}/");
        assert_eq!(get_text(&url, "pool").await.as_deref(), Some("ok"));
        assert_eq!(get_text(&url, "pool").await.as_deref(), Some("ok"));
        assert_eq!(accepts.load(Ordering::SeqCst), 1);
    }

    /// The aqicn token must not appear in tracing output when the JSON parse fails.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn aqicn_token_does_not_leak_on_parse_failure() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"not valid json {{" as &[u8])
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;
        let url = build_url_with_sentinel(&server.uri());
        let _ = get_json::<serde_json::Value>(&url, "aqicn").await;
        assert!(
            !logs_contain(AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG),
            "aqicn token leaked into tracing output on JSON parse failure"
        );
        assert!(
            !logs_contain("?token="),
            "raw token query string leaked into tracing output on JSON parse failure"
        );
    }

    /// The aqicn token must not appear in tracing output even on a successful 200 response.
    /// This catches a regression where someone adds an info-level "fetched {url}" log line.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn aqicn_token_does_not_leak_on_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .mount(&server)
            .await;
        let url = build_url_with_sentinel(&server.uri());
        let result = get_json::<serde_json::Value>(&url, "aqicn").await;
        assert!(result.is_some(), "expected successful JSON parse");
        assert!(
            !logs_contain(AQICN_LEAK_SENTINEL_TOKEN_DO_NOT_LOG),
            "aqicn token leaked into tracing output on successful request"
        );
        assert!(
            !logs_contain("?token="),
            "raw token query string leaked into tracing output on successful request"
        );
    }
}
