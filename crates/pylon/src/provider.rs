//! The provider: publish beacons, take free NIP-CJ conversation jobs from
//! admitted buyers, run them on the engine, and answer encrypted.
//!
//! Admission happens before any model work: a verified signature, this
//! pylon as the only recipient, a fresh and unseen request, a buyer on the
//! allowlist, the buyer's rate limit, a free slot, and the request bounds.
//! Connections live at most 115 seconds; a new one opens every 100 seconds,
//! so two overlap and no request falls between them. Requests are
//! deduplicated by event ID.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nostr::domain::{Event, MintedOwnerAttestation};
use nostr::pylon::{
    BEACON_V, Beacon, Class, Family, Lane, Service, Slots, Status, Tier, owned_beacon_event,
};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};
use tokio::time::timeout;

use crate::engine::Engine;
use crate::identity::Identity;
use crate::job::{self, Refusal};
use crate::lease::{Dedicated, Machine};
use crate::now;
use crate::relay::{self, Frame, LIFETIME};

/// How a pylon is set up.
#[derive(Debug, Clone)]
pub struct Config {
    pub relay: String,
    /// The beacon's `d` slug.
    pub pylon: String,
    pub label: String,
    pub slots: u32,
    /// Hex keys that may send jobs; `None` admits any key (rate limits
    /// still apply).
    pub allow: Option<BTreeSet<String>>,
    /// Jobs per buyer per minute.
    pub rate_per_minute: u32,
    pub max_tokens: u32,
    pub class: Class,
    pub pools: Vec<String>,
    /// Where the pylon keeps its generation counter.
    pub home: PathBuf,
    /// How long one job may run.
    pub job_timeout: Duration,
    /// The NIP-OA credential by which the owner authorized this pylon key;
    /// every beacon carries it.
    pub owner: Option<MintedOwnerAttestation>,
}

impl Config {
    /// Defaults for a 16 GB GPU pylon on `relay`.
    #[must_use]
    pub fn new(relay: &str, pylon: &str, home: PathBuf) -> Self {
        Self {
            relay: relay.into(),
            pylon: pylon.into(),
            label: pylon.into(),
            slots: 2,
            allow: Some(BTreeSet::new()),
            rate_per_minute: 10,
            max_tokens: 512,
            class: Class {
                family: Family::Gpu,
                tier: Tier::for_gpu(16),
                memory_gb: 16,
            },
            pools: vec!["everglade".into()],
            home,
            job_timeout: Duration::from_secs(90),
            owner: None,
        }
    }

    /// The capability ID this pylon serves: `<pylon key>:pylon/text-generation`,
    /// the qualified-ID form of a CAP DefinitionRef.
    #[must_use]
    pub fn capability(pubkey: &str) -> String {
        format!("{pubkey}:pylon/text-generation")
    }
}

/// A per-buyer token bucket.
struct Bucket {
    tokens: f64,
    at: Instant,
}

/// What the provider counts while it runs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counters {
    pub served: u64,
    pub refused: u64,
    pub failed: u64,
}

struct State {
    free: u32,
    seen: HashSet<String>,
    order: VecDeque<String>,
    buckets: HashMap<String, Bucket>,
    healthy: bool,
    draining: bool,
    /// The owner's work needs the machine, as last checked.
    yielding: bool,
    counters: Counters,
}

/// A running pylon.
pub struct Provider {
    config: Config,
    identity: Identity,
    engine: Arc<dyn Engine>,
    machine: Arc<dyn Machine>,
    generation: u64,
    since: u64,
    state: Mutex<State>,
    outbound: mpsc::Sender<Event>,
    inbound: Mutex<mpsc::Receiver<Event>>,
    changed: tokio::sync::Notify,
}

const SEEN_BOUND: usize = 4_096;

impl Provider {
    /// A provider for `config`, signing as `identity`, running jobs on `engine`.
    /// Bumps the generation counter in `config.home`.
    ///
    /// # Errors
    ///
    /// When the generation counter cannot be read or written.
    pub fn new(
        config: Config,
        identity: Identity,
        engine: Arc<dyn Engine>,
    ) -> Result<Arc<Self>, String> {
        Self::on(config, identity, engine, Arc::new(Dedicated))
    }

    /// A provider that shares `machine` with its owner's work: each job
    /// takes the machine's lease, and the pylon drains while the owner's
    /// work needs it.
    ///
    /// # Errors
    ///
    /// As [`Provider::new`], and when the owner credential does not verify
    /// for this pylon key.
    pub fn on(
        config: Config,
        identity: Identity,
        engine: Arc<dyn Engine>,
        machine: Arc<dyn Machine>,
    ) -> Result<Arc<Self>, String> {
        if let Some(owner) = &config.owner {
            crate::identity::check_owner(&identity, owner)?;
        }
        std::fs::create_dir_all(&config.home).map_err(|e| e.to_string())?;
        let path = config.home.join(format!("{}.generation", config.pylon));
        let generation = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| t.trim().parse::<u64>().ok())
            .unwrap_or(0)
            + 1;
        std::fs::write(&path, generation.to_string()).map_err(|e| e.to_string())?;
        let (outbound, inbound) = mpsc::channel(256);
        Ok(Arc::new(Self {
            state: Mutex::new(State {
                free: config.slots,
                seen: HashSet::new(),
                order: VecDeque::new(),
                buckets: HashMap::new(),
                healthy: true,
                draining: false,
                yielding: false,
                counters: Counters::default(),
            }),
            config,
            identity,
            engine,
            machine,
            generation,
            since: now(),
            outbound,
            inbound: Mutex::new(inbound),
            changed: tokio::sync::Notify::new(),
        }))
    }

    /// The pylon's public key.
    #[must_use]
    pub fn pubkey(&self) -> &str {
        self.identity.pubkey()
    }

    /// What the provider has done so far.
    pub async fn counters(&self) -> Counters {
        self.state.lock().await.counters
    }

    /// The beacon as of now.
    pub async fn beacon(&self, status: Option<Status>) -> Beacon {
        let state = self.state.lock().await;
        let status = status.unwrap_or(if state.draining || !state.healthy || state.yielding {
            Status::Draining
        } else {
            Status::Online
        });
        let observed_at = now();
        Beacon {
            v: BEACON_V.into(),
            requires: Vec::new(),
            meta: None,
            provider: self.pubkey().into(),
            pylon: self.config.pylon.clone(),
            label: self.config.label.clone(),
            status,
            generation: self.generation,
            since: self.since,
            observed_at,
            valid_until: observed_at + 240,
            class: self.config.class.clone(),
            slots: Slots {
                total: self.config.slots,
                free: if status == Status::Online {
                    state.free
                } else {
                    0
                },
            },
            services: vec![Service {
                capability: Config::capability(self.pubkey()),
                model: self.engine.model().chars().take(128).collect(),
                lanes: vec![Lane::CjConversation],
                offering: None,
                price_hint_msat: None,
            }],
            settlement: vec!["free-v1".into()],
            pools: self.config.pools.clone(),
        }
    }

    /// Serve until `stop` resolves, then publish an offline beacon.
    ///
    /// # Errors
    ///
    /// When the offline beacon cannot be signed. Relay failures retry.
    pub async fn run(self: Arc<Self>, stop: impl Future<Output = ()>) -> Result<(), String> {
        let beacons = tokio::spawn(Arc::clone(&self).beacon_loop());
        let sessions = tokio::spawn(Arc::clone(&self).session_loop());
        stop.await;
        self.state.lock().await.draining = true;
        beacons.abort();
        sessions.abort();
        let event = owned_beacon_event(
            self.identity.signer(),
            &self.beacon(Some(Status::Offline)).await,
            self.config.owner.as_ref(),
        )?;
        match relay::connect(&self.config.relay, &self.identity, Duration::from_secs(10)).await {
            Ok(mut conn) => {
                if let Err(e) = relay::publish(&mut conn, &event).await {
                    eprintln!("pylon: offline beacon: {e}");
                }
                let _ = conn.close().await;
            }
            Err(e) => eprintln!("pylon: offline beacon: {e}"),
        }
        Ok(())
    }

    async fn beacon_loop(self: Arc<Self>) {
        let mut last: Option<(Instant, (Status, u32))> = None;
        let mut checked = Instant::now() - Duration::from_secs(60);
        loop {
            if checked.elapsed() >= Duration::from_secs(30) {
                let healthy = self.engine.healthy().await;
                self.state.lock().await.healthy = healthy;
                checked = Instant::now();
            }
            let machine = Arc::clone(&self.machine);
            let yielding = tokio::task::spawn_blocking(move || machine.owner_busy())
                .await
                .unwrap_or(true);
            self.state.lock().await.yielding = yielding;
            let beacon = self.beacon(None).await;
            let key = (beacon.status, beacon.slots.free);
            let due = match last {
                None => true,
                Some((at, held)) => {
                    let since = at.elapsed();
                    (held != key && since >= Duration::from_secs(10))
                        || since >= Duration::from_secs(60)
                }
            };
            if due {
                match owned_beacon_event(
                    self.identity.signer(),
                    &beacon,
                    self.config.owner.as_ref(),
                ) {
                    Ok(event) => {
                        if self.outbound.send(event).await.is_err() {
                            return;
                        }
                        last = Some((Instant::now(), key));
                    }
                    Err(e) => eprintln!("pylon: beacon: {e}"),
                }
            }
            tokio::select! {
                () = self.changed.notified() => tokio::time::sleep(Duration::from_millis(200)).await,
                () = tokio::time::sleep(Duration::from_secs(5)) => {}
            }
        }
    }

    async fn session_loop(self: Arc<Self>) {
        loop {
            tokio::spawn(Arc::clone(&self).session());
            tokio::time::sleep(Duration::from_secs(100)).await;
        }
    }

    /// One connection: subscribe to requests for this pylon, answer them,
    /// and carry outbound events, until the connection's lifetime ends.
    async fn session(self: Arc<Self>) {
        let window = Instant::now() + LIFETIME - Duration::from_secs(5);
        let mut conn = loop {
            let left = window.saturating_duration_since(Instant::now());
            if left < Duration::from_secs(10) {
                return;
            }
            // The connection outlives the session by a few seconds, so the
            // session's own deadline ends it, not a read timeout.
            let lifetime = left + Duration::from_secs(4);
            match relay::connect(&self.config.relay, &self.identity, lifetime).await {
                Ok(conn) => break conn,
                Err(e) => {
                    eprintln!("pylon: {e}; retrying");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        };
        let filter = json!({
            "kinds": [job::REQUEST_KIND],
            "#p": [self.pubkey()],
            "since": now().saturating_sub(10),
        });
        if conn.send(json!(["REQ", "jobs", filter])).await.is_err() {
            return;
        }
        let deadline = tokio::time::Instant::from_std(window);
        loop {
            tokio::select! {
                frame = conn.next() => match frame {
                    Ok(value) => match Frame::parse(value) {
                        Frame::Event { sub, event } if sub == "jobs" => {
                            tokio::spawn(Arc::clone(&self).admit(*event));
                        }
                        Frame::Ok { accepted: false, message, .. } => {
                            eprintln!("pylon: relay refused an event: {message}");
                        }
                        Frame::Closed { message, .. } => {
                            eprintln!("pylon: relay closed the job subscription: {message}");
                            return;
                        }
                        _ => {}
                    },
                    Err(e) => {
                        eprintln!("pylon: relay connection ended: {e}");
                        return;
                    }
                },
                event = async { self.inbound.lock().await.recv().await } => {
                    let Some(event) = event else { return };
                    if let Err(e) = conn.send(json!(["EVENT", event])).await {
                        eprintln!("pylon: send failed: {e}");
                        let _ = self.outbound.try_send(event);
                        return;
                    }
                }
                () = tokio::time::sleep_until(deadline) => {
                    let _ = conn.close().await;
                    return;
                }
            }
        }
    }

    /// Admit one request event, run it, and queue the answer.
    pub async fn admit(self: Arc<Self>, event: Event) {
        if event.kind != job::REQUEST_KIND || event.validate_crypto().is_err() {
            return;
        }
        let p: Vec<_> = event.tag_values("p").collect();
        if p != [self.pubkey()] {
            return;
        }
        let at = now();
        if event.created_at + 120 < at || event.created_at > at + 30 {
            return;
        }
        {
            let mut state = self.state.lock().await;
            if !state.seen.insert(event.id.clone()) {
                return;
            }
            state.order.push_back(event.id.clone());
            if state.order.len() > SEEN_BOUND
                && let Some(old) = state.order.pop_front()
            {
                state.seen.remove(&old);
            }
        }
        let mut gate = self.gate(&event).await;
        let took_slot = gate.is_ok();
        // The machine's lease, held for the whole job.
        let mut _lease = None;
        if took_slot {
            let machine = Arc::clone(&self.machine);
            match tokio::task::spawn_blocking(move || machine.take()).await {
                Ok(Ok(guard)) => _lease = Some(guard),
                Ok(Err(_)) | Err(_) => {
                    self.state.lock().await.yielding = true;
                    let mut refusal =
                        Refusal::new("rate_limited", "the owner's work needs this computer");
                    refusal.retry_after_ms = Some(30_000);
                    gate = Err(refusal);
                }
            }
        }
        let request = match gate {
            Err(refusal) => Err(refusal),
            Ok(()) => match job::open(&self.identity, &event) {
                Err(_) => Err(Refusal::new("malformed", "request does not decrypt")),
                Ok(plaintext) => job::parse_request(&plaintext),
            },
        };
        let request = match request {
            Ok(request) => request,
            Err(refusal) => {
                let mut state = self.state.lock().await;
                state.counters.refused += 1;
                if took_slot {
                    state.free += 1;
                }
                drop(state);
                self.changed.notify_one();
                self.answer(&event, job::FEEDBACK_KIND, &job::refusal_body(1, &refusal));
                return;
            }
        };
        self.changed.notify_one();
        self.answer(
            &event,
            job::FEEDBACK_KIND,
            &job::status_body(request.version, "processing"),
        );
        let outcome = timeout(
            self.config.job_timeout,
            self.engine.generate(&request.turns, self.config.max_tokens),
        )
        .await;
        let body = match outcome {
            Ok(Ok(generation)) => {
                self.state.lock().await.counters.served += 1;
                let mut body = json!({
                    "v": request.version,
                    "requires": [],
                    "type": "result",
                    "text": generation.text,
                    "model": generation.model,
                });
                if let (Some(input), Some(output)) =
                    (generation.input_tokens, generation.output_tokens)
                {
                    body["usage"] = json!({"input": input, "output": output});
                }
                Some(body)
            }
            Ok(Err(e)) => {
                eprintln!("pylon: job {} failed: {e}", &event.id[..12]);
                self.state.lock().await.counters.failed += 1;
                self.answer(
                    &event,
                    job::FEEDBACK_KIND,
                    &job::refusal_body(
                        request.version,
                        &Refusal::new("unavailable", "the model failed"),
                    ),
                );
                None
            }
            Err(_) => {
                self.state.lock().await.counters.failed += 1;
                self.answer(
                    &event,
                    job::FEEDBACK_KIND,
                    &job::refusal_body(
                        request.version,
                        &Refusal::new("limit_exceeded", "the job ran out of time"),
                    ),
                );
                None
            }
        };
        if let Some(body) = body {
            self.answer(&event, job::RESULT_KIND, &body);
        }
        self.state.lock().await.free += 1;
        self.changed.notify_one();
    }

    /// The allowlist, the buyer's rate limit, and a slot. Takes the slot.
    async fn gate(&self, event: &Event) -> Result<(), Refusal> {
        if let Some(allow) = &self.config.allow
            && !allow.contains(&event.pubkey)
        {
            return Err(Refusal::new(
                "not_admitted",
                "this pylon serves listed keys only",
            ));
        }
        let mut state = self.state.lock().await;
        let rate = f64::from(self.config.rate_per_minute.max(1));
        let bucket = state.buckets.entry(event.pubkey.clone()).or_insert(Bucket {
            tokens: rate,
            at: Instant::now(),
        });
        bucket.tokens = (bucket.tokens + bucket.at.elapsed().as_secs_f64() * rate / 60.0).min(rate);
        bucket.at = Instant::now();
        if bucket.tokens < 1.0 {
            let wait = ((1.0 - bucket.tokens) * 60_000.0 / rate).ceil() as u64;
            let mut refusal = Refusal::new("rate_limited", "too many jobs from this key");
            refusal.retry_after_ms = Some(wait);
            return Err(refusal);
        }
        bucket.tokens -= 1.0;
        if state.draining || !state.healthy || state.yielding || state.free == 0 {
            let mut refusal = Refusal::new("rate_limited", "no free slot");
            refusal.retry_after_ms = Some(2_000);
            return Err(refusal);
        }
        state.free -= 1;
        Ok(())
    }

    /// Queue an encrypted answer to `request`.
    fn answer(&self, request: &Event, kind: u16, body: &serde_json::Value) {
        match job::seal(
            &self.identity,
            &request.pubkey,
            kind,
            job::response_tags(request),
            body,
            now(),
        ) {
            Ok(event) => {
                if self.outbound.try_send(event).is_err() {
                    eprintln!("pylon: outbound queue full; dropped an answer");
                }
            }
            Err(e) => eprintln!("pylon: sealing an answer: {e}"),
        }
    }
}
