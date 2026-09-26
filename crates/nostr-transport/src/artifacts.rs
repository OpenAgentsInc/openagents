//! Exact private artifact publication and retrieval over bounded authenticated sockets.
use crate::{Connection, Result};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde_json::json;
use std::time::Duration;

/// Publish one exact signed declaration. Retrying its event ID cannot mint a
/// second logical artifact; the receiver still applies durable deduplication.
pub async fn publish(url: &str, secret: &SecretKey, event: &Event) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        nostr::private_artifact::admit(event).map_err(|e| e.to_string())?;
        let mut socket = Connection::connect(url, secret, Duration::from_secs(10)).await?;
        socket.send(json!(["EVENT", event])).await?;
        for _ in 0..32 {
            let result = socket.next().await?;
            if result[0] == "OK" && result[1] == event.id {
                if result[2] != true {
                    return Err("private artifact relay refused the declaration".into());
                }
                socket.close().await?;
                return Ok(());
            }
        }
        Err("artifact publication acknowledgment unavailable".into())
    })
    .await
    .map_err(|_| "artifact publication deadline exceeded".to_owned())?
}
/// Fetch an exact retained declaration after reconnect. A missing event is
/// unavailable evidence, never permission to replace a job or assume it failed.
pub async fn fetch(url: &str, secret: &SecretKey, event_id: &str) -> Result<Event> {
    tokio::time::timeout(Duration::from_secs(10), async {
        if event_id.len() != 64 || !event_id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid private event identity".into());
        }
        let mut socket = Connection::connect(url, secret, Duration::from_secs(10)).await?;
        socket
            .send(
                json!(["REQ","private-artifact-exact",{"ids":[event_id],"kinds":[3188],"limit":1}]),
            )
            .await?;
        for _ in 0..32 {
            let value = socket.next().await?;
            if value[0] == "EVENT" && value[1] == "private-artifact-exact" {
                let event: Event =
                    serde_json::from_value(value[2].clone()).map_err(|e| e.to_string())?;
                if event.id != event_id {
                    return Err("private artifact relay returned another event".into());
                }
                nostr::private_artifact::open(&event, secret).map_err(|e| e.to_string())?;
                socket.close().await?;
                return Ok(event);
            }
            if value[0] == "EOSE" || value[0] == "CLOSED" {
                return Err("retained private declaration is unavailable to this reader".into());
            }
        }
        Err("artifact retrieval exceeded its message bound".into())
    })
    .await
    .map_err(|_| "artifact retrieval deadline exceeded".to_owned())?
}
