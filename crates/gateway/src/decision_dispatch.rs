//! `POST /v1/systemone` as the one decision entry point (#11225): every
//! decision goes to connected Pylons over Nostr first, with no dependency
//! on TypeSafe's Jev API.
//!
//! The chain, each door asked only when every door before it could not
//! answer:
//!
//! 1. **Connected Pylons.** The gateway reads NIP-PYLON beacons (`30200`)
//!    on its relay and keeps the pylons that advertise a decision service
//!    (`<pylon key>:pylon/decision` on the `cj-decision` lane) and have a
//!    free slot. It sends the decision as a NIP-DEC job (`25910`, NIP-44
//!    encrypted, `jev_hosted::RelayExchange`), waits with a deadline,
//!    checks the answer's shape and the served model identity against the
//!    beacon, and fails over to the next pylon.
//! 2. **Our hosted Clef** (`clef_url`), a Psionic `/v1/systemone` over HTTP.
//! 3. **Gemini on Vertex AI** answering the same typed questions with
//!    structured output (`vertex`), on the prepaid Google credit.
//! 4. **Jev** (TypeSafe), optional and last, off unless `jev` is set and
//!    `TYPESAFE_API_KEY` is in the environment.
//!
//! Evidence: every answer names the door (`service.door`: `pylon:<slug>`,
//! `clef`, `vertex`, or `jev`), the pylon key, the served model and its
//! identity, and the latency; one JSON line per decision goes to
//! `<registry>/decisions/YYYY-MM-DD.jsonl` with each attempt. A shadow
//! share (`shadow`, default one in twenty) is asked again at a second door
//! after the answer is sent, and the agreement goes to
//! `<registry>/decisions/shadow-YYYY-MM-DD.jsonl`.
//!
//! The work is free: no payment is asked of the caller or made to a pylon
//! (payments are not V1).

use std::collections::{BTreeSet, HashMap};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use nostr::pylon::{BeaconBook, Freshness, Lane, Status};
use secp256k1::SecretKey;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::sync::Mutex;

use jev::exchange::{Call, Exchange, Failure};

/// The gateway's own name in `service.version`.
const VERSION: &str = concat!("gateway@", env!("CARGO_PKG_VERSION"));

/// How old the beacon book may get before a request refreshes it.
const BOOK_STALE: Duration = Duration::from_secs(30);

/// How long a pylon that failed sits out.
const BENCH: Duration = Duration::from_secs(60);

/// The `decisions` section of `gateway.json`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decisions {
    /// The model names this route answers. A request naming another model
    /// goes to the registry's doors as before.
    #[serde(default = "default_models")]
    pub models: Vec<String>,
    /// The relay pylons are reached on; default the OpenAgents relay.
    #[serde(default)]
    pub relay: Option<String>,
    /// Pylon keys (hex) whose beacons count; empty counts every pylon.
    #[serde(default)]
    pub pylons: Vec<String>,
    /// Served identities a pylon may answer as (`clef-flash`, or
    /// `clef-flash@sha256:…`); empty admits any.
    #[serde(default)]
    pub identities: Vec<String>,
    /// How many pylons one decision may try.
    #[serde(default = "default_pylon_tries")]
    pub pylon_tries: usize,
    /// How long one pylon may take, in milliseconds.
    #[serde(default = "default_pylon_ms")]
    pub pylon_ms: u64,
    /// Our hosted Clef, a Psionic `/v1/systemone` base URL.
    #[serde(default)]
    pub clef_url: Option<String>,
    /// Gemini on Vertex AI with structured output; on by default when the
    /// process has a Google credential.
    #[serde(default = "default_vertex")]
    pub vertex: Option<VertexDoor>,
    /// TypeSafe's Jev as the last door, under `TYPESAFE_API_KEY`. Off by
    /// default.
    #[serde(default)]
    pub jev: bool,
    /// The share of answers asked again at a second door for agreement.
    #[serde(default = "default_shadow")]
    pub shadow: f64,
    /// The whole decision's deadline, in milliseconds.
    #[serde(default = "default_deadline_ms")]
    pub deadline_ms: u64,
}

/// The Vertex door's settings.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexDoor {
    /// The Vertex model id, such as `gemini-3.8-flash`.
    #[serde(default = "default_vertex_model")]
    pub model: String,
}

fn default_models() -> Vec<String> {
    [
        "openagents/decide",
        "jev-latest",
        "jev-1.13.0",
        "typesafe/jev-1.13",
        "clef-flash",
    ]
    .map(String::from)
    .to_vec()
}
fn default_pylon_tries() -> usize {
    2
}
fn default_pylon_ms() -> u64 {
    8_000
}
fn default_vertex() -> Option<VertexDoor> {
    Some(VertexDoor {
        model: default_vertex_model(),
    })
}
fn default_vertex_model() -> String {
    "gemini-3.8-flash".into()
}
fn default_shadow() -> f64 {
    0.05
}
fn default_deadline_ms() -> u64 {
    25_000
}

impl Default for Decisions {
    fn default() -> Self {
        Self {
            models: default_models(),
            relay: None,
            pylons: Vec::new(),
            identities: Vec::new(),
            pylon_tries: default_pylon_tries(),
            pylon_ms: default_pylon_ms(),
            clef_url: None,
            vertex: default_vertex(),
            jev: false,
            shadow: default_shadow(),
            deadline_ms: default_deadline_ms(),
        }
    }
}

/// What one pylon has done lately.
#[derive(Debug, Default, Clone)]
struct Standing {
    answered: u64,
    failed: u64,
    /// Smoothed answer time, milliseconds.
    ewma_ms: f64,
    benched_until: Option<Instant>,
}

/// The beacons held and each pylon's standing.
#[derive(Default)]
struct Book {
    fetched: Option<Instant>,
    beacons: BeaconBook,
    standing: HashMap<String, Standing>,
}

/// One pylon a decision may go to.
#[derive(Debug, Clone)]
struct Candidate {
    provider: String,
    slug: String,
    /// The advertised served identity, `clef-flash@sha256:…`.
    identity: String,
}

impl Candidate {
    fn address(&self) -> String {
        format!("30200:{}:{}", self.provider, self.slug)
    }
    fn model(&self) -> &str {
        self.identity.split('@').next().unwrap_or(&self.identity)
    }
}

/// One door's try at a decision, for the evidence line.
#[derive(Debug, Clone, serde::Serialize)]
struct Attempt {
    door: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pylon: Option<String>,
    outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    ms: u64,
}

/// Which door answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Pylon,
    Clef,
    Vertex,
    Jev,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Pylon => "pylon",
            Self::Clef => "clef",
            Self::Vertex => "vertex",
            Self::Jev => "jev",
        }
    }
}

/// The dispatcher behind `POST /v1/systemone`.
pub struct Dispatch {
    config: Decisions,
    dir: PathBuf,
    secret: SecretKey,
    pubkey: String,
    relay: String,
    book: Mutex<Book>,
    http: reqwest::Client,
    vertex: Option<(
        VertexDoor,
        inference::upstream::google::TokenSource,
        String,
        String,
    )>,
    jev_key: Option<String>,
    count: AtomicU64,
}

/// Why the dispatcher did not open.
pub type Trouble = String;

impl Dispatch {
    /// The dispatcher for `config`, keeping its key and logs under
    /// `<registry>/decisions`. The key that signs decision jobs is made on
    /// first start (`dispatch.key`, mode 0600).
    ///
    /// # Errors
    ///
    /// The directory or the key cannot be made.
    pub fn open(config: Decisions, registry: &std::path::Path) -> Result<Arc<Self>, Trouble> {
        let dir = registry.join("decisions");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let secret = jev_hosted::decision_key(&dir.join("dispatch.key"))?;
        let pubkey = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &secret)
            .x_only_public_key()
            .0
            .to_string();
        let relay = config
            .relay
            .clone()
            .unwrap_or_else(|| pylon::DEFAULT_RELAY.to_owned());
        let vertex = config.vertex.clone().and_then(|door| {
            let token = inference::upstream::google::TokenSource::from_env();
            if !token.present() {
                eprintln!("decisions: the Vertex door is off: no Google credential here");
                return None;
            }
            let settings = inference::upstream::vertex::Config::from_env();
            Some((door, token, settings.project, settings.location))
        });
        let jev_key = if config.jev {
            std::env::var("TYPESAFE_API_KEY")
                .ok()
                .filter(|key| !key.trim().is_empty())
        } else {
            None
        };
        if config.jev && jev_key.is_none() {
            eprintln!("decisions: the Jev door is off: TYPESAFE_API_KEY is not set");
        }
        eprintln!(
            "decisions: POST /v1/systemone for {} → connected pylons on {relay} (dispatch key {pubkey}){}{}{}",
            config.models.join(", "),
            config
                .clef_url
                .as_ref()
                .map(|url| format!(" → hosted Clef at {url}"))
                .unwrap_or_default(),
            vertex
                .as_ref()
                .map(|(door, ..)| format!(" → Vertex {}", door.model))
                .unwrap_or_default(),
            if jev_key.is_some() { " → Jev" } else { "" },
        );
        Ok(Arc::new(Self {
            config,
            dir,
            secret,
            pubkey,
            relay,
            book: Mutex::new(Book::default()),
            http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| e.to_string())?,
            vertex,
            jev_key,
            count: AtomicU64::new(0),
        }))
    }

    /// The key decision jobs are signed with, hex.
    #[must_use]
    pub fn pubkey(&self) -> &str {
        &self.pubkey
    }

    /// Whether this route answers `model`: a listed name, or any Jev name
    /// (`jev-…`, `typesafe/…`) a client pinned before #11225.
    #[must_use]
    pub fn answers(&self, model: &str) -> bool {
        self.config.models.iter().any(|m| m == model)
            || model.starts_with("jev-")
            || model.starts_with("typesafe/")
    }

    /// Answer one `POST /v1/systemone` body, or `None` when its model is
    /// not one this route answers.
    pub async fn handle(self: &Arc<Self>, headers: &HeaderMap, body: &Bytes) -> Option<Response> {
        let envelope: Value = serde_json::from_slice(body).ok()?;
        let model = envelope.get("model")?.as_str()?.to_owned();
        if !self.answers(&model) {
            return None;
        }
        let state = envelope.get("state").cloned().unwrap_or(Value::Null);
        let questions = envelope.get("questions").cloned().unwrap_or(Value::Null);
        if let Err(error) = nostr::decision::check_body(&model, &state, &questions) {
            let code = error.code().unwrap_or("invalid_request");
            return Some(refusal(code, &error.to_string(), &[]));
        }
        let questions = questions.as_object().cloned().unwrap_or_default();
        let request = headers
            .get("idempotency-key")
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty() && v.len() <= 128)
            .map_or_else(random_id, str::to_owned);
        let started = Instant::now();
        let deadline = started + Duration::from_millis(self.config.deadline_ms);
        let (answer, attempts) = self
            .chain(&model, &state, &questions, &request, deadline, None)
            .await;
        let total_ms = millis(started.elapsed());
        let response = match &answer {
            Some((kind, door, response)) => {
                let mut response = response.clone();
                response["latency_ms"] = json!(total_ms);
                eprintln!(
                    "decisions: {} answered as {} in {total_ms} ms{}",
                    door,
                    response["model"].as_str().unwrap_or("?"),
                    if attempts.len() > 1 {
                        format!(
                            " after {}",
                            attempts[..attempts.len() - 1]
                                .iter()
                                .map(|a| format!(
                                    "{} {}",
                                    a.door,
                                    a.code.as_deref().unwrap_or(a.outcome)
                                ))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    } else {
                        String::new()
                    }
                );
                self.record(
                    &model,
                    &questions,
                    &state,
                    Some((*kind, door, &response)),
                    &attempts,
                    total_ms,
                );
                if self.shadow_due() {
                    let this = Arc::clone(self);
                    let (kind, door) = (*kind, door.clone());
                    let first = response.clone();
                    tokio::spawn(async move {
                        this.shadow(&model, &state, &questions, kind, &door, &first)
                            .await;
                    });
                }
                let mut out = (
                    axum::http::StatusCode::OK,
                    [("content-type", "application/json")],
                    serde_json::to_vec(&response).unwrap_or_default(),
                )
                    .into_response();
                if let Ok(value) = door.parse() {
                    out.headers_mut().insert("x-decision-door", value);
                }
                out
            }
            None => {
                eprintln!(
                    "decisions: no door answered in {total_ms} ms ({})",
                    attempts
                        .iter()
                        .map(|a| format!("{} {}", a.door, a.code.as_deref().unwrap_or(a.outcome)))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                self.record(&model, &questions, &state, None, &attempts, total_ms);
                let code = attempts
                    .iter()
                    .find(|a| a.outcome == "refused")
                    .and_then(|a| a.code.clone())
                    .filter(|code| {
                        matches!(
                            code.as_str(),
                            "invalid_request" | "too_many_questions" | "too_many_options"
                        )
                    })
                    .unwrap_or_else(|| "unavailable".into());
                refusal(
                    &code,
                    "No decision door could answer right now. Try again shortly.",
                    &attempts,
                )
            }
        };
        Some(response)
    }

    /// Ask each door in order until one answers. `skip` leaves one door
    /// out (the shadow's second opinion).
    async fn chain(
        &self,
        model: &str,
        state: &Value,
        questions: &Map<String, Value>,
        request: &str,
        deadline: Instant,
        skip: Option<&str>,
    ) -> (Option<(Kind, String, Value)>, Vec<Attempt>) {
        let mut attempts = Vec::new();
        // 1. Connected pylons.
        let candidates = self.candidates(skip).await;
        for candidate in candidates.into_iter().take(self.config.pylon_tries.max(1)) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left < Duration::from_millis(500) {
                break;
            }
            let budget = left.min(Duration::from_millis(self.config.pylon_ms));
            let door = format!("pylon:{}", candidate.slug);
            let t = Instant::now();
            let result = self
                .ask_pylon(&candidate, model, state, questions, request, budget)
                .await;
            let ms = millis(t.elapsed());
            match result {
                Ok(response) => {
                    self.standing(&candidate, Some(ms)).await;
                    attempts.push(Attempt {
                        door: door.clone(),
                        pylon: Some(candidate.provider.clone()),
                        outcome: "answered",
                        code: None,
                        ms,
                    });
                    return (Some((Kind::Pylon, door, response)), attempts);
                }
                Err((outcome, code)) => {
                    self.standing(&candidate, None).await;
                    eprintln!(
                        "decisions: pylon {} ({}) {outcome}: {code}",
                        candidate.slug,
                        &candidate.provider[..12]
                    );
                    attempts.push(Attempt {
                        door,
                        pylon: Some(candidate.provider.clone()),
                        outcome,
                        code: Some(code),
                        ms,
                    });
                }
            }
        }
        // 2. Our hosted Clef.
        if let Some(url) = &self.config.clef_url
            && skip != Some("clef")
        {
            let t = Instant::now();
            let left = deadline.saturating_duration_since(Instant::now());
            let result = self
                .ask_http(url, None, "clef-flash", state, questions, left)
                .await;
            if let Some(found) =
                self.settle("clef", Kind::Clef, result, questions, t, &mut attempts)
            {
                return (Some(found), attempts);
            }
        }
        // 3. Gemini on Vertex AI, structured output.
        if self.vertex.is_some() && skip != Some("vertex") {
            let t = Instant::now();
            let left = deadline.saturating_duration_since(Instant::now());
            let result = self.ask_vertex(state, questions, left).await;
            if let Some(found) =
                self.settle("vertex", Kind::Vertex, result, questions, t, &mut attempts)
            {
                return (Some(found), attempts);
            }
        }
        // 4. Jev, optional and last.
        if let Some(key) = &self.jev_key
            && skip != Some("jev")
        {
            let t = Instant::now();
            let left = deadline.saturating_duration_since(Instant::now());
            let result = self
                .ask_http(
                    "https://api.typesafe.ai",
                    Some(key),
                    "jev-latest",
                    state,
                    questions,
                    left,
                )
                .await;
            if let Some(found) = self.settle("jev", Kind::Jev, result, questions, t, &mut attempts)
            {
                return (Some(found), attempts);
            }
        }
        (None, attempts)
    }

    /// Check an HTTP door's answer and add it to the attempts.
    fn settle(
        &self,
        door: &str,
        kind: Kind,
        result: Result<Value, (&'static str, String)>,
        questions: &Map<String, Value>,
        started: Instant,
        attempts: &mut Vec<Attempt>,
    ) -> Option<(Kind, String, Value)> {
        let ms = millis(started.elapsed());
        let checked = result.and_then(|mut response| {
            check_answers(questions, &response).map_err(|why| ("invalid", why))?;
            let served = response["model"].as_str().unwrap_or(door).to_owned();
            response["service"] = json!({"door": door, "version": VERSION, "model": served});
            Ok(response)
        });
        match checked {
            Ok(response) => {
                attempts.push(Attempt {
                    door: door.into(),
                    pylon: None,
                    outcome: "answered",
                    code: None,
                    ms,
                });
                Some((kind, door.to_owned(), response))
            }
            Err((outcome, code)) => {
                eprintln!("decisions: {door} {outcome}: {code}");
                attempts.push(Attempt {
                    door: door.into(),
                    pylon: None,
                    outcome,
                    code: Some(code),
                    ms,
                });
                None
            }
        }
    }

    /// The pylons a decision may go to now, best first.
    async fn candidates(&self, skip: Option<&str>) -> Vec<Candidate> {
        self.refresh_book().await;
        let book = self.book.lock().await;
        let now = unix_now();
        let allowed: BTreeSet<&str> = self.config.pylons.iter().map(String::as_str).collect();
        let mut found: Vec<(Candidate, Standing, u32)> = book
            .beacons
            .iter()
            .filter_map(|(_, beacon)| {
                if nostr::pylon::freshness(beacon, now) != Freshness::Fresh
                    || beacon.status != Status::Online
                    || beacon.slots.free == 0
                    || (!allowed.is_empty() && !allowed.contains(beacon.provider.as_str()))
                {
                    return None;
                }
                let service = beacon.serves(Lane::CjDecision)?;
                let candidate = Candidate {
                    provider: beacon.provider.clone(),
                    slug: beacon.pylon.clone(),
                    identity: service.model.clone(),
                };
                if !self.identity_admitted(&candidate.identity)
                    || skip.is_some_and(|skip| skip == format!("pylon:{}", candidate.slug))
                {
                    return None;
                }
                let standing = book
                    .standing
                    .get(&candidate.address())
                    .cloned()
                    .unwrap_or_default();
                if standing
                    .benched_until
                    .is_some_and(|until| until > Instant::now())
                {
                    return None;
                }
                Some((candidate, standing, beacon.slots.free))
            })
            .collect();
        found.sort_by(|a, b| {
            let rate = |s: &Standing| s.failed as f64 / (s.answered + s.failed + 1) as f64;
            rate(&a.1)
                .total_cmp(&rate(&b.1))
                .then(a.1.ewma_ms.total_cmp(&b.1.ewma_ms))
                .then(b.2.cmp(&a.2))
        });
        found.into_iter().map(|(c, ..)| c).collect()
    }

    fn identity_admitted(&self, identity: &str) -> bool {
        self.config.identities.is_empty()
            || self.config.identities.iter().any(|admitted| {
                identity == admitted
                    || (!admitted.contains('@')
                        && identity.split('@').next() == Some(admitted.as_str()))
            })
    }

    /// Read the relay's beacons when the book is older than [`BOOK_STALE`].
    async fn refresh_book(&self) {
        {
            let book = self.book.lock().await;
            if book.fetched.is_some_and(|at| at.elapsed() < BOOK_STALE) {
                return;
            }
        }
        let fetched = async {
            let me = pylon::identity::Identity::from_secret(self.secret)?;
            let mut conn = pylon::relay::connect(&self.relay, &me, Duration::from_secs(10)).await?;
            let authors = (!self.config.pylons.is_empty()).then_some(self.config.pylons.as_slice());
            let book = pylon::client::beacons(&mut conn, authors).await;
            let _ = conn.close().await;
            book
        };
        match tokio::time::timeout(Duration::from_secs(4), fetched).await {
            Ok(Ok(beacons)) => {
                let mut book = self.book.lock().await;
                book.beacons = beacons;
                book.fetched = Some(Instant::now());
            }
            Ok(Err(why)) => eprintln!("decisions: reading beacons on {}: {why}", self.relay),
            Err(_) => eprintln!("decisions: reading beacons on {} timed out", self.relay),
        }
    }

    /// Note how a pylon did: an answer time, or a failure that benches it.
    async fn standing(&self, candidate: &Candidate, answered_ms: Option<u64>) {
        let mut book = self.book.lock().await;
        let standing = book.standing.entry(candidate.address()).or_default();
        match answered_ms {
            Some(ms) => {
                standing.answered += 1;
                standing.ewma_ms = if standing.answered == 1 {
                    ms as f64
                } else {
                    0.8 * standing.ewma_ms + 0.2 * ms as f64
                };
                standing.benched_until = None;
            }
            None => {
                standing.failed += 1;
                standing.benched_until = Some(Instant::now() + BENCH);
            }
        }
    }

    /// One NIP-DEC job to one pylon, its answer checked.
    async fn ask_pylon(
        &self,
        candidate: &Candidate,
        model: &str,
        state: &Value,
        questions: &Map<String, Value>,
        request: &str,
        budget: Duration,
    ) -> Result<Value, (&'static str, String)> {
        let exchange =
            jev_hosted::RelayExchange::new(&self.relay, &candidate.provider, self.secret)
                .map_err(|why| ("failed", why))?;
        let body = json!({"model": model, "state": state, "questions": questions});
        let call = Call {
            method: "POST".into(),
            path: "/v1/systemone".into(),
            body: Some(serde_json::to_vec(&body).unwrap_or_default()),
            idempotency_key: Some(format!("{request}-{}", &candidate.provider[..8])),
            attempt: 1,
            timeout: budget,
        };
        let reply = match tokio::time::timeout(
            budget + Duration::from_millis(250),
            exchange.exchange(call),
        )
        .await
        {
            Err(_) | Ok(Err(Failure::Timeout)) => return Err(("failed", "timeout".into())),
            Ok(Err(Failure::Unreachable(why))) => return Err(("failed", why)),
            Ok(Ok(reply)) => reply,
        };
        let response: Value = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
        if reply.status != 200 {
            let code = response["error"]["code"].as_str().map_or_else(
                || nostr::decision::code_for_http_status(reply.status).to_owned(),
                str::to_owned,
            );
            return Err(("refused", code));
        }
        check_answers(questions, &response).map_err(|why| ("invalid", why))?;
        let served = response["model"].as_str().unwrap_or_default().to_owned();
        if served != candidate.model() {
            return Err((
                "invalid",
                format!(
                    "answered as `{served}`, but the beacon advertises `{}`",
                    candidate.identity
                ),
            ));
        }
        if let Some(identity) = response["service"]["identity"].as_str()
            && identity != candidate.identity
        {
            return Err((
                "invalid",
                format!(
                    "named identity `{identity}`, but the beacon advertises `{}`",
                    candidate.identity
                ),
            ));
        }
        let mut response = response;
        let pylon_service = response["service"].take();
        response["service"] = json!({
            "door": format!("pylon:{}", candidate.slug),
            "version": VERSION,
            "pylon": candidate.address(),
            "provider": candidate.provider,
            "identity": candidate.identity,
            "model": served,
            "upstream": pylon_service,
        });
        Ok(response)
    }

    /// A System One door over HTTP (our hosted Clef, or Jev).
    async fn ask_http(
        &self,
        base: &str,
        key: Option<&str>,
        model: &str,
        state: &Value,
        questions: &Map<String, Value>,
        budget: Duration,
    ) -> Result<Value, (&'static str, String)> {
        if budget < Duration::from_millis(300) {
            return Err(("failed", "no time left".into()));
        }
        let mut call = self
            .http
            .post(format!("{}/v1/systemone", base.trim_end_matches('/')))
            .timeout(budget)
            .json(&json!({"model": model, "state": state, "questions": questions}));
        if let Some(key) = key {
            call = call.bearer_auth(key);
        }
        let response = call.send().await.map_err(|e| {
            (
                "failed",
                if e.is_timeout() {
                    "timeout".to_owned()
                } else {
                    "unreachable".to_owned()
                },
            )
        })?;
        let status = response.status().as_u16();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        if status != 200 {
            let code = body["error"]["code"].as_str().map_or_else(
                || nostr::decision::code_for_http_status(status).to_owned(),
                str::to_owned,
            );
            return Err(("refused", code));
        }
        Ok(body)
    }

    /// Gemini on Vertex AI answering the typed questions with structured
    /// output, turned into NIP-DEC answers.
    async fn ask_vertex(
        &self,
        state: &Value,
        questions: &Map<String, Value>,
        budget: Duration,
    ) -> Result<Value, (&'static str, String)> {
        let Some((door, token, project, location)) = &self.vertex else {
            return Err(("failed", "off".into()));
        };
        if budget < Duration::from_millis(500) {
            return Err(("failed", "no time left".into()));
        }
        let token = token.token().await.map_err(|why| ("failed", why))?;
        let host = if location == "global" {
            "https://aiplatform.googleapis.com".to_owned()
        } else {
            format!("https://{location}-aiplatform.googleapis.com")
        };
        let host = std::env::var("VERTEX_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map_or(host, |v| v.trim_end_matches('/').to_owned());
        let url = format!(
            "{host}/v1/projects/{project}/locations/{location}/publishers/google/models/{}:generateContent",
            door.model
        );
        let response = self
            .http
            .post(url)
            .timeout(budget)
            .bearer_auth(token.expose())
            .json(&vertex_body(state, questions))
            .send()
            .await
            .map_err(|e| {
                (
                    "failed",
                    if e.is_timeout() {
                        "timeout".to_owned()
                    } else {
                        "unreachable".to_owned()
                    },
                )
            })?;
        let status = response.status().as_u16();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        if status != 200 {
            return Err(("refused", format!("vertex_{status}")));
        }
        let text: String = body["candidates"][0]["content"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|part| part["thought"] != true)
            .filter_map(|part| part["text"].as_str())
            .collect();
        let raw: Value = serde_json::from_str(&text)
            .map_err(|_| ("invalid", "the answer is not JSON".to_owned()))?;
        let answers = vertex_answers(questions, &raw).map_err(|why| ("invalid", why))?;
        Ok(json!({
            "model": door.model,
            "answers": answers,
            "usage": {
                "input_tokens": body["usageMetadata"]["promptTokenCount"],
                "output_tokens": body["usageMetadata"]["candidatesTokenCount"],
            },
        }))
    }

    fn shadow_due(&self) -> bool {
        if self.config.shadow <= 0.0 {
            return false;
        }
        let every = (1.0 / self.config.shadow.min(1.0)).round().max(1.0) as u64;
        self.count.fetch_add(1, Ordering::Relaxed) % every == 0
    }

    /// Ask the same decision at a second door and record how far the two
    /// answers agree.
    async fn shadow(
        &self,
        model: &str,
        state: &Value,
        questions: &Map<String, Value>,
        kind: Kind,
        door: &str,
        first: &Value,
    ) {
        let skip = match kind {
            Kind::Pylon => door.to_owned(),
            other => other.name().to_owned(),
        };
        let deadline = Instant::now() + Duration::from_millis(self.config.deadline_ms);
        let (second, _) = self
            .chain(model, state, questions, &random_id(), deadline, Some(&skip))
            .await;
        let Some((_, second_door, second)) = second else {
            return;
        };
        let (agreed, asked, max_dp) = agreement(questions, &first["answers"], &second["answers"]);
        eprintln!(
            "decisions: shadow {second_door} agrees with {door} on {agreed}/{asked} questions, max |Δp| {max_dp:.3}"
        );
        let line = json!({
            "at": unix_now(),
            "first": {"door": door, "model": first["model"]},
            "second": {"door": second_door, "model": second["model"]},
            "questions": asked,
            "agreed": agreed,
            "max_dp": max_dp,
        });
        self.append(&format!("shadow-{}.jsonl", day()), &line);
    }

    /// One evidence line per decision.
    fn record(
        &self,
        model: &str,
        questions: &Map<String, Value>,
        state: &Value,
        answer: Option<(Kind, &String, &Value)>,
        attempts: &[Attempt],
        total_ms: u64,
    ) {
        let line = json!({
            "at": unix_now(),
            "model_asked": model,
            "outcome": if answer.is_some() { "answered" } else { "unavailable" },
            "door": answer.map(|(_, door, _)| door.clone()),
            "kind": answer.map(|(kind, ..)| kind.name()),
            "model": answer.map(|(.., response)| response["model"].clone()),
            "identity": answer.map(|(.., response)| response["service"]["identity"].clone()),
            "pylon": answer.and_then(|(.., response)| response["service"]["provider"].as_str().map(str::to_owned)),
            "ms": total_ms,
            "questions": questions.len(),
            "state_bytes": state.to_string().len(),
            "attempts": attempts,
        });
        self.append(&format!("{}.jsonl", day()), &line);
    }

    /// Append one line off the async runtime: the registry can sit on a
    /// network disk, and a stalled write must not stall the gateway.
    fn append(&self, name: &str, line: &Value) {
        let path = self.dir.join(name);
        let text = line.to_string();
        tokio::task::spawn_blocking(move || {
            let result = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .and_then(|mut file| writeln!(file, "{text}"));
            if let Err(e) = result {
                eprintln!("decisions: {}: {e}", path.display());
            }
        });
    }
}

/// The Vertex `generateContent` body: the state and questions as JSON, a
/// response schema with one probability per option, and low thinking
/// (Gemini 3.8 Flash refuses `minimal`).
fn vertex_body(state: &Value, questions: &Map<String, Value>) -> Value {
    let mut properties = Map::new();
    let mut asked = Map::new();
    for (id, question) in questions {
        let kind = question["type"].as_str().unwrap_or("noul");
        let schema = match kind {
            "choice" => {
                let options: Vec<String> = question["criteria"]
                    .as_object()
                    .map(|o| o.keys().cloned().collect())
                    .unwrap_or_default();
                probabilities_schema(&options)
            }
            "score" => {
                let levels = question["criteria"].as_array().map_or(0, Vec::len);
                probabilities_schema(&(0..levels).map(|i| i.to_string()).collect::<Vec<_>>())
            }
            _ => {
                json!({"type": "object", "properties": {"p_yes": {"type": "number"}}, "required": ["p_yes"]})
            }
        };
        properties.insert(id.clone(), schema);
        asked.insert(
            id.clone(),
            json!({"type": kind, "instructions": question.get("instructions"), "criteria": question.get("criteria")}),
        );
    }
    let ids: Vec<&String> = questions.keys().collect();
    let prompt = json!({"state": state, "questions": asked});
    json!({
        "systemInstruction": {"parts": [{"text":
            "You are a decision model. Read the state, then answer every typed question about it \
             with calibrated probabilities. A noul question asks yes or no: give p_yes, the \
             probability the answer is yes, read with its instructions and its true and false \
             descriptions. A choice question: give a probability for every option id, read with \
             each option's description. A score question: give a probability for every level \
             index, level 0 first. The probabilities of one question sum to 1. Questions do not \
             read each other's answers. Answer only with JSON matching the schema."}]},
        "contents": [{"role": "user", "parts": [{"text": prompt.to_string()}]}],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": {"type": "object", "properties": properties, "required": ids},
            "temperature": 0,
            "thinkingConfig": {"thinkingLevel": "low"},
        },
    })
}

fn probabilities_schema(keys: &[String]) -> Value {
    let properties: Map<String, Value> = keys
        .iter()
        .map(|key| (key.clone(), json!({"type": "number"})))
        .collect();
    json!({
        "type": "object",
        "properties": {"probabilities": {"type": "object", "properties": properties, "required": keys}},
        "required": ["probabilities"],
    })
}

/// Turn Gemini's structured answer into NIP-DEC answers: each question's
/// probabilities clamped to `[0, 1]` and normalized to sum to one.
fn vertex_answers(questions: &Map<String, Value>, raw: &Value) -> Result<Value, String> {
    let mut answers = Map::new();
    for (id, question) in questions {
        let got = &raw[id];
        let answer = match question["type"].as_str().unwrap_or("noul") {
            "choice" => {
                let options: Vec<String> = question["criteria"]
                    .as_object()
                    .map(|o| o.keys().cloned().collect())
                    .unwrap_or_default();
                let p = normalized(&options, &got["probabilities"]);
                let (choice, confidence) = options
                    .iter()
                    .zip(&p)
                    .fold(
                        None::<(&String, f64)>,
                        |best, (option, &value)| match best {
                            Some((b, bv)) if bv > value || (bv == value && b <= option) => {
                                Some((b, bv))
                            }
                            _ => Some((option, value)),
                        },
                    )
                    .ok_or_else(|| format!("question `{id}` has no options"))?;
                let probabilities: Map<String, Value> = options
                    .iter()
                    .zip(&p)
                    .map(|(option, value)| (option.clone(), json!(value)))
                    .collect();
                json!({"type": "choice", "choice": choice, "confidence": confidence, "probabilities": probabilities})
            }
            "score" => {
                let levels = question["criteria"].as_array().cloned().unwrap_or_default();
                let keys: Vec<String> = (0..levels.len()).map(|i| i.to_string()).collect();
                let p = normalized(&keys, &got["probabilities"]);
                let score: f64 = p.iter().enumerate().map(|(i, v)| i as f64 * v).sum();
                let confidence = p.iter().copied().fold(0.0, f64::max);
                let probabilities: Map<String, Value> = keys
                    .iter()
                    .zip(&p)
                    .map(|(k, v)| (k.clone(), json!(v)))
                    .collect();
                let legend: Map<String, Value> = keys
                    .iter()
                    .zip(levels)
                    .map(|(k, level)| (k.clone(), level))
                    .collect();
                json!({"type": "score", "score": score, "confidence": confidence,
                       "legend": legend, "probabilities": probabilities})
            }
            _ => {
                let p = got["p_yes"]
                    .as_f64()
                    .filter(|p| p.is_finite())
                    .ok_or_else(|| format!("question `{id}` has no p_yes"))?;
                json!({"type": "noul", "noul": p.clamp(0.0, 1.0)})
            }
        };
        answers.insert(id.clone(), answer);
    }
    Ok(Value::Object(answers))
}

fn normalized(keys: &[String], got: &Value) -> Vec<f64> {
    let raw: Vec<f64> = keys
        .iter()
        .map(|key| {
            got[key]
                .as_f64()
                .filter(|p| p.is_finite())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0)
        })
        .collect();
    let sum: f64 = raw.iter().sum();
    if sum <= 0.0 {
        let n = keys.len().max(1) as f64;
        return vec![1.0 / n; keys.len()];
    }
    raw.iter().map(|p| p / sum).collect()
}

/// Whether `response` answers exactly `questions` in NIP-DEC's shape: every
/// question answered with its own type, probabilities finite in `[0, 1]`,
/// a choice's and a score's probabilities naming exactly the options or
/// levels asked and summing to one.
///
/// # Errors
///
/// What is wrong, as a short sentence.
pub fn check_answers(questions: &Map<String, Value>, response: &Value) -> Result<(), String> {
    let answers = response["answers"]
        .as_object()
        .ok_or("the answer has no `answers` object")?;
    if answers.len() != questions.len() {
        return Err(format!(
            "{} answers for {} questions",
            answers.len(),
            questions.len()
        ));
    }
    let unit = |v: &Value| {
        v.as_f64()
            .is_some_and(|p| p.is_finite() && (-1e-9..=1.0 + 1e-9).contains(&p))
    };
    for (id, question) in questions {
        let answer = answers
            .get(id)
            .ok_or_else(|| format!("question `{id}` is not answered"))?;
        let kind = question["type"].as_str().unwrap_or("noul");
        if answer["type"].as_str() != Some(kind) {
            return Err(format!(
                "question `{id}` is a {kind}, answered as another type"
            ));
        }
        let keys: Vec<String> = match kind {
            "noul" => {
                if !unit(&answer["noul"]) {
                    return Err(format!("question `{id}`'s noul is not a probability"));
                }
                continue;
            }
            "choice" => question["criteria"]
                .as_object()
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default(),
            "score" => (0..question["criteria"].as_array().map_or(0, Vec::len))
                .map(|i| i.to_string())
                .collect(),
            other => return Err(format!("question `{id}` has unknown type `{other}`")),
        };
        let probabilities = answer["probabilities"]
            .as_object()
            .ok_or_else(|| format!("question `{id}` has no probabilities"))?;
        let named: BTreeSet<&String> = probabilities.keys().collect();
        let asked: BTreeSet<&String> = keys.iter().collect();
        if named != asked {
            return Err(format!(
                "question `{id}`'s probabilities name other options"
            ));
        }
        if !probabilities.values().all(unit) {
            return Err(format!("question `{id}` has a probability out of range"));
        }
        let sum: f64 = probabilities.values().filter_map(Value::as_f64).sum();
        if (sum - 1.0).abs() > 1e-3 {
            return Err(format!("question `{id}`'s probabilities sum to {sum:.4}"));
        }
        if kind == "choice"
            && !answer["choice"]
                .as_str()
                .is_some_and(|choice| probabilities.contains_key(choice))
        {
            return Err(format!("question `{id}`'s choice is not an option"));
        }
    }
    Ok(())
}

/// How far two answers to the same questions agree: the questions whose
/// pick agrees (a choice's option, a score's most likely level, a noul's
/// side of one half), out of those asked, and the largest probability gap.
fn agreement(questions: &Map<String, Value>, a: &Value, b: &Value) -> (usize, usize, f64) {
    let mut agreed = 0;
    let mut max_dp: f64 = 0.0;
    for (id, question) in questions {
        let (x, y) = (&a[id], &b[id]);
        match question["type"].as_str().unwrap_or("noul") {
            "noul" => {
                let (p, q) = (
                    x["noul"].as_f64().unwrap_or(0.5),
                    y["noul"].as_f64().unwrap_or(0.5),
                );
                if (p >= 0.5) == (q >= 0.5) {
                    agreed += 1;
                }
                max_dp = max_dp.max((p - q).abs());
            }
            _ => {
                let pick = |v: &Value| {
                    v["probabilities"].as_object().and_then(|o| {
                        o.iter()
                            .filter_map(|(k, p)| p.as_f64().map(|p| (k.clone(), p)))
                            .max_by(|l, r| l.1.total_cmp(&r.1).then(r.0.cmp(&l.0)))
                            .map(|(k, _)| k)
                    })
                };
                if pick(x).is_some() && pick(x) == pick(y) {
                    agreed += 1;
                }
                if let (Some(px), Some(py)) = (
                    x["probabilities"].as_object(),
                    y["probabilities"].as_object(),
                ) {
                    for (k, p) in px {
                        let q = py.get(k).and_then(Value::as_f64).unwrap_or(0.0);
                        max_dp = max_dp.max((p.as_f64().unwrap_or(0.0) - q).abs());
                    }
                }
            }
        }
    }
    (agreed, questions.len(), max_dp)
}

fn refusal(code: &str, message: &str, attempts: &[Attempt]) -> Response {
    let status = nostr::decision::http_status(code);
    let mut body = json!({"error": {"code": code, "message": message}});
    if !attempts.is_empty() {
        body["error"]["attempts"] = json!(attempts);
    }
    (
        axum::http::StatusCode::from_u16(status).unwrap_or(axum::http::StatusCode::BAD_GATEWAY),
        [("content-type", "application/json")],
        serde_json::to_vec(&body).unwrap_or_default(),
    )
        .into_response()
}

fn random_id() -> String {
    let bytes: [u8; 12] = secp256k1::rand::random();
    format!("dec-{}", hex::encode(bytes))
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Today's UTC date, `YYYY-MM-DD`.
fn day() -> String {
    let days = unix_now() / 86_400;
    // Civil from days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", if m <= 2 { y + 1 } else { y })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn questions() -> Map<String, Value> {
        json!({
            "topic": {"type": "choice", "instructions": "Which team?", "criteria": {"billing": "money", "technical": "bugs"}},
            "refund": {"type": "noul", "instructions": "Money back?"},
            "sev": {"type": "score", "criteria": ["low", "mid", "high"]},
        })
        .as_object()
        .cloned()
        .unwrap()
    }

    #[test]
    fn a_clef_answer_passes_and_a_wrong_shape_does_not() {
        let good = json!({"model": "clef-flash", "answers": {
            "topic": {"type": "choice", "choice": "billing", "confidence": 0.98, "probabilities": {"billing": 0.98, "technical": 0.02}},
            "refund": {"type": "noul", "noul": 0.23},
            "sev": {"type": "score", "score": 1.86, "confidence": 0.88, "legend": {"0": "low", "1": "mid", "2": "high"}, "probabilities": {"0": 0.02, "1": 0.1, "2": 0.88}},
        }});
        check_answers(&questions(), &good).unwrap();
        let mut missing = good.clone();
        missing["answers"].as_object_mut().unwrap().remove("refund");
        assert!(check_answers(&questions(), &missing).is_err());
        let mut other = good.clone();
        other["answers"]["topic"]["probabilities"] = json!({"billing": 0.5, "sales": 0.5});
        assert!(check_answers(&questions(), &other).is_err());
        let mut sum = good.clone();
        sum["answers"]["sev"]["probabilities"]["2"] = json!(0.5);
        assert!(check_answers(&questions(), &sum).is_err());
        let mut out = good;
        out["answers"]["refund"]["noul"] = json!(1.5);
        assert!(check_answers(&questions(), &out).is_err());
    }

    #[test]
    fn gemini_structured_output_becomes_nip_dec_answers() {
        let raw = json!({
            "topic": {"probabilities": {"billing": 0.8, "technical": 0.4}},
            "refund": {"p_yes": 0.7},
            "sev": {"probabilities": {"0": 0.1, "1": 0.1, "2": 0.8}},
        });
        let answers = vertex_answers(&questions(), &raw).unwrap();
        let response = json!({"model": "gemini-3.8-flash", "answers": answers});
        check_answers(&questions(), &response).unwrap();
        assert_eq!(response["answers"]["topic"]["choice"], "billing");
        let p = response["answers"]["topic"]["probabilities"]["billing"]
            .as_f64()
            .unwrap();
        assert!((p - 0.8 / 1.2).abs() < 1e-9);
        let score = response["answers"]["sev"]["score"].as_f64().unwrap();
        assert!((score - 1.7).abs() < 1e-9);
        assert_eq!(response["answers"]["sev"]["legend"]["2"], "high");
        let body = vertex_body(&json!("I was billed twice"), &questions());
        assert_eq!(
            body["generationConfig"]["responseJsonSchema"]["properties"]["topic"]["properties"]["probabilities"]
                ["required"],
            json!(["billing", "technical"])
        );
    }

    #[test]
    fn agreement_counts_picks_and_the_largest_gap() {
        let a = json!({"topic": {"probabilities": {"billing": 0.9, "technical": 0.1}}, "refund": {"noul": 0.2},
                       "sev": {"probabilities": {"0": 0.1, "1": 0.2, "2": 0.7}}});
        let b = json!({"topic": {"probabilities": {"billing": 0.6, "technical": 0.4}}, "refund": {"noul": 0.7},
                       "sev": {"probabilities": {"0": 0.1, "1": 0.1, "2": 0.8}}});
        let (agreed, asked, max_dp) = agreement(&questions(), &a, &b);
        assert_eq!((agreed, asked), (2, 3));
        assert!((max_dp - 0.5).abs() < 1e-9);
    }

    #[test]
    fn the_date_is_civil() {
        assert_eq!(day().len(), 10);
    }
}
