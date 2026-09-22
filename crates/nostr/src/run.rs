//! NIP-RUN v1 encrypted journals and fenced recovery.
//!
//! A signed event transports a record. It is not a lock, and it does not
//! make an effect exactly once. Logical digests stay inside ciphertext.
//! Verification and integration stay distinct from the outcome.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Map, Value};

use crate::contracts::{ContractError, RefusalCode, digest_bytes, jcs};
use crate::domain::Event;
use crate::nip44::{conversation_key, decrypt, encrypt};

/// One encrypted durable record.
pub const RECORD_KIND: u16 = 3_187;
/// Encrypted head hint. It is not the journal.
pub const HEAD_KIND: u16 = 30_186;
/// Marker on both kinds.
pub const MARKER: &str = "oa:run:v1";
/// Schema identifier mixed into the logical digest.
pub const RECORD_SCHEMA: &str = "openagents.run-record.v1";
/// Head hint schema.
pub const HEAD_SCHEMA: &str = "openagents.run-head.v1";

const RECORD_TYPES: &[&str] = &[
    "created",
    "admitted",
    "dispatched",
    "observed",
    "resolved",
    "cancel_requested",
    "unknown",
    "reconciled",
    "handoff",
    "settled",
];

/// One logical record. Its digest does not depend on the Nostr event id.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// Run id. It is not the mailbox.
    pub run: String,
    /// Sequence, starting at zero.
    pub seq: u64,
    /// Previous record digest, or none at sequence zero.
    pub previous: Option<String>,
    /// Controller pubkey.
    pub controller: String,
    /// Fencing generation, starting at zero.
    pub generation: u64,
    /// Record type.
    pub record_type: String,
    /// Step, iteration, and attempt. Nulls are run-level.
    pub subject: Subject,
    /// Observational Unix seconds. They do not order the chain.
    pub time: u64,
    /// Type-specific data.
    pub data: Map<String, Value>,
    /// Logical digest, `sha256:` prefixed.
    pub digest: String,
}

/// The subject a record attaches to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    /// Step slug, or none for the run itself.
    pub step: Option<String>,
    /// Iteration index, or none.
    pub iteration: Option<u64>,
    /// Attempt number, starting at 1. None is run-level state.
    pub attempt: Option<u64>,
}

/// What applying a delivered record did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ingest {
    /// The record extended the chain.
    Applied,
    /// The same digest was already applied.
    Duplicate,
    /// A sequence is missing before this one.
    Gap,
    /// Two records claim one sequence or predecessor.
    Fork,
}

/// Retained records. A fork keeps both and does not pick a winner.
#[derive(Debug, Clone, Default)]
pub struct Journal {
    /// Applied chain, in sequence order.
    pub applied: Vec<Record>,
    /// Every digest seen at each sequence, including a fork.
    pub retained: BTreeMap<u64, BTreeSet<String>>,
    /// True after a fork. Recovery does not continue automatically.
    pub conflict: bool,
}

/// How far the controller persisted before a crash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// Admission and reservation are durable. Acceptance is not acknowledged.
    Reserved,
    /// Acceptance was acknowledged after the reservation.
    Acknowledged,
    /// Dispatch intent is durable. The effect has not started.
    DispatchIntent,
    /// The effect started and its result was not observed.
    Effect,
    /// The observation is durable.
    Observed,
}

/// What a crash leaves. Unknown work is not rewritten as success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Durability {
    /// Reservation id, retained until reconciliation.
    pub reservation: Option<String>,
    /// Whether acceptance was acknowledged.
    pub acknowledged: bool,
    /// Whether dispatch intent was recorded.
    pub dispatch_intent: bool,
    /// Whether the effect was started.
    pub effect_started: bool,
    /// Whether an observation was recorded.
    pub observed: bool,
    /// Outcome word, or none.
    pub outcome: Option<String>,
    /// Verification word, or none.
    pub verification: Option<String>,
    /// Integration word, or none.
    pub integration: Option<String>,
}

/// A historical name kept as it was stored, plus the journal type it links to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateLink {
    /// The stored name, unchanged.
    pub historical: String,
    /// The journal type a new record would use.
    pub journal: String,
}

/// An evidence reference. Linking does not mint a new log id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceLink {
    /// Existing decision receipt id.
    pub receipt: Option<String>,
    /// Existing artifact digest.
    pub artifact: Option<String>,
}

/// Offline reconstruction. It does not execute anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    /// How many records verified.
    pub records: usize,
    /// Always false. Replay is not a new attempt.
    pub executed: bool,
}

/// One dispatcher that can still cause an effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispatcher {
    /// Dispatcher id.
    pub id: String,
    /// Whether it can still be reached.
    pub reachable: bool,
    /// Generation it has acknowledged, if any.
    pub acked_generation: Option<u64>,
}

/// Digest of a logical record: schema id, one LF, then JCS.
///
/// # Errors
///
/// Returns [`RefusalCode::Malformed`] when the record is not canonical JSON.
pub fn logical_digest(record: &Value) -> Result<String, ContractError> {
    let canonical = jcs(record)?;
    let mut bytes = Vec::with_capacity(RECORD_SCHEMA.len() + 1 + canonical.len());
    bytes.extend_from_slice(RECORD_SCHEMA.as_bytes());
    bytes.push(b'\n');
    bytes.extend_from_slice(&canonical);
    Ok(digest_bytes(&bytes))
}

/// Parse a decrypted envelope and require the digest to match the record.
///
/// # Errors
///
/// Returns a typed refusal for a bad shape, a digest mismatch, or an
/// unsupported record type.
pub fn parse_envelope(value: &Value) -> Result<Record, ContractError> {
    let object = as_map(value, "envelope")?;
    reject(object, &["v", "requires", "record", "digest"], "envelope")?;
    if text(require(object, "v", "envelope")?, "v")? != RECORD_SCHEMA {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "envelope.v",
        ));
    }
    let requires = require(object, "requires", "envelope")?
        .as_array()
        .ok_or_else(|| malformed("requires"))?;
    if !requires.is_empty() {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "requires",
        ));
    }
    let record = require(object, "record", "envelope")?;
    let digest = text(require(object, "digest", "envelope")?, "digest")?;
    if logical_digest(record)? != digest {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "record digest",
        ));
    }
    parse_record(record, digest)
}

/// Apply one delivered record. Order of arrival does not choose a winner.
///
/// # Errors
///
/// Returns a refusal for a forged controller, a record after `settled`, or
/// a dispatch of a subject that already resolved.
pub fn ingest(
    journal: &mut Journal,
    record: Record,
    signer: &str,
) -> Result<Ingest, ContractError> {
    if signer != record.controller {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "controller",
        ));
    }
    if journal.conflict {
        retain(journal, &record);
        return Ok(Ingest::Fork);
    }
    if let Some(existing) = journal.retained.get(&record.seq) {
        if existing.contains(&record.digest) && journal.applied.len() as u64 > record.seq {
            return Ok(Ingest::Duplicate);
        }
        if !existing.is_empty() && !existing.contains(&record.digest) {
            retain(journal, &record);
            journal.conflict = true;
            return Ok(Ingest::Fork);
        }
    }
    if record.seq > journal.applied.len() as u64 {
        retain(journal, &record);
        return Ok(Ingest::Gap);
    }
    if record.seq < journal.applied.len() as u64 {
        return Ok(Ingest::Duplicate);
    }
    check_link(journal, &record)?;
    journal.applied.push(record.clone());
    retain(journal, &record);
    Ok(Ingest::Applied)
}

/// What a crash at `boundary` leaves behind.
#[must_use]
pub fn crash(boundary: Boundary, reservation: &str) -> Durability {
    match boundary {
        Boundary::Reserved => Durability {
            reservation: Some(reservation.to_string()),
            acknowledged: false,
            dispatch_intent: false,
            effect_started: false,
            observed: false,
            outcome: None,
            verification: None,
            integration: None,
        },
        Boundary::Acknowledged => Durability {
            reservation: Some(reservation.to_string()),
            acknowledged: true,
            dispatch_intent: false,
            effect_started: false,
            observed: false,
            outcome: None,
            verification: None,
            integration: None,
        },
        Boundary::DispatchIntent => Durability {
            reservation: Some(reservation.to_string()),
            acknowledged: true,
            dispatch_intent: true,
            effect_started: false,
            observed: false,
            outcome: None,
            verification: None,
            integration: None,
        },
        Boundary::Effect => Durability {
            reservation: Some(reservation.to_string()),
            acknowledged: true,
            dispatch_intent: true,
            effect_started: true,
            observed: false,
            outcome: Some("unknown".to_string()),
            verification: None,
            integration: None,
        },
        Boundary::Observed => Durability {
            reservation: Some(reservation.to_string()),
            acknowledged: true,
            dispatch_intent: true,
            effect_started: true,
            observed: true,
            outcome: Some("resolved".to_string()),
            verification: Some("not_run".to_string()),
            integration: Some("pending".to_string()),
        },
    }
}

/// Whether the observation may be advertised as recoverable.
///
/// # Errors
///
/// Returns [`RefusalCode::Unavailable`] when the observation is not durable.
pub fn advertise(state: &Durability) -> Result<(), ContractError> {
    if state.observed {
        Ok(())
    } else {
        Err(ContractError::new(RefusalCode::Unavailable, "observation"))
    }
}

/// Acknowledge acceptance only after the reservation is durable.
///
/// # Errors
///
/// Returns [`RefusalCode::Unavailable`] when the reservation is missing.
pub fn acknowledge(state: &Durability) -> Result<(), ContractError> {
    if state.reservation.is_some() && !state.dispatch_intent {
        Ok(())
    } else if state.reservation.is_none() {
        Err(ContractError::new(RefusalCode::Unavailable, "reservation"))
    } else {
        Ok(())
    }
}

/// Start an effect only after dispatch intent is durable.
///
/// # Errors
///
/// Returns [`RefusalCode::Unavailable`] when the intent is missing.
pub fn begin_effect(state: &Durability) -> Result<(), ContractError> {
    if state.dispatch_intent {
        Ok(())
    } else {
        Err(ContractError::new(
            RefusalCode::Unavailable,
            "dispatch intent",
        ))
    }
}

/// Outcome, verification, and integration are all required for acceptance.
#[must_use]
pub fn accepted(outcome: &str, verification: &str, integration: &str) -> bool {
    outcome == "resolved" && verification == "passed" && integration == "accepted"
}

/// Hand the journal to `new_controller`.
///
/// A reachable dispatcher that has not acked the next generation blocks the
/// handoff. An unreachable one stays unknown. Without `shared_authority`,
/// an absent old controller cannot be replaced automatically.
///
/// # Errors
///
/// Returns [`RefusalCode::NotAdmitted`] for an automatic takeover, and
/// [`RefusalCode::CannotEnforce`] when a reachable dispatcher is unfenced.
pub fn handoff(
    old_present: bool,
    shared_authority: bool,
    next_generation: u64,
    dispatchers: &[Dispatcher],
) -> Result<Vec<String>, ContractError> {
    if !old_present && !shared_authority {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "automatic takeover",
        ));
    }
    let mut unknown = Vec::new();
    for dispatcher in dispatchers {
        if !dispatcher.reachable {
            unknown.push(dispatcher.id.clone());
            continue;
        }
        if dispatcher.acked_generation != Some(next_generation) {
            return Err(ContractError::new(
                RefusalCode::CannotEnforce,
                "unfenced dispatcher",
            ));
        }
    }
    Ok(unknown)
}

/// A dispatcher persists `generation` and rejects an older one.
///
/// # Errors
///
/// Returns [`RefusalCode::Stale`] when `offered` is older than `persisted`.
pub fn accept_generation(persisted: u64, offered: u64) -> Result<(), ContractError> {
    if offered < persisted {
        Err(ContractError::new(RefusalCode::Stale, "generation"))
    } else {
        Ok(())
    }
}

/// Accept a head hint only when it does not erase a longer local chain.
///
/// # Errors
///
/// Returns [`RefusalCode::Stale`] when the hint is behind the local journal.
pub fn accept_head(
    head_seq: u64,
    head_generation: u64,
    local_seq: u64,
    local_generation: u64,
) -> Result<(), ContractError> {
    if head_seq < local_seq || head_generation < local_generation {
        Err(ContractError::new(RefusalCode::Stale, "head"))
    } else {
        Ok(())
    }
}

/// An empty retrieval does not prove the journal ended.
#[must_use]
pub fn empty_retrieval_complete(returned: usize, eose: bool) -> bool {
    let _ = (returned, eose);
    false
}

/// A record past the retention horizon is unavailable, not complete.
///
/// # Errors
///
/// Returns [`RefusalCode::ContentUnavailable`] after `retain_until`.
pub fn within_horizon(now: u64, retain_until: u64) -> Result<(), ContractError> {
    if now > retain_until {
        Err(ContractError::new(
            RefusalCode::ContentUnavailable,
            "retention",
        ))
    } else {
        Ok(())
    }
}

/// A deleted payload leaves the record valid and the bytes unavailable.
///
/// # Errors
///
/// Returns [`RefusalCode::ContentUnavailable`] when the payload is gone.
pub fn payload_status(record_valid: bool, payload_present: bool) -> Result<bool, ContractError> {
    if !payload_present {
        return Err(ContractError::new(
            RefusalCode::ContentUnavailable,
            "payload",
        ));
    }
    Ok(record_valid)
}

/// Verify a retained chain without executing it.
///
/// # Errors
///
/// Returns the chain's refusal. A gap is [`RefusalCode::ContentUnavailable`].
pub fn replay_offline(envelopes: &[Value]) -> Result<Replay, ContractError> {
    let mut journal = Journal::default();
    for envelope in envelopes {
        let record = parse_envelope(envelope)?;
        let signer = record.controller.clone();
        match ingest(&mut journal, record, &signer)? {
            Ingest::Applied | Ingest::Duplicate => {}
            Ingest::Gap => {
                return Err(ContractError::new(RefusalCode::ContentUnavailable, "gap"));
            }
            Ingest::Fork => {
                return Err(ContractError::new(RefusalCode::Conflict, "fork"));
            }
        }
    }
    Ok(Replay {
        records: journal.applied.len(),
        executed: false,
    })
}

/// Link a stored state name. The historical spelling is not rewritten.
///
/// # Errors
///
/// Returns [`RefusalCode::UnsupportedFeature`] for a name this host does not store.
pub fn link_historical(name: &str) -> Result<StateLink, ContractError> {
    let journal = match name {
        "pending" => "admitted",
        "dispatched" => "dispatched",
        "answered" | "refused" | "unverifiable" | "cancelled" | "completed" | "failed" => {
            "resolved"
        }
        "settled" => "settled",
        "unknown" => "unknown",
        _ => {
            return Err(ContractError::new(RefusalCode::UnsupportedFeature, "state"));
        }
    };
    Ok(StateLink {
        historical: name.to_string(),
        journal: journal.to_string(),
    })
}

/// Point at an existing receipt and artifact. This does not allocate a new log id.
#[must_use]
pub fn link_evidence(receipt: Option<&str>, artifact: Option<&str>) -> EvidenceLink {
    EvidenceLink {
        receipt: receipt.map(str::to_string),
        artifact: artifact.map(str::to_string),
    }
}

/// Whether a run record may be shown to `readers`.
///
/// The author and the single `p` recipient may read it. A relay owner who is
/// neither sees nothing. A record with any other recipient shape is hidden.
#[must_use]
pub fn record_visible(event: &Event, readers: &HashSet<String>) -> bool {
    if event.kind != RECORD_KIND && event.kind != HEAD_KIND {
        return false;
    }
    if !readers.contains(&event.pubkey) && !recipient_is(event, readers) {
        return false;
    }
    envelope_tags_ok(event)
}

/// Run ciphertext is not a search document.
#[must_use]
pub fn searchable(kind: u16) -> bool {
    kind != RECORD_KIND && kind != HEAD_KIND
}

/// Encrypt a logical record to one recipient. The digest is inside the ciphertext.
///
/// # Errors
///
/// Returns the NIP-44 error when the plaintext is outside the client bound.
pub fn protect(
    plaintext: &str,
    secret: &SecretKey,
    peer: &XOnlyPublicKey,
    nonce: [u8; 32],
) -> Result<String, String> {
    let key = conversation_key(secret, peer);
    encrypt(plaintext, &key, nonce)
}

/// Decrypt a record sealed to `peer`.
///
/// # Errors
///
/// Returns the NIP-44 error when the payload is not for this key.
pub fn reveal(payload: &str, secret: &SecretKey, peer: &XOnlyPublicKey) -> Result<String, String> {
    let key = conversation_key(secret, peer);
    decrypt(payload, &key)
}

/// A mailbox is 64 hex characters and is not the run id.
///
/// # Errors
///
/// Returns [`RefusalCode::Malformed`] when the mailbox is not that shape.
pub fn check_mailbox(mailbox: &str, run_id: &str) -> Result<(), ContractError> {
    if mailbox == run_id || mailbox.contains('/') || mailbox.contains('\\') || !is_hex(mailbox) {
        return Err(malformed("mailbox"));
    }
    Ok(())
}

fn check_link(journal: &Journal, record: &Record) -> Result<(), ContractError> {
    if journal.applied.is_empty() {
        if record.seq != 0
            || record.previous.is_some()
            || record.record_type != "created"
            || record.generation != 0
        {
            return Err(malformed("created"));
        }
        let owner = record
            .data
            .get("owner")
            .and_then(Value::as_str)
            .unwrap_or("");
        if owner != record.controller {
            return Err(ContractError::new(RefusalCode::IdentityMismatch, "owner"));
        }
        return Ok(());
    }
    let previous = journal.applied.last().expect("the chain is not empty");
    if previous.record_type == "settled" {
        return Err(ContractError::new(RefusalCode::Conflict, "settled"));
    }
    if record.seq != previous.seq + 1 {
        return Err(malformed("seq"));
    }
    if record.previous.as_deref() != Some(previous.digest.as_str()) {
        return Err(ContractError::new(RefusalCode::Conflict, "previous"));
    }
    if previous.record_type == "handoff" {
        let next = previous
            .data
            .get("controller")
            .and_then(Value::as_str)
            .unwrap_or("");
        if record.controller != next || record.generation != previous.generation + 1 {
            return Err(ContractError::new(RefusalCode::IdentityMismatch, "handoff"));
        }
        return Ok(());
    }
    if record.controller != previous.controller || record.generation != previous.generation {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "controller",
        ));
    }
    if record.record_type == "dispatched" && subject_resolved(journal, record) {
        return Err(ContractError::new(
            RefusalCode::Conflict,
            "resolved subject",
        ));
    }
    Ok(())
}

fn subject_resolved(journal: &Journal, record: &Record) -> bool {
    journal.applied.iter().any(|earlier| {
        earlier.record_type == "resolved"
            && earlier.subject == record.subject
            && earlier.run == record.run
    })
}

fn retain(journal: &mut Journal, record: &Record) {
    journal
        .retained
        .entry(record.seq)
        .or_default()
        .insert(record.digest.clone());
}

fn parse_record(value: &Value, digest: &str) -> Result<Record, ContractError> {
    let object = as_map(value, "record")?;
    reject(
        object,
        &[
            "run",
            "seq",
            "previous",
            "controller",
            "generation",
            "type",
            "subject",
            "time",
            "data",
        ],
        "record",
    )?;
    let run = text(require(object, "run", "record")?, "run")?.to_string();
    let seq = require(object, "seq", "record")?
        .as_u64()
        .ok_or_else(|| malformed("seq"))?;
    let previous = match object.get("previous") {
        Some(Value::Null) => None,
        Some(value) => Some(digest_text(value)?),
        None => return Err(malformed("previous")),
    };
    if seq == 0 && previous.is_some() || seq > 0 && previous.is_none() {
        return Err(malformed("previous"));
    }
    let controller = hex_key(text(
        require(object, "controller", "record")?,
        "controller",
    )?)?;
    let generation = require(object, "generation", "record")?
        .as_u64()
        .ok_or_else(|| malformed("generation"))?;
    let record_type = text(require(object, "type", "record")?, "type")?.to_string();
    if !RECORD_TYPES.contains(&record_type.as_str()) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "record type",
        ));
    }
    let subject = parse_subject(require(object, "subject", "record")?)?;
    let time = require(object, "time", "record")?
        .as_u64()
        .ok_or_else(|| malformed("time"))?;
    let data = as_map(require(object, "data", "record")?, "data")?.clone();
    require_data(&record_type, &data)?;
    Ok(Record {
        run,
        seq,
        previous,
        controller,
        generation,
        record_type,
        subject,
        time,
        data,
        digest: digest.to_string(),
    })
}

fn require_data(record_type: &str, data: &Map<String, Value>) -> Result<(), ContractError> {
    let required: &[&str] = match record_type {
        "created" => &[
            "owner",
            "request",
            "base",
            "program",
            "lock",
            "context",
            "policy",
            "parent",
            "recipients",
        ],
        "admitted" => &[
            "enforcement",
            "reservations",
            "deadlines",
            "retention",
            "input",
        ],
        "dispatched" => &[
            "binding",
            "attempt",
            "input",
            "context",
            "effect",
            "generation",
        ],
        "observed" => &["evidence", "sources", "complete", "provenance"],
        "resolved" => &[
            "outcome",
            "dispatched",
            "output",
            "receipts",
            "usage",
            "verification",
            "integration",
        ],
        "cancel_requested" => &["requester", "scope", "reason"],
        "unknown" => &["attempt", "reservation", "evidence"],
        "reconciled" => &["previous", "evidence", "outcome", "accounting"],
        "handoff" => &["controller", "generation", "fencing"],
        "settled" => &[
            "outcome",
            "unknowns",
            "verification",
            "integration",
            "results",
            "reservations",
        ],
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "record type",
            ));
        }
    };
    for key in required {
        if !data.contains_key(*key) {
            return Err(malformed(*key));
        }
    }
    Ok(())
}

fn parse_subject(value: &Value) -> Result<Subject, ContractError> {
    let object = as_map(value, "subject")?;
    reject(object, &["step", "iteration", "attempt"], "subject")?;
    let step = match object.get("step") {
        Some(Value::Null) => None,
        Some(value) => Some(text(value, "step")?.to_string()),
        None => return Err(malformed("step")),
    };
    let iteration = optional_u64(object, "iteration")?;
    let attempt = optional_u64(object, "attempt")?;
    if attempt == Some(0) {
        return Err(malformed("attempt"));
    }
    Ok(Subject {
        step,
        iteration,
        attempt,
    })
}

fn envelope_tags_ok(event: &Event) -> bool {
    let recipients: Vec<&str> = event.tag_values("p").collect();
    let mailboxes: Vec<&str> = event.tag_values("h").collect();
    let marked = event
        .tag_values("t")
        .filter(|value| *value == MARKER)
        .count()
        == 1;
    if recipients.len() != 1 || mailboxes.len() != 1 || !marked || !is_hex(mailboxes[0]) {
        return false;
    }
    if event.kind == HEAD_KIND {
        let identifiers: Vec<&str> = event.tag_values("d").collect();
        return identifiers.len() == 1 && identifiers[0] == mailboxes[0];
    }
    true
}

fn recipient_is(event: &Event, readers: &HashSet<String>) -> bool {
    let recipients: Vec<&str> = event.tag_values("p").collect();
    recipients.len() == 1 && readers.contains(recipients[0])
}

fn optional_u64(object: &Map<String, Value>, key: &str) -> Result<Option<u64>, ContractError> {
    match object.get(key) {
        Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(value.as_u64().ok_or_else(|| malformed(key))?)),
        None => Err(malformed(key)),
    }
}

fn digest_text(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "digest")?;
    let Some(hex) = text.strip_prefix("sha256:") else {
        return Err(malformed("digest"));
    };
    if !is_hex(hex) {
        return Err(malformed("digest"));
    }
    Ok(text.to_string())
}

fn hex_key(value: &str) -> Result<String, ContractError> {
    if is_hex(value) {
        Ok(value.to_string())
    } else {
        Err(malformed("pubkey"))
    }
}

fn as_map<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, ContractError> {
    value.as_object().ok_or_else(|| malformed(path))
}

fn require<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Value, ContractError> {
    object
        .get(key)
        .ok_or_else(|| malformed(format!("{path}.{key}")))
}

fn reject(object: &Map<String, Value>, allowed: &[&str], path: &str) -> Result<(), ContractError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                format!("{path}.{key}"),
            ));
        }
    }
    Ok(())
}

fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str, ContractError> {
    value.as_str().ok_or_else(|| malformed(path))
}

fn is_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn malformed(detail: impl Into<String>) -> ContractError {
    ContractError::new(RefusalCode::Malformed, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};
    use serde_json::json;

    fn signer(byte: u8) -> (RelaySigner, SecretKey) {
        let secret = [byte; 32];
        let key = SecretKey::from_byte_array(secret).unwrap();
        let hex: String = secret.iter().map(|item| format!("{item:02x}")).collect();
        (RelaySigner::from_secret_hex(&hex).unwrap(), key)
    }

    fn peer(signer: &RelaySigner) -> XOnlyPublicKey {
        let bytes: [u8; 32] = hex_decode(signer.pubkey()).try_into().unwrap();
        XOnlyPublicKey::from_byte_array(bytes).unwrap()
    }

    fn hex_decode(value: &str) -> Vec<u8> {
        (0..value.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&value[index..index + 2], 16).unwrap())
            .collect()
    }

    fn subject() -> Value {
        json!({"step": null, "iteration": null, "attempt": null})
    }

    fn created(controller: &str) -> Value {
        json!({
            "run": "run-1",
            "seq": 0,
            "previous": null,
            "controller": controller,
            "generation": 0,
            "type": "created",
            "subject": subject(),
            "time": 10,
            "data": {
                "owner": controller,
                "request": "req-1",
                "base": "base-1",
                "program": "sha256:aa",
                "lock": "sha256:bb",
                "context": "sha256:cc",
                "policy": "policy-1",
                "parent": null,
                "recipients": [controller]
            }
        })
    }

    fn envelope(record: Value) -> Value {
        let digest = logical_digest(&record).unwrap();
        json!({
            "v": RECORD_SCHEMA,
            "requires": [],
            "record": record,
            "digest": digest
        })
    }

    fn record_of(body: Value) -> Record {
        parse_envelope(&envelope(body)).unwrap()
    }

    fn tagged(kind: u16, author: &RelaySigner, recipient: &str, mailbox: &str) -> Event {
        let mut tags = vec![
            Tag::new(vec!["p".into(), recipient.into()]),
            Tag::new(vec!["h".into(), mailbox.into()]),
            Tag::new(vec!["t".into(), MARKER.into()]),
        ];
        if kind == HEAD_KIND {
            tags.push(Tag::new(vec!["d".into(), mailbox.into()]));
        }
        author.sign(20, kind, tags, "ciphertext".into())
    }

    #[test]
    fn the_logical_digest_is_stable_across_recipients_and_a_fork_keeps_both() {
        let (author, author_key) = signer(0x11);
        let (recipient, recipient_key) = signer(0x22);
        let (owner, _) = signer(0x33);
        let record = created(author.pubkey());
        let body = envelope(record);
        let digest = body["digest"].as_str().unwrap().to_string();
        let plaintext = body.to_string();
        let nonce = [7_u8; 32];
        let to_recipient = protect(&plaintext, &author_key, &peer(&recipient), nonce).unwrap();
        let to_owner = protect(&plaintext, &author_key, &peer(&owner), nonce).unwrap();
        assert_ne!(to_recipient, to_owner);
        let opened = reveal(&to_recipient, &recipient_key, &peer(&author)).unwrap();
        assert_eq!(
            parse_envelope(&serde_json::from_str(&opened).unwrap())
                .unwrap()
                .digest,
            digest
        );

        let mut journal = Journal::default();
        let first = record_of(created(author.pubkey()));
        assert_eq!(
            ingest(&mut journal, first.clone(), author.pubkey()).unwrap(),
            Ingest::Applied
        );
        assert_eq!(
            ingest(&mut journal, first, author.pubkey()).unwrap(),
            Ingest::Duplicate
        );
        let mut forked = created(author.pubkey());
        forked["time"] = json!(11);
        let forked = record_of(forked);
        assert_ne!(forked.digest, digest);
        assert_eq!(
            ingest(&mut journal, forked, author.pubkey()).unwrap(),
            Ingest::Fork
        );
        assert!(journal.conflict);
        assert_eq!(journal.retained.get(&0).unwrap().len(), 2);

        let mailbox = "ab".repeat(32);
        check_mailbox(&mailbox, "run-1").unwrap();
        assert!(check_mailbox("run-1", "run-1").is_err());
        let readers = HashSet::from([recipient.pubkey().to_string()]);
        let event = tagged(RECORD_KIND, &author, recipient.pubkey(), &mailbox);
        assert!(record_visible(&event, &readers));
        assert!(record_visible(
            &event,
            &HashSet::from([author.pubkey().to_string()])
        ));
        assert!(!record_visible(
            &event,
            &HashSet::from([owner.pubkey().to_string()])
        ));
        assert!(!searchable(RECORD_KIND));
        assert!(!searchable(HEAD_KIND));
        assert!(!empty_retrieval_complete(0, true));
    }

    #[test]
    fn crashes_handoffs_and_replay_do_not_invent_an_effect() {
        let reserved = crash(Boundary::Reserved, "hold-1");
        assert!(acknowledge(&reserved).is_ok());
        assert!(!reserved.acknowledged);
        assert!(begin_effect(&reserved).is_err());
        let intent = crash(Boundary::DispatchIntent, "hold-1");
        assert!(begin_effect(&intent).is_ok());
        assert!(advertise(&intent).is_err());
        let crashed = crash(Boundary::Effect, "hold-1");
        assert_eq!(crashed.outcome.as_deref(), Some("unknown"));
        assert!(crashed.reservation.is_some());
        assert!(crashed.verification.is_none());
        assert!(crashed.integration.is_none());
        assert!(!accepted("unknown", "not_run", "pending"));
        assert!(accepted("resolved", "passed", "accepted"));
        assert!(advertise(&crash(Boundary::Observed, "hold-1")).is_ok());

        let fenced = [Dispatcher {
            id: "worker".into(),
            reachable: true,
            acked_generation: Some(1),
        }];
        assert!(handoff(false, false, 1, &fenced).is_err());
        let unknown = handoff(
            true,
            false,
            1,
            &[
                Dispatcher {
                    id: "worker".into(),
                    reachable: true,
                    acked_generation: Some(1),
                },
                Dispatcher {
                    id: "gone".into(),
                    reachable: false,
                    acked_generation: None,
                },
            ],
        )
        .unwrap();
        assert_eq!(unknown, vec!["gone".to_string()]);
        assert!(
            handoff(
                true,
                false,
                1,
                &[Dispatcher {
                    id: "worker".into(),
                    reachable: true,
                    acked_generation: None,
                }]
            )
            .is_err()
        );
        assert!(accept_generation(2, 1).is_err());
        assert!(accept_head(1, 0, 3, 1).is_err());
        assert!(within_horizon(50, 40).is_err());
        assert!(payload_status(true, false).is_err());

        let (author, _) = signer(0x11);
        let mut second = created(author.pubkey());
        second["seq"] = json!(1);
        second["previous"] = json!(logical_digest(&created(author.pubkey())).unwrap());
        second["type"] = json!("admitted");
        second["data"] = json!({
            "enforcement": "host",
            "reservations": ["hold-1"],
            "deadlines": {},
            "retention": {"retain_until": 100},
            "input": "sha256:dd"
        });
        let replay =
            replay_offline(&[envelope(created(author.pubkey())), envelope(second)]).unwrap();
        assert_eq!(replay.records, 2);
        assert!(!replay.executed);
        assert_eq!(
            replay_offline(&[envelope(second_only())]).unwrap_err().code,
            RefusalCode::ContentUnavailable
        );
    }

    fn second_only() -> Value {
        let (author, _) = signer(0x11);
        let mut record = created(author.pubkey());
        record["seq"] = json!(1);
        record["previous"] =
            json!("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        record["type"] = json!("admitted");
        record["data"] = json!({
            "enforcement": "host",
            "reservations": [],
            "deadlines": {},
            "retention": {},
            "input": "sha256:dd"
        });
        record
    }

    #[test]
    fn historical_names_and_receipts_stay_themselves() {
        let unknown = link_historical("unknown").unwrap();
        assert_eq!(unknown.historical, "unknown");
        assert_eq!(unknown.journal, "unknown");
        let failed = link_historical("failed").unwrap();
        assert_eq!(failed.historical, "failed");
        assert_ne!(failed.journal, "unknown");
        let pending = link_historical("pending").unwrap();
        assert_eq!(pending.historical, "pending");
        assert_eq!(pending.journal, "admitted");
        let link = link_evidence(Some("receipt-7"), Some("sha256:abc"));
        assert_eq!(link.receipt.as_deref(), Some("receipt-7"));
        assert_eq!(link.artifact.as_deref(), Some("sha256:abc"));
        let (author, _) = signer(0x11);
        let mut forged = created(author.pubkey());
        forged["data"]["owner"] = json!("aa".repeat(32));
        assert_eq!(
            ingest(&mut Journal::default(), record_of(forged), author.pubkey())
                .unwrap_err()
                .code,
            RefusalCode::IdentityMismatch
        );
    }
}
