//! The managed-session authority. It admits openings and commands under
//! exact revisions, keeps one durable FIFO per session, dispatches a turn
//! only under a current ENV execution binding, arbitrates interactions, and
//! never repeats an engine execution whose outcome is unknown.
use crate::{
    Result,
    engine::{Capture, Configuration, Engine, Observation, Profile, Support},
};
use nostr::{
    contracts::{self, SCHEMA_MEDIA_TYPE, digest_bytes},
    domain::{Event, RelaySigner},
    session as sess,
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

pub const MAX_SESSIONS: usize = 64;
pub const MAX_QUEUE: usize = 256;

/// The current ENV execution binding for one CJ execution.
pub trait Bindings {
    /// Only an in-process fixture may opt into local, unmetered operations.
    /// Production allocators require the charged execution callback.
    fn local_fixture(&self) -> bool {
        false
    }
    fn check(&self, attachment: &str, fingerprint: &str, now: u64) -> Result<()>;
    fn with_execution(
        &self,
        _request: &env_host::runtime::RuntimeDispatch<'_>,
        _now: u64,
        _effect: &mut dyn FnMut(
            &env_host::runtime::RuntimeFence<'_>,
            &coordination_host::worker::Intake,
            u64,
        ) -> Result<Value>,
    ) -> Result<Value> {
        Err("cannot_enforce: charged runtime intake is not configured".into())
    }

    fn with_current(
        &self,
        _request: &env_host::runtime::RuntimeUse<'_>,
        _now: u64,
        _effect: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        Err("cannot_enforce: authoritative runtime admission is not configured".into())
    }
    fn with_continuation(
        &self,
        _request: &env_host::runtime::RuntimeContinuation<'_>,
        _now: u64,
        _effect: &mut dyn FnMut(&env_host::runtime::ContinuationFence<'_>, u64) -> Result<Value>,
    ) -> Result<Value> {
        Err("cannot_enforce: runtime continuation is not configured".into())
    }
}

impl Bindings for env_host::allocator::Allocator {
    fn with_continuation(
        &self,
        request: &env_host::runtime::RuntimeContinuation<'_>,
        now: u64,
        effect: &mut dyn FnMut(&env_host::runtime::ContinuationFence<'_>, u64) -> Result<Value>,
    ) -> Result<Value> {
        self.with_runtime_continuation(request, now, effect)
    }
    fn check(&self, attachment: &str, fingerprint: &str, now: u64) -> Result<()> {
        self.check_dispatch(attachment, fingerprint, now)
    }
    fn with_execution(
        &self,
        request: &env_host::runtime::RuntimeDispatch<'_>,
        now: u64,
        effect: &mut dyn FnMut(
            &env_host::runtime::RuntimeFence<'_>,
            &coordination_host::worker::Intake,
            u64,
        ) -> Result<Value>,
    ) -> Result<Value> {
        self.with_runtime_execution(request, now, effect)
    }
    fn with_current(
        &self,
        request: &env_host::runtime::RuntimeUse<'_>,
        now: u64,
        effect: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        self.with_runtime_use(request, now, effect)
    }
}

/// The attachment the transport delivered this execution under.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Binding {
    pub attachment: String,
    pub fingerprint: String,
    /// The signed ENV attachment grant reference.
    pub grant: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub owners: BTreeSet<String>,
    pub controllers: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prior {
    fingerprint: String,
    result: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Adapter {
    reference: Value,
    engine: String,
    definition: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Queued {
    id: String,
    #[serde(default)]
    requester: String,
    intent: Value,
    state: String,
    turn: Option<String>,
    receipt: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Turn {
    id: String,
    intent: Value,
    attachment: Value,
    #[serde(default)]
    binding: Option<Binding>,
    run: Vec<Value>,
    interrupted: bool,
    #[serde(default)]
    stop_cause: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Interaction {
    id: String,
    /// The original pending artifact: the identity responses name.
    reference: Value,
    /// The latest signed artifact for this interaction.
    current: Value,
    native_id: String,
    recipient: String,
    turn: String,
    expires_at: u64,
    state: String,
    claim: Option<Prior>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    id: String,
    lineage: String,
    owner: String,
    adapter: String,
    native: Option<String>,
    #[serde(default)]
    native_instance: Option<String>,
    /// An opening whose native acknowledgment has not been committed.
    #[serde(default)]
    opening: bool,
    #[serde(default)]
    opening_run: Vec<Value>,
    #[serde(default)]
    protocol_fault: Option<Value>,
    #[serde(default)]
    closing_native: bool,
    #[serde(default)]
    close_run: Vec<Value>,
    #[serde(default)]
    configuration_run: Vec<Value>,
    #[serde(default)]
    runtime_binding: Option<Value>,
    #[serde(default)]
    replacement: Option<Value>,
    #[serde(default)]
    replacement_run: Vec<Value>,
    #[serde(default)]
    replacing: bool,
    connection: String,
    ordinal: u64,
    generation: u64,
    revision: u64,
    phase: String,
    read_only: bool,
    archived: bool,
    admission: Value,
    source: Value,
    effective: Value,
    #[serde(default)]
    requested: Option<Value>,
    projection: Value,
    active: Option<Turn>,
    turn_revision: Option<u64>,
    queue: Vec<Queued>,
    interactions: Vec<Interaction>,
    children: Vec<Value>,
    natives: Vec<Value>,
    results: Vec<Value>,
    quiescent: bool,
    pending_steers: Vec<(Value, Value)>,
    steers: Vec<Value>,
    history: Option<Value>,
    state: Option<Value>,
    import: Option<Value>,
}

impl Session {
    fn scope(&self, authority: &str) -> Value {
        json!({"authority":authority,"session":self.id,"generation":self.generation})
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub(crate) policy: Policy,
    #[serde(default)]
    pub(crate) transport: Option<crate::transport::Transport>,
    #[serde(default)]
    turn_commands: BTreeMap<String, Value>,
    adapters: BTreeMap<String, Adapter>,
    sessions: BTreeMap<String, Session>,
    priors: BTreeMap<String, Prior>,
    signers: BTreeMap<String, BTreeSet<String>>,
    declarations: BTreeMap<String, Value>,
    bound: BTreeMap<String, String>,
    #[serde(default)]
    known: BTreeMap<String, BTreeSet<String>>,
    #[serde(default)]
    public: BTreeSet<String>,
}

/// A signed private request with the exact artifacts it references.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub event: Event,
    #[serde(default)]
    pub artifacts: BTreeMap<String, Vec<u8>>,
}

pub struct SessionHost {
    pub(crate) store: Artifacts,
    pub(crate) signer: RelaySigner,
    pub(crate) secret: SecretKey,
    engines: RefCell<BTreeMap<String, Box<dyn Engine>>>,
    bindings: Box<dyn Bindings>,
    reader: RefCell<Option<ReadScope>>,
    pub(crate) runtime_clocks: RefCell<BTreeMap<String, (std::time::Instant, u64)>>,
    instance: String,
}

struct ReadScope {
    principal: String,
    known: BTreeSet<String>,
}
struct ReadGuard<'a> {
    reader: &'a RefCell<Option<ReadScope>>,
    previous: Option<ReadScope>,
}
impl Drop for ReadGuard<'_> {
    fn drop(&mut self) {
        self.reader.replace(self.previous.take());
    }
}

fn refuse(code: &str, detail: &str) -> String {
    format!("{code}: {detail}")
}

/// The shared refusal code at the front of an error, or `unavailable`.
pub fn code(error: &str) -> &'static str {
    env_host::allocator::code(error)
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

/// The fixed answer schema for engine questions in this profile.
pub fn answer_schema() -> Value {
    json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object",
        "additionalProperties":false,"required":["text"],
        "properties":{"text":{"type":"string","maxLength":1024}}})
}

fn validate_against(schema: &[u8], value: &Value) -> Result<()> {
    let digest = digest_bytes(schema);
    let closure = contracts::prepare_closure(&BTreeMap::from([(digest.clone(), schema.to_vec())]))
        .map_err(contract)?;
    contracts::validate_instance(&closure, &digest, value).map_err(contract)
}

impl SessionHost {
    fn cj_runtime_facts(
        &self,
        state: &State,
        event: &Event,
        execute: &nostr::execution::Execute,
        key: &str,
        now: u64,
    ) -> Result<(Binding, Value, Value)> {
        let book = state.transport.as_ref().ok_or("no runtime invocation")?;
        let claim = book
            .service
            .claims
            .get(key)
            .ok_or("no runtime invocation")?;
        if claim.execute_event != event.id
            || claim.fingerprint != execute.fingerprint
            || claim.outcome.is_some()
            || !claim.dispatch_intent
            || book.until.get(key).is_none_or(|until| now >= *until)
            || self
                .runtime_clocks
                .borrow()
                .get(key)
                .is_some_and(|(start, millis)| start.elapsed().as_millis() >= u128::from(*millis))
        {
            return Err("stale: original runtime execution is not active".into());
        }
        let input = book.inputs.get(key).ok_or("original input")?;
        self.cj_reader(state, &event.pubkey, Some(input))?;
        let turn = state
            .turn_commands
            .get(digest_of(input))
            .ok_or("not_admitted: original operation started no turn")?;
        let session = state
            .sessions
            .get(turn["session"].as_str().ok_or("session")?)
            .ok_or("session")?;
        let active = session
            .active
            .as_ref()
            .filter(|a| a.id == turn["turn"])
            .ok_or("stale: original native turn is no longer active")?;
        if session.generation != turn["generation"]
            || session.protocol_fault.is_some()
            || session.opening
            || session.closing_native
            || active.interrupted
            || !matches!(session.phase.as_str(), "running" | "blocked")
        {
            return Err("stale: original native turn cannot accept input".into());
        }
        let target = self.cj_target(state, &execute.payload["target"])?;
        self.native_session(session, &state.adapters[&session.adapter].engine)?;
        if target.adapter != state.adapters[&session.adapter].reference {
            return Err(
                "identity_mismatch: native adapter differs from the original execution".into(),
            );
        }
        let binding = active
            .binding
            .clone()
            .ok_or("not_admitted: original native binding is unavailable")?;
        if binding.fingerprint != execute.fingerprint || binding.grant != active.attachment {
            return Err("identity_mismatch: original native binding differs".into());
        }
        Ok((binding, self.runtime_request(session)?, target.engine))
    }

    /// Deliver a signed question answer or steering control to the exact
    /// active execution. This never starts another turn or consumes a call.
    pub fn continue_turn(&self, event: &Event, bundle: &Bundle, now: u64) -> Result<Value> {
        let opened = nostr::execution::open_environment_request(
            event,
            self.pubkey(),
            &self.secret,
            event.created_at,
            nostr::execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let nostr::execution::Body::Execute(execute) = &opened.body else {
            return Err("original execute required".into());
        };
        if bundle.artifacts.len() > 256
            || bundle.artifacts.values().map(Vec::len).sum::<usize>() > 8 * 1_048_576
        {
            return Err("limit_exceeded: session input artifacts".into());
        }
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let book = state.transport.as_ref().ok_or("no runtime invocation")?;
        let key = book
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        let claim = book
            .service
            .claims
            .get(&key)
            .ok_or("no runtime invocation")?;
        if claim.execute_event != event.id
            || claim.fingerprint != execute.fingerprint
            || now > claim.retain_until
        {
            return Err(
                "idempotency_conflict: original runtime invocation differs or expired".into(),
            );
        }
        let original = book.inputs.get(&key).ok_or("original input")?;
        self.cj_reader(&state, &opened.principal, Some(original))?;
        let session_id = state
            .turn_commands
            .get(digest_of(original))
            .ok_or("original turn")?["session"]
            .clone();
        let (actor, body, input) = self.authenticate(&mut state, &bundle.event, now)?;
        self.remember(&mut state, &actor, &input)?;
        if actor != opened.principal || body["scope"]["session"] != session_id {
            return Err("not_admitted: continuation requires the original session owner".into());
        }
        match body["v"].as_str() {
            Some("openagents.session-interaction-response.v1") => {}
            Some("openagents.session-command.v1")
                if matches!(body["action"].as_str(), Some("steer" | "interrupt")) => {}
            _ => {
                return Err(
                    "unsupported_feature: continuation cannot create or configure work".into(),
                );
            }
        }
        for (digest, bytes) in &bundle.artifacts {
            if digest_bytes(bytes) != *digest {
                return Err("identity_mismatch: continuation artifact bytes".into());
            }
            let reference = self.store.put(bytes, "application/octet-stream", None)?;
            self.remember(&mut state, &actor, &reference)?;
        }
        sess::validate(&body).map_err(contract)?;
        let cached = if body["v"] == "openagents.session-command.v1" {
            state.priors.contains_key(&format!(
                "{}:{}:{}",
                self.pubkey(),
                session_id.as_str().ok_or("session")?,
                body["command"].as_str().ok_or("command")?
            ))
        } else {
            state
                .sessions
                .get(session_id.as_str().ok_or("session")?)
                .is_some_and(|s| {
                    s.interactions
                        .iter()
                        .any(|i| same(&i.reference, &body["interaction"]) && i.claim.is_some())
                })
        };
        if cached {
            return self.submit_authenticated(state, actor, body, input, None, now, None, None);
        }
        let (binding, opening, engine) =
            self.cj_runtime_facts(&state, event, execute, &key, now)?;
        let runtime = self.runtime_use(
            &binding,
            &opened.principal,
            &opening["lease"],
            &opening["materialization"],
            &engine,
        );
        let request = env_host::runtime::RuntimeContinuation {
            runtime,
            execute_event: &event.id,
            target: &execute.payload["target"],
        };
        let mut pending = Some((state, actor, body, input));
        self.bindings
            .with_continuation(&request, now, &mut |fence, current| {
                let (state, actor, body, input) = pending
                    .take()
                    .ok_or("unknown: continuation callback cannot repeat")?;
                self.submit_authenticated(
                    state,
                    actor,
                    body,
                    input,
                    None,
                    current,
                    None,
                    Some(fence),
                )
            })
    }

    pub(crate) fn cj_observe(
        &self,
        state: &mut State,
        event: &Event,
        execute: &nostr::execution::Execute,
        key: &str,
        input: &Value,
        now: u64,
    ) -> Result<crate::transport::JobProgress> {
        let facts = self.cj_runtime_facts(state, event, execute, key, now);
        let mut progress = None;
        let result = match facts {
            Ok((binding, opening, engine)) => {
                let runtime = self.runtime_use(
                    &binding,
                    &event.pubkey,
                    &opening["lease"],
                    &opening["materialization"],
                    &engine,
                );
                let request = env_host::runtime::RuntimeContinuation {
                    runtime,
                    execute_event: &event.id,
                    target: &execute.payload["target"],
                };
                self.bindings
                    .with_continuation(&request, now, &mut |_, current| {
                        if progress.is_some() {
                            return Err("unknown: observation callback cannot repeat".into());
                        }
                        progress = Some(self.cj_progress(
                            state,
                            input,
                            &event.pubkey,
                            key,
                            true,
                            current,
                        )?);
                        Ok(Value::Null)
                    })
            }
            Err(error) => Err(error),
        };
        if result.is_err() {
            self.cj_stop(state, input, now, "policy")?;
            return self.cj_progress(state, input, &event.pubkey, key, false, now);
        }
        progress.ok_or("unknown: observation was not acknowledged".into())
    }

    pub fn initialize(root: PathBuf, secret: SecretKey, policy: Policy) -> Result<()> {
        let store = Artifacts::new(root)?;
        if store.root().join("state.json").exists() {
            return Err("conflict: session host state already exists".into());
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
    pub fn open(
        root: PathBuf,
        secret: SecretKey,
        engines: Vec<Box<dyn Engine>>,
        bindings: Box<dyn Bindings>,
    ) -> Result<Self> {
        let store = Artifacts::new(root)?;
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        let engines = engines
            .into_iter()
            .map(|e| (e.profile().name.to_string(), e))
            .collect();
        let host = Self {
            store,
            signer,
            secret,
            engines: RefCell::new(engines),
            bindings,
            reader: RefCell::new(None),
            runtime_clocks: RefCell::new(BTreeMap::new()),
            instance: token(),
        };
        host.state()?;
        Ok(host)
    }
    /// Host-key administration. A remote session controller cannot call this
    /// through the signed session-operation endpoint.
    pub fn set_policy(&self, policy: Policy) -> Result<()> {
        for key in policy.owners.iter().chain(&policy.controllers) {
            key.parse::<secp256k1::XOnlyPublicKey>()
                .map_err(|_| "malformed: policy key")?;
        }
        let _g = self.store.lock()?;
        let mut state = self.state()?;
        state.policy = policy;
        self.save(&state)
    }
    fn command_authority(&self, state: &State, signer: &str, body: &Value) -> Result<()> {
        let session = state
            .sessions
            .get(body["scope"]["session"].as_str().ok_or("session")?)
            .ok_or("not_admitted: unknown session")?;
        if body["action"] == "replace_runtime" && signer != session.owner {
            return Err("not_admitted: only the session owner may replace its runtime".into());
        }
        let own_replacement = body["action"] == "replace_runtime"
            && body["scope"]["generation"]
                .as_u64()
                .and_then(|g| g.checked_add(1))
                == Some(session.generation)
            && session
                .replacement
                .as_ref()
                .is_some_and(|r| self.store.json(r).is_ok_and(|b| b == *body));
        if body["scope"]["authority"] != self.pubkey()
            || body["scope"]["generation"] != session.generation && !own_replacement
        {
            return Err(refuse("stale", "scope generation"));
        }
        let admin_stop = state.policy.controllers.contains(signer)
            && matches!(body["action"].as_str(), Some("close" | "interrupt"));
        if !admin_stop
            && (!state.policy.owners.contains(&session.owner)
                || signer != session.owner && !state.policy.controllers.contains(signer))
        {
            return Err(refuse("not_admitted", "session control was not admitted"));
        }
        Ok(())
    }
    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }
    pub fn store(&self) -> &Artifacts {
        &self.store
    }
    pub(crate) fn state(&self) -> Result<State> {
        self.store.load(&self.signer)
    }
    pub(crate) fn save(&self, state: &State) -> Result<()> {
        self.store.save(state, &self.signer)
    }
    /// How many turns an engine instance actually started.
    pub fn started(&self, engine: &str) -> usize {
        self.engines.borrow().get(engine).map_or(0, |e| e.started())
    }
    /// Test and operator access to an engine instance.
    pub fn with_engine<T>(&self, name: &str, f: impl FnOnce(&mut dyn Engine) -> T) -> Option<T> {
        self.engines
            .borrow_mut()
            .get_mut(name)
            .map(|engine| f(engine.as_mut()))
    }

    fn read_scope(&self, state: &State, principal: &str) -> ReadScope {
        let mut known = state.known.get(principal).cloned().unwrap_or_default();
        known.extend(state.public.iter().cloned());
        ReadScope {
            principal: principal.into(),
            known,
        }
    }
    fn with_read_scope<T>(
        &self,
        scope: ReadScope,
        effect: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let previous = self.reader.replace(Some(scope));
        let _guard = ReadGuard {
            reader: &self.reader,
            previous,
        };
        effect()
    }
    fn remember(&self, state: &mut State, principal: &str, reference: &Value) -> Result<()> {
        let digest = digest_of(reference);
        if digest.is_empty() {
            return Ok(());
        }
        let known = state.known.entry(principal.into()).or_default();
        if known.len() >= 65_536 && !known.contains(digest) {
            return Err(refuse("limit_exceeded", "shared artifact retention"));
        }
        known.insert(digest.into());
        if let Some(scope) = self.reader.borrow_mut().as_mut()
            && scope.principal == principal
        {
            scope.known.insert(digest.into());
        }
        Ok(())
    }
    fn share_output(
        &self,
        state: &mut State,
        body: &Value,
        recipient: &str,
        reference: &Value,
    ) -> Result<()> {
        self.remember(state, recipient, reference)?;
        // Share only bytes this record's producer created. Following arbitrary
        // references would let a refusal or queue receipt disclose other owners'
        // cached artifacts merely because the caller named their hashes.
        let fields: &[&str] = match body["v"].as_str() {
            Some("openagents.session-native.v1") => &["payload"],
            Some("openagents.session-history.v1") => &["portable", "resume"],
            Some("openagents.session-configuration.v1") => &["enforcement"],
            Some("openagents.session-question.v1") => &["prompt", "answer_schema"],
            Some("openagents.session-turn-result.v1") => &["run", "receipt"],
            Some("openagents.projection-definition.v1") => &["parameters", "row_schema"],
            _ => &[],
        };
        for field in fields {
            self.remember(state, recipient, &body[field])?;
        }
        let array = match body["v"].as_str() {
            Some("openagents.session-control-receipt.v1") => Some("records"),
            Some("openagents.session-state.v1") => Some("queue"),
            Some("openagents.session-turn-result.v1") => Some("outputs"),
            _ => None,
        };
        if let Some(name) = array {
            for reference in body[name].as_array().ok_or("record artifact list")? {
                self.remember(state, recipient, reference)?;
            }
        }
        Ok(())
    }
    fn share_adapter(&self, state: &mut State, reference: &Value) -> Result<()> {
        fn references(value: &Value, out: &mut Vec<Value>) {
            if contracts::parse_artifact(value).is_ok() {
                out.push(value.clone());
            } else {
                match value {
                    Value::Object(fields) => fields.values().for_each(|v| references(v, out)),
                    Value::Array(items) => items.iter().for_each(|v| references(v, out)),
                    _ => {}
                }
            }
        }
        let mut queue = vec![reference.clone()];
        let mut seen = BTreeSet::new();
        while let Some(reference) = queue.pop() {
            let digest = digest_of(&reference).to_string();
            if !seen.insert(digest.clone()) {
                continue;
            }
            if seen.len() > 512 {
                return Err(refuse("limit_exceeded", "adapter artifact graph"));
            }
            let bytes = self.store.get(&reference)?;
            state.public.insert(digest);
            if let Ok(body) = contracts::parse_strict_bounded(&bytes, 1_048_576) {
                references(&body, &mut queue);
            }
        }
        Ok(())
    }

    fn sign(&self, state: &mut State, body: &Value, recipient: &str, now: u64) -> Result<Value> {
        if body["v"].as_str().is_some_and(|v| {
            v.starts_with("openagents.session")
                || v.starts_with("openagents.turn-")
                || v.starts_with("openagents.queued-")
        }) {
            sess::validate(body).map_err(contract)?;
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
        // The same bytes may be signed by several principals; each request
        // is attributed to its own verified signer.
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
        Ok((opened.signer().into(), body, reference))
    }

    fn bytes(&self, reference: &Value) -> Result<Vec<u8>> {
        if self
            .reader
            .borrow()
            .as_ref()
            .is_some_and(|scope| !scope.known.contains(digest_of(reference)))
        {
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
        if schema.starts_with("openagents.environment-") {
            return nostr::environment::verify(reference, &bytes, schema).map_err(contract);
        }
        sess::verify(reference, &bytes, schema).map_err(contract)
    }

    /// Admit one engine adapter by its exact profile and feature matrix.
    pub fn admit_adapter(&self, engine: &str, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let profile: Profile = self
            .engines
            .borrow()
            .get(engine)
            .map(|e| e.profile())
            .ok_or_else(|| refuse("unavailable", "no such engine adapter"))?;
        let host = self.pubkey().to_string();
        let definition = crate::interface::definition(&self.store, &host, &profile, None)?;
        let mut features = Vec::new();
        for (name, support) in &profile.features {
            if *support == Support::Unsupported {
                features.push(
                    json!({"feature":name,"support":"unsupported","operation":null,
                    "semantics":null,"evidence":null,"limitations":[]}),
                );
                continue;
            }
            let operation = crate::interface::definition(&self.store, &host, &profile, Some(name))?;
            let capability = contracts::parse_strict(&self.store.get(&operation["artifact"])?)
                .map_err(contract)?;
            let semantics = capability["input"].clone();
            let evidence = if *name == "steer" {
                let s = profile.steering;
                self.store
                    .put_json(&json!({"v":"openagents.session-steering.v1","requires":[],
                    "native":s.native,"emulation":s.emulation,"acknowledgment":s.acknowledgment}))?
            } else {
                self.store.put_json(
                    &json!({"adapter":profile.name,"feature":name,"support":support.as_str()}),
                )?
            };
            let limitations: Vec<String> = match (*name, support) {
                ("close", _) => vec!["stop the active turn before closing; an unknown close cannot release the resume binding".into()],
                ("queue" | "reorder", Support::Emulated) => {
                    vec!["host-held FIFO; the engine has no native queue".into()]
                }
                ("import", Support::Emulated) => {
                    vec!["read-only foreign history; no native resume".into()]
                }
                ("steer", Support::Emulated) => vec![
                    "cancel_and_continue: interrupts, then the next started turn carries the correction"
                        .into(),
                ],
                _ => Vec::new(),
            };
            features.push(json!({"feature":name,"support":support.as_str(),
                "operation":operation,
                "semantics":semantics,"evidence":evidence,"limitations":limitations}));
        }
        let protocol = self.store.put(&profile.protocol, SCHEMA_MEDIA_TYPE, None)?;
        let compatibility = self
            .store
            .put_json(&json!({"engine":digest_bytes(&profile.engine),
            "protocol":digest_bytes(&profile.protocol),"verified":"in-process fixture"}))?;
        let body = json!({"v":"openagents.session-adapter.v1","requires":[],"adapter":definition,
            "engine":self.store.put(&profile.engine,"application/octet-stream",None)?,
            "protocol":protocol,"features":features,"compatibility":compatibility});
        let reference = self.sign(&mut state, &body, &host, now)?;
        self.store.put(
            &serde_json::to_vec(&profile.settings_schema).map_err(|e| e.to_string())?,
            SCHEMA_MEDIA_TYPE,
            None,
        )?;
        state.adapters.insert(
            digest_of(&reference).into(),
            Adapter {
                reference: reference.clone(),
                engine: engine.into(),
                definition,
            },
        );
        self.share_adapter(&mut state, &reference)?;
        self.save(&state)?;
        Ok(reference)
    }

    fn support(&self, state: &State, session: &Session, feature: &str) -> Result<Support> {
        let adapter = state.adapters.get(&session.adapter).ok_or("adapter")?;
        let body = self.store.json(&adapter.reference)?;
        let entry = body["features"]
            .as_array()
            .and_then(|f| f.iter().find(|f| f["feature"] == feature))
            .ok_or("feature")?;
        Ok(match entry["support"].as_str() {
            Some("native") => Support::Native,
            Some("emulated") => Support::Emulated,
            _ => Support::Unsupported,
        })
    }
    fn require(&self, state: &State, session: &Session, feature: &str) -> Result<Support> {
        let support = self.support(state, session, feature)?;
        if support == Support::Unsupported {
            return Err(refuse("unsupported_feature", feature));
        }
        Ok(support)
    }

    fn check_configuration(&self, adapter: &Adapter, reference: &Value) -> Result<Value> {
        let config = self.body(reference, "openagents.session-configuration.v1")?;
        if config["bounds"] != json!([]) {
            return Err("cannot_enforce: session configuration bounds are unsupported".into());
        }
        if config["adapter"] != adapter.definition {
            return Err(refuse("identity_mismatch", "configuration adapter"));
        }
        let profile = self
            .engines
            .borrow()
            .get(&adapter.engine)
            .map(|e| e.profile())
            .ok_or("engine")?;
        let schema = serde_json::to_vec(&profile.settings_schema).map_err(|e| e.to_string())?;
        if digest_of(&config["settings_schema"]) != digest_bytes(&schema) {
            return Err(refuse(
                "incompatible",
                "settings schema is not the adapter's",
            ));
        }
        let settings = contracts::parse_strict_bounded(&self.bytes(&config["settings"])?, 65_536)
            .map_err(contract)?;
        // No unsupported setting is dropped or defaulted: the closed schema refuses it.
        validate_against(&schema, &settings)
            .map_err(|_| refuse("unsupported_feature", "a requested setting is unsupported"))?;
        for field in ["recipient", "model", "policy"] {
            self.bytes(&config[field])?;
        }
        self.engines
            .borrow()
            .get(&adapter.engine)
            .ok_or("engine")?
            .check_configuration(&self.configuration_values(&config)?)?;
        Ok(config)
    }

    fn configuration_values(&self, config: &Value) -> Result<Configuration> {
        Ok(Configuration {
            settings: contracts::parse_strict_bounded(&self.bytes(&config["settings"])?, 65_536)
                .map_err(contract)?,
            model: self.bytes(&config["model"])?,
            recipient: self.bytes(&config["recipient"])?,
            policy: self.bytes(&config["policy"])?,
        })
    }

    fn runtime_request(&self, session: &Session) -> Result<Value> {
        if let Some(binding) = &session.runtime_binding {
            return self.store.json(binding);
        }
        let admission = self.store.json(&session.admission)?;
        self.store.json(&admission["request"])
    }

    fn native_session<'a>(&self, session: &'a Session, engine: &str) -> Result<&'a str> {
        let native = session
            .native
            .as_deref()
            .ok_or("unknown: native session is unavailable")?;
        if session.native_instance.as_deref() != Some(self.instance.as_str())
            || !self
                .engines
                .borrow()
                .get(engine)
                .is_some_and(|e| e.owns(native))
        {
            return Err(
                "unknown: the original native binding belongs to another engine instance".into(),
            );
        }
        Ok(native)
    }

    fn effective(
        &self,
        state: &mut State,
        requested: &Value,
        owner: &str,
        now: u64,
    ) -> Result<Value> {
        let mut body = self.store.json(requested)?;
        let enforcement = self
            .store
            .put_json(&json!({"host":self.pubkey(),"bounds":body["bounds"],
            "mechanism":"session-host","requested":digest_of(requested)}))?;
        body["enforcement"] = enforcement;
        self.sign(state, &body, owner, now)
    }

    fn run_record(
        &self,
        run: &mut Vec<Value>,
        run_id: &str,
        kind: &str,
        detail: Value,
        now: u64,
    ) -> Result<Value> {
        let mut data = match kind {
            "created" => json!({"owner":self.pubkey(),"request":detail["request"],"base":null,
                "program":"session","lock":null,"context":null,"policy":null,"parent":null,"recipients":[]}),
            "admitted" => {
                json!({"enforcement":null,"reservations":[],"deadlines":{},"retention":{},"input":detail["request"]})
            }
            "dispatched" => {
                json!({"binding":detail["binding"],"attempt":1,"input":detail["request"],
                "context":null,"effect":"engine","generation":0})
            }
            "resolved" => {
                json!({"outcome":detail["outcome"],"dispatched":detail["dispatched"],"output":null,
                "receipts":[],"usage":{},"verification":"not_run","integration":"not_requested"})
            }
            "unknown" => json!({"attempt":1,"reservation":null,"evidence":[]}),
            _ => json!({"evidence":[],"sources":[],"complete":true,"provenance":"session-host"}),
        };
        data["session"] = detail;
        let previous = run.last().map(|b| b["digest"].clone());
        let record = json!({"run":run_id,"seq":run.len(),"previous":previous,"controller":self.pubkey(),
            "generation":0,"type":kind,"subject":{"step":null,"iteration":null,"attempt":null},"time":now,"data":data});
        let digest = nostr::run::logical_digest(&record).map_err(contract)?;
        let body =
            json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"record":record,"digest":digest});
        let mut journal = nostr::run::Journal::default();
        for body in run.iter().chain(std::iter::once(&body)) {
            let record = nostr::run::parse_envelope(body).map_err(contract)?;
            if nostr::run::ingest(&mut journal, record, self.pubkey()).map_err(contract)?
                != nostr::run::Ingest::Applied
            {
                return Err("conflict: RUN did not extend its exact history".into());
            }
        }
        let reference = self.store.put_json(&body)?;
        run.push(body);
        Ok(reference)
    }

    /// Retain native captures as private envelopes.
    fn capture(&self, state: &mut State, session: &mut Session, now: u64) -> Result<Vec<Value>> {
        let adapter = state
            .adapters
            .get(&session.adapter)
            .ok_or("adapter")?
            .clone();
        let captures: Vec<Capture> = self
            .engines
            .borrow_mut()
            .get_mut(&adapter.engine)
            .map(|e| e.captures())
            .unwrap_or_default();
        self.retain_captures(state, session, captures, now)
    }

    fn retain_captures(
        &self,
        state: &mut State,
        session: &mut Session,
        captures: Vec<Capture>,
        now: u64,
    ) -> Result<Vec<Value>> {
        let adapter = state
            .adapters
            .get(&session.adapter)
            .ok_or("adapter")?
            .clone();
        let description = self.store.json(&adapter.reference)?;
        let mut refs = Vec::new();
        for capture in captures {
            let payload = match &capture.payload {
                Some(bytes) => Some(self.store.put(bytes, "application/json", None)?),
                None => None,
            };
            let body = json!({"v":"openagents.session-native.v1","requires":[],
                "scope":session.scope(self.pubkey()),"adapter":adapter.definition,
                "engine":description["engine"],"protocol":description["protocol"],
                "connection":session.connection,"direction":capture.direction,"ordinal":session.ordinal,
                "method":capture.method,"native_id":capture.native_id,"captured_at":now,
                "payload":payload,"capture":if capture.payload.is_some(){"complete"}else{"unavailable"}});
            session.ordinal += 1;
            let reference = self.sign(state, &body, &session.owner.clone(), now)?;
            session.natives.push(reference.clone());
            refs.push(reference);
        }
        Ok(refs)
    }

    fn cut(&self, state: &mut State, session: &Session, now: u64) -> Result<Value> {
        let body = json!({"v":"openagents.projection-cut.v1","requires":[],"projection":session.projection,
            "epoch":session.lineage,"sequence":session.revision,"frontiers":[],
            "consistency":"single_authority","coverage":"complete","captured_at":now});
        nostr::workspace::validate(&body).map_err(contract)?;
        self.sign(state, &body, &session.owner.clone(), now)
    }

    /// Export the session's history: exact native captures, a derived
    /// portable view, explicit losses, and opaque resume state if supported.
    fn export_history(&self, state: &mut State, session: &mut Session, now: u64) -> Result<Value> {
        if let Some(import) = session.import.clone() {
            return Ok(import);
        }
        let adapter = state
            .adapters
            .get(&session.adapter)
            .ok_or("adapter")?
            .clone();
        let description = self.store.json(&adapter.reference)?;
        let portable = self
            .store
            .put_json(&json!({"session":session.id,"turns":session.results,"protocol_fault":session.protocol_fault,
                "meta":{"runtime":session.runtime_binding,"replacement_pending":session.replacing}}))?;
        let resume = match (&session.native, self.support(state, session, "resume")?) {
            (Some(native), Support::Native)
                if self.native_session(session, &adapter.engine).is_ok() =>
            {
                self.engines
                    .borrow_mut()
                    .get_mut(&adapter.engine)
                    .and_then(|e| e.resume_state(native))
                    .map(|bytes| self.store.put(&bytes, "application/json", None))
                    .transpose()?
            }
            _ => None,
        };
        let mut losses = vec![json!({"source":portable,"category":"derived",
            "reason":"the portable view is derived from retained turn results"})];
        if resume.is_none() {
            losses.push(json!({"source":null,"category":"unavailable","reason":"no resume state"}));
        }
        let cut = self.cut(state, session, now)?;
        let body = json!({"v":"openagents.session-history.v1","requires":[],"lineage":session.lineage,
            "source":{"kind":"managed","admission":session.admission},"adapter":adapter.definition,
            "engine":description["engine"],"cut":cut,"native":session.natives,"portable":portable,
            "losses":losses,"resume":resume,"coverage":"complete"});
        let reference = self.sign(state, &body, &session.owner.clone(), now)?;
        session.history = Some(reference.clone());
        Ok(reference)
    }

    fn write_state(&self, state: &mut State, session: &mut Session, now: u64) -> Result<Value> {
        let history = self.export_history(state, session, now)?;
        let queue: Vec<Value> = session
            .queue
            .iter()
            .map(|q| {
                self.store
                    .put_json(&json!({"v":"openagents.queued-intent.v1","requires":[],
                    "intent":q.intent,"state":q.state,"turn":q.turn,"receipt":q.receipt}))
            })
            .collect::<Result<_>>()?;
        let interactions: Vec<Value> = session
            .interactions
            .iter()
            .map(|i| i.current.clone())
            .collect();
        let body = json!({"v":"openagents.session-state.v1","requires":[],"admission":session.admission,
            "scope":session.scope(self.pubkey()),"revision":session.revision,"effective":session.effective,
            "phase":session.phase,"active_turn":session.active.as_ref().map(|t| t.id.clone()),
            "turn_revision":session.active.as_ref().and(session.turn_revision),"queue":queue,
            "interactions":interactions,"history":history,"children":session.children,"archived":session.archived,
            "meta":{"runtime":session.runtime_binding,"replacement_pending":session.replacing}});
        let reference = self.sign(state, &body, &session.owner.clone(), now)?;
        session.state = Some(reference.clone());
        Ok(reference)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn control_result(
        &self,
        state: &mut State,
        recipient: &str,
        command: &Value,
        status: &str,
        receipt: Option<Value>,
        reason: Option<&str>,
        now: u64,
    ) -> Result<Value> {
        let body = json!({"v":"openagents.session-control-result.v1","requires":[],"command":command,
            "status":status,"receipt":receipt,"reason":reason});
        self.sign(state, &body, recipient, now)?;
        Ok(body)
    }

    #[allow(clippy::too_many_arguments)]
    fn receipt(
        &self,
        state: &mut State,
        session: &Session,
        command: &Value,
        before: u64,
        transition: &str,
        records: Vec<Value>,
        now: u64,
    ) -> Result<Value> {
        let body = json!({"v":"openagents.session-control-receipt.v1","requires":[],"command":command,
            "scope":session.scope(self.pubkey()),"before_revision":before,"after_revision":session.revision,
            "transition":transition,"admitted_at":now,"records":records});
        self.sign(state, &body, &session.owner.clone(), now)
    }

    /// Admit one signed open, command, or interaction response.
    pub fn submit(&self, bundle: &Bundle, binding: Option<&Binding>, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        if bundle.artifacts.len() > 256
            || bundle.artifacts.values().map(Vec::len).sum::<usize>() > 8 * 1_048_576
        {
            return Err(refuse("limit_exceeded", "session input artifacts"));
        }
        let (signer, body, input) = self.authenticate(&mut state, &bundle.event, now)?;
        self.remember(&mut state, &signer, &input)?;
        for (digest, bytes) in &bundle.artifacts {
            if digest_bytes(bytes) != *digest {
                return Err(refuse("identity_mismatch", "attached artifact digest"));
            }
            let reference = self.store.put(bytes, "application/octet-stream", None)?;
            self.remember(&mut state, &signer, &reference)?;
        }
        self.submit_authenticated(state, signer, body, input, binding, now, None, None)
    }

    fn submit_authenticated(
        &self,
        mut state: State,
        signer: String,
        body: Value,
        input: Value,
        binding: Option<&Binding>,
        now: u64,
        fence: Option<&env_host::runtime::RuntimeFence<'_>>,
        continuation: Option<&env_host::runtime::ContinuationFence<'_>>,
    ) -> Result<Value> {
        let scope = self.read_scope(&state, &signer);
        self.with_read_scope(scope, || {
            if let Err(e) = sess::validate(&body) {
                let result = self.control_result(
                    &mut state,
                    &signer,
                    &input,
                    "refused",
                    None,
                    Some(e.code.as_str()),
                    now,
                )?;
                self.save(&state)?;
                return Ok(result);
            }
            let outcome = match body["v"].as_str() {
                Some("openagents.session-open.v1") => {
                    self.open_session(&mut state, &signer, &body, &input, binding, now, fence)
                }
                Some("openagents.session-command.v1") => self.command(
                    &mut state,
                    &signer,
                    &body,
                    &input,
                    binding,
                    now,
                    fence,
                    continuation,
                ),
                Some("openagents.session-interaction-response.v1") => {
                    self.respond(&mut state, &signer, &body, &input, now, continuation)
                }
                _ => Err(refuse("unsupported_feature", "operation")),
            };
            let result = match outcome {
                Ok(result) => result,
                Err(error) => {
                    let mut fresh = self.state()?;
                    fresh.signers.extend(state.signers);
                    fresh.declarations.extend(state.declarations);
                    for (principal, known) in state.known {
                        fresh.known.entry(principal).or_default().extend(known);
                    }
                    state = fresh;
                    let pending_key = match body["v"].as_str() {
                        Some("openagents.session-command.v1")
                            if self.command_authority(&state, &signer, &body).is_ok() =>
                        {
                            Some(format!(
                                "{}:{}:{}",
                                body["scope"]["authority"].as_str().unwrap_or_default(),
                                body["scope"]["session"].as_str().unwrap_or_default(),
                                body["command"].as_str().unwrap_or_default()
                            ))
                        }
                        Some("openagents.session-open.v1")
                            if body["owner"] == signer
                                && body["authority"] == self.pubkey()
                                && state.policy.owners.contains(&signer) =>
                        {
                            Some(format!(
                                "{}:{}:{}",
                                self.pubkey(),
                                body["session"].as_str().unwrap_or_default(),
                                body["command"].as_str().unwrap_or_default()
                            ))
                        }
                        _ => None,
                    };
                    if let Some(key) = pending_key {
                        if let Some(prior) = state.priors.get(&key).filter(|p| {
                            p.fingerprint == digest_of(&input) && p.result["status"] == "unknown"
                        }) {
                            let result = prior.result.clone();
                            self.save(&state)?;
                            return Ok(result);
                        }
                    }
                    let status = if matches!(code(&error), "conflict" | "idempotency_conflict") {
                        "conflict"
                    } else {
                        "refused"
                    };
                    self.control_result(
                        &mut state,
                        &signer,
                        &input,
                        status,
                        None,
                        Some(code(&error)),
                        now,
                    )?
                }
            };
            self.save(&state)?;
            Ok(result)
        })
    }

    fn prior(state: &State, key: &str, fingerprint: &str) -> Result<Option<Value>> {
        match state.priors.get(key) {
            Some(prior) if prior.fingerprint == fingerprint => {
                let mut result = prior.result.clone();
                if result["status"] == "accepted" {
                    result["status"] = json!("duplicate");
                }
                Ok(Some(result))
            }
            Some(_) => Err(refuse(
                "idempotency_conflict",
                "same command ID, different bytes",
            )),
            None => Ok(None),
        }
    }

    fn need_binding<'a>(&self, binding: Option<&'a Binding>) -> Result<&'a Binding> {
        binding.ok_or_else(|| {
            refuse(
                "not_admitted",
                "execution needs a current environment attachment",
            )
        })
    }
    fn runtime_use<'a>(
        &'a self,
        binding: &'a Binding,
        controller: &'a str,
        lease: &'a Value,
        materialization: &'a Value,
        engine: &'a Value,
    ) -> env_host::runtime::RuntimeUse<'a> {
        env_host::runtime::RuntimeUse {
            attachment: &binding.attachment,
            fingerprint: &binding.fingerprint,
            grant: &binding.grant,
            controller,
            executor: self.pubkey(),
            lease,
            materialization,
            engine,
        }
    }
    fn check_binding(
        &self,
        request: &env_host::runtime::RuntimeUse<'_>,
        now: u64,
        fence: Option<&env_host::runtime::RuntimeFence<'_>>,
    ) -> Result<Value> {
        if let Some(fence) = fence {
            fence.with_current(request, now, &mut || Ok(()))?;
        } else {
            self.bindings.with_current(request, now, &mut || Ok(()))?;
        }
        Ok(request.grant.clone())
    }

    fn require_engine_execution(
        &self,
        engine: &str,
        fence: Option<&env_host::runtime::RuntimeFence<'_>>,
    ) -> Result<()> {
        let profile = self.engines.borrow().get(engine).ok_or("engine")?.profile();
        if fence.is_none()
            && (!self.bindings.local_fixture()
                || profile.effects["process"] == true
                || profile.effects["delegates"] == true
                || profile.effects["spend"] == true
                || profile.effects["network"]
                    .as_array()
                    .is_none_or(|v| !v.is_empty()))
        {
            return Err("not_admitted: this engine requires charged runtime execution".into());
        }
        Ok(())
    }

    fn check_continuation(
        &self,
        state: &State,
        session: &Session,
        fence: Option<&env_host::runtime::ContinuationFence<'_>>,
        now: u64,
    ) -> Result<()> {
        let adapter = &state.adapters[&session.adapter];
        self.native_session(session, &adapter.engine)?;
        let Some(fence) = fence else {
            return self.require_engine_execution(&adapter.engine, None);
        };
        let active = session.active.as_ref().ok_or("stale: no active turn")?;
        let opening = self.runtime_request(session)?;
        let engine = self.store.json(&adapter.reference)?["engine"].clone();
        let binding = active
            .binding
            .as_ref()
            .ok_or("not_admitted: original runtime binding is unavailable")?;
        let request = self.runtime_use(
            &binding,
            &session.owner,
            &opening["lease"],
            &opening["materialization"],
            &engine,
        );
        fence.with_current(&request, now, &mut || Ok(()))
    }

    fn pending_control(
        &self,
        state: &mut State,
        session: &Session,
        actor: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<()> {
        let result = self.control_result(
            state,
            actor,
            input,
            "unknown",
            None,
            Some("unavailable"),
            now,
        )?;
        state.priors.insert(
            format!(
                "{}:{}:{}",
                self.pubkey(),
                session.id,
                body["command"].as_str().ok_or("command")?
            ),
            Prior {
                fingerprint: digest_of(input).into(),
                result,
            },
        );
        let mut pending = session.clone();
        pending.phase = "unknown".into();
        state.sessions.insert(session.id.clone(), pending);
        self.save(state)
    }

    fn runtime_effect(
        &self,
        request: &env_host::runtime::RuntimeUse<'_>,
        now: u64,
        fence: Option<&env_host::runtime::RuntimeFence<'_>>,
        engine: &str,
        effect: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        if let Some(fence) = fence {
            return fence.with_current(request, now, effect);
        }
        self.require_engine_execution(engine, fence)?;
        self.bindings.with_current(request, now, effect)
    }

    #[allow(clippy::too_many_arguments)]
    fn open_session(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        binding: Option<&Binding>,
        now: u64,
        fence: Option<&env_host::runtime::RuntimeFence<'_>>,
    ) -> Result<Value> {
        let started = std::time::Instant::now();
        let key = format!(
            "{}:{}:{}",
            body["authority"].as_str().unwrap_or_default(),
            body["session"].as_str().unwrap_or_default(),
            body["command"].as_str().unwrap_or_default()
        );
        if body["authority"] != self.pubkey() {
            return Err(refuse("not_admitted", "open names another authority"));
        }
        if body["owner"] != signer || !state.policy.owners.contains(signer) {
            return Err(refuse(
                "not_admitted",
                "owner is not authorized by this host",
            ));
        }
        if let Some(result) = Self::prior(state, &key, digest_of(input))? {
            return Ok(result);
        }
        if sess::number(body, "expires_at").map_err(contract)? <= now {
            return Err(refuse("stale", "open request expired"));
        }
        let id = body["session"].as_str().unwrap_or_default().to_string();
        if state.sessions.contains_key(&id) {
            return Err(refuse("conflict", "session ID already used"));
        }
        if state.sessions.len() >= MAX_SESSIONS {
            return Err(refuse("limit_exceeded", "session retention limit"));
        }
        let adapter = state
            .adapters
            .get(digest_of(&body["adapter"]))
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "adapter is not admitted on this host"))?;
        let kind = body["source"]["kind"].as_str().unwrap_or_default();
        let feature = match kind {
            "new" => "create",
            other => other,
        };
        let adapter_body = self.store.json(&adapter.reference)?;
        if adapter_body["features"]
            .as_array()
            .and_then(|f| f.iter().find(|f| f["feature"] == feature))
            .is_none_or(|f| f["support"] == "unsupported")
        {
            return Err(refuse("unsupported_feature", feature));
        }
        self.body(&body["lease"], "openagents.environment-lease.v1")?;
        self.body(
            &body["materialization"],
            "openagents.environment-materialization.v1",
        )?;
        self.bytes(&body["authorization"])?;
        let requested = self.check_configuration(&adapter, &body["requested"])?;
        let configuration = self.configuration_values(&requested)?;
        let mut lineage = token();
        let mut resume = None;
        let mut import = None;
        if kind != "new" {
            let history =
                self.body(&body["source"]["artifact"], "openagents.session-history.v1")?;
            if history["adapter"] != adapter.definition && kind != "import" {
                return Err(refuse("incompatible", "history comes from another adapter"));
            }
            match kind {
                "resume" => {
                    if history["source"]["kind"] != "managed" || history["resume"].is_null() {
                        return Err(refuse("unsupported_feature", "history has no resume state"));
                    }
                    if !state
                        .signers
                        .get(digest_of(&body["source"]["artifact"]))
                        .is_some_and(|signers| signers.contains(self.pubkey()))
                        || !state.sessions.values().any(|source| {
                            source.owner == signer
                                && source.admission == history["source"]["admission"]
                                && source.lineage == history["lineage"].as_str().unwrap_or_default()
                        })
                    {
                        return Err(refuse(
                            "not_admitted",
                            "resume needs the source session owner's authority",
                        ));
                    }
                    resume = Some(self.bytes(&history["resume"])?);
                    let source_lineage = history["lineage"].as_str().unwrap_or_default();
                    if state
                        .sessions
                        .values()
                        .any(|s| s.lineage == source_lineage && s.phase != "closed")
                    {
                        return Err(refuse(
                            "conflict",
                            "the lineage is bound to an open session",
                        ));
                    }
                    lineage = source_lineage.into();
                }
                "import" => {
                    if history["source"]["kind"] != "foreign" {
                        return Err(refuse("incompatible", "import takes foreign history"));
                    }
                    self.bytes(&history["portable"])?;
                    import = Some(body["source"]["artifact"].clone());
                }
                _ => {}
            }
        }
        let binding = self.need_binding(binding)?;
        let engine = self.store.json(&adapter.reference)?["engine"].clone();
        let runtime = self.runtime_use(
            binding,
            signer,
            &body["lease"],
            &body["materialization"],
            &engine,
        );
        let grant = self.check_binding(&runtime, now, fence)?;
        self.require_engine_execution(&adapter.engine, fence)?;
        let owner = signer.to_string();
        let mut run = Vec::new();
        let opening_run = token();
        let created = self.run_record(
            &mut run,
            &opening_run,
            "created",
            json!({"request":digest_of(input)}),
            now,
        )?;
        self.run_record(
            &mut run,
            &opening_run,
            "admitted",
            json!({"request":digest_of(input)}),
            now,
        )?;
        let dispatched = self.run_record(
            &mut run,
            &opening_run,
            "dispatched",
            json!({"request":digest_of(input),"binding":digest_of(&grant)}),
            now,
        )?;
        let effective = self.effective(state, &body["requested"], &owner, now)?;
        let admission = json!({"v":"openagents.session-admission.v1","requires":[],"request":input,
            "session":id,"lineage":lineage,"owner":owner,"authority":self.pubkey(),"generation":0,"revision":0,
            "attachment":grant,"adapter":body["adapter"],"requested":body["requested"],"effective":effective,
            "source":body["source"],"authorization":body["authorization"],"admitted_at":now});
        let admission_ref = self.sign(state, &admission, &owner, now)?;
        let schema = self
            .store
            .put(sess::SCHEMA_BYTES, SCHEMA_MEDIA_TYPE, None)?;
        let projection = json!({"v":"openagents.projection-definition.v1","requires":[],"projection":id,
            "authority":self.pubkey(),"recipient":owner,"scope":admission_ref,"policy":body["authorization"],
            "authorization":body["authorization"],"transform":{"id":format!("{}:session/history",self.pubkey()),
            "artifact":schema},"parameters":self.store.put_json(&json!({"session":id}))?,"row_schema":schema,
            "sources":[],"retention":{"retain_until":now+30*86_400,"max_items":4096,"max_bytes":16*1_048_576}});
        nostr::workspace::validate(&projection).map_err(contract)?;
        let projection = self.sign(state, &projection, &owner, now)?;
        let mut session = Session {
            id: id.clone(),
            lineage,
            owner: owner.clone(),
            adapter: digest_of(&adapter.reference).into(),
            native: None,
            native_instance: None,
            opening: import.is_none(),
            opening_run: run,
            protocol_fault: None,
            closing_native: false,
            close_run: Vec::new(),
            configuration_run: Vec::new(),
            runtime_binding: None,
            replacement: None,
            replacement_run: Vec::new(),
            replacing: false,
            connection: token(),
            ordinal: 0,
            generation: 0,
            revision: 0,
            phase: if import.is_some() { "ready" } else { "unknown" }.into(),
            read_only: import.is_some(),
            archived: false,
            admission: admission_ref,
            source: body["source"].clone(),
            effective,
            requested: Some(body["requested"].clone()),
            projection,
            active: None,
            turn_revision: None,
            queue: Vec::new(),
            interactions: Vec::new(),
            children: Vec::new(),
            natives: Vec::new(),
            results: Vec::new(),
            quiescent: import.is_some(),
            pending_steers: Vec::new(),
            steers: Vec::new(),
            history: None,
            state: None,
            import,
        };
        if session.opening {
            // Save the session ID, lineage, complete dispatch history, and exact
            // pending reply before a native process or remote session can exist.
            let pending_receipt = self.receipt(
                state,
                &session,
                input,
                0,
                "open",
                vec![created.clone(), dispatched.clone()],
                now,
            )?;
            let pending = self.control_result(
                state,
                &owner,
                input,
                "unknown",
                Some(pending_receipt),
                Some("unavailable"),
                now,
            )?;
            state.priors.insert(
                key.clone(),
                Prior {
                    fingerprint: digest_of(input).into(),
                    result: pending.clone(),
                },
            );
            self.write_state(state, &mut session, now)?;
            state.sessions.insert(id.clone(), session.clone());
            self.save(state)?;
            let current = now.saturating_add(started.elapsed().as_secs());
            let mut native = None;
            let opening =
                self.runtime_effect(&runtime, current, fence, &adapter.engine, &mut || {
                    let current = now.saturating_add(started.elapsed().as_secs());
                    if body["expires_at"]
                        .as_u64()
                        .is_none_or(|expiry| expiry <= current)
                    {
                        return Err("stale: opening expired before native dispatch".into());
                    }
                    let applied = self
                        .engines
                        .borrow_mut()
                        .get_mut(&adapter.engine)
                        .ok_or("engine")?
                        .open_configured(resume.as_deref(), &configuration)?;
                    if applied.1 != configuration {
                        return Err("unknown: native configuration differs from the request".into());
                    }
                    native = Some(applied.0);
                    Ok(())
                });
            // Never turn a lost acknowledgment into a retryable refusal.
            if opening.is_err() || native.is_none() {
                self.capture(state, &mut session, now)?;
                self.write_state(state, &mut session, now)?;
                state.sessions.insert(id.clone(), session);
                return Ok(pending);
            }
            let native = native.ok_or("native acknowledgment")?;
            if state.bound.get(&native).is_some_and(|other| {
                state
                    .sessions
                    .get(other)
                    .is_some_and(|s| s.phase != "closed")
            }) {
                // The adapter has returned a binding already in use. Preserve
                // the unknown opening without changing the original binding.
                self.capture(state, &mut session, now)?;
                self.write_state(state, &mut session, now)?;
                state.sessions.insert(id.clone(), session);
                return Ok(pending);
            }
            session.native = Some(native.clone());
            session.native_instance = Some(self.instance.clone());
            session.opening = false;
            session.phase = "ready".into();
            session.quiescent = true;
            state.bound.insert(native, id.clone());
        }
        self.capture(state, &mut session, now)?;
        let receipt = self.receipt(
            state,
            &session,
            input,
            0,
            "open",
            vec![created, dispatched],
            now,
        )?;
        self.write_state(state, &mut session, now)?;
        state.sessions.insert(id, session);
        let result =
            self.control_result(state, &owner, input, "accepted", Some(receipt), None, now)?;
        state.priors.insert(
            key,
            Prior {
                fingerprint: digest_of(input).into(),
                result: result.clone(),
            },
        );
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn command(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        binding: Option<&Binding>,
        now: u64,
        fence: Option<&env_host::runtime::RuntimeFence<'_>>,
        continuation: Option<&env_host::runtime::ContinuationFence<'_>>,
    ) -> Result<Value> {
        let scope = &body["scope"];
        let key = format!(
            "{}:{}:{}",
            scope["authority"].as_str().unwrap_or_default(),
            scope["session"].as_str().unwrap_or_default(),
            body["command"].as_str().unwrap_or_default()
        );
        self.command_authority(state, signer, body)?;
        if let Some(result) = Self::prior(state, &key, digest_of(input))? {
            return Ok(result);
        }
        let id = scope["session"].as_str().unwrap_or_default().to_string();
        let mut session = state
            .sessions
            .remove(&id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let outcome = self.command_session(
            state,
            &mut session,
            signer,
            body,
            input,
            binding,
            now,
            fence,
            continuation,
        );
        state.sessions.insert(id, session);
        let result = outcome?;
        state.priors.insert(
            key,
            Prior {
                fingerprint: digest_of(input).into(),
                result: result.clone(),
            },
        );
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn command_session(
        &self,
        state: &mut State,
        session: &mut Session,
        signer: &str,
        body: &Value,
        input: &Value,
        binding: Option<&Binding>,
        now: u64,
        fence: Option<&env_host::runtime::RuntimeFence<'_>>,
        continuation: Option<&env_host::runtime::ContinuationFence<'_>>,
    ) -> Result<Value> {
        if body["scope"]["authority"] != self.pubkey()
            || body["scope"]["generation"] != session.generation
        {
            return Err(refuse("stale", "scope generation"));
        }
        if signer != session.owner && !state.policy.controllers.contains(signer) {
            return Err(refuse("not_admitted", "session control"));
        }
        if sess::number(body, "expires_at").map_err(contract)? <= now {
            return Err(refuse("stale", "command expired"));
        }
        self.bytes(&body["authorization"])?;
        if body["expected_revision"] != session.revision {
            return Err(refuse("conflict", "stale session revision"));
        }
        let action = body["action"].as_str().unwrap_or_default().to_string();
        let payload = &body["payload"];
        if session.opening && action != "archive" {
            return Err(refuse("unavailable", "native opening is still unknown"));
        }
        if session.closing_native && action != "archive" {
            return Err(refuse("unavailable", "native close is still unknown"));
        }
        if session.protocol_fault.is_some()
            && !matches!(
                action.as_str(),
                "interrupt" | "close" | "archive" | "cancel_queued" | "replace_runtime"
            )
        {
            return Err(refuse(
                "unavailable",
                "the native protocol needs operator attention",
            ));
        }
        if session.phase == "closed" && action != "archive" {
            return Err(refuse("unavailable", "session is closed"));
        }
        if session.read_only
            && matches!(
                action.as_str(),
                "submit" | "enqueue" | "steer" | "interrupt" | "configure" | "replace_runtime"
            )
        {
            return Err(refuse("not_admitted", "imported history is read-only"));
        }
        let before = session.revision;
        let mut records = Vec::new();
        match action.as_str() {
            "submit" => {
                self.require(state, session, "submit")?;
                if session.active.is_some()
                    || session.phase != "ready"
                    || session.queue.iter().any(|q| q.state == "queued")
                {
                    return Err(refuse(
                        "conflict",
                        "submit needs an idle session with no queued intent",
                    ));
                }
                if !session.quiescent {
                    return Err(refuse("conflict", "the prior turn is not quiescent"));
                }
                let intent = self.intent(session, &payload["intent"], now)?;
                let _ = intent;
                records = self.start_turn(
                    state,
                    session,
                    &payload["intent"],
                    binding,
                    now,
                    Some((signer, input)),
                    fence,
                )?;
            }
            "enqueue" => {
                self.require(state, session, "queue")?;
                let intent = self.intent(session, &payload["intent"], now)?;
                if session.queue.len() >= MAX_QUEUE {
                    return Err(refuse("limit_exceeded", "queue limit"));
                }
                let id = intent["intent"].as_str().unwrap_or_default().to_string();
                if session.queue.iter().any(|q| q.id == id) {
                    return Err(refuse("conflict", "intent already queued"));
                }
                session.queue.push(Queued {
                    id,
                    requester: signer.into(),
                    intent: payload["intent"].clone(),
                    state: "queued".into(),
                    turn: None,
                    receipt: input.clone(),
                });
            }
            "cancel_queued" => {
                self.require(state, session, "queue")?;
                let item = session
                    .queue
                    .iter_mut()
                    .find(|q| {
                        q.id == payload["intent"].as_str().unwrap_or_default()
                            && q.state == "queued"
                    })
                    .ok_or_else(|| refuse("conflict", "no such queued intent"))?;
                item.state = "cancelled".into();
            }
            "reorder" => {
                self.require(state, session, "reorder")?;
                let wanted: Vec<&str> = payload["intents"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                let queued: Vec<&Queued> = session
                    .queue
                    .iter()
                    .filter(|q| q.state == "queued")
                    .collect();
                let mut a: Vec<&str> = queued.iter().map(|q| q.id.as_str()).collect();
                let mut b = wanted.clone();
                a.sort_unstable();
                b.sort_unstable();
                if a != b {
                    return Err(refuse(
                        "conflict",
                        "reorder must name the exact remaining queue",
                    ));
                }
                let mut rest: Vec<Queued> = session
                    .queue
                    .iter()
                    .filter(|q| q.state != "queued")
                    .cloned()
                    .collect();
                for id in wanted {
                    rest.push(
                        session
                            .queue
                            .iter()
                            .find(|q| q.id == id)
                            .cloned()
                            .ok_or("queue")?,
                    );
                }
                session.queue = rest;
            }
            "steer" => {
                let support = self.require(state, session, "steer")?;
                let turn = self.active_turn(session, payload)?;
                let correction = self.body(
                    &payload["correction"],
                    "openagents.session-steer-correction.v1",
                )?;
                let content = self.bytes(&correction["correction"])?;
                let adapter = state
                    .adapters
                    .get(&session.adapter)
                    .ok_or("adapter")?
                    .clone();
                self.check_continuation(state, session, continuation, now)?;
                let steering = self
                    .engines
                    .borrow()
                    .get(&adapter.engine)
                    .ok_or("engine")?
                    .profile()
                    .steering;
                let native = self.native_session(session, &adapter.engine)?.to_string();
                let receipt = match correction["mode"].as_str() {
                    Some("native") => {
                        if support != Support::Native || steering.native != "mid_turn" {
                            return Err(refuse("unsupported_feature", "native mid-turn steering"));
                        }
                        self.pending_control(state, session, signer, body, input, now)?;
                        self.check_continuation(state, session, continuation, now)?;
                        let replayed = self
                            .engines
                            .borrow_mut()
                            .get_mut(&adapter.engine)
                            .ok_or("engine")?
                            .steer(&native, &turn, &content)?;
                        let evidence = self.capture(state, session, now)?;
                        json!({"v":"openagents.session-steer-receipt.v1","requires":[],"scope":session.scope(self.pubkey()),
                            "turn":turn,"correction":payload["correction"],"mode":"native",
                            "acknowledgment":if replayed {steering.acknowledgment} else {"input_written"},
                            "consumed":replayed && steering.acknowledgment == "engine_replay","evidence":evidence})
                    }
                    Some("emulated") => {
                        if steering.emulation != Some("cancel_and_continue") {
                            return Err(refuse(
                                "unsupported_feature",
                                "this adapter has no steering emulation",
                            ));
                        }
                        self.pending_control(state, session, signer, body, input, now)?;
                        self.check_continuation(state, session, continuation, now)?;
                        self.engines
                            .borrow_mut()
                            .get_mut(&adapter.engine)
                            .ok_or("engine")?
                            .interrupt(&native, &turn)?;
                        if let Some(active) = session.active.as_mut() {
                            active.interrupted = true;
                        }
                        session.phase = "interrupting".into();
                        let evidence = self.capture(state, session, now)?;
                        session.pending_steers.push((
                            payload["correction"].clone(),
                            serde_json::from_slice(&content).unwrap_or(Value::Null),
                        ));
                        json!({"v":"openagents.session-steer-receipt.v1","requires":[],"scope":session.scope(self.pubkey()),
                            "turn":turn,"correction":payload["correction"],"mode":"emulated",
                            "acknowledgment":steering.acknowledgment,"consumed":false,"evidence":evidence})
                    }
                    _ => return Err(refuse("unsupported_feature", "steer mode")),
                };
                let reference = self.sign(state, &receipt, &session.owner.clone(), now)?;
                session.steers.push(reference.clone());
                records.push(reference);
                session.turn_revision = session.turn_revision.map(|r| r + 1);
            }
            "interrupt" => {
                self.require(state, session, "interrupt")?;
                let turn = self.active_turn(session, payload)?;
                let adapter = state
                    .adapters
                    .get(&session.adapter)
                    .ok_or("adapter")?
                    .clone();
                let native = self.native_session(session, &adapter.engine)?.to_string();
                self.pending_control(state, session, signer, body, input, now)?;
                self.engines
                    .borrow_mut()
                    .get_mut(&adapter.engine)
                    .ok_or("engine")?
                    .interrupt(&native, &turn)?;
                if let Some(active) = session.active.as_mut() {
                    active.interrupted = true;
                }
                session.phase = "interrupting".into();
                records.extend(self.capture(state, session, now)?);
                session.turn_revision = session.turn_revision.map(|r| r + 1);
            }
            "configure" => {
                self.require(state, session, "configure")?;
                if session.active.is_some()
                    || session.queue.iter().any(|q| q.state == "queued")
                    || session.phase != "ready"
                    || !session.quiescent
                {
                    return Err(refuse(
                        "conflict",
                        "configure needs an idle session with an empty queue",
                    ));
                }
                let adapter = state
                    .adapters
                    .get(&session.adapter)
                    .ok_or("adapter")?
                    .clone();
                let requested = self.check_configuration(&adapter, &payload["requested"])?;
                let previous = self.store.json(&session.effective)?;
                if ["model", "recipient", "policy"]
                    .iter()
                    .any(|field| requested[*field] != previous[*field])
                {
                    return Err("unsupported_feature: model, recipient, or policy changes require runtime replacement".into());
                }
                self.require_engine_execution(&adapter.engine, fence)?;
                if fence.is_some() {
                    let opening = self.runtime_request(session)?;
                    let engine = self.store.json(&adapter.reference)?["engine"].clone();
                    let runtime = self.runtime_use(
                        self.need_binding(binding)?,
                        signer,
                        &opening["lease"],
                        &opening["materialization"],
                        &engine,
                    );
                    self.check_binding(&runtime, now, fence)?;
                }
                let configuration = self.configuration_values(&requested)?;
                let native = self.native_session(session, &adapter.engine)?.to_string();
                let run_id = token();
                let mut run = Vec::new();
                records.push(self.run_record(
                    &mut run,
                    &run_id,
                    "created",
                    json!({"request":digest_of(input)}),
                    now,
                )?);
                records.push(self.run_record(
                    &mut run,
                    &run_id,
                    "admitted",
                    json!({"request":digest_of(input)}),
                    now,
                )?);
                records.push(self.run_record(
                    &mut run,
                    &run_id,
                    "dispatched",
                    json!({"request":digest_of(input),"binding":native,"operation":"configure"}),
                    now,
                )?);
                session.configuration_run = run;
                session.quiescent = false;
                self.pending_control(state, session, signer, body, input, now)?;
                let mut applied = None;
                let mut effect = || {
                    applied = Some(
                        self.engines
                            .borrow_mut()
                            .get_mut(&adapter.engine)
                            .ok_or("engine")?
                            .configure(&native, &configuration)?,
                    );
                    Ok(())
                };
                let acknowledgment = if let Some(fence) = fence {
                    let opening = self.runtime_request(session)?;
                    let engine = self.store.json(&adapter.reference)?["engine"].clone();
                    let runtime = self.runtime_use(
                        self.need_binding(binding)?,
                        signer,
                        &opening["lease"],
                        &opening["materialization"],
                        &engine,
                    );
                    fence.with_current(&runtime, now, &mut effect)
                } else {
                    effect()
                };
                records.extend(self.capture(state, session, now)?);
                if acknowledgment.is_err() || applied.as_ref() != Some(&configuration) {
                    session.phase = "unknown".into();
                    self.write_state(state, session, now)?;
                    state.sessions.insert(session.id.clone(), session.clone());
                    self.save(state)?;
                    return Err("unknown: native configuration was not acknowledged".into());
                }
                records.push(self.run_record(
                    &mut session.configuration_run,
                    &run_id,
                    "resolved",
                    json!({"outcome":"completed","dispatched":true}),
                    now,
                )?);
                session.quiescent = true;
                session.effective =
                    self.effective(state, &payload["requested"], &session.owner.clone(), now)?;
                session.requested = Some(payload["requested"].clone());
            }
            "archive" => session.archived = payload["archived"] == true,
            "replace_runtime" => {
                records =
                    self.replace_runtime(state, session, signer, body, input, binding, fence, now)?;
            }
            "close" => {
                self.require(state, session, "close")?;
                records = self.close_native(state, session, signer, body, input, now)?;
            }
            _ => return Err(refuse("unsupported_feature", "action")),
        }
        session.revision += 1;
        let receipt = self.receipt(state, session, input, before, &action, records, now)?;
        self.write_state(state, session, now)?;
        self.control_result(
            state,
            &session.owner.clone(),
            input,
            "accepted",
            Some(receipt),
            None,
            now,
        )
    }

    fn replace_runtime(
        &self,
        state: &mut State,
        session: &mut Session,
        signer: &str,
        command: &Value,
        input: &Value,
        binding: Option<&Binding>,
        fence: Option<&env_host::runtime::RuntimeFence<'_>>,
        now: u64,
    ) -> Result<Vec<Value>> {
        let started = std::time::Instant::now();
        if signer != session.owner || !state.policy.owners.contains(signer) {
            return Err("not_admitted: runtime replacement requires the current owner".into());
        }
        if session.read_only
            || session.active.is_some()
            || !session.quiescent
            || !matches!(session.phase.as_str(), "ready" | "faulted")
            || session.opening
            || session.closing_native
            || session.replacing
            || session.queue.iter().any(|q| q.state == "queued")
            || session.interactions.iter().any(|i| i.state == "pending")
        {
            return Err("conflict: runtime replacement requires an idle quiescent session".into());
        }
        for reference in &session.children {
            let child = self.store.json(reference)?;
            if child["relationship"] == "managed"
                && !state.sessions.values().any(|s| {
                    s.admission == child["child"]
                        && s.active.is_none()
                        && s.quiescent
                        && matches!(s.phase.as_str(), "ready" | "closed")
                })
            {
                return Err("unknown: managed child work is not quiescent".into());
            }
        }
        let payload = &command["payload"];
        let adapter = state
            .adapters
            .get(digest_of(&payload["adapter"]))
            .cloned()
            .ok_or("not_admitted: replacement adapter is not admitted")?;
        let old_adapter = state
            .adapters
            .get(&session.adapter)
            .cloned()
            .ok_or("adapter")?;
        let old_native = self
            .native_session(session, &old_adapter.engine)?
            .to_string();
        if state.bound.get(&old_native) != Some(&session.id) {
            return Err("conflict: old native binding belongs to another session".into());
        }
        self.require(state, session, "close")?;
        if !self
            .engines
            .borrow()
            .get(&old_adapter.engine)
            .is_some_and(|e| e.owns(&old_native))
        {
            return Err("unknown: this engine no longer owns the old native binding".into());
        }
        let description = self.store.json(&adapter.reference)?;
        let kind = payload["source"]["kind"]
            .as_str()
            .ok_or("replacement source")?;
        let feature = if kind == "new" { "create" } else { "resume" };
        if description["features"]
            .as_array()
            .and_then(|f| f.iter().find(|f| f["feature"] == feature))
            .is_none_or(|f| f["support"] == "unsupported")
        {
            return Err("unsupported_feature: replacement source is unsupported".into());
        }
        let requested = self.check_configuration(&adapter, &payload["requested"])?;
        let configuration = self.configuration_values(&requested)?;
        self.body(&payload["lease"], "openagents.environment-lease.v1")?;
        self.body(
            &payload["materialization"],
            "openagents.environment-materialization.v1",
        )?;
        if !payload["workspace"].is_null() {
            self.bytes(&payload["workspace"])?;
        }
        let mut resume = None;
        if kind == "resume" {
            let reference = &payload["source"]["artifact"];
            let history = self.body(reference, "openagents.session-history.v1")?;
            if session.history.as_ref() != Some(reference)
                || history["lineage"] != session.lineage
                || history["source"]["admission"] != session.admission
                || history["adapter"] != adapter.definition
                || history["resume"].is_null()
                || !state
                    .signers
                    .get(digest_of(reference))
                    .is_some_and(|signers| signers.contains(self.pubkey()))
            {
                return Err(
                    "not_admitted: replacement resume needs the owner's latest compatible history"
                        .into(),
                );
            }
            resume = Some(self.bytes(&history["resume"])?);
        }
        let binding = self.need_binding(binding)?;
        let runtime = self.runtime_use(
            binding,
            signer,
            &payload["lease"],
            &payload["materialization"],
            &description["engine"],
        );
        let grant = self.check_binding(&runtime, now, fence)?;
        self.require_engine_execution(&adapter.engine, fence)?;
        let next = session
            .generation
            .checked_add(1)
            .filter(|g| *g <= 9_007_199_254_740_991)
            .ok_or("limit_exceeded: runtime generation")?;
        let run_id = token();
        let mut run = Vec::new();
        let mut records = Vec::new();
        records.push(self.run_record(
            &mut run,
            &run_id,
            "created",
            json!({"request":digest_of(input)}),
            now,
        )?);
        records.push(self.run_record(
            &mut run,
            &run_id,
            "admitted",
            json!({"request":digest_of(input)}),
            now,
        )?);
        records.push(self.run_record(&mut run,&run_id,"dispatched",
            json!({"request":digest_of(input),"binding":old_native,"operation":"replace_runtime","generation":next}),now)?);
        session.replacement_run = run;
        session.replacement = Some(input.clone());
        session.replacing = true;
        session.quiescent = false;
        self.pending_control(state, session, signer, command, input, now)?;
        let closed = self
            .engines
            .borrow_mut()
            .get_mut(&old_adapter.engine)
            .ok_or("engine")?
            .close(&old_native);
        records.extend(self.capture(state, session, now)?);
        if closed.is_err() {
            session.phase = "unknown".into();
            self.write_state(state, session, now)?;
            state.sessions.insert(session.id.clone(), session.clone());
            self.save(state)?;
            return Err(
                "unknown: old native closure was not acknowledged; replacement was not opened"
                    .into(),
            );
        }
        let previous = session
            .runtime_binding
            .clone()
            .unwrap_or_else(|| session.admission.clone());
        state.bound.remove(&old_native);
        session.generation = next;
        session.native = None;
        session.native_instance = None;
        session.adapter = digest_of(&adapter.reference).into();
        session.connection = token();
        session.ordinal = 0;
        session.phase = "unknown".into();
        self.write_state(state, session, now)?;
        state.sessions.insert(session.id.clone(), session.clone());
        self.save(state)?;
        let current = now.saturating_add(started.elapsed().as_secs());
        let mut opened = None;
        let admission = self.runtime_effect(&runtime, current, fence, &adapter.engine, &mut || {
            let current = now.saturating_add(started.elapsed().as_secs());
            if command["expires_at"].as_u64().is_none_or(|v| v <= current) {
                return Err("stale: replacement expired before native opening".into());
            }
            opened = Some(
                self.engines
                    .borrow_mut()
                    .get_mut(&adapter.engine)
                    .ok_or("engine")?
                    .open_configured(resume.as_deref(), &configuration)?,
            );
            Ok(())
        });
        records.extend(self.capture(state, session, now)?);
        if admission.is_err()
            || opened
                .as_ref()
                .is_none_or(|(_, actual)| *actual != configuration)
        {
            self.write_state(state, session, now)?;
            state.sessions.insert(session.id.clone(), session.clone());
            self.save(state)?;
            return Err("unknown: replacement opening was not acknowledged".into());
        }
        let native = opened.ok_or("native acknowledgment")?.0;
        if state.bound.get(&native).is_some_and(|other| {
            other != &session.id
                && state
                    .sessions
                    .get(other)
                    .is_some_and(|s| s.phase != "closed")
        }) {
            self.write_state(state, session, now)?;
            state.sessions.insert(session.id.clone(), session.clone());
            self.save(state)?;
            return Err("unknown: replacement native binding is already held".into());
        }
        state.bound.insert(native.clone(), session.id.clone());
        session.native = Some(native);
        session.native_instance = Some(self.instance.clone());
        session.effective = self.effective(state, &payload["requested"], signer, now)?;
        session.requested = Some(payload["requested"].clone());
        records.push(self.run_record(
            &mut session.replacement_run,
            &run_id,
            "resolved",
            json!({"outcome":"completed","dispatched":true}),
            now,
        )?);
        let record = json!({"v":"openagents.session-runtime-binding.v1","requires":[],
            "scope":session.scope(self.pubkey()),"previous":previous,"request":input,"attachment":grant,
            "lease":payload["lease"],"materialization":payload["materialization"],"adapter":payload["adapter"],
            "requested":payload["requested"],"effective":session.effective,"admitted_at":now});
        let reference = self.sign(state, &record, signer, now)?;
        records.push(reference.clone());
        session.runtime_binding = Some(reference);
        session.protocol_fault = None;
        session.quiescent = true;
        session.replacing = false;
        session.phase = "ready".into();
        Ok(records)
    }

    fn close_native(
        &self,
        state: &mut State,
        session: &mut Session,
        actor: &str,
        command: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Vec<Value>> {
        if session.active.is_some() || !session.quiescent {
            return Err(refuse(
                "conflict",
                "stop the current turn before closing the session",
            ));
        }
        for item in session.queue.iter_mut().filter(|q| q.state == "queued") {
            item.state = "cancelled".into();
        }
        let Some(native) = session.native.clone() else {
            if !session.read_only {
                return Err(refuse("unavailable", "native session is unknown"));
            }
            session.phase = "closed".into();
            return Ok(Vec::new());
        };
        if state.bound.get(&native) != Some(&session.id) {
            return Err(refuse(
                "conflict",
                "native binding differs from this session",
            ));
        }
        let adapter = state
            .adapters
            .get(&session.adapter)
            .ok_or("adapter")?
            .clone();
        let mut run = Vec::new();
        self.native_session(session, &adapter.engine)?;
        let run_id = token();
        let mut records = vec![
            self.run_record(
                &mut run,
                &run_id,
                "created",
                json!({"request":digest_of(input)}),
                now,
            )?,
            self.run_record(
                &mut run,
                &run_id,
                "admitted",
                json!({"request":digest_of(input)}),
                now,
            )?,
            self.run_record(
                &mut run,
                &run_id,
                "dispatched",
                json!({"request":digest_of(input),"binding":native,"operation":"close"}),
                now,
            )?,
        ];
        session.close_run = run;
        session.closing_native = true;
        session.phase = "unknown".into();
        session.quiescent = false;
        let key = format!(
            "{}:{}:{}",
            self.pubkey(),
            session.id,
            command["command"].as_str().ok_or("command")?
        );
        let pending = self.control_result(
            state,
            actor,
            input,
            "unknown",
            None,
            Some("unavailable"),
            now,
        )?;
        state.priors.insert(
            key,
            Prior {
                fingerprint: digest_of(input).into(),
                result: pending,
            },
        );
        self.write_state(state, session, now)?;
        state.sessions.insert(session.id.clone(), session.clone());
        self.save(state)?;
        // An unknown native close is never sent again, even with a new command.
        self.engines
            .borrow_mut()
            .get_mut(&adapter.engine)
            .ok_or("engine")?
            .close(&native)?;
        records.extend(self.capture(state, session, now)?);
        records.push(self.run_record(
            &mut session.close_run,
            &run_id,
            "resolved",
            json!({"outcome":"completed","dispatched":true,"operation":"close"}),
            now,
        )?);
        session.closing_native = false;
        session.quiescent = true;
        session.phase = "closed".into();
        Ok(records)
    }

    fn intent(&self, session: &Session, reference: &Value, now: u64) -> Result<Value> {
        let intent = self.body(reference, "openagents.turn-intent.v1")?;
        if sess::number(&intent, "expires_at").map_err(contract)? <= now {
            return Err(refuse("stale", "intent expired"));
        }
        let requested = match &session.requested {
            Some(reference) => reference.clone(),
            None => self.store.json(&session.admission)?["requested"].clone(),
        };
        if !same(&intent["configuration"], &session.effective)
            && !same(&intent["configuration"], &requested)
        {
            return Err(refuse(
                "incompatible",
                "intent configuration is not the session's",
            ));
        }
        for field in ["task_frame", "input", "context", "authorization"] {
            self.bytes(&intent[field])?;
        }
        Ok(intent)
    }

    fn active_turn(&self, session: &Session, payload: &Value) -> Result<String> {
        let active = session
            .active
            .as_ref()
            .ok_or_else(|| refuse("stale", "no active turn"))?;
        if payload["turn"] != active.id.as_str() {
            return Err(refuse("stale", "turn is not active"));
        }
        if Some(
            payload["expected_turn_revision"]
                .as_u64()
                .unwrap_or(u64::MAX),
        ) != session.turn_revision
        {
            return Err(refuse("conflict", "stale turn revision"));
        }
        Ok(active.id.clone())
    }

    fn start_turn(
        &self,
        state: &mut State,
        session: &mut Session,
        intent_ref: &Value,
        binding: Option<&Binding>,
        now: u64,
        pending: Option<(&str, &Value)>,
        fence: Option<&env_host::runtime::RuntimeFence<'_>>,
    ) -> Result<Vec<Value>> {
        let started = std::time::Instant::now();
        let binding = self.need_binding(binding)?;
        let controller = pending
            .map(|(actor, _)| actor.to_string())
            .or_else(|| {
                session
                    .queue
                    .iter()
                    .find(|q| q.state == "queued" && q.intent == *intent_ref)
                    .map(|q| q.requester.clone())
            })
            .ok_or("not_admitted: original turn requester required")?;
        let opening = self.runtime_request(session)?;
        let adapter = state
            .adapters
            .get(&session.adapter)
            .ok_or("adapter")?
            .clone();
        let engine = self.store.json(&adapter.reference)?["engine"].clone();
        let runtime = self.runtime_use(
            binding,
            &controller,
            &opening["lease"],
            &opening["materialization"],
            &engine,
        );
        let grant = self.check_binding(&runtime, now, fence)?;
        self.require_engine_execution(&adapter.engine, fence)?;
        let intent = self.store.json(intent_ref)?;
        let mut input: Value =
            contracts::parse_strict_bounded(&self.bytes(&intent["input"])?, 1_048_576)
                .map_err(contract)?;
        let carried: Vec<(Value, Value)> = std::mem::take(&mut session.pending_steers);
        if !carried.is_empty() {
            input["corrections"] = Value::Array(carried.iter().map(|(_, c)| c.clone()).collect());
        }
        let turn = token();
        let mut run = Vec::new();
        let mut records = vec![
            self.run_record(
                &mut run,
                &turn,
                "created",
                json!({"request":digest_of(intent_ref)}),
                now,
            )?,
            self.run_record(
                &mut run,
                &turn,
                "admitted",
                json!({"request":digest_of(intent_ref)}),
                now,
            )?,
            self.run_record(
                &mut run,
                &turn,
                "dispatched",
                json!({"request":digest_of(intent_ref),"binding":digest_of(&grant)}),
                now,
            )?,
        ];
        let adapter = state
            .adapters
            .get(&session.adapter)
            .ok_or("adapter")?
            .clone();
        let native = self.native_session(session, &adapter.engine)?.to_string();
        session.active = Some(Turn {
            id: turn.clone(),
            intent: intent_ref.clone(),
            attachment: grant.clone(),
            binding: Some(binding.clone()),
            run: run.clone(),
            interrupted: false,
            stop_cause: None,
        });
        session.turn_revision = Some(0);
        session.phase = "running".into();
        session.quiescent = false;
        for queued in session
            .queue
            .iter_mut()
            .filter(|q| q.state == "queued" && q.intent == *intent_ref)
        {
            queued.state = "promoted".into();
            queued.turn = Some(turn.clone());
        }
        if let Some((actor, request)) = pending {
            let body = self.store.json(request)?;
            let key = format!(
                "{}:{}:{}",
                body["scope"]["authority"].as_str().ok_or("authority")?,
                session.id,
                body["command"].as_str().ok_or("command")?
            );
            let result = self.control_result(
                state,
                actor,
                request,
                "unknown",
                None,
                Some("unavailable"),
                now,
            )?;
            state.priors.insert(
                key,
                Prior {
                    fingerprint: digest_of(request).into(),
                    result,
                },
            );
        }
        if let Some((_, command)) = pending {
            state.turn_commands.insert(
                digest_of(command).into(),
                json!({
                    "session":session.id,"turn":turn,"generation":session.generation,
                }),
            );
        }
        self.write_state(state, session, now)?;
        state.sessions.insert(session.id.clone(), session.clone());
        self.save(state)?;
        // Recheck after the durable intent, holding both authorities through
        // actual native input disclosure. A read-only probe is insufficient.
        let current = now.saturating_add(started.elapsed().as_secs());
        self.runtime_effect(&runtime, current, fence, &adapter.engine, &mut || {
            let current = now.saturating_add(started.elapsed().as_secs());
            if intent["expires_at"]
                .as_u64()
                .is_none_or(|deadline| deadline <= current)
            {
                return Err("stale: turn expired before disclosure".into());
            }
            if let Some((_, request)) = pending {
                if self.store.json(request)?["expires_at"]
                    .as_u64()
                    .is_none_or(|deadline| deadline <= current)
                {
                    return Err("stale: command expired before disclosure".into());
                }
            }
            self.engines
                .borrow_mut()
                .get_mut(&adapter.engine)
                .ok_or("engine")?
                .start(
                    &native,
                    &turn,
                    &serde_json::to_vec(&input).map_err(|e| e.to_string())?,
                )
        })?;
        let evidence = self.capture(state, session, now)?;
        for (correction, _) in carried {
            let steering = self
                .engines
                .borrow()
                .get(&adapter.engine)
                .ok_or("engine")?
                .profile()
                .steering;
            let receipt = json!({"v":"openagents.session-steer-receipt.v1","requires":[],
                "scope":session.scope(self.pubkey()),"turn":turn,"correction":correction,"mode":"emulated",
                "acknowledgment":steering.acknowledgment,"consumed":steering.acknowledgment == "next_turn_start",
                "evidence":evidence});
            let reference = self.sign(state, &receipt, &session.owner.clone(), now)?;
            session.steers.push(reference.clone());
            records.push(reference);
        }
        records.extend(evidence);
        session.active = Some(Turn {
            id: turn,
            intent: intent_ref.clone(),
            attachment: grant,
            binding: Some(binding.clone()),
            run,
            interrupted: false,
            stop_cause: None,
        });
        session.turn_revision = Some(0);
        session.phase = "running".into();
        session.quiescent = false;
        Ok(records)
    }

    /// Observe the active turn once. Terminal engine evidence yields a turn
    /// result and quiescence; a lost observation yields unknown, never a rerun.
    pub fn observe(&self, session_id: &str, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut session = state
            .sessions
            .remove(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let outcome = self.observe_session(&mut state, &mut session, now);
        state.sessions.insert(session_id.into(), session.clone());
        let value = outcome?;
        self.save(&state)?;
        Ok(value)
    }

    fn observe_session(&self, state: &mut State, session: &mut Session, now: u64) -> Result<Value> {
        let Some(mut turn) = session.active.clone() else {
            if session.native.is_some()
                && !session.read_only
                && session.phase != "closed"
                && self
                    .native_session(session, &state.adapters[&session.adapter].engine)
                    .is_err()
            {
                session.phase = "unknown".into();
                session.quiescent = false;
                self.write_state(state, session, now)?;
            }
            return Ok(json!({"phase":session.phase,"state":session.state}));
        };
        let adapter = state
            .adapters
            .get(&session.adapter)
            .ok_or("adapter")?
            .clone();
        let owned = self.native_session(session, &adapter.engine).is_ok();
        let observation = if owned {
            match (
                &session.native,
                self.engines.borrow_mut().get_mut(&adapter.engine),
            ) {
                (Some(native), Some(engine)) => engine.step(native),
                _ => Observation::Lost,
            }
        } else {
            Observation::Lost
        };
        let captured = if owned {
            self.capture(state, session, now)?
        } else {
            Vec::new()
        };
        let mut result = Value::Null;
        let fault = match &observation {
            Observation::ProtocolFault { request } => {
                let refs = self.retain_captures(state, session, vec![request.clone()], now)?;
                Some((refs.last().cloned(), "unhandled_required_message"))
            }
            Observation::Question { .. }
                if self.support(state, session, "questions")? == Support::Unsupported =>
            {
                Some((captured.last().cloned(), "unsupported_question"))
            }
            _ => None,
        };
        if let Some((native_request, reason)) = fault {
            if session.protocol_fault.is_some() {
                return Ok(json!({"phase":session.phase,"state":session.state,
                    "protocol_fault":session.protocol_fault}));
            }
            let fault = self.run_record(
                &mut turn.run,
                &turn.id,
                "unknown",
                json!({"protocol_fault":{"native_request":native_request,"reason":reason},
                    "dispatched":true}),
                now,
            )?;
            session.protocol_fault = Some(fault.clone());
            session.phase = "faulted".into();
            session.quiescent = false;
            if let Some(active) = session.active.as_mut() {
                active.run = turn.run;
            }
            let result = json!({"v":"openagents.session-turn-result.v1","requires":[],
                "scope":session.scope(self.pubkey()),"turn":turn.id,"intent":turn.intent,
                "run":fault,"outcome":"unknown","dispatched":true,"cause":"protocol_fault",
                "outputs":[],"receipt":fault,"verification":"not_run",
                "integration":"not_requested","quiescence":null});
            let result = self.sign(state, &result, &session.owner.clone(), now)?;
            session.results.push(result.clone());
            session.revision += 1;
            self.write_state(state, session, now)?;
            return Ok(
                json!({"phase":session.phase,"state":session.state,"result":result,
                "protocol_fault":session.protocol_fault}),
            );
        }
        match observation {
            Observation::ProtocolFault { .. } => unreachable!("fault handled before observation"),
            Observation::Running => {
                return Ok(json!({"phase":session.phase,"state":session.state}));
            }
            Observation::Question { native_id, prompt } => {
                if session
                    .interactions
                    .iter()
                    .any(|i| i.native_id == native_id)
                {
                    return Ok(json!({"phase":session.phase,"state":session.state}));
                }
                let native_request = captured.last().cloned().ok_or("native request")?;
                let question = json!({"v":"openagents.session-question.v1","requires":[],
                        "prompt":self.store.put(&prompt,"application/json",None)?,
                        "answer_schema":self.store.put(&serde_json::to_vec(&answer_schema()).map_err(|e| e.to_string())?,SCHEMA_MEDIA_TYPE,None)?});
                let question = self.sign(state, &question, &session.owner.clone(), now)?;
                let id = token();
                let expires_at = now + 3600;
                let body = json!({"v":"openagents.session-interaction.v1","requires":[],"interaction":id,
                        "scope":session.scope(self.pubkey()),"turn":turn.id,"turn_revision":session.turn_revision.unwrap_or(0),
                        "native_request":native_request,"kind":"question","recipient":session.owner,"body":question,
                        "expires_at":expires_at,"state":"pending"});
                let reference = self.sign(state, &body, &session.owner.clone(), now)?;
                session.interactions.push(Interaction {
                    id,
                    current: reference.clone(),
                    reference,
                    native_id,
                    recipient: session.owner.clone(),
                    turn: turn.id.clone(),
                    expires_at,
                    state: "pending".into(),
                    claim: None,
                });
                session.phase = "blocked".into();
            }
            Observation::Finished {
                outcome,
                cause,
                output,
            } => {
                let cause = if outcome == "cancelled" {
                    turn.stop_cause.as_deref().unwrap_or(cause)
                } else {
                    cause
                };
                let output_ref = self.store.put(&output, "application/json", None)?;
                let resolved = self.run_record(
                    &mut turn.run,
                    &turn.id,
                    "resolved",
                    json!({"outcome":outcome,"dispatched":true}),
                    now,
                )?;
                let terminal: Vec<Value> = captured.iter().rev().take(1).cloned().collect();
                let quiescence = json!({"v":"openagents.session-quiescence.v1","requires":[],
                    "scope":session.scope(self.pubkey()),"turn":turn.id,"attachment":turn.attachment,
                    "evidence":terminal,"observed_at":now});
                let quiescence = self.sign(state, &quiescence, &session.owner.clone(), now)?;
                result = json!({"v":"openagents.session-turn-result.v1","requires":[],"scope":session.scope(self.pubkey()),
                    "turn":turn.id,"intent":turn.intent,"run":resolved,"outcome":outcome,"dispatched":true,"cause":cause,
                    "outputs":[output_ref],"receipt":resolved,"verification":"not_run","integration":"not_requested",
                    "quiescence":quiescence});
                session.quiescent = true;
                self.end_interactions(state, session, &turn.id, "cancelled", now)?;
                session.phase = if session.phase == "closing" {
                    "closing"
                } else if session.protocol_fault.is_some() {
                    "faulted"
                } else {
                    "ready"
                }
                .into();
                session.active = None;
                session.turn_revision = None;
            }
            Observation::Lost => {
                self.end_interactions(state, session, &turn.id, "unknown", now)?;
                let unknown = self.run_record(
                    &mut turn.run,
                    &turn.id,
                    "unknown",
                    json!({"lost":true}),
                    now,
                )?;
                result = json!({"v":"openagents.session-turn-result.v1","requires":[],"scope":session.scope(self.pubkey()),
                    "turn":turn.id,"intent":turn.intent,"run":unknown,"outcome":"unknown","dispatched":true,
                    "cause":"lost_observation","outputs":[],"receipt":unknown,"verification":"not_run",
                    "integration":"not_requested","quiescence":null});
                // Unknown work blocks automatic promotion and is never rerun.
                session.quiescent = false;
                session.phase = if session.protocol_fault.is_some() {
                    "faulted"
                } else {
                    "unknown"
                }
                .into();
                session.active = None;
                session.turn_revision = None;
            }
        }
        if let Some(active) = session.active.as_mut() {
            active.run = turn.run.clone();
        }
        let result_ref = if result.is_null() {
            Value::Null
        } else {
            let reference = self.sign(state, &result, &session.owner.clone(), now)?;
            session.results.push(reference.clone());
            reference
        };
        session.revision += 1;
        self.write_state(state, session, now)?;
        Ok(json!({"phase":session.phase,"state":session.state,"result":result_ref}))
    }

    fn end_interactions(
        &self,
        state: &mut State,
        session: &mut Session,
        turn: &str,
        status: &str,
        now: u64,
    ) -> Result<()> {
        for interaction in session
            .interactions
            .iter_mut()
            .filter(|i| i.turn == turn && i.state == "pending")
        {
            let mut current = self.store.json(&interaction.current)?;
            current["state"] = json!(status);
            interaction.current = self.sign(state, &current, &session.owner, now)?;
            interaction.state = status.into();
        }
        Ok(())
    }

    /// Promote the queue head after quiescence, under a fresh binding.
    pub fn promote(&self, session_id: &str, binding: Option<&Binding>, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut session = state
            .sessions
            .remove(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let requester = session
            .queue
            .iter()
            .find(|q| q.state == "queued")
            .map(|q| q.requester.clone())
            .unwrap_or_default();
        let scope = self.read_scope(&state, &requester);
        let outcome = self.with_read_scope(scope, || -> Result<Value> {
            if session.phase != "ready" || session.active.is_some() {
                return Err(refuse("conflict", "the session is not idle"));
            }
            if !session.quiescent {
                return Err(refuse("conflict", "the prior turn is not quiescent"));
            }
            let index = session
                .queue
                .iter()
                .position(|q| q.state == "queued")
                .ok_or_else(|| refuse("unavailable", "the queue is empty"))?;
            let requester = &session.queue[index].requester;
            if !state.policy.owners.contains(&session.owner)
                || requester.is_empty()
                || requester != &session.owner && !state.policy.controllers.contains(requester)
            {
                return Err(refuse(
                    "not_admitted",
                    "queued requester is no longer authorized",
                ));
            }
            let intent_ref = session.queue[index].intent.clone();
            match self.intent(&session, &intent_ref, now) {
                Ok(_) => {}
                Err(e) => {
                    session.queue[index].state = if code(&e) == "stale" {
                        "expired"
                    } else {
                        "refused"
                    }
                    .into();
                    session.revision += 1;
                    self.write_state(&mut state, &mut session, now)?;
                    return Ok(
                        json!({"promoted":null,"state":session.queue[index].state,"reason":code(&e)}),
                    );
                }
            }
            self.require(&state, &session, "queue")?;
            self.start_turn(&mut state, &mut session, &intent_ref, binding, now, None, None)?;
            let turn = session.active.as_ref().map(|t| t.id.clone());
            session.queue[index].state = "promoted".into();
            session.queue[index].turn = turn.clone();
            session.revision += 1;
            self.write_state(&mut state, &mut session, now)?;
            Ok(json!({"promoted":turn,"state":"promoted"}))
        });
        state.sessions.insert(session_id.into(), session);
        let value = outcome?;
        self.save(&state)?;
        Ok(value)
    }

    fn respond(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
        continuation: Option<&env_host::runtime::ContinuationFence<'_>>,
    ) -> Result<Value> {
        let id = body["scope"]["session"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let mut session = state
            .sessions
            .remove(&id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let outcome =
            self.respond_session(state, &mut session, signer, body, input, now, continuation);
        state.sessions.insert(id, session);
        outcome
    }

    fn respond_session(
        &self,
        state: &mut State,
        session: &mut Session,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
        continuation: Option<&env_host::runtime::ContinuationFence<'_>>,
    ) -> Result<Value> {
        let index = session
            .interactions
            .iter()
            .position(|i| same(&i.reference, &body["interaction"]))
            .ok_or_else(|| refuse("not_admitted", "unknown interaction"))?;
        let interaction = session.interactions[index].clone();
        if body["responder"] != signer
            || signer != interaction.recipient
            || !state.policy.owners.contains(&session.owner)
        {
            return Err(refuse(
                "not_admitted",
                "only the current admitted recipient may answer",
            ));
        }
        if body["scope"]["authority"] != self.pubkey()
            || body["scope"]["generation"] != session.generation
        {
            return Err(refuse("stale", "scope generation"));
        }
        if session.protocol_fault.is_some() {
            return Err(refuse(
                "unavailable",
                "the native protocol needs operator attention",
            ));
        }
        if let Some(claim) = &interaction.claim {
            if claim.fingerprint == digest_of(input) {
                // An unknown delivery stays unknown; it is never sent again.
                let mut result = claim.result.clone();
                if result["status"] == "accepted" {
                    result["status"] = json!("duplicate");
                }
                return Ok(result);
            }
            return Err(refuse("conflict", "the interaction was already answered"));
        }
        if body["responder"] != signer || signer != interaction.recipient {
            return Err(refuse(
                "not_admitted",
                "only the admitted recipient may answer",
            ));
        }
        if body["scope"]["generation"] != session.generation {
            return Err(refuse("stale", "scope generation"));
        }
        if interaction.state != "pending"
            || interaction.expires_at <= now
            || body["expires_at"]
                .as_u64()
                .is_none_or(|expiry| expiry <= now)
            || session
                .active
                .as_ref()
                .is_none_or(|t| t.id != interaction.turn)
        {
            return Err(refuse("stale", "the interaction is no longer pending"));
        }
        let response = contracts::parse_strict_bounded(&self.bytes(&body["response"])?, 65_536)
            .map_err(contract)?;
        let schema = serde_json::to_vec(&answer_schema()).map_err(|e| e.to_string())?;
        validate_against(&schema, &response)
            .map_err(|_| refuse("malformed", "answer does not match its schema"))?;
        let adapter = state
            .adapters
            .get(&session.adapter)
            .ok_or("adapter")?
            .clone();
        self.check_continuation(state, session, continuation, now)?;
        // Claim first and persist delivery intent before the engine reply.
        let mut claimed = session.clone();
        claimed.interactions[index].state = "unknown".into();
        let pending = self.control_result(
            state,
            signer,
            input,
            "unknown",
            None,
            Some("unavailable"),
            now,
        )?;
        claimed.interactions[index].claim = Some(Prior {
            fingerprint: digest_of(input).into(),
            result: pending,
        });
        let mut snapshot = self.state()?;
        snapshot.sessions.insert(session.id.clone(), claimed);
        snapshot.signers.extend(state.signers.clone());
        snapshot.declarations.extend(state.declarations.clone());
        for (principal, known) in &state.known {
            snapshot
                .known
                .entry(principal.clone())
                .or_default()
                .extend(known.iter().cloned());
        }
        self.save(&snapshot)?;
        self.check_continuation(state, session, continuation, now)?;
        let native = self.native_session(session, &adapter.engine)?.to_string();
        let delivered = self
            .engines
            .borrow_mut()
            .get_mut(&adapter.engine)
            .ok_or("engine")?
            .answer(
                &native,
                &interaction.native_id,
                &serde_json::to_vec(&response).map_err(|e| e.to_string())?,
            );
        let next = if delivered.is_ok() {
            "answered"
        } else {
            "unknown"
        };
        let mut updated = self.store.json(&interaction.current)?;
        updated["state"] = json!(next);
        let reference = self.sign(state, &updated, &session.owner.clone(), now)?;
        self.capture(state, session, now)?;
        session.interactions[index].state = next.into();
        session.interactions[index].current = reference;
        if delivered.is_ok() {
            session.phase = "running".into();
        }
        let before = session.revision;
        session.revision += 1;
        let receipt = self.receipt(state, session, input, before, "respond", Vec::new(), now)?;
        self.write_state(state, session, now)?;
        let status = if delivered.is_ok() {
            "accepted"
        } else {
            "unknown"
        };
        let result = self.control_result(state, signer, input, status, Some(receipt), None, now)?;
        session.interactions[index].claim = Some(Prior {
            fingerprint: digest_of(input).into(),
            result: result.clone(),
        });
        Ok(result)
    }

    /// Build a foreign history export from exact retained source bytes. The
    /// result is archival: it establishes no native resume or authority.
    pub fn import_history(&self, source: &Value, content: &[u8], now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        sess::validate(source).map_err(contract)?;
        if source["v"] != "openagents.session-import-source.v1" {
            return Err(refuse("unsupported_version", "import source"));
        }
        let content_ref = self.store.put(content, "application/octet-stream", None)?;
        if !same(&content_ref, &source["content"]) {
            return Err(refuse("identity_mismatch", "import content"));
        }
        let source_ref = self.sign(&mut state, source, self.pubkey(), now)?;
        let mut losses = vec![
            json!({"source":null,"category":"unsupported","reason":"foreign import has no native resume"}),
        ];
        if source["coverage"] != "complete" {
            losses.push(json!({"source":content_ref,"category":"unavailable","reason":"the source is not complete"}));
        }
        let body = json!({"v":"openagents.session-history.v1","requires":[],"lineage":token(),
            "source":{"kind":"foreign","import":source_ref},"adapter":source["importer"],"engine":source["engine"],
            "cut":null,"native":[],"portable":content_ref,"losses":losses,"resume":null,"coverage":source["coverage"]});
        let reference = self.sign(&mut state, &body, self.pubkey(), now)?;
        self.save(&state)?;
        Ok(reference)
    }

    /// Export the current history for the owner.
    pub fn export(&self, session_id: &str, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut session = state
            .sessions
            .remove(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let reference = self.export_history(&mut state, &mut session, now);
        state.sessions.insert(session_id.into(), session);
        let reference = reference?;
        self.save(&state)?;
        Ok(reference)
    }

    /// Link a child: managed needs a child admission and delegated
    /// authorization; observed needs native evidence and grants no control.
    pub fn link_child(
        &self,
        session_id: &str,
        turn: &str,
        child: Option<&str>,
        native: Option<Value>,
        authorization: Option<Value>,
        now: u64,
    ) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let child_admission = match child {
            Some(id) => Some(
                state
                    .sessions
                    .get(id)
                    .ok_or_else(|| refuse("not_admitted", "unknown child session"))?
                    .admission
                    .clone(),
            ),
            None => None,
        };
        let mut session = state
            .sessions
            .remove(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        let outcome = (|| -> Result<Value> {
            if !session
                .results
                .iter()
                .any(|r| self.store.json(r).is_ok_and(|b| b["turn"] == turn))
                && session.active.as_ref().is_none_or(|t| t.id != turn)
            {
                return Err(refuse("not_admitted", "unknown parent turn"));
            }
            let relationship = if child_admission.is_some() && authorization.is_some() {
                "managed"
            } else {
                "observed"
            };
            let body = json!({"v":"openagents.session-child.v1","requires":[],"parent":session.scope(self.pubkey()),
                "turn":turn,"child":child_admission,"native_source":native,"relationship":relationship,
                "authorization":authorization});
            let reference = self.sign(&mut state, &body, &session.owner.clone(), now)?;
            session.children.push(reference.clone());
            session.revision += 1;
            self.write_state(&mut state, &mut session, now)?;
            Ok(reference)
        })();
        state.sessions.insert(session_id.into(), session);
        let reference = outcome?;
        self.save(&state)?;
        Ok(reference)
    }

    /// Current session view.
    pub fn status(&self, session_id: &str) -> Result<Value> {
        let state = self.state()?;
        let session = state
            .sessions
            .get(session_id)
            .ok_or_else(|| refuse("not_admitted", "unknown session"))?;
        Ok(
            json!({"session":session.id,"lineage":session.lineage,"phase":session.phase,"revision":session.revision,
            "generation":session.generation,"active_turn":session.active.as_ref().map(|t| t.id.clone()),
            "turn_revision":session.turn_revision,"state":session.state,"history":session.history,
            "queue":session.queue.iter().map(|q| json!({"intent":q.id,"state":q.state,"turn":q.turn})).collect::<Vec<_>>(),
            "interactions":session.interactions.iter().map(|i| json!({"interaction":i.reference,"state":i.state})).collect::<Vec<_>>(),
            "results":session.results,"steers":session.steers,"children":session.children,"read_only":session.read_only,
            "admission":session.admission,"effective":session.effective,"requested":session.requested,"protocol_fault":session.protocol_fault,
            "runtime":session.runtime_binding,"replacement_pending":session.replacing}),
        )
    }
}

impl SessionHost {
    pub(crate) fn cj_run_in_use(&self, state: &State, run: &str) -> Result<bool> {
        for prior in state.priors.values() {
            if prior.result["receipt"].is_null() {
                continue;
            }
            let receipt = self.store.json(&prior.result["receipt"])?;
            for reference in receipt["records"].as_array().ok_or("receipt records")? {
                let record = self.store.json(reference)?;
                if record["v"] == nostr::run::RECORD_SCHEMA && record["record"]["run"] == run {
                    return Ok(true);
                }
            }
        }
        for session in state.sessions.values() {
            if session
                .opening_run
                .iter()
                .chain(&session.close_run)
                .chain(&session.configuration_run)
                .chain(&session.replacement_run)
                .chain(session.active.iter().flat_map(|t| t.run.iter()))
                .any(|r| r["record"]["run"] == run)
            {
                return Ok(true);
            }
            for reference in &session.results {
                let result = self.store.json(reference)?;
                if self.store.json(&result["run"])?["record"]["run"] == run {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn cj_stop(
        &self,
        state: &mut State,
        input: &Value,
        now: u64,
        cause: &str,
    ) -> Result<()> {
        let Some(turn) = state.turn_commands.get(digest_of(input)).cloned() else {
            return Ok(());
        };
        let id = turn["session"].as_str().ok_or("session")?.to_string();
        let session = state.sessions.get_mut(&id).ok_or("session")?;
        let Some(active) = session.active.as_mut().filter(|t| t.id == turn["turn"]) else {
            return Ok(());
        };
        if active.interrupted {
            return Ok(());
        }
        active.interrupted = true;
        active.stop_cause = Some(cause.into());
        let turn_id = active.id.clone();
        let adapter = state
            .adapters
            .get(&session.adapter)
            .ok_or("adapter")?
            .clone();
        let native = self
            .native_session(session, &adapter.engine)
            .map(str::to_string)
            .ok();
        self.save(state)?;
        // This cleanup may stop only the original active turn. Its durable
        // intent prevents a lost acknowledgment from repeating the command.
        if let Some(native) = native {
            let _ = self
                .engines
                .borrow_mut()
                .get_mut(&adapter.engine)
                .ok_or("engine")?
                .interrupt(&native, &turn_id);
        }
        let mut session = state.sessions.remove(&id).ok_or("session")?;
        let observed = self.observe_session(state, &mut session, now);
        state.sessions.insert(id, session);
        observed?;
        Ok(())
    }

    pub(crate) fn cj_target(
        &self,
        state: &State,
        target: &Value,
    ) -> Result<crate::transport::Target> {
        for adapter in state.adapters.values() {
            let description = self.store.json(&adapter.reference)?;
            let matched = adapter.definition == *target
                || description["features"]
                    .as_array()
                    .is_some_and(|entries| entries.iter().any(|f| f["operation"] == *target));
            if !matched {
                continue;
            }
            let card =
                contracts::parse_strict(&self.store.get(&target["artifact"])?).map_err(contract)?;
            let feature = card["meta"]["feature"].as_str();
            if matches!(
                feature,
                Some("questions" | "approvals" | "steer" | "interrupt" | "history")
            ) {
                return Err("unsupported_feature: this operation requires its continuation or read endpoint".into());
            }
            let profile = self
                .engines
                .borrow()
                .get(&adapter.engine)
                .ok_or("engine")?
                .profile();
            let expected =
                crate::interface::definition(&self.store, self.pubkey(), &profile, feature)?;
            if expected != *target
                || description["engine"]["digest"] != digest_bytes(&profile.engine)
            {
                return Err(
                    "identity_mismatch: installed engine differs from its admitted capability"
                        .into(),
                );
            }
            return Ok(crate::transport::Target {
                adapter: adapter.reference.clone(),
                engine: description["engine"].clone(),
                input: card["input"].clone(),
            });
        }
        Err("not_admitted: session capability is not admitted".into())
    }

    pub(crate) fn cj_reader(
        &self,
        state: &State,
        actor: &str,
        input: Option<&Value>,
    ) -> Result<()> {
        if !state.policy.owners.contains(actor) {
            return Err("not_admitted: runtime caller is not a current owner".into());
        }
        if let Some(input) = input {
            let body = self.store.json(input)?;
            if body["v"] == "openagents.session-command.v1" {
                self.command_authority(state, actor, &body)?;
                let session = state
                    .sessions
                    .get(body["scope"]["session"].as_str().ok_or("session")?)
                    .ok_or("session")?;
                if session.owner != actor {
                    return Err(
                        "not_admitted: this runtime endpoint requires the session owner".into(),
                    );
                }
            } else if body["v"] == "openagents.session-open.v1" && body["owner"] != actor {
                return Err(
                    "not_admitted: this runtime endpoint requires the session owner".into(),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn cj_reply_reader(&self, state: &State, actor: &str, key: &str) -> Result<()> {
        let book = state.transport.as_ref().ok_or("transport")?;
        self.cj_reader(
            state,
            actor,
            if book.denied.contains(key) {
                None
            } else {
                book.inputs.get(key)
            },
        )
    }

    fn cj_outcome(
        &self,
        state: &mut State,
        key: &str,
        actor: &str,
        value: Value,
        now: u64,
    ) -> Result<Value> {
        // A duplicate domain reply also gets its own signed, shared body;
        // it cannot borrow another invocation's output by command ID alone.
        self.sign(state, &value, actor, now)?;
        state
            .transport
            .as_mut()
            .ok_or("transport")?
            .outcomes
            .insert(key.into(), value.clone());
        self.save(state)?;
        Ok(value)
    }

    pub(crate) fn dispatch_cj(
        &self,
        state: State,
        event: &Event,
        opened: &nostr::execution::Opened,
        execute: &nostr::execution::Execute,
        artifacts: &BTreeMap<String, Vec<u8>>,
        binding: &Binding,
        target: &crate::transport::Target,
        key: &str,
        now: u64,
    ) -> Result<Value> {
        let intake = workspace_host::host::request(
            &self.secret,
            &opened.principal,
            json!({"v":"openagents.coordination-native-intake.v1","requires":[],
                "request":event.id,"fingerprint":execute.fingerprint}),
            now,
        )?;
        let dispatch = env_host::runtime::RuntimeDispatch {
            attachment: &binding.attachment,
            fingerprint: &binding.fingerprint,
            grant: &binding.grant,
            controller: &opened.principal,
            executor: self.pubkey(),
            engine: &target.engine,
            target: &execute.payload["target"],
            intake: &intake,
        };
        let mut state = Some(state);
        self.bindings
            .with_execution(&dispatch, now, &mut |fence, admitted, current| {
                if admitted.0 != execute.payload {
                    return Err("identity_mismatch: admitted session execution differs".into());
                }
                nostr::execution::require_pins(execute, &admitted.1)
                    .map_err(|e| format!("{e:?}"))?;
                let mut state = state
                    .take()
                    .ok_or("unknown: runtime callback cannot repeat")?;
                let reference = execute.payload["input"]["artifact"].clone();
                let bytes = &admitted.1[digest_of(&reference)];
                let body = contracts::parse_strict_bounded(bytes, 1_048_576).map_err(contract)?;
                self.store
                    .put(bytes, "application/json", reference["schema"].as_str())?;
                self.remember(&mut state, &opened.principal, &reference)?;
                for (digest, bytes) in artifacts.iter().chain(admitted.1.iter()) {
                    if digest_bytes(bytes) != *digest {
                        return Err("identity_mismatch: runtime artifact bytes".into());
                    }
                    let r = self.store.put(bytes, "application/octet-stream", None)?;
                    self.remember(&mut state, &opened.principal, &r)?;
                }
                let event_ref = self.store.put(
                    &serde_json::to_vec(event).map_err(|e| e.to_string())?,
                    "application/json",
                    None,
                )?;
                state.declarations.insert(
                    format!("{}:cj:{}", digest_of(&reference), opened.principal),
                    event_ref,
                );
                state
                    .signers
                    .entry(digest_of(&reference).into())
                    .or_default()
                    .insert(opened.principal.clone());
                state
                    .transport
                    .as_mut()
                    .ok_or("transport")?
                    .inputs
                    .insert(key.into(), reference.clone());
                let preflight = (|| -> Result<()> {
                    validate_against(&self.store.get(&target.input)?, &body)?;
                    match body["v"].as_str() {
                        Some("openagents.session-open.v1") if body["adapter"] == target.adapter => {
                        }
                        Some("openagents.session-command.v1") => {
                            let session = state
                                .sessions
                                .get(body["scope"]["session"].as_str().ok_or("session")?)
                                .ok_or("not_admitted: unknown session")?;
                            let adapter = if body["action"] == "replace_runtime" {
                                &body["payload"]["adapter"]
                            } else {
                                &state.adapters[&session.adapter].reference
                            };
                            if *adapter != target.adapter
                                || matches!(body["action"].as_str(), Some("steer" | "interrupt"))
                            {
                                return Err(
                                    "not_admitted: session operation or adapter differs".into()
                                );
                            }
                        }
                        _ => {
                            return Err(
                                "unsupported_feature: session operation needs another endpoint"
                                    .into(),
                            );
                        }
                    }
                    self.cj_reader(&state, &opened.principal, Some(&reference))
                })();
                if let Err(error) = preflight {
                    let reason = code(&error);
                    let status = if reason == "conflict" {
                        "conflict"
                    } else {
                        "refused"
                    };
                    let value = self.control_result(
                        &mut state,
                        &opened.principal,
                        &reference,
                        status,
                        None,
                        Some(reason),
                        current,
                    )?;
                    state
                        .transport
                        .as_mut()
                        .ok_or("transport")?
                        .denied
                        .insert(key.into());
                    return self.cj_outcome(&mut state, key, &opened.principal, value, current);
                }
                self.save(&state)?;
                let value = self.submit_authenticated(
                    state,
                    opened.principal.clone(),
                    body,
                    reference,
                    Some(binding),
                    current,
                    Some(fence),
                    None,
                )?;
                let mut state = self.state()?;
                self.cj_outcome(&mut state, key, &opened.principal, value, current)
            })
    }
}

impl SessionHost {
    pub(crate) fn cj_progress(
        &self,
        state: &mut State,
        input: &Value,
        actor: &str,
        key: &str,
        observe: bool,
        now: u64,
    ) -> Result<crate::transport::JobProgress> {
        self.cj_reply_reader(state, actor, key)?;
        let book = state.transport.as_ref().ok_or("transport")?;
        if book.denied.contains(key) {
            return self.cj_control_progress(
                book.outcomes
                    .get(key)
                    .cloned()
                    .ok_or("unknown: refusal was not acknowledged")?,
            );
        }
        let body = self.store.json(input)?;
        let command_key = if body["v"] == "openagents.session-open.v1" {
            format!(
                "{}:{}:{}",
                self.pubkey(),
                body["session"].as_str().ok_or("session")?,
                body["command"].as_str().ok_or("command")?
            )
        } else {
            format!(
                "{}:{}:{}",
                self.pubkey(),
                body["scope"]["session"].as_str().ok_or("session")?,
                body["command"].as_str().ok_or("command")?
            )
        };
        if let Some(turn) = state.turn_commands.get(digest_of(input)).cloned() {
            let id = turn["session"].as_str().ok_or("session")?.to_string();
            let mut session = state.sessions.remove(&id).ok_or("session")?;
            if session.generation != turn["generation"].as_u64().ok_or("generation")? {
                state.sessions.insert(id, session);
                return Err("stale: runtime generation changed".into());
            }
            if observe
                && session
                    .active
                    .as_ref()
                    .is_some_and(|a| a.id == turn["turn"].as_str().unwrap_or_default())
            {
                let observed = self.observe_session(state, &mut session, now);
                state.sessions.insert(id.clone(), session.clone());
                observed?;
            }
            let result = session.results.iter().find_map(|r| {
                let b = self.store.json(r).ok()?;
                (b["turn"] == turn["turn"]).then_some((r.clone(), b))
            });
            state.sessions.insert(id, session);
            if let Some((reference, result)) = result {
                let outcome = match result["outcome"].as_str() {
                    Some("completed") => "completed",
                    Some("cancelled") => "cancelled",
                    Some("failed") => "failed",
                    _ => "unknown",
                };
                let mut files = vec![
                    reference.clone(),
                    result["run"].clone(),
                    result["quiescence"].clone(),
                ];
                files.extend(
                    result["outputs"]
                        .as_array()
                        .ok_or("turn outputs")?
                        .iter()
                        .cloned(),
                );
                if !result["quiescence"].is_null() {
                    let q = self.store.json(&result["quiescence"])?;
                    for evidence in q["evidence"].as_array().ok_or("quiescence evidence")? {
                        files.push(evidence.clone());
                        let native = self.store.json(evidence)?;
                        files.push(native["payload"].clone());
                    }
                }
                return Ok(crate::transport::JobProgress::Done {
                    outcome,
                    output: reference,
                    files,
                });
            }
            return Ok(crate::transport::JobProgress::Pending);
        }
        let value = state
            .transport
            .as_ref()
            .ok_or("transport")?
            .outcomes
            .get(key)
            .cloned()
            .or_else(|| {
                state
                    .priors
                    .get(&command_key)
                    .filter(|p| p.fingerprint == digest_of(input))
                    .map(|p| p.result.clone())
            })
            .ok_or("unknown: domain admission was not acknowledged")?;
        self.cj_control_progress(value)
    }

    fn cj_control_progress(&self, value: Value) -> Result<crate::transport::JobProgress> {
        let outcome = match value["status"].as_str() {
            Some("accepted" | "duplicate") => "completed",
            Some("refused" | "conflict") => "failed",
            _ => "unknown",
        };
        let output = self.store.put_json(&value)?;
        let mut files = vec![output.clone()];
        if !value["receipt"].is_null() {
            files.push(value["receipt"].clone());
            let receipt = self.store.json(&value["receipt"])?;
            files.extend(
                receipt["records"]
                    .as_array()
                    .ok_or("receipt records")?
                    .iter()
                    .cloned(),
            );
        }
        Ok(crate::transport::JobProgress::Done {
            outcome,
            output,
            files,
        })
    }

    pub(crate) fn cj_files(
        &self,
        state: &State,
        actor: &str,
        references: Vec<Value>,
    ) -> Result<Vec<Value>> {
        let known = state.known.get(actor);
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for reference in references {
            let digest = digest_of(&reference);
            if reference.is_null() || !seen.insert(digest.to_string()) {
                continue;
            }
            if !known.is_some_and(|k| k.contains(digest)) {
                return Err("not_admitted: runtime result contains an unshared artifact".into());
            }
            self.store.get(&reference)?;
            out.push(reference);
        }
        Ok(out)
    }
}

/// Build a signed private request envelope for a session host.
pub fn envelope(secret: &SecretKey, host: &str, body: &Value, now: u64) -> Result<Event> {
    sess::validate(body).map_err(contract)?;
    workspace_host::host::request(secret, host, body.clone(), now)
}
