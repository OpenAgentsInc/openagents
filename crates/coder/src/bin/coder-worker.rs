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
//! `CODER_WORKER_ALLOW` names the customers this worker answers, as a
//! comma-separated list of `npub` or hex public keys. A request names a
//! worker with a `p` tag and public keys are not secret, so a worker that
//! spends a door key on whoever finds it has no spend control at all. A
//! request from anyone else is refused with the typed code `not_admitted`
//! rather than dropped, because a silent refusal looks like an outage to
//! the terminal. Unset, the worker answers every request, which is right
//! for a local relay and wrong for a public one.
//!
//! Jobs run concurrently, `CODER_WORKER_JOBS` at once; unset, an executor
//! door runs as many as its manifest's `concurrent_max` and a model door
//! runs four. A job that arrives with every slot taken is refused with the
//! typed code `busy` at once, so the terminal can route it elsewhere
//! rather than wait behind work it cannot see.
//!
//! Two request shapes go beyond an ordinary turn. A payload with
//! `"type": "probe"` is answered without generating, with the door's name
//! and model, so a terminal can learn the worker is there. A payload with a
//! `delegation` object (`writes`, `minutes`) is one bounded task from a
//! fan-out on the terminal's side; an executor door runs it under its own
//! approval and boundary with those bounds, and a model door refuses a
//! writing one rather than answering an edit with prose. Read
//! [`docs/coder/worker-executor.md`] for that path.
//!
//! Read [`docs/coder/relay-transport.md`] for the proof this binary was
//! written to make possible.

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use coder::generate::{
    Door, Generate, GenerateError, Lane, Message, Role, Usage, WORKER_MODEL_VAR, model_from_env,
};
use coder::relay::{
    DEFAULT_RELAY_URL, FEEDBACK_KIND, Identity, PAYLOAD_VERSION, REQUEST_KIND, RESULT_KIND,
    connect, parse_pubkey, partial_payload, payload_version, send,
};
use futures_util::StreamExt;
use nostr::domain::{Event, Tag};
use nostr::nip44;
use secp256k1::XOnlyPublicKey;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;
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
CODER_RELAY picks the relay. CODER_WORKER_ALLOW, when set, lists the
customer pubkeys (npub or hex, comma-separated) this worker answers; any
other request is refused with code not_admitted. CODER_WORKER_JOBS bounds
how many jobs run at once; the rest are refused busy. The door the worker
answers through comes from the environment exactly as it does for the
agent, except for the lane: CODER_WORKER_MODEL names the model or lane
this worker runs, and outranks CODER_MODEL.";

/// What the command line asked for.
struct Options {
    once: bool,
    decline: Option<String>,
    /// Customers this worker answers; `None` admits everyone.
    allow: Option<Vec<String>>,
}

/// The environment variable that lists admitted customers.
const ALLOW_VAR: &str = "CODER_WORKER_ALLOW";

/// The environment variable that bounds how many jobs run at once.
const JOBS_VAR: &str = "CODER_WORKER_JOBS";

/// Jobs at once for a model door when `CODER_WORKER_JOBS` is unset.
const DEFAULT_JOBS: usize = 4;

/// Reads `CODER_WORKER_ALLOW` into hex pubkeys, or `None` when unset.
///
/// An entry that does not parse is an error, not an admitted nobody: a
/// typo in the one list that controls spend must stop the worker.
fn allowed_from_env() -> Result<Option<Vec<String>>, String> {
    let Ok(text) = env::var(ALLOW_VAR) else {
        return Ok(None);
    };
    allowed(&text).map(Some)
}

fn allowed(text: &str) -> Result<Vec<String>, String> {
    let mut keys = Vec::new();
    for entry in text
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let key = parse_pubkey(entry)
            .ok_or_else(|| format!("{ALLOW_VAR}: {entry} is not an npub or 64 hex"))?;
        keys.push(hex(&key.serialize()));
    }
    if keys.is_empty() {
        return Err(format!("{ALLOW_VAR} is set but names no pubkey"));
    }
    Ok(keys)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
    Ok(Options {
        once,
        decline,
        allow: allowed_from_env()?,
    })
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
///
/// Jobs run concurrently, up to the worker's bound; the socket stays with
/// this loop, and every job publishes through one channel it drains, so
/// two jobs' frames never interleave on the wire.
async fn serve(options: &Options) -> Result<(), String> {
    let secret = env::var("CODER_WORKER_SECRET")
        .map_err(|_| "CODER_WORKER_SECRET is not set".to_string())?;
    let identity = Arc::new(Identity::from_text(&secret, "CODER_WORKER_SECRET")?);
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
    let door = Arc::new(door);
    let jobs = jobs_bound(&door)?;

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
    eprintln!("jobs    {jobs} at once; more are refused busy");
    if let Some(code) = &options.decline {
        eprintln!("declining every job with {code}");
    }
    match &options.allow {
        Some(keys) => eprintln!("admits  {} customer(s)", keys.len()),
        None => eprintln!("admits  every customer ({ALLOW_VAR} unset)"),
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

    let (outgoing, mut frames) = mpsc::unbounded_channel::<Value>();
    let running = Arc::new(Semaphore::new(jobs));
    let mut tasks: JoinSet<Result<(), String>> = JoinSet::new();
    let mut answered = 0usize;
    loop {
        tokio::select! {
            frame = frames.recv() => {
                let Some(frame) = frame else { continue };
                send(&mut socket, frame).await.map_err(|error| error.to_string())?;
            }
            Some(ended) = tasks.join_next(), if !tasks.is_empty() => {
                match ended {
                    Ok(Ok(())) => {}
                    Ok(Err(why)) => eprintln!("job: {why}"),
                    Err(join) => eprintln!("job: {join}"),
                }
                answered += 1;
                if options.once && answered >= 1 {
                    // The job's frames were queued before its task ended;
                    // drain them before the socket goes.
                    while let Ok(frame) = frames.try_recv() {
                        send(&mut socket, frame).await.map_err(|error| error.to_string())?;
                    }
                    return Ok(());
                }
            }
            frame = socket.next() => {
                let Some(frame) = frame else {
                    return Err("the relay closed the socket".to_string());
                };
                let frame = frame.map_err(|error| format!("socket: {error}"))?;
                let tungstenite::Message::Text(text) = frame else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if value[1].as_str() != Some("jobs") {
                    continue;
                }
                // The relay buffers live events until the history query
                // behind a REQ finishes, and a CLOSED subscription receives
                // nothing at all. Both are the worker's business to report,
                // because from the terminal each looks like a worker that
                // is not there.
                match value[0].as_str() {
                    Some("EOSE") => {
                        eprintln!("subscribed; jobs arrive live from here");
                        continue;
                    }
                    Some("CLOSED") => {
                        return Err(format!(
                            "the relay closed the jobs subscription: {}",
                            value[2].as_str().unwrap_or_default()
                        ));
                    }
                    Some("EVENT") => {}
                    _ => continue,
                }
                let Ok(request) = serde_json::from_value::<Event>(value[2].clone()) else {
                    continue;
                };
                if request.kind != REQUEST_KIND || request.validate_crypto().is_err() {
                    continue;
                }
                // Admission is decided here, before anything is spawned: a
                // job over the bound is refused `busy` at once rather than
                // queued behind work the terminal cannot see.
                let permit = running.clone().try_acquire_owned().ok();
                let job = Job {
                    identity: identity.clone(),
                    door: door.clone(),
                    decline: options.decline.clone(),
                    allow: options.allow.clone(),
                    publish: outgoing.clone(),
                    permit,
                };
                tasks.spawn(async move { job.answer(&request).await });
            }
        }
    }
}

/// How many jobs run at once.
///
/// `CODER_WORKER_JOBS` states it. Unset, an executor door runs as many as
/// its manifest claims are safe at once, and a model door runs
/// [`DEFAULT_JOBS`].
fn jobs_bound(door: &Door) -> Result<usize, String> {
    match env::var(JOBS_VAR) {
        Ok(text) if !text.is_empty() => text
            .parse::<usize>()
            .ok()
            .filter(|jobs| *jobs > 0)
            .ok_or_else(|| format!("{JOBS_VAR} must be a positive whole number")),
        _ => Ok(match door {
            Door::Executor(executor) => executor.concurrent_max().max(1),
            _ => DEFAULT_JOBS,
        }),
    }
}

/// One job request and everything answering it needs.
struct Job {
    identity: Arc<Identity>,
    door: Arc<Door>,
    decline: Option<String>,
    allow: Option<Vec<String>>,
    /// Frames for the serving loop to write to the socket, in order.
    publish: mpsc::UnboundedSender<Value>,
    /// A slot under the concurrency bound, or `None` when every slot was
    /// taken at admission and the job is refused `busy`.
    permit: Option<OwnedSemaphorePermit>,
}

impl Job {
    /// Answers one job request: decrypt, generate, publish.
    async fn answer(self, request: &Event) -> Result<(), String> {
        let customer = parse_hex(&request.pubkey)
            .and_then(|bytes| XOnlyPublicKey::from_byte_array(bytes).ok())
            .ok_or("the request's pubkey does not parse")?;
        let conversation = nip44::conversation_key(self.identity.secret(), &customer);
        let plaintext = nip44::decrypt(&request.content, &conversation)
            .map_err(|error| format!("decrypt: {error}"))?;
        let payload: Value =
            serde_json::from_str(&plaintext).map_err(|error| format!("payload: {error}"))?;
        let label = &request.id[..16];

        let publish = |kind: u16, content: Value| -> Result<(), String> {
            let ciphertext = nip44::encrypt(
                &content.to_string(),
                &conversation,
                secp256k1::rand::random::<[u8; 32]>(),
            )
            .map_err(|error| format!("encrypt: {error}"))?;
            let event = self.identity.signer().sign(
                unix_now(),
                kind,
                vec![
                    Tag::new(vec!["e".into(), request.id.clone()]),
                    Tag::new(vec!["p".into(), request.pubkey.clone()]),
                ],
                ciphertext,
            );
            self.publish
                .send(json!(["EVENT", event]))
                .map_err(|_| "the serving loop is gone".to_string())
        };
        let refuse = |version: u64, code: &str, message: String| -> Result<(), String> {
            publish(
                FEEDBACK_KIND,
                json!({
                    "v": version,
                    "type": "status",
                    "status": "error",
                    "code": code,
                    "message": message,
                }),
            )?;
            eprintln!("job {label} declined: {code}");
            Ok(())
        };

        // The worker answers at the version the request named, so a
        // version-1 terminal gets version-1 feedback — partials with no
        // `seq`, since it cannot check one — and a request that names
        // anything else is declined rather than generated against a schema
        // this worker cannot read.
        let Some(version) = payload_version(&payload) else {
            return refuse(
                PAYLOAD_VERSION,
                "unsupported_version",
                "the job request names a payload version this worker does not serve".to_string(),
            );
        };

        if let Some(keys) = &self.allow
            && !keys.contains(&request.pubkey)
        {
            return refuse(
                version,
                "not_admitted",
                "this worker does not answer requests from your pubkey".to_string(),
            );
        }

        if let Some(code) = &self.decline {
            return refuse(
                version,
                code,
                format!("this worker is configured to decline every job ({code})"),
            );
        }

        // A probe asks whether this worker is here and what answers
        // through it. It costs nothing at the door: the answer is the
        // door's name and model, which is what a capability probe records
        // as the version.
        if payload["type"].as_str() == Some("probe") {
            publish(
                RESULT_KIND,
                json!({
                    "v": version,
                    "type": "result",
                    "text": format!("{} {}", self.door.name(), self.door.model()),
                    "model": self.door.model(),
                    "probe": {
                        "door": self.door.name(),
                        "model": self.door.model(),
                        "delegates": matches!(*self.door, Door::Executor(_)),
                    },
                }),
            )?;
            eprintln!("job {label} probed");
            return Ok(());
        }

        if self.permit.is_none() {
            return refuse(
                version,
                "busy",
                "this worker is running as many jobs as it admits at once".to_string(),
            );
        }

        let started = Instant::now();
        // A request that carries a `delegation` is one bounded task from a
        // fan-out on the terminal's side: the terminal holds no executor,
        // so the bounds it states are applied here, under this worker's
        // approval. A worker with no executor door cannot hold them and
        // says so rather than answering a writing task with prose.
        let answered = match (&payload["delegation"], &*self.door) {
            (Value::Object(delegation), Door::Executor(executor)) => {
                let writes = delegation["writes"].as_bool().unwrap_or(false);
                let minutes = delegation["minutes"].as_u64().unwrap_or(0);
                let prompt = payload["task"].as_str().unwrap_or_default();
                eprintln!(
                    "job {label} delegated: {} task, {minutes} min",
                    if writes { "writing" } else { "reading" }
                );
                executor
                    .delegate(prompt, writes, minutes)
                    .await
                    .map(|text| (text, None))
            }
            (Value::Object(delegation), _) if delegation["writes"].as_bool() == Some(true) => {
                return refuse(
                    version,
                    "internal",
                    format!(
                        "this worker answers through {}, which cannot run a writing task; \
                         it needs an executor door",
                        self.door.name()
                    ),
                );
            }
            _ => {
                let instructions = payload["instructions"].as_str().unwrap_or_default();
                let input = transcript(&payload);
                self.generate(version, instructions, &input, &publish).await
            }
        };

        match answered {
            Ok((text, usage)) => {
                publish(
                    RESULT_KIND,
                    json!({
                        "v": version,
                        "type": "result",
                        "text": text,
                        "usage": usage.map(|usage| json!({
                            "input": usage.input_tokens,
                            "output": usage.output_tokens,
                        })),
                        "model": self.door.model(),
                    }),
                )?;
                eprintln!(
                    "job {label} answered in {} ms, {} chars",
                    started.elapsed().as_millis(),
                    text.len()
                );
            }
            // The door said no with a code: the code travels as it is, so
            // the terminal can tell an executor's refusal from a failure.
            Err(GenerateError::Refused { code, message }) => {
                refuse(version, &code, message)?;
            }
            Err(error) => {
                // The worker's own door failed. That is the worker's
                // problem and the caller should hear it as one, with a
                // code, rather than as silence it cannot tell from an
                // absent worker.
                refuse(version, "internal", error.to_string())?;
                eprintln!("job {label} failed: {error}");
            }
        }
        Ok(())
    }

    /// Generates through the door, publishing partials as text collects.
    async fn generate(
        &self,
        version: u64,
        instructions: &str,
        input: &[Message],
        publish: &(dyn Fn(u16, Value) -> Result<(), String> + Sync),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        // The `Generate` sink is synchronous and publishing wants the
        // version and a sequence, so deltas go down a channel and the
        // loop below drains it while the generation runs. Dropping the
        // sender is what ends the drain.
        let (deltas, mut incoming) = mpsc::unbounded_channel::<String>();
        let generating = async {
            let mut sink = |delta: &str| {
                let _ = deltas.send(delta.to_string());
            };
            let answered = self
                .door
                .generate(instructions, input, &mut sink, &mut |_| {})
                .await;
            drop(deltas);
            answered
        };
        tokio::pin!(generating);

        let mut buffer = String::new();
        let mut partial_seq = 0u64;
        let mut draining = true;
        loop {
            tokio::select! {
                delta = incoming.recv(), if draining => match delta {
                    Some(delta) => {
                        buffer.push_str(&delta);
                        if buffer.len() >= PARTIAL_BYTES {
                            // `seq` is the signed ordering the terminal
                            // checks deltas against; arrival order proves
                            // nothing. A version-1 answer makes no such
                            // promise and carries none.
                            publish(FEEDBACK_KIND, partial_payload(version, partial_seq, &buffer))
                                .map_err(GenerateError::Stream)?;
                            partial_seq += 1;
                            buffer.clear();
                        }
                    }
                    // The generation dropped the sender, so nothing more is
                    // coming and the branch would otherwise spin.
                    None => draining = false,
                },
                answered = &mut generating => return answered,
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use coder::generate::StubGenerate;
    use secp256k1::SecretKey;

    // Exercise the worker's response path in process. The stub door needs
    // no credentials and never makes a model request.
    async fn response(payload: Value, decline: Option<&str>) -> Value {
        response_admitting(payload, decline, None, true).await
    }

    fn identities() -> (Identity, Identity, [u8; 32]) {
        let worker = Identity::from_secret(SecretKey::from_byte_array([41; 32]).unwrap()).unwrap();
        let client = Identity::from_secret(SecretKey::from_byte_array([42; 32]).unwrap()).unwrap();
        let public = XOnlyPublicKey::from_byte_array(parse_hex(client.pubkey()).unwrap()).unwrap();
        let conversation = nip44::conversation_key(worker.secret(), &public);
        (worker, client, conversation)
    }

    async fn response_admitting(
        payload: Value,
        decline: Option<&str>,
        allow: Option<Vec<String>>,
        admitted: bool,
    ) -> Value {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let (worker, client, conversation) = identities();
            let content = nip44::encrypt(&payload.to_string(), &conversation, [43; 32]).unwrap();
            let request = client.signer().sign(
                unix_now(),
                REQUEST_KIND,
                vec![Tag::new(vec!["p".into(), worker.pubkey().to_string()])],
                content,
            );
            let (publish, mut frames) = mpsc::unbounded_channel();
            let slots = Arc::new(Semaphore::new(1));
            let permit = admitted.then(|| slots.clone().try_acquire_owned().unwrap());
            let job = Job {
                identity: Arc::new(worker),
                door: Arc::new(Door::Stub(StubGenerate::default())),
                decline: decline.map(str::to_owned),
                allow,
                publish,
                permit,
            };
            job.answer(&request).await.unwrap();
            let value = frames.recv().await.unwrap();
            let event: Event = serde_json::from_value(value[1].clone()).unwrap();
            event.validate_crypto().unwrap();
            assert!(event.tag_values("e").any(|id| id == request.id));
            assert!(event.tag_values("p").any(|key| key == client.pubkey()));
            serde_json::from_str(&nip44::decrypt(&event.content, &conversation).unwrap()).unwrap()
        })
        .await
        .expect("the local worker response must finish")
    }

    #[tokio::test]
    async fn the_worker_preserves_supported_request_versions() {
        for version in [1, 2] {
            let result = response(json!({"v":version,"task":"hello"}), None).await;
            assert_eq!(result["v"], version);
            assert_eq!(result["type"], "result");
            assert!(!result["text"].as_str().unwrap().is_empty());
            let refused =
                response(json!({"v":version,"task":"hello"}), Some("quota_exhausted")).await;
            assert_eq!(refused["v"], version);
            assert_eq!(refused["code"], "quota_exhausted");
        }
    }

    #[tokio::test]
    async fn a_customer_off_the_allowlist_is_refused_with_a_typed_code() {
        let client = Identity::from_secret(SecretKey::from_byte_array([42; 32]).unwrap()).unwrap();
        let stranger =
            Identity::from_secret(SecretKey::from_byte_array([44; 32]).unwrap()).unwrap();
        let payload = json!({"v":2,"task":"hello"});
        let admitted = response_admitting(
            payload.clone(),
            None,
            Some(vec![client.pubkey().to_string()]),
            true,
        )
        .await;
        assert_eq!(admitted["type"], "result");
        let refused = response_admitting(
            payload,
            None,
            Some(vec![stranger.pubkey().to_string()]),
            true,
        )
        .await;
        assert_eq!(refused["type"], "status");
        assert_eq!(refused["code"], "not_admitted");
        assert!(refused.get("text").is_none());
    }

    /// A job that finds every slot taken is refused `busy` before any
    /// generation, and a probe is answered regardless: it is how a
    /// terminal learns the worker is here at all.
    #[tokio::test]
    async fn a_full_worker_refuses_busy_and_still_answers_probes() {
        let busy = response_admitting(json!({"v":2,"task":"hello"}), None, None, false).await;
        assert_eq!(busy["type"], "status");
        assert_eq!(busy["code"], "busy");

        let probed = response_admitting(json!({"v":2,"type":"probe"}), None, None, false).await;
        assert_eq!(probed["type"], "result");
        assert_eq!(probed["probe"]["door"], "stub");
        assert_eq!(probed["probe"]["delegates"], false);
        assert_eq!(probed["model"], Door::Stub(StubGenerate::default()).model());
    }

    /// A writing delegation needs an executor door. A model door answers
    /// a reading one as a prompt and refuses a writing one with a reason,
    /// rather than describing an edit it cannot make.
    #[tokio::test]
    async fn a_model_door_refuses_a_writing_delegation() {
        let reading = response(
            json!({"v":2,"task":"hello","delegation":{"writes":false,"minutes":1}}),
            None,
        )
        .await;
        assert_eq!(reading["type"], "result");

        let writing = response(
            json!({"v":2,"task":"edit a file","delegation":{"writes":true,"minutes":1}}),
            None,
        )
        .await;
        assert_eq!(writing["type"], "status");
        assert_eq!(writing["code"], "internal");
        assert!(
            writing["message"]
                .as_str()
                .unwrap()
                .contains("executor door"),
            "{writing}"
        );
    }

    #[test]
    fn the_allowlist_reads_npub_and_hex_and_refuses_typos() {
        let client = Identity::from_secret(SecretKey::from_byte_array([42; 32]).unwrap()).unwrap();
        let public = XOnlyPublicKey::from_byte_array(parse_hex(client.pubkey()).unwrap()).unwrap();
        let npub = nostr::nip19::encode_npub(&public.serialize());
        let keys = allowed(&format!(" {npub}, {}", client.pubkey())).unwrap();
        assert_eq!(keys, vec![client.pubkey().to_string(); 2]);
        assert!(allowed("").is_err());
        assert!(allowed("not-a-key").is_err());
    }

    #[tokio::test]
    async fn unsupported_requests_refuse_before_generation_or_configured_decline() {
        for payload in [json!({"task":"hello"}), json!({"v":99,"task":"hello"})] {
            for decline in [None, Some("quota_exhausted")] {
                let result = response(payload.clone(), decline).await;
                assert_eq!(result["type"], "status");
                assert_eq!(result["code"], "unsupported_version");
                assert!(result.get("text").is_none());
            }
        }
    }
}
