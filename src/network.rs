// SPDX-License-Identifier: MIT OR Apache-2.0

//! Network connectivity monitoring via NetworkManager D-Bus signals.

use futures::Stream;
use std::pin::Pin;

/// Network connectivity events from NetworkManager over D-Bus.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// System reached full network connectivity (NM_STATE_CONNECTED_GLOBAL).
    Connected,
}

/// NM_STATE_CONNECTED_GLOBAL from the NetworkManager D-Bus API.
const NM_STATE_CONNECTED_GLOBAL: u32 = 70;

/// Decode the body of a NetworkManager StateChanged signal message.
///
/// Returns `Some(state)` on a well-formed `(u32,)` body, or `None` if the body
/// cannot be decoded (malformed or unexpected message shape). Emits a debug log
/// in both the success and error cases; on error the stream continues rather
/// than terminating.
fn decode_state_changed(msg: &zbus::Message) -> Option<u32> {
    match msg.body().deserialize::<(u32,)>() {
        Ok(body) => {
            tracing::debug!("NetworkManager state changed: {}", body.0);
            Some(body.0)
        }
        Err(e) => {
            tracing::debug!("NetworkManager message decode error: {}", e);
            None
        }
    }
}

/// Returns an async stream that yields `NetworkEvent::Connected` when the system
/// transitions to full network connectivity via NetworkManager.
///
/// Falls back to an idle stream if NetworkManager is unavailable.
/// Frontend wraps this in its own subscription type (e.g. iced Subscription).
pub fn network_stream() -> Pin<Box<dyn Stream<Item = NetworkEvent> + Send>> {
    Box::pin(async_stream::stream! {
        let Ok(connection) = zbus::Connection::system().await else {
            tracing::warn!("Could not connect to system D-Bus, network monitoring disabled");
            std::future::pending::<()>().await;
            return;
        };

        let rule = "type='signal',\
                    sender='org.freedesktop.NetworkManager',\
                    interface='org.freedesktop.NetworkManager',\
                    member='StateChanged',\
                    path='/org/freedesktop/NetworkManager'";

        if let Err(e) = connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "AddMatch",
                &rule,
            )
            .await
        {
            tracing::warn!("Failed to subscribe to NetworkManager signals: {}", e);
            std::future::pending::<()>().await;
            return;
        }

        tracing::info!("Listening for NetworkManager connectivity changes");

        let mut stream = zbus::MessageStream::from(&connection);

        use futures::StreamExt;
        while let Some(item) = stream.next().await {
            match item {
                Err(e) => {
                    tracing::debug!("NetworkManager stream error: {}", e);
                    continue;
                }
                Ok(msg) => {
                    let header = msg.header();
                    if header.member().is_none_or(|m| m != "StateChanged")
                        || header
                            .interface()
                            .is_none_or(|i| i != "org.freedesktop.NetworkManager")
                    {
                        continue;
                    }

                    if let Some(state) = decode_state_changed(&msg) {
                        if state == NM_STATE_CONNECTED_GLOBAL {
                            tracing::info!("Network connectivity restored");
                            yield NetworkEvent::Connected;
                        }
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_state_changed_returns_none_on_wrong_body_shape() {
        // Build a signal message whose body is (String,) -- wrong shape for (u32,).
        // The D-Bus signature "s" does not match "u", so deserialize returns Err.
        let msg = zbus::Message::signal(
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
            "StateChanged",
        )
        .expect("valid signal builder")
        .build(&("oops".to_string(),))
        .expect("valid message build");

        assert_eq!(decode_state_changed(&msg), None);
    }

    #[test]
    fn decode_state_changed_returns_some_on_well_formed_body() {
        // Build a signal message whose body is (u32,) -- the expected shape.
        // NM_STATE_CONNECTED_GLOBAL = 70 is a good concrete value to use here.
        let msg = zbus::Message::signal(
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
            "StateChanged",
        )
        .expect("valid signal builder")
        .build(&(70u32,))
        .expect("valid message build");

        assert_eq!(decode_state_changed(&msg), Some(70));
    }
}
