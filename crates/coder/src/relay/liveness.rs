//! Keeping a long-lived relay subscription honest (#9946).
//!
//! A socket can stay open on a worker's side after the relay behind it is
//! gone. `relay.openagents.com` is a Cloud Run service behind Google's
//! front end, and when the relay instance restarts, the front end can keep
//! a subscriber's TCP connection established while nothing reaches it: the
//! subscriber reads silence and every request published meanwhile is lost.
//! Silence cannot tell an idle relay from a dead one, so a subscriber asks:
//! every probe interval it sends a `REQ` that matches nothing new
//! ([`probe_request`]) and expects the relay's `EOSE` or `CLOSED` for it
//! before the next probe. A probe still unanswered then ends the connection
//! as a fault, and the subscriber reconnects and subscribes again.
//!
//! Cloud Run also ends every request, a WebSocket included, at its request
//! timeout (an hour for the relay), so the subscriber renews its
//! subscription before then on a [`successor`] connection that is
//! subscribed before the old one is closed, and reads the old one for
//! [`DRAIN`] longer, so no request falls into the gap between them.
//!
//! These are the pieces the chat worker (`coder-worker`) and the hosted
//! eval runner (`crates/eval-runner`) share; each keeps its own session
//! loop around them.

use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt as _;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite;

use super::{Identity, Socket, connect, send};

/// How often a subscriber proves its relay connection still carries its
/// subscription, and how long each proof may take.
pub const PROBE_EVERY: Duration = Duration::from_secs(30);

/// How long one connection's subscription is kept before the subscriber
/// opens its successor: before the relay's one-hour Cloud Run request
/// timeout.
pub const RENEW_EVERY: Duration = Duration::from_secs(45 * 60);

/// How long a replaced connection is still read after its successor
/// subscribed, for a request the relay delivered on it in the meantime.
pub const DRAIN: Duration = Duration::from_secs(5);

/// The bound on opening a successor connection, authentication and the
/// subscription's `EOSE` included.
pub const RENEW_WITHIN: Duration = Duration::from_secs(30);

/// The prefix of a liveness probe's subscription ID.
pub const PROBE_PREFIX: &str = "alive-";

/// How a subscriber keeps its relay connection honest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Liveness {
    /// How often a probe goes out, and how long each may take.
    pub probe: Duration,
    /// How long a subscription is kept before it is renewed.
    pub renew: Duration,
}

impl Default for Liveness {
    fn default() -> Self {
        Self {
            probe: PROBE_EVERY,
            renew: RENEW_EVERY,
        }
    }
}

impl Liveness {
    /// The default periods, each overridden by its environment variable
    /// (a positive number of milliseconds) when set.
    ///
    /// # Errors
    ///
    /// Names a variable that is set but isn't a positive number.
    pub fn from_env(probe_var: &str, renew_var: &str) -> Result<Self, String> {
        Ok(Self {
            probe: millis_from_env(probe_var)?.unwrap_or(PROBE_EVERY),
            renew: millis_from_env(renew_var)?.unwrap_or(RENEW_EVERY),
        })
    }
}

/// The duration in milliseconds that `name` holds, if it is set.
///
/// # Errors
///
/// Names the variable when it is set but isn't a positive number.
pub fn millis_from_env(name: &str) -> Result<Option<Duration>, String> {
    match env::var(name) {
        Ok(text) => match text.trim().parse::<u64>() {
            Ok(millis) if millis > 0 => Ok(Some(Duration::from_millis(millis))),
            _ => Err(format!(
                "{name} is a positive number of milliseconds, not `{text}`"
            )),
        },
        Err(_) => Ok(None),
    }
}

/// A liveness probe on subscription `id`: a `REQ` for `filter` with
/// `limit` 0, which the relay answers with `EOSE` at once since it asks for
/// no stored events. Events that arrive on it before its `CLOSE` are set
/// aside by subscription ID; requests are taken only from the real
/// subscription.
#[must_use]
pub fn probe_request(id: &str, mut filter: Value) -> Value {
    filter["limit"] = json!(0);
    json!(["REQ", id, filter])
}

/// Whether subscription `id` is a liveness probe.
#[must_use]
pub fn is_probe_id(id: &str) -> bool {
    id.starts_with(PROBE_PREFIX)
}

/// Connects, authenticates, and sends `request` (a `REQ`).
///
/// # Errors
///
/// Why the connection or the send failed.
pub async fn subscribe(url: &str, identity: &Identity, request: Value) -> Result<Socket, String> {
    let mut socket = connect(url, identity)
        .await
        .map_err(|error| error.to_string())?;
    send(&mut socket, request)
        .await
        .map_err(|error| error.to_string())?;
    Ok(socket)
}

/// A successor connection being opened.
pub type Renewal = Pin<Box<dyn Future<Output = Result<Successor, String>> + Send>>;

/// A renewed connection whose subscription the relay confirmed, with any
/// frames it delivered on that subscription before its `EOSE`.
pub struct Successor {
    /// The new connection, subscribed.
    pub socket: Socket,
    /// `EVENT` frames on the subscription before its `EOSE`.
    pub early: Vec<Value>,
}

/// Opens the connection that replaces the current one, sends `request`
/// (a `REQ` on `subscription`), and returns it once the relay confirmed
/// the subscription with `EOSE`, within [`RENEW_WITHIN`].
///
/// # Errors
///
/// Why no subscription came up in time.
pub async fn successor(
    url: String,
    identity: Arc<Identity>,
    subscription: &'static str,
    request: Value,
) -> Result<Successor, String> {
    let opening = async {
        let mut socket = subscribe(&url, &identity, request).await?;
        let mut early = Vec::new();
        loop {
            let frame = socket
                .next()
                .await
                .ok_or_else(|| "the relay closed the socket".to_string())?
                .map_err(|error| format!("socket: {error}"))?;
            let tungstenite::Message::Text(text) = frame else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if value[1].as_str() != Some(subscription) {
                continue;
            }
            match value[0].as_str() {
                Some("EOSE") => return Ok(Successor { socket, early }),
                Some("CLOSED") => {
                    return Err(format!(
                        "the relay closed the {subscription} subscription: {}",
                        value[2].as_str().unwrap_or_default()
                    ));
                }
                Some("EVENT") => early.push(value),
                _ => {}
            }
        }
    };
    tokio::time::timeout(RENEW_WITHIN, opening)
        .await
        .map_err(|_| format!("no subscription in {} s", RENEW_WITHIN.as_secs()))?
}

/// Awaits the pending future in `slot`, or never when there is none.
pub async fn poll_some<T>(slot: &mut Option<Pin<Box<dyn Future<Output = T> + Send>>>) -> T {
    match slot {
        Some(future) => future.as_mut().await,
        None => std::future::pending().await,
    }
}

/// The next frame of the connection being drained, or `None` once it
/// ends or its [`DRAIN`] is over; never when there is none.
pub async fn next_draining(
    draining: &mut Option<(Socket, tokio::time::Instant)>,
) -> Option<Result<tungstenite::Message, tungstenite::Error>> {
    match draining {
        Some((socket, end)) => tokio::time::timeout_at(*end, socket.next())
            .await
            .ok()
            .flatten(),
        None => std::future::pending().await,
    }
}

/// Tells systemd's watchdog the subscriber has just proven its relay
/// subscription live.
///
/// Under a unit with `WatchdogSec=`, systemd restarts a process that stops
/// saying so, a backstop behind the probes for one that is stuck somewhere
/// they cannot see. Without `NOTIFY_SOCKET` (any other way the binary
/// runs) this does nothing.
pub fn notify_watchdog() {
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixDatagram;
        let Some(path) = env::var_os("NOTIFY_SOCKET") else {
            return;
        };
        let Ok(socket) = UnixDatagram::unbound() else {
            return;
        };
        let bytes = path.as_encoded_bytes();
        if let Some(name) = bytes.strip_prefix(b"@") {
            #[cfg(target_os = "linux")]
            {
                use std::os::linux::net::SocketAddrExt;
                if let Ok(address) = std::os::unix::net::SocketAddr::from_abstract_name(name) {
                    let _ = socket.send_to_addr(b"WATCHDOG=1", &address);
                }
            }
            #[cfg(not(target_os = "linux"))]
            let _ = name;
            return;
        }
        let _ = socket.send_to(b"WATCHDOG=1", &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_probe_asks_for_nothing_stored() {
        let probe = probe_request("alive-1", json!({"kinds": [5], "#p": ["ab"]}));
        assert_eq!(
            probe,
            json!(["REQ", "alive-1", {"kinds": [5], "#p": ["ab"], "limit": 0}])
        );
        assert!(is_probe_id("alive-1"));
        assert!(!is_probe_id("jobs"));
    }
}
