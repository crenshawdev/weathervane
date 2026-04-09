// SPDX-License-Identifier: MIT OR Apache-2.0

//! Suspend and resume detection via systemd-logind D-Bus signals.

use futures::Stream;
use std::pin::Pin;

/// Sleep events from systemd-logind over D-Bus.
#[derive(Debug, Clone)]
pub enum SleepEvent {
    /// System has resumed from suspend.
    Resumed,
}

/// Returns an async stream that yields `SleepEvent::Resumed` when the system
/// wakes from suspend, watching the `PrepareForSleep` signal on
/// `org.freedesktop.login1.Manager`.
///
/// Falls back to an idle stream if the system bus or logind is unavailable.
/// Frontend wraps this in its own subscription type (e.g. iced Subscription).
pub fn sleep_stream() -> Pin<Box<dyn Stream<Item = SleepEvent> + Send>> {
    Box::pin(async_stream::stream! {
        let Ok(connection) = zbus::Connection::system().await else {
            tracing::warn!("Could not connect to system D-Bus, sleep monitoring disabled");
            std::future::pending::<()>().await;
            return;
        };

        let rule = "type='signal',\
                    sender='org.freedesktop.login1',\
                    interface='org.freedesktop.login1.Manager',\
                    member='PrepareForSleep',\
                    path='/org/freedesktop/login1'";

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
            tracing::warn!("Failed to subscribe to logind PrepareForSleep signal: {}", e);
            std::future::pending::<()>().await;
            return;
        }

        tracing::info!("Listening for logind suspend and resume events");

        let mut stream = zbus::MessageStream::from(&connection);

        use futures::StreamExt;
        while let Some(Ok(msg)) = stream.next().await {
            let header = msg.header();
            if header.member().is_none_or(|m| m != "PrepareForSleep")
                || header
                    .interface()
                    .is_none_or(|i| i != "org.freedesktop.login1.Manager")
            {
                continue;
            }

            if let Ok(body) = msg.body().deserialize::<(bool,)>() {
                let going_to_sleep = body.0;
                tracing::debug!("logind PrepareForSleep: {}", going_to_sleep);

                if !going_to_sleep {
                    tracing::info!("System resumed from suspend");
                    yield SleepEvent::Resumed;
                }
            }
        }
    })
}
