//! Sending a signed results publication to a relay, for
//! `gym-leaderboard sign --relay` (feature `publish`).
//!
//! One connection, one event: send it, answer a NIP-42 challenge with the
//! same key when the relay asks, send it again after the relay accepts the
//! authentication, and return the relay's `OK` verdict. A relay's `OK`
//! proves only that it stored the event; readers check the signature.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, RelaySigner, Tag};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{Connector, connect_async_tls_with_config};

const WAIT: Duration = Duration::from_secs(20);

/// Publishes `event` to the relay at `url`, signing a NIP-42
/// authentication with `signer` if the relay challenges. Returns the
/// relay's `OK` message.
///
/// # Errors
///
/// A connection failure, a timeout, or the relay's refusal.
pub fn publish(url: &str, event: &Event, signer: &RelaySigner) -> Result<String, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(async {
            tokio::time::timeout(WAIT, send(url, event, signer))
                .await
                .map_err(|_| format!("{url} didn't answer within {} s", WAIT.as_secs()))?
        })
}

async fn send(url: &str, event: &Event, signer: &RelaySigner) -> Result<String, String> {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| e.to_string())?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let (mut socket, _) = connect_async_tls_with_config(
        url,
        None,
        false,
        Some(Connector::Rustls(std::sync::Arc::new(tls))),
    )
    .await
    .map_err(|e| e.to_string())?;
    let frame = |value: Value| Message::Text(value.to_string().into());
    socket
        .send(frame(json!(["EVENT", event])))
        .await
        .map_err(|e| e.to_string())?;
    let mut auth_id: Option<String> = None;
    while let Some(message) = socket.next().await {
        let Message::Text(text) = message.map_err(|e| e.to_string())? else {
            continue;
        };
        let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let word = |i: usize| parts.get(i).and_then(Value::as_str).unwrap_or_default();
        match word(0) {
            "AUTH" => {
                let created_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let auth = signer.sign(
                    created_at,
                    22_242,
                    vec![
                        Tag(vec!["relay".into(), url.into()]),
                        Tag(vec!["challenge".into(), word(1).into()]),
                    ],
                    String::new(),
                );
                auth_id = Some(auth.id.clone());
                socket
                    .send(frame(json!(["AUTH", auth])))
                    .await
                    .map_err(|e| e.to_string())?;
            }
            "OK" if Some(word(1)) == auth_id.as_deref() => {
                if parts.get(2) != Some(&Value::Bool(true)) {
                    return Err(format!("authentication refused: {}", word(3)));
                }
                socket
                    .send(frame(json!(["EVENT", event])))
                    .await
                    .map_err(|e| e.to_string())?;
            }
            "OK" if word(1) == event.id => {
                let accepted = parts.get(2) == Some(&Value::Bool(true));
                let reason = word(3).to_owned();
                if accepted {
                    return Ok(reason);
                }
                // Refused until authenticated: the challenge answer resends.
                if reason.starts_with("auth-required:") {
                    continue;
                }
                return Err(reason);
            }
            _ => {}
        }
    }
    Err("the relay closed the connection".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A relay fixture that challenges, refuses the unauthenticated event,
    /// accepts the authentication, then accepts the resent event.
    #[test]
    fn a_challenged_publish_authenticates_and_is_accepted() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let signer = crate::signed::tests::signer("publisher");
        let parts =
            nostr::gym_results::publication(&"a".repeat(64), &"b".repeat(40), &["x".into()])
                .unwrap();
        let event = signer.sign(1, parts.kind, parts.tags, parts.content);
        let expected = event.id.clone();
        let pubkey = signer.pubkey().to_owned();
        let server = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                listener.set_nonblocking(true).unwrap();
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                let (stream, _) = listener.accept().await.unwrap();
                let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                let text = |v: Value| Message::Text(v.to_string().into());
                ws.send(text(json!(["AUTH", "challenge-1"]))).await.unwrap();
                let mut sent = 0;
                while let Some(Ok(Message::Text(frame))) = ws.next().await {
                    let parts: Vec<Value> = serde_json::from_str(&frame).unwrap();
                    match parts[0].as_str().unwrap() {
                        "EVENT" => {
                            sent += 1;
                            let id = parts[1]["id"].as_str().unwrap().to_owned();
                            assert_eq!(id, expected);
                            let verdict = if sent == 1 {
                                json!(["OK", id, false, "auth-required: sign in"])
                            } else {
                                json!(["OK", id, true, ""])
                            };
                            ws.send(text(verdict)).await.unwrap();
                        }
                        "AUTH" => {
                            let auth: Event = serde_json::from_value(parts[1].clone()).unwrap();
                            assert_eq!(auth.kind, 22_242);
                            assert_eq!(auth.pubkey, pubkey);
                            auth.validate_crypto().unwrap();
                            ws.send(text(json!(["OK", auth.id, true, ""])))
                                .await
                                .unwrap();
                        }
                        _ => {}
                    }
                }
                sent
            })
        });
        let url = format!("ws://127.0.0.1:{port}");
        assert_eq!(publish(&url, &event, &signer), Ok(String::new()));
        assert_eq!(server.join().unwrap(), 2);
    }
}
