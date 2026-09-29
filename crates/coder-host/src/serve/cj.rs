//! The CAP/CJ binding of NIP-HOST.
//!
//! The host publishes its `host-access` capability (`kind:30180`) on every
//! relay it serves, subscribes to NIP-CJ execution requests (`kind:25920`)
//! addressed to its key, and answers each with a `kind:26920` result.
//! `coder_access::cj::intake` checks the binding fields; the embedded
//! request then goes through the same admission as the direct artifact
//! binding, [`host_request`], so grants, epochs, rights, idempotency, and
//! refusals are shared. A `completed` result means the operation answered,
//! not that a task ran.

use std::sync::Arc;
use std::time::Duration;

use coder_access::cj::{Capability, Intake, intake};
use nostr::domain::Event;
use nostr::execution;
use serde_json::json;

use super::Shared;
use super::relay::host_request;
use super::standing::{self, Subscription};
use crate::unix_time;

/// One subscription connection's lifetime. The next one subscribes
/// [`standing::OVERLAP`] before it ends.
const LIFETIME: Duration = Duration::from_secs(110);
/// The subscription ID on each relay connection.
const SUBSCRIPTION: &str = "host-cj";

/// Advertise the capability, then serve CJ requests on every relay.
pub(super) async fn serve(shared: Arc<Shared>) {
    let Ok(capability) = Capability::new(&shared.host_key, shared.config.relays.clone()) else {
        return;
    };
    let capability = Arc::new(capability);
    if let Ok(now) = unix_time()
        && let Ok(manifest) = capability.manifest(&shared.secret, now)
    {
        shared.publisher.everywhere(&manifest).await;
    }
    // Dropping the set when the host shuts down stops every relay loop.
    let mut loops = tokio::task::JoinSet::new();
    for relay in &shared.config.relays {
        loops.spawn(serve_one(shared.clone(), capability.clone(), relay.clone()));
    }
    while loops.join_next().await.is_some() {}
}

async fn serve_one(shared: Arc<Shared>, capability: Arc<Capability>, relay: String) {
    let host_key = shared.host_key.clone();
    let standing = Subscription {
        relay: relay.clone(),
        subscription: SUBSCRIPTION,
        lifetime: LIFETIME,
        filter: Box::new(move |now| {
            json!({
                "kinds": [execution::REQUEST_KIND],
                "#p": [host_key],
                "since": now.saturating_sub(60)
            })
        }),
    };
    // Execution kinds are ephemeral: a request sent while no subscription
    // is open is not replayed. Renewals overlap, so one always is; a
    // request both connections deliver is answered once, and a device's
    // retry is answered as a retransmission.
    let handler = shared.clone();
    standing::serve(
        shared.secret,
        shared.publisher.clone(),
        standing,
        || {},
        move |event, reply| {
            let (shared, capability, relay) = (handler.clone(), capability.clone(), relay.clone());
            async move {
                if let Some(result) = answer(&shared, &capability, event, &relay).await {
                    reply.send(result).await;
                }
            }
        },
    )
    .await;
}

/// The sealed CJ result for one request, if the request earns one. A
/// request that NIP-HOST answers with no signed reply, such as an unknown
/// invitation or an expired request, earns no result here either.
async fn answer(
    shared: &Arc<Shared>,
    capability: &Capability,
    event: Event,
    relay: &str,
) -> Option<Event> {
    let now = unix_time().ok()?;
    match intake(&event, &shared.secret, capability, now) {
        Intake::Ignore => None,
        Intake::Refuse(result) => Some(*result),
        Intake::Call { request, answering } => {
            let reply = host_request(shared, *request, relay).await?;
            answering
                .answer(&shared.secret, &reply, unix_time().ok()?)
                .ok()
        }
    }
}
