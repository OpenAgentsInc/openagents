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

/// Send `job` to the runner `runner` as `key`, with `extra` tags on the
/// request (an agent's identity), and wait up to `wait` for its result.
/// `progress` gets each progress line.
///
/// # Errors
///
/// The runner's refusal, or why it didn't answer.
pub async fn ask_runner(
    url: &str,
    key: &crate::keys::Signer,
    runner: &str,
    job: &kiln::remote::Job,
    extra: &[nostr::domain::Tag],
    wait: Duration,
    progress: &dyn Fn(&str),
) -> Result<kiln::remote::Outcome, String> {
    use kiln::cj;
    let body = json!({
        "v": 1,
        "requires": [],
        "task": serde_json::to_string(job).map_err(|e| e.to_string())?,
        "transcript": [],
        "client": concat!("kiln-git ", env!("CARGO_PKG_VERSION")),
    });
    let nonce: [u8; 32] = secp256k1::rand::random();
    let request = key
        .key
        .seal_request_tagged(runner, &body, nonce, now(), extra)?;
    // A relay connection lasts at most two minutes and a run can take ten,
    // so reconnect and read the stored answers again until one is final.
    tokio::time::timeout(wait, async {
        let mut sent = false;
        loop {
            let mut socket =
                match nostr_transport::Connection::connect(url, &key.secret, LINK).await {
                    Ok(socket) => socket,
                    Err(e) if sent => {
                        progress(&format!("reconnecting: {e}"));
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                };
            socket
                .send(json!([
                    "REQ",
                    "a",
                    cj::answer_filter(key.key.pubkey(), &request.id)
                ]))
                .await?;
            if !sent {
                socket.send(json!(["EVENT", request])).await?;
                sent = true;
            }
            loop {
                let Ok(frame) = socket.next().await else {
                    break;
                };
                match frame[0].as_str() {
                    Some("OK") if frame[1] == request.id.as_str() && frame[2] == false => {
                        return Err(format!("the relay refused the request: {}", frame[3]));
                    }
                    Some("EVENT") if frame[1] == "a" => {
                        let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
                            continue;
                        };
                        if !cj::answers(&event, &request.id, runner, key.key.pubkey()) {
                            continue;
                        }
                        let Ok(plain) = key.key.open(&event) else {
                            continue;
                        };
                        match cj::read(event.kind, &plain) {
                            cj::Answer::Processing(Some(line)) => progress(&line),
                            cj::Answer::Result { text, .. } => {
                                let _ = socket.close().await;
                                return serde_json::from_str(&text).map_err(|_| {
                                    "the runner's answer is not a result".to_string()
                                });
                            }
                            cj::Answer::Error(e) => return Err(e),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    })
    .await
    .map_err(|_| format!("the runner did not answer in {} s", wait.as_secs()))?
}

/// How long one relay connection lasts while waiting on a runner.
const LINK: Duration = Duration::from_secs(110);

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
