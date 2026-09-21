//! The execution receipt: one decision call, claimed.
//!
//! One receipt per attempt. A retried request produces a second receipt
//! under the same `request` id with a later `attempt` — the attempt chain
//! is how a caller reconstructs what a timeout or a retry actually did,
//! and settlement counts receipts, not calls imagined.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The schema tag an execution receipt carries.
pub const SCHEMA: &str = "openagents.receipt.execution.v1";

/// What the call came to.
///
/// The names match the row semantics the gym already uses, so an outcome
/// means the same thing whether a receipt or a recorded row carries it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The door produced an answer.
    Answered,
    /// The door declined — a refusal is a recorded outcome, not an absence.
    Refused,
    /// Transport or capacity denied the call before the door decided:
    /// timeout, overload, dead door. The attempt is named, and its cause.
    Unavailable,
    /// The attempt was admitted but never dispatched — the reservation
    /// was cancelled or the caller walked away. Never scored as an error.
    Unattempted,
    /// The service cannot say what happened — a crash after dispatch, a
    /// lost response. Explicit, because guessing would be a claim.
    Unknown,
}

/// The identity side of a call: what was asked for, or what answered.
///
/// The same shape holds the caller's `requested` identity and the
/// operation's `served` identity, so a drifted door or a substituted
/// artifact is a field-level comparison, not a paragraph of prose.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Served {
    /// The model id named or reported.
    pub model: String,
    /// The adapter package, when one applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// The content digest of the artifact, when the side knows one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub artifact_signature: String,
    /// The execution settings, when the side knows them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub execution: BTreeMap<String, String>,
}

/// The timing a call carried, each phase named separately.
///
/// Queue time and execution time are different measurements: a relay call
/// that waited forty minutes in a queue and answered in ninety
/// milliseconds is not a ninety-millisecond call, and the receipt keeps
/// both.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Timing {
    /// Milliseconds between admission and dispatch — queue and forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_ms: Option<u64>,
    /// Milliseconds the operation itself took.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    /// When the attempt resolved, as RFC 3339 in UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
}

/// The spend lane a call belongs to.
///
/// Decision calls, generation calls, and external-executor jobs are
/// different expenses on the same ledger: a caller that can only say
/// "the run cost this much" cannot separate what a review decided from
/// what a delegate did. `Absent` is not a variant — a receipt that does
/// not know its lane carries no `lane` field, and no reader upgrades a
/// missing lane into a guessed one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lane {
    /// A System One decision call — routing, judging, selecting.
    Decision,
    /// A reply or plan generation call.
    Generation,
    /// An external executor or delegate job.
    Executor,
}

/// The evaluation context a call carried, when it carried one.
///
/// Ordinary inference has none of this, and the receipt does not invent
/// it — a production call is not a measurement. When a declared
/// evaluation supplies the context, these references tie the call to the
/// suite, question set, gate, and item it was part of, and the gym row
/// that records it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Evaluation {
    /// The suite the call belonged to, by name.
    pub suite: String,
    /// The suite's content digest.
    pub suite_digest: String,
    /// The question set's digest, when the call pinned one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_digest: Option<String>,
    /// The gate's digest, when the call pinned one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_digest: Option<String>,
    /// The item the call evaluated, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    /// The item's partition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// The receipt itself: one attempt at one call.
///
/// `digest` covers every field but itself, canonicalized key-sorted JSON
/// over SHA-256 — the same self-verifying shape the report commitment and
/// the tenancy manifest use. A reader recomputes it before believing the
/// document; a tampered receipt refuses on load.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutionReceipt {
    /// The schema tag.
    pub v: String,
    /// The logical request this attempt belongs to. One request's retries
    /// share it; the attempt chain is the retry's history.
    pub request: String,
    /// Which attempt this receipt records, one-based.
    pub attempt: u32,
    /// This attempt's own identity.
    pub attempt_id: String,
    /// The transport that carried the call: `http`, `relay`, or another
    /// named lane. Transport stays in the receipt because a relay call's
    /// queue time and failure causes are not a direct call's.
    pub transport: String,
    /// The authorized tenant reference — the key id or digest the
    /// authentication layer resolved, never the credential itself. Absent
    /// for an anonymous call to a shared door.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    /// The registry revision the call was admitted under — digest and
    /// sequence, so which binding authorized the call is a lookup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<Registry>,
    /// The identity the caller asked for.
    pub requested: Served,
    /// The identity that actually answered — or that was bound when the
    /// attempt failed before an answer. Artifact identity binds to the
    /// operation here, so a mid-request model change is detectable
    /// without trusting a later `GET /v1/models`.
    pub served: Served,
    /// What the attempt came to.
    pub outcome: Outcome,
    /// The refusal or failure cause, when the outcome carries one — the
    /// door's refusal code, or the transport's reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    /// The timing the attempt carried.
    #[serde(default, skip_serializing_if = "Timing::is_empty")]
    pub timing: Timing,
    /// Digest of the canonical request envelope — the call this attempt
    /// served, bound without carrying its content.
    pub request_digest: String,
    /// Digest of the response body this attempt produced, when it
    /// produced one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_digest: Option<String>,
    /// The accounting reference this attempt settled against — the quota
    /// reservation or usage record id — when the service keeps one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
    /// The spend lane this attempt belongs to, when anyone recorded it.
    /// Absent stays absent — a lane nobody wrote down is an unknown
    /// lane, not a decision call by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<Lane>,
    /// The attempt this attempt revises, when it is a review or a
    /// fallback rather than a first call — the original attempt's own
    /// id. A revision names its original so the chain is the record's,
    /// not a reader's guess; an original carries nothing here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revises: Option<String>,
    /// The evaluation context, when the call carried one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<Evaluation>,
    /// The digest over every field above.
    pub digest: String,
}

/// The registry revision a call was admitted under.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Registry {
    /// The manifest's digest.
    pub digest: String,
    /// The manifest's sequence number.
    pub sequence: u64,
}

impl Timing {
    fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// Why a receipt failed to load or seal.
#[derive(Debug)]
pub enum ReceiptError {
    /// The document did not parse.
    Malformed(String),
    /// The schema tag is not this version's.
    UnknownSchema(String),
    /// The digest does not recompute over the contents.
    Tampered,
    /// A required field was empty — a receipt with no request identity or
    /// no request digest binds nothing.
    Missing(&'static str),
}

impl std::fmt::Display for ReceiptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(error) => write!(f, "malformed receipt: {error}"),
            Self::UnknownSchema(schema) => {
                write!(f, "receipt schema `{schema}` is not `{SCHEMA}`")
            }
            Self::Tampered => write!(
                f,
                "the receipt's digest does not recompute over its contents"
            ),
            Self::Missing(field) => write!(f, "the receipt names no {field}"),
        }
    }
}

impl std::error::Error for ReceiptError {}

impl ExecutionReceipt {
    /// Begin a receipt for one attempt.
    ///
    /// `request` is the caller-facing request id, `attempt` the one-based
    /// attempt number, and `request_digest` the digest of the canonical
    /// request envelope — the caller computes it over its own envelope, so
    /// private content never enters the receipt.
    #[must_use]
    pub fn for_attempt(
        transport: impl Into<String>,
        request: impl Into<String>,
        attempt: u32,
        request_digest: impl Into<String>,
    ) -> Self {
        let request = request.into();
        Self {
            v: SCHEMA.to_string(),
            request,
            attempt,
            attempt_id: String::new(),
            transport: transport.into(),
            tenant: None,
            registry: None,
            requested: Served::default(),
            served: Served::default(),
            outcome: Outcome::Unknown,
            cause: None,
            timing: Timing::default(),
            request_digest: request_digest.into(),
            result_digest: None,
            usage: None,
            lane: None,
            revises: None,
            evaluation: None,
            digest: String::new(),
        }
    }

    /// Fill in `digest` over the receipt's other fields.
    pub fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// The digest over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a receipt serializes");
        value
            .as_object_mut()
            .expect("a receipt is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Serialize the sealed receipt as canonical JSON — the bytes whose
    /// digest a verifier recomputes.
    pub fn to_json(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a receipt serializes");
        // Keep the digest in place: the document as written, not only the
        // covered fields.
        value
            .as_object_mut()
            .expect("a receipt is an object")
            .entry("digest")
            .or_insert_with(|| Value::String(self.digest.clone()));
        serde_json::to_string_pretty(&value).expect("a receipt serializes")
    }

    /// Read and self-check a receipt's text.
    pub fn parse(text: &str) -> Result<Self, ReceiptError> {
        let receipt: Self = serde_json::from_str(text)
            .map_err(|error| ReceiptError::Malformed(error.to_string()))?;
        receipt.verify()?;
        Ok(receipt)
    }

    /// The checks [`ExecutionReceipt::parse`] runs: schema, digest, and
    /// the fields without which the receipt binds nothing.
    pub fn verify(&self) -> Result<(), ReceiptError> {
        if self.v != SCHEMA {
            return Err(ReceiptError::UnknownSchema(self.v.clone()));
        }
        if self.digest != self.compute_digest() {
            return Err(ReceiptError::Tampered);
        }
        if self.request.is_empty() {
            return Err(ReceiptError::Missing("request identity"));
        }
        if self.request_digest.is_empty() {
            return Err(ReceiptError::Missing("request digest"));
        }
        if self.attempt_id.is_empty() {
            return Err(ReceiptError::Missing("attempt identity"));
        }
        Ok(())
    }
}

/// Digest a request envelope's canonical form.
///
/// The caller and the service must agree on the envelope bytes — the
/// canonicalization is the same key-sorted JSON the suites, manifests, and
/// commitments digest by — so a receipt's `request_digest` is checkable by
/// whoever holds the request.
#[must_use]
pub fn digest_request(envelope: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonicalize(envelope).as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// Canonical JSON: keys sorted, whitespace gone. The same
/// canonicalization every digest in this workspace shares. The keys are
/// sorted here rather than trusted to the map: `preserve_order` makes a
/// `serde_json` map insertion-ordered whenever a sibling crate enables
/// it, and the digest agreement must not depend on who wrote the bytes.
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

    fn digest_of(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    /// A receipt the way an HTTP gateway writes one for an answered call.
    fn answered() -> ExecutionReceipt {
        let mut receipt = ExecutionReceipt::for_attempt("http", "req-1", 1, digest_of('q'));
        receipt.attempt_id = "att-1".to_string();
        receipt.tenant = Some("key-ref:acme/2026-09".to_string());
        receipt.registry = Some(Registry {
            digest: digest_of('r'),
            sequence: 3,
        });
        receipt.requested = Served {
            model: "kev-0.6b".to_string(),
            ..Served::default()
        };
        receipt.served = Served {
            model: "kev-0.6b".to_string(),
            artifact_signature: digest_of('a'),
            execution: [("dtype".to_string(), "bf16".to_string())]
                .into_iter()
                .collect(),
            ..Served::default()
        };
        receipt.outcome = Outcome::Answered;
        receipt.result_digest = Some(digest_of('s'));
        receipt.timing = Timing {
            queued_ms: Some(4),
            latency_ms: Some(87),
            resolved_at: Some("2026-09-21T00:00:00Z".to_string()),
        };
        receipt.usage = Some("resv-881".to_string());
        receipt.seal();
        receipt
    }

    #[test]
    fn a_sealed_receipt_round_trips_and_self_verifies() {
        let receipt = answered();
        let parsed = ExecutionReceipt::parse(&receipt.to_json()).unwrap();
        assert_eq!(parsed.digest, receipt.digest);
        assert_eq!(parsed.outcome, Outcome::Answered);
        assert_eq!(parsed.served.artifact_signature, digest_of('a'));
    }

    #[test]
    fn a_tampered_receipt_refuses() {
        let receipt = answered();
        let text = receipt.to_json().replacen("kev-0.6b", "kev-4b   ", 1);
        assert!(matches!(
            ExecutionReceipt::parse(&text),
            Err(ReceiptError::Tampered)
        ));
    }

    #[test]
    fn an_unknown_schema_is_refused() {
        let mut receipt = answered();
        receipt.v = "openagents.receipt.execution.v0".to_string();
        receipt.seal();
        assert!(matches!(
            ExecutionReceipt::parse(&receipt.to_json()),
            Err(ReceiptError::UnknownSchema(_))
        ));
    }

    #[test]
    fn retries_share_the_request_and_name_their_attempts() {
        let mut timed_out = answered();
        timed_out.outcome = Outcome::Unavailable;
        timed_out.cause = Some("timeout".to_string());
        timed_out.result_digest = None;
        timed_out.seal();

        let mut second = answered();
        second.attempt = 2;
        second.attempt_id = "att-2".to_string();
        second.seal();

        assert_eq!(timed_out.request, second.request);
        assert_ne!(timed_out.attempt_id, second.attempt_id);
        assert_eq!(timed_out.outcome, Outcome::Unavailable);
        assert_eq!(second.outcome, Outcome::Answered);
        // Both verify independently — the chain is the caller's assembly,
        // each receipt stands alone.
        ExecutionReceipt::parse(&timed_out.to_json()).unwrap();
        ExecutionReceipt::parse(&second.to_json()).unwrap();
    }

    #[test]
    fn a_relay_receipt_keeps_its_transport_and_queue() {
        let mut receipt = answered();
        receipt.transport = "relay".to_string();
        receipt.timing.queued_ms = Some(2400);
        receipt.seal();
        let parsed = ExecutionReceipt::parse(&receipt.to_json()).unwrap();
        assert_eq!(parsed.transport, "relay");
        assert_eq!(parsed.timing.queued_ms, Some(2400));
        // A direct call and a relay call are the same shape with their
        // transports named — semantically verifiable, not identical.
        assert_ne!(parsed.transport, answered().transport);
    }

    #[test]
    fn an_evaluation_context_is_present_only_when_supplied() {
        let plain = answered();
        assert!(plain.evaluation.is_none());

        let mut measured = answered();
        measured.evaluation = Some(Evaluation {
            suite: "caller-v1".to_string(),
            suite_digest: digest_of('e'),
            question_digest: Some(digest_of('d')),
            gate_digest: Some(digest_of('g')),
            item: Some("routing/001".to_string()),
            partition: Some("development".to_string()),
        });
        measured.seal();
        let parsed = ExecutionReceipt::parse(&measured.to_json()).unwrap();
        let evaluation = parsed.evaluation.unwrap();
        assert_eq!(evaluation.item.as_deref(), Some("routing/001"));
        assert_eq!(evaluation.suite_digest, digest_of('e'));
    }

    #[test]
    fn a_substituted_artifact_is_a_field_level_difference() {
        let receipt = answered();
        let mut drifted = receipt.served.clone();
        drifted.artifact_signature = digest_of('f');
        // The comparison a caller or the registry runs: requested versus
        // served, bound into the operation, not a later /v1/models lookup.
        assert_ne!(receipt.served, drifted);
        assert_eq!(receipt.requested.model, receipt.served.model);
    }

    #[test]
    fn a_receipt_without_a_lane_keeps_none() {
        let receipt = answered();
        assert!(receipt.lane.is_none());
        let parsed = ExecutionReceipt::parse(&receipt.to_json()).unwrap();
        // Absent stays absent through the round trip — nobody upgraded
        // it to a guessed lane.
        assert!(parsed.lane.is_none());
        assert!(parsed.revises.is_none());
    }

    #[test]
    fn a_lane_and_a_revision_survive_the_round_trip() {
        let mut reviewed = answered();
        reviewed.lane = Some(Lane::Decision);
        reviewed.revises = Some("att-0".to_string());
        reviewed.attempt_id = "att-1-review".to_string();
        reviewed.seal();
        let parsed = ExecutionReceipt::parse(&reviewed.to_json()).unwrap();
        assert_eq!(parsed.lane, Some(Lane::Decision));
        assert_eq!(parsed.revises.as_deref(), Some("att-0"));
        // A generation call and an executor job are lanes of their own —
        // the enum does not collapse them into decision work.
        let mut generated = answered();
        generated.lane = Some(Lane::Generation);
        generated.seal();
        let mut delegated = answered();
        delegated.lane = Some(Lane::Executor);
        delegated.seal();
        assert_ne!(generated.lane, delegated.lane);
        assert_ne!(generated.lane, reviewed.lane);
    }

    #[test]
    fn a_request_digest_binds_without_content() {
        let envelope = serde_json::json!({
            "state": "private caller text",
            "questions": {"q1": {"type": "noul", "instructions": "…", "criteria": "…"}}
        });
        let digest = digest_request(&envelope);
        // Reordered keys digest identically — canonicalization is the
        // agreement between caller and service.
        let reordered = serde_json::json!({
            "questions": {"q1": {"criteria": "…", "instructions": "…", "type": "noul"}},
            "state": "private caller text"
        });
        assert_eq!(digest, digest_request(&reordered));
        // And the receipt that carries the digest holds no content.
        let receipt = ExecutionReceipt::for_attempt("http", "req-9", 1, &digest);
        assert!(!receipt.to_json().contains("private caller text"));
    }

    #[test]
    fn an_unattempted_attempt_is_a_distinct_outcome() {
        let mut receipt = answered();
        receipt.outcome = Outcome::Unattempted;
        receipt.result_digest = None;
        receipt.usage = Some("resv-882-released".to_string());
        receipt.seal();
        let parsed = ExecutionReceipt::parse(&receipt.to_json()).unwrap();
        assert_eq!(parsed.outcome, Outcome::Unattempted);
    }
}
