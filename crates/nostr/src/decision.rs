//! The NIP-CJ decision-job family: pure wire-shape validation for kinds
//! `25910`, `26910`, and `27010`.
//!
//! One decision job carries one `POST /v1/systemone` call — `state` plus
//! typed `questions` — over the relay instead of HTTP. The wire shapes are
//! `nips/openagents/NIP-CJ.md` ("Decision jobs"); the service semantics are
//! `docs/decision-models/relay-decision-contract.md`. This module is the
//! protocol half both ends share: envelope construction, the checks a
//! signature can back, and the bounds every field lives under.
//!
//! What this layer is not, because honesty about it is what keeps the
//! contract intact:
//!
//! - It holds no state. Deduplication by event `id` is the caller's and the
//!   worker's own bounded set; durable `(request, attempt)` settlement is
//!   `tenancy::quota`'s ledger, not a helper here. `idempotency_key` is a
//!   naming scheme for that scope, nothing more.
//! - It maps no principal to a tenant. The verified signer's pubkey is the
//!   principal; the npub-to-tenant binding is operator-provisioned and
//!   lives outside the payload.
//! - It runs no job. A worker that uses it still needs a door, a
//!   concurrency bound, and a publish loop.
//! - It does not verify the embedded execution receipt beyond its schema
//!   tag and its correlation fields; sealing and verifying a receipt is
//!   `receipts::execution`'s job.
//!
//! The relay is transport, not authority. Every acceptance check here runs
//! on what the signature covers — kind, signer, `e` and `p` tags, and the
//! decrypted payload's own `request`/`attempt` — so a relabeled or replayed
//! event fails as unbound rather than reading as an answer.

use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::domain::{DomainError, Event, RelaySigner, Tag};
use crate::nip44;

/// The decision job request, caller to worker. Ephemeral.
pub const REQUEST_KIND: u16 = 25_910;
/// The decision job result, worker to caller. Ephemeral.
pub const RESULT_KIND: u16 = 26_910;
/// The decision job status, worker to caller. Ephemeral.
pub const FEEDBACK_KIND: u16 = 27_010;

/// The payload schema tag every envelope in this family leads with. A
/// string, never the integer `v` of the conversation family, so the two
/// payload grammars cannot share a version check.
pub const SCHEMA: &str = "openagents.systemone.v1";
/// The schema tag the embedded execution receipt carries.
pub const RECEIPT_SCHEMA: &str = "openagents.receipt.execution.v1";
/// The transport name a relay-lane receipt records.
pub const RELAY_TRANSPORT: &str = "relay";

/// The largest decrypted envelope this layer reads, in bytes. The NIP-44
/// client bound already refuses larger plaintexts; this restates it so the
/// bound is the protocol's own rather than a cipher accident.
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
/// The most bytes a logical `request` id carries.
pub const MAX_REQUEST_ID_BYTES: usize = 128;
/// The most bytes a `model` door name carries.
pub const MAX_MODEL_BYTES: usize = 128;
/// The most serialized bytes `state` carries.
pub const MAX_STATE_BYTES: usize = 128 * 1024;
/// The most questions one request asks. A refusal names
/// `too_many_questions`, as the HTTP lane does.
pub const MAX_QUESTIONS: usize = 64;
/// The most bytes a question id carries.
pub const MAX_QUESTION_ID_BYTES: usize = 64;
/// The most serialized bytes one question carries.
pub const MAX_QUESTION_BYTES: usize = 32 * 1024;
/// The most options a `choice` question names, matching the SDK bound.
pub const MAX_CHOICE_OPTIONS: usize = 255;
/// The fewest levels a `score` question names, matching the SDK bound.
pub const MIN_SCORE_LEVELS: usize = 2;
/// The most levels a `score` question names, matching the SDK bound.
pub const MAX_SCORE_LEVELS: usize = 10;
/// The most bytes a refusal `code` carries.
pub const MAX_CODE_BYTES: usize = 64;
/// The most bytes a refusal `message` carries.
pub const MAX_MESSAGE_BYTES: usize = 1_024;

/// Payload field names a credential could hide behind. A field claiming to
/// be a bearer secret authorizes nothing — the payload carries no
/// credential — so its presence is `malformed` and the credential is
/// treated as already leaked.
const CREDENTIAL_FIELDS: &[&str] = &[
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

/// How old a request event's `created_at` may be and how far ahead of the
/// worker's clock it may sit, in seconds.
///
/// Every kind in the family is ephemeral: a live request is at most a
/// clock skew old, so one outside the window is a replay or a forgery of
/// freshness, refused `stale`. The default past side matches the
/// conversation family's ten minutes; the future side matches the relay's
/// `max_future_seconds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestWindow {
    /// How far in the past `created_at` may be.
    pub max_age_seconds: u64,
    /// How far in the future `created_at` may be.
    pub max_future_seconds: u64,
}

impl RequestWindow {
    /// Ten minutes back, five minutes ahead.
    pub const DEFAULT: Self = Self {
        max_age_seconds: 10 * 60,
        max_future_seconds: 5 * 60,
    };

    /// A window of the given sizes, in seconds.
    #[must_use]
    pub const fn new(max_age_seconds: u64, max_future_seconds: u64) -> Self {
        Self {
            max_age_seconds,
            max_future_seconds,
        }
    }

    /// Whether `created_at` sits inside the window at `now`.
    #[must_use]
    pub fn admits(&self, created_at: u64, now: u64) -> bool {
        now.saturating_sub(created_at) <= self.max_age_seconds
            && created_at.saturating_sub(now) <= self.max_future_seconds
    }
}

/// Why an event or a payload was not accepted.
///
/// The variants split by what the failure means, because each maps to a
/// different handling: [`DecisionError::code`] names the typed refusal a
/// worker can send back when the signer is verified, and returns `None`
/// for the failures that are not answerable — an event that was never
/// deliverable here, a signature that proves nothing about its claimed
/// signer, and a signed event that does not bind to the job in flight.
#[derive(Debug)]
pub enum DecisionError {
    /// The event's kind is not one this direction accepts. The relay's
    /// filter is the first rejection; an event that arrives anyway is
    /// rejected before anything is decrypted.
    UnexpectedKind { kind: u16 },
    /// NIP-01 structure, id, or signature verification failed. The claimed
    /// signer is unproven, so a refusal would be addressed to nobody.
    Event(DomainError),
    /// No `p` tag names the recipient this direction answers to.
    NotAddressed,
    /// A correctly signed event that does not bind to the job in flight:
    /// the signer is not the expected worker, no `e` tag names this
    /// attempt's request event, no `p` tag names this caller, or the
    /// payload's `request`/`attempt` names another job. This is the
    /// replay-prone correlation the contract counts on — a relay can
    /// relabel a delivery but cannot make the signature cover this job.
    Unbound { field: &'static str },
    /// `created_at` is outside the request window, or the event's NIP-40
    /// `expiration` has passed.
    Stale { created_at: u64, now: u64 },
    /// The payload's `deadline` passed before admission.
    DeadlinePassed { deadline: u64, now: u64 },
    /// The payload does not decrypt, does not parse, is not a JSON object,
    /// names an unknown `type`, or fails a structural bound.
    Malformed { reason: String },
    /// `v` is absent or names a schema this layer does not serve.
    UnsupportedVersion { found: Option<String> },
    /// A field shaped like a credential appeared in the payload. Refused
    /// as `malformed`: a caller that pastes its key into job content has
    /// already leaked it to the worker.
    Credential { field: String },
    /// The request envelope is missing `request`, `attempt`, `model`,
    /// `state`, or `questions`, or one fails validation.
    InvalidRequest { reason: String },
    /// The envelope asks more than [`MAX_QUESTIONS`] questions.
    TooManyQuestions { count: usize },
    /// One question names more than [`MAX_CHOICE_OPTIONS`] options.
    TooManyOptions { question: String, count: usize },
    /// A cancellation arrived signed by a key other than the original
    /// request's. A guessed request id grants no cancellation authority.
    WrongPrincipal,
}

impl DecisionError {
    /// The refusal code the worker can answer this failure with, when the
    /// event's signer is verified enough to be told. `None` means the
    /// event is not answerable: drop it and let the caller's contact
    /// deadline report `worker_absent`.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Stale { .. } | Self::DeadlinePassed { .. } => Some("stale"),
            Self::Malformed { .. } | Self::Credential { .. } => Some("malformed"),
            Self::UnsupportedVersion { .. } => Some("unsupported_version"),
            Self::InvalidRequest { .. } => Some("invalid_request"),
            Self::TooManyQuestions { .. } => Some("too_many_questions"),
            Self::TooManyOptions { .. } => Some("too_many_options"),
            Self::WrongPrincipal => Some("not_admitted"),
            Self::UnexpectedKind { .. }
            | Self::Event(_)
            | Self::NotAddressed
            | Self::Unbound { .. } => None,
        }
    }

    /// Whether the failure can be told to the event's signer as a typed
    /// refusal — [`DecisionError::code`] is `Some`.
    #[must_use]
    pub fn answerable(&self) -> bool {
        self.code().is_some()
    }
}

impl std::fmt::Display for DecisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedKind { kind } => {
                write!(
                    f,
                    "kind {kind} is not a decision-job kind for this direction"
                )
            }
            Self::Event(error) => write!(f, "the event does not verify: {error}"),
            Self::NotAddressed => f.write_str("no p tag names the expected recipient"),
            Self::Unbound { field } => {
                write!(
                    f,
                    "the event does not bind to this job: {field} does not match"
                )
            }
            Self::Stale { created_at, now } => write!(
                f,
                "the request was created at {created_at}, outside the request window at {now}"
            ),
            Self::DeadlinePassed { deadline, now } => {
                write!(
                    f,
                    "the deadline {deadline} passed before admission at {now}"
                )
            }
            Self::Malformed { reason } => write!(f, "malformed payload: {reason}"),
            Self::UnsupportedVersion { found } => write!(
                f,
                "payload schema {} is not {SCHEMA}",
                found.as_deref().unwrap_or("<absent>")
            ),
            Self::Credential { field } => write!(
                f,
                "the payload carries a credential-shaped field `{field}`; treat it as leaked"
            ),
            Self::InvalidRequest { reason } => write!(f, "invalid request: {reason}"),
            Self::TooManyQuestions { count } => write!(
                f,
                "the request asks {count} questions, more than {MAX_QUESTIONS}"
            ),
            Self::TooManyOptions { question, count } => write!(
                f,
                "question `{question}` names {count} options, more than {MAX_CHOICE_OPTIONS}"
            ),
            Self::WrongPrincipal => {
                f.write_str("the cancel signer is not the original request signer")
            }
        }
    }
}

impl std::error::Error for DecisionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Event(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DomainError> for DecisionError {
    fn from(error: DomainError) -> Self {
        Self::Event(error)
    }
}

/// The content of a `type: "systemone"` request payload, validated.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestBody {
    /// The logical request identity, caller-chosen and stable across
    /// retries — the same role `Idempotency-Key` plays on the HTTP lane.
    pub request: String,
    /// The one-based attempt number. A retry keeps `request` and bumps
    /// this, exactly as `X-Attempt` does on the HTTP lane.
    pub attempt: u32,
    /// The door the call is authorized against, as `POST /v1/systemone`
    /// names it.
    pub model: String,
    /// The state every question reads, verbatim.
    pub state: Value,
    /// The questions to ask, keyed by id.
    pub questions: Map<String, Value>,
    /// The latest time an answer is useful, unix seconds. The worker
    /// refuses work that cannot start before it.
    pub deadline: Option<u64>,
}

impl RequestBody {
    /// A request body with no deadline. Validation runs at seal and at
    /// admission, not here — this type is a container until then.
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        attempt: u32,
        model: impl Into<String>,
        state: Value,
        questions: Map<String, Value>,
    ) -> Self {
        Self {
            request: request.into(),
            attempt,
            model: model.into(),
            state,
            questions,
            deadline: None,
        }
    }

    /// The same body with a deadline.
    #[must_use]
    pub fn deadline(mut self, deadline: u64) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// The payload as it goes inside the encrypted content.
    #[must_use]
    pub fn payload(&self) -> Value {
        let mut payload = json!({
            "v": SCHEMA,
            "type": "systemone",
            "request": self.request,
            "attempt": self.attempt,
            "model": self.model,
            "state": self.state,
            "questions": self.questions,
        });
        if let Some(deadline) = self.deadline {
            payload["deadline"] = json!(deadline);
        }
        payload
    }

    /// The canonical digest of the execution-affecting envelope —
    /// `{model, state, questions, request, attempt, deadline}` with an
    /// absent deadline represented as null, so caller and worker digest
    /// the same bytes. This is what a receipt's `request_digest` records
    /// and what a repeated `(request, attempt)` pair is compared by.
    #[must_use]
    pub fn digest(&self) -> String {
        digest_canonical(&json!({
            "attempt": self.attempt,
            "deadline": self.deadline,
            "model": self.model,
            "questions": self.questions,
            "request": self.request,
            "state": self.state,
        }))
    }

    /// The field checks [`admit`] runs on a parsed envelope.
    pub fn validate(&self) -> Result<(), DecisionError> {
        check_identifier(&self.request, "request")?;
        if self.attempt == 0 {
            return Err(invalid_request(
                "attempt is one-based; zero is not an attempt",
            ));
        }
        bounded_str(&self.model, MAX_MODEL_BYTES, "model")
            .map_err(|reason| invalid_request(&reason))?;
        if self.model.is_empty() {
            return Err(invalid_request("the envelope names no model"));
        }
        let state_bytes = serialized_len(&self.state);
        if state_bytes > MAX_STATE_BYTES {
            return Err(invalid_request(&format!(
                "state is {state_bytes} bytes, more than {MAX_STATE_BYTES}"
            )));
        }
        check_questions(&self.questions)?;
        Ok(())
    }
}

/// A `type: "cancel"` request payload: best-effort cancellation of a
/// request the same signer published.
#[must_use]
pub fn cancel_payload(request: &str) -> Value {
    json!({"v": SCHEMA, "type": "cancel", "request": request})
}

/// The `status` vocabulary of a kind-`27010` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The job was admitted and is waiting for a slot.
    Queued,
    /// The job is running.
    Processing,
    /// Terminal refusal; the refusal fields carry the reason.
    Error,
}

impl Status {
    /// The wire word for this status.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Error => "error",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "processing" => Some(Self::Processing),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// A typed refusal: the `code`, `message`, and `retry_after_ms` a status
/// `error` or a non-answered result carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The refusal vocabulary of the HTTP lane plus the relay lane's own
    /// causes — `stale`, `not_admitted`, `unsupported_version`, and the
    /// rest of the contract's table.
    pub code: String,
    /// Display text, when the refusal has any.
    pub message: Option<String>,
    /// When a retry may be worth attempting, the role `Retry-After` plays
    /// on HTTP.
    pub retry_after_ms: Option<u64>,
}

impl Refusal {
    /// A refusal carrying only its code.
    #[must_use]
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: None,
            retry_after_ms: None,
        }
    }

    /// The same refusal with display text.
    #[must_use]
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// The same refusal with a retry hint.
    #[must_use]
    pub fn retry_after_ms(mut self, retry_after_ms: u64) -> Self {
        self.retry_after_ms = Some(retry_after_ms);
        self
    }

    /// A refusal from a decision error's own vocabulary.
    #[must_use]
    pub fn from_error(error: &DecisionError) -> Option<Self> {
        error
            .code()
            .map(|code| Self::new(code).message(error.to_string()))
    }

    fn write_into(&self, map: &mut Map<String, Value>) {
        map.insert("code".into(), json!(self.code));
        if let Some(message) = &self.message {
            map.insert("message".into(), json!(message));
        }
        if let Some(retry_after_ms) = self.retry_after_ms {
            map.insert("retry_after_ms".into(), json!(retry_after_ms));
        }
    }

    fn parse(value: &Value, where_is: &str) -> Result<Self, DecisionError> {
        let code = value
            .get("code")
            .and_then(Value::as_str)
            .ok_or_else(|| malformed(&format!("{where_is} carries no `code`")))?;
        bounded_str(code, MAX_CODE_BYTES, "code").map_err(|r| malformed(&r))?;
        let message = optional_str(value, "message", MAX_MESSAGE_BYTES, where_is)?;
        let retry_after_ms = match value.get("retry_after_ms") {
            None | Some(Value::Null) => None,
            Some(v) => Some(v.as_u64().ok_or_else(|| {
                malformed(&format!("{where_is} `retry_after_ms` is not an integer"))
            })?),
        };
        Ok(Self {
            code: code.to_owned(),
            message,
            retry_after_ms,
        })
    }
}

/// A progress status payload as it goes on the wire.
#[must_use]
pub fn status_payload(request: &str, attempt: u32, status: Status) -> Value {
    json!({
        "v": SCHEMA,
        "type": "status",
        "request": request,
        "attempt": attempt,
        "status": status.as_str(),
    })
}

/// A terminal refusal status payload as it goes on the wire.
#[must_use]
pub fn refusal_payload(request: &str, attempt: u32, refusal: &Refusal) -> Value {
    let mut payload = status_payload(request, attempt, Status::Error);
    refusal.write_into(
        payload
            .as_object_mut()
            .expect("a status payload is an object"),
    );
    payload
}

/// The `outcome` vocabulary of a kind-`26910` result — the receipt's own
/// vocabulary, so an answer means the same thing on either transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The door produced an answer.
    Answered,
    /// The door declined — a refusal is a recorded outcome.
    Refused,
    /// The attempt was admitted but never dispatched.
    Unattempted,
    /// Transport or capacity denied the call before the door decided.
    Unavailable,
    /// The service cannot say what happened. Never a successful answer.
    Unknown,
}

impl Outcome {
    /// The wire word for this outcome.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Answered => "answered",
            Self::Refused => "refused",
            Self::Unattempted => "unattempted",
            Self::Unavailable => "unavailable",
            Self::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "answered" => Some(Self::Answered),
            "refused" => Some(Self::Refused),
            "unattempted" => Some(Self::Unattempted),
            "unavailable" => Some(Self::Unavailable),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// What a job ended in, for [`result_payload`].
#[derive(Debug)]
pub enum Resolution {
    /// `outcome: "answered"` — the `POST /v1/systemone` response body,
    /// verbatim.
    Answered(Value),
    /// A terminal outcome with no answer: `refused`, `unattempted`,
    /// `unavailable`, or `unknown`, plus the refusal-shaped `error`.
    Ended { outcome: Outcome, error: Refusal },
}

/// The result payload as it goes on the wire: outcome, the response or the
/// error, and the sealed execution receipt with `transport: "relay"`.
pub fn result_payload(
    request: &str,
    attempt: u32,
    resolution: &Resolution,
    receipt: &Value,
) -> Result<Value, DecisionError> {
    check_receipt(receipt)?;
    let mut payload = json!({
        "v": SCHEMA,
        "type": "result",
        "request": request,
        "attempt": attempt,
        "receipt": receipt,
    });
    let map = payload
        .as_object_mut()
        .expect("a result payload is an object");
    match resolution {
        Resolution::Answered(response) => {
            if !response.is_object() {
                return Err(malformed(
                    "an answered result's `response` is not an object",
                ));
            }
            map.insert("outcome".into(), json!(Outcome::Answered.as_str()));
            map.insert("response".into(), response.clone());
        }
        Resolution::Ended { outcome, error } => {
            if *outcome == Outcome::Answered {
                return Err(malformed("an ended result cannot claim outcome `answered`"));
            }
            map.insert("outcome".into(), json!(outcome.as_str()));
            let mut error_value = Map::new();
            error.write_into(&mut error_value);
            map.insert("error".into(), Value::Object(error_value));
        }
    }
    Ok(payload)
}

/// A decrypted kind-`27010` status payload, validated.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusPayload {
    /// The logical request it speaks for.
    pub request: String,
    /// The attempt it speaks for.
    pub attempt: u32,
    /// The status word.
    pub status: Status,
    /// The typed refusal, present when `status` is [`Status::Error`].
    pub refusal: Option<Refusal>,
}

/// A decrypted kind-`26910` result payload, validated.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultPayload {
    /// The logical request it answers.
    pub request: String,
    /// The attempt it answers.
    pub attempt: u32,
    /// What the attempt came to.
    pub outcome: Outcome,
    /// The verbatim systemone response, present when `outcome` is
    /// [`Outcome::Answered`].
    pub response: Option<Value>,
    /// The refusal-shaped `error`, present on every non-answered outcome.
    pub refusal: Option<Refusal>,
    /// The sealed execution receipt. This layer checks its schema tag and
    /// its `request`/`attempt` correlation only; `receipts::execution`
    /// verifies the seal.
    pub receipt: Value,
}

/// What signing one event takes: the identity, the NIP-44 conversation
/// the payload encrypts under, a fresh nonce, and the timestamp.
///
/// One seal per event — a nonce reused across payloads of one conversation
/// reuses the cipher's keystream, so each build takes its own.
#[derive(Clone, Copy)]
pub struct Seal<'a> {
    /// The signer that puts the sender's key on the event.
    pub signer: &'a RelaySigner,
    /// `nip44::conversation_key` of the sender's secret and the peer's
    /// public key.
    pub conversation: [u8; 32],
    /// A fresh 32-byte NIP-44 nonce.
    pub nonce: [u8; 32],
    /// The event's `created_at`, unix seconds.
    pub created_at: u64,
}

impl Seal<'_> {
    /// Encrypt `payload` and sign it as `kind` with `tags`.
    ///
    /// # Errors
    ///
    /// Returns [`DecisionError::Malformed`] when the serialized payload
    /// exceeds [`MAX_PAYLOAD_BYTES`].
    pub fn event(
        &self,
        kind: u16,
        tags: Vec<Tag>,
        payload: &Value,
    ) -> Result<Event, DecisionError> {
        let plaintext = payload.to_string();
        if plaintext.len() > MAX_PAYLOAD_BYTES {
            return Err(malformed(&format!(
                "the payload serializes to {} bytes, more than {MAX_PAYLOAD_BYTES}",
                plaintext.len()
            )));
        }
        let content = nip44::encrypt(&plaintext, &self.conversation, self.nonce)
            .map_err(|error| malformed(&format!("the payload does not encrypt: {error}")))?;
        Ok(self.signer.sign(self.created_at, kind, tags, content))
    }
}

/// A request event that passed every check the signature can back, as the
/// worker sees it.
#[derive(Debug)]
pub enum Admitted {
    /// A `type: "systemone"` decision call.
    Call(AdmittedCall),
    /// A `type: "cancel"` cancellation.
    Cancel(AdmittedCancel),
}

/// An admitted decision call: the verified signer, the attempt's transport
/// identity, and the validated envelope.
#[derive(Debug)]
pub struct AdmittedCall {
    /// The verified request signer — the principal a tenant binding maps.
    pub principal: String,
    /// The request event's `id`: this attempt's transport identity, and
    /// the receipt's `attempt_id`.
    pub attempt_id: String,
    /// The request event's `created_at`.
    pub created_at: u64,
    /// The validated envelope.
    pub body: RequestBody,
    /// `body.digest()`, precomputed for the reservation and the receipt.
    pub request_digest: String,
    /// The decrypted envelope as received, for fields this layer does not
    /// model — a later schema revision's additions land here.
    pub payload: Value,
}

impl AdmittedCall {
    /// The tags that bind a worker event to this job: `e` to this
    /// attempt's request event, `p` to the caller.
    fn reply_tags(&self) -> Vec<Tag> {
        vec![
            Tag::new(vec!["e".into(), self.attempt_id.clone()]),
            Tag::new(vec!["p".into(), self.principal.clone()]),
        ]
    }

    /// A kind-`27010` progress status event bound to this job.
    pub fn status_event(&self, seal: Seal<'_>, status: Status) -> Result<Event, DecisionError> {
        let payload = status_payload(&self.body.request, self.body.attempt, status);
        seal.event(FEEDBACK_KIND, self.reply_tags(), &payload)
    }

    /// A kind-`27010` terminal refusal bound to this job.
    pub fn refusal_event(&self, seal: Seal<'_>, refusal: &Refusal) -> Result<Event, DecisionError> {
        let payload = refusal_payload(&self.body.request, self.body.attempt, refusal);
        seal.event(FEEDBACK_KIND, self.reply_tags(), &payload)
    }

    /// The kind-`26910` terminal result bound to this job, carrying the
    /// sealed execution receipt.
    pub fn result_event(
        &self,
        seal: Seal<'_>,
        resolution: &Resolution,
        receipt: &Value,
    ) -> Result<Event, DecisionError> {
        let payload = result_payload(&self.body.request, self.body.attempt, resolution, receipt)?;
        seal.event(RESULT_KIND, self.reply_tags(), &payload)
    }
}

/// An admitted cancellation: a signed, addressed `type: "cancel"` naming a
/// logical request and `e`-tagged to the request event it cancels.
///
/// Cancellation authority comes from the signer, not the reference: only
/// [`AdmittedCancel::authorize`] decides whether this event may act on a
/// job, and it requires the original caller's key.
#[derive(Debug)]
pub struct AdmittedCancel {
    /// The verified cancel signer.
    pub principal: String,
    /// The cancel event's own `id`.
    pub cancel_id: String,
    /// The request event the cancel `e`-tags.
    pub target: String,
    /// The logical request it names.
    pub request: String,
}

impl AdmittedCancel {
    /// Whether this cancellation may act on a request signed by
    /// `request_principal`. A guessed request id or an `e` tag grants
    /// nothing on its own.
    #[must_use]
    pub fn authorizes(&self, request_principal: &str) -> bool {
        self.principal == request_principal
    }

    /// The same check as a typed error, for a worker that refuses rather
    /// than ignores.
    pub fn authorize(&self, request_principal: &str) -> Result<(), DecisionError> {
        if self.authorizes(request_principal) {
            Ok(())
        } else {
            Err(DecisionError::WrongPrincipal)
        }
    }
}

/// What a bound worker event carries, as the caller sees it.
#[derive(Debug)]
pub enum Answer {
    /// A kind-`27010` status: progress or a terminal typed refusal.
    Status(StatusPayload),
    /// A kind-`26910` result: the job's terminal event.
    Result(ResultPayload),
}

/// The job a caller is waiting on: everything a worker event must name to
/// count as an answer to it.
///
/// The subscription label a relay delivers an event under is an unsigned
/// routing hint; this struct is the signed identity the label can only
/// point at.
#[derive(Debug, Clone, Copy)]
pub struct Pending<'a> {
    /// This attempt's request event `id` — an answer `e`-tags it.
    pub attempt_id: &'a str,
    /// The worker pubkey (hex) the request was sent to — an answer is
    /// signed by it.
    pub worker: &'a str,
    /// The caller's own pubkey (hex) — an answer `p`-tags it.
    pub customer: &'a str,
    /// The logical request id the payload must name.
    pub request: &'a str,
    /// The attempt number the payload must name.
    pub attempt: u32,
}

/// Whether `event` is a decision-job request addressed to `worker`, and
/// what it asks, as the worker sees it.
///
/// The checks run in the contract's order: kind, then what the signature
/// covers — structure, id, signature, the `p` tag — then `created_at`
/// against `window`, and only then decryption and the envelope. A failure
/// maps to a typed refusal through [`DecisionError::code`]; `None` means
/// the event is not this worker's to answer.
///
/// `secret` is the worker's own secret key, for the NIP-44 conversation
/// the payload is encrypted under. `now` is unix seconds.
///
/// # Errors
///
/// Returns [`DecisionError`] — see its variants for which are answerable.
pub fn admit(
    event: &Event,
    worker: &str,
    secret: &SecretKey,
    now: u64,
    window: RequestWindow,
) -> Result<Admitted, DecisionError> {
    if event.kind != REQUEST_KIND {
        return Err(DecisionError::UnexpectedKind { kind: event.kind });
    }
    event.validate_structure()?;
    event.validate_crypto()?;
    if !event.tag_values("p").any(|key| key == worker) {
        return Err(DecisionError::NotAddressed);
    }
    if !window.admits(event.created_at, now) {
        return Err(DecisionError::Stale {
            created_at: event.created_at,
            now,
        });
    }
    if event.is_expired(now) {
        return Err(DecisionError::Stale {
            created_at: event.created_at,
            now,
        });
    }

    let payload = decrypt_payload(event, secret)?;
    check_schema(&payload)?;
    check_credentials(&payload)?;
    match payload.get("type").and_then(Value::as_str) {
        Some("systemone") => {
            let body = parse_request(&payload)?;
            if let Some(deadline) = body.deadline
                && deadline <= now
            {
                return Err(DecisionError::DeadlinePassed { deadline, now });
            }
            Ok(Admitted::Call(AdmittedCall {
                principal: event.pubkey.clone(),
                attempt_id: event.id.clone(),
                created_at: event.created_at,
                request_digest: body.digest(),
                body,
                payload,
            }))
        }
        Some("cancel") => {
            let request = payload
                .get("request")
                .and_then(Value::as_str)
                .ok_or_else(|| malformed("a cancel names no `request`"))?;
            identifier_reason(request, "request").map_err(|reason| malformed(&reason))?;
            let targets: Vec<&str> = event.tag_values("e").collect();
            let [target] = targets.as_slice() else {
                return Err(malformed("a cancel e-tags exactly one request event"));
            };
            event_id(target).map_err(|reason| malformed(&reason))?;
            Ok(Admitted::Cancel(AdmittedCancel {
                principal: event.pubkey.clone(),
                cancel_id: event.id.clone(),
                target: (*target).to_owned(),
                request: request.to_owned(),
            }))
        }
        other => Err(malformed(&format!(
            "kind {REQUEST_KIND} carries unknown type {}",
            other.unwrap_or("<absent>")
        ))),
    }
}

/// Whether `event` is an answer to `pending`, and what it says, as the
/// caller sees it.
///
/// Every field checked is inside the signature: the kind, the worker's
/// key, this attempt's `e` tag, this caller's `p` tag, and the decrypted
/// `request`/`attempt`. A correctly signed answer to an older attempt
/// fails the `e` check; a relabeled one fails the payload check. Both
/// return [`DecisionError::Unbound`], which carries no code — the caller
/// ignores the event and keeps waiting rather than treating it as a
/// refusal. Deduplication of delivered event ids is the caller's own
/// bounded set, run after these checks.
///
/// `secret` is the caller's own secret key.
///
/// # Errors
///
/// Returns [`DecisionError`] — see its variants for which are answerable.
pub fn bind_answer(
    event: &Event,
    pending: &Pending<'_>,
    secret: &SecretKey,
) -> Result<Answer, DecisionError> {
    if event.kind != RESULT_KIND && event.kind != FEEDBACK_KIND {
        return Err(DecisionError::UnexpectedKind { kind: event.kind });
    }
    if event.pubkey != pending.worker {
        return Err(DecisionError::Unbound { field: "pubkey" });
    }
    event.validate_structure()?;
    event.validate_crypto()?;
    if !event.tag_values("e").any(|id| id == pending.attempt_id) {
        return Err(DecisionError::Unbound { field: "e" });
    }
    if !event.tag_values("p").any(|key| key == pending.customer) {
        return Err(DecisionError::Unbound { field: "p" });
    }

    let payload = decrypt_payload(event, secret)?;
    check_schema(&payload)?;
    let expected_type = if event.kind == RESULT_KIND {
        "result"
    } else {
        "status"
    };
    let found_type = payload.get("type").and_then(Value::as_str);
    if found_type != Some(expected_type) {
        return Err(malformed(&format!(
            "kind {} carries type {}, not {expected_type}",
            event.kind,
            found_type.unwrap_or("<absent>")
        )));
    }
    check_correlation(&payload, pending.request, pending.attempt)?;

    if event.kind == RESULT_KIND {
        Ok(Answer::Result(parse_result(&payload)?))
    } else {
        Ok(Answer::Status(parse_status(&payload)?))
    }
}

/// The payload a decrypt yields, checked for shape only — object form and
/// size. The schema check is [`check_schema`]'s; keeping them separate lets
/// a worker recover `request`/`attempt` for a refusal through
/// [`payload_correlation`] when admission already failed.
///
/// `secret` is the reader's own secret key; the conversation key is
/// derived against the event's claimed (and already signature-verified)
/// signer.
///
/// # Errors
///
/// Returns [`DecisionError::Malformed`] when the content does not decrypt,
/// exceeds [`MAX_PAYLOAD_BYTES`], or is not a JSON object.
pub fn decrypt_payload(event: &Event, secret: &SecretKey) -> Result<Value, DecisionError> {
    let peer: XOnlyPublicKey = event
        .pubkey
        .parse()
        .map_err(|_| DecisionError::Event(DomainError::InvalidPublicKey))?;
    let conversation = nip44::conversation_key(secret, &peer);
    let plaintext = nip44::decrypt(&event.content, &conversation).map_err(|error| {
        malformed(&format!(
            "the content does not decrypt under NIP-44: {error}"
        ))
    })?;
    if plaintext.len() > MAX_PAYLOAD_BYTES {
        return Err(malformed(&format!(
            "the payload is {} bytes, more than {MAX_PAYLOAD_BYTES}",
            plaintext.len()
        )));
    }
    let payload: Value = serde_json::from_str(&plaintext)
        .map_err(|error| malformed(&format!("the payload is not JSON: {error}")))?;
    if !payload.is_object() {
        return Err(malformed("the payload is not a JSON object"));
    }
    Ok(payload)
}

/// The `(request, attempt)` a decrypted payload claims, for naming a
/// refusal to a request that could not be admitted. This is a claim, not
/// correlation — [`bind_answer`] checks the same fields against the job in
/// flight before believing them.
#[must_use]
pub fn payload_correlation(payload: &Value) -> Option<(String, u32)> {
    let request = payload.get("request")?.as_str()?;
    let attempt = u32::try_from(payload.get("attempt")?.as_u64()?).ok()?;
    Some((request.to_owned(), attempt))
}

/// The settlement identity of an attempt: `(principal, tenant, request,
/// attempt)` folded into one digest, so two callers' `req-1` attempt 1 —
/// or one caller's under two tenant bindings — never share a reservation.
///
/// This is a naming scheme for the scope `tenancy::quota`'s ledger settles
/// by; it is not deduplication and stores nothing. `tenant` is `None` for
/// a shared door, exactly as an anonymous HTTP call resolves.
#[must_use]
pub fn idempotency_key(
    principal: &str,
    tenant: Option<&str>,
    request: &str,
    attempt: u32,
) -> String {
    digest_canonical(&json!([
        "openagents.systemone.idempotency.v1",
        principal,
        tenant,
        request,
        attempt
    ]))
}

/// The scope a cancellation resolves within: `(principal, tenant,
/// request)`. A cancel names a logical request, and the worker resolves it
/// only inside the signer's own scope — a guessed id names nothing in
/// another principal's scope.
#[must_use]
pub fn request_scope(principal: &str, tenant: Option<&str>, request: &str) -> String {
    digest_canonical(&json!([
        "openagents.systemone.request.v1",
        principal,
        tenant,
        request
    ]))
}

/// The signed, encrypted kind-`25910` request event for `body`, `p`-tagged
/// to `worker` and carrying a NIP-40 `expiration` tag when the body has a
/// deadline.
///
/// # Errors
///
/// Returns [`DecisionError`] when the body fails validation or the payload
/// exceeds [`MAX_PAYLOAD_BYTES`].
pub fn request_event(
    seal: Seal<'_>,
    body: &RequestBody,
    worker: &str,
) -> Result<Event, DecisionError> {
    body.validate()?;
    let mut tags = vec![Tag::new(vec!["p".into(), worker.to_owned()])];
    if let Some(deadline) = body.deadline {
        tags.push(Tag::new(vec!["expiration".into(), deadline.to_string()]));
    }
    seal.event(REQUEST_KIND, tags, &body.payload())
}

/// The signed, encrypted kind-`25910` cancel event for `request`,
/// `e`-tagged to `target` — the request event it cancels — and `p`-tagged
/// to `worker`. Only the original request's signer gives it authority;
/// building one is cheap and means nothing without it.
///
/// # Errors
///
/// Returns [`DecisionError`] when `request` or `target` is malformed or
/// the payload cannot be sealed.
pub fn cancel_event(
    seal: Seal<'_>,
    worker: &str,
    request: &str,
    target: &str,
) -> Result<Event, DecisionError> {
    check_identifier(request, "request")?;
    event_id(target).map_err(|reason| malformed(&reason))?;
    let tags = vec![
        Tag::new(vec!["e".into(), target.to_owned()]),
        Tag::new(vec!["p".into(), worker.to_owned()]),
    ];
    seal.event(REQUEST_KIND, tags, &cancel_payload(request))
}

/// A worker event bound to a job: `payload` encrypted to the caller and
/// signed as `kind` with `e` and `p` tags. This is the general form behind
/// [`AdmittedCall`]'s status, refusal, and result builders — for the reply
/// a worker still owes an event it could not admit, when
/// [`payload_correlation`] recovered enough of a request to name the
/// refusal.
///
/// # Errors
///
/// Returns [`DecisionError::Malformed`] when `request_event_id` is not an
/// event id or the serialized payload exceeds [`MAX_PAYLOAD_BYTES`].
pub fn answer_event(
    seal: Seal<'_>,
    kind: u16,
    request_event_id: &str,
    customer: &str,
    payload: &Value,
) -> Result<Event, DecisionError> {
    event_id(request_event_id).map_err(|reason| malformed(&reason))?;
    let tags = vec![
        Tag::new(vec!["e".into(), request_event_id.to_owned()]),
        Tag::new(vec!["p".into(), customer.to_owned()]),
    ];
    seal.event(kind, tags, payload)
}

/// The `v` check every payload gets before any other field is read: the
/// string `openagents.systemone.v1` or nothing.
fn check_schema(payload: &Value) -> Result<(), DecisionError> {
    match payload.get("v").and_then(Value::as_str) {
        Some(SCHEMA) => Ok(()),
        found => Err(DecisionError::UnsupportedVersion {
            found: found.map(str::to_owned),
        }),
    }
}

/// The `request`/`attempt` an answer must echo to bind to the job in
/// flight.
fn check_correlation(payload: &Value, request: &str, attempt: u32) -> Result<(), DecisionError> {
    if payload.get("request").and_then(Value::as_str) != Some(request) {
        return Err(DecisionError::Unbound { field: "request" });
    }
    if payload.get("attempt").and_then(Value::as_u64) != Some(u64::from(attempt)) {
        return Err(DecisionError::Unbound { field: "attempt" });
    }
    Ok(())
}

/// Refuse a payload carrying a credential-shaped field: it authorizes
/// nothing, and the secret in it is already leaked.
fn check_credentials(payload: &Value) -> Result<(), DecisionError> {
    let Some(map) = payload.as_object() else {
        return Ok(());
    };
    for field in map.keys() {
        if CREDENTIAL_FIELDS.contains(&field.to_ascii_lowercase().as_str()) {
            return Err(DecisionError::Credential {
                field: field.clone(),
            });
        }
    }
    Ok(())
}

/// Parse and validate a `type: "systemone"` envelope.
fn parse_request(payload: &Value) -> Result<RequestBody, DecisionError> {
    let request = payload
        .get("request")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_request("the envelope names no `request`"))?;
    let attempt = payload
        .get("attempt")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_request("the envelope names no integer `attempt`"))?;
    let attempt = u32::try_from(attempt)
        .ok()
        .filter(|attempt| *attempt >= 1)
        .ok_or_else(|| invalid_request("`attempt` is one-based and fits a u32"))?;
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_request("the envelope names no `model`"))?;
    let state = payload
        .get("state")
        .cloned()
        .ok_or_else(|| invalid_request("the envelope names no `state`"))?;
    let questions = payload
        .get("questions")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| invalid_request("the envelope names no `questions` object"))?;
    let body = RequestBody {
        request: request.to_owned(),
        attempt,
        model: model.to_owned(),
        state,
        questions,
        deadline: match payload.get("deadline") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_u64()
                    .ok_or_else(|| invalid_request("`deadline` is not an integer"))?,
            ),
        },
    };
    body.validate()?;
    Ok(body)
}

/// The request envelope's question checks: count, ids, per-question size,
/// and the `type`/`criteria` shape the SDK holds a raw question to.
fn check_questions(questions: &Map<String, Value>) -> Result<(), DecisionError> {
    if questions.is_empty() {
        return Err(invalid_request("a request asks at least one question"));
    }
    if questions.len() > MAX_QUESTIONS {
        return Err(DecisionError::TooManyQuestions {
            count: questions.len(),
        });
    }
    for (id, question) in questions {
        if id.is_empty() || id.len() > MAX_QUESTION_ID_BYTES {
            return Err(invalid_request(&format!(
                "a question id must contain 1 to {MAX_QUESTION_ID_BYTES} bytes"
            )));
        }
        if serialized_len(question) > MAX_QUESTION_BYTES {
            return Err(invalid_request(&format!(
                "question `{id}` serializes past {MAX_QUESTION_BYTES} bytes"
            )));
        }
        let kind = question
            .get("type")
            .and_then(Value::as_str)
            .filter(|kind| !kind.is_empty())
            .ok_or_else(|| invalid_request(&format!("question `{id}` names a nonempty `type`")))?;
        match kind {
            "choice" => {
                let options = question
                    .get("criteria")
                    .and_then(Value::as_object)
                    .ok_or_else(|| {
                        invalid_request(&format!("a choice question `{id}` names `criteria`"))
                    })?;
                if options.len() > MAX_CHOICE_OPTIONS {
                    return Err(DecisionError::TooManyOptions {
                        question: id.clone(),
                        count: options.len(),
                    });
                }
            }
            "score" => {
                let levels = question
                    .get("criteria")
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        invalid_request(&format!("a score question `{id}` names `criteria`"))
                    })?;
                if levels.len() < MIN_SCORE_LEVELS {
                    return Err(invalid_request(&format!(
                        "a score question `{id}` names at least {MIN_SCORE_LEVELS} levels"
                    )));
                }
                if levels.len() > MAX_SCORE_LEVELS {
                    return Err(DecisionError::TooManyOptions {
                        question: id.clone(),
                        count: levels.len(),
                    });
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Parse and validate a `type: "status"` payload.
fn parse_status(payload: &Value) -> Result<StatusPayload, DecisionError> {
    let (request, attempt) = correlation_fields(payload)?;
    let status = Status::parse(
        payload
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
    .ok_or_else(|| malformed("a status payload names `queued`, `processing`, or `error`"))?;
    let refusal = if status == Status::Error {
        Some(Refusal::parse(payload, "a status error")?)
    } else {
        None
    };
    Ok(StatusPayload {
        request,
        attempt,
        status,
        refusal,
    })
}

/// Parse and validate a `type: "result"` payload.
fn parse_result(payload: &Value) -> Result<ResultPayload, DecisionError> {
    let (request, attempt) = correlation_fields(payload)?;
    let outcome = Outcome::parse(
        payload
            .get("outcome")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
    .ok_or_else(|| malformed("a result names a known `outcome`"))?;
    let receipt = payload
        .get("receipt")
        .cloned()
        .ok_or_else(|| malformed("a result carries a `receipt`"))?;
    check_receipt(&receipt)?;
    // The receipt's own correlation fields agree with the envelope's, or
    // the worker is describing a different job than it answered.
    if receipt.get("request").and_then(Value::as_str) != Some(request.as_str())
        || receipt.get("attempt").and_then(Value::as_u64) != Some(u64::from(attempt))
    {
        return Err(malformed(
            "the receipt's `request`/`attempt` does not match the result's",
        ));
    }
    let (response, refusal) = if outcome == Outcome::Answered {
        let response = payload
            .get("response")
            .filter(|response| response.is_object())
            .cloned()
            .ok_or_else(|| malformed("an answered result carries a `response` object"))?;
        (Some(response), None)
    } else {
        let error = payload
            .get("error")
            .filter(|error| error.is_object())
            .ok_or_else(|| malformed("a non-answered result carries an `error` object"))?;
        (None, Some(Refusal::parse(error, "a result error")?))
    };
    Ok(ResultPayload {
        request,
        attempt,
        outcome,
        response,
        refusal,
        receipt,
    })
}

/// The `request`/`attempt` pair a status or result names, shape-checked.
fn correlation_fields(payload: &Value) -> Result<(String, u32), DecisionError> {
    let request = payload
        .get("request")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("the payload names no `request`"))?;
    bounded_str(request, MAX_REQUEST_ID_BYTES, "request").map_err(|r| malformed(&r))?;
    if request.is_empty() {
        return Err(malformed("the payload names an empty `request`"));
    }
    let attempt = payload
        .get("attempt")
        .and_then(Value::as_u64)
        .and_then(|attempt| u32::try_from(attempt).ok())
        .filter(|attempt| *attempt >= 1)
        .ok_or_else(|| malformed("`attempt` is one-based and fits a u32"))?;
    Ok((request.to_owned(), attempt))
}

/// The shallow check this layer runs on an embedded receipt: object form
/// and the schema tag. The seal and the remaining fields are
/// `receipts::execution`'s to verify.
fn check_receipt(receipt: &Value) -> Result<(), DecisionError> {
    if !receipt.is_object() {
        return Err(malformed("the `receipt` is not an object"));
    }
    match receipt.get("v").and_then(Value::as_str) {
        Some(RECEIPT_SCHEMA) => Ok(()),
        found => Err(malformed(&format!(
            "the receipt's `v` is {}, not {RECEIPT_SCHEMA}",
            found.unwrap_or("<absent>")
        ))),
    }
}

/// A logical `request` id: nonempty, bounded, and visible ASCII, so it is
/// safe to quote in a tag, a log line, or a ledger key.
fn check_identifier(value: &str, field: &str) -> Result<(), DecisionError> {
    identifier_reason(value, field).map_err(|reason| invalid_request(&reason))
}

/// The same check, as a reason — for the callers that refuse `malformed`
/// rather than `invalid_request`.
fn identifier_reason(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("the envelope names no `{field}`"));
    }
    bounded_str(value, MAX_REQUEST_ID_BYTES, field)?;
    if !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Err(format!("`{field}` must be visible ASCII"));
    }
    Ok(())
}

/// A `pubkey`-shaped field's hex check — `p` tag values, worker and
/// customer keys arrive as strings.
fn event_id(value: &str) -> Result<(), String> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err("an event id is 64 lowercase hexadecimal characters".to_owned())
    }
}

/// A string field within `max` bytes.
fn bounded_str(value: &str, max: usize, field: &str) -> Result<(), String> {
    if value.len() > max {
        Err(format!(
            "`{field}` is {} bytes, more than {max}",
            value.len()
        ))
    } else {
        Ok(())
    }
}

/// An optional bounded string field.
fn optional_str(
    value: &Value,
    field: &str,
    max: usize,
    where_is: &str,
) -> Result<Option<String>, DecisionError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let text = v
                .as_str()
                .ok_or_else(|| malformed(&format!("{where_is} `{field}` is not a string")))?;
            bounded_str(text, max, field).map_err(|r| malformed(&r))?;
            Ok(Some(text.to_owned()))
        }
    }
}

fn malformed(reason: &str) -> DecisionError {
    DecisionError::Malformed {
        reason: reason.to_owned(),
    }
}

fn invalid_request(reason: &str) -> DecisionError {
    DecisionError::InvalidRequest {
        reason: reason.to_owned(),
    }
}

fn serialized_len(value: &Value) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len())
}

/// Digest a value's canonical form as `sha256:<hex>`.
fn digest_canonical(value: &Value) -> String {
    let digest = Sha256::digest(canonicalize(value).as_bytes());
    let mut out = String::from("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Canonical JSON: keys sorted, whitespace gone — the canonicalization
/// `receipts::execution` digests by, reimplemented because this crate
/// carries no workspace dependencies. The keys are sorted rather than
/// trusted to the map: `preserve_order` makes a `serde_json` map
/// insertion-ordered whenever a sibling crate enables it, and digest
/// agreement must not depend on who wrote the bytes.
fn canonicalize(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("a key serializes"));
                out.push(':');
                out.push_str(&canonicalize(&map[*key]));
            }
            out.push('}');
            out
        }
        Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&canonicalize(item));
            }
            out.push(']');
            out
        }
        other => serde_json::to_string(other).expect("a value serializes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::{Keypair, Secp256k1};

    const NOW: u64 = 1_800_000_000;
    const NONCE: [u8; 32] = [7u8; 32];

    /// A throwaway identity derived from a label: generated in the test,
    /// valid only here, and never a fixture secret.
    fn identity(label: &str) -> (SecretKey, String, RelaySigner) {
        let secret = SecretKey::from_byte_array(Sha256::digest(label.as_bytes()).into()).unwrap();
        let keypair = Keypair::from_secret_key(&Secp256k1::new(), &secret);
        let pubkey = keypair.x_only_public_key().0.to_string();
        let signer = RelaySigner::from_secret_hex(&hex(&secret.secret_bytes())).unwrap();
        (secret, pubkey, signer)
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn conversation(secret: &SecretKey, peer_hex: &str) -> [u8; 32] {
        let peer: XOnlyPublicKey = peer_hex.parse().unwrap();
        nip44::conversation_key(secret, &peer)
    }

    fn noul() -> Value {
        json!({"type": "noul", "instructions": "Does the customer ask for money back?"})
    }

    fn body() -> RequestBody {
        let mut questions = Map::new();
        questions.insert("refund".to_owned(), noul());
        RequestBody::new(
            "req-9f4c2a",
            1,
            "shared-kev",
            json!("I was charged twice on the March invoice."),
            questions,
        )
    }

    struct Ends {
        caller: (SecretKey, String, RelaySigner),
        worker: (SecretKey, String, RelaySigner),
    }

    fn ends() -> Ends {
        Ends {
            caller: identity("decision-test-caller"),
            worker: identity("decision-test-worker"),
        }
    }

    impl Ends {
        fn caller_conversation(&self) -> [u8; 32] {
            conversation(&self.caller.0, &self.worker.1)
        }

        fn worker_conversation(&self) -> [u8; 32] {
            conversation(&self.worker.0, &self.caller.1)
        }

        fn caller_seal(&self) -> Seal<'_> {
            Seal {
                signer: &self.caller.2,
                conversation: self.caller_conversation(),
                nonce: NONCE,
                created_at: NOW,
            }
        }

        fn caller_seal_at(&self, created_at: u64, nonce: [u8; 32]) -> Seal<'_> {
            Seal {
                created_at,
                nonce,
                ..self.caller_seal()
            }
        }

        fn worker_seal(&self) -> Seal<'_> {
            Seal {
                signer: &self.worker.2,
                conversation: self.worker_conversation(),
                nonce: NONCE,
                created_at: NOW,
            }
        }

        /// A signed, encrypted request from the caller to the worker.
        fn request(&self, body: &RequestBody) -> Event {
            request_event(self.caller_seal(), body, &self.worker.1).unwrap()
        }

        fn admit(&self, event: &Event) -> Result<Admitted, DecisionError> {
            admit(
                event,
                &self.worker.1,
                &self.worker.0,
                NOW,
                RequestWindow::DEFAULT,
            )
        }

        fn pending<'a>(&'a self, request: &'a Event) -> Pending<'a> {
            Pending {
                attempt_id: &request.id,
                worker: &self.worker.1,
                customer: &self.caller.1,
                request: "req-9f4c2a",
                attempt: 1,
            }
        }

        /// Sign a worker→caller event with arbitrary kind/tags/payload.
        fn answer(&self, kind: u16, tags: Vec<Tag>, payload: &Value) -> Event {
            self.worker_seal().event(kind, tags, payload).unwrap()
        }

        /// A properly bound status event for `request`.
        fn status(&self, request: &Event, payload: &Value) -> Event {
            self.answer(
                FEEDBACK_KIND,
                vec![
                    Tag::new(vec!["e".into(), request.id.clone()]),
                    Tag::new(vec!["p".into(), self.caller.1.clone()]),
                ],
                payload,
            )
        }

        /// A properly bound result event for `request`.
        fn result(&self, request: &Event, payload: &Value) -> Event {
            self.answer(
                RESULT_KIND,
                vec![
                    Tag::new(vec!["e".into(), request.id.clone()]),
                    Tag::new(vec!["p".into(), self.caller.1.clone()]),
                ],
                payload,
            )
        }
    }

    fn receipt(request: &str, attempt: u32) -> Value {
        json!({
            "v": RECEIPT_SCHEMA,
            "request": request,
            "attempt": attempt,
            "transport": "relay",
            "digest": format!("sha256:{}", "4".repeat(64)),
        })
    }

    fn result_value() -> Value {
        result_payload(
            "req-9f4c2a",
            1,
            &Resolution::Answered(json!({
                "model": "shared-kev",
                "answers": {"refund": {"type": "noul", "noul": 0.91}},
                "usage": {"input_tokens": 412, "output_tokens": 2},
            })),
            &receipt("req-9f4c2a", 1),
        )
        .unwrap()
    }

    // ---- admission, worker side ----

    #[test]
    fn a_signed_request_is_admitted() {
        let ends = ends();
        let request = ends.request(&body());
        let Admitted::Call(call) = ends.admit(&request).unwrap() else {
            panic!("a systemone request admits as a call");
        };
        assert_eq!(call.principal, ends.caller.1);
        assert_eq!(call.attempt_id, request.id);
        assert_eq!(call.body.request, "req-9f4c2a");
        assert_eq!(call.body.attempt, 1);
        assert_eq!(call.body.model, "shared-kev");
        assert_eq!(call.body.questions.len(), 1);
        assert_eq!(call.request_digest, body().digest());
        assert!(call.request_digest.starts_with("sha256:"));
    }

    #[test]
    fn a_request_on_the_conversation_kind_is_not_deliverable() {
        let ends = ends();
        let mut request = ends.request(&body());
        request.kind = 25_900;
        assert!(matches!(
            ends.admit(&request),
            Err(DecisionError::UnexpectedKind { kind: 25_900 })
        ));
    }

    #[test]
    fn a_request_addressed_elsewhere_is_not_this_workers() {
        let ends = ends();
        let other = identity("decision-test-other-worker");
        let request = request_event(ends.caller_seal(), &body(), &other.1).unwrap();
        assert!(matches!(
            ends.admit(&request),
            Err(DecisionError::NotAddressed)
        ));
    }

    #[test]
    fn a_forged_signature_is_not_answerable() {
        let ends = ends();
        let mut request = ends.request(&body());
        // A relay can rewrite an event but cannot sign it: the id no
        // longer covers the fields.
        request.created_at += 1;
        let error = ends.admit(&request).unwrap_err();
        assert!(matches!(error, DecisionError::Event(_)));
        assert!(!error.answerable());
    }

    #[test]
    fn an_old_request_is_refused_stale() {
        let ends = ends();
        let request = request_event(
            ends.caller_seal_at(NOW - 11 * 60, NONCE),
            &body(),
            &ends.worker.1,
        )
        .unwrap();
        let error = ends.admit(&request).unwrap_err();
        assert!(matches!(error, DecisionError::Stale { .. }));
        assert_eq!(error.code(), Some("stale"));
    }

    #[test]
    fn a_request_too_far_ahead_is_refused_stale() {
        let ends = ends();
        let request = request_event(
            ends.caller_seal_at(NOW + 10 * 60, NONCE),
            &body(),
            &ends.worker.1,
        )
        .unwrap();
        assert!(matches!(
            ends.admit(&request),
            Err(DecisionError::Stale { .. })
        ));
    }

    #[test]
    fn an_expired_request_is_refused_stale() {
        let ends = ends();
        // An expiration tag that already passed is stale even inside the
        // created_at window.
        let body = body().deadline(NOW - 1);
        let request = ends.request(&body);
        assert!(matches!(
            ends.admit(&request),
            Err(DecisionError::Stale { .. } | DecisionError::DeadlinePassed { .. })
        ));
    }

    #[test]
    fn a_passed_deadline_is_refused_stale() {
        let ends = ends();
        // The event has no expiration tag here; the payload deadline
        // alone produces the refusal.
        let mut event = ends.request(&body().deadline(NOW + 60));
        event.tags.retain(|tag| tag.name() != Some("expiration"));
        let body = body().deadline(NOW - 1);
        let payload = ends
            .caller_seal()
            .event(REQUEST_KIND, event.tags.clone(), &body.payload())
            .unwrap();
        let error = ends.admit(&payload).unwrap_err();
        assert!(matches!(error, DecisionError::DeadlinePassed { .. }));
        assert_eq!(error.code(), Some("stale"));
    }

    #[test]
    fn an_integer_version_is_unsupported() {
        let ends = ends();
        // A conversation-family payload under the decision kind: `2` is
        // not "openagents.systemone.v1", so it can never reinterpret.
        let payload = json!({"v": 2, "type": "systemone"});
        let request = ends
            .caller_seal()
            .event(
                REQUEST_KIND,
                vec![Tag::new(vec!["p".into(), ends.worker.1.clone()])],
                &payload,
            )
            .unwrap();
        let error = ends.admit(&request).unwrap_err();
        assert!(matches!(error, DecisionError::UnsupportedVersion { .. }));
        assert_eq!(error.code(), Some("unsupported_version"));
    }

    #[test]
    fn an_unknown_version_string_is_unsupported() {
        let ends = ends();
        let payload = json!({"v": "openagents.systemone.v9", "type": "systemone"});
        let request = ends
            .caller_seal()
            .event(
                REQUEST_KIND,
                vec![Tag::new(vec!["p".into(), ends.worker.1.clone()])],
                &payload,
            )
            .unwrap();
        assert!(matches!(
            ends.admit(&request),
            Err(DecisionError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn an_unknown_type_is_malformed() {
        let ends = ends();
        let payload = json!({"v": SCHEMA, "type": "status", "status": "processing"});
        let request = ends
            .caller_seal()
            .event(
                REQUEST_KIND,
                vec![Tag::new(vec!["p".into(), ends.worker.1.clone()])],
                &payload,
            )
            .unwrap();
        let error = ends.admit(&request).unwrap_err();
        assert!(matches!(error, DecisionError::Malformed { .. }));
        assert_eq!(error.code(), Some("malformed"));
    }

    #[test]
    fn an_undecryptable_or_unreadable_payload_is_malformed() {
        let ends = ends();
        let other = identity("decision-test-third");
        // Signed by the caller but encrypted to a different key — the
        // worker cannot read it.
        let payload = body().payload();
        let request = Seal {
            conversation: conversation(&ends.caller.0, &other.1),
            ..ends.caller_seal()
        }
        .event(
            REQUEST_KIND,
            vec![Tag::new(vec!["p".into(), ends.worker.1.clone()])],
            &payload,
        )
        .unwrap();
        let error = ends.admit(&request).unwrap_err();
        assert!(matches!(error, DecisionError::Malformed { .. }));
    }

    #[test]
    fn a_missing_envelope_field_is_an_invalid_request() {
        let ends = ends();
        let payload = json!({
            "v": SCHEMA,
            "type": "systemone",
            "request": "req-9f4c2a",
            "attempt": 1,
            "state": "x",
            "questions": {"q": {"type": "noul"}},
        });
        let request = ends
            .caller_seal()
            .event(
                REQUEST_KIND,
                vec![Tag::new(vec!["p".into(), ends.worker.1.clone()])],
                &payload,
            )
            .unwrap();
        let error = ends.admit(&request).unwrap_err();
        assert!(matches!(error, DecisionError::InvalidRequest { .. }));
        assert_eq!(error.code(), Some("invalid_request"));
    }

    #[test]
    fn a_payload_carrying_a_credential_is_refused_malformed() {
        let ends = ends();
        let mut payload = body().payload();
        payload["key"] = json!("oak_deadbeef.not-a-real-secret");
        let request = ends
            .caller_seal()
            .event(
                REQUEST_KIND,
                vec![Tag::new(vec!["p".into(), ends.worker.1.clone()])],
                &payload,
            )
            .unwrap();
        let error = ends.admit(&request).unwrap_err();
        assert!(matches!(error, DecisionError::Credential { .. }));
        assert_eq!(error.code(), Some("malformed"));
    }

    #[test]
    fn too_many_questions_is_its_own_refusal() {
        let ends = ends();
        let mut questions = Map::new();
        for index in 0..=MAX_QUESTIONS {
            questions.insert(format!("q{index}"), noul());
        }
        let mut body = body();
        body.questions = questions;
        let request = ends
            .caller_seal()
            .event(
                REQUEST_KIND,
                vec![Tag::new(vec!["p".into(), ends.worker.1.clone()])],
                &body.payload(),
            )
            .unwrap();
        let error = ends.admit(&request).unwrap_err();
        assert!(matches!(error, DecisionError::TooManyQuestions { .. }));
        assert_eq!(error.code(), Some("too_many_questions"));
    }

    #[test]
    fn too_many_options_is_its_own_refusal() {
        let ends = ends();
        let criteria: Map<String, Value> = (0..=MAX_CHOICE_OPTIONS)
            .map(|index| (format!("o{index}"), Value::Null))
            .collect();
        let mut questions = Map::new();
        questions.insert(
            "pick".to_owned(),
            json!({"type": "choice", "criteria": criteria}),
        );
        let mut body = body();
        body.questions = questions;
        let request = ends
            .caller_seal()
            .event(
                REQUEST_KIND,
                vec![Tag::new(vec!["p".into(), ends.worker.1.clone()])],
                &body.payload(),
            )
            .unwrap();
        let error = ends.admit(&request).unwrap_err();
        assert!(matches!(error, DecisionError::TooManyOptions { .. }));
        assert_eq!(error.code(), Some("too_many_options"));
    }

    // ---- cancellation ----

    #[test]
    fn a_cancel_from_the_original_caller_authorizes() {
        let ends = ends();
        let request = ends.request(&body());
        let cancel = cancel_event(
            ends.caller_seal(),
            &ends.worker.1,
            "req-9f4c2a",
            &request.id,
        )
        .unwrap();
        let Admitted::Cancel(cancel) = ends.admit(&cancel).unwrap() else {
            panic!("a cancel payload admits as a cancellation");
        };
        assert_eq!(cancel.target, request.id);
        assert_eq!(cancel.request, "req-9f4c2a");
        assert!(cancel.authorizes(&ends.caller.1));
        cancel.authorize(&ends.caller.1).unwrap();
    }

    #[test]
    fn a_cancel_from_another_principal_authorizes_nothing() {
        let ends = ends();
        let request = ends.request(&body());
        let stranger = identity("decision-test-stranger");
        let stranger_seal = Seal {
            signer: &stranger.2,
            conversation: conversation(&stranger.0, &ends.worker.1),
            nonce: NONCE,
            created_at: NOW,
        };
        let cancel =
            cancel_event(stranger_seal, &ends.worker.1, "req-9f4c2a", &request.id).unwrap();
        // The cancel is a valid, addressed event — it is just not the
        // original caller's.
        let Admitted::Cancel(cancel) = ends.admit(&cancel).unwrap() else {
            panic!("a signed cancel still admits for inspection");
        };
        assert!(!cancel.authorizes(&ends.caller.1));
        assert!(matches!(
            cancel.authorize(&ends.caller.1),
            Err(DecisionError::WrongPrincipal)
        ));
    }

    #[test]
    fn a_cancel_without_a_target_is_malformed() {
        let ends = ends();
        let payload = cancel_payload("req-9f4c2a");
        // Signed and addressed, but e-tagging nothing: it names a request
        // it cannot point at.
        let cancel = ends
            .caller_seal()
            .event(
                REQUEST_KIND,
                vec![Tag::new(vec!["p".into(), ends.worker.1.clone()])],
                &payload,
            )
            .unwrap();
        assert!(matches!(
            ends.admit(&cancel),
            Err(DecisionError::Malformed { .. })
        ));
    }

    // ---- answers, caller side ----

    #[test]
    fn a_signed_status_is_bound_to_the_job() {
        let ends = ends();
        let request = ends.request(&body());
        let status = ends.status(
            &request,
            &status_payload("req-9f4c2a", 1, Status::Processing),
        );
        let Answer::Status(status) =
            bind_answer(&status, &ends.pending(&request), &ends.caller.0).unwrap()
        else {
            panic!("a processing status binds as status");
        };
        assert_eq!(status.status, Status::Processing);
        assert_eq!(status.request, "req-9f4c2a");
        assert_eq!(status.attempt, 1);
    }

    #[test]
    fn a_signed_result_is_bound_to_the_job() {
        let ends = ends();
        let request = ends.request(&body());
        let result = ends.result(&request, &result_value());
        let Answer::Result(result) =
            bind_answer(&result, &ends.pending(&request), &ends.caller.0).unwrap()
        else {
            panic!("a result binds as a result");
        };
        assert_eq!(result.outcome, Outcome::Answered);
        assert!(result.response.is_some());
        assert_eq!(result.receipt["transport"], json!("relay"));
    }

    #[test]
    fn a_typed_refusal_status_is_bound() {
        let ends = ends();
        let request = ends.request(&body());
        let refusal = Refusal::new("quota_exhausted")
            .message("daily input budget spent")
            .retry_after_ms(3_600_000);
        let status = ends.status(&request, &refusal_payload("req-9f4c2a", 1, &refusal));
        let Answer::Status(status) =
            bind_answer(&status, &ends.pending(&request), &ends.caller.0).unwrap()
        else {
            panic!("a refusal binds as status");
        };
        assert_eq!(status.status, Status::Error);
        let refusal = status.refusal.unwrap();
        assert_eq!(refusal.code, "quota_exhausted");
        assert_eq!(refusal.retry_after_ms, Some(3_600_000));
    }

    #[test]
    fn an_answer_from_the_wrong_signer_is_unbound() {
        let ends = ends();
        let request = ends.request(&body());
        let stranger = identity("decision-test-stranger");
        // A stranger's honestly signed result: every field right but the
        // signer.
        let result = Seal {
            signer: &stranger.2,
            conversation: conversation(&stranger.0, &ends.caller.1),
            nonce: NONCE,
            created_at: NOW,
        }
        .event(
            RESULT_KIND,
            vec![
                Tag::new(vec!["e".into(), request.id.clone()]),
                Tag::new(vec!["p".into(), ends.caller.1.clone()]),
            ],
            &result_value(),
        )
        .unwrap();
        assert!(matches!(
            bind_answer(&result, &ends.pending(&request), &ends.caller.0),
            Err(DecisionError::Unbound { field: "pubkey" })
        ));
    }

    #[test]
    fn an_answer_on_the_conversation_kind_is_not_deliverable() {
        let ends = ends();
        let request = ends.request(&body());
        let result = ends.result(&request, &result_value());
        let mut relabeled = result.clone();
        relabeled.kind = 26_900;
        assert!(matches!(
            bind_answer(&relabeled, &ends.pending(&request), &ends.caller.0),
            Err(DecisionError::UnexpectedKind { kind: 26_900 })
        ));
    }

    #[test]
    fn an_answer_tagging_another_request_is_unbound() {
        let ends = ends();
        let request = ends.request(&body());
        // Signed by the worker, e-tagging a different job's event: a
        // correctly signed answer to an older job is not this job's. A
        // different nonce gives the other request its own event id.
        let other_request =
            request_event(ends.caller_seal_at(NOW, [9u8; 32]), &body(), &ends.worker.1).unwrap();
        let result = ends.result(&other_request, &result_value());
        assert!(matches!(
            bind_answer(&result, &ends.pending(&request), &ends.caller.0),
            Err(DecisionError::Unbound { field: "e" })
        ));
    }

    #[test]
    fn an_answer_tagging_another_caller_is_unbound() {
        let ends = ends();
        let request = ends.request(&body());
        let other = identity("decision-test-other-customer");
        let result = ends.answer(
            RESULT_KIND,
            vec![
                Tag::new(vec!["e".into(), request.id.clone()]),
                Tag::new(vec!["p".into(), other.1.clone()]),
            ],
            &result_value(),
        );
        assert!(matches!(
            bind_answer(&result, &ends.pending(&request), &ends.caller.0),
            Err(DecisionError::Unbound { field: "p" })
        ));
    }

    #[test]
    fn an_answer_naming_another_attempt_is_unbound() {
        let ends = ends();
        let request = ends.request(&body());
        // The worker's signature covers everything but the payload's own
        // attempt: a relabeled answer fails the payload check.
        let mut payload = result_value();
        payload["attempt"] = json!(2);
        let result = ends.result(&request, &payload);
        assert!(matches!(
            bind_answer(&result, &ends.pending(&request), &ends.caller.0),
            Err(DecisionError::Unbound { field: "attempt" })
        ));
    }

    #[test]
    fn an_answer_naming_another_request_id_is_unbound() {
        let ends = ends();
        let request = ends.request(&body());
        let mut payload = result_value();
        payload["request"] = json!("req-someone-else");
        let result = ends.result(&request, &payload);
        assert!(matches!(
            bind_answer(&result, &ends.pending(&request), &ends.caller.0),
            Err(DecisionError::Unbound { field: "request" })
        ));
    }

    #[test]
    fn an_answer_with_a_foreign_schema_is_unsupported() {
        let ends = ends();
        let request = ends.request(&body());
        let payload = json!({"v": "openagents.systemone.v0", "type": "result",
            "request": "req-9f4c2a", "attempt": 1, "outcome": "answered"});
        let result = ends.result(&request, &payload);
        assert!(matches!(
            bind_answer(&result, &ends.pending(&request), &ends.caller.0),
            Err(DecisionError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn a_result_without_a_receipt_is_malformed() {
        let ends = ends();
        let request = ends.request(&body());
        let payload = json!({"v": SCHEMA, "type": "result",
            "request": "req-9f4c2a", "attempt": 1, "outcome": "answered",
            "response": {"model": "shared-kev", "answers": {}}});
        let result = ends.result(&request, &payload);
        assert!(matches!(
            bind_answer(&result, &ends.pending(&request), &ends.caller.0),
            Err(DecisionError::Malformed { .. })
        ));
    }

    #[test]
    fn an_admitted_call_builds_a_bound_result() {
        let ends = ends();
        let request = ends.request(&body());
        let Admitted::Call(call) = ends.admit(&request).unwrap() else {
            panic!("admitted");
        };
        // The worker's reply builders produce events the caller binds
        // without copying protocol logic.
        let status = call
            .status_event(ends.worker_seal(), Status::Queued)
            .unwrap();
        let Answer::Status(status) =
            bind_answer(&status, &ends.pending(&request), &ends.caller.0).unwrap()
        else {
            panic!("the worker's status binds");
        };
        assert_eq!(status.status, Status::Queued);

        let result = call
            .result_event(
                ends.worker_seal(),
                &Resolution::Answered(json!({"model": "shared-kev", "answers": {}})),
                &receipt("req-9f4c2a", 1),
            )
            .unwrap();
        assert!(matches!(
            bind_answer(&result, &ends.pending(&request), &ends.caller.0),
            Ok(Answer::Result(_))
        ));
    }

    // ---- digest and scope ----

    #[test]
    fn a_request_digest_is_canonical() {
        let body = body();
        // The same envelope rebuilt with keys in another order digests
        // identically — canonicalization is the agreement.
        let mut reordered = Map::new();
        reordered.insert("refund".to_owned(), noul());
        let same = RequestBody::new(
            "req-9f4c2a",
            1,
            "shared-kev",
            json!("I was charged twice on the March invoice."),
            reordered,
        );
        assert_eq!(body.digest(), same.digest());
        // An absent deadline digests as null — never as a missing key,
        // which would change the hash.
        let explicit_null = digest_canonical(&json!({
            "attempt": 1, "deadline": null, "model": "shared-kev",
            "questions": body.questions, "request": "req-9f4c2a",
            "state": body.state,
        }));
        assert_eq!(body.digest(), explicit_null);
        assert_ne!(body.digest(), body.clone().deadline(NOW).digest());
    }

    #[test]
    fn an_idempotency_key_scopes_by_principal_and_tenant() {
        let key = idempotency_key("pubkey-a", Some("tenant-1"), "req-1", 1);
        assert!(key.starts_with("sha256:"));
        // Same tuple, same key — a retry is not a second spend.
        assert_eq!(
            key,
            idempotency_key("pubkey-a", Some("tenant-1"), "req-1", 1)
        );
        // Every element of the scope changes it.
        assert_ne!(
            key,
            idempotency_key("pubkey-b", Some("tenant-1"), "req-1", 1)
        );
        assert_ne!(
            key,
            idempotency_key("pubkey-a", Some("tenant-2"), "req-1", 1)
        );
        assert_ne!(key, idempotency_key("pubkey-a", None, "req-1", 1));
        assert_ne!(
            key,
            idempotency_key("pubkey-a", Some("tenant-1"), "req-2", 1)
        );
        assert_ne!(
            key,
            idempotency_key("pubkey-a", Some("tenant-1"), "req-1", 2)
        );
        // The request scope is the cancel's resolution space.
        assert_eq!(
            request_scope("pubkey-a", Some("tenant-1"), "req-1"),
            request_scope("pubkey-a", Some("tenant-1"), "req-1"),
        );
        assert_ne!(
            request_scope("pubkey-a", Some("tenant-1"), "req-1"),
            request_scope("pubkey-b", Some("tenant-1"), "req-1"),
        );
    }
}
