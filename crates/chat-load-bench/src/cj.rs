//! The basic Coder's send path, leg by leg: one NIP-CJ conversation job
//! (`25900`) to the OpenAgents chat worker, its `27000` partials, and its
//! `26900` result, through `relay.openagents.com`.
//!
//! This mirrors `Relay::run` in `crates/openagents-mobile/src/basic_coder.rs`
//! (its payload, subscription, and order of frames), with a timestamp at
//! every step. Keep it in step with that file.

use nostr::domain::{Event, RelaySigner, Tag};
use nostr::kinds::{CJ_CONVERSATION_FEEDBACK, CJ_CONVERSATION_REQUEST, CJ_CONVERSATION_RESULT};
use nostr::nip44;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// `basic_coder::RELAY`.
pub const RELAY: &str = "wss://relay.openagents.com";
/// `basic_coder::WORKER`, the chat worker's public key.
pub const WORKER: &str = "32c078952ff8b1f1d6f431e30fb240b0d1f91e30f977844557e8267390e3599b";
/// `basic_coder::INSTRUCTIONS`, verbatim, so the model sees the same prompt.
const INSTRUCTIONS: &str = "You are Coder, the OpenAgents assistant, \
chatting with the user in the OpenAgents app on their phone. Answer directly and \
helpfully; use Markdown when it helps, and keep answers short on a small screen. \
In this chat you cannot run commands, read files, or reach the user's computer. \
When the user asks for work that needs a computer, such as running code or \
reading or changing a repository, say that plainly in one sentence and tell them \
to tap Run Coder below the chat: it starts Coder on their connected computer with \
this conversation, or helps them connect one first.";

/// When each step of one job happened, from the tap on Send.
#[derive(Debug, Default)]
pub struct Legs {
    /// Payload encrypted and the request signed.
    pub sealed: Duration,
    /// WebSocket and TLS open and NIP-42 AUTH acknowledged.
    pub connected: Duration,
    /// The reply subscription's EOSE: the worker answers only
    /// subscriptions open before the request.
    pub subscribed: Duration,
    /// The relay's OK for the request.
    pub accepted: Option<Duration>,
    /// The first partial with text.
    pub first_words: Option<Duration>,
    /// The result.
    pub done: Option<Duration>,
    pub partials: usize,
    pub model: Option<String>,
    pub failure: Option<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Send `text` as a new conversation from a new device key, as a new
/// install's first message.
///
/// # Errors
/// When the worker key does not parse or signing fails.
pub async fn ask(relay: &str, worker: &str, text: &str) -> Result<Legs, String> {
    let started = Instant::now();
    let mut legs = Legs::default();
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let bytes: Vec<u8> = (0..worker.len())
        .step_by(2)
        .filter_map(|at| worker.get(at..at + 2))
        .filter_map(|pair| u8::from_str_radix(pair, 16).ok())
        .collect();
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "the worker key is not 32 bytes")?;
    let worker = XOnlyPublicKey::from_byte_array(bytes).map_err(|_| "not a public key")?;
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|e| e.to_string())?;
    let me = coder_connect::protocol::pubkey(&secret);
    let key = nip44::conversation_key(&secret, &worker);
    let payload = json!({
        "v": 2,
        "requires": [],
        "task": text,
        "transcript": [{"role": "user", "content": text}],
        "instructions": INSTRUCTIONS,
        "client": "openagents-mobile",
    });
    let content = nip44::encrypt(
        &payload.to_string(),
        &key,
        secp256k1::rand::random::<[u8; 32]>(),
    )?;
    let request = signer.sign(
        coder_connect::unix_time().map_err(|e| e.to_string())?,
        CJ_CONVERSATION_REQUEST,
        vec![Tag::new(vec!["p".into(), hex(&worker.serialize())])],
        content,
    );
    legs.sealed = started.elapsed();
    let mut connection = match nostr_transport::Connection::connect(
        relay,
        &secret,
        Duration::from_secs(120),
    )
    .await
    {
        Ok(connection) => connection.with_frame_budget(2_048),
        Err(error) => {
            legs.failure = Some(error);
            return Ok(legs);
        }
    };
    legs.connected = started.elapsed();
    let subscription = format!("chat-{}", &request.id[..16]);
    let result: Result<(), String> = async {
        connection
            .send(json!(["REQ", subscription, {
                "kinds": [CJ_CONVERSATION_FEEDBACK, CJ_CONVERSATION_RESULT],
                "#p": [me],
                "#e": [request.id],
            }]))
            .await?;
        loop {
            let frame = connection.next().await?;
            if frame[0] == "EOSE" && frame[1] == subscription.as_str() {
                break;
            }
            if frame[0] == "CLOSED" {
                return Err(frame[2].as_str().unwrap_or("closed").to_owned());
            }
        }
        legs.subscribed = started.elapsed();
        connection.send(json!(["EVENT", request])).await?;
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(60), connection.next())
                .await
                .map_err(|_| "no answer in 60 s".to_string())??;
            match frame[0].as_str() {
                Some("OK") if frame[1] == request.id.as_str() => {
                    legs.accepted = Some(started.elapsed());
                    if frame[2] == false {
                        return Err(frame[3].as_str().unwrap_or("refused").to_owned());
                    }
                }
                Some("CLOSED") => return Err(frame[2].as_str().unwrap_or("closed").to_owned()),
                Some("EVENT") if frame[1] == subscription.as_str() => {
                    let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
                        continue;
                    };
                    if event.pubkey != hex(&worker.serialize()) || event.validate_crypto().is_err()
                    {
                        continue;
                    }
                    let Some(body) = nip44::decrypt(&event.content, &key)
                        .ok()
                        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                    else {
                        continue;
                    };
                    match (event.kind, body["type"].as_str()) {
                        (CJ_CONVERSATION_FEEDBACK, Some("partial")) => {
                            legs.partials += 1;
                            if legs.first_words.is_none()
                                && body["delta"].as_str().is_some_and(|d| !d.is_empty())
                            {
                                legs.first_words = Some(started.elapsed());
                            }
                        }
                        (CJ_CONVERSATION_FEEDBACK, Some("status")) if body["status"] == "error" => {
                            return Err(format!(
                                "{}: {}",
                                body["code"].as_str().unwrap_or("error"),
                                body["message"].as_str().unwrap_or("")
                            ));
                        }
                        (CJ_CONVERSATION_RESULT, _) => {
                            legs.done = Some(started.elapsed());
                            legs.model = body["model"].as_str().map(str::to_owned);
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    .await;
    if let Err(error) = result {
        legs.failure = Some(error);
    }
    let _ = connection.send(json!(["CLOSE", subscription])).await;
    let _ = connection.close().await;
    Ok(legs)
}
