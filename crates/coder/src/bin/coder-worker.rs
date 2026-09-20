//! `coder-worker`: the other end of the relay door.
//!
//! [`coder::relay::RelayDoor`] publishes NIP-CJ job requests and waits for
//! an answer. Something has to be there to answer them, and until this
//! binary existed nothing was — the transport had a client, a relay, and a
//! specification, and no fulfillment side, so a job request went out and
//! the only thing a run could learn was that nobody came back.
//!
//! The worker subscribes to `{ "kinds": [25900], "#p": [<its pubkey>] }`,
//! decrypts each request, answers it through an Open Responses door, and
//! publishes the reply as `27000` partial feedback and one `26900` result.
//! The relay carries ciphertext and holds nothing: every kind is
//! ephemeral, so a worker that is not connected when a request is
//! published never sees it, and there is no queue to drain.
//!
//! ```sh
//! export CODER_WORKER_SECRET=<64 hex or nsec>
//! export CODER_RELAY=wss://relay.openagents.com
//! export CODER_DOOR_KEY=…            # the door the worker answers through
//! export CODER_WORKER_MODEL=glm      # the lane this worker runs
//! coder-worker --once
//! ```
//!
//! The lane is the worker's own. [`coder::generate::WORKER_MODEL_VAR`]
//! outranks `CODER_MODEL` because the model a service pays for is not
//! automatically the model a local user would pick, and one constant
//! cannot be both.
//!
//! | Flag | Effect |
//! | --- | --- |
//! | `--once` | Answer one job, then exit. |
//! | `--decline <CODE>` | Refuse every job with this NIP-CJ error code. |
//! | `-h`, `--help` | Print the usage text. |
//!
//! `--decline` is there because a worker that refuses is one of the three
//! states a caller has to be able to tell apart, and it cannot be
//! exercised by turning something off: an absent worker and a refusing
//! worker differ precisely in that the refusing one answers.
//!
//! Read [`docs/coder/relay-transport.md`] for the proof this binary was
//! written to make possible.

use std::env;
use std::process::ExitCode;
use std::time::Instant;

use coder::generate::{
    Door, Generate, GenerateError, Lane, Message, Role, WORKER_MODEL_VAR, model_from_env,
};
use coder::relay::{
    DEFAULT_RELAY_URL, FEEDBACK_KIND, Identity, REQUEST_KIND, RESULT_KIND, Socket, connect, send,
};
use futures_util::StreamExt;
use nostr::domain::{Event, Tag};
use nostr::nip44;
use secp256k1::XOnlyPublicKey;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite;

/// How much text collects before a partial goes out.
///
/// One relay event per token delta would put the round trip in the middle
/// of every word and tell the operator more about the relay's event rate
/// than about the answer. The result carries the whole text anyway, so
/// partials are a progress signal, not the payload.
const PARTIAL_BYTES: usize = 160;

const USAGE: &str = "\
coder-worker — answer NIP-CJ job requests from a relay.

Usage: coder-worker [--once] [--decline <CODE>]

  --once             Answer one job, then exit.
  --decline <CODE>   Refuse every job with this NIP-CJ error code.
  -h, --help         Print this text.

CODER_WORKER_SECRET names the worker identity, 64 hex or an nsec.
CODER_RELAY picks the relay. The door the worker answers through comes
from the environment exactly as it does for the agent, except for the
lane: CODER_WORKER_MODEL names the model or lane this worker runs, and
outranks CODER_MODEL.";

/// What the command line asked for.
struct Options {
    once: bool,
    decline: Option<String>,
}

fn options() -> Result<Options, String> {
    let mut once = false;
    let mut decline = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--once" => once = true,
            "--decline" => {
                decline = Some(
                    arguments
                        .next()
                        .ok_or_else(|| "--decline needs a code".to_string())?,
                );
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Options { once, decline })
}

#[tokio::main]
async fn main() -> ExitCode {
    let options = match options() {
        Ok(options) => options,
        Err(why) => {
            eprintln!("coder-worker: {why}\n\n{USAGE}");
            return ExitCode::from(64);
        }
    };
    match serve(&options).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("coder-worker: {why}");
            ExitCode::FAILURE
        }
    }
}

/// Connects, subscribes, and answers jobs until the socket closes or
/// `--once` is satisfied.
async fn serve(options: &Options) -> Result<(), String> {
    let secret = env::var("CODER_WORKER_SECRET")
        .map_err(|_| "CODER_WORKER_SECRET is not set".to_string())?;
    let identity = Identity::from_text(&secret, "CODER_WORKER_SECRET")?;
    let url = env::var("CODER_RELAY").unwrap_or_else(|_| DEFAULT_RELAY_URL.to_string());
    // The worker's lane is its own. The model a service pays for is not
    // automatically the model someone would pick at their own terminal, so
    // `CODER_WORKER_MODEL` outranks the `CODER_MODEL` the door would
    // otherwise read, and a lane named for a door that cannot run it is
    // refused rather than quietly dropped.
    let mut door = Door::from_env()?;
    if let Some(model) = model_from_env(WORKER_MODEL_VAR) {
        door = door
            .serving(&model)
            .map_err(|why| format!("{WORKER_MODEL_VAR}: {why}"))?;
    }

    eprintln!("worker  {}", identity.pubkey());
    eprintln!("relay   {url}");
    // The lane is named beside the model, and the model is always there:
    // a run whose evidence cannot say which model answered cannot be
    // compared against one that used another.
    match Lane::read(door.model()) {
        Some(lane) => eprintln!(
            "door    {} ({}, lane {})",
            door.name(),
            door.model(),
            lane.name()
        ),
        None => eprintln!("door    {} ({})", door.name(), door.model()),
    }
    if let Some(code) = &options.decline {
        eprintln!("declining every job with {code}");
    }

    let mut socket = connect(&url, &identity)
        .await
        .map_err(|error| error.to_string())?;
    send(
        &mut socket,
        json!(["REQ", "jobs", { "kinds": [REQUEST_KIND], "#p": [identity.pubkey()] }]),
    )
    .await
    .map_err(|error| error.to_string())?;
    eprintln!("waiting for jobs");

    while let Some(frame) = socket.next().await {
        let frame = frame.map_err(|error| format!("socket: {error}"))?;
        let tungstenite::Message::Text(text) = frame else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if value[0].as_str() != Some("EVENT") || value[1].as_str() != Some("jobs") {
            continue;
        }
        let Ok(request) = serde_json::from_value::<Event>(value[2].clone()) else {
            continue;
        };
        if request.kind != REQUEST_KIND || request.validate_crypto().is_err() {
            continue;
        }
        match answer(&mut socket, &identity, &door, options, &request).await {
            Ok(()) => {}
            Err(why) => eprintln!("job {}: {why}", &request.id[..16]),
        }
        if options.once {
            return Ok(());
        }
    }
    Err("the relay closed the socket".to_string())
}

/// Answers one job request: decrypt, generate, publish.
async fn answer(
    socket: &mut Socket,
    identity: &Identity,
    door: &Door,
    options: &Options,
    request: &Event,
) -> Result<(), String> {
    let customer = parse_hex(&request.pubkey)
        .and_then(|bytes| XOnlyPublicKey::from_byte_array(bytes).ok())
        .ok_or("the request's pubkey does not parse")?;
    let conversation = nip44::conversation_key(identity.secret(), &customer);
    let plaintext = nip44::decrypt(&request.content, &conversation)
        .map_err(|error| format!("decrypt: {error}"))?;
    let payload: Value =
        serde_json::from_str(&plaintext).map_err(|error| format!("payload: {error}"))?;

    let publish = |kind: u16, content: Value| -> Result<Event, String> {
        let ciphertext = nip44::encrypt(
            &content.to_string(),
            &conversation,
            secp256k1::rand::random::<[u8; 32]>(),
        )
        .map_err(|error| format!("encrypt: {error}"))?;
        Ok(identity.signer().sign(
            unix_now(),
            kind,
            vec![
                Tag::new(vec!["e".into(), request.id.clone()]),
                Tag::new(vec!["p".into(), request.pubkey.clone()]),
            ],
            ciphertext,
        ))
    };

    if let Some(code) = &options.decline {
        let event = publish(
            FEEDBACK_KIND,
            json!({
                "v": 1,
                "type": "status",
                "status": "error",
                "code": code,
                "message": format!("this worker is configured to decline every job ({code})"),
            }),
        )?;
        send(socket, json!(["EVENT", event]))
            .await
            .map_err(|error| error.to_string())?;
        eprintln!("job {} declined: {code}", &request.id[..16]);
        return Ok(());
    }

    let instructions = payload["instructions"].as_str().unwrap_or_default();
    let input = transcript(&payload);
    let started = Instant::now();

    // The `Generate` sink is synchronous and publishing is not, so deltas
    // go down a channel and the loop below drains it while the generation
    // runs. Dropping the sender is what ends the drain.
    let (deltas, mut incoming) = tokio::sync::mpsc::unbounded_channel::<String>();
    let generating = async {
        let mut sink = |delta: &str| {
            let _ = deltas.send(delta.to_string());
        };
        let answered = door
            .generate(instructions, &input, &mut sink, &mut |_| {})
            .await;
        drop(deltas);
        answered
    };
    tokio::pin!(generating);

    let mut buffer = String::new();
    let mut draining = true;
    let answered: Result<_, GenerateError> = loop {
        tokio::select! {
            delta = incoming.recv(), if draining => match delta {
                Some(delta) => {
                    buffer.push_str(&delta);
                    if buffer.len() >= PARTIAL_BYTES {
                        let event = publish(
                            FEEDBACK_KIND,
                            json!({ "v": 1, "type": "partial", "delta": buffer }),
                        )?;
                        send(socket, json!(["EVENT", event]))
                            .await
                            .map_err(|error| error.to_string())?;
                        buffer.clear();
                    }
                }
                // The generation dropped the sender, so nothing more is
                // coming and the branch would otherwise spin.
                None => draining = false,
            },
            answered = &mut generating => break answered,
        }
    };

    match answered {
        Ok((text, usage)) => {
            let event = publish(
                RESULT_KIND,
                json!({
                    "v": 1,
                    "type": "result",
                    "text": text,
                    "usage": usage.map(|usage| json!({
                        "input": usage.input_tokens,
                        "output": usage.output_tokens,
                    })),
                    "model": door.model(),
                }),
            )?;
            send(socket, json!(["EVENT", event]))
                .await
                .map_err(|error| error.to_string())?;
            eprintln!(
                "job {} answered in {} ms, {} chars",
                &request.id[..16],
                started.elapsed().as_millis(),
                text.len()
            );
        }
        Err(error) => {
            // The worker's own door failed. That is the worker's problem
            // and the caller should hear it as one, with a code, rather
            // than as silence it cannot tell from an absent worker.
            let event = publish(
                FEEDBACK_KIND,
                json!({
                    "v": 1,
                    "type": "status",
                    "status": "error",
                    "code": "internal",
                    "message": error.to_string(),
                }),
            )?;
            send(socket, json!(["EVENT", event]))
                .await
                .map_err(|error| error.to_string())?;
            eprintln!("job {} failed: {error}", &request.id[..16]);
        }
    }
    Ok(())
}

/// The conversation the request carries, oldest first.
fn transcript(payload: &Value) -> Vec<Message> {
    let mut input = Vec::new();
    if let Some(turns) = payload["transcript"].as_array() {
        for turn in turns {
            let role = match turn["role"].as_str() {
                Some("assistant") => Role::Assistant,
                _ => Role::User,
            };
            input.push(Message {
                role,
                text: turn["content"].as_str().unwrap_or_default().to_string(),
            });
        }
    }
    // A request with no transcript still carries the task.
    if input.is_empty()
        && let Some(task) = payload["task"].as_str()
    {
        input.push(Message {
            role: Role::User,
            text: task.to_string(),
        });
    }
    input
}

fn parse_hex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        bytes[index] = ((high << 4) | low) as u8;
    }
    Some(bytes)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
