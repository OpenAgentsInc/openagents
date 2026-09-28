//! The CAP/CJ binding of NIP-HOST.
//!
//! A host advertises one NIP-CAP `adapter` definition, `host-access`, whose
//! input and output are pinned JSON Schemas for the direct-channel messages:
//! `openagents.host-call.v1` carries the exact signed request artifact, and
//! `openagents.host-answer.v1` carries the exact signed reply. A device
//! invokes it as a NIP-CJ execution job on kinds `25920`, `26920`, and
//! `27020`.
//!
//! The binding adds transport, never authority. The host hands the embedded
//! request to the same admission as the direct artifact binding, so the
//! grant, epoch, right, idempotency, and refusal rules are the ones that
//! binding applies. A CJ `completed` result means the NIP-HOST operation
//! answered; the embedded reply says whether it was admitted. It never means
//! that a task ran or finished.
//!
//! Every field of the execute body is fixed by the capability and the
//! embedded request: `request` and `run` are the NIP-HOST request ID,
//! `attempt` is 1, `deadline` is the request's expiry, and `retain_until` is
//! [`RETENTION`] seconds later. The CJ idempotency key therefore collapses to
//! the NIP-HOST request ID: a retransmission carries the same body, and
//! changed bytes under a known request ID are a changed NIP-HOST request,
//! which the host refuses with its signed `conflict` reply.

use crate::client::{Client, Pending};
use crate::protocol::{REPLY, REQUEST, Request, open, pubkey, public};
use crate::{Code, Error, Outcome, RelayPolicy, Result, fail};
use nostr::cap;
use nostr::contracts::{ArtifactRef, digest_bytes, jcs, prepare_closure, validate_instance};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::execution::{self, Body, Execute, Seal, Window};
use nostr::nip44;
use secp256k1::{SecretKey, XOnlyPublicKey, rand::RngCore};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::str::FromStr;
use std::time::Duration;

/// The capability's `d` slug and component.
pub const SLUG: &str = "host-access";
/// The qualified ID's package.
pub const PACKAGE: &str = "openagents";
/// Input body: the exact signed request artifact.
pub const CALL: &str = "openagents.host-call.v1";
/// Output body: the exact signed reply artifact.
pub const ANSWER: &str = "openagents.host-answer.v1";
/// Seconds past the deadline that `retain_until` names: the NIP-HOST request
/// window. After the deadline a retransmission earns no reply and no effect.
pub const RETENTION: u64 = 60;
/// The input schema's exact bytes.
pub const CALL_SCHEMA: &[u8] = include_bytes!("../../../nips/openagents/schemas/host-call.v1.json");
/// The output schema's exact bytes.
pub const ANSWER_SCHEMA: &[u8] =
    include_bytes!("../../../nips/openagents/schemas/host-answer.v1.json");
/// Every NIP-HOST operation this binding carries.
pub const OPERATIONS: [&str; 17] = [
    "enroll.redeem",
    "enroll.approve",
    "enroll.deny",
    "invite.create",
    "invite.cancel",
    "device.list",
    "device.revoke",
    "task.create",
    "task.steer",
    "task.cancel",
    "task.archive",
    "task.command",
    "task.queue",
    "workspace.list",
    "terminal.open",
    "spend.list",
    "spend.settle",
];

const LOCK: &str = "openagents.lock.v1";
const CONTEXT: &str = "openagents.host-context.v1";
const REQUIREMENTS: &str = "openagents.host-requirements.v1";
const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
const DEFINITION: &str = "openagents.capability-definition.v1";
const MAX_RELAYS: usize = 8;
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(12);
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// One host's `host-access` capability: its definition and the references
/// an execute body pins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capability {
    host: String,
    relays: Vec<String>,
    definition: Value,
    bytes: Vec<u8>,
}

impl Capability {
    /// The capability a host serving `relays` advertises. The definition
    /// passes the NIP-CAP validator before it is returned.
    ///
    /// # Errors
    /// Refuses a malformed host key, an empty or oversized relay list, or a
    /// definition the validator refuses.
    pub fn new(host: &str, relays: Vec<String>) -> Result<Self> {
        public(host)?;
        if relays.is_empty() || relays.len() > MAX_RELAYS {
            return fail(Code::Bounds, "a capability names one to eight relays");
        }
        let definition = definition(host, &relays);
        cap::parse_definition(&definition)
            .map_err(|_| Error::new(Code::Malformed, "the capability definition is invalid"))?;
        let bytes = jcs(&definition).map_err(|_| malformed("definition"))?;
        Ok(Self {
            host: host.into(),
            relays,
            definition,
            bytes,
        })
    }

    /// Read a published `kind:30180` manifest from the pinned `host`. Only
    /// the exact shape [`Capability::new`] builds is accepted.
    ///
    /// # Errors
    /// Refuses another signer, kind, slug, or definition, and tags that
    /// disagree with the definition.
    pub fn from_event(event: &Event, host: &str) -> Result<Self> {
        let refuse = || {
            Error::new(
                Code::Forbidden,
                "the capability manifest is not this host's",
            )
        };
        if event.kind != cap::DISCOVERY_KIND || event.pubkey != host {
            return Err(refuse());
        }
        event.validate_structure().map_err(|_| refuse())?;
        event.validate_crypto().map_err(|_| refuse())?;
        if event.tag_values("d").collect::<Vec<_>>() != [SLUG] {
            return Err(refuse());
        }
        let content = cap::parse_json(event.content.as_bytes())
            .map_err(|_| malformed("capability manifest"))?;
        let Some(object) = content.as_object().filter(|o| o.len() == 1) else {
            return Err(malformed("capability manifest"));
        };
        let published = object
            .get("definition")
            .ok_or_else(|| malformed("capability manifest"))?;
        let parsed =
            cap::parse_definition(published).map_err(|_| malformed("capability definition"))?;
        cap::check_discovery_tags(&event.tags, &parsed).map_err(|_| refuse())?;
        let relays = published["binding_contract"]["remote"]["relays"]
            .as_array()
            .map(|relays| {
                relays
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let capability = Self::new(host, relays)?;
        if &capability.definition != published {
            return fail(
                Code::Unsupported,
                "the host advertises another host-access definition",
            );
        }
        Ok(capability)
    }

    /// The host key: the CJ worker.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The relays the host answers CJ requests on.
    #[must_use]
    pub fn relays(&self) -> &[String] {
        &self.relays
    }

    /// The NIP-CAP definition.
    #[must_use]
    pub fn definition(&self) -> &Value {
        &self.definition
    }

    /// The qualified definition ID.
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}:{PACKAGE}/{SLUG}", self.host)
    }

    /// The DefinitionRef an execute body's `target` names.
    #[must_use]
    pub fn target(&self) -> Value {
        json!({"id": self.id(), "artifact": reference(&self.bytes, DEFINITION)})
    }

    /// The ArtifactRef of the one-entry dependency lock.
    #[must_use]
    pub fn lock(&self) -> Value {
        let target = self.target();
        let lock = json!({
            "v": LOCK,
            "requires": [],
            "root": target,
            "entries": [{"id": self.id(), "definition": target, "dependencies": []}],
        });
        reference(&jcs(&lock).unwrap_or_default(), LOCK)
    }

    /// The signed `kind:30180` manifest: `d` is [`SLUG`], with the NIP-CAP
    /// discovery tags.
    ///
    /// # Errors
    /// Refuses a signing key that is not this capability's host.
    pub fn manifest(&self, secret: &SecretKey, now: u64) -> Result<Event> {
        if pubkey(secret) != self.host {
            return fail(Code::Forbidden, "only the host signs its capability");
        }
        let parsed = cap::parse_definition(&self.definition)
            .map_err(|_| malformed("capability definition"))?;
        let mut tags = vec![Tag::new(vec!["d".into(), SLUG.into()])];
        tags.extend(cap::discovery_tags(&parsed));
        Ok(signer(secret)?.sign(
            now,
            cap::DISCOVERY_KIND,
            tags,
            json!({"definition": self.definition}).to_string(),
        ))
    }

    /// The execute body that carries `pending`. Each field is fixed by the
    /// capability and the request, so a retransmission repeats it exactly.
    #[must_use]
    pub fn execute(&self, pending: &Pending) -> Value {
        let request = &pending.request;
        json!({
            "v": execution::SCHEMA,
            "requires": [],
            "type": "execute",
            "request": request.request,
            "attempt": 1,
            "run": request.request,
            "target": self.target(),
            "lock": self.lock(),
            "input": {"v": CALL, "event": pending.event},
            "context": fixed(CONTEXT),
            "requirements": fixed(REQUIREMENTS),
            "bounds": {},
            "deadline": request.expires_at,
            "retain_until": request.expires_at + RETENTION,
        })
    }

    /// Seal the execute body for `pending` as a `kind:25920` request from
    /// `secret` to the host.
    ///
    /// # Errors
    /// Refuses a request for another host and a payload over the CJ bound.
    pub fn request(&self, secret: &SecretKey, pending: &Pending, now: u64) -> Result<Event> {
        if pending.request.host != self.host || pending.event.pubkey != pubkey(secret) {
            return fail(
                Code::Forbidden,
                "the request is not this device's to this host",
            );
        }
        seal_execute(secret, &self.host, &self.execute(pending), now)
    }
}

/// Seal an execute body as a `kind:25920` request from `secret` to `host`,
/// with the body's deadline as its expiration. The host, not this function,
/// decides whether the body fits the binding.
///
/// # Errors
/// Refuses a body without a deadline and a payload over the CJ bound.
pub fn seal_execute(secret: &SecretKey, host: &str, body: &Value, now: u64) -> Result<Event> {
    let deadline = body["deadline"]
        .as_u64()
        .ok_or_else(|| malformed("the execute body has no deadline"))?;
    let tags = vec![
        Tag::new(vec!["p".into(), host.into()]),
        Tag::new(vec!["expiration".into(), deadline.to_string()]),
    ];
    seal(secret, host, execution::REQUEST_KIND, tags, body, now)
}

/// The NIP-CAP definition for a host serving `relays`.
#[must_use]
pub fn definition(host: &str, relays: &[String]) -> Value {
    json!({
        "v": 1,
        "requires": [],
        "id": format!("{host}:{PACKAGE}/{SLUG}"),
        "profile": "adapter",
        "summary": "Answers NIP-HOST requests from enrolled devices under the host's own grants: enrollment, invitations, device listing and revocation, task creation, steering, cancellation, archiving, durable task commands, and queue editing, workspace listing, terminal opening, and agent spend requests and receipts.",
        "input": schema_ref(CALL_SCHEMA),
        "output": schema_ref(ANSWER_SCHEMA),
        "effects": {
            "reads": ["host-access"],
            "writes": ["host-access", "host-tasks", "host-terminals"],
            "network": ["relay"],
            "process": true,
            "delegates": false,
            "spend": false
        },
        "minimum": {},
        "support": {
            "bounds": {"input_bytes": "enforced"},
            "cancellation": "unsupported",
            "idempotency": "request_attempt",
            "evidence": [REPLY]
        },
        "binding_contract": {
            "interface": REQUEST,
            "transport": "nostr-cj",
            "operations": OPERATIONS,
            "remote": {"worker": host, "relays": relays}
        }
    })
}

/// What the host does with one `kind:25920` event.
#[derive(Debug)]
pub enum Intake {
    /// Hand `request` to NIP-HOST admission, then answer with
    /// [`Answering::answer`].
    Call {
        /// The exact signed request artifact.
        request: Box<Event>,
        /// Who and what the answer binds to.
        answering: Answering,
    },
    /// The binding refused before admission. Publish this sealed result.
    Refuse(Box<Event>),
    /// Not an answerable CJ request: another family, another worker, a bad
    /// signature, or a body without a request identity. It earns no reply.
    Ignore,
}

/// The CJ request an answer binds to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answering {
    principal: String,
    event: String,
    request: String,
}

impl Answering {
    /// Seal the NIP-HOST reply as a `completed` `kind:26920` result. The
    /// outcome says the operation answered; the reply says whether it was
    /// admitted.
    ///
    /// # Errors
    /// Refuses a reply that is not the host's.
    pub fn answer(&self, secret: &SecretKey, reply: &Event, now: u64) -> Result<Event> {
        if reply.pubkey != pubkey(secret) {
            return fail(Code::Forbidden, "the reply is not the host's");
        }
        let payload = json!({
            "v": execution::SCHEMA,
            "requires": [],
            "type": "result",
            "request": self.request,
            "attempt": 1,
            "run": self.request,
            "outcome": "completed",
            "dispatched": true,
            "output": {"v": ANSWER, "event": reply},
            "artifacts": [],
            "receipts": [],
            "verification": "not_run",
            "integration": "not_requested",
            "record": null,
            "spend": null
        });
        self.seal(secret, &payload, now)
    }

    fn refuse(&self, secret: &SecretKey, code: &str, message: &str, now: u64) -> Intake {
        let payload = execution::refusal_result(&self.request, 1, &self.request, code, message);
        match self.seal(secret, &payload, now) {
            Ok(event) => Intake::Refuse(Box::new(event)),
            Err(_) => Intake::Ignore,
        }
    }

    fn seal(&self, secret: &SecretKey, payload: &Value, now: u64) -> Result<Event> {
        let tags = vec![
            Tag::new(vec!["p".into(), self.principal.clone()]),
            Tag::new(vec!["e".into(), self.event.clone()]),
        ];
        seal(
            secret,
            &self.principal,
            execution::RESULT_KIND,
            tags,
            payload,
            now,
        )
    }
}

/// Open one `kind:25920` event addressed to the host and check every
/// binding field before anything reaches NIP-HOST admission.
///
/// The CJ signer must be the request's signer: a relayed or copied request
/// is refused as `not_admitted`. The target, lock, context, and
/// requirements must be the capability's; the input must match the pinned
/// schema; and the request ID and deadline must be the embedded request's.
#[must_use]
pub fn intake(event: &Event, secret: &SecretKey, capability: &Capability, now: u64) -> Intake {
    let Ok(opened) =
        execution::open_request(event, capability.host(), secret, now, Window::DEFAULT)
    else {
        return Intake::Ignore;
    };
    let Body::Execute(execute) = opened.body else {
        // Status, replay, and cancel controls have nothing to act on: the
        // operation answers within the request, so no run stays open.
        return Intake::Ignore;
    };
    let answering = Answering {
        principal: opened.principal,
        event: opened.event_id,
        request: execute.request.clone(),
    };
    match check(&execute, &answering, secret, capability) {
        Ok(request) => Intake::Call {
            request: Box::new(request),
            answering,
        },
        Err((code, message)) => answering.refuse(secret, code, message, now),
    }
}

type Refusal = (&'static str, &'static str);

fn check(
    execute: &Execute,
    answering: &Answering,
    secret: &SecretKey,
    capability: &Capability,
) -> std::result::Result<Event, Refusal> {
    let binding = (
        "malformed",
        "the execute body does not fit the host-access binding",
    );
    let identity = (
        "identity_mismatch",
        "the target, lock, context, or requirements is not this host's",
    );
    if execute.attempt != 1 || execute.run != execute.request {
        return Err(binding);
    }
    if execute.payload.get("meta").is_some()
        || execute.parent.is_some()
        || execute.input_artifact.is_some()
        || execute.bounds
            != (execution::Bounds {
                wall_ms: None,
                output_bytes: None,
                jobs: None,
                spend_microunits: None,
            })
        || execute.retain_until != execute.deadline.saturating_add(RETENTION)
    {
        return Err(binding);
    }
    let target = capability.target();
    if execute.target.id != capability.id()
        || execute.target.event.is_some()
        || !same(&execute.target.artifact, &target["artifact"])
        || !same(&execute.lock, &capability.lock())
        || !same(&execute.context, &fixed(CONTEXT))
        || !same(&execute.requirements, &fixed(REQUIREMENTS))
    {
        return Err(identity);
    }
    let mut documents = BTreeMap::new();
    documents.insert(digest_bytes(CALL_SCHEMA), CALL_SCHEMA.to_vec());
    let closure = prepare_closure(&documents).map_err(|_| binding)?;
    validate_instance(&closure, &digest_bytes(CALL_SCHEMA), &execute.input).map_err(|_| {
        (
            "malformed",
            "the input does not match the pinned call schema",
        )
    })?;
    let request: Event =
        serde_json::from_value(execute.input["event"].clone()).map_err(|_| binding)?;
    // The embedded request travels only under its own signer's job.
    if request.pubkey != answering.principal {
        return Err((
            "not_admitted",
            "the request signer is not the execution signer",
        ));
    }
    let opened: Request = open(
        &request,
        secret,
        &request.pubkey,
        capability.host(),
        REQUEST,
    )
    .map_err(|_| {
        (
            "malformed",
            "the input is not a NIP-HOST request to this host",
        )
    })?;
    if opened.request != execute.request || opened.expires_at != execute.deadline {
        return Err(binding);
    }
    Ok(request)
}

impl Client {
    /// Send one operation through the host's CAP/CJ binding.
    ///
    /// # Errors
    /// Returns the host's NIP-HOST refusal, a binding refusal, or a
    /// transport failure.
    pub async fn call_cj(&self, capability: &Capability, op: crate::Operation) -> Result<Outcome> {
        let pending = self.prepare(op, crate::unix_time()?)?;
        self.send_cj(capability, &pending).await
    }

    /// Send a prepared request through the CAP/CJ binding and verify the
    /// reply exactly as the direct artifact binding does. Retry by sending
    /// the same `pending` again.
    ///
    /// # Errors
    /// Returns the host's NIP-HOST refusal, a binding refusal, or a
    /// transport failure.
    pub async fn send_cj(&self, capability: &Capability, pending: &Pending) -> Result<Outcome> {
        let reply = self.exchange_cj(capability, pending).await?;
        self.verify_reply(pending, &reply, crate::unix_time()?)
    }

    /// Send a prepared request through the CAP/CJ binding and return the
    /// host's signed reply without reading its outcome.
    ///
    /// # Errors
    /// Refuses a capability for another host or relay, a CJ refusal, and a
    /// result that does not bind to this request; reports transport failures.
    pub async fn exchange_cj(&self, capability: &Capability, pending: &Pending) -> Result<Event> {
        if capability.host() != self.host() {
            return fail(Code::Forbidden, "the capability is another host's");
        }
        let relay = self.relay();
        if !capability.relays().iter().any(|r| r == relay) {
            return fail(
                Code::Unavailable,
                "the host does not answer CJ requests on this relay",
            );
        }
        self.policy().validate(relay).map_err(Error::from)?;
        let secret = *self.secret();
        let event = capability.request(&secret, pending, crate::unix_time()?)?;
        exchange(
            relay,
            &secret,
            capability.host(),
            &pending.request.request,
            &event,
        )
        .await
    }
}

/// Publish one sealed `kind:25920` request on `relay` and return the NIP-HOST
/// reply inside the host's bound result for `request`. A binding refusal
/// returns its mapped code: `not_admitted` as `forbidden`, a foreign target
/// as `unsupported`.
///
/// # Errors
/// Returns the binding refusal, or a transport failure when no bound result
/// arrives in time.
pub async fn exchange(
    relay: &str,
    secret: &SecretKey,
    host: &str,
    request: &str,
    event: &Event,
) -> Result<Event> {
    tokio::time::timeout(
        EXCHANGE_TIMEOUT,
        exchange_once(relay, secret, host, request, event),
    )
    .await
    .map_err(|_| Error::new(Code::Transport, "the CJ exchange timed out"))?
}

async fn exchange_once(
    relay: &str,
    secret: &SecretKey,
    host: &str,
    subscription: &str,
    event: &Event,
) -> Result<Event> {
    let transport = |_| Error::new(Code::Transport, "the CJ exchange is unavailable");
    let me = pubkey(secret);
    let mut socket = nostr_transport::Connection::connect(relay, secret, EXCHANGE_TIMEOUT)
        .await
        .map_err(transport)?;
    socket
        .send(json!(["REQ", subscription, {
            "kinds": [execution::RESULT_KIND],
            "authors": [host],
            "#p": [me],
            "#e": [event.id],
            "limit": 0
        }]))
        .await
        .map_err(transport)?;
    loop {
        let frame = socket.next().await.map_err(transport)?;
        if frame[1] == subscription && frame[0] == "EOSE" {
            break;
        }
        if frame[1] == subscription && frame[0] == "CLOSED" {
            return fail(Code::Transport, "the relay closed the CJ subscription");
        }
    }
    socket
        .send(json!(["EVENT", event]))
        .await
        .map_err(transport)?;
    let waiting = execution::Pending {
        execute_event: &event.id,
        worker: host,
        customer: &me,
        request: subscription,
        attempt: 1,
    };
    loop {
        let frame = socket.next().await.map_err(transport)?;
        if frame[0] == "OK" && frame[1] == event.id.as_str() && frame[2] != true {
            return fail(Code::Transport, "the relay refused the CJ request");
        }
        if frame[0] == "CLOSED" && frame[1] == subscription {
            return fail(Code::Transport, "the relay closed the CJ subscription");
        }
        if frame[0] != "EVENT" || frame[1] != subscription {
            continue;
        }
        let Ok(received) = serde_json::from_value::<Event>(frame[2].clone()) else {
            continue;
        };
        // Admission and progress feedback carries no answer.
        let Ok(payload) = execution::bind_worker_event(&received, &waiting, secret) else {
            continue;
        };
        if payload.get("type").and_then(Value::as_str) != Some("result") {
            continue;
        }
        let _ = socket.close().await;
        return read_result(&payload);
    }
}

/// The host's reply inside a CJ result, or the binding's refusal.
fn read_result(payload: &Value) -> Result<Event> {
    match payload.get("outcome").and_then(Value::as_str) {
        Some("completed") => {
            let output = payload.get("output").cloned().unwrap_or(Value::Null);
            let mut documents = BTreeMap::new();
            documents.insert(digest_bytes(ANSWER_SCHEMA), ANSWER_SCHEMA.to_vec());
            let closure = prepare_closure(&documents).map_err(|_| malformed("answer schema"))?;
            validate_instance(&closure, &digest_bytes(ANSWER_SCHEMA), &output)
                .map_err(|_| malformed("the CJ output does not match the pinned answer schema"))?;
            serde_json::from_value(output["event"].clone())
                .map_err(|_| malformed("the CJ output carries no reply"))
        }
        Some("refused") => {
            let code = match payload.get("code").and_then(Value::as_str) {
                Some("not_admitted") => Code::Forbidden,
                Some("identity_mismatch" | "unsupported_version" | "unsupported_feature") => {
                    Code::Unsupported
                }
                Some("malformed") => Code::Malformed,
                Some("limit_exceeded") => Code::Bounds,
                Some("stale") => Code::Stale,
                _ => Code::Unavailable,
            };
            fail(code, "the host refused the CJ binding before admission")
        }
        _ => fail(Code::Unavailable, "the CJ result carries no answer"),
    }
}

/// Read the newest `host-access` manifest the pinned `host` published on
/// `relay`.
///
/// # Errors
/// Reports transport failures and `unavailable` when the host published no
/// valid manifest.
pub async fn fetch_capability(
    relay: &str,
    secret: &SecretKey,
    host: &str,
    policy: RelayPolicy,
) -> Result<Capability> {
    policy.validate(relay).map_err(Error::from)?;
    public(host)?;
    let transport = |_| Error::new(Code::Transport, "the capability fetch is unavailable");
    tokio::time::timeout(FETCH_TIMEOUT, async {
        let mut socket = nostr_transport::Connection::connect(relay, secret, FETCH_TIMEOUT)
            .await
            .map_err(transport)?;
        let id = "host-capability";
        socket
            .send(json!(["REQ", id, {
                "kinds": [cap::DISCOVERY_KIND], "authors": [host], "#d": [SLUG], "limit": 8
            }]))
            .await
            .map_err(transport)?;
        let mut newest: Option<(u64, Capability)> = None;
        loop {
            let frame = socket.next().await.map_err(transport)?;
            if frame[1] != id {
                continue;
            }
            match frame[0].as_str() {
                Some("EOSE") => break,
                Some("CLOSED") => return fail(Code::Transport, "the relay closed the fetch"),
                Some("EVENT") => {
                    let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
                        continue;
                    };
                    if let Ok(capability) = Capability::from_event(&event, host)
                        && newest.as_ref().is_none_or(|(at, _)| event.created_at > *at)
                    {
                        newest = Some((event.created_at, capability));
                    }
                }
                _ => {}
            }
        }
        let _ = socket.close().await;
        newest
            .map(|(_, capability)| capability)
            .ok_or_else(|| Error::new(Code::Unavailable, "the host advertises no CJ binding"))
    })
    .await
    .map_err(|_| Error::new(Code::Transport, "the capability fetch timed out"))?
}

fn reference(bytes: &[u8], schema: &str) -> Value {
    json!({
        "digest": digest_bytes(bytes),
        "size": bytes.len(),
        "media_type": "application/json",
        "schema": schema
    })
}

/// A SchemaRef to a pinned JSON Schema 2020-12 document.
fn schema_ref(bytes: &[u8]) -> Value {
    json!({
        "digest": digest_bytes(bytes),
        "size": bytes.len(),
        "media_type": "application/schema+json",
        "schema": DIALECT
    })
}

/// The fixed empty context or requirements body the binding pins: the
/// binding discloses no context and states no requirement beyond NIP-HOST
/// admission.
fn fixed(schema: &str) -> Value {
    let body = json!({"v": schema, "requires": []});
    reference(&jcs(&body).unwrap_or_default(), schema)
}

fn same(artifact: &ArtifactRef, expected: &Value) -> bool {
    Some(artifact.digest.as_str()) == expected["digest"].as_str()
        && Some(artifact.size) == expected["size"].as_u64()
        && Some(artifact.media_type.as_str()) == expected["media_type"].as_str()
        && artifact.schema.as_deref() == expected["schema"].as_str()
        && artifact.event.is_none()
        && artifact.sources.is_empty()
}

fn seal(
    secret: &SecretKey,
    recipient: &str,
    kind: u16,
    tags: Vec<Tag>,
    payload: &Value,
    now: u64,
) -> Result<Event> {
    let peer = XOnlyPublicKey::from_str(recipient).map_err(|_| malformed("recipient"))?;
    let mut nonce = [0u8; 32];
    secp256k1::rand::rng().fill_bytes(&mut nonce);
    let signer = signer(secret)?;
    Seal {
        signer: &signer,
        conversation: nip44::conversation_key(secret, &peer),
        nonce,
        created_at: now,
    }
    .event(kind, tags, payload)
    .map_err(|_| Error::new(Code::Bounds, "the CJ payload exceeds its bound"))
}

fn signer(secret: &SecretKey) -> Result<RelaySigner> {
    RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|_| malformed("signing key"))
}

fn malformed(message: &str) -> Error {
    Error::new(Code::Malformed, message)
}
