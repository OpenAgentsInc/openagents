//! One relay exchange per call: publish an event and wait for `OK`, or read
//! stored events to the end-of-stored-events marker.

use std::time::Duration;

use nostr::domain::Event;
use secp256k1::SecretKey;
use serde_json::{Value, json};

/// How long one exchange may take.
pub const WAIT: Duration = Duration::from_secs(20);

/// Publish `event` and wait for the relay to accept it.
///
/// # Errors
///
/// The relay's refusal, or why it couldn't be reached.
pub async fn publish(url: &str, secret: &SecretKey, event: &Event) -> Result<(), String> {
    tokio::time::timeout(WAIT, async {
        let mut socket = nostr_transport::Connection::connect(url, secret, WAIT).await?;
        socket.send(json!(["EVENT", event])).await?;
        loop {
            let frame = socket.next().await?;
            if frame[0] == "OK" && frame[1] == event.id.as_str() {
                let _ = socket.close().await;
                return if frame[2] == true {
                    Ok(())
                } else {
                    Err(format!("the relay refused it: {}", frame[3]))
                };
            }
        }
    })
    .await
    .map_err(|_| format!("{url} did not answer in {} s", WAIT.as_secs()))?
}

/// Read the stored events that match `filter`.
///
/// # Errors
///
/// Why the relay couldn't be read.
pub async fn query(url: &str, secret: &SecretKey, filter: Value) -> Result<Vec<Event>, String> {
    tokio::time::timeout(WAIT, async {
        let mut socket = nostr_transport::Connection::connect(url, secret, WAIT).await?;
        socket.send(json!(["REQ", "q", filter])).await?;
        let mut found = Vec::new();
        loop {
            let frame = socket.next().await?;
            match frame[0].as_str() {
                Some("EVENT") if frame[1] == "q" => {
                    if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone())
                        && event.validate_crypto().is_ok()
                    {
                        found.push(event);
                    }
                }
                Some("EOSE") if frame[1] == "q" => break,
                Some("CLOSED") if frame[1] == "q" => {
                    return Err(format!("the relay closed the query: {}", frame[2]));
                }
                _ => {}
            }
        }
        let _ = socket.close().await;
        Ok(found)
    })
    .await
    .map_err(|_| format!("{url} did not answer in {} s", WAIT.as_secs()))?
}
