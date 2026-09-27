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
use nostr_transport::Connection;
use serde_json::json;

use super::Shared;
use super::relay::host_request;
use crate::unix_time;

/// One subscription connection's lifetime. The loop reconnects after it.
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
    loop {
        let Ok(mut socket) = Connection::connect(&relay, &shared.secret, LIFETIME).await else {
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        };
        let since = unix_time().unwrap_or_default().saturating_sub(60);
        let filter = json!({
            "kinds": [execution::REQUEST_KIND],
            "#p": [shared.host_key],
            "since": since
        });
        if socket
            .send(json!(["REQ", SUBSCRIPTION, filter]))
            .await
            .is_err()
        {
            continue;
        }
        // Execution kinds are ephemeral: a request sent while this loop
        // reconnects is not replayed, and the device retries the same
        // request, which the host answers as a retransmission.
        while let Ok(frame) = socket.next().await {
            if frame[0] == "CLOSED" && frame[1] == SUBSCRIPTION {
                break;
            }
            if frame[0] != "EVENT" || frame[1] != SUBSCRIPTION {
                continue;
            }
            let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
                continue;
            };
            if let Some(result) = answer(&shared, &capability, event, &relay).await
                && socket.send(json!(["EVENT", result])).await.is_err()
            {
                break;
            }
        }
    }
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
