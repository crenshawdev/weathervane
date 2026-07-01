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

/// Decode the body of a logind PrepareForSleep signal message.
///
/// Returns `Some(going_to_sleep)` on a well-formed `(bool,)` body, or `None` if
/// the body cannot be decoded (malformed or unexpected message shape). Emits a
/// debug log in both the success and error cases; on error the stream continues
/// rather than terminating.
fn decode_prepare_for_sleep(msg: &zbus::Message) -> Option<bool> {
    match msg.body().deserialize::<(bool,)>() {
        Ok(body) => {
            tracing::debug!("logind PrepareForSleep: {}", body.0);
            Some(body.0)
        }
        Err(e) => {
            tracing::debug!("logind message decode error: {}", e);
            None
        }
    }
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
        while let Some(item) = stream.next().await {
            match item {
                Err(e) => {
                    tracing::debug!("logind stream error: {}", e);
                    continue;
                }
                Ok(msg) => {
                    let header = msg.header();
                    if header.member().is_none_or(|m| m != "PrepareForSleep")
                        || header
                            .interface()
                            .is_none_or(|i| i != "org.freedesktop.login1.Manager")
                    {
                        continue;
                    }

                    if let Some(going_to_sleep) = decode_prepare_for_sleep(&msg) {
                        if !going_to_sleep {
                            tracing::info!("System resumed from suspend");
                            yield SleepEvent::Resumed;
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
    fn decode_prepare_for_sleep_returns_none_on_wrong_body_shape() {
        // Build a signal message whose body is (u32,) -- wrong shape for (bool,).
        // The D-Bus signature "u" does not match "b", so deserialize returns Err.
        let msg = zbus::Message::signal(
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "PrepareForSleep",
        )
        .expect("valid signal builder")
        .build(&(42u32,))
        .expect("valid message build");

        assert_eq!(decode_prepare_for_sleep(&msg), None);
    }

    #[test]
    fn decode_prepare_for_sleep_returns_some_on_well_formed_body() {
        // Build a signal message whose body is (bool,) -- the expected shape.
        // true means going to sleep; false means resuming. Test both values.
        let msg_sleep = zbus::Message::signal(
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "PrepareForSleep",
        )
        .expect("valid signal builder")
        .build(&(true,))
        .expect("valid message build");

        assert_eq!(decode_prepare_for_sleep(&msg_sleep), Some(true));

        let msg_resume = zbus::Message::signal(
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "PrepareForSleep",
        )
        .expect("valid signal builder")
        .build(&(false,))
        .expect("valid message build");

        assert_eq!(decode_prepare_for_sleep(&msg_resume), Some(false));
    }
}
