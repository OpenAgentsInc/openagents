//! The NIP-CJ execution family: kinds `25920`, `26920`, and `27020`.
//!
//! Conversation jobs use integer `v: 1`. Decision jobs use
//! `openagents.systemone.v1`. This module accepts only
//! `openagents.execution.v1` on its own kinds. A payload from another
//! family is refused here, and an execution payload is not a decision
//! call.
//!
//! The relay delivers ciphertext. Acceptance is a worker claim persisted
//! before a `27020` `accepted` payload. A relay `OK` and a closed socket
//! do not accept or cancel a run. The same idempotency key with the same
//! fingerprint is a retransmission. Changed bytes are
//! `idempotency_conflict`. A crash after dispatch intent stays `unknown`,
//! and this layer does not promise exactly-once effects.

use std::collections::{BTreeMap, BTreeSet};

use secp256k1::{Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::contracts::{
    ArtifactRef, ContractError, DefinitionRef, RefusalCode, digest_bytes, digest_value,
    parse_artifact, parse_definition,
};
use crate::domain::{DomainError, Event, RelaySigner, Tag};
use crate::nip44;
use crate::run::{self, Durability};

/// Execution request or control. Ephemeral.
pub const REQUEST_KIND: u16 = 25_920;
/// Execution result or control answer. Ephemeral.
pub const RESULT_KIND: u16 = 26_920;
/// Execution admission and progress. Ephemeral.
pub const FEEDBACK_KIND: u16 = 27_020;
/// Payload schema for every body in this family.
pub const SCHEMA: &str = "openagents.execution.v1";

const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
const MAX_ID_BYTES: usize = 128;
const MAX_REASON_BYTES: usize = 1_024;
const MAX_REPLAY: u32 = 256;
const CREDENTIALS: &[&str] = &[
    "key",
    "secret",
    "authorization",
    "credential",
    "credentials",
    "api_key",
    "apikey",
    "bearer",
    "token",
];

/// How far `created_at` may sit from the worker clock, in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// How far in the past `created_at` may be.
    pub max_age_seconds: u64,
    /// How far in the future `created_at` may be.
    pub max_future_seconds: u64,
}

impl Window {
    /// Ten minutes back, five minutes ahead.
    pub const DEFAULT: Self = Self {
        max_age_seconds: 10 * 60,
        max_future_seconds: 5 * 60,
    };

    /// Whether `created_at` is inside this window at `now`.
    #[must_use]
    pub fn admits(self, created_at: u64, now: u64) -> bool {
        now.saturating_sub(created_at) <= self.max_age_seconds
            && created_at.saturating_sub(now) <= self.max_future_seconds
    }
}

/// Why an execution event was not accepted.
#[derive(Debug)]
pub enum Error {
    /// The kind belongs to another family, or to no job family.
    UnexpectedKind {
        /// The event kind.
        kind: u16,
    },
    /// A signed event does not name this job.
    Unbound {
        /// The field that failed to match.
        field: &'static str,
    },
    /// The `p` tag does not name this worker.
    NotAddressed,
    /// The signature does not prove the claimed author.
    Event(DomainError),
    /// The body is not the execution shape.
    Malformed {
        /// A field path.
        detail: String,
    },
    /// `v` is not [`SCHEMA`].
    UnsupportedVersion,
    /// A field has no defined behavior.
    UnsupportedFeature {
        /// The field name.
        detail: String,
    },
    /// The signer may not control this run.
    NotAdmitted,
    /// The worker cannot answer yet.
    Unavailable,
    /// Pinned bytes or an expired record are absent.
    ContentUnavailable,
    /// A digest does not match the bytes.
    IdentityMismatch,
    /// The event or its deadline is outside the admitted window.
    Stale,
    /// The worker cannot enforce the requested semantics.
    CannotEnforce {
        /// What cannot be enforced.
        detail: String,
    },
    /// A ceiling was exceeded.
    LimitExceeded,
    /// The idempotency key arrived with a different fingerprint.
    IdempotencyConflict,
    /// Two terminal results disagree.
    Conflict,
    /// Every execution slot is taken.
    Busy,
    /// The target pin was revoked.
    Revoked,
}

impl Error {
    /// The refusal code, when the signer is known well enough to be told.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::UnexpectedKind { .. }
            | Self::Unbound { .. }
            | Self::NotAddressed
            | Self::Event(_) => None,
            Self::Malformed { .. } => Some("malformed"),
            Self::UnsupportedVersion => Some("unsupported_version"),
            Self::UnsupportedFeature { .. } => Some("unsupported_feature"),
            Self::NotAdmitted => Some("not_admitted"),
            Self::Unavailable => Some("unavailable"),
            Self::ContentUnavailable => Some("content_unavailable"),
            Self::IdentityMismatch => Some("identity_mismatch"),
            Self::Stale => Some("stale"),
            Self::CannotEnforce { .. } => Some("cannot_enforce"),
            Self::LimitExceeded => Some("limit_exceeded"),
            Self::IdempotencyConflict => Some("idempotency_conflict"),
            Self::Conflict => Some("conflict"),
            Self::Busy => Some("busy"),
            Self::Revoked => Some("revoked"),
        }
    }
}

impl From<DomainError> for Error {
    fn from(error: DomainError) -> Self {
        Self::Event(error)
    }
}

impl From<ContractError> for Error {
    fn from(error: ContractError) -> Self {
        let detail = error.detail.clone();
        match error.code {
            RefusalCode::Malformed => Self::Malformed { detail },
            RefusalCode::UnsupportedVersion => Self::UnsupportedVersion,
            RefusalCode::UnsupportedFeature => Self::UnsupportedFeature { detail },
            RefusalCode::NotAdmitted => Self::NotAdmitted,
            RefusalCode::Unavailable => Self::Unavailable,
            RefusalCode::ContentUnavailable => Self::ContentUnavailable,
            RefusalCode::IdentityMismatch => Self::IdentityMismatch,
            RefusalCode::Stale => Self::Stale,
            RefusalCode::CannotEnforce => Self::CannotEnforce { detail },
            RefusalCode::LimitExceeded => Self::LimitExceeded,
            RefusalCode::IdempotencyConflict => Self::IdempotencyConflict,
            RefusalCode::Conflict => Self::Conflict,
            RefusalCode::Incompatible | RefusalCode::Revoked => Self::Revoked,
        }
    }
}

/// Whole-attempt ceilings. An absent field is not a zero budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bounds {
    /// Wall-clock ceiling, in milliseconds.
    pub wall_ms: Option<u64>,
    /// Output byte ceiling.
    pub output_bytes: Option<u64>,
    /// Concurrent job ceiling.
    pub jobs: Option<u64>,
    /// Spend ceiling, in microunits.
    pub spend_microunits: Option<u64>,
}

/// Parent attribution. It grants no authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parent {
    /// Parent run id.
    pub run: String,
    /// Parent step.
    pub step: String,
    /// Parent iteration.
    pub iteration: u64,
    /// Parent attempt.
    pub attempt: u32,
}

/// A validated execute body.
#[derive(Debug, Clone)]
pub struct Execute {
    /// The decrypted object. The fingerprint covers these exact bytes.
    pub payload: Value,
    /// SHA-256 of the JCS execute body, `sha256:` prefixed.
    pub fingerprint: String,
    /// Logical request id.
    pub request: String,
    /// Positive attempt number.
    pub attempt: u32,
    /// Logical run id.
    pub run: String,
    /// Pinned target.
    pub target: DefinitionRef,
    /// Dependency lock.
    pub lock: ArtifactRef,
    /// Typed input.
    pub input: Value,
    /// Digest of the canonical input.
    pub input_digest: String,
    /// Artifact input, when the body names one.
    pub input_artifact: Option<ArtifactRef>,
    /// Context manifest.
    pub context: ArtifactRef,
    /// Requirements manifest. Not a grant.
    pub requirements: ArtifactRef,
    /// Attempt ceilings.
    pub bounds: Bounds,
    /// Unix-second deadline. It matches the expiration tag.
    pub deadline: u64,
    /// Recovery horizon, later than `deadline`.
    pub retain_until: u64,
    /// Optional parent attribution.
    pub parent: Option<Parent>,
}

/// A control carried on kind `25920`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    /// Ask for the current state.
    Status,
    /// Ask for retained record references.
    Replay {
        /// Exclusive lower bound. `None` starts at the root.
        after_seq: Option<u64>,
        /// Maximum references to return.
        max_records: u32,
    },
    /// Ask the worker to stop queued work.
    Cancel {
        /// Bounded reason.
        reason: String,
    },
}

/// A request event that passed the signature checks.
#[derive(Debug, Clone)]
pub struct Opened {
    /// Verified signer, the principal.
    pub principal: String,
    /// Request event id.
    pub event_id: String,
    /// `created_at` of that event.
    pub created_at: u64,
    /// Execute or control body.
    pub body: Body,
}

/// Which body an opened request carries.
#[derive(Debug, Clone)]
pub enum Body {
    /// An execute request.
    Execute(Box<Execute>),
    /// A status, replay, or cancel control.
    Control {
        /// Logical request id.
        request: String,
        /// Attempt number.
        attempt: u32,
        /// Run id.
        run: String,
        /// The accepted execute event the `e` tag names.
        target: String,
        /// Control body.
        control: Control,
    },
}

/// What preparing a new or repeated execute produced.
#[derive(Debug, Clone, PartialEq)]
pub enum Admission {
    /// A new claim, reserved and not yet acknowledged.
    Reserved {
        /// The claim as reserved.
        claim: Claim,
        /// NIP-RUN root bytes the host persists before acknowledgement.
        root: Vec<u8>,
    },
    /// The same key and fingerprint. No new reservation.
    Retransmission(Claim),
}

/// One progress note. It is not the run journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// Monotone counter.
    pub seq: u64,
    /// `queued`, `running`, or `reconciling`.
    pub status: String,
}

/// One retained journal reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeqRecord {
    /// Journal sequence.
    pub seq: u64,
    /// Logical record digest.
    pub digest: String,
}

/// Durable claim for one `(worker, principal, request, attempt)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    /// Idempotency key.
    pub key: String,
    /// Worker pubkey.
    pub worker: String,
    /// Request signer.
    pub principal: String,
    /// Logical request id.
    pub request: String,
    /// Attempt number.
    pub attempt: u32,
    /// Run id.
    pub run: String,
    /// Execute-body fingerprint.
    pub fingerprint: String,
    /// Execute event id.
    pub execute_event: String,
    /// Deadline, Unix seconds.
    pub deadline: u64,
    /// Retention horizon copied from the request.
    pub retain_until: u64,
    /// Input digest.
    pub input_digest: String,
    /// Lock digest.
    pub lock_digest: String,
    /// Mailbox. It is not the run id.
    pub mailbox: String,
    /// Controller pubkey.
    pub controller: String,
    /// Artifact reference of the NIP-RUN root.
    pub record: Value,
    /// Digest of the persisted root bytes.
    pub record_digest: String,
    /// Lifecycle word.
    pub phase: String,
    /// Reservation id. Kept while the outcome is unknown.
    pub reservation: String,
    /// Acceptance was acknowledged.
    pub acknowledged: bool,
    /// Dispatch intent is durable.
    pub dispatch_intent: bool,
    /// The effect was started.
    pub effect_started: bool,
    /// A terminal observation is durable.
    pub observed: bool,
    /// Terminal outcome, when one is known.
    pub outcome: Option<String>,
    /// `Some(false)` only when dispatch is known not to have started.
    pub dispatched: Option<bool>,
    /// Known spend. `None` is unknown, not zero.
    pub spend: Option<u64>,
    /// Typed output, when the attempt produced one.
    pub output: Option<Value>,
    /// Published result object, when one was persisted.
    pub result: Option<Value>,
    /// Digest of that result.
    pub result_digest: Option<String>,
    /// Digests of conflicting terminal results.
    pub conflicts: Vec<String>,
    /// True when terminal results disagree.
    pub conflict: bool,
    /// Verification word.
    pub verification: Option<String>,
    /// Integration word.
    pub integration: Option<String>,
    /// Progress notes.
    pub progress: Vec<Progress>,
    /// Journal references.
    pub records: Vec<SeqRecord>,
    /// Transport event ids for this attempt.
    pub aliases: Vec<String>,
    /// A cancel control was authenticated.
    pub cancel_requested: bool,
    /// The supervisor confirmed the effect stopped.
    pub cancel_confirmed: bool,
    /// Retention expired. The fingerprint remains.
    pub tombstone: bool,
    /// Refusal code, when the attempt was refused.
    pub code: Option<String>,
    /// Refusal message.
    pub message: Option<String>,
    /// Whether this claim holds a capacity slot.
    pub holds_slot: bool,
}

impl Claim {
    fn durability(&self) -> Durability {
        Durability {
            reservation: Some(self.reservation.clone()),
            acknowledged: self.acknowledged,
            dispatch_intent: self.dispatch_intent,
            effect_started: self.effect_started,
            observed: self.observed,
            outcome: self.outcome.clone(),
            verification: self.verification.clone(),
            integration: self.integration.clone(),
        }
    }

    /// The outcome a reader may treat as settled.
    ///
    /// A conflict has no winner. First arrival does not select one.
    #[must_use]
    pub fn winner(&self) -> Option<&str> {
        if self.conflict {
            None
        } else {
            self.outcome.as_deref()
        }
    }
}

/// In-memory execution ledger. The host persists it before acknowledging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    /// Worker pubkey.
    pub worker: String,
    /// Concurrent execute claims.
    pub capacity: u32,
    /// Claims currently holding a slot.
    pub active: u32,
    /// Latest `retain_until` this worker will promise.
    pub horizon: u64,
    /// Target digests the operator revoked.
    pub revoked: BTreeSet<String>,
    /// Principals who may control runs they did not sign.
    pub controllers: BTreeSet<String>,
    /// Claims by idempotency key.
    pub claims: BTreeMap<String, Claim>,
}

impl Service {
    /// An empty ledger for `worker`.
    #[must_use]
    pub fn new(worker: impl Into<String>, capacity: u32, horizon: u64) -> Self {
        Self {
            worker: worker.into(),
            capacity,
            active: 0,
            horizon,
            revoked: BTreeSet::new(),
            controllers: BTreeSet::new(),
            claims: BTreeMap::new(),
        }
    }

    /// The protocol does not promise exactly-once effects.
    #[must_use]
    pub const fn promises_exactly_once(&self) -> bool {
        false
    }

    /// A relay `OK` is delivery, not acceptance.
    #[must_use]
    pub const fn relay_ok_is_acceptance(&self) -> bool {
        false
    }

    /// A closed socket does not cancel remote work.
    #[must_use]
    pub const fn socket_close_is_cancel(&self) -> bool {
        false
    }

    /// NIP-40 expiration does not stop a subprocess.
    #[must_use]
    pub const fn expiration_stops_subprocess(&self) -> bool {
        false
    }

    /// Record that `digest` may not be executed.
    pub fn revoke_target(&mut self, digest: impl Into<String>) {
        self.revoked.insert(digest.into());
    }

    /// Idempotency key `(worker, principal, request, attempt)`.
    #[must_use]
    pub fn key(&self, principal: &str, request: &str, attempt: u32) -> String {
        format!("{}|{principal}|{request}|{attempt}", self.worker)
    }

    /// Reserve a new execute claim, or return the recorded one.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal when the body cannot be claimed. A matching
    /// fingerprint never reserves a second slot, including after `deadline`.
    pub fn prepare(
        &mut self,
        opened: &Opened,
        now: u64,
        mailbox: &str,
    ) -> Result<Admission, Error> {
        let Body::Execute(execute) = &opened.body else {
            return Err(Error::Malformed {
                detail: "control".into(),
            });
        };
        run::check_mailbox(mailbox, &execute.run)?;
        let key = self.key(&opened.principal, &execute.request, execute.attempt);
        if let Some(existing) = self.claims.get(&key) {
            if existing.fingerprint != execute.fingerprint {
                return Err(Error::IdempotencyConflict);
            }
            let claim = self.claims.get_mut(&key).expect("the claim was just found");
            if !claim.aliases.contains(&opened.event_id) {
                claim.aliases.push(opened.event_id.clone());
            }
            return Ok(Admission::Retransmission(claim.clone()));
        }
        if execute.retain_until > self.horizon {
            return Err(Error::LimitExceeded);
        }
        if self.revoked.contains(&execute.target.artifact.digest) {
            return Err(Error::Revoked);
        }
        if now >= execute.deadline {
            return Err(Error::Stale);
        }
        self.require_previous(&opened.principal, &execute.request, execute.attempt)?;
        if self.active >= self.capacity {
            return Err(Error::Busy);
        }
        let envelope = root_envelope(execute, &self.worker, now)?;
        let root_bytes = serde_json::to_vec(&envelope).map_err(|_| Error::Unavailable)?;
        let record_digest = digest_bytes(&root_bytes);
        let record = json!({
            "digest": record_digest,
            "size": root_bytes.len() as u64,
            "media_type": "application/json"
        });
        let claim = Claim {
            key: key.clone(),
            worker: self.worker.clone(),
            principal: opened.principal.clone(),
            request: execute.request.clone(),
            attempt: execute.attempt,
            run: execute.run.clone(),
            fingerprint: execute.fingerprint.clone(),
            execute_event: opened.event_id.clone(),
            deadline: execute.deadline,
            retain_until: execute.retain_until,
            input_digest: execute.input_digest.clone(),
            lock_digest: execute.lock.digest.clone(),
            mailbox: mailbox.to_string(),
            controller: self.worker.clone(),
            record,
            record_digest,
            phase: "reserved".into(),
            reservation: format!("reservation:{key}"),
            acknowledged: false,
            dispatch_intent: false,
            effect_started: false,
            observed: false,
            outcome: None,
            dispatched: Some(false),
            spend: None,
            output: None,
            result: None,
            result_digest: None,
            conflicts: Vec::new(),
            conflict: false,
            verification: None,
            integration: None,
            progress: Vec::new(),
            records: vec![SeqRecord {
                seq: 0,
                digest: envelope["digest"].as_str().unwrap_or_default().to_string(),
            }],
            aliases: vec![opened.event_id.clone()],
            cancel_requested: false,
            cancel_confirmed: false,
            tombstone: false,
            code: None,
            message: None,
            holds_slot: true,
        };
        self.active += 1;
        self.claims.insert(key, claim.clone());
        Ok(Admission::Reserved {
            claim,
            root: root_bytes,
        })
    }

    /// Acknowledge a reserved claim after the host has persisted `root`.
    ///
    /// `accepted.retain_until` is the request's horizon. This method does
    /// not substitute a shorter one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IdentityMismatch`] when `root` is not the reserved
    /// bytes, and [`Error::Unavailable`] when the reservation is missing.
    pub fn acknowledge(&mut self, key: &str, root: &[u8]) -> Result<Value, Error> {
        let claim = self.claims.get_mut(key).ok_or(Error::Unavailable)?;
        if claim.tombstone {
            return Err(Error::ContentUnavailable);
        }
        if digest_bytes(root) != claim.record_digest {
            return Err(Error::IdentityMismatch);
        }
        if claim.acknowledged {
            return Ok(accepted_payload(claim));
        }
        run::acknowledge(&claim.durability())?;
        claim.acknowledged = true;
        claim.phase = "accepted".into();
        Ok(accepted_payload(claim))
    }

    /// Drop a reservation that was never persisted.
    pub fn rollback(&mut self, key: &str) {
        let Some(claim) = self.claims.get(key) else {
            return;
        };
        if claim.acknowledged || claim.dispatch_intent {
            return;
        }
        if claim.holds_slot {
            self.active = self.active.saturating_sub(1);
        }
        self.claims.remove(key);
    }

    /// Record dispatch intent. The effect has not started.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unavailable`] when the claim is not acknowledged.
    pub fn intend(&mut self, key: &str) -> Result<(), Error> {
        let claim = self.claims.get_mut(key).ok_or(Error::Unavailable)?;
        if !claim.acknowledged || claim.tombstone {
            return Err(Error::Unavailable);
        }
        if claim.cancel_requested && !claim.effect_started {
            return Err(Error::CannotEnforce {
                detail: "cancel_requested".into(),
            });
        }
        claim.dispatch_intent = true;
        claim.phase = "intended".into();
        claim.dispatched = None;
        Ok(())
    }

    /// Apply the NIP-RUN crash boundary and keep the reservation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unavailable`] when `key` is unknown.
    pub fn crash(&mut self, key: &str, boundary: run::Boundary) -> Result<&Claim, Error> {
        let claim = self.claims.get_mut(key).ok_or(Error::Unavailable)?;
        let durability = run::crash(boundary, &claim.reservation);
        claim.acknowledged = durability.acknowledged;
        claim.dispatch_intent = durability.dispatch_intent;
        claim.effect_started = durability.effect_started;
        claim.observed = durability.observed;
        claim.verification = durability.verification;
        claim.integration = durability.integration;
        match boundary {
            run::Boundary::DispatchIntent | run::Boundary::Effect => {
                claim.outcome = Some("unknown".into());
                claim.spend = None;
                claim.dispatched = if durability.effect_started {
                    Some(true)
                } else {
                    None
                };
                claim.phase = "unknown".into();
                claim.result = Some(result_payload(claim));
                claim.result_digest = claim.result.as_ref().map(digest_of);
            }
            run::Boundary::Observed => {
                claim.outcome = Some("completed".into());
                claim.phase = "terminal".into();
            }
            _ => {
                claim.outcome = None;
                claim.phase = "reserved".into();
            }
        }
        self.hold_unknown_slot(key);
        self.active = self
            .claims
            .values()
            .filter(|claim| claim.holds_slot)
            .count() as u32;
        Ok(self.claims.get(key).expect("the claim remains"))
    }

    /// Persist a terminal observation. Conflicts do not pick a winner.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Conflict`] when a different terminal result is
    /// already stored, and [`Error::Unavailable`] when the result cannot
    /// be advertised.
    pub fn observe(&mut self, key: &str, report: Report) -> Result<Value, Error> {
        let claim = self.claims.get_mut(key).ok_or(Error::Unavailable)?;
        if claim.tombstone {
            return Err(Error::ContentUnavailable);
        }
        let payload = report_payload(claim, &report);
        let digest = digest_of(&payload);
        if let Some(previous) = claim.result_digest.clone() {
            if previous != digest {
                if !claim.conflicts.contains(&previous) {
                    claim.conflicts.push(previous);
                }
                if !claim.conflicts.contains(&digest) {
                    claim.conflicts.push(digest);
                }
                claim.conflict = true;
                return Err(Error::Conflict);
            }
            return Ok(claim.result.clone().unwrap_or(payload));
        }
        claim.outcome = Some(report.outcome.to_string());
        claim.dispatched = Some(report.dispatched);
        claim.output = report.output.clone();
        claim.spend = report.spend;
        claim.code = report.code.clone();
        claim.message = report.message.clone();
        if report.dispatched {
            claim.effect_started = true;
            claim.dispatch_intent = true;
        }
        claim.verification = Some(report.verification.unwrap_or("not_run").to_string());
        claim.integration = Some(report.integration.unwrap_or("not_requested").to_string());
        claim.observed = report.outcome != "unknown";
        if claim.observed {
            run::advertise(&claim.durability())?;
        }
        claim.result = Some(payload.clone());
        claim.result_digest = Some(digest);
        claim.phase = if report.outcome == "unknown" {
            "unknown".into()
        } else {
            "terminal".into()
        };
        if claim.holds_slot && report.outcome != "unknown" {
            claim.holds_slot = false;
            self.active = self.active.saturating_sub(1);
        }
        Ok(payload)
    }

    /// Record a progress note. Progress does not complete the attempt.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] when `seq` is not the next note, and
    /// [`Error::Unavailable`] when `key` is unknown.
    pub fn progress(&mut self, key: &str, seq: u64, status: &str) -> Result<Value, Error> {
        if !matches!(status, "queued" | "running" | "reconciling") {
            return Err(Error::Malformed {
                detail: "status".into(),
            });
        }
        let claim = self.claims.get_mut(key).ok_or(Error::Unavailable)?;
        let next = claim.progress.last().map(|note| note.seq + 1).unwrap_or(0);
        if seq != next {
            return Err(Error::Malformed {
                detail: "seq".into(),
            });
        }
        claim.progress.push(Progress {
            seq,
            status: status.to_string(),
        });
        if claim.phase != "terminal" && claim.phase != "unknown" {
            claim.phase = "running".into();
        }
        Ok(json!({
            "v": SCHEMA,
            "requires": [],
            "type": "progress",
            "request": claim.request,
            "attempt": claim.attempt,
            "run": claim.run,
            "seq": seq,
            "status": status
        }))
    }

    /// Append a journal reference. A gap stays visible to replay.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unavailable`] when `key` is unknown.
    pub fn append_record(&mut self, key: &str, seq: u64, digest: &str) -> Result<(), Error> {
        let claim = self.claims.get_mut(key).ok_or(Error::Unavailable)?;
        if claim.records.iter().any(|record| record.seq == seq) {
            return Err(Error::Conflict);
        }
        claim.records.push(SeqRecord {
            seq,
            digest: digest.to_string(),
        });
        claim.records.sort_by_key(|record| record.seq);
        Ok(())
    }

    /// Answer a control. Controls do not create execution.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotAdmitted`] for an unauthorized principal, and
    /// [`Error::ContentUnavailable`] when the attempt has no retained record.
    pub fn control(&mut self, opened: &Opened) -> Result<Value, Error> {
        let Body::Control {
            request,
            attempt,
            run,
            target,
            control,
        } = &opened.body
        else {
            return Err(Error::Malformed {
                detail: "execute".into(),
            });
        };
        let before = self.claims.len();
        let stored_key = self
            .claims
            .iter()
            .find(|(_, claim)| {
                claim.request == *request
                    && claim.attempt == *attempt
                    && claim.run == *run
                    && claim.execute_event == *target
                    && (claim.principal == opened.principal
                        || self.controllers.contains(&opened.principal))
            })
            .map(|(key, _)| key.clone());
        let Some(key) = stored_key else {
            return Err(Error::NotAdmitted);
        };
        let claim = self.claims.get_mut(&key).expect("the claim was found");
        if claim.tombstone {
            return Err(Error::ContentUnavailable);
        }
        let mut release_slot = false;
        let payload = match control {
            Control::Status => json!({
                "v": SCHEMA,
                "requires": [],
                "type": "status_result",
                "request": claim.request,
                "attempt": claim.attempt,
                "run": claim.run,
                "state": claim.phase,
                "record": claim.record,
                "outcome": claim.outcome,
                "dispatched": dispatched_bool(claim)
            }),
            Control::Replay {
                after_seq,
                max_records,
            } => replay_payload(claim, *after_seq, *max_records),
            Control::Cancel { reason } => {
                claim.cancel_requested = true;
                claim.message = Some(reason.clone());
                if !claim.dispatch_intent && !claim.effect_started {
                    claim.outcome = Some("cancelled".into());
                    claim.dispatched = Some(false);
                    claim.observed = true;
                    claim.phase = "terminal".into();
                    claim.verification = Some("not_run".into());
                    claim.integration = Some("not_requested".into());
                    claim.result = Some(result_payload(claim));
                    claim.result_digest = claim.result.as_ref().map(digest_of);
                    if claim.holds_slot {
                        claim.holds_slot = false;
                        release_slot = true;
                    }
                }
                json!({
                    "v": SCHEMA,
                    "requires": [],
                    "type": "cancel_result",
                    "request": claim.request,
                    "attempt": claim.attempt,
                    "run": claim.run,
                    "acknowledged": true,
                    "confirmed": claim.cancel_confirmed,
                    "outcome": claim.outcome,
                    "dispatched": dispatched_bool(claim)
                })
            }
        };
        debug_assert_eq!(self.claims.len(), before);
        if release_slot {
            self.active = self.active.saturating_sub(1);
        }
        Ok(payload)
    }

    /// Replace expired claims with tombstones. Absence of a tombstone is
    /// not permission to run the effect again; `prepare` still refuses a
    /// deadline that has passed.
    pub fn sweep(&mut self, now: u64) {
        for claim in self.claims.values_mut() {
            if now >= claim.retain_until && !claim.tombstone {
                claim.tombstone = true;
                claim.output = None;
                claim.result = None;
                claim.phase = "tombstone".into();
                if claim.holds_slot {
                    claim.holds_slot = false;
                }
            }
        }
        self.active = self
            .claims
            .values()
            .filter(|claim| claim.holds_slot)
            .count() as u32;
    }

    /// Whether another worker may be selected for this attempt.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CannotEnforce`] while the outcome is unknown.
    pub fn failover_allowed(
        &self,
        principal: &str,
        request: &str,
        attempt: u32,
    ) -> Result<(), Error> {
        let Some(claim) = self.claims.get(&self.key(principal, request, attempt)) else {
            return Ok(());
        };
        if claim.outcome.as_deref() == Some("unknown")
            || (claim.dispatch_intent && claim.outcome.is_none())
        {
            return Err(Error::CannotEnforce {
                detail: "reconcile the unknown effect before selecting another worker".into(),
            });
        }
        Ok(())
    }

    fn require_previous(&self, principal: &str, request: &str, attempt: u32) -> Result<(), Error> {
        if attempt <= 1 {
            return Ok(());
        }
        let Some(previous) = self.claims.get(&self.key(principal, request, attempt - 1)) else {
            return Err(Error::CannotEnforce {
                detail: "the preceding attempt is not on this worker".into(),
            });
        };
        if previous.tombstone {
            return Err(Error::ContentUnavailable);
        }
        match previous.winner() {
            Some("unknown") | None => Err(Error::CannotEnforce {
                detail: "reconcile the preceding outcome before another attempt".into(),
            }),
            Some(_) => Ok(()),
        }
    }

    fn hold_unknown_slot(&mut self, key: &str) {
        if let Some(claim) = self.claims.get_mut(key) {
            claim.holds_slot = true;
        }
    }
}

/// A terminal observation supplied by the host.
#[derive(Debug, Clone)]
pub struct Report {
    /// `completed`, `refused`, `failed`, `cancelled`, or `unknown`.
    pub outcome: &'static str,
    /// Whether the effect is known to have started.
    pub dispatched: bool,
    /// Typed output, or none.
    pub output: Option<Value>,
    /// Artifact references.
    pub artifacts: Vec<Value>,
    /// Receipt references, separate from decision receipts.
    pub receipts: Vec<Value>,
    /// Known spend. `None` stays null on the result.
    pub spend: Option<u64>,
    /// Verification word. Completion does not imply `passed`.
    pub verification: Option<&'static str>,
    /// Integration word.
    pub integration: Option<&'static str>,
    /// Refusal code.
    pub code: Option<String>,
    /// Refusal message.
    pub message: Option<String>,
}

/// Whether a pinned artifact's exact bytes are in `bytes`.
///
/// # Errors
///
/// Returns [`Error::ContentUnavailable`] when a pin is missing, and
/// [`Error::IdentityMismatch`] when the bytes differ.
pub fn require_pins(execute: &Execute, bytes: &BTreeMap<String, Vec<u8>>) -> Result<(), Error> {
    for artifact in [
        &execute.target.artifact,
        &execute.lock,
        &execute.context,
        &execute.requirements,
    ] {
        require_bytes(artifact, bytes)?;
    }
    if let Some(artifact) = &execute.input_artifact {
        require_bytes(artifact, bytes)?;
    }
    Ok(())
}

/// Reserve spend without treating an unknown figure as zero.
///
/// # Errors
///
/// Returns [`Error::CannotEnforce`] when the request states a spend ceiling
/// and either the quote or the remaining budget is unknown. Returns
/// [`Error::LimitExceeded`] when the quote exceeds the remaining budget.
pub fn reserve_spend(
    bounds: &Bounds,
    quoted: Option<u64>,
    remaining: Option<u64>,
) -> Result<Option<u64>, Error> {
    let Some(ceiling) = bounds.spend_microunits else {
        return Ok(None);
    };
    let Some(quoted) = quoted else {
        return Err(Error::CannotEnforce {
            detail: "spend quote".into(),
        });
    };
    let Some(remaining) = remaining else {
        return Err(Error::CannotEnforce {
            detail: "remaining budget".into(),
        });
    };
    if quoted > ceiling || quoted > remaining {
        return Err(Error::LimitExceeded);
    }
    Ok(Some(quoted))
}

/// The cause a caller records when an execution worker does not answer.
#[must_use]
pub const fn contact_cause(heard: bool) -> &'static str {
    if heard {
        "worker_silent"
    } else {
        "worker_absent"
    }
}

/// Open a kind-`25920` event addressed to `worker`.
///
/// Deadline expiry is not decided here. A retransmission of a known
/// fingerprint remains valid after the deadline, so the ledger decides.
///
/// # Errors
///
/// Returns [`Error`] when the event is the wrong family, the signature
/// fails, or the body is not execution v1.
pub fn open_request(
    event: &Event,
    worker: &str,
    secret: &SecretKey,
    now: u64,
    window: Window,
) -> Result<Opened, Error> {
    open_request_profile(event, worker, secret, now, window, false)
}

/// Decode an execute request requiring the NIP-LAB binding feature.
///
/// This decoder grants no execution authority. Its caller must authenticate and
/// durably admit the exact order and signed LAB linkage before dispatch. Generic
/// workers keep using [`open_request`], which refuses this required feature.
pub fn open_labor_request(
    event: &Event,
    worker: &str,
    secret: &SecretKey,
    now: u64,
    window: Window,
) -> Result<Opened, Error> {
    let opened = open_request_profile(event, worker, secret, now, window, true)?;
    if !matches!(opened.body, Body::Execute(_)) {
        return Err(malformed("labor intake requires an execute event"));
    }
    Ok(opened)
}

fn open_request_profile(
    event: &Event,
    worker: &str,
    secret: &SecretKey,
    now: u64,
    window: Window,
    labor: bool,
) -> Result<Opened, Error> {
    if event.kind != REQUEST_KIND {
        return Err(Error::UnexpectedKind { kind: event.kind });
    }
    event.validate_structure()?;
    event.validate_crypto()?;
    if secret.x_only_public_key(&Secp256k1::new()).0.to_string() != worker {
        return Err(Error::NotAddressed);
    }
    let recipients: Vec<&str> = event.tag_values("p").collect();
    if recipients != [worker] {
        return Err(Error::NotAddressed);
    }
    if !window.admits(event.created_at, now) {
        return Err(Error::Stale);
    }
    let payload = decrypt_object(event, secret)?;
    check_schema(&payload)?;
    check_credentials(&payload)?;
    let kind = payload.get("type").and_then(Value::as_str).unwrap_or("");
    let body = match kind {
        "execute" => Body::Execute(Box::new(parse_execute(event, &payload, labor)?)),
        "status" | "replay" | "cancel" => parse_control(event, &payload, kind)?,
        _ => {
            return Err(Error::Malformed {
                detail: "type".into(),
            });
        }
    };
    Ok(Opened {
        principal: event.pubkey.clone(),
        event_id: event.id.clone(),
        created_at: event.created_at,
        body,
    })
}

/// Bind a worker event to the execute event the caller is waiting on.
///
/// # Errors
///
/// Returns [`Error::Unbound`] when the signer, recipient, or `e` tag is a
/// different job, and [`Error::UnexpectedKind`] for another family's kind.
pub fn bind_worker_event(
    event: &Event,
    pending: &Pending<'_>,
    secret: &SecretKey,
) -> Result<Value, Error> {
    if event.kind != RESULT_KIND && event.kind != FEEDBACK_KIND {
        return Err(Error::UnexpectedKind { kind: event.kind });
    }
    if event.pubkey != pending.worker {
        return Err(Error::Unbound { field: "pubkey" });
    }
    event.validate_structure()?;
    event.validate_crypto()?;
    let targets: Vec<&str> = event.tag_values("e").collect();
    let recipients: Vec<&str> = event.tag_values("p").collect();
    if targets != [pending.execute_event] {
        return Err(Error::Unbound { field: "e" });
    }
    if recipients != [pending.customer] {
        return Err(Error::Unbound { field: "p" });
    }
    if secret.x_only_public_key(&Secp256k1::new()).0.to_string() != pending.customer {
        return Err(Error::Unbound { field: "customer" });
    }
    let payload = decrypt_object(event, secret)?;
    check_schema(&payload)?;
    let request = payload.get("request").and_then(Value::as_str);
    let attempt = payload.get("attempt").and_then(Value::as_u64);
    if request != Some(pending.request) || attempt != Some(u64::from(pending.attempt)) {
        return Err(Error::Unbound { field: "request" });
    }
    Ok(payload)
}

/// The job a caller is waiting on.
#[derive(Debug, Clone, Copy)]
pub struct Pending<'a> {
    /// Execute event id.
    pub execute_event: &'a str,
    /// Worker pubkey.
    pub worker: &'a str,
    /// Caller pubkey.
    pub customer: &'a str,
    /// Logical request id.
    pub request: &'a str,
    /// Attempt number.
    pub attempt: u32,
}

/// Caller-side observation. A missing `accepted` is not proof of idleness.
#[derive(Debug, Clone)]
pub struct Watch {
    /// Last contiguous progress sequence that was rendered.
    pub last_seq: Option<u64>,
    /// Whether incremental progress is still contiguous.
    pub rendering: bool,
    /// Accepted payload, when one arrived.
    pub accepted: Option<Value>,
    /// Terminal payloads, in arrival order.
    pub results: Vec<Value>,
    /// True when two terminal outcomes disagree.
    pub conflict: bool,
    /// A relay `OK` was observed.
    pub relay_ok: bool,
    /// The socket closed.
    pub socket_closed: bool,
    /// Seen event ids.
    seen: BTreeSet<String>,
}

impl Watch {
    /// A watch with no worker traffic.
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_seq: None,
            rendering: true,
            accepted: None,
            results: Vec::new(),
            conflict: false,
            relay_ok: false,
            socket_closed: false,
            seen: BTreeSet::new(),
        }
    }

    /// Record a relay `OK`. The run is still unaccepted.
    pub fn note_relay_ok(&mut self) {
        self.relay_ok = true;
    }

    /// Record a socket close. Remote work is unchanged.
    pub fn note_socket_closed(&mut self) {
        self.socket_closed = true;
    }

    /// Whether missing feedback proves the worker did nothing.
    #[must_use]
    pub const fn missing_feedback_proves_idle(&self) -> bool {
        false
    }

    /// Ingest one verified worker payload.
    pub fn ingest(&mut self, event_id: &str, payload: &Value) {
        if !self.seen.insert(event_id.to_string()) {
            return;
        }
        match payload.get("type").and_then(Value::as_str) {
            Some("accepted") => self.accepted = Some(payload.clone()),
            Some("progress") => self.note_progress(payload),
            Some("result") => self.note_result(payload),
            _ => {}
        }
    }

    fn note_progress(&mut self, payload: &Value) {
        let Some(seq) = payload.get("seq").and_then(Value::as_u64) else {
            self.rendering = false;
            return;
        };
        let expected = self.last_seq.map(|seq| seq + 1).unwrap_or(0);
        if seq == expected {
            self.last_seq = Some(seq);
        } else {
            self.rendering = false;
        }
    }

    fn note_result(&mut self, payload: &Value) {
        let digest = digest_of(payload);
        if self
            .results
            .iter()
            .any(|existing| digest_of(existing) == digest)
        {
            return;
        }
        if !self.results.is_empty() {
            self.conflict = true;
        }
        self.results.push(payload.clone());
    }
}

impl Default for Watch {
    fn default() -> Self {
        Self::new()
    }
}

/// What signing one encrypted event takes.
#[derive(Clone, Copy)]
pub struct Seal<'a> {
    /// The signer.
    pub signer: &'a RelaySigner,
    /// NIP-44 conversation key.
    pub conversation: [u8; 32],
    /// Fresh nonce.
    pub nonce: [u8; 32],
    /// `created_at`, Unix seconds.
    pub created_at: u64,
}

impl Seal<'_> {
    /// Encrypt `payload` and sign it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] when the payload exceeds the byte cap.
    pub fn event(&self, kind: u16, tags: Vec<Tag>, payload: &Value) -> Result<Event, Error> {
        let plaintext = payload.to_string();
        if plaintext.len() > MAX_PAYLOAD_BYTES {
            return Err(Error::Malformed {
                detail: "payload".into(),
            });
        }
        let content = nip44::encrypt(&plaintext, &self.conversation, self.nonce).map_err(|_| {
            Error::Malformed {
                detail: "encrypt".into(),
            }
        })?;
        Ok(self.signer.sign(self.created_at, kind, tags, content))
    }
}

/// A refusal result that names the logical attempt and dispatches nothing.
#[must_use]
pub fn refusal_result(request: &str, attempt: u32, run: &str, code: &str, message: &str) -> Value {
    json!({
        "v": SCHEMA,
        "requires": [],
        "type": "result",
        "request": request,
        "attempt": attempt,
        "run": run,
        "outcome": "refused",
        "dispatched": false,
        "output": null,
        "artifacts": [],
        "receipts": [],
        "verification": "not_run",
        "integration": "not_requested",
        "record": null,
        "spend": null,
        "code": code,
        "message": message
    })
}

fn accepted_payload(claim: &Claim) -> Value {
    json!({
        "v": SCHEMA,
        "requires": [],
        "type": "accepted",
        "request": claim.request,
        "attempt": claim.attempt,
        "run": claim.run,
        "input_digest": claim.input_digest,
        "lock_digest": claim.lock_digest,
        "record": claim.record,
        "mailbox": claim.mailbox,
        "retain_until": claim.retain_until,
        "controller": claim.controller
    })
}

fn result_payload(claim: &Claim) -> Value {
    json!({
        "v": SCHEMA,
        "requires": [],
        "type": "result",
        "request": claim.request,
        "attempt": claim.attempt,
        "run": claim.run,
        "outcome": claim.outcome,
        "dispatched": dispatched_bool(claim),
        "output": claim.output,
        "artifacts": [],
        "receipts": [],
        "verification": claim.verification.clone().unwrap_or_else(|| "not_run".into()),
        "integration": claim.integration.clone().unwrap_or_else(|| "not_requested".into()),
        "record": claim.record,
        "spend": claim.spend
    })
}

fn report_payload(claim: &Claim, report: &Report) -> Value {
    json!({
        "v": SCHEMA,
        "requires": [],
        "type": "result",
        "request": claim.request,
        "attempt": claim.attempt,
        "run": claim.run,
        "outcome": report.outcome,
        "dispatched": report.dispatched,
        "output": report.output,
        "artifacts": report.artifacts,
        "receipts": report.receipts,
        "verification": report.verification.unwrap_or("not_run"),
        "integration": report.integration.unwrap_or("not_requested"),
        "record": claim.record,
        "spend": report.spend,
        "code": report.code,
        "message": report.message
    })
}

fn replay_payload(claim: &Claim, after_seq: Option<u64>, max_records: u32) -> Value {
    let mut ordered: Vec<&SeqRecord> = claim
        .records
        .iter()
        .filter(|record| after_seq.is_none_or(|bound| record.seq > bound))
        .collect();
    let mut gap = false;
    let mut previous = after_seq;
    for record in &ordered {
        if let Some(prev) = previous {
            if record.seq != prev + 1 {
                gap = true;
            }
        } else if record.seq != 0 {
            gap = true;
        }
        previous = Some(record.seq);
    }
    let truncated = ordered.len() > max_records as usize;
    if truncated {
        ordered.truncate(max_records as usize);
    }
    let next_seq = ordered
        .last()
        .map(|record| record.seq + 1)
        .unwrap_or(after_seq.unwrap_or(0));
    json!({
        "v": SCHEMA,
        "requires": [],
        "type": "replay_result",
        "request": claim.request,
        "attempt": claim.attempt,
        "run": claim.run,
        "records": ordered.iter().map(|record| json!({"seq": record.seq, "digest": record.digest})).collect::<Vec<_>>(),
        "next_seq": next_seq,
        "complete": !gap && !truncated,
        "gap": gap,
        "code": if gap { Value::String("content_unavailable".into()) } else { Value::Null }
    })
}

fn dispatched_bool(claim: &Claim) -> bool {
    claim.dispatched.unwrap_or(true)
}

fn digest_of(value: &Value) -> String {
    digest_value(value).unwrap_or_else(|_| "sha256:00".into())
}

fn root_envelope(execute: &Execute, controller: &str, now: u64) -> Result<Value, Error> {
    let parent = execute.parent.as_ref().map(|parent| {
        json!({
            "run": parent.run,
            "step": parent.step,
            "iteration": parent.iteration,
            "attempt": parent.attempt
        })
    });
    let record = json!({
        "run": execute.run,
        "seq": 0,
        "previous": null,
        "controller": controller,
        "generation": 0,
        "type": "created",
        "subject": {"step": null, "iteration": null, "attempt": null},
        "time": now,
        "data": {
            "owner": controller,
            "request": execute.request,
            "base": execute.lock.digest,
            "program": execute.target.artifact.digest,
            "lock": execute.lock.digest,
            "context": execute.context.digest,
            "policy": execute.requirements.digest,
            "parent": parent,
            "recipients": [controller]
        }
    });
    let digest = run::logical_digest(&record)?;
    let envelope = json!({
        "v": run::RECORD_SCHEMA,
        "requires": [],
        "record": record,
        "digest": digest
    });
    run::parse_envelope(&envelope)?;
    Ok(envelope)
}

fn parse_execute(event: &Event, payload: &Value, labor: bool) -> Result<Execute, Error> {
    let map = object(payload, "execute")?;
    require_keys(
        map,
        &[
            "v",
            "requires",
            "type",
            "request",
            "attempt",
            "run",
            "target",
            "lock",
            "input",
            "context",
            "requirements",
            "bounds",
            "deadline",
            "retain_until",
        ],
    )?;
    allow_keys(
        map,
        &[
            "v",
            "requires",
            "type",
            "request",
            "attempt",
            "run",
            "target",
            "lock",
            "input",
            "context",
            "requirements",
            "bounds",
            "deadline",
            "retain_until",
            "parent",
            "meta",
        ],
    )?;
    let requires = map.get("requires").ok_or_else(|| malformed("requires"))?;
    if labor {
        if requires != &json!(["openagents.labor-binding.v1"]) {
            return Err(Error::UnsupportedFeature {
                detail: "labor requires".into(),
            });
        }
    } else {
        parse_requires(requires)?;
    }
    let deadline = map
        .get("deadline")
        .and_then(Value::as_u64)
        .ok_or_else(|| malformed("deadline"))?;
    match event.expiration() {
        Some(expiration) if expiration == deadline => {}
        _ => {
            return Err(malformed("expiration"));
        }
    }
    let retain_until = map
        .get("retain_until")
        .and_then(Value::as_u64)
        .ok_or_else(|| malformed("retain_until"))?;
    if retain_until <= deadline {
        return Err(malformed("retain_until"));
    }
    let input = map
        .get("input")
        .cloned()
        .ok_or_else(|| malformed("input"))?;
    if serde_json::to_vec(&input)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
        > 128 * 1024
    {
        return Err(Error::LimitExceeded);
    }
    let input_artifact = input_artifact(&input)?;
    Ok(Execute {
        fingerprint: digest_value(payload)?,
        payload: payload.clone(),
        request: ident(map, "request")?,
        attempt: attempt_of(map)?,
        run: ident(map, "run")?,
        target: parse_definition(map.get("target").ok_or_else(|| malformed("target"))?)?,
        lock: parse_artifact(map.get("lock").ok_or_else(|| malformed("lock"))?)?,
        input_digest: digest_value(&input)?,
        input,
        input_artifact,
        context: parse_artifact(map.get("context").ok_or_else(|| malformed("context"))?)?,
        requirements: parse_artifact(
            map.get("requirements")
                .ok_or_else(|| malformed("requirements"))?,
        )?,
        bounds: parse_bounds(map.get("bounds").ok_or_else(|| malformed("bounds"))?)?,
        deadline,
        retain_until,
        parent: match map.get("parent") {
            None => None,
            Some(value) => Some(parse_parent(value)?),
        },
    })
}

fn parse_control(event: &Event, payload: &Value, kind: &str) -> Result<Body, Error> {
    let map = object(payload, "control")?;
    allow_keys(
        map,
        &[
            "v",
            "requires",
            "type",
            "request",
            "attempt",
            "run",
            "after_seq",
            "max_records",
            "reason",
            "meta",
        ],
    )?;
    parse_requires(map.get("requires").ok_or_else(|| malformed("requires"))?)?;
    let targets: Vec<&str> = event.tag_values("e").collect();
    let [target] = targets.as_slice() else {
        return Err(malformed("e"));
    };
    if target.len() != 64
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(malformed("e"));
    }
    let request = ident(map, "request")?;
    let attempt = attempt_of(map)?;
    let run = ident(map, "run")?;
    let control = match kind {
        "status" => {
            if map.contains_key("after_seq") || map.contains_key("reason") {
                return Err(Error::UnsupportedFeature {
                    detail: "status".into(),
                });
            }
            Control::Status
        }
        "replay" => Control::Replay {
            after_seq: match map.get("after_seq") {
                Some(Value::Null) => None,
                Some(value) => Some(value.as_u64().ok_or_else(|| malformed("after_seq"))?),
                None => {
                    return Err(malformed("after_seq"));
                }
            },
            max_records: map
                .get("max_records")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| (1..=MAX_REPLAY).contains(value))
                .ok_or_else(|| malformed("max_records"))?,
        },
        "cancel" => {
            let reason = map
                .get("reason")
                .and_then(Value::as_str)
                .ok_or_else(|| malformed("reason"))?;
            if reason.is_empty() || reason.len() > MAX_REASON_BYTES {
                return Err(malformed("reason"));
            }
            Control::Cancel {
                reason: reason.to_string(),
            }
        }
        _ => {
            return Err(malformed("type"));
        }
    };
    Ok(Body::Control {
        request,
        attempt,
        run,
        target: (*target).to_string(),
        control,
    })
}

fn input_artifact(input: &Value) -> Result<Option<ArtifactRef>, Error> {
    let Some(map) = input.as_object() else {
        return Ok(None);
    };
    if map.len() == 1 && map.contains_key("artifact") {
        return Ok(Some(parse_artifact(
            map.get("artifact").ok_or_else(|| malformed("artifact"))?,
        )?));
    }
    Ok(None)
}

fn parse_bounds(value: &Value) -> Result<Bounds, Error> {
    let map = object(value, "bounds")?;
    allow_keys(
        map,
        &["wall_ms", "output_bytes", "jobs", "spend_microunits"],
    )?;
    Ok(Bounds {
        wall_ms: optional_u64(map, "wall_ms")?,
        output_bytes: optional_u64(map, "output_bytes")?,
        jobs: optional_u64(map, "jobs")?,
        spend_microunits: optional_u64(map, "spend_microunits")?,
    })
}

fn parse_parent(value: &Value) -> Result<Parent, Error> {
    let map = object(value, "parent")?;
    allow_keys(map, &["run", "step", "iteration", "attempt"])?;
    require_keys(map, &["run", "step", "iteration", "attempt"])?;
    Ok(Parent {
        run: ident(map, "run")?,
        step: ident(map, "step")?,
        iteration: map
            .get("iteration")
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed("iteration"))?,
        attempt: attempt_of(map)?,
    })
}

fn parse_requires(value: &Value) -> Result<(), Error> {
    let Some(items) = value.as_array() else {
        return Err(malformed("requires"));
    };
    if items.is_empty() {
        Ok(())
    } else {
        Err(Error::UnsupportedFeature {
            detail: "requires".into(),
        })
    }
}

fn check_schema(payload: &Value) -> Result<(), Error> {
    match payload.get("v") {
        Some(Value::String(version)) if version == SCHEMA => Ok(()),
        _ => Err(Error::UnsupportedVersion),
    }
}

fn check_credentials(payload: &Value) -> Result<(), Error> {
    let Some(map) = payload.as_object() else {
        return Err(malformed("payload"));
    };
    for field in map.keys() {
        if CREDENTIALS.contains(&field.to_ascii_lowercase().as_str()) {
            return Err(malformed(field));
        }
    }
    Ok(())
}

fn decrypt_object(event: &Event, secret: &SecretKey) -> Result<Value, Error> {
    let peer: XOnlyPublicKey = event
        .pubkey
        .parse()
        .map_err(|_| Error::Event(DomainError::InvalidPublicKey))?;
    let conversation = nip44::conversation_key(secret, &peer);
    let plaintext =
        nip44::decrypt(&event.content, &conversation).map_err(|_| malformed("content"))?;
    if plaintext.len() > MAX_PAYLOAD_BYTES {
        return Err(Error::LimitExceeded);
    }
    let payload: Value = serde_json::from_str(&plaintext).map_err(|_| malformed("json"))?;
    if !payload.is_object() {
        return Err(malformed("payload"));
    }
    Ok(payload)
}

fn object<'a>(value: &'a Value, detail: &str) -> Result<&'a Map<String, Value>, Error> {
    value.as_object().ok_or_else(|| malformed(detail))
}

fn require_keys(map: &Map<String, Value>, keys: &[&str]) -> Result<(), Error> {
    for key in keys {
        if !map.contains_key(*key) {
            return Err(malformed(*key));
        }
    }
    Ok(())
}

fn allow_keys(map: &Map<String, Value>, keys: &[&str]) -> Result<(), Error> {
    for key in map.keys() {
        if !keys.contains(&key.as_str()) {
            return Err(Error::UnsupportedFeature {
                detail: key.clone(),
            });
        }
    }
    Ok(())
}

fn ident(map: &Map<String, Value>, field: &str) -> Result<String, Error> {
    let text = map
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(field))?;
    if text.is_empty()
        || text.len() > MAX_ID_BYTES
        || !text.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err(malformed(field));
    }
    Ok(text.to_string())
}

fn attempt_of(map: &Map<String, Value>) -> Result<u32, Error> {
    let value = map
        .get("attempt")
        .and_then(Value::as_u64)
        .ok_or_else(|| malformed("attempt"))?;
    u32::try_from(value)
        .ok()
        .filter(|attempt| *attempt >= 1)
        .ok_or_else(|| malformed("attempt"))
}

fn optional_u64(map: &Map<String, Value>, field: &str) -> Result<Option<u64>, Error> {
    match map.get(field) {
        None => Ok(None),
        Some(Value::Number(number)) => number.as_u64().map(Some).ok_or_else(|| malformed(field)),
        Some(_) => Err(malformed(field)),
    }
}

fn require_bytes(artifact: &ArtifactRef, bytes: &BTreeMap<String, Vec<u8>>) -> Result<(), Error> {
    let Some(found) = bytes.get(&artifact.digest) else {
        return Err(Error::ContentUnavailable);
    };
    if found.len() as u64 != artifact.size || digest_bytes(found) != artifact.digest {
        return Err(Error::IdentityMismatch);
    }
    Ok(())
}

fn malformed(detail: impl Into<String>) -> Error {
    Error::Malformed {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision;
    use secp256k1::SecretKey;

    const NOW: u64 = 1_700_000_000;
    const DEADLINE: u64 = 1_700_003_600;
    const RETAIN: u64 = 1_700_010_000;

    struct Keys {
        signer: RelaySigner,
        secret: SecretKey,
        pubkey: String,
    }

    fn keys(byte: u8) -> Keys {
        let raw = [byte; 32];
        let secret = SecretKey::from_byte_array(raw).unwrap();
        let hex: String = raw.iter().map(|item| format!("{item:02x}")).collect();
        let signer = RelaySigner::from_secret_hex(&hex).unwrap();
        let pubkey = signer.pubkey().to_string();
        Keys {
            signer,
            secret,
            pubkey,
        }
    }

    fn artifact(bytes: &[u8]) -> Value {
        json!({
            "digest": digest_bytes(bytes),
            "size": bytes.len() as u64,
            "media_type": "application/json"
        })
    }

    fn body() -> Value {
        let publisher = "a".repeat(64);
        json!({
            "v": SCHEMA,
            "requires": [],
            "type": "execute",
            "request": "req-1",
            "attempt": 1,
            "run": "run-1",
            "target": {
                "id": format!("{publisher}:pkg/op"),
                "artifact": artifact(b"target")
            },
            "lock": artifact(b"lock"),
            "input": {"task": "count"},
            "context": artifact(b"context"),
            "requirements": artifact(b"requirements"),
            "bounds": {"output_bytes": 1024},
            "deadline": DEADLINE,
            "retain_until": RETAIN
        })
    }

    #[test]
    fn labor_requires_explicit_decoder_and_keeps_signed_fingerprint() {
        let caller = keys(1);
        let worker = keys(2);
        let mut payload = body();
        payload["requires"] = json!(["openagents.labor-binding.v1"]);
        let event = request(&caller, &worker, &payload, 17);
        assert!(
            open_request(&event, &worker.pubkey, &worker.secret, NOW, Window::DEFAULT).is_err()
        );
        let opened =
            open_labor_request(&event, &worker.pubkey, &worker.secret, NOW, Window::DEFAULT)
                .unwrap();
        let Body::Execute(execute) = opened.body else {
            panic!("expected execute")
        };
        assert_eq!(execute.fingerprint, digest_value(&payload).unwrap());
        assert_eq!(execute.payload, payload);
        for required in [json!([]), json!(["openagents.labor-binding.v1", "unknown"])] {
            payload["requires"] = required;
            let event = request(&caller, &worker, &payload, 18);
            assert!(
                open_labor_request(&event, &worker.pubkey, &worker.secret, NOW, Window::DEFAULT)
                    .is_err()
            );
        }
    }

    fn pins_of(payload: &Value) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        for bytes in [b"target".as_slice(), b"lock", b"context", b"requirements"] {
            out.insert(digest_bytes(bytes), bytes.to_vec());
        }
        let _ = payload;
        out
    }

    fn seal_between<'a>(from: &'a Keys, to: &Keys, nonce: u8) -> Seal<'a> {
        let peer = to.pubkey.parse().unwrap();
        Seal {
            signer: &from.signer,
            conversation: nip44::conversation_key(&from.secret, &peer),
            nonce: [nonce; 32],
            created_at: NOW,
        }
    }

    fn request(from: &Keys, to: &Keys, payload: &Value, nonce: u8) -> Event {
        seal_between(from, to, nonce)
            .event(
                REQUEST_KIND,
                vec![
                    Tag::new(vec!["p".into(), to.pubkey.clone()]),
                    Tag::new(vec!["expiration".into(), DEADLINE.to_string()]),
                ],
                payload,
            )
            .unwrap()
    }

    fn opened(from: &Keys, to: &Keys, payload: &Value) -> Opened {
        open_request(
            &request(from, to, payload, 7),
            &to.pubkey,
            &to.secret,
            NOW,
            Window::DEFAULT,
        )
        .unwrap()
    }

    fn reserved(service: &mut Service, from: &Keys, to: &Keys) -> (Claim, Vec<u8>) {
        let opened = opened(from, to, &body());
        let Admission::Reserved { claim, root } =
            service.prepare(&opened, NOW, &"ab".repeat(32)).unwrap()
        else {
            panic!("a first execute reserves");
        };
        (claim, root)
    }

    #[test]
    fn an_accepted_execute_keeps_the_requested_retention_and_root() {
        let caller = keys(1);
        let worker = keys(2);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        let (claim, root) = reserved(&mut service, &caller, &worker);
        assert!(!claim.acknowledged);
        assert_eq!(service.active, 1);
        let accepted = service.acknowledge(&claim.key, &root).unwrap();
        assert_eq!(accepted["type"], "accepted");
        assert_eq!(accepted["retain_until"], RETAIN);
        assert_eq!(accepted["controller"], worker.pubkey);
        assert_eq!(accepted["mailbox"], "ab".repeat(32));
        assert_eq!(accepted["record"]["digest"], digest_bytes(&root));
        run::parse_envelope(&serde_json::from_slice::<Value>(&root).unwrap()).unwrap();
        assert!(!service.promises_exactly_once());
        assert!(!service.relay_ok_is_acceptance());
        assert!(!service.socket_close_is_cancel());
        assert!(!service.expiration_stops_subprocess());
    }

    #[test]
    fn retransmission_does_not_reserve_again_after_the_deadline() {
        let caller = keys(1);
        let worker = keys(2);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        let (claim, root) = reserved(&mut service, &caller, &worker);
        service.acknowledge(&claim.key, &root).unwrap();
        let mut again = body();
        again["deadline"] = json!(DEADLINE);
        let event = request(&caller, &worker, &again, 8);
        let opened = open_request(
            &event,
            &worker.pubkey,
            &worker.secret,
            DEADLINE + 10,
            Window {
                max_age_seconds: u64::MAX / 4,
                max_future_seconds: 60,
            },
        )
        .unwrap();
        let Admission::Retransmission(found) = service
            .prepare(&opened, DEADLINE + 10, &"cd".repeat(32))
            .unwrap()
        else {
            panic!("the same fingerprint is a retransmission");
        };
        assert_eq!(found.fingerprint, claim.fingerprint);
        assert_eq!(service.active, 1);
        assert_eq!(found.aliases.len(), 2);
    }

    #[test]
    fn changed_bytes_under_the_same_key_conflict() {
        let caller = keys(1);
        let worker = keys(2);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        let _ = reserved(&mut service, &caller, &worker);
        let mut changed = body();
        changed["input"] = json!({"task": "other"});
        let opened = opened(&caller, &worker, &changed);
        let error = service.prepare(&opened, NOW, &"ab".repeat(32)).unwrap_err();
        assert_eq!(error.code(), Some("idempotency_conflict"));
        assert_eq!(service.active, 1);
    }

    #[test]
    fn a_second_worker_does_not_deduplicate_the_first() {
        let caller = keys(1);
        let first = keys(2);
        let second = keys(3);
        let mut one = Service::new(first.pubkey.clone(), 2, RETAIN);
        let mut two = Service::new(second.pubkey.clone(), 2, RETAIN);
        let _ = reserved(&mut one, &caller, &first);
        let _ = reserved(&mut two, &caller, &second);
        assert_eq!(one.active, 1);
        assert_eq!(two.active, 1);
        assert_ne!(one.claims.keys().next(), two.claims.keys().next());
    }

    #[test]
    fn an_unknown_effect_blocks_another_attempt_and_failover() {
        let caller = keys(1);
        let worker = keys(2);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        let (claim, root) = reserved(&mut service, &caller, &worker);
        service.acknowledge(&claim.key, &root).unwrap();
        service.intend(&claim.key).unwrap();
        let crashed = service.crash(&claim.key, run::Boundary::Effect).unwrap();
        assert_eq!(crashed.outcome.as_deref(), Some("unknown"));
        assert!(crashed.spend.is_none());
        assert!(crashed.result.as_ref().unwrap()["spend"].is_null());
        assert!(run::advertise(&crashed.durability()).is_err());
        assert_eq!(service.active, 1);
        assert!(
            service
                .failover_allowed(&caller.pubkey, "req-1", 1)
                .is_err()
        );
        let mut next = body();
        next["attempt"] = json!(2);
        let opened = opened(&caller, &worker, &next);
        assert_eq!(
            service
                .prepare(&opened, NOW, &"ef".repeat(32))
                .unwrap_err()
                .code(),
            Some("cannot_enforce")
        );
    }

    #[test]
    fn a_busy_worker_refuses_without_taking_the_slot() {
        let caller = keys(1);
        let worker = keys(2);
        let mut service = Service::new(worker.pubkey.clone(), 1, RETAIN);
        let _ = reserved(&mut service, &caller, &worker);
        let mut other = body();
        other["request"] = json!("req-2");
        other["run"] = json!("run-2");
        let opened = opened(&caller, &worker, &other);
        assert_eq!(
            service
                .prepare(&opened, NOW, &"cd".repeat(32))
                .unwrap_err()
                .code(),
            Some("busy")
        );
        assert_eq!(service.active, 1);
    }

    #[test]
    fn families_are_not_reinterpreted() {
        let caller = keys(1);
        let worker = keys(2);
        let event = seal_between(&caller, &worker, 1)
            .event(
                25_900,
                vec![Tag::new(vec!["p".into(), worker.pubkey.clone()])],
                &json!({"v": 1, "type": "task", "task": "hi"}),
            )
            .unwrap();
        assert!(matches!(
            open_request(&event, &worker.pubkey, &worker.secret, NOW, Window::DEFAULT),
            Err(Error::UnexpectedKind { kind: 25_900 })
        ));
        let foreign = request(
            &caller,
            &worker,
            &json!({
                "v": decision::SCHEMA,
                "requires": [],
                "type": "systemone",
                "request": "req-1",
                "attempt": 1
            }),
            2,
        );
        assert_eq!(
            open_request(
                &foreign,
                &worker.pubkey,
                &worker.secret,
                NOW,
                Window::DEFAULT
            )
            .unwrap_err()
            .code(),
            Some("unsupported_version")
        );
        let execution = request(&caller, &worker, &body(), 3);
        assert!(matches!(
            decision::admit(
                &execution,
                &worker.pubkey,
                &worker.secret,
                NOW,
                decision::RequestWindow::DEFAULT
            ),
            Err(decision::DecisionError::UnexpectedKind { .. })
        ));
    }

    #[test]
    fn a_wrong_signer_does_not_bind() {
        let caller = keys(1);
        let worker = keys(2);
        let other = keys(3);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        let (claim, root) = reserved(&mut service, &caller, &worker);
        let accepted = service.acknowledge(&claim.key, &root).unwrap();
        let event = seal_between(&other, &caller, 9)
            .event(
                FEEDBACK_KIND,
                vec![
                    Tag::new(vec!["e".into(), claim.execute_event.clone()]),
                    Tag::new(vec!["p".into(), caller.pubkey.clone()]),
                ],
                &accepted,
            )
            .unwrap();
        let pending = Pending {
            execute_event: &claim.execute_event,
            worker: &worker.pubkey,
            customer: &caller.pubkey,
            request: "req-1",
            attempt: 1,
        };
        assert!(matches!(
            bind_worker_event(&event, &pending, &caller.secret),
            Err(Error::Unbound { field: "pubkey" })
        ));
    }

    #[test]
    fn controls_require_the_caller_and_do_not_confirm_a_stop() {
        let caller = keys(1);
        let worker = keys(2);
        let stranger = keys(3);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        let (claim, root) = reserved(&mut service, &caller, &worker);
        service.acknowledge(&claim.key, &root).unwrap();
        let cancel = control_event(
            &stranger,
            &worker,
            &claim,
            "cancel",
            json!({"reason": "stop"}),
        );
        let opened = open_request(
            &cancel,
            &worker.pubkey,
            &worker.secret,
            NOW,
            Window::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            service.control(&opened).unwrap_err().code(),
            Some("not_admitted")
        );
        let own = control_event(
            &caller,
            &worker,
            &claim,
            "cancel",
            json!({"reason": "stop"}),
        );
        let opened =
            open_request(&own, &worker.pubkey, &worker.secret, NOW, Window::DEFAULT).unwrap();
        let answer = service.control(&opened).unwrap();
        assert_eq!(answer["type"], "cancel_result");
        assert_eq!(answer["acknowledged"], true);
        assert_eq!(answer["confirmed"], false);
        assert_eq!(answer["outcome"], "cancelled");
        assert_eq!(answer["dispatched"], false);
        assert_eq!(service.claims.len(), 1);
    }

    #[test]
    fn cancel_after_dispatch_preserves_the_unknown_effect() {
        let caller = keys(1);
        let worker = keys(2);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        let (claim, root) = reserved(&mut service, &caller, &worker);
        service.acknowledge(&claim.key, &root).unwrap();
        service.intend(&claim.key).unwrap();
        let own = control_event(
            &caller,
            &worker,
            &claim,
            "cancel",
            json!({"reason": "stop"}),
        );
        let opened =
            open_request(&own, &worker.pubkey, &worker.secret, NOW, Window::DEFAULT).unwrap();
        let answer = service.control(&opened).unwrap();
        assert_eq!(answer["confirmed"], false);
        assert!(service.claims[&claim.key].cancel_requested);
        assert!(service.claims[&claim.key].outcome.is_none());
        let late = service
            .observe(
                &claim.key,
                Report {
                    outcome: "completed",
                    dispatched: true,
                    output: Some(json!({"count": 2})),
                    artifacts: vec![artifact(b"out")],
                    receipts: Vec::new(),
                    spend: None,
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: None,
                    message: None,
                },
            )
            .unwrap();
        assert_eq!(late["outcome"], "completed");
        assert!(late["spend"].is_null());
        assert_eq!(late["verification"], "not_run");
        assert_eq!(late["output"]["count"], 2);
    }

    #[test]
    fn replay_names_gaps_and_truncation() {
        let caller = keys(1);
        let worker = keys(2);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        let (claim, root) = reserved(&mut service, &caller, &worker);
        service.acknowledge(&claim.key, &root).unwrap();
        service
            .append_record(&claim.key, 2, &digest_bytes(b"later"))
            .unwrap();
        let replay = control_event(
            &caller,
            &worker,
            &claim,
            "replay",
            json!({"after_seq": null, "max_records": 1}),
        );
        let opened = open_request(
            &replay,
            &worker.pubkey,
            &worker.secret,
            NOW,
            Window::DEFAULT,
        )
        .unwrap();
        let answer = service.control(&opened).unwrap();
        assert_eq!(answer["gap"], true);
        assert_eq!(answer["complete"], false);
        assert_eq!(answer["code"], "content_unavailable");
        assert_eq!(answer["records"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn a_tombstone_does_not_recreate_the_effect() {
        let caller = keys(1);
        let worker = keys(2);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        let (claim, _) = reserved(&mut service, &caller, &worker);
        let active = service.active;
        service.sweep(RETAIN);
        assert!(service.claims[&claim.key].tombstone);
        assert_eq!(service.active, 0);
        let opened = opened(&caller, &worker, &body());
        let Admission::Retransmission(found) =
            service.prepare(&opened, NOW, &"ab".repeat(32)).unwrap()
        else {
            panic!("the fingerprint still names the tombstone");
        };
        assert!(found.tombstone);
        assert_eq!(service.active, 0);
        assert_eq!(active, 1);
        assert_eq!(
            service.acknowledge(&claim.key, b"gone").unwrap_err().code(),
            Some("content_unavailable")
        );
    }

    #[test]
    fn stale_expiration_and_retention_are_refused() {
        let caller = keys(1);
        let worker = keys(2);
        let mut stale = body();
        let old = Seal {
            created_at: NOW - 10_000,
            ..seal_between(&caller, &worker, 4)
        }
        .event(
            REQUEST_KIND,
            vec![
                Tag::new(vec!["p".into(), worker.pubkey.clone()]),
                Tag::new(vec!["expiration".into(), DEADLINE.to_string()]),
            ],
            &stale,
        )
        .unwrap();
        assert_eq!(
            open_request(&old, &worker.pubkey, &worker.secret, NOW, Window::DEFAULT)
                .unwrap_err()
                .code(),
            Some("stale")
        );
        stale["deadline"] = json!(DEADLINE + 1);
        let mismatched = request(&caller, &worker, &stale, 5);
        assert_eq!(
            open_request(
                &mismatched,
                &worker.pubkey,
                &worker.secret,
                NOW,
                Window::DEFAULT
            )
            .unwrap_err()
            .code(),
            Some("malformed")
        );
        let mut short = body();
        short["retain_until"] = json!(DEADLINE);
        let short = request(&caller, &worker, &short, 6);
        assert_eq!(
            open_request(&short, &worker.pubkey, &worker.secret, NOW, Window::DEFAULT)
                .unwrap_err()
                .code(),
            Some("malformed")
        );
    }

    #[test]
    fn unknown_spend_is_not_a_remaining_budget() {
        let bounds = Bounds {
            wall_ms: None,
            output_bytes: None,
            jobs: None,
            spend_microunits: Some(10),
        };
        assert_eq!(
            reserve_spend(&bounds, None, Some(10)).unwrap_err().code(),
            Some("cannot_enforce")
        );
        assert_eq!(
            reserve_spend(&bounds, Some(1), None).unwrap_err().code(),
            Some("cannot_enforce")
        );
        assert_eq!(reserve_spend(&bounds, Some(1), Some(10)).unwrap(), Some(1));
    }

    #[test]
    fn missing_pins_are_unavailable() {
        let caller = keys(1);
        let worker = keys(2);
        let opened = opened(&caller, &worker, &body());
        let Body::Execute(execute) = opened.body else {
            panic!("execute");
        };
        assert_eq!(
            require_pins(&execute, &BTreeMap::new()).unwrap_err().code(),
            Some("content_unavailable")
        );
        assert!(require_pins(&execute, &pins_of(&body())).is_ok());
    }

    #[test]
    fn the_caller_keeps_gaps_conflicts_and_a_closed_socket_distinct() {
        let mut watch = Watch::new();
        watch.note_relay_ok();
        assert!(watch.relay_ok);
        assert!(watch.accepted.is_none());
        assert!(!watch.missing_feedback_proves_idle());
        watch.ingest(
            "1",
            &json!({"type": "progress", "seq": 0, "status": "queued"}),
        );
        watch.ingest(
            "2",
            &json!({"type": "progress", "seq": 2, "status": "running"}),
        );
        assert!(!watch.rendering);
        watch.ingest(
            "3",
            &json!({"type": "result", "outcome": "completed", "spend": null}),
        );
        watch.ingest(
            "4",
            &json!({"type": "result", "outcome": "failed", "spend": null}),
        );
        assert!(watch.conflict);
        assert_eq!(watch.results.len(), 2);
        watch.note_socket_closed();
        assert!(watch.socket_closed);
        assert_eq!(watch.results.len(), 2);
        assert_eq!(contact_cause(false), "worker_absent");
        assert_eq!(contact_cause(true), "worker_silent");
    }

    #[test]
    fn a_revoked_target_is_refused() {
        let caller = keys(1);
        let worker = keys(2);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        service.revoke_target(digest_bytes(b"target"));
        let opened = opened(&caller, &worker, &body());
        assert_eq!(
            service
                .prepare(&opened, NOW, &"ab".repeat(32))
                .unwrap_err()
                .code(),
            Some("revoked")
        );
        assert_eq!(service.active, 0);
    }

    #[test]
    fn progress_does_not_complete_the_attempt() {
        let caller = keys(1);
        let worker = keys(2);
        let mut service = Service::new(worker.pubkey.clone(), 2, RETAIN);
        let (claim, root) = reserved(&mut service, &caller, &worker);
        service.acknowledge(&claim.key, &root).unwrap();
        let note = service.progress(&claim.key, 0, "running").unwrap();
        assert_eq!(note["type"], "progress");
        assert!(service.claims[&claim.key].winner().is_none());
        assert!(service.progress(&claim.key, 2, "running").is_err());
    }

    fn control_event(from: &Keys, to: &Keys, claim: &Claim, kind: &str, extra: Value) -> Event {
        let mut payload = json!({
            "v": SCHEMA,
            "requires": [],
            "type": kind,
            "request": claim.request,
            "attempt": claim.attempt,
            "run": claim.run
        });
        let map = payload.as_object_mut().unwrap();
        for (key, value) in extra.as_object().cloned().unwrap_or_default() {
            map.insert(key, value);
        }
        seal_between(from, to, 11)
            .event(
                REQUEST_KIND,
                vec![
                    Tag::new(vec!["e".into(), claim.execute_event.clone()]),
                    Tag::new(vec!["p".into(), to.pubkey.clone()]),
                ],
                &payload,
            )
            .unwrap()
    }
}
