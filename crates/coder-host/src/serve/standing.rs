//! A relay subscription with no renewal gap.
//!
//! A relay connection lives at most 120 seconds, so a subscription has to be
//! renewed. Closing one connection and then opening the next leaves a gap,
//! and an event published in it is lost when the relay does not store it:
//! NIP-CJ execution requests are ephemeral, so before this a phone's message
//! sent in that gap waited for the device's retry. Here the next connection
//! subscribes [`OVERLAP`] before the current one's lifetime ends, both read
//! until the older one closes, and an event both deliver is handled once. A
//! connection that ends early is replaced at once.
//!
//! Each event is handled in its own task, so a slow request does not hold
//! the ones behind it. Its reply goes out on the connection that delivered
//! it, or, when that connection has closed by then, through the host's
//! publisher on the same relay.

use std::collections::{HashSet, VecDeque};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use nostr::domain::Event;
use nostr_transport::Connection;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use secp256k1::SecretKey;

use crate::publish::{Outgoing, Publisher};

/// How long before a connection's lifetime ends the next one subscribes (a
/// third of a shorter lifetime).
pub(super) const OVERLAP: Duration = Duration::from_secs(20);
/// The most event IDs remembered to handle each event once.
const REMEMBERED: usize = 4096;
/// Frames one connection may read: hundreds of requests in its life.
const FRAMES: usize = 4096;

/// A standing subscription's shape.
pub(super) struct Subscription {
    pub relay: String,
    /// The subscription ID on each connection.
    pub subscription: &'static str,
    /// Each connection's lifetime, at most 120 seconds.
    pub lifetime: Duration,
    /// The filter for a connection opened at a Unix time.
    pub filter: Box<dyn Fn(u64) -> Value + Send + Sync>,
}

/// Where a handled event's reply goes: the connection that delivered it,
/// else the publisher.
#[derive(Clone)]
pub(super) struct Reply {
    publisher: Publisher,
    relay: String,
    connection: mpsc::Sender<Event>,
}

impl Reply {
    /// Send `event` on the delivering connection, or publish it on the
    /// same relay when that connection has closed.
    pub(super) async fn send(&self, event: Event) {
        if let Err(mpsc::error::SendError(event)) = self.connection.send(event).await {
            self.publisher
                .to(&self.relay, Outgoing::Signed(event))
                .await;
        }
    }
}

/// Keep `standing` subscribed until the host stops, handling each event
/// once in its own task. `connected` runs each time a connection has
/// subscribed.
pub(super) async fn serve<H, F>(
    secret: SecretKey,
    publisher: Publisher,
    standing: Subscription,
    mut connected: impl FnMut() + Send,
    handle: H,
) where
    H: Fn(Event, Reply) -> F + Send + Sync + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let standing = Arc::new(standing);
    let (events, mut delivered) = mpsc::channel::<(Event, Reply)>(256);
    let renewing = {
        let standing = standing.clone();
        async move {
            loop {
                let Some(socket) = subscribe(&secret, &standing).await else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                };
                connected();
                let (ended, early) = oneshot::channel();
                tokio::spawn(read(
                    publisher.clone(),
                    standing.clone(),
                    socket,
                    events.clone(),
                    ended,
                ));
                // Renew before this one's lifetime ends, or at once when it
                // ends early.
                let renew = standing
                    .lifetime
                    .saturating_sub(OVERLAP.min(standing.lifetime / 3));
                tokio::select! {
                    () = tokio::time::sleep(renew) => {}
                    _ = early => {}
                }
            }
        }
    };
    let dispatching = async move {
        let mut seen = Seen::default();
        while let Some((event, reply)) = delivered.recv().await {
            if seen.first(&event.id) {
                tokio::spawn(handle(event, reply));
            }
        }
    };
    tokio::join!(renewing, dispatching);
}

async fn subscribe(secret: &SecretKey, standing: &Subscription) -> Option<Connection> {
    let mut socket = Connection::connect(&standing.relay, secret, standing.lifetime)
        .await
        .ok()?
        .with_frame_budget(FRAMES);
    let now = crate::unix_time().ok()?;
    socket
        .send(json!([
            "REQ",
            standing.subscription,
            (standing.filter)(now)
        ]))
        .await
        .ok()?;
    Some(socket)
}

/// Read one connection until it ends, sending its replies meanwhile. A
/// reply queued as it ends goes to the publisher.
async fn read(
    publisher: Publisher,
    standing: Arc<Subscription>,
    mut socket: Connection,
    events: mpsc::Sender<(Event, Reply)>,
    ended: oneshot::Sender<()>,
) {
    let (replies, mut outbound) = mpsc::channel::<Event>(64);
    let reply = Reply {
        publisher: publisher.clone(),
        relay: standing.relay.clone(),
        connection: replies,
    };
    loop {
        tokio::select! {
            out = outbound.recv() => {
                let Some(event) = out else { break };
                if socket.send(json!(["EVENT", event])).await.is_err() {
                    publisher.to(&standing.relay, Outgoing::Signed(event)).await;
                    break;
                }
            }
            frame = socket.next() => {
                let Ok(frame) = frame else { break };
                if frame[1] != standing.subscription {
                    continue;
                }
                if frame[0] == "CLOSED" {
                    break;
                }
                if frame[0] != "EVENT" {
                    continue;
                }
                let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
                    continue;
                };
                if events.send((event, reply.clone())).await.is_err() {
                    break;
                }
            }
        }
    }
    let _ = ended.send(());
    outbound.close();
    while let Some(event) = outbound.recv().await {
        publisher.to(&standing.relay, Outgoing::Signed(event)).await;
    }
}

/// Event IDs already handled, the newest [`REMEMBERED`].
#[derive(Default)]
struct Seen {
    ids: HashSet<String>,
    order: VecDeque<String>,
}

impl Seen {
    fn first(&mut self, id: &str) -> bool {
        if !self.ids.insert(id.to_owned()) {
            return false;
        }
        self.order.push_back(id.to_owned());
        if self.order.len() > REMEMBERED
            && let Some(oldest) = self.order.pop_front()
        {
            self.ids.remove(&oldest);
        }
        true
    }
}

#[cfg(test)]
mod tests;
