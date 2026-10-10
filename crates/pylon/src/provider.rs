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

use nostr::decision::{self, Admitted, RequestWindow, Resolution, Seal};
use nostr::domain::{Event, MintedOwnerAttestation};
use nostr::pylon::{
    BEACON_V, Beacon, Class, Family, Lane, Service, Slots, Status, Tier, owned_beacon_event,
};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};
use tokio::time::timeout;

use crate::decide::Decider;
use crate::engine::Engine;
use crate::identity::Identity;
use crate::job::{self, Refusal};
use crate::lease::{Dedicated, Machine};
use crate::now;
use crate::paid::{self, Grant, Price, Receiver, Seller};
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
    /// A posted price per job; `None` serves free (`free-v1`). A priced
    /// pylon sells each job under NIP-X402 ([`Provider::priced`]).
    pub price: Option<Price>,
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
            price: None,
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
    /// The text model; `None` for a pylon that only answers decisions.
    engine: Option<Arc<dyn Engine>>,
    /// The System One server decisions go to, when this pylon answers them.
    decider: Option<Arc<dyn Decider>>,
    machine: Arc<dyn Machine>,
    seller: Option<Seller>,
    generation: u64,
    since: u64,
    state: Mutex<State>,
    outbound: mpsc::Sender<Event>,
    inbound: Mutex<mpsc::Receiver<Event>>,
    changed: tokio::sync::Notify,
    /// Set in the attested serve mode (NIP-ATT): decisions must be sealed
    /// jobs for this endpoint, and their answers name it.
    attested: std::sync::OnceLock<crate::attested::Attestation>,
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
        Self::build(config, identity, Some(engine), None, machine, None)
    }

    /// A free pylon that answers NIP-DEC decision jobs with `decider`, and
    /// text jobs too when `engine` is given (#11225). Its beacon advertises
    /// `<pylon key>:pylon/decision` on the `cj-decision` lane with the
    /// decider's served identity.
    ///
    /// # Errors
    ///
    /// As [`Provider::on`], and for a priced config: decisions are free work.
    pub fn deciding(
        config: Config,
        identity: Identity,
        engine: Option<Arc<dyn Engine>>,
        decider: Arc<dyn Decider>,
        machine: Arc<dyn Machine>,
    ) -> Result<Arc<Self>, String> {
        if config.price.is_some() {
            return Err("a pylon answers decisions for free; drop the price".into());
        }
        Self::build(config, identity, engine, Some(decider), machine, None)
    }

    /// A priced pylon: every job is bought first under NIP-X402, with
    /// invoices from `receiver` on the price's network; its purchase
    /// ledger and replay store live under `config.home`.
    ///
    /// # Errors
    ///
    /// As [`Provider::on`], and when the config has no price, the price is
    /// on a network x402 does not name, or the price is on `bitcoin`
    /// without the owner's standing `grant` or over its per-payment
    /// ceiling.
    pub fn priced(
        config: Config,
        identity: Identity,
        engine: Arc<dyn Engine>,
        machine: Arc<dyn Machine>,
        receiver: Arc<dyn Receiver>,
        grant: Option<Grant>,
    ) -> Result<Arc<Self>, String> {
        let price = config.price.ok_or("a priced pylon needs a price")?;
        if !price.network.is_test() {
            let grant = grant.ok_or("mainnet pylon sales need the owner's standing grant")?;
            if price.msat > grant.per_payment_msat {
                return Err("the price is over the owner's per-payment ceiling".into());
            }
        }
        let dir =
            config
                .home
                .join("x402")
                .join(format!("{}-{}", config.pylon, &identity.pubkey()[..16]));
        let per_hour = config.rate_per_minute.saturating_mul(60);
        let seller = Seller::open(&dir, identity.pubkey(), price, receiver, Some(per_hour))?;
        Self::build(config, identity, Some(engine), None, machine, Some(seller))
    }

    fn build(
        config: Config,
        identity: Identity,
        engine: Option<Arc<dyn Engine>>,
        decider: Option<Arc<dyn Decider>>,
        machine: Arc<dyn Machine>,
        seller: Option<Seller>,
    ) -> Result<Arc<Self>, String> {
        if config.price.is_some() && seller.is_none() {
            return Err("a priced pylon needs a wallet".into());
        }
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
            decider,
            machine,
            seller,
            generation,
            since: now(),
            outbound,
            inbound: Mutex::new(inbound),
            changed: tokio::sync::Notify::new(),
            attested: std::sync::OnceLock::new(),
        }))
    }

    /// Turn on the attested mode: from now on a decision is answered only
    /// as a sealed job for `attestation`'s endpoint and release.
    ///
    /// # Errors
    ///
    /// When the mode was already set.
    pub fn attest(&self, attestation: crate::attested::Attestation) -> Result<(), String> {
        self.attested
            .set(attestation)
            .map_err(|_| "the attested mode is already set".to_string())
    }

    /// Queue an event this pylon signed (an attested endpoint, say) for
    /// the relay.
    ///
    /// # Errors
    ///
    /// When the provider has stopped.
    pub async fn queue(&self, event: Event) -> Result<(), String> {
        self.outbound
            .send(event)
            .await
            .map_err(|_| "the pylon has stopped".to_string())
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
            // Inert: the attested endpoint this pylon's key serves. The
            // level a reader shows comes only from that endpoint's evidence.
            meta: self
                .attested
                .get()
                .map(|a| json!({"attested_endpoint": a.address, "claimed_level": a.level.as_str()})),
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
            services: self.services(),
            settlement: vec![if self.config.price.is_some() {
                paid::PROFILE.into()
            } else {
                "free-v1".into()
            }],
            pools: self.config.pools.clone(),
        }
    }

    /// The services the beacon advertises: text generation when there is an
    /// engine, decisions when there is a decider.
    fn services(&self) -> Vec<Service> {
        let mut services = Vec::new();
        if let Some(engine) = &self.engine {
            services.push(Service {
                capability: Config::capability(self.pubkey()),
                model: engine.model().chars().take(128).collect(),
                lanes: vec![Lane::CjConversation],
                offering: None,
                price_hint_msat: self.config.price.map(|p| p.msat),
            });
        }
        if let Some(decider) = &self.decider {
            services.push(Service {
                capability: crate::decide::capability(self.pubkey()),
                model: decider.identity().advertised(),
                lanes: vec![Lane::CjDecision],
                offering: None,
                price_hint_msat: None,
            });
        }
        services
    }

    /// Whether every model this pylon serves answers right now.
    async fn models_healthy(&self) -> bool {
        let text = match &self.engine {
            Some(engine) => engine.healthy().await,
            None => true,
        };
        let decisions = match &self.decider {
            Some(decider) => decider.healthy().await,
            None => true,
        };
        text && decisions
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
                let healthy = self.models_healthy().await;
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
        let mut kinds = Vec::new();
        if self.engine.is_some() {
            kinds.push(job::REQUEST_KIND);
        }
        if self.decider.is_some() {
            kinds.push(decision::REQUEST_KIND);
        }
        let filter = json!({
            "kinds": kinds,
            "#p": [self.pubkey()],
            "since": now().saturating_sub(10),
        });
        let mut req = json!(["REQ", "jobs", filter]);
        if self.seller.is_some()
            && let Some(frame) = req.as_array_mut()
        {
            frame.push(paid::inbox(self.pubkey(), now().saturating_sub(10)));
        }
        if conn.send(req).await.is_err() {
            return;
        }
        let deadline = tokio::time::Instant::from_std(window);
        loop {
            tokio::select! {
                frame = conn.next() => match frame {
                    Ok(value) => match Frame::parse(value) {
                        Frame::Event { sub, event } if sub == "jobs" => {
                            if event.kind == job::REQUEST_KIND {
                                tokio::spawn(Arc::clone(&self).admit(*event));
                            } else if event.kind == decision::REQUEST_KIND {
                                tokio::spawn(Arc::clone(&self).decide(*event));
                            } else {
                                tokio::spawn(Arc::clone(&self).purchase(*event));
                            }
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
        let Some(engine) = self.engine.clone() else {
            return;
        };
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
        let mut plaintext = String::new();
        let request = match gate {
            Err(refusal) => Err(refusal),
            Ok(()) => match job::open(&self.identity, &event) {
                Err(_) => Err(Refusal::new("malformed", "request does not decrypt")),
                Ok(plain) => {
                    let parsed = job::parse_request(&plain);
                    plaintext = plain;
                    parsed
                }
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
        let purchase = match self.collect(&event, &plaintext).await {
            Ok(purchase) => purchase,
            Err(refusal) => {
                let mut state = self.state.lock().await;
                state.counters.refused += 1;
                state.free += 1;
                drop(state);
                self.changed.notify_one();
                self.answer(
                    &event,
                    job::FEEDBACK_KIND,
                    &job::refusal_body(request.version, &refusal),
                );
                return;
            }
        };
        self.answer(
            &event,
            job::FEEDBACK_KIND,
            &job::status_body(request.version, "processing"),
        );
        let outcome = timeout(
            self.config.job_timeout,
            engine.generate(&request.turns, self.config.max_tokens),
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
                Ok(body)
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
                Err("worker_failed")
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
                Err("execute_until_passed")
            }
        };
        if let Ok(body) = &body {
            self.answer(&event, job::RESULT_KIND, body);
        }
        if let Some(purchase) = purchase {
            // The result's plaintext is what `answer` sealed: the body's
            // JSON text.
            let result = body.as_ref().map(ToString::to_string).map_err(|e| *e);
            self.settle_purchase(&event.pubkey, &purchase, result).await;
        }
        self.state.lock().await.free += 1;
        self.changed.notify_one();
    }

    /// Answer one NIP-DEC decision job (`25910`) with the decider (#11225):
    /// the same admission as a text job (a verified, addressed, fresh,
    /// unseen request from an allowed key under its rate, with a free
    /// slot), then `27010 processing`, the decider's answer, and a `26910`
    /// result carrying a sealed execution receipt. A refusal is a `27010`
    /// error.
    pub async fn decide(self: Arc<Self>, event: Event) {
        let Some(decider) = self.decider.clone() else {
            return;
        };
        if event.kind != decision::REQUEST_KIND || event.validate_crypto().is_err() {
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
        let call = match decision::admit(
            &event,
            self.pubkey(),
            self.identity.secret(),
            now(),
            RequestWindow::new(120, 30),
        ) {
            Ok(Admitted::Call(call)) => call,
            // A decision runs for a second or two; there is nothing to stop.
            Ok(Admitted::Cancel(_)) => return,
            Err(error) => {
                self.state.lock().await.counters.refused += 1;
                if let Some(refusal) = decision::Refusal::from_error(&error)
                    && let Ok(payload) = decision::decrypt_payload(&event, self.identity.secret())
                    && let Some((request, attempt)) = decision::payload_correlation(&payload)
                {
                    let body = decision::refusal_payload(&request, attempt, &refusal);
                    self.send_decision(&event, |seal| {
                        decision::answer_event(
                            seal,
                            decision::FEEDBACK_KIND,
                            &event.id,
                            &event.pubkey,
                            &body,
                        )
                    });
                }
                return;
            }
        };
        let refuse = |this: &Self, code: &str, message: String, retry: Option<u64>| {
            let mut refusal = decision::Refusal::new(code).message(message);
            if let Some(ms) = retry {
                refusal = refusal.retry_after_ms(ms);
            }
            this.send_decision(&event, |seal| call.refusal_event(seal, &refusal));
        };
        if let Some(attested) = self.attested.get()
            && let Err(why) =
                nostr::att::check_sealed(&call.payload, &attested.address, &attested.release)
        {
            self.state.lock().await.counters.refused += 1;
            refuse(&self, "unsupported_feature", why, None);
            return;
        }
        if let Err(refusal) = self.gate(&event).await {
            self.state.lock().await.counters.refused += 1;
            let code = match refusal.code {
                "rate_limited" if refusal.message == "no free slot" => "busy",
                other => other,
            };
            refuse(&self, code, refusal.message, refusal.retry_after_ms);
            return;
        }
        self.changed.notify_one();
        let machine = Arc::clone(&self.machine);
        let _lease = match tokio::task::spawn_blocking(move || machine.take()).await {
            Ok(Ok(guard)) => guard,
            Ok(Err(_)) | Err(_) => {
                let mut state = self.state.lock().await;
                state.yielding = true;
                state.counters.refused += 1;
                state.free += 1;
                drop(state);
                self.changed.notify_one();
                refuse(
                    &self,
                    "busy",
                    "the owner's work needs this computer".into(),
                    Some(30_000),
                );
                return;
            }
        };
        self.send_decision(&event, |seal| {
            call.status_event(seal, decision::Status::Processing)
        });
        let started = Instant::now();
        let outcome = timeout(
            self.config.job_timeout,
            decider.decide(&call.body.state, &call.body.questions),
        )
        .await;
        let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let identity = decider.identity();
        match outcome {
            Ok(Ok(mut response)) => {
                self.state.lock().await.counters.served += 1;
                response["model"] = json!(identity.model);
                response["service"] = json!({
                    "door": format!("pylon:{}", self.config.pylon),
                    "version": concat!("pylon@", env!("CARGO_PKG_VERSION")),
                    "provider": self.pubkey(),
                    "identity": identity.advertised(),
                });
                response["latency_ms"] = json!(latency_ms);
                let attested = self.attested.get();
                if let Some(a) = attested {
                    response["attested"] = nostr::att::attested_block(
                        &a.address,
                        &a.release,
                        a.level,
                        &a.measurement,
                        &event.content,
                        &a.model,
                        &a.model_digest,
                    );
                }
                let mut receipt = decision_receipt(&call, &identity, latency_ms, &response);
                if attested.is_some() {
                    receipt = attested_receipt(receipt, &response);
                }
                let resolution = Resolution::Answered(response);
                self.send_decision(&event, |seal| {
                    call.result_event(seal, &resolution, &receipt)
                });
                eprintln!(
                    "pylon: decision {} answered by {} in {latency_ms} ms",
                    &event.id[..12],
                    identity.model
                );
            }
            Ok(Err(refusal)) => {
                self.state.lock().await.counters.failed += 1;
                eprintln!(
                    "pylon: decision {} refused by the model server: {}",
                    &event.id[..12],
                    refusal.code
                );
                self.send_decision(&event, |seal| call.refusal_event(seal, &refusal));
            }
            Err(_) => {
                self.state.lock().await.counters.failed += 1;
                refuse(
                    &self,
                    "timeout",
                    "the model server did not answer in time".into(),
                    None,
                );
            }
        }
        self.state.lock().await.free += 1;
        self.changed.notify_one();
    }

    /// Seal one decision-job event to the request's signer and queue it.
    fn send_decision(
        &self,
        request: &Event,
        build: impl FnOnce(Seal<'_>) -> Result<Event, decision::DecisionError>,
    ) {
        let Some(peer) = crate::identity::parse_pubkey(&request.pubkey) else {
            return;
        };
        let seal = Seal {
            signer: self.identity.signer(),
            conversation: nostr::nip44::conversation_key(self.identity.secret(), &peer),
            nonce: secp256k1::rand::random(),
            created_at: now(),
        };
        match build(seal) {
            Ok(event) => {
                if self.outbound.try_send(event).is_err() {
                    eprintln!("pylon: outbound queue full; dropped a decision answer");
                }
            }
            Err(e) => eprintln!("pylon: sealing a decision answer: {e}"),
        }
    }

    /// For a priced pylon, the buyer's admitted NIP-X402 purchase whose
    /// input is this request's plaintext, moved to `running`; a free pylon
    /// collects nothing. A job no settled purchase admits is refused.
    async fn collect(&self, request: &Event, plaintext: &str) -> Result<Option<String>, Refusal> {
        let Some(seller) = &self.seller else {
            return Ok(None);
        };
        let (purchase, records) = seller
            .start(
                &request.pubkey,
                plaintext,
                &self.config.relay,
                &request.id,
                now(),
            )
            .map_err(|e| Refusal::new("payment_required", e))?;
        self.send_records(&request.pubkey, &purchase, &records);
        Ok(Some(purchase))
    }

    /// Record a paid job's end on its purchase and tell the buyer.
    async fn settle_purchase(
        &self,
        buyer: &str,
        purchase: &str,
        result: Result<String, &'static str>,
    ) {
        let Some(seller) = &self.seller else {
            return;
        };
        let records = seller.finish(buyer, purchase, result.as_deref().map_err(|e| *e), now());
        self.send_records(buyer, purchase, &records);
    }

    /// Answer one NIP-X402 record a buyer sealed to this pylon. Only keys
    /// the allowlist admits may buy.
    pub async fn purchase(self: Arc<Self>, event: Event) {
        let Some(seller) = &self.seller else {
            return;
        };
        if event.validate_crypto().is_err() {
            return;
        }
        if let Some(allow) = &self.config.allow
            && !allow.contains(&event.pubkey)
        {
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
        let Some((record, signed)) = paid::open_record(&self.identity, &event) else {
            return;
        };
        if record.provider != self.pubkey() || record.buyer != event.pubkey {
            return;
        }
        let records = seller.handle(&record, &signed, now());
        self.send_records(&record.buyer, &record.purchase, &records);
    }

    /// Seal NIP-X402 records to `buyer` in the purchase's mailbox and queue them.
    fn send_records(&self, buyer: &str, purchase: &str, records: &[serde_json::Value]) {
        for record in records {
            match paid::seal_record(&self.identity, buyer, purchase, record, now()) {
                Ok(event) => {
                    if self.outbound.try_send(event).is_err() {
                        eprintln!("pylon: outbound queue full; dropped a purchase record");
                    }
                }
                Err(e) => eprintln!("pylon: sealing a purchase record: {e}"),
            }
        }
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

/// An attested answer's receipt: the result digest is over the response's
/// JCS bytes (`nostr::att::response_digest`), which a client in another
/// language or JSON library recomputes exactly, and the seal is redone.
fn attested_receipt(receipt: serde_json::Value, response: &serde_json::Value) -> serde_json::Value {
    let Ok(mut parsed) = serde_json::from_value::<receipts::execution::ExecutionReceipt>(receipt.clone()) else {
        return receipt;
    };
    parsed.result_digest = Some(nostr::att::response_digest(response));
    parsed.seal();
    serde_json::to_value(&parsed).unwrap_or(receipt)
}

/// The sealed execution receipt a decision result carries: bound to the
/// request event and the request's digest, naming the served model and its
/// artifact digest.
fn decision_receipt(
    call: &decision::AdmittedCall,
    identity: &crate::decide::Identity,
    latency_ms: u64,
    response: &serde_json::Value,
) -> serde_json::Value {
    use receipts::execution::{ExecutionReceipt, Outcome, Served, Timing};
    let mut receipt = ExecutionReceipt::for_attempt(
        decision::RELAY_TRANSPORT,
        call.body.request.clone(),
        call.body.attempt,
        call.request_digest.clone(),
    );
    receipt.attempt_id = call.attempt_id.clone();
    receipt.requested = Served {
        model: call.body.model.clone(),
        ..Served::default()
    };
    receipt.served = Served {
        model: identity.model.clone(),
        artifact_signature: identity.artifact_digest.clone().unwrap_or_default(),
        ..Served::default()
    };
    receipt.outcome = Outcome::Answered;
    receipt.timing = Timing {
        queued_ms: None,
        latency_ms: Some(latency_ms),
        resolved_at: None,
    };
    receipt.result_digest = Some(nostr::pylon::sha256_hex(response.to_string().as_bytes()));
    receipt.seal();
    serde_json::to_value(&receipt).unwrap_or(serde_json::Value::Null)
}
