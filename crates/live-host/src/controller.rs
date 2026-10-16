//! The NIP-LIVE session controller. It admits plans and participants under
//! exact revisions, mints scoped credentials through the host broker, holds
//! input and speaking floors bound to the media epoch, retains captures and
//! transcripts, and applies device input only after a separate admission
//! with fresh observation preconditions. It never replays input or speech.
use crate::{
    Result,
    media::{Applied, Broker, Device, Grant, InputGuard, Revocation, Source, token_digest},
};
use nostr::{
    contracts::{self, digest_bytes},
    domain::{Event, RelaySigner},
    live,
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use workspace_host::artifact::{Artifacts, token};
mod input;
mod readers;
mod retention;
#[cfg(unix)]
mod transport;
pub use input::InputGrant;

pub const MAX_PARTICIPANTS: usize = 64;
pub const MAX_SESSIONS: usize = 64;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub owners: BTreeSet<String>,
    /// Tasks that already exist on this host.
    pub tasks: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prior {
    fingerprint: String,
    result: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    tracks: Vec<String>,
    rights: Vec<String>,
    expires_at: u64,
    consent: Vec<Value>,
    /// Credential digests only; tokens never reach controller state.
    credentials: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Floor {
    nonce: String,
    participant: String,
    track: String,
    until: u64,
    epoch: u64,
    lease: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    id: String,
    task: String,
    owner: String,
    plan: Value,
    body: Value,
    revision: u64,
    state: String,
    epoch: u64,
    members: BTreeMap<String, Member>,
    input_floors: BTreeMap<String, Floor>,
    speak_floors: BTreeMap<String, Floor>,
    observations: BTreeMap<String, Value>,
    transcripts: BTreeMap<String, Vec<Value>>,
    run: Vec<Value>,
    unresolved: Vec<String>,
    disclosed_bytes: u64,
}

impl Session {
    fn track(&self, id: &str) -> Option<&Value> {
        self.body["tracks"]
            .as_array()?
            .iter()
            .find(|t| t["id"] == id)
    }
    fn consent_policy(&self, store: &Artifacts) -> Result<Value> {
        store.json(&self.body["consent_policy"])
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    policy: Policy,
    sessions: BTreeMap<String, Session>,
    /// Every admitted plan digest, current or superseded, to its session.
    plans: BTreeMap<String, String>,
    priors: BTreeMap<String, Prior>,
    signers: BTreeMap<String, BTreeSet<String>>,
    declarations: BTreeMap<String, Value>,
    #[serde(default)]
    input_rules: input::Book,
    #[serde(default)]
    known: BTreeMap<String, BTreeSet<String>>,
}

/// A signed request, its exact artifacts, and further signed declarations
/// (for example a participant's consent).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub event: Event,
    #[serde(default)]
    pub artifacts: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    pub declarations: Vec<Event>,
}

/// Host-owned adapters behind explicit roles.
pub struct Adapters {
    pub broker: Box<dyn Broker>,
    pub source: Box<dyn Source>,
    pub device: Box<dyn Device>,
}

pub struct Controller {
    store: Artifacts,
    signer: RelaySigner,
    secret: SecretKey,
    adapters: RefCell<Adapters>,
    delivery: RefCell<BTreeMap<(String, String), String>>,
    reader: RefCell<Option<readers::ReadScope>>,
    pub(crate) capture_clocks: RefCell<BTreeMap<String, std::time::Instant>>,
}

fn refuse(code: &str, detail: &str) -> String {
    format!("{code}: {detail}")
}

/// The shared refusal code at the front of an error, or `unavailable`.
pub fn code(error: &str) -> &'static str {
    const CODES: &[&str] = &[
        "malformed",
        "unsupported_version",
        "unsupported_feature",
        "not_admitted",
        "unavailable",
        "content_unavailable",
        "identity_mismatch",
        "incompatible",
        "revoked",
        "stale",
        "cannot_enforce",
        "limit_exceeded",
        "idempotency_conflict",
        "conflict",
    ];
    let head = error.split(':').next().unwrap_or_default().trim();
    CODES
        .iter()
        .find(|c| **c == head)
        .copied()
        .unwrap_or("unavailable")
}

fn contract(e: contracts::ContractError) -> String {
    format!("{}: {}", e.code.as_str(), e)
}

fn digest_of(reference: &Value) -> &str {
    reference["digest"].as_str().unwrap_or_default()
}

fn same(a: &Value, b: &Value) -> bool {
    !digest_of(a).is_empty() && digest_of(a) == digest_of(b)
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

impl Controller {
    pub fn initialize(root: PathBuf, secret: SecretKey, policy: Policy) -> Result<()> {
        let store = Artifacts::new(root)?;
        if store.root().join("state.json").exists() {
            return Err("conflict: controller state already exists".into());
        }
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        store.save(
            &State {
                policy,
                ..State::default()
            },
            &signer,
        )
    }
    pub fn open(root: PathBuf, secret: SecretKey, adapters: Adapters) -> Result<Self> {
        let store = Artifacts::new(root)?;
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        let controller = Self {
            store,
            signer,
            secret,
            adapters: RefCell::new(adapters),
            delivery: RefCell::new(BTreeMap::new()),
            reader: RefCell::new(None),
            capture_clocks: RefCell::new(BTreeMap::new()),
        };
        controller.state()?;
        Ok(controller)
    }
    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }
    pub fn store(&self) -> &Artifacts {
        &self.store
    }
    pub fn with_adapters<T>(&self, f: impl FnOnce(&mut Adapters) -> T) -> T {
        f(&mut self.adapters.borrow_mut())
    }
    fn state(&self) -> Result<State> {
        self.store.load(&self.signer)
    }
    fn save(&self, state: &State) -> Result<()> {
        self.store.save(state, &self.signer)
    }

    fn sign(&self, state: &mut State, body: &Value, recipient: &str, now: u64) -> Result<Value> {
        if body["v"]
            .as_str()
            .is_some_and(|v| v.starts_with("openagents.live-"))
        {
            live::validate(body).map_err(contract)?;
        }
        let reference = self.store.put_json(body)?;
        let digest = digest_of(&reference).to_string();
        let declaration = format!("{digest}:to:{recipient}");
        if !state.declarations.contains_key(&declaration) {
            let event = workspace_host::host::request(&self.secret, recipient, body.clone(), now)?;
            let event_ref = self.store.put(
                &serde_json::to_vec(&event).map_err(|e| e.to_string())?,
                "application/json",
                None,
            )?;
            state
                .declarations
                .entry(digest.clone())
                .or_insert_with(|| event_ref.clone());
            state.declarations.insert(declaration, event_ref);
            state
                .signers
                .entry(digest)
                .or_default()
                .insert(self.pubkey().into());
        }
        self.share_output(state, body, recipient, &reference)?;
        Ok(reference)
    }

    fn authenticate(
        &self,
        state: &mut State,
        event: &Event,
        now: u64,
    ) -> Result<(String, Value, Value)> {
        let opened =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        if opened.recipient() != self.pubkey()
            || opened.body().issued_at > now.saturating_add(300)
            || now > opened.body().retain_until
        {
            return Err(refuse("not_admitted", "request recipient or time differs"));
        }
        let bytes = opened
            .inline_bytes()
            .ok_or("malformed: requests carry their artifact inline")?
            .to_vec();
        let body = contracts::parse_strict_bounded(&bytes, 1_048_576).map_err(contract)?;
        let schema = body["v"].as_str().unwrap_or_default().to_string();
        let reference = self.store.put(
            &bytes,
            "application/json",
            Some(schema.as_str()).filter(|s| !s.is_empty()),
        )?;
        let digest = digest_of(&reference).to_string();
        state
            .signers
            .entry(digest.clone())
            .or_default()
            .insert(opened.signer().into());
        let event_ref = self.store.put(
            &serde_json::to_vec(event).map_err(|e| e.to_string())?,
            "application/json",
            None,
        )?;
        state
            .declarations
            .entry(format!("{digest}:{}", opened.signer()))
            .or_insert(event_ref);
        self.remember(state, opened.signer(), &reference)?;
        Ok((opened.signer().into(), body, reference))
    }

    fn bytes(&self, reference: &Value) -> Result<Vec<u8>> {
        if self.reader.borrow().as_ref().is_some_and(|scope| {
            scope.principal != self.pubkey() && !scope.known.contains(digest_of(reference))
        }) {
            return Err(refuse(
                "not_admitted",
                "referenced bytes were not supplied or shared",
            ));
        }
        self.store
            .get(reference)
            .map_err(|_| refuse("content_unavailable", "referenced bytes are not retained"))
    }
    fn body(&self, reference: &Value, schema: &str) -> Result<Value> {
        let bytes = self.bytes(reference)?;
        if schema.starts_with("openagents.live-") {
            return live::verify(reference, &bytes, schema).map_err(contract);
        }
        let value = contracts::parse_strict_bounded(&bytes, 1_048_576).map_err(contract)?;
        if value["v"] != schema {
            return Err(refuse("identity_mismatch", schema));
        }
        Ok(value)
    }
    fn signed_by(state: &State, reference: &Value, signer: &str) -> bool {
        state
            .signers
            .get(digest_of(reference))
            .is_some_and(|s| s.contains(signer))
    }

    fn record(&self, session: &mut Session, kind: &str, detail: Value, now: u64) -> Result<Value> {
        let mut data = match kind {
            "created" => json!({"owner":self.pubkey(),"request":detail["request"],"base":null,
                "program":"live","lock":null,"context":null,"policy":null,"parent":null,"recipients":[session.owner]}),
            "dispatched" => json!({"binding":"live","attempt":1,"input":detail["request"],
                "context":null,"effect":detail["effect"],"generation":0}),
            "unknown" => json!({"attempt":1,"reservation":null,"evidence":[]}),
            "cancel_requested" => {
                json!({"requester":session.owner,"scope":"session","reason":detail["reason"]})
            }
            _ => json!({"evidence":[],"sources":[],"complete":true,"provenance":"live-host"}),
        };
        data["live"] = detail;
        let previous = session.run.last().map(|b| b["digest"].clone());
        let record = json!({"run":session.id,"seq":session.run.len(),"previous":previous,
            "controller":self.pubkey(),"generation":0,"type":kind,
            "subject":{"step":null,"iteration":null,"attempt":null},"time":now,"data":data});
        let digest = nostr::run::logical_digest(&record).map_err(contract)?;
        let body =
            json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"record":record,"digest":digest});
        let mut journal = nostr::run::Journal::default();
        for body in session.run.iter().chain(std::iter::once(&body)) {
            let record = nostr::run::parse_envelope(body).map_err(contract)?;
            if nostr::run::ingest(&mut journal, record, self.pubkey()).map_err(contract)?
                != nostr::run::Ingest::Applied
            {
                return Err("conflict: RUN did not extend its exact history".into());
            }
        }
        let reference = self.store.put_json(&body)?;
        session.run.push(body);
        Ok(reference)
    }

    #[allow(clippy::too_many_arguments)]
    fn result(
        &self,
        state: &mut State,
        recipient: &str,
        input: &Value,
        session: Option<&Session>,
        status: &str,
        reason: Option<&str>,
        value: Option<Value>,
        receipts: Vec<Value>,
        now: u64,
    ) -> Result<Value> {
        let record = session
            .and_then(|s| s.run.last())
            .map(|b| self.store.put_json(b))
            .transpose()?;
        let body = json!({"v":"openagents.live-result.v1","requires":[],"request":input,
            "plan":session.map(|s| s.plan.clone()),"revision":session.map_or(0, |s| s.revision),
            "state":session.map_or("unknown", |s| s.state.as_str()),"status":status,"reason":reason,
            "record":record,"value":value,"receipts":receipts});
        self.sign(state, &body, recipient, now)?;
        Ok(body)
    }

    /// Admit one signed operation.
    pub fn submit(&self, bundle: &Bundle, now: u64) -> Result<Value> {
        let received = std::time::Instant::now();
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        for (digest, bytes) in &bundle.artifacts {
            if digest_bytes(bytes) != *digest {
                return Err(refuse("identity_mismatch", "attached artifact digest"));
            }
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        for declaration in &bundle.declarations {
            self.authenticate(&mut state, declaration, now)?;
        }
        let (signer, body, input) = self.authenticate(&mut state, &bundle.event, now)?;
        for digest in bundle.artifacts.keys() {
            self.remember(&mut state, &signer, &json!({"digest":digest}))?;
        }
        self.with_read_scope(self.read_scope(&state, &signer)?, || {
            if let Err(e) = live::validate(&body) {
                let result = self.result(
                    &mut state,
                    &signer,
                    &input,
                    None,
                    "refused",
                    Some(e.code.as_str()),
                    None,
                    Vec::new(),
                    now,
                )?;
                self.save(&state)?;
                return Ok(result);
            }
            let session_id = if body["v"] == "openagents.live-plan.v1" {
                body["session"].as_str().map(str::to_string)
            } else {
                // A request naming a superseded plan reaches its session and is
                // refused as stale there.
                state.plans.get(digest_of(&body["plan"])).cloned()
            };
            let request = match body["v"].as_str() {
                Some("openagents.live-plan.v1") => format!("plan-{}", body["expected_revision"]),
                _ => body["request"].as_str().unwrap_or_default().to_string(),
            };
            let key = format!(
                "{signer}:{}:{request}",
                session_id.clone().unwrap_or_default()
            );
            if !self.may_request(&state, session_id.as_deref(), &signer, &body, now) {
                let result = self.result(
                    &mut state,
                    &signer,
                    &input,
                    None,
                    "refused",
                    Some("not_admitted"),
                    None,
                    Vec::new(),
                    now,
                )?;
                self.save(&state)?;
                return Ok(result);
            }
            if body["v"] == "openagents.live-input-admission.v1"
                && let Some(result) =
                    self.recover_input(&mut state, session_id.as_deref(), &signer, &input, now)?
            {
                state.priors.insert(
                    key,
                    Prior {
                        fingerprint: digest_of(&input).into(),
                        result: result.clone(),
                    },
                );
                self.save(&state)?;
                return Ok(result);
            }
            let outcome = match state.priors.get(&key) {
                Some(prior) if prior.fingerprint == digest_of(&input) => {
                    let mut result = prior.result.clone();
                    if result["status"] == "accepted" {
                        result["status"] = json!("duplicate");
                    }
                    self.save(&state)?;
                    return Ok(result);
                }
                Some(_) => Err(refuse(
                    "idempotency_conflict",
                    "same request, different bytes",
                )),
                None => self.dispatch(
                    &mut state,
                    session_id.as_deref(),
                    &signer,
                    &body,
                    &input,
                    now,
                    received,
                ),
            };
            let result = match outcome {
                Ok((result, remember)) => {
                    if remember {
                        state.priors.insert(
                            key,
                            Prior {
                                fingerprint: digest_of(&input).into(),
                                result: result.clone(),
                            },
                        );
                    }
                    result
                }
                Err(error) => {
                    let mut fresh = self.state()?;
                    fresh.signers.extend(state.signers);
                    fresh.declarations.extend(state.declarations);
                    for (principal, known) in state.known {
                        fresh.known.entry(principal).or_default().extend(known);
                    }
                    state = fresh;
                    if body["v"] == "openagents.live-input-admission.v1"
                        && let Some(result) = self.recover_input(
                            &mut state,
                            session_id.as_deref(),
                            &signer,
                            &input,
                            now,
                        )?
                    {
                        state.priors.insert(
                            key,
                            Prior {
                                fingerprint: digest_of(&input).into(),
                                result: result.clone(),
                            },
                        );
                        self.save(&state)?;
                        return Ok(result);
                    }
                    let session = session_id
                        .as_ref()
                        .and_then(|id| state.sessions.get(id))
                        .cloned();
                    let status = if matches!(code(&error), "conflict" | "idempotency_conflict") {
                        "conflict"
                    } else {
                        "refused"
                    };
                    self.result(
                        &mut state,
                        &signer,
                        &input,
                        session.as_ref(),
                        status,
                        Some(code(&error)),
                        None,
                        Vec::new(),
                        now,
                    )?
                }
            };
            self.save(&state)?;
            Ok(result)
        })
    }

    fn dispatch(
        &self,
        state: &mut State,
        session_id: Option<&str>,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
        received: std::time::Instant,
    ) -> Result<(Value, bool)> {
        if body["v"] == "openagents.live-plan.v1" {
            return self
                .plan(state, signer, body, input, now)
                .map(|r| (r, true));
        }
        let id = session_id.ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let mut session = state
            .sessions
            .remove(id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let outcome = (|| -> Result<(Value, bool)> {
            if !same(&session.plan, &body["plan"]) {
                return Err(refuse("stale", "request names a superseded plan"));
            }
            let status_read =
                body["v"] == "openagents.live-control.v1" && body["action"] == "status";
            if !status_read
                && body["v"] != "openagents.live-input.v1"
                && body["expected_revision"] != session.revision
            {
                return Err(refuse("conflict", "stale session revision"));
            }
            let (value, receipts, status) = match body["v"].as_str() {
                Some("openagents.live-participant.v1") => {
                    self.participant(state, &mut session, signer, body, now)?
                }
                Some("openagents.live-control.v1") => {
                    self.control(&mut session, signer, body, now)?
                }
                Some("openagents.live-input-floor-request.v1") => {
                    self.input_floor(state, &mut session, signer, body, input, now)?
                }
                Some("openagents.live-floor-request.v1") => {
                    self.speak_floor(state, &mut session, signer, body, input, now)?
                }
                Some("openagents.live-capture.v1") => {
                    self.capture(state, &mut session, signer, body, now)?
                }
                Some("openagents.live-input.v1") => {
                    self.prepare_input(state, &mut session, signer, body, input, now)?
                }
                Some("openagents.live-input-admission.v1") => {
                    self.device_input(state, &mut session, signer, body, input, now, received)?
                }
                _ => return Err(refuse("unsupported_feature", "operation")),
            };
            if !status_read {
                session.revision += 1;
            }
            let refusal = value
                .as_ref()
                .filter(|_| status == "refused")
                .map(|reference| self.store.json(reference))
                .transpose()?
                .and_then(|record| record["reason"].as_str().map(str::to_owned));
            let owner = session.owner.clone();
            let result = self.result(
                state,
                if signer == owner { &owner } else { signer },
                input,
                Some(&session),
                status,
                refusal.as_deref(),
                value,
                receipts,
                now,
            )?;
            Ok((result, !status_read))
        })();
        state.sessions.insert(id.into(), session);
        outcome
    }

    fn plan(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        if body["owner"] != signer || !state.policy.owners.contains(signer) {
            return Err(refuse(
                "not_admitted",
                "owner is not authorized by this host",
            ));
        }
        if body["controller"] != self.pubkey() {
            return Err(refuse("not_admitted", "plan names another controller"));
        }
        let task = body["task"].as_str().unwrap_or_default();
        if !state.policy.tasks.contains(task) {
            return Err(refuse("not_admitted", "the task does not exist"));
        }
        if live::number(body, "expires_at").map_err(contract)? <= now {
            return Err(refuse("stale", "plan expired"));
        }
        let conversation = self.body(&body["conversation"], "openagents.live-conversation.v1")?;
        let participants = strings(&conversation["participants"]);
        let scope_ok = match conversation["scope"].as_str() {
            Some("owner_private") => participants == [signer.to_string()],
            Some("direct") => participants.len() == 2 && participants.iter().any(|p| p == signer),
            Some("group") => true,
            _ => false,
        };
        if !scope_ok {
            return Err(refuse(
                "incompatible",
                "conversation scope and participants disagree",
            ));
        }
        let consent = self.body(&body["consent_policy"], "openagents.live-consent-policy.v1")?;
        let _ = consent;
        contracts::parse_reservation(
            &self.body(&body["reservation"], "openagents.reservation.v1")?,
        )
        .map_err(contract)?;
        for reference in body["bindings"].as_array().into_iter().flatten() {
            self.bytes(reference)?;
        }
        self.bytes(&body["disclosure"])?;
        let frame = nostr::work::parse(&self.bytes(&body["frame"])?).map_err(contract)?;
        if frame["v"] != "openagents.task-frame.v1"
            || body["frame"]["schema"] != "openagents.task-frame.v1"
            || frame["task"] != body["task"]
            || frame["owner"] != signer
            || frame["controller"] != signer
        {
            return Err("not_admitted: exact owner-controlled task frame required".into());
        }
        self.bytes(&body["transport"]["artifact"])?;
        for track in body["tracks"].as_array().into_iter().flatten() {
            for recipient in track["recipients"].as_array().into_iter().flatten() {
                self.bytes(recipient)?;
            }
            if track["retention"]["until"]
                .as_u64()
                .is_some_and(|u| u <= now)
            {
                return Err(refuse("stale", "track retention already ended"));
            }
        }
        let id = body["session"].as_str().unwrap_or_default().to_string();
        let mut session = if body["previous"].is_null() {
            if state.sessions.contains_key(&id) {
                return Err(refuse("conflict", "session already admitted"));
            }
            if state.sessions.len() >= MAX_SESSIONS {
                return Err(refuse("limit_exceeded", "session retention limit"));
            }
            if body["media_epoch"] != 0 || body["generation"] != 0 {
                return Err(refuse(
                    "stale",
                    "a joining client cannot choose epoch or generation",
                ));
            }
            let mut session = Session {
                id: id.clone(),
                task: task.into(),
                owner: signer.into(),
                plan: input.clone(),
                body: body.clone(),
                revision: 0,
                state: "admitted".into(),
                epoch: 0,
                members: BTreeMap::new(),
                input_floors: BTreeMap::new(),
                speak_floors: BTreeMap::new(),
                observations: BTreeMap::new(),
                transcripts: BTreeMap::new(),
                run: Vec::new(),
                unresolved: Vec::new(),
                disclosed_bytes: 0,
            };
            self.record(
                &mut session,
                "created",
                json!({"request":digest_of(input)}),
                now,
            )?;
            session
        } else {
            let mut session = state
                .sessions
                .remove(&id)
                .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
            let outcome = (|| -> Result<()> {
                if !same(&session.plan, &body["previous"])
                    || body["expected_revision"] != session.revision
                {
                    return Err(refuse("conflict", "plan predecessor or revision"));
                }
                if session.task != task || session.owner != signer || session.state == "closed" {
                    return Err(refuse("conflict", "replacement changes session identity"));
                }
                if body["media_epoch"].as_u64() != Some(session.epoch + 1) {
                    return Err(refuse(
                        "stale",
                        "replacement must advance the media epoch by one",
                    ));
                }
                // Pause old dispatch and fence every old credential first.
                if self.fence_all(&mut session) {
                    session.state = "unknown".into();
                    return Err(refuse(
                        "cannot_enforce",
                        "old credentials could not be fenced",
                    ));
                }
                Ok(())
            })();
            if let Err(e) = outcome {
                state.sessions.insert(id, session);
                return Err(e);
            }
            session.plan = input.clone();
            session.body = body.clone();
            session.epoch += 1;
            session.state = "admitted".into();
            session
        };
        let detail = json!({"plan":digest_of(input),"epoch":session.epoch});
        self.record(&mut session, "observed", detail, now)?;
        session.revision += 1;
        let owner = session.owner.clone();
        let result = self.result(
            state,
            &owner,
            input,
            Some(&session),
            "accepted",
            None,
            None,
            Vec::new(),
            now,
        )?;
        state.plans.insert(digest_of(input).into(), id.clone());
        state.sessions.insert(id, session);
        Ok(result)
    }

    /// Revoke every credential and drop every floor. Returns true when any
    /// revocation could not be confirmed.
    fn fence_all(&self, session: &mut Session) -> bool {
        let mut unknown = false;
        for (participant, member) in session.members.iter_mut() {
            for digest in member.credentials.drain(..) {
                if self.adapters.borrow_mut().broker.revoke(&digest) == Revocation::Unknown {
                    unknown = true;
                    session.unresolved.push(format!("credential {participant}"));
                }
            }
        }
        session.members.clear();
        session.input_floors.clear();
        session.speak_floors.clear();
        unknown
    }

    #[allow(clippy::too_many_arguments)]
    fn consented(
        &self,
        state: &State,
        session: &Session,
        participant: &str,
        rights: &[String],
        tracks: &[String],
        consent: &[Value],
        now: u64,
    ) -> Result<()> {
        let policy = session.consent_policy(&self.store)?;
        let required: Vec<String> = strings(&policy["requires_consent"])
            .into_iter()
            .filter(|r| rights.contains(r))
            .collect();
        for right in required {
            let ok = consent.iter().any(|reference| {
                Self::signed_by(state, reference, participant)
                    && self
                        .store
                        .get(reference)
                        .and_then(|bytes| {
                            live::verify(reference, &bytes, "openagents.live-consent.v1")
                                .map_err(contract)
                        })
                        .is_ok_and(|c| {
                            same(&c["plan"], &session.plan)
                                && c["participant"] == participant
                                && strings(&c["rights"]).contains(&right)
                                && tracks.iter().all(|t| strings(&c["tracks"]).contains(t))
                                && c["expires_at"].as_u64().is_some_and(|e| e > now)
                        })
            });
            if !ok {
                return Err(refuse(
                    "not_admitted",
                    "required participant consent is missing",
                ));
            }
        }
        Ok(())
    }

    #[allow(clippy::type_complexity)]
    fn participant(
        &self,
        state: &mut State,
        session: &mut Session,
        signer: &str,
        body: &Value,
        now: u64,
    ) -> Result<(Option<Value>, Vec<Value>, &'static str)> {
        let participant = body["participant"].as_str().unwrap_or_default().to_string();
        let action = body["action"].as_str().unwrap_or_default();
        let self_withdrawal = action == "revoke" && signer == participant;
        if signer != session.owner && !self_withdrawal {
            return Err(refuse(
                "not_admitted",
                "participants cannot grant themselves access",
            ));
        }
        if session.state == "closed" {
            return Err(refuse("unavailable", "session is closed"));
        }
        let plan_expiry = live::number(&session.body, "expires_at").map_err(contract)?;
        if action == "revoke" {
            let member = session
                .members
                .remove(&participant)
                .ok_or_else(|| refuse("conflict", "not a participant"))?;
            session
                .input_floors
                .retain(|_, f| f.participant != participant);
            session
                .speak_floors
                .retain(|_, f| f.participant != participant);
            let mut status = "accepted";
            for digest in member.credentials {
                if self.adapters.borrow_mut().broker.revoke(&digest) == Revocation::Unknown {
                    session.unresolved.push(format!("credential {participant}"));
                    status = "unknown";
                }
            }
            self.record(
                session,
                "observed",
                json!({"revoked":participant,"confirmed":status == "accepted"}),
                now,
            )?;
            return Ok((None, Vec::new(), status));
        }
        let conversation = self.store.json(&session.body["conversation"])?;
        if !strings(&conversation["participants"]).contains(&participant) {
            return Err(refuse("not_admitted", "not a conversation participant"));
        }
        if session.members.len() >= MAX_PARTICIPANTS && !session.members.contains_key(&participant)
        {
            return Err(refuse("limit_exceeded", "participant limit"));
        }
        let expires_at = live::number(body, "expires_at").map_err(contract)?;
        if expires_at <= now || expires_at > plan_expiry {
            return Err(refuse(
                "limit_exceeded",
                "participant expiry exceeds the plan",
            ));
        }
        let tracks = strings(&body["tracks"]);
        let rights = strings(&body["rights"]);
        let directions: Vec<String> = tracks
            .iter()
            .map(|t| {
                session
                    .track(t)
                    .map(|t| t["direction"].as_str().unwrap_or_default().to_string())
            })
            .collect::<Option<_>>()
            .ok_or_else(|| refuse("not_admitted", "unknown track"))?;
        for right in &rights {
            let needs = match right.as_str() {
                "publish" | "record" | "transcribe" => Some("capture"),
                "playback" => Some("playback"),
                _ => None,
            };
            if let Some(direction) = needs
                && !directions.iter().any(|d| d == direction)
            {
                return Err(refuse(
                    "incompatible",
                    "a right does not fit its tracks' direction",
                ));
            }
        }
        let consent: Vec<Value> = body["consent"].as_array().cloned().unwrap_or_default();
        self.consented(
            state,
            session,
            &participant,
            &rights,
            &tracks,
            &consent,
            now,
        )?;
        let token = self.adapters.borrow_mut().broker.mint(Grant {
            session: session.id.clone(),
            epoch: session.epoch,
            participant: participant.clone(),
            tracks: tracks.clone(),
            rights: rights.clone(),
            expires_at: expires_at.min(now + 600),
        });
        let mut credentials = session
            .members
            .remove(&participant)
            .map(|m| m.credentials)
            .unwrap_or_default();
        credentials.push(token_digest(&token));
        self.delivery
            .borrow_mut()
            .insert((session.id.clone(), participant.clone()), token);
        session.members.insert(
            participant.clone(),
            Member {
                tracks,
                rights,
                expires_at,
                consent,
                credentials,
            },
        );
        self.record(session, "observed", json!({"admitted":participant}), now)?;
        let _ = state;
        Ok((None, Vec::new(), "accepted"))
    }

    /// Secure local delivery of a freshly minted credential, once.
    pub fn take_credential(&self, session: &str, participant: &str) -> Option<String> {
        self.delivery
            .borrow_mut()
            .remove(&(session.to_string(), participant.to_string()))
    }

    #[allow(clippy::type_complexity)]
    fn control(
        &self,
        session: &mut Session,
        signer: &str,
        body: &Value,
        now: u64,
    ) -> Result<(Option<Value>, Vec<Value>, &'static str)> {
        if signer != session.owner {
            return Err(refuse("not_admitted", "session control"));
        }
        let action = body["action"].as_str().unwrap_or_default();
        let plan_expiry = live::number(&session.body, "expires_at").map_err(contract)?;
        match (action, session.state.as_str()) {
            ("status", _) => return Ok((None, Vec::new(), "accepted")),
            ("start", "admitted") | ("resume", "paused") => {
                if plan_expiry <= now {
                    return Err(refuse("stale", "plan expired"));
                }
                let members: Vec<(String, Member)> = session
                    .members
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                let state = self.state()?;
                for (participant, member) in members {
                    self.consented(
                        &state,
                        session,
                        &participant,
                        &member.rights,
                        &member.tracks,
                        &member.consent,
                        now,
                    )?;
                }
                session.state = "active".into();
                self.record(
                    session,
                    "dispatched",
                    json!({"request":action,"effect":"media"}),
                    now,
                )?;
            }
            ("pause", "active") => {
                session.state = "paused".into();
                self.record(
                    session,
                    "observed",
                    json!({"paused":true,"in_flight_floors":session.speak_floors.len()}),
                    now,
                )?;
            }
            ("stop", "admitted" | "active" | "paused" | "unknown") => {
                self.record(session, "cancel_requested", json!({"reason":"stop"}), now)?;
                let unknown = self.fence_all(session);
                session.state = if unknown || !session.unresolved.is_empty() {
                    "unknown"
                } else {
                    "closed"
                }
                .into();
                let kind = if session.state == "closed" {
                    "observed"
                } else {
                    "unknown"
                };
                self.record(
                    session,
                    kind,
                    json!({"stopped":session.state == "closed","unresolved":session.unresolved}),
                    now,
                )?;
                return Ok((
                    None,
                    Vec::new(),
                    if session.state == "closed" {
                        "accepted"
                    } else {
                        "unknown"
                    },
                ));
            }
            _ => return Err(refuse("conflict", "lifecycle transition is not allowed")),
        }
        Ok((None, Vec::new(), "accepted"))
    }

    fn floor_until(
        &self,
        session: &Session,
        participant: &str,
        body: &Value,
        now: u64,
    ) -> Result<u64> {
        let until = live::number(body, "until").map_err(contract)?;
        let member = session
            .members
            .get(participant)
            .ok_or_else(|| refuse("not_admitted", "not a participant"))?;
        let plan_expiry = live::number(&session.body, "expires_at").map_err(contract)?;
        if until <= now || until > member.expires_at.min(plan_expiry) {
            return Err(refuse("limit_exceeded", "floor outlasts its authorization"));
        }
        Ok(until)
    }

    #[allow(clippy::type_complexity)]
    fn input_floor(
        &self,
        state: &mut State,
        session: &mut Session,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<(Option<Value>, Vec<Value>, &'static str)> {
        let participant = body["participant"].as_str().unwrap_or_default().to_string();
        let key = format!(
            "{}|{}",
            digest_of(&body["destination"]),
            body["processing"]["id"].as_str().unwrap_or_default()
        );
        if body["action"] == "release" {
            let floor = session
                .input_floors
                .get(&key)
                .ok_or_else(|| refuse("conflict", "no such floor"))?;
            if floor.nonce != body["nonce"].as_str().unwrap_or_default()
                || signer != floor.participant && signer != session.owner
            {
                return Err(refuse("not_admitted", "release names another floor"));
            }
            session.input_floors.remove(&key);
            self.record(
                session,
                "observed",
                json!({"input_floor_released":key}),
                now,
            )?;
            return Ok((None, Vec::new(), "accepted"));
        }
        if signer != participant {
            return Err(refuse(
                "not_admitted",
                "a floor is requested by its source participant",
            ));
        }
        if session.state != "active" {
            return Err(refuse("unavailable", "session is not active"));
        }
        let track_id = body["track"].as_str().unwrap_or_default();
        let track = session
            .track(track_id)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "unknown track"))?;
        let member = session
            .members
            .get(&participant)
            .ok_or_else(|| refuse("not_admitted", "not a participant"))?;
        // Playback rights never become input authority.
        if track["direction"] != "capture"
            || !member.rights.iter().any(|r| r == "publish")
            || !member.tracks.iter().any(|t| t == track_id)
        {
            return Err(refuse(
                "not_admitted",
                "the source holds no publish right on that track",
            ));
        }
        if !track["recipients"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|r| same(r, &body["destination"]))
        {
            return Err(refuse(
                "not_admitted",
                "destination is not an admitted recipient",
            ));
        }
        if !track["processing"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| *p == body["processing"])
        {
            return Err(refuse(
                "not_admitted",
                "processing is not admitted for the track",
            ));
        }
        if session.input_floors.contains_key(&key) {
            return Err(refuse(
                "conflict",
                "another input floor holds that destination",
            ));
        }
        let until = self.floor_until(session, &participant, body, now)?;
        let lease = json!({"v":"openagents.live-input-floor.v1","requires":[],"request":input,"plan":session.plan,
            "participant":participant,"track":track_id,"destination":body["destination"],"processing":body["processing"],
            "nonce":body["nonce"],"until":until,"revision":session.revision + 1});
        let lease = self.sign(state, &lease, &participant, now)?;
        session.input_floors.insert(
            key,
            Floor {
                nonce: body["nonce"].as_str().unwrap_or_default().into(),
                participant,
                track: track_id.into(),
                until,
                epoch: session.epoch,
                lease: lease.clone(),
            },
        );
        self.record(
            session,
            "observed",
            json!({"input_floor":digest_of(&lease)}),
            now,
        )?;
        Ok((Some(lease), Vec::new(), "accepted"))
    }

    #[allow(clippy::type_complexity)]
    fn speak_floor(
        &self,
        state: &mut State,
        session: &mut Session,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<(Option<Value>, Vec<Value>, &'static str)> {
        let participant = body["participant"].as_str().unwrap_or_default().to_string();
        let track_id = body["track"].as_str().unwrap_or_default().to_string();
        if body["action"] == "release" {
            let floor = session
                .speak_floors
                .get(&track_id)
                .ok_or_else(|| refuse("conflict", "no such floor"))?;
            if floor.nonce != body["nonce"].as_str().unwrap_or_default()
                || signer != floor.participant && signer != session.owner
            {
                return Err(refuse("not_admitted", "release names another floor"));
            }
            session.speak_floors.remove(&track_id);
            self.record(session, "observed", json!({"floor_released":track_id}), now)?;
            return Ok((None, Vec::new(), "accepted"));
        }
        if signer != participant {
            return Err(refuse(
                "not_admitted",
                "a floor is requested by its participant",
            ));
        }
        if session.state != "active" {
            return Err(refuse("unavailable", "session is not active"));
        }
        let track = session
            .track(&track_id)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "unknown track"))?;
        let member = session
            .members
            .get(&participant)
            .ok_or_else(|| refuse("not_admitted", "not a participant"))?;
        if track["direction"] != "playback"
            || !member.rights.iter().any(|r| r == "playback")
            || !member.tracks.contains(&track_id)
        {
            return Err(refuse("not_admitted", "no playback right on that track"));
        }
        if session.speak_floors.contains_key(&track_id) {
            return Err(refuse("conflict", "the playback track already has a floor"));
        }
        let until = self.floor_until(session, &participant, body, now)?;
        let lease = json!({"v":"openagents.live-floor.v1","requires":[],"request":input,"plan":session.plan,
            "participant":participant,"track":track_id,"nonce":body["nonce"],"until":until,"revision":session.revision + 1});
        let lease = self.sign(state, &lease, &participant, now)?;
        session.speak_floors.insert(
            track_id.clone(),
            Floor {
                nonce: body["nonce"].as_str().unwrap_or_default().into(),
                participant,
                track: track_id,
                until,
                epoch: session.epoch,
                lease: lease.clone(),
            },
        );
        self.record(session, "observed", json!({"floor":digest_of(&lease)}), now)?;
        Ok((Some(lease), Vec::new(), "accepted"))
    }

    #[allow(clippy::type_complexity)]
    fn capture(
        &self,
        state: &mut State,
        session: &mut Session,
        signer: &str,
        body: &Value,
        now: u64,
    ) -> Result<(Option<Value>, Vec<Value>, &'static str)> {
        let captured_start = std::time::Instant::now();
        if self.capture_clocks.borrow().len() >= 65_536 {
            return Err("limit_exceeded: live observation clocks".into());
        }
        if signer != session.owner {
            return Err(refuse("not_admitted", "capture control"));
        }
        if session.state != "active" {
            return Err(refuse("unavailable", "session is not active"));
        }
        let track_id = body["track"].as_str().unwrap_or_default();
        let track = session
            .track(track_id)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "unknown track"))?;
        if track["direction"] != "capture" {
            return Err(refuse(
                "not_admitted",
                "playback tracks are not capture sources",
            ));
        }
        let retained = track["retention"]["mode"] == "bounded";
        if body["purpose"] == "recording" && !retained {
            return Err(refuse(
                "cannot_enforce",
                "live delivery is not a recording right",
            ));
        }
        if retained {
            self.retention_consent(state, session, &track, &["record"], now)?;
        }
        let expiry = live::number(&session.body, "expires_at").map_err(contract)?;
        if expiry <= now {
            return Err("stale: capture plan expired".into());
        }
        let ceiling = |limits: &Value| {
            limits
                .as_array()
                .into_iter()
                .flatten()
                .find(|b| b["bound"] == "storage_bytes")
                .and_then(|b| b["ceiling"].as_u64())
        };
        let max_bytes = ceiling(&session.body["bounds"])
            .ok_or("cannot_enforce: capture byte ceiling")?
            .min(ceiling(&body["limits"]).unwrap_or(u64::MAX))
            .min(1_048_576) as usize;
        let (generation, focus) = self.adapters.borrow().device.state();
        let context = crate::media::CaptureContext {
            owner: &session.owner,
            session: &session.id,
            plan: &session.plan,
            generation,
            focus,
            max_bytes,
            deadline: captured_start
                .checked_add(std::time::Duration::from_secs(expiry - now))
                .ok_or("limit_exceeded: capture deadline")?,
        };
        let captured = self.adapters.borrow_mut().source.capture_checked(
            track["source"]["scope"].as_str().unwrap_or_default(),
            track["source"]["id"].as_str().unwrap_or_default(),
            track["media"].as_str().unwrap_or_default(),
            &context,
        )?;
        if captured.bytes.len() > max_bytes || std::time::Instant::now() >= context.deadline {
            return Err("limit_exceeded: capture exceeds its bounds".into());
        }
        let (generation, focus) = captured
            .anchor
            .ok_or("cannot_enforce: capture has no native anchor")?;
        let content = if retained {
            Some(
                self.store
                    .put(&captured.bytes, "application/octet-stream", None)?,
            )
        } else {
            None
        };
        let geometry = match &captured.geometry {
            Some((w, h, space)) => json!({"width_px":w,"height_px":h,"coordinate_space":space,
                "transform":self.store.put_json(&json!({"transform":"identity","space":space}))?}),
            None => Value::Null,
        };
        let interval = match &captured.interval {
            Some((clock, start, end)) => json!({"clock":clock,"start_ms":start,"end_ms":end}),
            None => Value::Null,
        };
        let freshness = self.sign(
            state,
            &json!({"v":"openagents.live-freshness.v1","requires":[],
            "max_age_ms":30_000,"host_generation":generation,"focus_version":focus}),
            &session.owner.clone(),
            now,
        )?;
        let capture = token();
        let observation = json!({"v":"openagents.live-observation.v1","requires":[],"plan":session.plan,"track":track_id,
            "capture":capture,"source":track["source"],"captured_at":now,"host_generation":generation,"content":content,
            "evidence":null,"format":self.store.put_json(&json!({"media":track["media"]}))?,"geometry":geometry,
            "interval":interval,"freshness":freshness,"meta":{"media_epoch":session.epoch,"native":captured.native}});
        let observation = self.sign(state, &observation, &session.owner.clone(), now)?;
        self.capture_clocks
            .borrow_mut()
            .insert(digest_of(&observation).into(), captured_start);
        session.observations.insert(capture, observation.clone());
        self.record(
            session,
            "observed",
            json!({"capture":digest_of(&observation),"purpose":body["purpose"]}),
            now,
        )?;
        Ok((Some(observation), Vec::new(), "accepted"))
    }

    /// The transport input gate: forward bytes only under the current lease,
    /// source, track, and media epoch.
    #[allow(clippy::too_many_arguments)]
    pub fn forward(
        &self,
        session_id: &str,
        nonce: &str,
        participant: &str,
        track: &str,
        epoch: u64,
        bytes: &[u8],
        now: u64,
    ) -> Result<()> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let session = state
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        if session.state != "active" {
            return Err(refuse("unavailable", "session is not active"));
        }
        if epoch != session.epoch {
            return Err(refuse("stale", "audio from another media epoch"));
        }
        let floor = session
            .input_floors
            .values()
            .find(|f| f.nonce == nonce)
            .ok_or_else(|| refuse("not_admitted", "no current input floor"))?;
        if floor.participant != participant
            || floor.track != track
            || floor.epoch != epoch
            || floor.until <= now
        {
            return Err(refuse(
                "not_admitted",
                "audio is outside the admitted floor",
            ));
        }
        session.disclosed_bytes += bytes.len() as u64;
        self.save(&state)
    }

    /// The playback gate for one bounded segment.
    pub fn play(
        &self,
        session_id: &str,
        track: &str,
        nonce: &str,
        epoch: u64,
        now: u64,
    ) -> Result<()> {
        let state = self.state()?;
        let session = state
            .sessions
            .get(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let floor = session
            .speak_floors
            .get(track)
            .ok_or_else(|| refuse("not_admitted", "no speaking floor"))?;
        if session.state != "active"
            || floor.nonce != nonce
            || floor.epoch != epoch
            || epoch != session.epoch
            || floor.until <= now
        {
            return Err(refuse(
                "not_admitted",
                "playback is outside the admitted floor",
            ));
        }
        Ok(())
    }

    /// Human barge-in: revoke the agent's floor and record what was heard.
    pub fn barge_in(
        &self,
        session_id: &str,
        track: &str,
        confirmed_ms: u64,
        uncertain_until_ms: u64,
        now: u64,
    ) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut session = state
            .sessions
            .remove(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let outcome = (|| -> Result<Value> {
            let floor = session
                .speak_floors
                .remove(track)
                .ok_or_else(|| refuse("conflict", "no speaking floor"))?;
            let body = json!({"v":"openagents.live-playback-interruption.v1","requires":[],"floor":floor.lease,
                "confirmed_ms":confirmed_ms,"uncertain_until_ms":uncertain_until_ms,"reason":"human interruption"});
            let reference = self.sign(&mut state, &body, &session.owner.clone(), now)?;
            self.record(
                &mut session,
                "observed",
                json!({"barge_in":digest_of(&reference)}),
                now,
            )?;
            session.revision += 1;
            Ok(reference)
        })();
        state.sessions.insert(session_id.into(), session);
        let reference = outcome?;
        self.save(&state)?;
        Ok(reference)
    }

    /// A transport reconnection: a new media epoch, every floor invalidated,
    /// pending buffers fenced. Nothing replays.
    pub fn reconnect(&self, session_id: &str, now: u64) -> Result<u64> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let session = state
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        session.epoch += 1;
        session.input_floors.clear();
        session.speak_floors.clear();
        session.revision += 1;
        let epoch = session.epoch;
        let mut session = state.sessions.remove(session_id).ok_or("session")?;
        self.record(&mut session, "observed", json!({"reconnected":epoch}), now)?;
        state.sessions.insert(session_id.into(), session);
        self.save(&state)?;
        Ok(epoch)
    }

    /// A dropped client follows the pinned consent policy, never a guess.
    pub fn disconnect(&self, session_id: &str, now: u64) -> Result<String> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut session = state
            .sessions
            .remove(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let policy = session.consent_policy(&self.store)?;
        if session.state == "active" {
            if policy["on_disconnect"] == "stop" {
                self.record(
                    &mut session,
                    "cancel_requested",
                    json!({"reason":"disconnect"}),
                    now,
                )?;
                let unknown = self.fence_all(&mut session);
                session.state = if unknown { "unknown" } else { "closed" }.into();
            } else {
                session.state = "paused".into();
            }
            let detail = json!({"disconnect":session.state});
            self.record(&mut session, "observed", detail, now)?;
            session.revision += 1;
        }
        let out = session.state.clone();
        state.sessions.insert(session_id.into(), session);
        self.save(&state)?;
        Ok(out)
    }

    /// Retain a transcript revision derived from a retained observation.
    pub fn transcribe(
        &self,
        session_id: &str,
        observation: &Value,
        transform: &Value,
        segments: Vec<Value>,
        coverage: &str,
        now: u64,
    ) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut session = state
            .sessions
            .remove(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let outcome = (|| -> Result<Value> {
            if !session.observations.values().any(|o| same(o, observation)) {
                return Err(refuse(
                    "not_admitted",
                    "observation is not from this session",
                ));
            }
            let body = self.body(observation, "openagents.live-observation.v1")?;
            let track = session
                .track(body["track"].as_str().unwrap_or_default())
                .cloned()
                .ok_or("track")?;
            if !track["processing"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|p| p == transform)
            {
                return Err(refuse(
                    "not_admitted",
                    "the transform is not admitted for this track",
                ));
            }
            if body["content"].is_null() {
                return Err(refuse(
                    "content_unavailable",
                    "the observation's bytes were not retained",
                ));
            }
            self.retention_consent(&state, &session, &track, &["record", "transcribe"], now)?;
            let window = &body["interval"];
            if window.is_null() {
                return Err(refuse(
                    "incompatible",
                    "the observation has no media interval",
                ));
            }
            for segment in &segments {
                let interval = &segment["interval"];
                if interval["clock"] != window["clock"] {
                    return Err(refuse("incompatible", "clock domains differ"));
                }
                if interval["start_ms"].as_u64() < window["start_ms"].as_u64()
                    || interval["end_ms"].as_u64() > window["end_ms"].as_u64()
                {
                    return Err(refuse("incompatible", "segment outside its observation"));
                }
                self.bytes(&segment["text"])?;
            }
            let chain = session
                .transcripts
                .entry(digest_of(observation).to_string())
                .or_default();
            let previous = chain.last().cloned();
            let transcript = json!({"v":"openagents.live-transcript.v1","requires":[],"observation":observation,
                "transform":transform,"revision":chain.len(),"previous":previous,"segments":segments,"coverage":coverage});
            let owner = session.owner.clone();
            let reference = self.sign(&mut state, &transcript, &owner, now)?;
            session
                .transcripts
                .entry(digest_of(observation).to_string())
                .or_default()
                .push(reference.clone());
            Ok(reference)
        })();
        state.sessions.insert(session_id.into(), session);
        let reference = outcome?;
        self.save(&state)?;
        Ok(reference)
    }

    pub fn status(&self, session_id: &str) -> Result<Value> {
        let state = self.state()?;
        let session = state
            .sessions
            .get(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        Ok(
            json!({"session":session.id,"plan":session.plan,"revision":session.revision,"state":session.state,
            "epoch":session.epoch,"members":session.members.keys().collect::<Vec<_>>(),
            "input_floors":session.input_floors.len(),"speak_floors":session.speak_floors.len(),
            "unresolved":session.unresolved,"disclosed_bytes":session.disclosed_bytes,
            "observations":session.observations.values().collect::<Vec<_>>()}),
        )
    }
}

/// Build a signed private request envelope for a controller.
pub fn envelope(secret: &SecretKey, controller: &str, body: &Value, now: u64) -> Result<Event> {
    if body["v"] == nostr::policy::DECISION_SCHEMA {
        nostr::policy::validate(body).map_err(contract)?;
    } else {
        live::validate(body).map_err(contract)?;
    }
    workspace_host::host::request(secret, controller, body.clone(), now)
}
