//! Host publication: one authenticated connection per relay, reused until
//! the relay or its bounds end it, with each event acknowledged in order.
//!
//! Terminal frames and results are sealed here rather than in the terminal
//! host's frame sink, which must return without waiting.

use std::time::Duration;

use coder_pty::ext::{RECORDS, RecordsFrame};
use coder_pty::wire::{FRAME, Frame, RESULT, TerminalResult};
use nostr::domain::Event;
use nostr_transport::Connection;
use secp256k1::SecretKey;
use serde_json::json;
use tokio::sync::mpsc;

/// How long relay artifacts that carry terminal traffic are retained.
const TERMINAL_RETENTION: u64 = 60 * 60;
/// How many queued items one relay holds before new ones are refused.
const QUEUE: usize = 1024;

/// One item to publish.
#[derive(Debug)]
pub(crate) enum Outgoing {
    /// An event the host already signed.
    Signed(Event),
    /// A terminal frame for one device, sealed under the attachment mailbox.
    Frame { device: String, frame: Frame },
    /// A part of a record stream for one device, sealed under the
    /// attachment mailbox like frames.
    Records { device: String, part: RecordsFrame },
    /// A terminal result for one device, sealed under the request mailbox.
    Result {
        device: String,
        result: TerminalResult,
    },
}

/// Queues for every relay the host serves.
#[derive(Clone)]
pub(crate) struct Publisher {
    relays: Vec<(String, mpsc::Sender<Outgoing>)>,
}

impl Publisher {
    pub(crate) fn spawn(relays: &[String], secret: SecretKey) -> Self {
        let relays = relays
            .iter()
            .map(|relay| {
                let (sender, receiver) = mpsc::channel(QUEUE);
                tokio::spawn(run(relay.clone(), secret, receiver));
                (relay.clone(), sender)
            })
            .collect();
        Self { relays }
    }

    /// Queue a signed event on every relay.
    pub(crate) async fn everywhere(&self, event: &Event) {
        for (_, sender) in &self.relays {
            let _ = sender.send(Outgoing::Signed(event.clone())).await;
        }
    }

    /// Queue an item on one relay without waiting. `false` means the queue is
    /// full or the relay is not served.
    pub(crate) fn try_to(&self, relay: &str, item: Outgoing) -> bool {
        self.relays
            .iter()
            .find(|(url, _)| url == relay)
            .is_some_and(|(_, sender)| sender.try_send(item).is_ok())
    }

    /// Queue an item on one relay, waiting for room.
    pub(crate) async fn to(&self, relay: &str, item: Outgoing) {
        if let Some((_, sender)) = self.relays.iter().find(|(url, _)| url == relay) {
            let _ = sender.send(item).await;
        }
    }
}

async fn run(relay: String, secret: SecretKey, mut queue: mpsc::Receiver<Outgoing>) {
    let mut connection: Option<Connection> = None;
    while let Some(item) = queue.recv().await {
        let Some(event) = seal(item, &secret) else {
            continue;
        };
        for _ in 0..3 {
            if connection.is_none() {
                connection = Connection::connect(&relay, &secret, Duration::from_secs(110))
                    .await
                    .ok();
            }
            let Some(socket) = connection.as_mut() else {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            };
            match acknowledged(socket, &event).await {
                Ok(()) => break,
                // A socket at the end of its lifetime or frame budget fails
                // here; reconnect and send the same exact event again.
                Err(()) => connection = None,
            }
        }
    }
}

/// Send one event and wait for its acknowledgment, skipping other frames.
/// A refused event is not retried: the relay's policy will not change.
async fn acknowledged(socket: &mut Connection, event: &Event) -> Result<(), ()> {
    socket.send(json!(["EVENT", event])).await.map_err(|_| ())?;
    for _ in 0..64 {
        let frame = socket.next().await.map_err(|_| ())?;
        if frame[0] == "OK" && frame[1] == event.id {
            return Ok(());
        }
    }
    Err(())
}

fn seal(item: Outgoing, secret: &SecretKey) -> Option<Event> {
    let now = crate::unix_time().ok()?;
    match item {
        Outgoing::Signed(event) => Some(event),
        Outgoing::Frame { device, frame } => coder_reach::artifact::seal(
            &frame,
            FRAME,
            secret,
            &device,
            &frame.attachment,
            now,
            now + TERMINAL_RETENTION,
        )
        .ok(),
        Outgoing::Records { device, part } => coder_reach::artifact::seal(
            &part,
            RECORDS,
            secret,
            &device,
            &part.attachment,
            now,
            now + TERMINAL_RETENTION,
        )
        .ok(),
        Outgoing::Result { device, result } => coder_reach::artifact::seal(
            &result,
            RESULT,
            secret,
            &device,
            &result.request,
            now,
            now + TERMINAL_RETENTION,
        )
        .ok(),
    }
}
