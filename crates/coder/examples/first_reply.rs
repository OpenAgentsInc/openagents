//! Measure the phone's send → first visible reply path through a relay.
//!
//! Acts as the OpenAgents app's basic Coder does: a fresh key per run, a
//! fresh relay connection per turn (connect, NIP-42, REQ, EOSE), then the
//! NIP-CJ `25900` request, and prints when each kind of answer arrives.
//!
//! ```sh
//! FIRST_REPLY_WORKER=<worker hex> CODER_RELAY=wss://relay.openagents.com \
//!   cargo run -p coder --example first_reply -- "what is a closure in rust?" 3
//! ```
//!
//! One line per run, milliseconds from the start of the turn:
//! `connect`, `eose`, `ack` (first feedback of any type), `judgment`,
//! `first_partial`, `result`. `FIRST_REPLY_RANK=1` sends a `rank` job
//! instead. Read `docs/coder/measurements/2026-09-28-first-reply.md`.

use std::time::Instant;

use coder::relay::{
    DEFAULT_RELAY_URL, FEEDBACK_KIND, Identity, REQUEST_KIND, RESULT_KIND, connect, parse_pubkey,
    send,
};
use futures_util::StreamExt;
use nostr::domain::{Event, Tag};
use nostr::nip44;
use secp256k1::SecretKey;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite;

#[tokio::main]
async fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let task = args
        .next()
        .unwrap_or_else(|| "In one sentence, what is a closure in Rust?".to_string());
    let runs: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(3);
    let worker_text = std::env::var("FIRST_REPLY_WORKER").map_err(|_| "set FIRST_REPLY_WORKER")?;
    let worker = parse_pubkey(&worker_text).ok_or("FIRST_REPLY_WORKER is not a key")?;
    let worker_hex: String = worker
        .serialize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let url = std::env::var("CODER_RELAY").unwrap_or_else(|_| DEFAULT_RELAY_URL.to_string());
    for run in 0..runs {
        let identity = Identity::from_secret(SecretKey::new(&mut secp256k1::rand::rng()))?;
        let conversation = nip44::conversation_key(identity.secret(), &worker);
        let started = Instant::now();
        let ms = |at: Instant| at.duration_since(started).as_millis();
        let mut socket = connect(&url, &identity).await.map_err(|e| e.to_string())?;
        let connected = Instant::now();
        let payload = json!({
            "v": 2, "requires": [], "task": task,
            "transcript": [{"role": "user", "content": task}],
            "instructions": std::env::var("FIRST_REPLY_INSTRUCTIONS")
                .unwrap_or_else(|_| PHONE_INSTRUCTIONS.to_string()),
            "client": "first-reply-bench",
            "opener": std::env::var_os("FIRST_REPLY_NO_OPENER").is_none(),
        });
        // `FIRST_REPLY_RANK=1` asks for a ranking of three suggestions
        // instead of a turn, with the task as the conversation so far.
        let payload = if std::env::var_os("FIRST_REPLY_RANK").is_some() {
            json!({
                "v": 2, "requires": [], "type": "rank", "draft": "",
                "transcript": [{"role": "user", "content": task}],
                "candidates": [
                    {"id": "openagents", "label": "OpenAgentsInc/openagents: the product apps and Worker"},
                    {"id": "psionic", "label": "OpenAgentsInc/psionic: inference and training runtime"},
                    {"id": "new_chat", "label": "Start a new chat"},
                ],
            })
        } else {
            payload
        };
        let content = nip44::encrypt(
            &payload.to_string(),
            &conversation,
            secp256k1::rand::random::<[u8; 32]>(),
        )?;
        let request: Event = identity.signer().sign(
            now(),
            REQUEST_KIND,
            vec![Tag::new(vec!["p".into(), worker_hex.clone()])],
            content,
        );
        send(
            &mut socket,
            json!(["REQ", "r", {"kinds": [FEEDBACK_KIND, RESULT_KIND], "#p": [identity.pubkey()], "#e": [request.id]}]),
        )
        .await
        .map_err(|e| e.to_string())?;
        let mut eose = None;
        let (mut ack, mut judgment, mut first_partial, mut result) = (None, None, None, None);
        let mut line = String::new();
        while let Some(frame) = socket.next().await {
            let tungstenite::Message::Text(text) = frame.map_err(|e| e.to_string())? else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            match value[0].as_str() {
                Some("EOSE") if eose.is_none() => {
                    eose = Some(Instant::now());
                    send(&mut socket, json!(["EVENT", request]))
                        .await
                        .map_err(|e| e.to_string())?;
                }
                Some("EVENT") => {
                    let Ok(event) = serde_json::from_value::<Event>(value[2].clone()) else {
                        continue;
                    };
                    let Ok(plain) = nip44::decrypt(&event.content, &conversation) else {
                        continue;
                    };
                    let body: Value = serde_json::from_str(&plain).unwrap_or(Value::Null);
                    let at = Some(Instant::now());
                    ack = ack.or(at);
                    match body["type"].as_str() {
                        Some("judgment") => {
                            judgment = judgment.or(at);
                            line = body["line"].as_str().unwrap_or_default().to_string();
                        }
                        Some("partial") => first_partial = first_partial.or(at),
                        Some("status") if body["status"] == "error" => {
                            return Err(format!("refused: {body}"));
                        }
                        _ => {}
                    }
                    if event.kind == RESULT_KIND {
                        if !body["ranked"].is_null() {
                            line = body["ranked"].to_string();
                        }
                        result = at;
                        break;
                    }
                }
                _ => {}
            }
        }
        let show = |at: Option<Instant>| at.map_or("-".to_string(), |at| ms(at).to_string());
        println!(
            "run {run}: connect {} eose {} ack {} judgment {} first_partial {} result {}  [{line}]",
            ms(connected),
            show(eose),
            show(ack),
            show(judgment),
            show(first_partial),
            show(result),
        );
    }
    Ok(())
}

/// The instructions the OpenAgents app sends (`basic_coder::INSTRUCTIONS`
/// in `crates/openagents-mobile`), so the model sees what a phone turn does.
const PHONE_INSTRUCTIONS: &str = "You are Coder, the OpenAgents assistant, \
chatting with the user in the OpenAgents app on their phone. Answer directly and \
helpfully; use Markdown when it helps, and keep answers short on a small screen. \
In this chat you cannot run commands, read files, or reach the user's computer. \
When the user asks for work that needs a computer, such as running code or \
reading or changing a repository, say that plainly in one sentence and tell them \
to tap Run Coder below the chat: it starts Coder on their connected computer with \
this conversation, or helps them connect one first.";

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
