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

use nostr::contracts::{self, ARTIFACT_ENVELOPE_KIND, ARTIFACT_MARKER, digest_bytes, jcs};
use nostr::domain::Event;
use nostr::private_artifact;
use openagents_x402::facilitator::Facilitator;
use openagents_x402::native::{
    self, BYTES_SCHEMA, Emit, NATIVE_ONLY, Offer, PROFILE, Provider, PurchaseStore, RECORD_SCHEMA,
    Record, RecordType, Signed, artifact_bytes, buyer, bytes_artifact, parse_record, parse_status,
};
use openagents_x402::server::Receiver;
use openagents_x402::{FileReplayStore, PaymentPayload, network_id};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Map, Value, json};

use crate::relay::{Client, identity_for, relay_url, unix_now};
use crate::x402::{Budget, Node, fail_wallet, open_wallet, pay_invoice, replay_dir};
use crate::{Args, Output};

const SWITCHES: &[&str] = &["show-proof"];
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

// ---------------------------------------------------------------- provider

pub fn serve(output: &Output, words: &[String]) -> u8 {
    let Some(split) = words.iter().position(|word| word == "--") else {
        return usage(output, "native-serve needs `-- CMD [ARGS...]`");
    };
    let (flags, command) = words.split_at(split);
    let command = &command[1..];
    let Some((program, program_args)) = command.split_first() else {
        return usage(output, "native-serve needs a command after `--`");
    };
    let args = match Args::parse(flags, SWITCHES) {
        Ok(args) => args,
        Err(message) => return usage(output, &message),
    };
    let Some(slug) = args.option("slug") else {
        return usage(output, "native-serve needs --slug SLUG");
    };
    let amount_msat = match args.number::<u64>("msat", 0) {
        Ok(0) | Err(_) => return usage(output, "native-serve needs --msat N (N > 0)"),
        Ok(n) => n,
    };
    let timeout_secs = match args.number::<u32>("timeout", 300) {
        Ok(n) if n > 0 => n,
        _ => return usage(output, "--timeout takes seconds above zero"),
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
        },
        receiver: &receiver,
        facilitator: &facilitator,
        store: &store,
        skew: nostr::x402::DEFAULT_CLOCK_SKEW,
    };
    let relay = relay_url(args.option("relay"));
    let mut client = Client::connect(&relay, party.signer.clone());
    let started = unix_now();
    if let Err(message) = client.listen(vec![inbox_filter(
        &party.pubkey,
        started.saturating_sub(CATCH_UP),
    )]) {
        return output.fail("x402", &message);
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
    let mut served = 0u64;
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
                        Some(input) => match provider.start(&buyer, &purchase, unix_now()) {
                            Ok(running) => {
                                if let Err(message) = publish_records(
                                    &mut client,
                                    &party,
                                    &buyer,
                                    &purchase,
                                    &running,
                                ) {
                                    log("relay_failed", json!({"cause": message}));
                                }
                                run_command(program, program_args, &input)
                            }
                            Err(cause) => Err(cause),
                        },
                    };
                    let finished = match result {
                        Ok(stdout) => {
                            let inline = bytes_artifact(&stdout);
                            let bytes = jcs(&inline).unwrap_or_default();
                            match party
                                .seal(&buyer, &purchase, BYTES_SCHEMA, &inline, unix_now())
                                .and_then(|event| {
                                    client.publish(event.clone(), ACK).map(|ack| (event, ack))
                                }) {
                                Ok((event, ack)) if ack.accepted => {
                                    Ok(reference(&event, &bytes, BYTES_SCHEMA))
                                }
                                Ok(_) | Err(_) => Err("output could not be published"),
                            }
                        }
                        Err(cause) => Err(cause),
                    };
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
    let max_msat = match args.number::<u64>("max-msat", 0) {
        Ok(0) | Err(_) => return usage(output, "buy needs --max-msat N (N > 0)"),
        Ok(n) => n,
    };
    let max_fee = match args.number::<u64>("max-fee-msat", max_msat / 100 + 1_000) {
        Ok(n) => n,
        Err(message) => return usage(output, &message),
    };
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
    if terms.amount_msat > max_msat {
        client.close();
        return output.fail(
            "x402",
            &format!(
                "the run costs {} msat, above --max-msat {max_msat}; nothing was paid",
                terms.amount_msat
            ),
        );
    }
    output.line(
        &json!({"event": "challenged", "purchase": purchase, "amount_msat": terms.amount_msat, "pay_to": terms.requirements.pay_to, "expires_at": terms.expires_at}),
        |value| format!("challenge: {} msat to {}", value["amount_msat"], value["pay_to"]),
    );

    // Pay exactly once; a pending payment is left for `wallet lookup`.
    let budget = Budget {
        max_msat,
        max_fee,
        wait: wait.min(90),
    };
    let proof = match pay_invoice(&terms.invoice, &terms.requirements.network, &budget) {
        Ok(proof) => proof,
        Err(message) => {
            client.close();
            return output.fail("x402", &message);
        }
    };
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
        eprintln!(
            "openagents x402: no terminal status yet; ask again with `openagents x402 status {} {}` (nothing is paid twice)",
            receipt.provider, receipt.purchase
        );
    }
    code
}

// ------------------------------------------------------------------ status

pub fn status(output: &Output, words: &[String]) -> u8 {
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return usage(output, &message),
    };
    let [provider, purchase] = args.positional() else {
        return usage(output, "status needs PROVIDER PURCHASE");
    };
    let wait = match args.number::<u64>("wait", 30) {
        Ok(n) if n > 0 => n,
        _ => return usage(output, "--wait takes seconds above zero"),
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
