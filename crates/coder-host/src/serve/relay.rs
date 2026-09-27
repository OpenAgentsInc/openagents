//! Relay-carried operations: NIP-HOST requests in the direct artifact
//! binding, and NIP-TERM bodies as private `3188` artifacts.
//!
//! The host subscribes to `3188` events addressed to its key on every relay
//! it serves and reads each one's schema. Anything that is not a host
//! request or a terminal request is ignored, and an unknown key earns no
//! reply.

use std::sync::Arc;
use std::time::Duration;

use coder_connect::transport::Receiver;
use coder_pty::host::{FrameSink, SinkError};
use coder_pty::wire::{Frame, Reason, Refusal, TerminalResult};
use nostr::domain::Event;
use serde_json::Value;
use tokio::sync::oneshot;

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
    loop {
        let mut receiver =
            match Receiver::connect(&relay, &shared.secret, shared.config.policy).await {
                Ok(receiver) => receiver,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };
        if let Some(ready) = ready.take() {
            let _ = ready.send(());
        }
        // The subscription ends with the connection's lifetime or frame
        // budget; reconnecting replays the last minute, and every path below
        // answers an exact retry with its original result.
        while let Ok(event) = receiver.next_request().await {
            if event.pubkey == shared.host_key {
                continue;
            }
            match schema(&shared, &event).as_deref() {
                Some(coder_access::protocol::REQUEST) => {
                    if let Some(reply) = host_request(&shared, event, &relay).await
                        && receiver.publish(&reply).await.is_err()
                    {
                        break;
                    }
                }
                Some(schema) if schema.starts_with("openagents.terminal-") => {
                    tokio::spawn(terminal_request(shared.clone(), relay.clone(), event));
                }
                _ => {}
            }
        }
    }
}

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

impl FrameSink for RelaySink {
    fn deliver(&mut self, frame: &Frame) -> Result<(), SinkError> {
        let item = Outgoing::Frame {
            device: self.device.clone(),
            frame: frame.clone(),
        };
        if self.shared.publisher.try_to(&self.relay, item) {
            Ok(())
        } else {
            Err(SinkError::Full)
        }
    }
}
