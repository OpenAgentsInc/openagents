//! Relay-carried operations: NIP-HOST requests in the direct artifact
//! binding, and NIP-TERM bodies as private `3188` artifacts.
//!
//! The host subscribes to `3188` events addressed to its key on every relay
//! it serves and reads each one's schema. Anything that is not a host
//! request or a terminal request is ignored, and an unknown key earns no
//! reply.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use coder_pty::ext::RecordsFrame;
use coder_pty::host::{FrameSink, SinkError};
use coder_pty::wire::{Frame, Reason, Refusal, TerminalResult};
use nostr::domain::Event;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::standing::{self, Subscription};
use super::{Shared, dispatch::Dispatcher, summarize, terminal};
use crate::authority::Standing;
use crate::message::TermRequest;
use crate::publish::Outgoing;
use crate::unix_time;

/// The widest distance between a terminal request's issue time and the host
/// clock. Older requests earn no reply.
const TERMINAL_WINDOW: u64 = 60;

/// Serve every configured relay. `ready` fires once the first subscription
/// is up.
pub(super) async fn serve(shared: Arc<Shared>, ready: oneshot::Sender<()>) {
    let (first, rest) = shared.config.relays.split_first().expect("validated");
    for relay in rest {
        tokio::spawn(serve_one(shared.clone(), relay.clone(), None));
    }
    serve_one(shared.clone(), first.clone(), Some(ready)).await;
}

async fn serve_one(shared: Arc<Shared>, relay: String, mut ready: Option<oneshot::Sender<()>>) {
    let host_key = shared.host_key.clone();
    let standing = Subscription {
        relay: relay.clone(),
        subscription: SUBSCRIPTION,
        lifetime: LIFETIME,
        filter: Box::new(move |now| {
            json!({
                "kinds": [nostr::contracts::ARTIFACT_ENVELOPE_KIND],
                "#p": [host_key],
                "since": now.saturating_sub(60),
                "limit": 128
            })
        }),
    };
    // When the last subscription connected, by the wall clock, which keeps
    // counting while the machine sleeps.
    let mut connected_at: Option<u64> = None;
    let catching = (shared.clone(), relay.clone());
    let connected = move || {
        if let Some(ready) = ready.take() {
            let _ = ready.send(());
        }
        // Renewals overlap and each replays its last minute, so an ordinary
        // renewal misses nothing. After a longer gap, such as the machine
        // sleeping, nudges sent meanwhile wait on the relay.
        let now = unix_time().unwrap_or_default();
        if connected_at.is_none_or(|at| now.saturating_sub(at) > CATCH_UP_GAP) {
            tokio::spawn(catch_up(catching.0.clone(), catching.1.clone()));
        }
        connected_at = Some(now);
    };
    let handler = shared.clone();
    standing::serve(
        shared.secret,
        shared.publisher.clone(),
        standing,
        connected,
        move |event, reply| {
            let (shared, relay) = (handler.clone(), relay.clone());
            async move {
                if event.pubkey == shared.host_key
                    || nostr::private_artifact::admit(&event).is_err()
                    || event.tag_values("p").collect::<Vec<_>>() != [shared.host_key.as_str()]
                {
                    return;
                }
                // Every path answers an exact retry with its original result, so
                // a request read again after a reconnect is answered the same.
                match schema(&shared, &event).as_deref() {
                    Some(coder_access::protocol::REQUEST) => {
                        if let Some(answer) = host_request(&shared, event, &relay).await {
                            reply.send(answer).await;
                        }
                    }
                    Some(schema) if schema.starts_with("openagents.terminal-") => {
                        terminal_request(shared, relay, event).await;
                    }
                    Some(crate::nudge::SCHEMA) => nudged(&shared, &event).await,
                    _ => {}
                }
            }
        },
    )
    .await;
}

/// The subscription ID on each relay connection.
const SUBSCRIPTION: &str = "history-input";
/// One subscription connection's lifetime, the longest a relay connection
/// may hold. The next one subscribes [`standing::OVERLAP`] before it ends.
const LIFETIME: Duration = Duration::from_secs(120);

fn schema(shared: &Shared, event: &Event) -> Option<String> {
    nostr::private_artifact::open(event, &shared.secret)
        .ok()?
        .artifact()
        .schema
        .clone()
}

/// Admit one signed NIP-HOST request and return the signed reply, if the
/// host produced one. Task changes publish summaries afterwards.
pub(super) async fn host_request(shared: &Arc<Shared>, event: Event, relay: &str) -> Option<Event> {
    let _busy = shared.activity.begin();
    let worker = shared.clone();
    let relay = relay.to_owned();
    let (reply, changed) = tokio::task::spawn_blocking(move || {
        let mut dispatcher = Dispatcher::new(worker.clone());
        let reply = worker
            .authority
            .handle(&event, &relay, &mut dispatcher)
            .ok();
        (reply, dispatcher.changed)
    })
    .await
    .ok()?;
    for task in &changed {
        summarize(shared, task).await;
    }
    reply
}

/// Answer one relay-carried terminal request with a sealed result, and
/// deliver an attachment's frames as sealed artifacts on the same relay.
async fn terminal_request(shared: Arc<Shared>, relay: String, event: Event) {
    let busy = shared.clone();
    let _busy = busy.activity.begin();
    let Ok(now) = unix_time() else { return };
    let device = event.pubkey.clone();
    // Only a key this host enrolled, even a revoked one, earns an answer.
    if shared.authority.standing(&device, now) == Standing::Unknown {
        return;
    }
    let Some(schema) = schema(&shared, &event) else {
        return;
    };
    let Ok((value, sealed)) = coder_reach::artifact::open::<Value>(
        &event,
        &shared.secret,
        &device,
        &shared.host_key,
        &schema,
    ) else {
        return;
    };
    if sealed.issued_at.abs_diff(now) > TERMINAL_WINDOW {
        return;
    }
    let request_id = value
        .get("request")
        .and_then(Value::as_str)
        .map(str::to_owned);
    // The request travels under its own ID as the mailbox, so its result
    // can be found under the same one.
    let mailbox_ok = request_id
        .as_deref()
        .is_some_and(|id| event.tag_values("h").collect::<Vec<_>>() == [id]);
    let result = match TermRequest::from_value(value) {
        Ok(request) if mailbox_ok => {
            let worker = shared.clone();
            let sink_relay = relay.clone();
            let sink_device = device.clone();
            tokio::task::spawn_blocking(move || {
                terminal::run(&worker, &sink_device.clone(), &request, || {
                    Box::new(RelaySink {
                        shared: worker.clone(),
                        relay: sink_relay,
                        device: sink_device,
                    })
                })
            })
            .await
            .ok()
        }
        Ok(_) => None,
        Err(refusal) => request_id
            .clone()
            .filter(|id| coder_pty::wire::is_common_id(id))
            .map(|id| TerminalResult::from_outcome(id, Err(refusal))),
    };
    let result = result.unwrap_or_else(|| {
        TerminalResult::from_outcome(
            request_id.unwrap_or_default(),
            Err(Refusal::new(
                Reason::Malformed,
                "request and mailbox differ",
            )),
        )
    });
    if coder_pty::wire::is_common_id(&result.request) {
        shared
            .publisher
            .to(&relay, Outgoing::Result { device, result })
            .await;
    }
}

/// Delivers an attachment's frames as sealed artifacts on one relay.
struct RelaySink {
    shared: Arc<Shared>,
    relay: String,
    device: String,
}

impl RelaySink {
    fn publish(&self, item: Outgoing) -> Result<(), SinkError> {
        if self.shared.publisher.try_to(&self.relay, item) {
            Ok(())
        } else {
            Err(SinkError::Full)
        }
    }
}

impl FrameSink for RelaySink {
    fn deliver(&mut self, frame: &Frame) -> Result<(), SinkError> {
        self.publish(Outgoing::Frame {
            device: self.device.clone(),
            frame: frame.clone(),
        })
    }

    fn carries_records(&self) -> bool {
        true
    }

    fn deliver_records(&mut self, part: &RecordsFrame) -> Result<(), SinkError> {
        self.publish(Outgoing::Records {
            device: self.device.clone(),
            part: part.clone(),
        })
    }
}

/// The most nudge event IDs a host remembers answering.
const REMEMBERED: usize = 1024;
/// The wall-clock time between two subscriptions past which the host reads
/// stored nudges: longer than a subscription's life and its renewal.
const CATCH_UP_GAP: u64 = 150;
/// How long a catch-up read of stored nudges may take.
const CATCH_UP_TIMEOUT: Duration = Duration::from_secs(10);

/// The nudges this process answered and when it last answered each device.
#[derive(Debug, Default)]
pub(crate) struct Answered {
    events: BTreeSet<String>,
    order: VecDeque<String>,
    devices: BTreeMap<String, u64>,
}

impl Answered {
    /// Whether to answer this nudge now: it is new to this process, and the
    /// device was not answered in the last [`crate::nudge::MIN_INTERVAL`].
    fn admit(&mut self, event: &str, device: &str, now: u64) -> bool {
        if self.events.contains(event) {
            return false;
        }
        self.events.insert(event.to_owned());
        self.order.push_back(event.to_owned());
        if self.order.len() > REMEMBERED
            && let Some(oldest) = self.order.pop_front()
        {
            self.events.remove(&oldest);
        }
        let recent = self
            .devices
            .get(device)
            .is_some_and(|at| now < at + crate::nudge::MIN_INTERVAL);
        if !recent {
            self.devices.insert(device.to_owned(), now);
        }
        !recent
    }
}

/// Answer one nudge: from a device whose grant is current, evaluate held
/// commands and publish presence and hints to that device now. Anything
/// else is ignored.
pub(super) async fn nudged(shared: &Arc<Shared>, event: &Event) {
    let Ok(now) = unix_time() else { return };
    let Ok(nudge) = crate::nudge::Nudge::open(event, &shared.secret, now) else {
        return;
    };
    if !shared
        .authority
        .active_devices(None, now)
        .contains(&nudge.device)
    {
        return;
    }
    let admitted = shared
        .nudges
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .admit(&event.id, &nudge.device, now);
    if !admitted {
        return;
    }
    let tasks = shared.tasks.clone();
    let authority = shared.authority.clone();
    let _ = tokio::task::spawn_blocking(move || {
        let standing = |principal: &crate::tasks::Principal| super::standing(&authority, principal);
        tasks.tick(&standing);
    })
    .await;
    super::publish_reach_to(shared, &nudge.device, now).await;
}

/// Read the nudges enrolled devices left on `relay` within a command's
/// lifetime and answer each new one.
async fn catch_up(shared: Arc<Shared>, relay: String) {
    let Ok(now) = unix_time() else { return };
    let mailboxes: Vec<String> = shared
        .authority
        .active_devices(None, now)
        .iter()
        .take(64)
        .filter_map(|device| {
            crate::mailbox::mailbox(&shared.secret, device, crate::mailbox::Stream::Nudges).ok()
        })
        .collect();
    if mailboxes.is_empty() {
        return;
    }
    let filter = serde_json::json!({
        "kinds": [nostr::contracts::ARTIFACT_ENVELOPE_KIND],
        "#p": [shared.host_key],
        "#h": mailboxes,
        "since": now.saturating_sub(crate::nudge::LIFETIME),
    });
    let read = tokio::time::timeout(CATCH_UP_TIMEOUT, async {
        let mut socket =
            nostr_transport::Connection::connect(&relay, &shared.secret, CATCH_UP_TIMEOUT)
                .await
                .ok()?;
        let id = coder_reach::new_id();
        socket
            .send(serde_json::json!(["REQ", id, filter]))
            .await
            .ok()?;
        let mut events = Vec::new();
        loop {
            let frame = socket.next().await.ok()?;
            if frame[1] != id.as_str() {
                continue;
            }
            match frame[0].as_str() {
                Some("EVENT") if events.len() < 256 => {
                    if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) {
                        events.push(event);
                    }
                }
                Some("EOSE" | "CLOSED") => break,
                _ => {}
            }
        }
        let _ = socket.close().await;
        Some(events)
    })
    .await;
    let Ok(Some(mut events)) = read else { return };
    // The newest nudge per device is enough.
    events.sort_by_key(|event| std::cmp::Reverse(event.created_at));
    let mut seen = BTreeSet::new();
    for event in events {
        if seen.insert(event.pubkey.clone()) {
            nudged(&shared, &event).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_nudge_is_answered_once_and_a_device_at_most_every_interval() {
        let mut answered = Answered::default();
        let now = 1_790_000_000;
        assert!(answered.admit("a", "phone", now));
        // The same nudge read again after a reconnect.
        assert!(!answered.admit("a", "phone", now + 600));
        // Another nudge from the same device soon after.
        assert!(!answered.admit("b", "phone", now + 1));
        assert!(answered.admit("c", "tablet", now + 1));
        assert!(answered.admit("d", "phone", now + crate::nudge::MIN_INTERVAL));
        // The memory of answered nudges is bounded.
        for index in 0..REMEMBERED + 10 {
            answered.admit(&index.to_string(), "laptop", now);
        }
        assert!(answered.events.len() <= REMEMBERED);
    }
}
