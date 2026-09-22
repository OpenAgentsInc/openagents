//! The NIP-CJ decision worker: a relay front for the one admission path
//! this crate serves.
//!
//! A decision job is one `POST /v1/systemone` call carried as Nostr
//! events instead of an HTTP request (`nips/openagents/NIP-CJ.md`,
//! "Decision jobs"; `docs/decision-models/api/relay-decision-contract.md`).
//! The worker subscribes kind `25910` addressed to its key, admits each
//! request through `nostr::decision` — signature, addressing, freshness,
//! payload — resolves the verified signer to an operator-provisioned
//! principal binding, and forwards the envelope to the configured
//! upstream, which is this same serving path over HTTP. Tenant
//! authorization, artifact binding, quota reservation, and settlement
//! therefore run once, in the code that already owns them; the relay
//! lane adds durable worker state only where the contract puts durable
//! ownership on it: a `jobs.jsonl` ledger that republishes a settled
//! pair's recorded result and refuses a changed body as
//! `idempotency_conflict`, plus a live table cancellation resolves
//! against.
//!
//! The worker bounds its own side independently of the upstream: a
//! concurrency cap, the request-freshness window, the caller's deadline,
//! and cancellation. A signer that maps to no principal forwards with no
//! bearer — the anonymous shared-door call, exactly as a keyless HTTP
//! request resolves — unless `anonymous` is configured off.
//!
//! What this module is not, because the contract is careful about it:
//! the relay is transport, and this worker holds no authority a caller
//! can reach through it. It authenticates the connection by NIP-42,
//! verifies every event's own signature before reading a byte of
//! payload, and maps the signer to a tenant through provisioning — never
//! through a claim inside the job.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use nostr::decision::{
    self, Admitted, AdmittedCall, AdmittedCancel, Outcome, Refusal, RequestBody, RequestWindow,
    Resolution, Seal, Status,
};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::nip44;
use receipts::execution::{ExecutionReceipt, Outcome as ReceiptOutcome, Served, Timing};
use secp256k1::{Secp256k1, SecretKey, XOnlyPublicKey};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{Semaphore, mpsc};
use tokio_tungstenite::tungstenite;

use crate::serve::{now_utc, unix_now};

/// The schema tag the worker's durable job ledger writes.
const JOBS_SCHEMA: &str = "openagents.decision-worker.jobs.v1";
/// The ledger file under `jobs_dir`.
const JOBS_FILE: &str = "jobs.jsonl";
/// Event ids the worker remembers for deduplication.
const SEEN_LIMIT: usize = 8_192;
/// The `retry_after_ms` a congestion refusal quotes when the upstream
/// said nothing.
const BUSY_RETRY_MS: u64 = 250;
/// Seconds between relay reconnect attempts.
const RECONNECT_SECS: u64 = 2;
/// The socket write a stalled relay gets before the session drops.
const WRITE_TIMEOUT: Duration = Duration::from_secs(15);

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The worker's configuration: one `decision-worker.json`.
#[derive(Debug, Deserialize)]
pub struct WorkerConfig {
    /// The relay URL the worker connects to.
    pub relay: String,
    /// The worker's secret key, 64 lowercase hex. Absent means the
    /// `DECISION_WORKER_SECRET` environment variable supplies it.
    #[serde(default)]
    pub worker_secret: Option<String>,
    /// The base URL of the serving path this worker fronts — a gateway
    /// that answers `POST /v1/systemone`.
    pub upstream: String,
    /// Signer pubkey (hex) to the credential its jobs forward under. A
    /// signer missing from this map reaches the anonymous lane, or is
    /// refused when `anonymous` is false.
    #[serde(default)]
    pub principals: BTreeMap<String, Principal>,
    /// Whether an unmapped signer forwards with no bearer — the shared
    /// doors an anonymous HTTP call reaches. Default true, matching the
    /// HTTP lane.
    #[serde(default = "default_anonymous")]
    pub anonymous: bool,
    /// How many jobs run at once; a request past the bound is refused
    /// `busy` before anything is held.
    #[serde(default = "default_jobs")]
    pub jobs: usize,
    /// The upstream call's ceiling when the caller set no deadline.
    #[serde(default = "default_upstream_timeout_secs")]
    pub upstream_timeout_secs: u64,
    /// The durable job ledger's directory.
    pub jobs_dir: PathBuf,
    /// The `created_at` window a request must fall inside; absent means
    /// [`RequestWindow::DEFAULT`].
    #[serde(default)]
    pub request_window: Option<WindowConfig>,
}

fn default_anonymous() -> bool {
    true
}

fn default_jobs() -> usize {
    4
}

fn default_upstream_timeout_secs() -> u64 {
    120
}

/// One operator-provisioned signer binding: the credential the
/// principal's jobs forward under.
#[derive(Clone, Debug, Deserialize)]
pub struct Principal {
    /// The `oak_<id>.<secret>` bearer key. It stays in the provisioning
    /// file and the `Authorization` header — never a log line.
    pub key: String,
    /// The tenant reference receipts and the ledger name — `key-ref:<id>`
    /// derived from the key when unset. Never the credential.
    #[serde(default)]
    pub tenant: Option<String>,
    /// The `X-Workspace-Id` a membership-gated upstream requires.
    #[serde(default)]
    pub workspace: Option<String>,
}

/// The `created_at` window override.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct WindowConfig {
    /// How old a request may be, seconds.
    pub max_age_seconds: u64,
    /// How far ahead of the worker's clock a request may be, seconds.
    pub max_future_seconds: u64,
}

/// Why the worker refuses before anything runs.
#[derive(Debug)]
pub enum Trouble {
    /// The configuration cannot load or cannot be satisfied.
    Config(String),
    /// The durable ledger cannot be opened or written.
    Ledger(String),
    /// The relay session ended; the supervisor reconnects.
    Relay(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(error) => write!(f, "config: {error}"),
            Self::Ledger(error) => write!(f, "jobs ledger: {error}"),
            Self::Relay(error) => write!(f, "relay: {error}"),
        }
    }
}

impl std::error::Error for Trouble {}

/// The credential a resolved signer forwards under — `key: None` is
/// the anonymous shared-door lane, which the HTTP path already defines.
#[derive(Clone)]
struct Binding {
    key: Option<String>,
    tenant: Option<String>,
    workspace: Option<String>,
}

/// One settled attempt's record — the pieces a republished result
/// rebuilds its receipt from, so a retransmission gets the same
/// terminal answer sealed to its own transport identity.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Settled {
    /// The receipt-level outcome word.
    outcome: String,
    /// The refusal or failure cause, when the outcome carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cause: Option<String>,
    /// The refusal code the `error` object reports, when ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    /// The refusal message, when ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    /// The congestion hint, when ended with one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retry_after_ms: Option<u64>,
    /// The verbatim systemone response, when answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<Value>,
    /// The model that answered, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    served_model: Option<String>,
    /// The upstream receipt reference — `x-receipt` on the forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    usage: Option<String>,
    /// The upstream call's milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latency_ms: Option<u64>,
    /// When the attempt resolved, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resolved_at: Option<String>,
    /// Digest of the response body, when one came back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result_digest: Option<String>,
}

/// A ledger line: one admission or settlement for a `(principal,
/// tenant, request, attempt)` key.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct LedgerEntry {
    /// The ledger's own schema tag.
    v: String,
    /// `decision::idempotency_key` of the pair.
    key: String,
    /// `decision::request_scope` of the request.
    scope: String,
    /// The envelope identity a republished result reuses.
    request: String,
    /// The attempt, one-based.
    attempt: u32,
    /// The door the call named.
    model: String,
    /// The tenant reference receipts name — never a credential.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tenant: Option<String>,
    /// `RequestBody::digest()` of the envelope the pair admitted.
    request_digest: String,
    /// `admitted` while the pair is in flight, `settled` at its end.
    state: String,
    /// The terminal record, present once `state` is `settled`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    settled: Option<Settled>,
}

/// The recorded state of one pair: the envelope identity plus the
/// terminal record when it exists.
#[derive(Clone)]
struct JobRecord {
    request: String,
    attempt: u32,
    model: String,
    tenant: Option<String>,
    request_digest: String,
    settled: Option<Settled>,
}

/// The durable job ledger: `jobs.jsonl` under `jobs_dir`, folded into
/// memory at boot. An `admitted` record with no `settled` line is the
/// crash-recovery mark — a redelivery of the pair redrives the upstream
/// call under the same idempotency identity, which joins the existing
/// reservation rather than spending twice.
struct Jobs {
    inner: Mutex<JobsInner>,
}

struct JobsInner {
    file: std::fs::File,
    map: HashMap<String, JobRecord>,
    /// Request scope → the live pair's key, for cancellation.
    inflight: HashMap<String, String>,
}

impl Jobs {
    /// Open or create `jobs.jsonl` under `dir` and fold it into memory.
    fn open(dir: &Path) -> Result<Self, Trouble> {
        std::fs::create_dir_all(dir).map_err(|error| {
            Trouble::Ledger(format!("cannot create {}: {error}", dir.display()))
        })?;
        let path = dir.join(JOBS_FILE);
        let mut map = HashMap::new();
        let mut inflight = HashMap::new();
        if let Ok(contents) = std::fs::read_to_string(&path) {
            for (index, line) in contents.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let entry: LedgerEntry = serde_json::from_str(line).map_err(|error| {
                    Trouble::Ledger(format!(
                        "{} line {} does not parse: {error}",
                        path.display(),
                        index + 1
                    ))
                })?;
                if entry.v != JOBS_SCHEMA {
                    return Err(Trouble::Ledger(format!(
                        "{} line {} carries schema {}, not {JOBS_SCHEMA}",
                        path.display(),
                        index + 1,
                        entry.v
                    )));
                }
                let record = JobRecord {
                    request: entry.request,
                    attempt: entry.attempt,
                    model: entry.model,
                    tenant: entry.tenant,
                    request_digest: entry.request_digest,
                    settled: entry.settled,
                };
                match entry.state.as_str() {
                    "admitted" => {
                        inflight.insert(entry.scope, entry.key.clone());
                    }
                    "settled" => {
                        inflight.remove(&entry.scope);
                    }
                    state => {
                        return Err(Trouble::Ledger(format!(
                            "{} line {} carries unknown state {state}",
                            path.display(),
                            index + 1
                        )));
                    }
                }
                map.insert(entry.key, record);
            }
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| Trouble::Ledger(format!("cannot open {}: {error}", path.display())))?;
        Ok(Self {
            inner: Mutex::new(JobsInner {
                file,
                map,
                inflight,
            }),
        })
    }

    /// The recorded state of one `(principal, tenant, request, attempt)`
    /// key, when any delivery admitted it before.
    fn lookup(&self, key: &str) -> Option<JobRecord> {
        self.inner.lock().expect("jobs").map.get(key).cloned()
    }

    /// Persist the admission before dispatch — a crash after this line
    /// leaves the recovery mark a redelivery reconciles.
    fn admit(&self, key: &str, scope: &str, record: &JobRecord) -> Result<(), Trouble> {
        let mut inner = self.inner.lock().expect("jobs");
        if inner.map.contains_key(key) {
            return Ok(());
        }
        Self::append(
            &mut inner.file,
            &LedgerEntry {
                v: JOBS_SCHEMA.to_string(),
                key: key.to_string(),
                scope: scope.to_string(),
                request: record.request.clone(),
                attempt: record.attempt,
                model: record.model.clone(),
                tenant: record.tenant.clone(),
                request_digest: record.request_digest.clone(),
                state: "admitted".to_string(),
                settled: None,
            },
        )?;
        inner.map.insert(key.to_string(), record.clone());
        inner.inflight.insert(scope.to_string(), key.to_string());
        Ok(())
    }

    /// Persist the terminal record. Returns false when the pair already
    /// settled — a racing cancel won — so the caller publishes nothing.
    fn settle(&self, key: &str, scope: &str, settled: Settled) -> Result<bool, Trouble> {
        let mut inner = self.inner.lock().expect("jobs");
        if inner
            .map
            .get(key)
            .is_some_and(|record| record.settled.is_some())
        {
            return Ok(false);
        }
        let Some(record) = inner.map.get(key).cloned() else {
            return Err(Trouble::Ledger(format!(
                "settling a pair that was never admitted: {key}"
            )));
        };
        Self::append(
            &mut inner.file,
            &LedgerEntry {
                v: JOBS_SCHEMA.to_string(),
                key: key.to_string(),
                scope: scope.to_string(),
                request: record.request.clone(),
                attempt: record.attempt,
                model: record.model.clone(),
                tenant: record.tenant.clone(),
                request_digest: record.request_digest.clone(),
                state: "settled".to_string(),
                settled: Some(settled.clone()),
            },
        )?;
        inner
            .map
            .get_mut(key)
            .expect("the record was present")
            .settled = Some(settled);
        inner.inflight.remove(scope);
        Ok(true)
    }

    /// The idempotency key a request scope currently owns — the pair a
    /// cancel resolves, inside its principal's scope only.
    fn inflight_key(&self, scope: &str) -> Option<String> {
        self.inner
            .lock()
            .expect("jobs")
            .inflight
            .get(scope)
            .cloned()
    }

    fn append(file: &mut std::fs::File, entry: &LedgerEntry) -> Result<(), Trouble> {
        let mut line = serde_json::to_string(entry)
            .map_err(|error| Trouble::Ledger(format!("an entry does not serialize: {error}")))?;
        line.push('\n');
        file.write_all(line.as_bytes())
            .and_then(|()| file.sync_data())
            .map_err(|error| Trouble::Ledger(format!("the ledger does not append: {error}")))
    }
}

/// One transport delivery of a pair the worker owes an answer to: the
/// request event's `id`, which the reply `e`-tags, and the caller's
/// pubkey, which it `p`-tags.
#[derive(Clone)]
struct Delivery {
    attempt_id: String,
    principal: String,
}

/// A pair in flight: its deliveries, whether the upstream call has been
/// dispatched, and the abort handle a cancel uses after dispatch.
struct Running {
    deliveries: Vec<Delivery>,
    dispatched: bool,
    abort: Option<tokio::task::AbortHandle>,
}

/// The live worker: config, keys, the durable ledger, the dedup set,
/// the in-flight table, the job semaphore, and the publish channel.
///
/// The channel persists across sessions — events owed while the relay
/// connection is down queue for the next one, because a settlement owed
/// is a settlement owed, not a fact the socket's mood can erase.
pub struct Worker {
    config: WorkerConfig,
    signer: RelaySigner,
    secret: SecretKey,
    pubkey: String,
    http: reqwest::Client,
    jobs: Jobs,
    seen: Mutex<(HashSet<String>, VecDeque<String>)>,
    running: Mutex<HashMap<String, Running>>,
    slots: Semaphore,
    outbox: mpsc::UnboundedSender<String>,
    inbox: tokio::sync::Mutex<mpsc::UnboundedReceiver<String>>,
    window: RequestWindow,
    published: AtomicU64,
}

impl Worker {
    /// Build the worker from its configuration. `worker_secret` falls
    /// back to `DECISION_WORKER_SECRET`.
    pub fn open(config: WorkerConfig) -> Result<Arc<Self>, Trouble> {
        let secret_hex = config
            .worker_secret
            .clone()
            .or_else(|| std::env::var("DECISION_WORKER_SECRET").ok())
            .ok_or_else(|| {
                Trouble::Config(
                    "no worker secret: set `worker_secret` or DECISION_WORKER_SECRET".into(),
                )
            })?;
        let secret_bytes = hex_decode(&secret_hex)?;
        let secret_array: [u8; 32] = secret_bytes
            .as_slice()
            .try_into()
            .map_err(|_| Trouble::Config("the worker secret is not 32 bytes".into()))?;
        let secret = SecretKey::from_byte_array(secret_array)
            .map_err(|error| Trouble::Config(format!("the worker secret is not a key: {error}")))?;
        let signer = RelaySigner::from_secret_hex(&secret_hex).map_err(|error| {
            Trouble::Config(format!("the worker secret does not parse: {error}"))
        })?;
        let pubkey = secret.x_only_public_key(&Secp256k1::new()).0.to_string();
        for key in config.principals.keys() {
            let bytes: [u8; 32] = hex_decode(key)
                .map_err(|_| Trouble::Config(format!("principal {key} is not hex")))?
                .as_slice()
                .try_into()
                .map_err(|_| Trouble::Config(format!("principal {key} is not 32 bytes")))?;
            XOnlyPublicKey::from_byte_array(bytes)
                .map_err(|_| Trouble::Config(format!("principal {key} is not a public key")))?;
        }
        let jobs = Jobs::open(&config.jobs_dir)?;
        let (outbox, inbox) = mpsc::unbounded_channel();
        let window = config
            .request_window
            .map(|window| RequestWindow::new(window.max_age_seconds, window.max_future_seconds))
            .unwrap_or(RequestWindow::DEFAULT);
        Ok(Arc::new(Self {
            slots: Semaphore::new(config.jobs),
            window,
            jobs,
            config,
            signer,
            secret,
            pubkey,
            http: reqwest::Client::new(),
            seen: Mutex::new((HashSet::new(), VecDeque::new())),
            running: Mutex::new(HashMap::new()),
            outbox,
            inbox: tokio::sync::Mutex::new(inbox),
            published: AtomicU64::new(0),
        }))
    }

    /// The worker's public key, hex — the value callers `p`-tag and the
    /// subscription filters on.
    pub fn pubkey(&self) -> &str {
        &self.pubkey
    }

    /// Events the worker has owed the relay since boot.
    pub fn published(&self) -> u64 {
        self.published.load(Ordering::Relaxed)
    }

    /// Serve one relay session: authenticate, subscribe, then read
    /// events and write owed answers until the socket ends.
    pub async fn serve(self: &Arc<Self>, mut socket: Socket) -> Result<(), Trouble> {
        let challenge = read_json(&mut socket).await?;
        if challenge[0].as_str() != Some("AUTH") {
            return Err(Trouble::Relay(format!(
                "expected an AUTH challenge, got {challenge}"
            )));
        }
        let auth = self.signer.sign(
            unix_now(),
            22_242,
            vec![
                Tag::new(vec!["relay".into(), self.config.relay.clone()]),
                Tag::new(vec![
                    "challenge".into(),
                    challenge[1].as_str().unwrap_or_default().into(),
                ]),
            ],
            String::new(),
        );
        send_json(&mut socket, &json!(["AUTH", auth])).await?;
        let ok = read_json(&mut socket).await?;
        if ok[1].as_str() != Some(auth.id.as_str()) || ok[2] != true {
            return Err(Trouble::Relay(format!("the relay refused AUTH: {ok}")));
        }
        send_json(
            &mut socket,
            &json!(["REQ", "jobs", {"kinds": [decision::REQUEST_KIND], "#p": [self.pubkey]}]),
        )
        .await?;

        // The session owns the receiver for its duration; owed events
        // published while disconnected wait in the channel for the
        // next session to drain.
        let mut inbox = self.inbox.lock().await;
        loop {
            tokio::select! {
                outbound = inbox.recv() => {
                    match outbound {
                        Some(text) => send_text(&mut socket, &text).await?,
                        None => return Err(Trouble::Relay("the outbox closed".into())),
                    }
                }
                inbound = socket.next() => {
                    let message = match inbound {
                        Some(Ok(message)) => message,
                        Some(Err(error)) => {
                            return Err(Trouble::Relay(format!("socket: {error}")));
                        }
                        None => return Err(Trouble::Relay("the socket closed".into())),
                    };
                    self.on_message(message)?;
                }
            }
        }
    }

    fn on_message(self: &Arc<Self>, message: tungstenite::Message) -> Result<(), Trouble> {
        let tungstenite::Message::Text(text) = message else {
            return Ok(());
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return Ok(());
        };
        match value[0].as_str() {
            Some("EVENT") => {
                if let Ok(event) = serde_json::from_value::<Event>(value[2].clone()) {
                    self.on_event(event);
                }
            }
            Some("CLOSED") => {
                return Err(Trouble::Relay(format!(
                    "the relay closed the subscription: {value}"
                )));
            }
            _ => {}
        }
        Ok(())
    }

    /// Whether this event id is new; remembers it when so.
    fn fresh(&self, id: &str) -> bool {
        let (set, order) = &mut *self.seen.lock().expect("seen");
        if !set.insert(id.to_string()) {
            return false;
        }
        order.push_back(id.to_string());
        while order.len() > SEEN_LIMIT {
            if let Some(oldest) = order.pop_front() {
                set.remove(&oldest);
            }
        }
        true
    }

    fn on_event(self: &Arc<Self>, event: Event) {
        if !self.fresh(&event.id) {
            return;
        }
        match decision::admit(&event, &self.pubkey, &self.secret, unix_now(), self.window) {
            Ok(Admitted::Call(call)) => self.on_call(*call),
            Ok(Admitted::Cancel(cancel)) => self.on_cancel(cancel),
            Err(error) => {
                let Some(refusal) = Refusal::from_error(&error) else {
                    return;
                };
                // A refusal the caller can bind needs the decrypted
                // envelope's correlation; an undecryptable one is
                // unanswerable.
                let Ok(payload) = decision::decrypt_payload(&event, &self.secret) else {
                    return;
                };
                let Some((request, attempt)) = decision::payload_correlation(&payload) else {
                    return;
                };
                if let Ok(event) = decision::answer_event(
                    self.seal(&event.pubkey),
                    decision::FEEDBACK_KIND,
                    &event.id,
                    &event.pubkey,
                    &decision::refusal_payload(&request, attempt, &refusal),
                ) {
                    self.publish(&event);
                }
            }
        }
    }

    /// One admitted decision call: resolve the principal, dedupe the
    /// pair against the ledger, then answer, join the in-flight run,
    /// or start one.
    fn on_call(self: &Arc<Self>, call: AdmittedCall) {
        let delivery = Delivery {
            attempt_id: call.attempt_id.clone(),
            principal: call.principal.clone(),
        };
        let Some(binding) = self.binding(&call.principal) else {
            self.refuse(
                &delivery,
                &call.body.request,
                call.body.attempt,
                Refusal::new("not_admitted").message("this worker does not answer this signer"),
            );
            return;
        };
        let key = decision::idempotency_key(
            &call.principal,
            binding.tenant.as_deref(),
            &call.body.request,
            call.body.attempt,
        );
        let scope = decision::request_scope(
            &call.principal,
            binding.tenant.as_deref(),
            &call.body.request,
        );

        match self.jobs.lookup(&key) {
            Some(record) if record.request_digest != call.request_digest => {
                self.refuse(
                    &delivery,
                    &call.body.request,
                    call.body.attempt,
                    Refusal::new("idempotency_conflict")
                        .message("this (request, attempt) pair is taken by different content"),
                );
                return;
            }
            Some(JobRecord {
                settled: Some(_), ..
            }) => {
                // Settled: republish the recorded result sealed to this
                // delivery's transport identity. Execution stays once.
                self.publish_record(&delivery, &key);
                return;
            }
            Some(_) => {
                // Admitted, not yet settled: a second delivery of the
                // same pair joins the in-flight run and gets the result
                // bound to its own event id when it lands.
                let mut running = self.running.lock().expect("running");
                if let Some(entry) = running.get_mut(&key) {
                    entry.deliveries.push(delivery);
                    return;
                }
                drop(running);
                // Settled between the lookup and the join: republish.
                if matches!(self.jobs.lookup(&key), Some(ref record) if record.settled.is_some()) {
                    self.publish_record(&delivery, &key);
                    return;
                }
                // An admitted record with no runner is the crash-
                // recovery case: redrive under the same idempotency
                // identity — the upstream's settlement joins the
                // existing reservation rather than spending twice.
            }
            None => {}
        }

        let record = JobRecord {
            request: call.body.request.clone(),
            attempt: call.body.attempt,
            model: call.body.model.clone(),
            tenant: binding.tenant.clone(),
            request_digest: call.request_digest.clone(),
            settled: None,
        };
        if let Err(error) = self.jobs.admit(&key, &scope, &record) {
            self.refuse(
                &delivery,
                &call.body.request,
                call.body.attempt,
                Refusal::new("internal").message(error.to_string()),
            );
            return;
        }
        self.start_job(
            &key,
            scope,
            binding,
            call.body,
            call.request_digest,
            delivery,
        );
    }

    /// Registers the run and spawns it. The register-then-spawn order
    /// keeps `run_job`'s first `deliveries()` read nonempty.
    fn start_job(
        self: &Arc<Self>,
        key: &str,
        scope: String,
        binding: Binding,
        body: RequestBody,
        request_digest: String,
        delivery: Delivery,
    ) {
        {
            let mut running = self.running.lock().expect("running");
            running.insert(
                key.to_string(),
                Running {
                    deliveries: vec![delivery],
                    dispatched: false,
                    abort: None,
                },
            );
        }
        let worker = Arc::clone(self);
        let task_key = key.to_string();
        let task = tokio::spawn(async move {
            worker
                .run_job(task_key, scope, binding, body, request_digest)
                .await;
        });
        if let Some(entry) = self.running.lock().expect("running").get_mut(key) {
            entry.abort = Some(task.abort_handle());
        }
    }

    /// The job lifecycle after admission: progress statuses, a slot,
    /// the upstream call under the pair's idempotency identity, then
    /// settlement and one result per delivery that carried the pair.
    async fn run_job(
        self: &Arc<Self>,
        key: String,
        scope: String,
        binding: Binding,
        body: RequestBody,
        request_digest: String,
    ) {
        let deliveries = || {
            self.running
                .lock()
                .expect("running")
                .get(&key)
                .map(|entry| entry.deliveries.clone())
                .unwrap_or_default()
        };
        for delivery in deliveries() {
            self.status(&delivery, &body, Status::Queued);
        }
        let permit = match self.slots.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                for delivery in deliveries() {
                    self.refuse(
                        &delivery,
                        &body.request,
                        body.attempt,
                        Refusal::new("busy")
                            .message("the worker's job bound is full")
                            .retry_after_ms(BUSY_RETRY_MS),
                    );
                }
                self.running.lock().expect("running").remove(&key);
                return;
            }
        };
        {
            let mut running = self.running.lock().expect("running");
            if let Some(entry) = running.get_mut(&key) {
                entry.dispatched = true;
            }
        }
        for delivery in deliveries() {
            self.status(&delivery, &body, Status::Processing);
        }

        let started = Instant::now();
        let settled = self.dispatch(&binding, &body, started).await;
        drop(permit);
        let settled = match self.jobs.settle(&key, &scope, settled.clone()) {
            Ok(true) => settled,
            Ok(false) | Err(_) => {
                // Settled first elsewhere — a cancel or a redrive of a
                // pair already on record. Deliveries that joined before
                // the running entry left still get the recorded result.
                let recorded = match self.jobs.lookup(&key) {
                    Some(JobRecord {
                        settled: Some(record),
                        ..
                    }) => record,
                    _ => return,
                };
                let deliveries = {
                    let mut running = self.running.lock().expect("running");
                    running
                        .remove(&key)
                        .map(|entry| entry.deliveries)
                        .unwrap_or_default()
                };
                for delivery in deliveries {
                    self.publish_result(
                        &delivery,
                        &body,
                        &request_digest,
                        binding.tenant.as_deref(),
                        &recorded,
                    );
                }
                return;
            }
        };
        // Every delivery that carried this pair gets the result bound
        // to its own request event; later deliveries of the pair take
        // the republish path through the ledger.
        for delivery in {
            let mut running = self.running.lock().expect("running");
            running
                .remove(&key)
                .map(|entry| entry.deliveries)
                .unwrap_or_default()
        } {
            self.publish_result(
                &delivery,
                &body,
                &request_digest,
                binding.tenant.as_deref(),
                &settled,
            );
        }
    }

    /// A `type: "cancel"`: resolves only inside the signer's scope, and
    /// only the original request signer may act on the job.
    fn on_cancel(self: &Arc<Self>, cancel: AdmittedCancel) {
        let Some(binding) = self.binding(&cancel.principal) else {
            return;
        };
        let scope = decision::request_scope(
            &cancel.principal,
            binding.tenant.as_deref(),
            &cancel.request,
        );
        let Some(key) = self.jobs.inflight_key(&scope) else {
            // Settled or never seen: a cancel of nothing is ignored.
            return;
        };
        let Some(running) = self.running.lock().expect("running").remove(&key) else {
            return;
        };
        let Some(first) = running.deliveries.first() else {
            return;
        };
        if cancel.authorize(&first.principal).is_err() {
            // Not the job's signer: the cancel is refused by inaction —
            // the job stays live — and nothing is owed to a key that
            // cannot name the job.
            self.running.lock().expect("running").insert(key, running);
            return;
        }
        if let Some(abort) = &running.abort {
            abort.abort();
        }
        let record = self.jobs.lookup(&key);
        let settled = Settled {
            outcome: if running.dispatched {
                "unavailable".to_string()
            } else {
                "unattempted".to_string()
            },
            cause: Some("cancelled".to_string()),
            code: Some("cancelled".to_string()),
            message: Some("the caller cancelled this job".to_string()),
            retry_after_ms: None,
            response: None,
            served_model: None,
            usage: None,
            latency_ms: None,
            resolved_at: Some(now_utc()),
            result_digest: None,
        };
        if let Some(record) = record
            && matches!(self.jobs.settle(&key, &scope, settled.clone()), Ok(true))
        {
            for delivery in &running.deliveries {
                self.publish_result(
                    delivery,
                    &RequestBody::new(
                        record.request.clone(),
                        record.attempt,
                        record.model.clone(),
                        Value::Null,
                        serde_json::Map::new(),
                    ),
                    &record.request_digest,
                    record.tenant.as_deref(),
                    &settled,
                );
            }
        }
    }

    /// The signer-to-principal resolution: bound keys forward under
    /// their credential, unbound signers forward anonymously when the
    /// worker serves the shared lane.
    fn binding(&self, principal: &str) -> Option<Binding> {
        if let Some(principal) = self.config.principals.get(principal) {
            let tenant = principal.tenant.clone().or_else(|| {
                principal
                    .key
                    .strip_prefix("oak_")
                    .and_then(|rest| rest.split('.').next())
                    .map(|id| format!("key-ref:{id}"))
            });
            return Some(Binding {
                key: Some(principal.key.clone()),
                tenant,
                workspace: principal.workspace.clone(),
            });
        }
        self.config.anonymous.then_some(Binding {
            key: None,
            tenant: None,
            workspace: None,
        })
    }

    /// One event's signing material for a reply to `customer` — a fresh
    /// nonce each call, because a reused one reuses the keystream.
    fn seal(&self, customer: &str) -> Seal<'_> {
        let peer: XOnlyPublicKey = customer
            .parse()
            .unwrap_or_else(|_| self.secret.x_only_public_key(&Secp256k1::new()).0);
        Seal {
            signer: &self.signer,
            conversation: nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: unix_now(),
        }
    }

    /// A progress status bound to one delivery.
    fn status(&self, delivery: &Delivery, body: &RequestBody, status: Status) {
        if let Ok(event) = decision::answer_event(
            self.seal(&delivery.principal),
            decision::FEEDBACK_KIND,
            &delivery.attempt_id,
            &delivery.principal,
            &decision::status_payload(&body.request, body.attempt, status),
        ) {
            self.publish(&event);
        }
    }

    /// A terminal refusal bound to one delivery.
    fn refuse(&self, delivery: &Delivery, request: &str, attempt: u32, refusal: Refusal) {
        if let Ok(event) = decision::answer_event(
            self.seal(&delivery.principal),
            decision::FEEDBACK_KIND,
            &delivery.attempt_id,
            &delivery.principal,
            &decision::refusal_payload(request, attempt, &refusal),
        ) {
            self.publish(&event);
        }
    }

    /// Republish a settled pair's recorded result for one delivery.
    fn publish_record(&self, delivery: &Delivery, key: &str) {
        let Some(record) = self.jobs.lookup(key) else {
            return;
        };
        let Some(settled) = &record.settled else {
            return;
        };
        let body = RequestBody::new(
            record.request.clone(),
            record.attempt,
            record.model.clone(),
            Value::Null,
            serde_json::Map::new(),
        );
        self.publish_result(
            delivery,
            &body,
            &record.request_digest,
            record.tenant.as_deref(),
            settled,
        );
    }

    /// Publish one delivery's terminal result — the outcome and answer
    /// from the settled record, the receipt resealed to that delivery's
    /// transport identity.
    fn publish_result(
        &self,
        delivery: &Delivery,
        body: &RequestBody,
        request_digest: &str,
        tenant: Option<&str>,
        settled: &Settled,
    ) {
        let receipt = receipt(delivery, body, request_digest, tenant, settled);
        let resolution = if settled.outcome == "answered" {
            Resolution::Answered(settled.response.clone().unwrap_or(Value::Null))
        } else {
            let outcome = match settled.outcome.as_str() {
                "refused" => Outcome::Refused,
                "unattempted" => Outcome::Unattempted,
                "unavailable" => Outcome::Unavailable,
                _ => Outcome::Unknown,
            };
            let mut refusal = Refusal::new(
                settled
                    .code
                    .clone()
                    .unwrap_or_else(|| "internal".to_string()),
            );
            if let Some(message) = &settled.message {
                refusal = refusal.message(message.clone());
            }
            if let Some(ms) = settled.retry_after_ms {
                refusal = refusal.retry_after_ms(ms);
            }
            Resolution::Ended {
                outcome,
                error: refusal,
            }
        };
        let Ok(payload) =
            decision::result_payload(&body.request, body.attempt, &resolution, &receipt)
        else {
            return;
        };
        if let Ok(event) = decision::answer_event(
            self.seal(&delivery.principal),
            decision::RESULT_KIND,
            &delivery.attempt_id,
            &delivery.principal,
            &payload,
        ) {
            self.publish(&event);
        }
    }

    /// Queue one event for the relay.
    fn publish(&self, event: &Event) {
        self.published.fetch_add(1, Ordering::Relaxed);
        let _ = self.outbox.send(json!(["EVENT", event]).to_string());
    }

    /// The upstream call: the same envelope, the bound credential, and
    /// the pair's idempotency identity — `Idempotency-Key` is the
    /// logical request, `X-Attempt` the attempt, so a relay retry is
    /// not a second spend on the HTTP lane either.
    async fn dispatch(&self, binding: &Binding, body: &RequestBody, started: Instant) -> Settled {
        let envelope = json!({
            "model": body.model,
            "state": body.state,
            "questions": body.questions,
        });
        let mut request = self
            .http
            .post(format!("{}/v1/systemone", self.config.upstream))
            .header("idempotency-key", &body.request)
            .header("x-attempt", body.attempt.to_string())
            .json(&envelope);
        if let Some(key) = &binding.key {
            request = request.bearer_auth(key);
        }
        if let Some(workspace) = &binding.workspace {
            request = request.header("x-workspace-id", workspace);
        }
        let timeout = body
            .deadline
            .map(|deadline| {
                let now = unix_now();
                Duration::from_secs(deadline.saturating_sub(now).max(1))
            })
            .unwrap_or_else(|| Duration::from_secs(self.config.upstream_timeout_secs));
        match tokio::time::timeout(timeout, request.send()).await {
            Ok(Ok(response)) => self.settle_response(response, started).await,
            Ok(Err(error)) => Settled {
                outcome: "unavailable".to_string(),
                cause: Some("unavailable".to_string()),
                code: Some("unavailable".to_string()),
                message: Some(format!("the upstream call failed: {error}")),
                retry_after_ms: None,
                response: None,
                served_model: None,
                usage: None,
                latency_ms: Some(started.elapsed().as_millis() as u64),
                resolved_at: Some(now_utc()),
                result_digest: None,
            },
            Err(_) => Settled {
                outcome: "unavailable".to_string(),
                cause: Some("timeout".to_string()),
                code: Some("unavailable".to_string()),
                message: Some("the deadline or upstream timeout elapsed".to_string()),
                retry_after_ms: None,
                response: None,
                served_model: None,
                usage: None,
                latency_ms: Some(started.elapsed().as_millis() as u64),
                resolved_at: Some(now_utc()),
                result_digest: None,
            },
        }
    }

    /// Map the upstream's HTTP answer to the settled record: the
    /// door's own refusals are `refused`; capacity and transport
    /// failures are `unavailable`, with the exact code kept as cause.
    async fn settle_response(&self, response: reqwest::Response, started: Instant) -> Settled {
        let status = response.status();
        let receipt_ref = response
            .headers()
            .get("x-receipt")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());
        let retry_after_ms = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(|secs| secs * 1000);
        let latency_ms = Some(started.elapsed().as_millis() as u64);
        let resolved_at = Some(now_utc());
        let raw = response.bytes().await.unwrap_or_default();
        let parsed: Option<Value> = serde_json::from_slice(&raw).ok();
        if status.is_success() {
            let response = parsed.unwrap_or(Value::Null);
            let served_model = response
                .get("model")
                .and_then(Value::as_str)
                .map(|model| model.to_string());
            return Settled {
                outcome: "answered".to_string(),
                cause: None,
                code: None,
                message: None,
                retry_after_ms: None,
                response: Some(response),
                served_model,
                usage: receipt_ref,
                latency_ms,
                resolved_at,
                result_digest: Some(digest_bytes(&raw)),
            };
        }
        let (code, message) = parsed
            .as_ref()
            .and_then(|value| value.get("error"))
            .map(|error| {
                (
                    error
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or("unavailable")
                        .chars()
                        .take(decision::MAX_CODE_BYTES)
                        .collect::<String>(),
                    error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("the upstream refused")
                        .chars()
                        .take(decision::MAX_MESSAGE_BYTES)
                        .collect::<String>(),
                )
            })
            .unwrap_or_else(|| {
                (
                    "unavailable".to_string(),
                    format!("the upstream answered {status} without a typed error"),
                )
            });
        let outcome = match code.as_str() {
            "busy"
            | "rate_limited"
            | "overloaded"
            | "door_unavailable"
            | "identity_mismatch"
            | "unavailable"
            | "registry_unavailable"
            | "membership_unavailable" => "unavailable",
            _ => "refused",
        };
        Settled {
            outcome: outcome.to_string(),
            cause: Some(code.clone()),
            code: Some(code),
            message: Some(message),
            retry_after_ms,
            response: None,
            served_model: None,
            usage: receipt_ref,
            latency_ms,
            resolved_at,
            result_digest: None,
        }
    }
}

/// The sealed `ExecutionReceipt` for one delivery of a settled attempt —
/// `attempt_id` is that delivery's request event, so a republished
/// answer binds to the transport event it answers.
fn receipt(
    delivery: &Delivery,
    body: &RequestBody,
    request_digest: &str,
    tenant: Option<&str>,
    settled: &Settled,
) -> Value {
    let mut receipt = ExecutionReceipt::for_attempt(
        decision::RELAY_TRANSPORT,
        body.request.clone(),
        body.attempt,
        request_digest.to_string(),
    );
    receipt.attempt_id = delivery.attempt_id.clone();
    receipt.tenant = tenant.map(|tenant| tenant.to_string());
    receipt.requested = Served {
        model: body.model.clone(),
        ..Served::default()
    };
    receipt.served = Served {
        model: settled.served_model.clone().unwrap_or_default(),
        ..Served::default()
    };
    receipt.outcome = match settled.outcome.as_str() {
        "answered" => ReceiptOutcome::Answered,
        "refused" => ReceiptOutcome::Refused,
        "unattempted" => ReceiptOutcome::Unattempted,
        "unavailable" => ReceiptOutcome::Unavailable,
        _ => ReceiptOutcome::Unknown,
    };
    receipt.cause = settled.cause.clone();
    receipt.timing = Timing {
        queued_ms: None,
        latency_ms: settled.latency_ms,
        resolved_at: settled.resolved_at.clone(),
    };
    receipt.result_digest = settled.result_digest.clone();
    receipt.usage = settled.usage.clone();
    receipt.seal();
    serde_json::to_value(&receipt).unwrap_or(Value::Null)
}

/// The supervisor: open the worker, then serve the relay connection,
/// reconnecting until the process is stopped.
pub async fn run(config: WorkerConfig) -> Result<(), Trouble> {
    let worker = Worker::open(config)?;
    eprintln!("decision-worker: pubkey {}", worker.pubkey());
    eprintln!("decision-worker: upstream {}", worker.config.upstream);
    eprintln!("decision-worker: relay {}", worker.config.relay);
    loop {
        match tokio_tungstenite::connect_async(&worker.config.relay).await {
            Ok((socket, _)) => {
                if let Err(error) = worker.serve(socket).await {
                    eprintln!("decision-worker: session ended: {error}; reconnecting");
                }
            }
            Err(error) => {
                eprintln!("decision-worker: connect failed: {error}; retrying");
            }
        }
        tokio::time::sleep(Duration::from_secs(RECONNECT_SECS)).await;
    }
}

/// Read one JSON message from the socket.
async fn read_json(socket: &mut Socket) -> Result<Value, Trouble> {
    loop {
        match socket.next().await {
            Some(Ok(tungstenite::Message::Text(text))) => {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    return Ok(value);
                }
            }
            Some(Ok(_)) => {}
            Some(Err(error)) => return Err(Trouble::Relay(format!("socket: {error}"))),
            None => return Err(Trouble::Relay("the socket closed".into())),
        }
    }
}

async fn send_json(socket: &mut Socket, value: &Value) -> Result<(), Trouble> {
    send_text(socket, &value.to_string()).await
}

async fn send_text(socket: &mut Socket, text: &str) -> Result<(), Trouble> {
    tokio::time::timeout(
        WRITE_TIMEOUT,
        socket.send(tungstenite::Message::Text(text.to_string().into())),
    )
    .await
    .map_err(|_| Trouble::Relay("a socket write stalled".into()))?
    .map_err(|error| Trouble::Relay(format!("socket write: {error}")))
}

/// SHA-256 of raw bytes, `sha256:`-prefixed — the result digest the
/// receipt carries.
fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::from("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Lowercase-hex decode, for the configured key material.
fn hex_decode(hex: &str) -> Result<Vec<u8>, Trouble> {
    if !hex.len().is_multiple_of(2) {
        return Err(Trouble::Config(
            "odd-length hex in key material".to_string(),
        ));
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16)
                .map_err(|error| Trouble::Config(format!("key material is not hex: {error}")))
        })
        .collect()
}
