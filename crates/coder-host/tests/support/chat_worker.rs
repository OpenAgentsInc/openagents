//! A local NIP-42 relay with a scripted OpenAgents chat worker behind it,
//! the way `openagents chat`'s own tests run: every conversation request
//! gets a judgment, two streamed partials a quarter second apart, and a
//! result naming how many turns of context it carried. No public relay or
//! worker is reached.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::kinds::{CJ_CONVERSATION_FEEDBACK, CJ_CONVERSATION_REQUEST, CJ_CONVERSATION_RESULT};
use nostr::nip44;
use secp256k1::SecretKey;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

/// The worker's feedback payloads and its result for one request.
fn script(payload: &Value) -> (Vec<Value>, Value) {
    let turns = payload["transcript"].as_array().map_or(0, Vec::len);
    (
        vec![
            json!({"v": 2, "type": "judgment", "lane": "chat", "route": "general", "tier": "model", "model": "jev-fixture"}),
            json!({"v": 2, "type": "partial", "seq": 0, "delta": "Rain on "}),
            json!({"v": 2, "type": "partial", "seq": 1, "delta": "the roof"}),
        ],
        json!({"v": 2, "type": "result", "model": "fixture-model", "tier": "model",
            "text": format!("Rain on the roof. (turns: {turns})")}),
    )
}

/// The worker's key as the chat door names it.
pub fn worker_key(secret: &SecretKey) -> String {
    secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), secret)
        .x_only_public_key()
        .0
        .serialize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Start the relay and worker. Returns the relay URL and every request's
/// decrypted payload.
pub async fn start(worker: SecretKey) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let payloads = Arc::new(Mutex::new(vec![]));
    let seen = payloads.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let seen = seen.clone();
            tokio::spawn(async move {
                let Ok(mut socket) = tokio_tungstenite::accept_async(stream).await else {
                    return;
                };
                let send = |value: Value| Message::Text(value.to_string().into());
                let _ = socket.send(send(json!(["AUTH", "challenge"]))).await;
                let mut subscription = String::new();
                while let Some(Ok(Message::Text(text))) = socket.next().await {
                    let frame: Value = serde_json::from_str(&text).unwrap();
                    match frame[0].as_str() {
                        Some("AUTH") => {
                            let id = frame[1]["id"].clone();
                            let _ = socket.send(send(json!(["OK", id, true, ""]))).await;
                        }
                        Some("REQ") => {
                            subscription = frame[1].as_str().unwrap().to_owned();
                            let _ = socket.send(send(json!(["EOSE", subscription]))).await;
                        }
                        Some("EVENT") => {
                            let request: Event = serde_json::from_value(frame[1].clone()).unwrap();
                            if request.kind != CJ_CONVERSATION_REQUEST {
                                continue;
                            }
                            let _ = socket.send(send(json!(["OK", request.id, true, ""]))).await;
                            let bytes: [u8; 32] = (0..32)
                                .map(|at| {
                                    u8::from_str_radix(&request.pubkey[at * 2..at * 2 + 2], 16)
                                        .unwrap()
                                })
                                .collect::<Vec<u8>>()
                                .try_into()
                                .unwrap();
                            let from = secp256k1::XOnlyPublicKey::from_byte_array(bytes).unwrap();
                            let key = nip44::conversation_key(&worker, &from);
                            let payload: Value = serde_json::from_str(
                                &nip44::decrypt(&request.content, &key).unwrap(),
                            )
                            .unwrap();
                            seen.lock().unwrap().push(payload.clone());
                            let signer =
                                RelaySigner::from_secret_hex(&worker.display_secret().to_string())
                                    .unwrap();
                            let (feedback, result) = script(&payload);
                            let answers = feedback
                                .into_iter()
                                .map(|body| (CJ_CONVERSATION_FEEDBACK, body))
                                .chain(std::iter::once((CJ_CONVERSATION_RESULT, result)));
                            for (kind, body) in answers {
                                let event = signer.sign(
                                    request.created_at,
                                    kind,
                                    vec![
                                        Tag::new(vec!["e".into(), request.id.clone()]),
                                        Tag::new(vec!["p".into(), request.pubkey.clone()]),
                                    ],
                                    nip44::encrypt(&body.to_string(), &key, [9; 32]).unwrap(),
                                );
                                let _ = socket
                                    .send(send(json!(["EVENT", subscription, event])))
                                    .await;
                                tokio::time::sleep(Duration::from_millis(250)).await;
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    (url, payloads)
}
