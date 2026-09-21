//! The admission record a `trained` binding is activated on.
//!
//! `gym::admission` writes a decision document — the frozen plan, the
//! ruling, the evidence references, the candidate identity — and seals it
//! as `admission:<sha256>` over the canonical serialization of every field
//! but the digest itself. This module is the registry's copy of that
//! contract: it parses the same document, recomputes the same digest under
//! the same canonicalization the manifest uses, and refuses anything that
//! does not verify. The two crates do not link to each other; the document
//! is the interface, which is the point — a record verified on read cannot
//! be swapped for a stronger-looking one on the way to activation.
//!
//! What the registry consumes of the record is small on purpose: the
//! ruling, the exact candidate identity, the family scope the admission
//! covered, and the plan it was judged under. Everything else — the
//! criteria, the evidence references, the phases — stays inside the
//! document's digest, so it is bound even where this crate does not read
//! it. `docs/decision-models/candidate-admission.md` states the full
//! contract.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::manifest::canonicalize;

/// The schema an admission decision carries. This crate does not import
/// `gym`; the tag is the contract, stated once on each side of it.
pub const DECISION_SCHEMA: &str = "openagents.gym.admission_decision.v1";

/// What the admission decision concluded, as the record carries it.
///
/// Only [`Ruling::Passed`] may activate a binding. `failed` is a measured
/// loss or a tie where improvement was required; `unverifiable` is
/// evidence that could not answer; `refused` is a plan the evidence
/// violated.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Ruling {
    /// Every guard held; the candidate may activate.
    Passed,
    /// The candidate lost, or tied where the plan required improvement.
    Failed,
    /// The evidence could not answer: incomplete coverage, an unmeasured
    /// floor, or a confirmation that has not run.
    Unverifiable,
    /// The evidence or the spend is not the plan's — instrument drift, an
    /// undeclared identity change, or locked evidence spent on another
    /// admission.
    Refused,
}

/// The artifact identity the record admits, extracted from the decision's
/// `candidate` field — the gym row's `door_identity` shape.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Candidate {
    /// The model id the admitted door must publish.
    #[serde(default)]
    pub model: String,
    /// The adapter package the door must serve, when it serves one.
    #[serde(default)]
    pub adapter: Option<String>,
    /// The exact artifact digest the admission ran against.
    #[serde(default)]
    pub artifact_signature: String,
    /// The execution settings the admission was measured under.
    #[serde(default)]
    pub execution: BTreeMap<String, String>,
}

/// A verified admission record.
///
/// `Record::parse` runs the verification: a record that exists at all has
/// already had its schema checked and its digest recomputed over the
/// document it arrived in. What remains to a caller is whether the ruling
/// admits — [`Record::admitted`] — and what the record binds.
#[derive(Clone, Debug)]
pub struct Record {
    /// The plan's id.
    pub plan: String,
    /// The plan's digest — the identity the locked read was spent under.
    pub plan_digest: String,
    /// What the evidence concluded.
    pub ruling: Ruling,
    /// The exact artifact identity the activation binds.
    pub candidate: Candidate,
    /// The family scope the admission covered, which the binding carries.
    pub scope: Vec<String>,
    /// When the decision was taken, as the recorder dated it.
    pub decided_at: String,
    /// `admission:<sha256>` over the document's other fields.
    pub digest: String,
    /// The verified document, kept whole: the criteria, the phases, and
    /// the evidence references are bound by the digest whether or not this
    /// crate reads them.
    document: Value,
}

/// Why an admission record cannot be trusted.
#[derive(Debug)]
pub enum Fault {
    /// The document is not JSON, or not the shape the schema promises.
    Malformed(String),
    /// The document is tagged for another schema — a record that says a
    /// different version is refused rather than read partially.
    Schema {
        /// The tag it carried.
        found: String,
    },
    /// The recorded digest does not recompute over the contents — a bound,
    /// an identity, or a verdict was changed after the record was sealed.
    Tampered {
        /// The digest the record claims.
        recorded: String,
        /// The digest its contents produce.
        computed: String,
    },
    /// The record verified and does not admit. A failed, unverifiable, or
    /// refused decision activates nothing.
    NotAdmitted {
        /// The ruling the record carries.
        ruling: String,
    },
    /// The record pins no artifact digest, so an activation under it would
    /// bind a name rather than an artifact.
    UnpinnedArtifact,
    /// The record admits no family scope, so a binding under it would be
    /// unbounded.
    NoScope,
    /// A write tried to reach the trained lane without a verified record:
    /// a manifest that adds or changes a trained binding outside
    /// `Registry::activate`, or an activation for a tenant the registry
    /// does not know.
    Denied(String),
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(reason) => write!(f, "the admission record is not readable: {reason}"),
            Self::Schema { found } => {
                write!(f, "the admission record is tagged {found}, which is not {DECISION_SCHEMA}")
            }
            Self::Tampered { recorded, computed } => write!(
                f,
                "the admission record's digest does not recompute over its contents: \
                 recorded {recorded}, computed {computed} — a changed record is a different \
                 admission"
            ),
            Self::NotAdmitted { ruling } => write!(
                f,
                "the admission record's ruling is {ruling}; only a passed decision may \
                 activate a candidate"
            ),
            Self::UnpinnedArtifact => write!(
                f,
                "the admission record pins no artifact digest; a trained binding binds the \
                 exact artifact the admission ran against, not a name for it"
            ),
            Self::NoScope => write!(
                f,
                "the admission record names no family scope; a binding carries the scope the \
                 admission covered"
            ),
            Self::Denied(reason) => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for Fault {}

/// The fields the registry reads, lifted out of the verified document.
#[derive(Deserialize)]
struct Wire {
    /// The plan's id.
    plan: String,
    /// The plan's digest.
    plan_digest: String,
    /// The ruling.
    ruling: Ruling,
    /// The admitted candidate identity.
    candidate: Candidate,
    /// The admitted family scope.
    #[serde(default)]
    scope: Vec<String>,
    /// When the decision was taken.
    #[serde(default)]
    decided_at: String,
}

impl Record {
    /// Parse and verify an admission record.
    ///
    /// Three checks, all before a field is trusted: the document parses,
    /// it is tagged `openagents.gym.admission_decision.v1`, and its
    /// `admission:<sha256>` digest recomputes over every other field under
    /// the manifest's canonicalization. A record that fails any of them is
    /// not a record — it is refused, and no ruling inside it is read.
    pub fn parse(text: &str) -> Result<Self, Fault> {
        let mut document: Value =
            serde_json::from_str(text).map_err(|error| Fault::Malformed(error.to_string()))?;
        let object = document
            .as_object_mut()
            .ok_or_else(|| Fault::Malformed("the record is not an object".to_string()))?;
        let found = object
            .get("schema")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if found != DECISION_SCHEMA {
            return Err(Fault::Schema { found });
        }
        let recorded = object
            .remove("digest")
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
        let computed = format!("admission:{:x}", Sha256::digest(canonicalize(&document).as_bytes()));
        if recorded != computed {
            return Err(Fault::Tampered { recorded, computed });
        }
        let wire: Wire =
            serde_json::from_value(document.clone()).map_err(|error| {
                Fault::Malformed(format!("the record is not the decision's shape: {error}"))
            })?;
        Ok(Self {
            plan: wire.plan,
            plan_digest: wire.plan_digest,
            ruling: wire.ruling,
            candidate: wire.candidate,
            scope: wire.scope,
            decided_at: wire.decided_at,
            digest: recorded,
            document,
        })
    }

    /// Whether the record admits the candidate — the ruling is `passed`
    /// and the record binds an exact artifact under a named scope.
    ///
    /// This is the check [`crate::Registry::activate`] runs; it is a
    /// method so a caller can ask the question without activating.
    pub fn admitted(&self) -> Result<(), Fault> {
        if self.ruling != Ruling::Passed {
            return Err(Fault::NotAdmitted {
                ruling: ruling_name(self.ruling).to_string(),
            });
        }
        if self.candidate.artifact_signature.is_empty() {
            return Err(Fault::UnpinnedArtifact);
        }
        if self.scope.is_empty() {
            return Err(Fault::NoScope);
        }
        Ok(())
    }

    /// The reference a binding's `promotion` field carries: the record's
    /// own digest, which is the admission's identity, not a name for it.
    #[must_use]
    pub fn reference(&self) -> &str {
        &self.digest
    }

    /// The verified document, for a reader that wants the criteria and the
    /// evidence references the digest already binds.
    #[must_use]
    pub fn document(&self) -> &Value {
        &self.document
    }
}

/// The word a ruling serializes to, for errors.
const fn ruling_name(ruling: Ruling) -> &'static str {
    match ruling {
        Ruling::Passed => "passed",
        Ruling::Failed => "failed",
        Ruling::Unverifiable => "unverifiable",
        Ruling::Refused => "refused",
    }
}
