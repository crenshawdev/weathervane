// SPDX-License-Identifier: MIT OR Apache-2.0

use futures::Stream;
use std::pin::Pin;

/// Network connectivity events from NetworkManager.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Network connectivity was established or restored.
    Connected,
}

/// NM_STATE_CONNECTED_GLOBAL from the NetworkManager D-Bus API.
const NM_STATE_CONNECTED_GLOBAL: u32 = 70;

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
        while let Some(Ok(msg)) = stream.next().await {
            let header = msg.header();
            if header.member().is_none_or(|m| m != "StateChanged")
                || header
                    .interface()
                    .is_none_or(|i| i != "org.freedesktop.NetworkManager")
            {
                continue;
            }

            if let Ok(body) = msg.body().deserialize::<(u32,)>() {
                let state = body.0;
                tracing::debug!("NetworkManager state changed: {}", state);

                if state == NM_STATE_CONNECTED_GLOBAL {
                    tracing::info!("Network connectivity restored");
                    yield NetworkEvent::Connected;
                }
            }
        }
    })
}
