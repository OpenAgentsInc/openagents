//! `openagents x402 native-serve`, `buy`, and `status`: the
//! `nostr:openagents:1` binding. Every record between buyer and provider is
//! a private kind 3188 artifact (NIP-44 v2, NIP-42 relay authentication)
//! whose mailbox is the purchase nonce. Payment binds to the buyer, the
//! provider, that nonce, and the request bytes; settlement and execution
//! admission share one durable ledger in `crates/x402::native`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use coder::generate::GenerateError;
use coder::relay::{Identity, RelayDoor, parse_pubkey};
use nostr::contracts::{self, ARTIFACT_ENVELOPE_KIND, ARTIFACT_MARKER, digest_bytes, jcs};
use nostr::domain::Event;
use nostr::private_artifact;
use openagents_x402::facilitator::Facilitator;
use openagents_x402::native::{
    self, BYTES_SCHEMA, Emit, NATIVE_ONLY, NO_WORKER, OPERATOR_CAUSES, Offer, PROFILE, Provider,
    PurchaseStore, RECORD_SCHEMA, Record, RecordType, Recovery, Signed, WORKER_FAILED,
    WORKER_REFUSED, WORKER_SILENT, artifact_bytes, buyer, bytes_artifact, operator_cause,
    parse_record, parse_request, parse_status,
};
use openagents_x402::server::Receiver;
use openagents_x402::{FileReplayStore, PaymentPayload, network_id};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Map, Value, json};

use crate::relay::{Client, identity_for, relay_url, unix_now};
use crate::x402::{
    Node, Spend, admit, ceiling_present, expiry, fail_wallet, flags, limits, load_policy,
    open_wallet, pay_invoice, record_payment, replay_dir, set_phase, toll_floor,
};
use crate::{Args, Output};

const SWITCHES: &[&str] = &["show-proof", "rerun-safe", "list"];
const ACK: Duration = Duration::from_secs(20);
/// How far back a provider reads stored records on start, so a request
/// sealed while it was down is still answered.
const CATCH_UP: u64 = 300;
/// How long a buyer keeps its records and outputs on the relay.
const RETAIN: u64 = 7 * 24 * 3_600;

fn usage(output: &Output, message: &str) -> u8 {
    output.usage("x402", message, "run `openagents x402 --help`")
}

fn purchases_dir() -> PathBuf {
    replay_dir()
        .parent()
        .map(|home| home.join("native"))
        .unwrap_or_else(|| PathBuf::from("native"))
}

/// One party's key: the secret that opens records and the hex it signs as.
struct Party {
    secret: SecretKey,
    pubkey: String,
    signer: nostr::domain::RelaySigner,
}

impl Party {
    fn load(profile: Option<&str>) -> Result<Self, String> {
        let identity = identity_for(profile)?;
        Ok(Self {
            secret: identity.secret,
            pubkey: identity.signer.pubkey().to_owned(),
            signer: identity.signer,
        })
    }

    /// Seal `inline` under `schema` to `recipient` in mailbox `purchase`.
    fn seal(
        &self,
        recipient: &str,
        purchase: &str,
        schema: &str,
        inline: &Value,
        now: u64,
    ) -> Result<Event, String> {
        let bytes = jcs(inline).map_err(|error| error.to_string())?;
        let body = json!({
            "v": "openagents.artifact-envelope.v1",
            "requires": [],
            "artifact": {
                "digest": digest_bytes(&bytes),
                "size": bytes.len(),
                "media_type": "application/json",
                "schema": schema,
            },
            "inline": inline,
            "issued_at": now,
            "retain_until": now + RETAIN,
        });
        let recipient = recipient
            .parse::<XOnlyPublicKey>()
            .map_err(|_| format!("{recipient} is not an x-only public key"))?;
        private_artifact::seal(
            &body,
            &self.secret,
            &recipient,
            purchase,
            now,
            verse::identity::random_bytes(),
        )
        .map_err(|error| error.to_string())
    }

    /// Open a record or bytes artifact sealed to this party.
    fn open(&self, event: &Event) -> Option<Opened> {
        let opened = private_artifact::open(event, &self.secret).ok()?;
        let schema = opened.artifact().schema.clone()?;
        let bytes = opened.inline_bytes()?.to_vec();
        Some(Opened {
            schema,
            bytes,
            digest: opened.artifact().digest.clone(),
            event_id: event.id.clone(),
            signer: event.pubkey.clone(),
        })
    }
}

/// The inline content of one opened kind 3188 event.
struct Opened {
    schema: String,
    bytes: Vec<u8>,
    digest: String,
    event_id: String,
    signer: String,
}

impl Opened {
    fn record(&self) -> Option<(Record, Signed)> {
        if self.schema != RECORD_SCHEMA {
            return None;
        }
        let record = parse_record(&self.bytes, &self.signer).ok()?;
        let value = contracts::parse_strict(&self.bytes).ok()?;
        let signed = Signed::new(&value)
            .ok()?
            .with_event(&self.event_id, &self.signer);
        Some((record, signed))
    }
}

/// The artifact reference other records use to name a published artifact.
fn reference(event: &Event, bytes: &[u8], schema: &str) -> Value {
    json!({
        "digest": digest_bytes(bytes),
        "size": bytes.len(),
        "media_type": "application/json",
        "schema": schema,
        "event": {"id": event.id, "pubkey": event.pubkey, "kind": ARTIFACT_ENVELOPE_KIND},
    })
}

fn inbox_filter(pubkey: &str, since: u64) -> Value {
    json!({
        "kinds": [ARTIFACT_ENVELOPE_KIND],
        "#p": [pubkey],
        "#t": [ARTIFACT_MARKER],
        "since": since,
    })
}

/// Seal every record in `emit` to `to` and publish it. The first refusal
/// stops the batch; a record that never reached the relay is reported, not
/// retried, because the ledger already holds it.
fn publish_records(
    client: &mut Client,
    party: &Party,
    to: &str,
    purchase: &str,
    emit: &Emit,
) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    for record in &emit.records {
        let event = party.seal(to, purchase, RECORD_SCHEMA, record, unix_now())?;
        let ack = client.publish(event, ACK)?;
        if !ack.accepted {
            return Err(format!("relay refused record {}: {}", ack.id, ack.message));
        }
        ids.push(ack.id);
    }
    Ok(ids)
}

/// Publish a run's stdout as a bytes artifact to the buyer and return its
/// reference; a failed run or a refused publication passes through as the
/// cause `finish` records.
fn publish_output(
    client: &mut Client,
    party: &Party,
    buyer: &str,
    purchase: &str,
    result: Result<Vec<u8>, &'static str>,
) -> Result<Value, &'static str> {
    let stdout = result?;
    let inline = bytes_artifact(&stdout);
    let bytes = jcs(&inline).unwrap_or_default();
    match party
        .seal(buyer, purchase, BYTES_SCHEMA, &inline, unix_now())
        .and_then(|event| client.publish(event.clone(), ACK).map(|ack| (event, ack)))
    {
        Ok((event, ack)) if ack.accepted => Ok(reference(&event, &bytes, BYTES_SCHEMA)),
        Ok(_) | Err(_) => Err("output could not be published"),
    }
}

fn run_command(program: &str, args: &[String], input: &[u8]) -> Result<Vec<u8>, &'static str> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|_| "command did not start")?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input);
    }
    let done = child
        .wait_with_output()
        .map_err(|_| "command did not finish")?;
    if done.status.success() {
        Ok(done.stdout)
    } else {
        Err("command exited with failure")
    }
}

/// How an admitted purchase's input is turned into its output.
enum Executor {
    /// Run a local command with the input on stdin.
    Command { program: String, args: Vec<String> },
    /// Hand the input to a NIP-CJ worker as one job and wait for its result.
    Job(Box<Dispatch>),
}

/// The worker a job executor posts to and the runtime it waits on.
struct Dispatch {
    door: RelayDoor,
    relay: String,
    worker: String,
    runtime: tokio::runtime::Runtime,
}

/// What an executor reports before its output is ready.
enum Progress<'a> {
    /// The running status to publish, before any work starts, so a buyer
    /// sees the phase change while the work is under way.
    Running(&'a Emit),
    /// The job record a NIP-CJ dispatch persisted.
    Dispatched(&'a Value),
}

/// The margin kept between the job's wait and the purchase's execution
/// window, so the failure is recorded before the window closes.
const WINDOW_MARGIN: u64 = 5;

/// The wait left in an execution window ending at `execute_until`.
fn window_left(execute_until: u64) -> Duration {
    Duration::from_secs(
        execute_until
            .saturating_sub(unix_now())
            .saturating_sub(WINDOW_MARGIN)
            .max(1),
    )
}

/// The stable cause a job's failure records on the purchase.
fn job_cause(error: &GenerateError) -> &'static str {
    match error {
        GenerateError::Silent { heard: false, .. } => NO_WORKER,
        GenerateError::Silent { heard: true, .. } => WORKER_SILENT,
        GenerateError::Refused { .. } => WORKER_REFUSED,
        _ => WORKER_FAILED,
    }
}

impl Executor {
    /// Move `purchase` to `running` and produce its output from `input`.
    fn run(
        &self,
        provider: &Provider<'_, FileReplayStore>,
        buyer: &str,
        purchase: &str,
        input: &[u8],
        execute_until: u64,
        report: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Vec<u8>, &'static str> {
        match self {
            Self::Command { program, args } => {
                let running = provider.start(buyer, purchase, unix_now())?;
                report(Progress::Running(&running));
                run_command(program, args, input)
            }
            Self::Job(dispatch) => {
                let Dispatch {
                    door,
                    relay,
                    worker,
                    runtime,
                } = dispatch.as_ref();
                let task = String::from_utf8_lossy(input).into_owned();
                let minutes = window_left(execute_until).as_secs().div_ceil(60).max(1);
                let payload = json!({
                    "task": task,
                    "delegation": {"writes": false, "minutes": minutes},
                    "client": concat!("openagents x402 ", env!("CARGO_PKG_VERSION")),
                });
                let prepared = door
                    .prepare(payload)
                    .map_err(|_| "job request could not be encrypted")?;
                let job = json!({
                    "relay": relay,
                    "worker": worker,
                    "request": prepared.id(),
                });
                let running = provider.start_job(buyer, purchase, Some(job.clone()), unix_now())?;
                report(Progress::Running(&running));
                report(Progress::Dispatched(&job));
                let mut sink = |_: &str| {};
                runtime
                    .block_on(door.post(&prepared, window_left(execute_until), &mut sink))
                    .map(|answer| answer.text.into_bytes())
                    .map_err(|error| job_cause(&error))
            }
        }
    }

    /// Wait for the result of a job an earlier process posted, when this
    /// executor talks to the same worker.
    fn follow(&self, job: &Value, execute_until: u64) -> Result<Vec<u8>, &'static str> {
        let Self::Job(dispatch) = self else {
            return Err(native::PROVIDER_RESTARTED);
        };
        let Dispatch {
            door,
            worker,
            runtime,
            ..
        } = dispatch.as_ref();
        if job["worker"].as_str() != Some(worker.as_str()) {
            return Err(native::PROVIDER_RESTARTED);
        }
        let request = job["request"].as_str().unwrap_or_default();
        let mut sink = |_: &str| {};
        runtime
            .block_on(door.follow(request, window_left(execute_until), &mut sink))
            .map(|answer| answer.text.into_bytes())
            .map_err(|error| job_cause(&error))
    }
}

// ---------------------------------------------------------------- provider

pub fn serve(output: &Output, words: &[String]) -> u8 {
    let (flags, command) = match words.iter().position(|word| word == "--") {
        Some(split) => (&words[..split], Some(&words[split + 1..])),
        None => (words, None),
    };
    let args = match Args::parse(flags, SWITCHES) {
        Ok(args) => args,
        Err(message) => return usage(output, &message),
    };
    let executor = match (args.option("cj"), command) {
        (Some(_), Some(_)) => {
            return usage(
                output,
                "native-serve takes either --cj WORKER or `-- CMD`, not both",
            );
        }
        (None, None) => {
            return usage(
                output,
                "native-serve needs `-- CMD [ARGS...]` or --cj WORKER",
            );
        }
        (None, Some(command)) => match command.split_first() {
            Some((program, program_args)) => Executor::Command {
                program: program.clone(),
                args: program_args.to_vec(),
            },
            None => return usage(output, "native-serve needs a command after `--`"),
        },
        (Some(worker), None) => {
            let Some(worker_key) = parse_pubkey(worker) else {
                return usage(output, "--cj takes the worker's npub or 64 lowercase hex");
            };
            let party = match Party::load(args.option("as")) {
                Ok(party) => party,
                Err(message) => return output.fail("x402", &message),
            };
            let identity = match Identity::from_secret(party.secret) {
                Ok(identity) => identity,
                Err(message) => return output.fail("x402", &message),
            };
            let cj_relay = args
                .option("cj-relay")
                .map(str::to_owned)
                .or_else(|| std::env::var("CODER_RELAY").ok().filter(|s| !s.is_empty()))
                .unwrap_or_else(|| relay_url(args.option("relay")));
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => return output.fail("x402", &format!("runtime: {error}")),
            };
            Executor::Job(Box::new(Dispatch {
                door: RelayDoor::new(cj_relay.clone(), worker_key, identity),
                relay: cj_relay,
                worker: worker_key.to_string(),
                runtime,
            }))
        }
    };
    let Some(slug) = args.option("slug") else {
        return usage(output, "native-serve needs --slug SLUG");
    };
    let amount_msat = match args.number::<u64>("msat", 0) {
        Ok(0) | Err(_) => return usage(output, "native-serve needs --msat N (N > 0)"),
        Ok(n) => n,
    };
    let timeout_secs = match expiry(&args) {
        Ok(n) => n,
        Err(message) => return usage(output, &message),
    };
    let per_buyer_hourly = match args.number::<u32>("per-buyer", 0) {
        Ok(0) => None,
        Ok(n) => Some(n),
        Err(message) => return usage(output, &message),
    };
    let seconds = match args.number::<u64>("seconds", 0) {
        Ok(n) => n,
        Err(message) => return usage(output, &message),
    };
    let party = match Party::load(args.option("as")) {
        Ok(party) => party,
        Err(message) => return output.fail("x402", &message),
    };
    let (wallet, wallet_config) = match open_wallet() {
        Ok(opened) => opened,
        Err(error) => return fail_wallet(output, error),
    };
    let Some(network) = network_id(wallet_config.network.as_str()) else {
        let _ = wallet.stop();
        return output.fail(
            "x402",
            &format!(
                "wallet network `{}` has no x402 network id; use bitcoin or testnet",
                wallet_config.network.as_str()
            ),
        );
    };
    if let Err(message) = toll_floor(&wallet_config, amount_msat) {
        let _ = wallet.stop();
        return output.fail("x402", &message);
    }
    let wallet = Arc::new(wallet);
    let receiver = Node(wallet.clone());
    let facilitator = match FileReplayStore::open(&replay_dir()) {
        Ok(store) => {
            Facilitator::with_profiles(store, nostr::x402::DEFAULT_CLOCK_SKEW, NATIVE_ONLY)
        }
        Err(error) => return output.fail("x402", &format!("replay store: {error}")),
    };
    let store = match PurchaseStore::open(&purchases_dir()) {
        Ok(store) => store,
        Err(error) => return output.fail("x402", &error.to_string()),
    };
    let provider = Provider {
        pubkey: party.pubkey.clone(),
        offer: Offer {
            capability_id: format!("{}:x402/{slug}", party.pubkey),
            operation: slug.to_owned(),
            network: network.to_owned(),
            amount_msat,
            timeout_secs,
            description: slug.to_owned(),
            per_buyer_hourly,
        },
        receiver: &receiver,
        facilitator: &facilitator,
        store: &store,
        skew: nostr::x402::DEFAULT_CLOCK_SKEW,
    };
    let relay = relay_url(args.option("relay"));
    let mut client = Client::connect(&relay, party.signer.clone());
    let started = unix_now();
    // Work a previous process left open: fail it, or with --rerun-safe fetch
    // its input again and run it once more.
    let recovered = match provider.recover(args.switch("rerun-safe"), started) {
        Ok(recovered) => recovered,
        Err(cause) => return output.fail("x402", &format!("recovery: {cause}")),
    };
    let mut reruns: HashMap<String, (String, String, native::Phase, u64)> = HashMap::new();
    let mut rerun_ids = Vec::new();
    for recovery in &recovered {
        if let Recovery::Rerun { input, .. } = recovery
            && let Some(id) = input["event"]["id"].as_str()
        {
            rerun_ids.push(id.to_owned());
        }
    }
    let mut filters = vec![inbox_filter(
        &party.pubkey,
        started.saturating_sub(CATCH_UP),
    )];
    if !rerun_ids.is_empty() {
        filters.push(json!({"ids": rerun_ids}));
    }
    if let Err(message) = client.listen(filters) {
        return output.fail("x402", &message);
    }
    let mut served = 0u64;
    for recovery in recovered {
        match recovery {
            Recovery::Follow {
                buyer,
                purchase,
                job,
                execute_until,
            } => {
                output.line(
                    &json!({
                        "event": "following",
                        "buyer": buyer,
                        "purchase": purchase,
                        "job": job,
                        "execute_until": execute_until,
                    }),
                    |value| {
                        format!(
                            "following job {} for purchase {}",
                            &value["job"]["request"].as_str().unwrap_or_default()[..16],
                            &value["purchase"].as_str().unwrap_or_default()[..16]
                        )
                    },
                );
                let result = executor.follow(&job, execute_until);
                let finished = publish_output(&mut client, &party, &buyer, &purchase, result);
                served += 1;
                let (emit, cause) = match provider.finish(&buyer, &purchase, finished, unix_now()) {
                    Ok(emit) => (emit, None),
                    Err(cause) => (Emit::default(), Some(cause)),
                };
                let published = publish_records(&mut client, &party, &buyer, &purchase, &emit);
                output.line(
                    &json!({
                        "event": "followed",
                        "buyer": buyer,
                        "purchase": purchase,
                        "phase": emit.records.last().and_then(|r| parse_status(&r["body"]).ok()).map(|s| s.phase.name()),
                        "cause": emit.records.last().and_then(|r| parse_status(&r["body"]).ok()).and_then(|s| s.cause).or(cause.map(str::to_owned)),
                        "records": published.as_ref().ok(),
                        "relay_error": published.as_ref().err(),
                    }),
                    |value| {
                        format!(
                            "followed purchase {} -> {}{}",
                            &value["purchase"].as_str().unwrap_or_default()[..16],
                            value["phase"].as_str().unwrap_or("error"),
                            value["cause"]
                                .as_str()
                                .map(|cause| format!(": {cause}"))
                                .unwrap_or_default()
                        )
                    },
                );
            }
            Recovery::Failed {
                buyer,
                purchase,
                was,
                emit,
            } => {
                let published = publish_records(&mut client, &party, &buyer, &purchase, &emit);
                output.line(
                    &json!({
                        "event": "recovered",
                        "buyer": buyer,
                        "purchase": purchase,
                        "was": was.name(),
                        "phase": "failed",
                        "cause": native::PROVIDER_RESTARTED,
                        "records": published.as_ref().ok(),
                        "relay_error": published.as_ref().err(),
                    }),
                    |value| {
                        format!(
                            "recovered purchase {} ({} -> failed: provider_restarted)",
                            &value["purchase"].as_str().unwrap_or_default()[..16],
                            value["was"].as_str().unwrap_or_default()
                        )
                    },
                );
            }
            Recovery::Rerun {
                buyer,
                purchase,
                phase,
                input,
                execute_until,
            } => {
                let digest = input["digest"].as_str().unwrap_or_default().to_owned();
                output.line(
                    &json!({
                        "event": "rerun_pending",
                        "buyer": buyer,
                        "purchase": purchase,
                        "was": phase.name(),
                        "input": input,
                        "execute_until": execute_until,
                    }),
                    |value| {
                        format!(
                            "rerun pending for purchase {} once its input arrives",
                            &value["purchase"].as_str().unwrap_or_default()[..16]
                        )
                    },
                );
                reruns.insert(digest, (buyer, purchase, phase, execute_until));
            }
        }
    }
    output.line(
        &json!({
            "event": "serving",
            "binding": PROFILE,
            "provider": party.pubkey,
            "slug": slug,
            "capability": provider.offer.capability_id,
            "amount_msat": amount_msat,
            "network": network,
            "pay_to": receiver.pay_to(),
            "relay": relay,
            "purchases": purchases_dir(),
            "executor": match &executor {
                Executor::Command { program, .. } => json!({"command": program}),
                Executor::Job(dispatch) => {
                    json!({"cj": {"relay": dispatch.relay, "worker": dispatch.worker}})
                }
            },
        }),
        |value| {
            format!(
                "serving {} on {} for {} msat as {} (payTo {})",
                value["slug"],
                value["relay"],
                value["amount_msat"],
                value["provider"],
                value["pay_to"]
            )
        },
    );
    let deadline = (seconds > 0).then(|| Instant::now() + Duration::from_secs(seconds));
    let mut inputs: HashMap<String, Vec<u8>> = HashMap::new();
    let mut seen: Vec<String> = Vec::new();
    loop {
        if deadline.is_some_and(|end| Instant::now() >= end) {
            break;
        }
        let Some(event) = client.recv(Duration::from_secs(1)) else {
            continue;
        };
        if seen.contains(&event.id) {
            continue;
        }
        seen.push(event.id.clone());
        let Some(opened) = party.open(&event) else {
            continue;
        };
        if opened.schema == BYTES_SCHEMA {
            if let Ok(value) = contracts::parse_strict(&opened.bytes)
                && let Ok(bytes) = artifact_bytes(&value)
            {
                if let Some((buyer, purchase, phase, execute_until)) = reruns.remove(&opened.digest)
                {
                    let result = if phase == native::Phase::Admitted {
                        executor.run(
                            &provider,
                            &buyer,
                            &purchase,
                            &bytes,
                            execute_until,
                            &mut |progress| match progress {
                                Progress::Running(running) => {
                                    if let Err(message) = publish_records(
                                        &mut client,
                                        &party,
                                        &buyer,
                                        &purchase,
                                        running,
                                    ) {
                                        output.line(
                                            &json!({"event": "relay_failed", "purchase": purchase, "cause": message}),
                                            |value| format!("relay failed: {}", value["cause"]),
                                        );
                                    }
                                }
                                Progress::Dispatched(job) => output.line(
                                    &json!({"event": "dispatched", "purchase": purchase, "job": job}),
                                    |value| format!("dispatched job {}", value["job"]["request"]),
                                ),
                            },
                        )
                    } else {
                        match &executor {
                            Executor::Command { program, args } => {
                                run_command(program, args, &bytes)
                            }
                            Executor::Job(_) => Err(native::PROVIDER_RESTARTED),
                        }
                    };
                    let finished = publish_output(&mut client, &party, &buyer, &purchase, result);
                    served += 1;
                    let outcome = provider.finish(&buyer, &purchase, finished, unix_now());
                    let (emit, cause) = match outcome {
                        Ok(emit) => (emit, None),
                        Err(cause) => (Emit::default(), Some(cause)),
                    };
                    let published = publish_records(&mut client, &party, &buyer, &purchase, &emit);
                    output.line(
                        &json!({
                            "event": "rerun",
                            "buyer": buyer,
                            "purchase": purchase,
                            "phase": emit.records.last().and_then(|r| parse_status(&r["body"]).ok()).map(|s| s.phase.name()),
                            "cause": cause,
                            "records": published.as_ref().ok(),
                            "relay_error": published.as_ref().err(),
                        }),
                        |value| {
                            format!(
                                "rerun purchase {} -> {}",
                                &value["purchase"].as_str().unwrap_or_default()[..16],
                                value["phase"].as_str().unwrap_or("error")
                            )
                        },
                    );
                    continue;
                }
                inputs.insert(opened.digest.clone(), bytes);
            }
            continue;
        }
        let Some((record, signed)) = opened.record() else {
            continue;
        };
        let now = unix_now();
        let buyer = record.buyer.clone();
        let purchase = record.purchase.clone();
        let log = |event: &str, extra: Value| {
            let mut doc = json!({
                "event": event,
                "type": record.kind.name(),
                "buyer": buyer,
                "purchase": purchase,
            });
            if let (Some(doc), Some(extra)) = (doc.as_object_mut(), extra.as_object()) {
                doc.extend(extra.clone());
            }
            output.line(&doc, |value| {
                format!(
                    "{} {} from {} purchase {}{}",
                    value["event"].as_str().unwrap_or_default(),
                    value["type"].as_str().unwrap_or_default(),
                    &value["buyer"].as_str().unwrap_or_default()[..16],
                    &value["purchase"].as_str().unwrap_or_default()[..16],
                    value["cause"]
                        .as_str()
                        .map(|cause| format!(": {cause}"))
                        .unwrap_or_default()
                )
            });
        };
        let outcome = match record.kind {
            RecordType::Request => provider
                .offer(&signed, now)
                .map(|emit| (emit, None))
                .map_err(|refusal| (refusal.emit, refusal.cause)),
            RecordType::Claim => match provider.claim(&signed, now) {
                Ok(admitted) => {
                    let mut chain = admitted.emit;
                    let published = publish_records(&mut client, &party, &buyer, &purchase, &chain);
                    chain.records.clear();
                    if let Err(message) = published {
                        log("relay_failed", json!({"cause": message}));
                    }
                    log("admitted", json!({"execute_until": admitted.execute_until}));
                    let digest = admitted.input["digest"].as_str().unwrap_or_default();
                    let input = inputs.remove(digest);
                    let result = match input {
                        None => Err("input artifact was not received"),
                        Some(input) => executor.run(
                            &provider,
                            &buyer,
                            &purchase,
                            &input,
                            admitted.execute_until,
                            &mut |progress| match progress {
                                Progress::Running(running) => {
                                    if let Err(message) = publish_records(
                                        &mut client,
                                        &party,
                                        &buyer,
                                        &purchase,
                                        running,
                                    ) {
                                        log("relay_failed", json!({"cause": message}));
                                    }
                                }
                                Progress::Dispatched(job) => log("dispatched", json!({"job": job})),
                            },
                        ),
                    };
                    let finished = publish_output(&mut client, &party, &buyer, &purchase, result);
                    served += 1;
                    provider
                        .finish(&buyer, &purchase, finished, unix_now())
                        .map(|emit| (emit, None))
                        .map_err(|cause| (Emit::default(), cause))
                }
                Err(refusal) => Err((refusal.emit, refusal.cause)),
            },
            RecordType::StatusQuery => provider
                .statuses(&signed)
                .map(|emit| (emit, None))
                .map_err(|cause| (Emit::default(), cause)),
            _ => Err((Emit::default(), "record type is not for a provider")),
        };
        let (emit, cause) = match outcome {
            Ok((emit, cause)) => (emit, cause),
            Err((emit, cause)) => (emit, Some(cause)),
        };
        let phase = emit
            .records
            .last()
            .and_then(|record| parse_status(&record["body"]).ok())
            .map(|status| status.phase.name().to_owned());
        match publish_records(&mut client, &party, &buyer, &purchase, &emit) {
            Ok(ids) => log(
                if cause.is_some() {
                    "refused"
                } else {
                    "answered"
                },
                json!({"records": ids, "phase": phase, "cause": cause}),
            ),
            Err(message) => log("relay_failed", json!({"cause": message})),
        }
    }
    client.close();
    let _ = wallet.stop();
    output.line(&json!({"event": "stopped", "served": served}), |value| {
        format!("stopped after serving {} run(s)", value["served"])
    });
    0
}

// ------------------------------------------------------------------- buyer

/// The NIP-CAP head `provider:slug` as the capability a request names.
fn resolve_capability(
    client: &mut Client,
    provider: &str,
    slug: &str,
) -> Result<(Value, nostr::cap::X402Descriptor), String> {
    let mut newest: Option<Event> = None;
    client.subscribe(
        vec![json!({
            "kinds": [nostr::cap::DISCOVERY_KIND],
            "authors": [provider],
            "#d": [slug],
            "#t": [nostr::cap::CAP_MARKER],
            "limit": 8,
        })],
        false,
        Duration::from_secs(15),
        |event| {
            if newest
                .as_ref()
                .is_none_or(|current| event.created_at > current.created_at)
            {
                newest = Some(event.clone());
            }
        },
    )?;
    let Some(event) = newest else {
        return Err(format!(
            "the relay has no kind {} head {provider}:{slug}; the provider must `x402 advertise --binding {PROFILE}` first",
            nostr::cap::DISCOVERY_KIND
        ));
    };
    event
        .validate_crypto()
        .map_err(|error| format!("capability {}: signature: {error}", event.id))?;
    let body: Value = serde_json::from_str(&event.content)
        .map_err(|error| format!("capability {}: {error}", event.id))?;
    let definition = nostr::cap::parse_definition(&body)
        .and_then(|definition| {
            nostr::cap::check_discovery_tags(&event.tags, &definition).map(|()| definition)
        })
        .map_err(|error| format!("capability {}: {error}", event.id))?;
    let Some(x402) = definition.x402 else {
        return Err(format!("capability {} is not a paid resource", event.id));
    };
    if !x402.bindings.iter().any(|binding| binding == PROFILE) {
        return Err(format!(
            "capability {} does not advertise {PROFILE}",
            event.id
        ));
    }
    if definition.transport != "nostr-cj" {
        return Err(format!(
            "capability {} is reached over {}, not nostr-cj",
            event.id, definition.transport
        ));
    }
    let bytes = jcs(&body).map_err(|error| error.to_string())?;
    let capability = json!({
        "id": definition.id,
        "artifact": {
            "digest": digest_bytes(&bytes),
            "size": bytes.len(),
            "media_type": "application/json",
            "schema": "openagents.capability-definition.v1",
            "event": {"id": event.id, "pubkey": event.pubkey, "kind": nostr::cap::DISCOVERY_KIND},
        },
    });
    Ok((capability, x402))
}

/// What a buyer wrote down about one purchase, so `status` can ask again
/// and no later command pays for it twice.
#[derive(serde::Serialize, serde::Deserialize)]
struct Receipt {
    provider: String,
    purchase: String,
    request: Signed,
    challenge: Option<Signed>,
    claim: Option<Signed>,
    paid: Option<String>,
    statuses: Vec<Value>,
}

fn receipts_dir() -> PathBuf {
    purchases_dir().join("bought")
}

fn save_receipt(receipt: &Receipt) -> Result<(), String> {
    let dir = receipts_dir();
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let path = dir.join(format!("{}-{}.json", receipt.provider, receipt.purchase));
    let tmp = path.with_extension("tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec(receipt).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|error| error.to_string())
}

fn load_receipt(provider: &str, purchase: &str) -> Result<Receipt, String> {
    let path = receipts_dir().join(format!("{provider}-{purchase}.json"));
    let bytes = std::fs::read(&path).map_err(|_| {
        format!(
            "no purchase {purchase} from {provider} in {}",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

/// Follow the provider's records for one purchase until a terminal status
/// or `wait`. Returns the newest status body and any output bytes.
fn follow(
    client: &mut Client,
    party: &Party,
    receipt: &mut Receipt,
    wait: Duration,
    output: &Output,
) -> (Option<Value>, Option<Vec<u8>>) {
    let deadline = Instant::now() + wait;
    let mut outputs: HashMap<String, Vec<u8>> = HashMap::new();
    let mut newest: Option<Value> = None;
    let mut wanted: Option<String> = None;
    let mut fetched: Option<String> = None;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let Some(event) = client.recv(remaining.min(Duration::from_secs(1))) else {
            continue;
        };
        if event.pubkey != receipt.provider {
            continue;
        }
        let Some(opened) = party.open(&event) else {
            continue;
        };
        if opened.schema == BYTES_SCHEMA {
            if let Ok(value) = contracts::parse_strict(&opened.bytes)
                && let Ok(bytes) = artifact_bytes(&value)
            {
                outputs.insert(opened.digest.clone(), bytes);
            }
            if fetched.is_some() && wanted.as_deref() == Some(opened.digest.as_str()) {
                break;
            }
        } else if let Some((record, _)) = opened.record()
            && record.purchase == receipt.purchase
            && record.kind == RecordType::Status
            && let Ok(status) = parse_status(&record.body)
        {
            output.line(
                &json!({
                    "event": "status",
                    "purchase": receipt.purchase,
                    "seq": status.seq,
                    "phase": status.phase.name(),
                    "cause": status.cause,
                }),
                |value| {
                    format!(
                        "status {} {}{}",
                        value["seq"],
                        value["phase"].as_str().unwrap_or_default(),
                        value["cause"]
                            .as_str()
                            .map(|cause| format!(": {cause}"))
                            .unwrap_or_default()
                    )
                },
            );
            receipt.statuses.push(record.body.clone());
            let _ = save_receipt(receipt);
            let terminal = matches!(
                status.phase,
                native::Phase::Completed | native::Phase::Failed | native::Phase::Refused
            );
            wanted = status
                .output
                .as_ref()
                .and_then(|reference| reference["digest"].as_str())
                .map(str::to_owned);
            newest = Some(record.body.clone());
            if terminal
                && wanted
                    .as_ref()
                    .is_none_or(|digest| outputs.contains_key(digest))
            {
                break;
            }
            // An output published before this subscription opened (a rerun
            // after a provider restart, or a later `status`) is read by id.
            if terminal
                && let Some(id) = status
                    .output
                    .as_ref()
                    .and_then(|reference| reference["event"]["id"].as_str())
                && fetched.as_deref() != Some(id)
            {
                fetched = Some(id.to_owned());
                let _ = client.listen(vec![json!({"ids": [id]})]);
            }
        }
    }
    let bytes = wanted.and_then(|digest| outputs.remove(&digest));
    (newest, bytes)
}

pub fn buy(output: &Output, words: &[String]) -> u8 {
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return usage(output, &message),
    };
    let Some(provider) = args.positional().first() else {
        return usage(output, "buy needs PROVIDER (hex public key)");
    };
    let provider = provider.to_ascii_lowercase();
    if provider.len() != 64 || !provider.chars().all(|c| c.is_ascii_hexdigit()) {
        return usage(output, "PROVIDER must be a 64-hex public key");
    }
    let Some(slug) = args.option("slug") else {
        return usage(output, "buy needs --slug SLUG");
    };
    let flags = match flags(&args).and_then(|flags| ceiling_present(flags).map(|()| flags)) {
        Ok(flags) => flags,
        Err(message) => return usage(output, &message),
    };
    // The provider's node key is not known until its challenge arrives, so
    // the request carries the capability's (or default) ceiling; the
    // provider-specific one is applied to the challenge before paying.
    let policy = match load_policy() {
        Ok(policy) => policy,
        Err(message) => return output.fail("x402", &message),
    };
    let expected_capability = format!("{provider}:x402/{slug}");
    let request_limits = match limits(policy.as_ref(), flags, None, Some(&expected_capability)) {
        Ok(limits) => limits,
        Err(message) => return output.fail("x402", &message),
    };
    let max_msat = request_limits.max_msat;
    let max_fee = request_limits.max_fee_msat;
    let wait = match args.number::<u64>("wait", 120) {
        Ok(n) if n > 0 => n,
        _ => return usage(output, "--wait takes seconds above zero"),
    };
    let input = match args.option("input") {
        None => Vec::new(),
        Some("-") => {
            let mut buffer = Vec::new();
            if let Err(error) = std::io::Read::read_to_end(&mut std::io::stdin(), &mut buffer) {
                return output.fail("x402", &format!("stdin: {error}"));
            }
            buffer
        }
        Some(path) => match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => return output.fail("x402", &format!("{path}: {error}")),
        },
    };
    let party = match Party::load(args.option("as")) {
        Ok(party) => party,
        Err(message) => return output.fail("x402", &message),
    };
    if party.pubkey == provider {
        return usage(output, "the buyer and the provider must be different keys");
    }
    let relay = relay_url(args.option("relay"));
    let mut client = Client::connect(&relay, party.signer.clone());
    let (capability, descriptor) = match resolve_capability(&mut client, &provider, slug) {
        Ok(found) => found,
        Err(message) => {
            client.close();
            return output.fail("x402", &message);
        }
    };
    let capability_id = capability["id"].as_str().unwrap_or_default().to_owned();
    let now = unix_now();
    let purchase: String = verse::identity::random_bytes::<32>()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if let Err(message) = client.listen(vec![inbox_filter(&party.pubkey, now.saturating_sub(5))]) {
        client.close();
        return output.fail("x402", &message);
    }

    // The input travels first, as its own artifact, so the request can name it.
    let inline = bytes_artifact(&input);
    let input_bytes = jcs(&inline).unwrap_or_default();
    let input_reference = match party
        .seal(&provider, &purchase, BYTES_SCHEMA, &inline, now)
        .and_then(|event| client.publish(event.clone(), ACK).map(|ack| (event, ack)))
    {
        Ok((event, ack)) if ack.accepted => reference(&event, &input_bytes, BYTES_SCHEMA),
        Ok((_, ack)) => {
            client.close();
            return output.fail("x402", &format!("relay refused the input: {}", ack.message));
        }
        Err(message) => {
            client.close();
            return output.fail("x402", &message);
        }
    };
    let request = match buyer::request(
        &purchase,
        &party.pubkey,
        &provider,
        capability,
        slug,
        input_reference,
        max_msat,
        max_fee,
        now,
        600,
        wait.max(600),
        7 * 24 * 3_600,
    ) {
        Ok(value) => value,
        Err(cause) => {
            client.close();
            return output.fail("x402", cause);
        }
    };
    let request_signed = match party
        .seal(&provider, &purchase, RECORD_SCHEMA, &request, now)
        .and_then(|event| client.publish(event.clone(), ACK).map(|ack| (event, ack)))
    {
        Ok((event, ack)) if ack.accepted => match Signed::new(&request) {
            Ok(signed) => signed.with_event(&event.id, &event.pubkey),
            Err(cause) => {
                client.close();
                return output.fail("x402", cause);
            }
        },
        Ok((_, ack)) => {
            client.close();
            return output.fail(
                "x402",
                &format!("relay refused the request: {}", ack.message),
            );
        }
        Err(message) => {
            client.close();
            return output.fail("x402", &message);
        }
    };
    let mut receipt = Receipt {
        provider: provider.clone(),
        purchase: purchase.clone(),
        request: request_signed.clone(),
        challenge: None,
        claim: None,
        paid: None,
        statuses: Vec::new(),
    };
    if let Err(message) = save_receipt(&receipt) {
        client.close();
        return output.fail("x402", &message);
    }
    output.line(
        &json!({"event": "requested", "purchase": purchase, "provider": provider, "capability": capability_id, "relay": relay}),
        |value| format!("requested purchase {} from {}", value["purchase"], value["provider"]),
    );

    // Wait for the challenge (or a refusal).
    let deadline = Instant::now() + Duration::from_secs(wait);
    let mut challenge: Option<(Record, Signed)> = None;
    while challenge.is_none() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            client.close();
            return output.fail(
                "x402",
                &format!(
                    "no challenge for purchase {purchase} within {wait}s; nothing was paid. Ask again with `openagents x402 status {provider} {purchase}`"
                ),
            );
        }
        let Some(event) = client.recv(remaining.min(Duration::from_secs(1))) else {
            continue;
        };
        if event.pubkey != provider {
            continue;
        }
        let Some((record, signed)) = party.open(&event).and_then(|opened| opened.record()) else {
            continue;
        };
        if record.purchase != purchase {
            continue;
        }
        match record.kind {
            RecordType::Challenge => challenge = Some((record, signed)),
            RecordType::Status => {
                if let Ok(status) = parse_status(&record.body)
                    && status.phase == native::Phase::Refused
                {
                    receipt.statuses.push(record.body.clone());
                    let _ = save_receipt(&receipt);
                    client.close();
                    return output.fail(
                        "x402",
                        &format!(
                            "the provider refused the request: {}",
                            status.cause.unwrap_or_default()
                        ),
                    );
                }
            }
            _ => {}
        }
    }
    let (challenge, challenge_signed) = challenge.expect("challenge found");
    receipt.challenge = Some(challenge_signed.clone());
    let _ = save_receipt(&receipt);
    let terms = match buyer::check_challenge(
        &challenge,
        &request_signed,
        unix_now(),
        nostr::x402::DEFAULT_CLOCK_SKEW,
    ) {
        Ok(terms) => terms,
        Err(cause) => {
            client.close();
            return output.fail("x402", &format!("challenge: {cause}"));
        }
    };
    let advertised = descriptor.receivers.iter().any(|receiver| {
        receiver.network == terms.requirements.network
            && receiver.pay_to == terms.requirements.pay_to
    });
    if !advertised {
        client.close();
        return output.fail(
            "x402",
            "the challenge's payTo is not a receiver the capability advertises",
        );
    }
    let limits = match limits(
        policy.as_ref(),
        flags,
        Some(&terms.requirements.pay_to),
        Some(&capability_id),
    )
    .and_then(|limits| {
        admit(
            policy.as_ref(),
            limits,
            &terms.requirements.pay_to,
            terms.amount_msat,
        )
        .map(|()| limits)
    }) {
        Ok(limits) => limits,
        Err(message) => {
            client.close();
            return output.fail("x402", &format!("{message}; nothing was paid"));
        }
    };
    output.line(
        &json!({"event": "challenged", "purchase": purchase, "amount_msat": terms.amount_msat, "pay_to": terms.requirements.pay_to, "expires_at": terms.expires_at}),
        |value| format!("challenge: {} msat to {}", value["amount_msat"], value["pay_to"]),
    );

    // Pay exactly once; a pending payment is left for `wallet lookup`.
    let proof = match pay_invoice(
        &terms.invoice,
        &terms.requirements.network,
        limits.max_fee_msat,
        wait.min(90),
    ) {
        Ok(proof) => proof,
        Err(message) => {
            client.close();
            return output.fail("x402", &message);
        }
    };
    record_payment(
        &Spend {
            flags,
            capability: Some(capability_id.clone()),
            wait,
            binding: PROFILE,
            resource: format!("{provider} {slug} {purchase}"),
            phone: false,
        },
        &terms.requirements.network,
        &terms.requirements.pay_to,
        &proof,
        "paid",
    );
    receipt.paid = Some(proof.payment_hash.clone());
    let _ = save_receipt(&receipt);
    let mut payload = Map::new();
    payload.insert("preimage".into(), Value::String(proof.preimage.clone()));
    let payment = PaymentPayload {
        x402_version: 2,
        resource: None,
        accepted: terms.requirements.clone(),
        payload,
        extensions: None,
    };
    let claim = match buyer::claim(
        &challenge,
        &request_signed,
        &challenge_signed,
        &payment,
        unix_now(),
    ) {
        Ok(claim) => claim,
        Err(cause) => {
            client.close();
            return output.fail("x402", &format!("claim: {cause}"));
        }
    };
    match party
        .seal(&provider, &purchase, RECORD_SCHEMA, &claim, unix_now())
        .and_then(|event| client.publish(event.clone(), ACK).map(|ack| (event, ack)))
    {
        Ok((event, ack)) if ack.accepted => {
            receipt.claim = Signed::new(&claim)
                .ok()
                .map(|signed| signed.with_event(&event.id, &event.pubkey));
            let _ = save_receipt(&receipt);
        }
        Ok((_, ack)) => {
            client.close();
            return output.fail(
                "x402",
                &format!(
                    "paid {} but the relay refused the claim: {}; retry with `openagents x402 status {provider} {purchase}`",
                    proof.payment_hash, ack.message
                ),
            );
        }
        Err(message) => {
            client.close();
            return output.fail(
                "x402",
                &format!("paid {} but: {message}", proof.payment_hash),
            );
        }
    }
    output.line(
        &json!({"event": "claimed", "purchase": purchase, "payment_hash": proof.payment_hash, "amount_msat": terms.amount_msat, "fee_msat": proof.fee_msat}),
        |value| format!("paid {} msat (fee {}), claim sealed", value["amount_msat"], value["fee_msat"]),
    );

    let (newest, bytes) = follow(
        &mut client,
        &party,
        &mut receipt,
        Duration::from_secs(wait),
        output,
    );
    client.close();
    finish(
        output,
        &receipt,
        newest,
        bytes,
        args.switch("show-proof").then_some(proof.preimage.as_str()),
    )
}

fn finish(
    output: &Output,
    receipt: &Receipt,
    newest: Option<Value>,
    bytes: Option<Vec<u8>>,
    preimage: Option<&str>,
) -> u8 {
    let status = newest.as_ref().and_then(|body| parse_status(body).ok());
    let phase = status
        .as_ref()
        .map_or("unknown", |status| status.phase.name());
    if let Some(hash) = &receipt.paid
        && phase != "unknown"
    {
        set_phase(hash, phase);
    }
    let text = bytes
        .as_deref()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned());
    let mut doc = json!({
        "binding": PROFILE,
        "provider": receipt.provider,
        "purchase": receipt.purchase,
        "phase": phase,
        "seq": status.as_ref().map(|status| status.seq),
        "cause": status.as_ref().and_then(|status| status.cause.clone()),
        "settlement": status.as_ref().and_then(|status| serde_json::to_value(&status.settlement).ok()),
        "payment_hash": receipt.paid,
        "output": text,
    });
    if let Some(preimage) = preimage {
        doc["preimage"] = Value::String(preimage.to_owned());
    }
    let code = match phase {
        "completed" => 0,
        _ => 1,
    };
    if output.json() {
        output.emit(&doc, |_| String::new());
    } else if let Some(text) = &text {
        print!("{text}");
        if !text.ends_with('\n') {
            println!();
        }
    } else {
        output.emit(&doc, |value| {
            format!(
                "purchase {} is {}{}",
                value["purchase"],
                value["phase"].as_str().unwrap_or_default(),
                value["cause"]
                    .as_str()
                    .map(|cause| format!(": {cause}"))
                    .unwrap_or_default()
            )
        });
    }
    if code != 0 && phase == "unknown" {
        let recover_until = receipt
            .statuses
            .iter()
            .filter_map(|body| parse_status(body).ok())
            .map(|status| status.recover_until)
            .max();
        match recover_until {
            Some(until) if unix_now() >= until => eprintln!(
                "openagents x402: the provider answered nothing before its recovery window closed at {until}; the purchase is lost to it, and nothing is paid twice"
            ),
            _ => eprintln!(
                "openagents x402: the provider has not answered; ask again with `openagents x402 status {} {}` (nothing is paid twice)",
                receipt.provider, receipt.purchase
            ),
        }
    }
    code
}

// ------------------------------------------------------------------ status

/// A receiver for provider commands that never mint an invoice: listing and
/// finishing purchases touch only the purchase ledger, not the wallet.
struct NoReceiver;

impl Receiver for NoReceiver {
    fn pay_to(&self) -> String {
        String::new()
    }

    fn invoice(&self, _: u64, _: [u8; 32], _: u32) -> Result<String, String> {
        Err("this command does not mint invoices".to_owned())
    }
}

/// `status --list` lists the provider's open purchases; `status --finish
/// BUYER:PURCHASE --cause CAUSE` ends one by hand and publishes the status.
fn provider_status(output: &Output, args: &Args) -> u8 {
    let party = match Party::load(args.option("as")) {
        Ok(party) => party,
        Err(message) => return output.fail("x402", &message),
    };
    let facilitator = match FileReplayStore::open(&replay_dir()) {
        Ok(store) => {
            Facilitator::with_profiles(store, nostr::x402::DEFAULT_CLOCK_SKEW, NATIVE_ONLY)
        }
        Err(error) => return output.fail("x402", &format!("replay store: {error}")),
    };
    let store = match PurchaseStore::open(&purchases_dir()) {
        Ok(store) => store,
        Err(error) => return output.fail("x402", &error.to_string()),
    };
    let receiver = NoReceiver;
    let provider = Provider {
        pubkey: party.pubkey.clone(),
        offer: Offer {
            capability_id: String::new(),
            operation: String::new(),
            network: String::new(),
            amount_msat: 0,
            timeout_secs: 0,
            description: String::new(),
            per_buyer_hourly: None,
        },
        receiver: &receiver,
        facilitator: &facilitator,
        store: &store,
        skew: nostr::x402::DEFAULT_CLOCK_SKEW,
    };
    let Some(target) = args.option("finish") else {
        let open = match provider.open_purchases() {
            Ok(open) => open,
            Err(cause) => return output.fail("x402", cause),
        };
        let now = unix_now();
        let entries: Vec<Value> = open
            .iter()
            .map(|purchase| {
                json!({
                    "buyer": purchase.buyer,
                    "purchase": purchase.purchase,
                    "phase": purchase.phase().map(|phase| phase.name()).unwrap_or("unknown"),
                    "statuses": purchase.statuses.len(),
                    "execute_until": purchase.execute_until,
                    "recover_until": purchase.recover_until,
                    "execute_window_open": now < purchase.execute_until,
                    "job": purchase.job(),
                })
            })
            .collect();
        output.emit(
            &json!({
                "provider": party.pubkey,
                "purchases": purchases_dir(),
                "count": entries.len(),
                "open": entries,
            }),
            |value| {
                let mut lines = vec![format!("{} open purchase(s)", value["count"])];
                for entry in value["open"].as_array().into_iter().flatten() {
                    lines.push(format!(
                        "  {} {} from {} ({}, execute_until {})",
                        entry["phase"].as_str().unwrap_or_default(),
                        &entry["purchase"].as_str().unwrap_or_default()[..16],
                        &entry["buyer"].as_str().unwrap_or_default()[..16],
                        if entry["execute_window_open"].as_bool() == Some(true) {
                            "window open"
                        } else {
                            "window passed"
                        },
                        entry["execute_until"]
                    ));
                }
                lines.join("\n")
            },
        );
        return 0;
    };
    let Some((buyer, purchase)) = target.split_once(':') else {
        return usage(output, "--finish takes BUYER:PURCHASE (both hex)");
    };
    let Some(cause) = args.option("cause").and_then(operator_cause) else {
        return usage(
            output,
            &format!(
                "--finish needs --cause, one of: {}",
                OPERATOR_CAUSES.join(", ")
            ),
        );
    };
    let emit = match provider.finish(buyer, purchase, Err(cause), unix_now()) {
        Ok(emit) => emit,
        Err(cause) => return output.fail("x402", cause),
    };
    let relay = relay_url(args.option("relay"));
    let mut client = Client::connect(&relay, party.signer.clone());
    let published = publish_records(&mut client, &party, buyer, purchase, &emit);
    client.close();
    output.emit(
        &json!({
            "buyer": buyer,
            "purchase": purchase,
            "phase": "failed",
            "cause": cause,
            "records": published.as_ref().ok(),
            "relay_error": published.as_ref().err(),
        }),
        |value| {
            format!(
                "purchase {} finished as failed: {}{}",
                &value["purchase"].as_str().unwrap_or_default()[..16],
                value["cause"].as_str().unwrap_or_default(),
                value["relay_error"]
                    .as_str()
                    .map(|error| format!(" (status not published: {error})"))
                    .unwrap_or_default()
            )
        },
    );
    if published.is_err() { 1 } else { 0 }
}

pub fn status(output: &Output, words: &[String]) -> u8 {
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return usage(output, &message),
    };
    if args.switch("list") || args.option("finish").is_some() {
        return provider_status(output, &args);
    }
    let [provider, purchase] = args.positional() else {
        return usage(
            output,
            "status needs PROVIDER PURCHASE (buyer), or --list / --finish BUYER:PURCHASE --cause CAUSE (provider)",
        );
    };
    let party = match Party::load(args.option("as")) {
        Ok(party) => party,
        Err(message) => return output.fail("x402", &message),
    };
    let mut receipt = match load_receipt(provider, purchase) {
        Ok(receipt) => receipt,
        Err(message) => return output.fail("x402", &message),
    };
    let request_record = match receipt.request.value().and_then(|value| {
        parse_record(
            &receipt.request.bytes,
            value["issuer"].as_str().unwrap_or(""),
        )
    }) {
        Ok(record) => record,
        Err(cause) => return output.fail("x402", &format!("stored request: {cause}")),
    };
    // Without --wait, a purchase still running is followed to the end of
    // its execution window rather than reported unfinished.
    let default_wait = parse_request(&request_record.body, request_record.issued_at)
        .map(|request| request.execute_until.saturating_sub(unix_now()) + WINDOW_MARGIN)
        .unwrap_or(30)
        .max(30);
    let wait = match args.number::<u64>("wait", default_wait) {
        Ok(n) if n > 0 => n,
        _ => return usage(output, "--wait takes seconds above zero"),
    };
    let relay = relay_url(args.option("relay"));
    let mut client = Client::connect(&relay, party.signer.clone());
    let now = unix_now();
    if let Err(message) = client.listen(vec![inbox_filter(&party.pubkey, now.saturating_sub(5))]) {
        client.close();
        return output.fail("x402", &message);
    }
    let query = buyer::status_query(&receipt.request, &request_record, now);
    if let Err(message) = publish_records(&mut client, &party, provider, purchase, &query) {
        client.close();
        return output.fail("x402", &message);
    }
    let (newest, bytes) = follow(
        &mut client,
        &party,
        &mut receipt,
        Duration::from_secs(wait),
        output,
    );
    client.close();
    finish(output, &receipt, newest, bytes, None)
}
