//! Receipt validation: how far an attributable claim can be checked.
//!
//! A receipt is an attributable claim, never remote attestation — the
//! issuer names itself and what it did, and a reader decides how much
//! of the claim the evidence checks. Validation is that decision in
//! three parts. [`Validate::receipt`] checks the schema facts a receipt
//! must hold to mean anything. [`Validate::binding`] checks the receipt
//! against the trace step it was joined to — the caller's word against
//! the service's. [`Validate::report`] folds both into a [`Support`]
//! mark per step over a whole joined trace.
//!
//! A self-consistent digest is not verification. The receipt's digest
//! proves the document agrees with itself; it says nothing about who
//! wrote it. `verified` asks for whatever origin authentication the
//! caller's transport established on top of a clean schema and binding.
//! A receipt whose format carries none — or that predates the fields
//! the identity checks need — is `legacy` with the missing pieces
//! named, never a verified mark the evidence cannot carry.
//!
//! Validation is pure over supplied records: no network, no clock, no
//! format reading. Every fault is typed and names what it offends, and
//! the report's ordering is stable, so two runs over the same records
//! agree.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::execution::{ExecutionReceipt, Outcome, Registry, SCHEMA};
use crate::join::{Joined, Problem, Row, StepRef, Verification};

/// The outcomes this schema names, in their serialized forms.
const OUTCOMES: [&str; 5] = [
    "answered",
    "refused",
    "unavailable",
    "unattempted",
    "unknown",
];

/// One thing wrong with a receipt, a binding, or an attempt chain.
///
/// Every fault is typed and names what it offends — a validation that
/// cannot say which record is wrong is noise, the same rule the join's
/// [`Problem`] follows.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Fault {
    /// A field the receipt needs to mean anything is absent or empty —
    /// a receipt with no request identity binds nothing.
    MissingField {
        /// The absent field's name.
        field: String,
    },
    /// The schema tag is not this version's — the receipt predates the
    /// fields these checks read, or comes from somewhere newer.
    UnknownSchema {
        /// The tag the receipt carries.
        found: String,
    },
    /// A digest field is not `sha256:` followed by sixty-four lowercase
    /// hexadecimal digits — it binds nothing anyone can recompute.
    MalformedDigest {
        /// The field carrying the malformed digest.
        field: String,
    },
    /// The receipt's own digest does not recompute over its contents —
    /// the document disagrees with itself, so every claim in it is
    /// suspect.
    InconsistentDigest,
    /// The attempt and request identities contradict: an attempt
    /// numbered zero, a retry that left the request's digest or tenant
    /// behind, or one attempt identity claimed under two requests.
    InconsistentIdentity {
        /// The request the offending attempt claims.
        request: String,
        /// The offending attempt.
        attempt: u32,
        /// What contradicts what.
        detail: String,
    },
    /// The timing contradicts itself: a resolution instant that is not
    /// RFC 3339, or an attempt that resolved before an earlier attempt
    /// of the same request.
    IncoherentTiming {
        /// The request the offending attempt claims.
        request: String,
        /// The offending attempt.
        attempt: u32,
        /// What contradicts what.
        detail: String,
    },
    /// The outcome is not one this schema names. A parsed receipt
    /// cannot carry one — the check stays so a receipt constructed
    /// rather than read still faults by name.
    UnknownOutcome {
        /// The outcome the receipt carried.
        found: String,
    },
    /// The receipt and the step it joined disagree on a field both
    /// carry — same call, different claim.
    Mismatch {
        /// The field the disagreement is on.
        field: String,
        /// The step's claim.
        step: String,
        /// The receipt's claim.
        receipt: String,
    },
    /// The receipt was issued under one call identity and joined to a
    /// step recording another — a different request, attempt, tenant,
    /// or document. An identity mix is never a generic mismatch.
    IdentityMix {
        /// The identity axis that diverged.
        field: String,
        /// The step's claim.
        step: String,
        /// The receipt's claim.
        receipt: String,
    },
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField { field } => write!(f, "the receipt names no `{field}`"),
            Self::UnknownSchema { found } => {
                write!(f, "receipt schema `{found}` is not `{SCHEMA}`")
            }
            Self::MalformedDigest { field } => {
                write!(f, "`{field}` is not a well-formed sha256 digest")
            }
            Self::InconsistentDigest => write!(
                f,
                "the receipt's digest does not recompute over its contents"
            ),
            Self::InconsistentIdentity {
                request,
                attempt,
                detail,
            }
            | Self::IncoherentTiming {
                request,
                attempt,
                detail,
            } => write!(f, "{request} attempt {attempt}: {detail}"),
            Self::UnknownOutcome { found } => {
                write!(f, "outcome `{found}` is not one the schema names")
            }
            Self::Mismatch {
                field,
                step,
                receipt,
            } => write!(
                f,
                "the step records `{step}` and the receipt claims `{receipt}` for `{field}`"
            ),
            Self::IdentityMix {
                field,
                step,
                receipt,
            } => write!(
                f,
                "`{field}` joins a receipt claiming `{receipt}` to a step recording `{step}`"
            ),
        }
    }
}

impl std::error::Error for Fault {}

impl Fault {
    /// Whether this attempt-chain fault attaches to one request's
    /// attempt — the report files it under that step's entry.
    fn attempt_of(&self, request: &str, attempt: u32) -> bool {
        match self {
            Self::InconsistentIdentity {
                request: claimed,
                attempt: at,
                ..
            }
            | Self::IncoherentTiming {
                request: claimed,
                attempt: at,
                ..
            } => claimed == request && *at == attempt,
            _ => false,
        }
    }
}

/// How far a step's receipt evidence goes: a three-state availability
/// mark.
///
/// A receipt is an attributable claim, and this mark is the answer to
/// how far that claim can be checked. `verified` means the schema, the
/// binding, and the origin check all pass. `legacy` means the receipt
/// exists and is well-formed, but the claim cannot be fully checked —
/// it predates the fields the identity checks need, or the format
/// carries no origin authentication — and the reasons say which.
/// `absent` means no receipt exists at all. A self-consistent digest
/// alone is never `verified`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Support {
    /// Schema, binding, and origin checks all pass — everything this
    /// validation can check, checked. Still an attributable claim,
    /// never remote attestation.
    Verified,
    /// The receipt is well-formed but the claim cannot be fully
    /// checked; the reasons name the fields or the authentication the
    /// check never had.
    Legacy {
        /// Why the check could not complete — the missing fields or
        /// the corroboration nobody recorded.
        reasons: Vec<String>,
    },
    /// No receipt exists for the step — a call the service never owned
    /// up to.
    Absent,
}

/// One step's support and the faults against it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StepReport {
    /// The step as the caller recorded it.
    pub step: StepRef,
    /// How far the step's receipt evidence goes.
    pub support: Support,
    /// Every fault the step's receipt and binding carry, typed and
    /// named, in the order the checks found them.
    pub faults: Vec<Fault>,
}

/// A receipt no step claimed, validated on its own.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OrphanReport {
    /// The request the orphaned receipt claims.
    pub request: String,
    /// The attempt it claims.
    pub attempt: u32,
    /// Its attempt identity.
    pub attempt_id: String,
    /// Its own digest — the identity a reader quotes for the call.
    pub receipt_digest: String,
    /// Every fault it carries, typed and named.
    pub faults: Vec<Fault>,
}

/// The report's counts — the summary a reader checks first.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Counts {
    /// Steps the trace recorded, receipted or not.
    pub steps: usize,
    /// Steps whose claim verified.
    pub verified: usize,
    /// Steps whose claim is legacy.
    pub legacy: usize,
    /// Steps no receipt claimed.
    pub absent: usize,
    /// Receipts no step claimed.
    pub orphaned: usize,
    /// Faults across every entry.
    pub faults: usize,
}

/// The deterministic report over one joined trace: every step's
/// support state, every fault typed and named, and the counts.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Report {
    /// One entry per step the trace recorded — matched or not — in a
    /// stable identity order.
    pub steps: Vec<StepReport>,
    /// Receipts no step claimed, each validated on its own.
    pub orphans: Vec<OrphanReport>,
    /// The problems the join itself found, kept with the report so the
    /// trace's evidence travels together.
    pub problems: Vec<Problem>,
    /// The counts.
    pub counts: Counts,
}

/// The validation itself: schema, binding, chain, and report checks
/// over caller-supplied records.
pub struct Validate;

impl Validate {
    /// The schema facts a receipt must hold to mean anything: the
    /// version tag, the fields that bind the claim, well-formed
    /// digests, a self-consistent document, a coherent timing, and an
    /// outcome the schema names — each absence or contradiction a
    /// typed fault.
    #[must_use]
    pub fn receipt(receipt: &ExecutionReceipt) -> Vec<Fault> {
        let mut faults = Vec::new();

        if receipt.v != SCHEMA {
            faults.push(Fault::UnknownSchema {
                found: receipt.v.clone(),
            });
        }

        // The fields without which the receipt binds nothing.
        for (field, value) in [
            ("request", receipt.request.as_str()),
            ("attempt_id", receipt.attempt_id.as_str()),
            ("transport", receipt.transport.as_str()),
            ("request_digest", receipt.request_digest.as_str()),
            ("digest", receipt.digest.as_str()),
        ] {
            if value.is_empty() {
                faults.push(Fault::MissingField {
                    field: field.to_string(),
                });
            }
        }
        if receipt.attempt == 0 {
            faults.push(Fault::InconsistentIdentity {
                request: receipt.request.clone(),
                attempt: receipt.attempt,
                detail: "attempts are one-based; attempt 0 names no attempt".to_string(),
            });
        }

        for (field, digest) in digest_fields(receipt) {
            if !digest.is_empty() && !well_formed_digest(digest) {
                faults.push(Fault::MalformedDigest {
                    field: field.to_string(),
                });
            }
        }

        // A digest that does not recompute is worse than a missing one:
        // the document claims a consistency it does not have.
        if well_formed_digest(&receipt.digest) && receipt.digest != receipt.compute_digest() {
            faults.push(Fault::InconsistentDigest);
        }

        // The one instant a receipt carries must be a real instant; the
        // ordering rule — no attempt resolving before an earlier one —
        // is the chain check's, in `request`.
        if let Some(resolved) = &receipt.timing.resolved_at
            && resolved_seconds(resolved).is_none()
        {
            faults.push(Fault::IncoherentTiming {
                request: receipt.request.clone(),
                attempt: receipt.attempt,
                detail: format!("resolved_at `{resolved}` is not an RFC 3339 instant"),
            });
        }

        let outcome = serde_json::to_value(receipt.outcome)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
        if !OUTCOMES.contains(&outcome.as_str()) {
            faults.push(Fault::UnknownOutcome { found: outcome });
        }

        // What the outcome claims decides what else must be present: an
        // answer binds the result it produced, and a refusal or a
        // transport failure names its cause.
        match receipt.outcome {
            Outcome::Answered if receipt.result_digest.is_none() => {
                faults.push(Fault::MissingField {
                    field: "result_digest".to_string(),
                });
            }
            Outcome::Refused | Outcome::Unavailable if receipt.cause.is_none() => {
                faults.push(Fault::MissingField {
                    field: "cause".to_string(),
                });
            }
            _ => {}
        }

        faults
    }

    /// The checks one logical request's receipts must hold together:
    /// retries share the request's digest and tenant, an attempt
    /// identity is claimed once, and no attempt resolves before an
    /// earlier attempt of the same request. An attempt three that
    /// precedes attempt one is not a retry, it is a contradiction.
    #[must_use]
    pub fn request(receipts: &[ExecutionReceipt]) -> Vec<Fault> {
        let mut faults = Vec::new();

        // One attempt identity belongs to one attempt of one request —
        // an identity claimed under two is a contradiction wherever it
        // appears, and every claimant faults so the answer does not
        // depend on the order the receipts arrived in.
        let mut owners: BTreeMap<&str, Vec<&ExecutionReceipt>> = BTreeMap::new();
        for receipt in receipts {
            if !receipt.attempt_id.is_empty() {
                owners
                    .entry(receipt.attempt_id.as_str())
                    .or_default()
                    .push(receipt);
            }
        }
        for (attempt_id, claimants) in &owners {
            let mut calls: Vec<(&str, u32)> = claimants
                .iter()
                .map(|receipt| (receipt.request.as_str(), receipt.attempt))
                .collect();
            calls.sort_unstable();
            calls.dedup();
            if calls.len() < 2 {
                continue;
            }
            let claims = calls
                .iter()
                .map(|(request, attempt)| format!("{request} attempt {attempt}"))
                .collect::<Vec<_>>()
                .join(", ");
            for receipt in claimants {
                faults.push(Fault::InconsistentIdentity {
                    request: receipt.request.clone(),
                    attempt: receipt.attempt,
                    detail: format!(
                        "attempt identity `{attempt_id}` names more than one call: {claims}"
                    ),
                });
            }
        }

        let mut chains: BTreeMap<&str, Vec<&ExecutionReceipt>> = BTreeMap::new();
        for receipt in receipts {
            if !receipt.request.is_empty() {
                chains
                    .entry(receipt.request.as_str())
                    .or_default()
                    .push(receipt);
            }
        }
        for (request, mut chain) in chains {
            chain.sort_by(|a, b| {
                a.attempt
                    .cmp(&b.attempt)
                    .then_with(|| a.attempt_id.cmp(&b.attempt_id))
            });
            // A retry serves the same envelope under the same
            // admission — the earliest attempt's is the request's own.
            let first = chain[0];
            for receipt in &chain[1..] {
                if receipt.request_digest != first.request_digest {
                    faults.push(Fault::InconsistentIdentity {
                        request: request.to_string(),
                        attempt: receipt.attempt,
                        detail: format!(
                            "attempt {} carries request digest `{}`, not the request's `{}`",
                            receipt.attempt, receipt.request_digest, first.request_digest
                        ),
                    });
                }
                if receipt.tenant != first.tenant {
                    faults.push(Fault::InconsistentIdentity {
                        request: request.to_string(),
                        attempt: receipt.attempt,
                        detail: format!(
                            "attempt {} was issued under tenant `{:?}`, not the request's `{:?}`",
                            receipt.attempt, receipt.tenant, first.tenant
                        ),
                    });
                }
            }
            // Resolution follows attempt order — a later attempt cannot
            // finish before an earlier one.
            let mut latest: Option<(u32, i64)> = None;
            for receipt in &chain {
                let Some(instant) = receipt
                    .timing
                    .resolved_at
                    .as_deref()
                    .and_then(resolved_seconds)
                else {
                    continue;
                };
                if let Some((earlier, at)) = latest
                    && instant < at
                {
                    faults.push(Fault::IncoherentTiming {
                        request: request.to_string(),
                        attempt: receipt.attempt,
                        detail: format!(
                            "attempt {} resolved before attempt {} of the same request",
                            receipt.attempt, earlier
                        ),
                    });
                    continue;
                }
                latest = Some((receipt.attempt, instant));
            }
        }
        faults
    }

    /// The checks a receipt must pass against the trace step it was
    /// joined to: the call identities are the same identity, the
    /// function and program references the receipt carries name the
    /// step's, and the digests the caller recorded corroborate rather
    /// than contradict. An identity mix — a receipt issued under one
    /// request, attempt, tenant, or document joined to a step from
    /// another — is its own fault, never folded into a generic
    /// mismatch.
    #[must_use]
    pub fn binding(receipt: &ExecutionReceipt, joined: &Row) -> Vec<Fault> {
        let step = &joined.step;
        let mut faults = Vec::new();

        // The identity axes first — a receipt joined across call
        // identities is a mix, not a mismatch.
        if receipt.request != step.request {
            faults.push(Fault::IdentityMix {
                field: "request".to_string(),
                step: step.request.clone(),
                receipt: receipt.request.clone(),
            });
        }
        if receipt.attempt != step.attempt {
            faults.push(Fault::IdentityMix {
                field: "attempt".to_string(),
                step: step.attempt.to_string(),
                receipt: receipt.attempt.to_string(),
            });
        }
        if receipt.digest != joined.receipt_digest {
            faults.push(Fault::IdentityMix {
                field: "receipt_digest".to_string(),
                step: joined.receipt_digest.clone(),
                receipt: receipt.digest.clone(),
            });
        }
        if receipt.tenant != joined.references.tenant {
            faults.push(Fault::IdentityMix {
                field: "tenant".to_string(),
                step: joined.references.tenant.clone().unwrap_or_default(),
                receipt: receipt.tenant.clone().unwrap_or_default(),
            });
        }
        if receipt.registry != joined.references.registry {
            faults.push(Fault::IdentityMix {
                field: "registry".to_string(),
                step: registry_label(&joined.references.registry),
                receipt: registry_label(&receipt.registry),
            });
        }

        // The function and program references the receipt carries must
        // name the step's — the policy slot in this version, and the
        // evaluated item against the step's work item.
        if let Some(policy) = &joined.references.policy {
            let expected = if step.program.is_empty() {
                &step.function
            } else {
                &step.program
            };
            if !expected.is_empty() && policy != expected {
                faults.push(Fault::Mismatch {
                    field: "policy".to_string(),
                    step: expected.clone(),
                    receipt: policy.clone(),
                });
            }
        }
        if let Some(item) = receipt
            .evaluation
            .as_ref()
            .and_then(|evaluation| evaluation.item.as_ref())
            && !step.work_item.is_empty()
            && item != &step.work_item
        {
            faults.push(Fault::Mismatch {
                field: "work_item".to_string(),
                step: step.work_item.clone(),
                receipt: item.clone(),
            });
        }

        // The digests the caller recorded corroborate the receipt or
        // contradict it — a contradiction is a typed fault, and a
        // digest the caller never recorded simply cannot be checked.
        if !step.request_digest.is_empty() && step.request_digest != receipt.request_digest {
            faults.push(Fault::Mismatch {
                field: "request_digest".to_string(),
                step: step.request_digest.clone(),
                receipt: receipt.request_digest.clone(),
            });
        }
        match (&step.result_digest, &receipt.result_digest) {
            (Some(caller), Some(service)) if caller != service => {
                faults.push(Fault::Mismatch {
                    field: "result_digest".to_string(),
                    step: caller.clone(),
                    receipt: service.clone(),
                });
            }
            (Some(caller), None) => faults.push(Fault::Mismatch {
                field: "result_digest".to_string(),
                step: caller.clone(),
                receipt: String::new(),
            }),
            _ => {}
        }
        if let Some(job) = &step.job
            && job != &receipt.attempt_id
        {
            faults.push(Fault::Mismatch {
                field: "attempt_id".to_string(),
                step: job.clone(),
                receipt: receipt.attempt_id.clone(),
            });
        }

        faults
    }

    /// The deterministic report over one joined trace: every step's
    /// support state, every fault typed and named, and the counts.
    /// Receipts are matched back to their rows by digest — the identity
    /// a reader quotes — so the same records produce the same report
    /// however they were ordered.
    #[must_use]
    pub fn report(joined: &Joined, receipts: &[ExecutionReceipt]) -> Report {
        let chain = Self::request(receipts);

        let mut steps: Vec<StepReport> = Vec::new();
        for row in &joined.rows {
            let receipt = receipts
                .iter()
                .find(|receipt| !receipt.digest.is_empty() && receipt.digest == row.receipt_digest)
                .or_else(|| {
                    receipts.iter().find(|receipt| {
                        receipt.request == row.references.request
                            && receipt.attempt == row.references.attempt
                    })
                });
            let mut faults = Vec::new();
            match receipt {
                Some(receipt) => {
                    faults.extend(Self::receipt(receipt));
                    faults.extend(Self::binding(receipt, row));
                }
                None => faults.push(Fault::InconsistentIdentity {
                    request: row.references.request.clone(),
                    attempt: row.references.attempt,
                    detail: "the row's receipt was not among the supplied records".to_string(),
                }),
            }
            faults.extend(
                chain
                    .iter()
                    .filter(|fault| {
                        fault.attempt_of(&row.references.request, row.references.attempt)
                    })
                    .cloned(),
            );
            steps.push(StepReport {
                support: support(row, receipt, &faults),
                step: row.step.clone(),
                faults,
            });
        }
        for step in &joined.unreceipted {
            steps.push(StepReport {
                step: step.clone(),
                support: Support::Absent,
                faults: Vec::new(),
            });
        }
        steps.sort_by(|a, b| step_key(&a.step).cmp(&step_key(&b.step)));

        let mut orphans: Vec<OrphanReport> = joined
            .orphaned
            .iter()
            .map(|receipt| {
                let mut faults = Self::receipt(receipt);
                faults.extend(
                    chain
                        .iter()
                        .filter(|fault| fault.attempt_of(&receipt.request, receipt.attempt))
                        .cloned(),
                );
                OrphanReport {
                    request: receipt.request.clone(),
                    attempt: receipt.attempt,
                    attempt_id: receipt.attempt_id.clone(),
                    receipt_digest: receipt.digest.clone(),
                    faults,
                }
            })
            .collect();
        orphans.sort_by(|a, b| {
            (&a.request, a.attempt, &a.attempt_id).cmp(&(&b.request, b.attempt, &b.attempt_id))
        });

        let counts = Counts {
            steps: steps.len(),
            verified: steps
                .iter()
                .filter(|step| step.support == Support::Verified)
                .count(),
            legacy: steps
                .iter()
                .filter(|step| matches!(step.support, Support::Legacy { .. }))
                .count(),
            absent: steps
                .iter()
                .filter(|step| step.support == Support::Absent)
                .count(),
            orphaned: orphans.len(),
            faults: steps.iter().map(|step| step.faults.len()).sum::<usize>()
                + orphans
                    .iter()
                    .map(|orphan| orphan.faults.len())
                    .sum::<usize>(),
        };

        Report {
            steps,
            orphans,
            problems: joined.problems(),
            counts,
        }
    }
}

/// The support a row earns: verified only when schema, binding, and
/// origin all pass, legacy with the reason named when they cannot.
fn support(row: &Row, receipt: Option<&ExecutionReceipt>, faults: &[Fault]) -> Support {
    if !faults.is_empty() {
        return Support::Legacy {
            reasons: faults.iter().map(ToString::to_string).collect(),
        };
    }
    match row.verification {
        Verification::Verified => Support::Verified,
        Verification::SelfConsistent => Support::Legacy {
            reasons: self_consistent_reasons(row, receipt),
        },
        Verification::Unsupported => Support::Legacy {
            reasons: unsupported_reasons(receipt),
        },
    }
}

/// What a self-consistent row still cannot say, named — the
/// corroboration the caller never recorded or the issuer
/// authentication the transport never established.
fn self_consistent_reasons(row: &Row, receipt: Option<&ExecutionReceipt>) -> Vec<String> {
    let mut reasons = Vec::new();
    if row.step.request_digest.is_empty() {
        reasons.push("the caller recorded no request digest to corroborate".to_string());
    }
    if row.step.result_digest.is_none()
        && receipt
            .and_then(|receipt| receipt.result_digest.as_ref())
            .is_some()
    {
        reasons.push("the caller recorded no result digest to corroborate".to_string());
    }
    if !row.step.origin_authenticated {
        reasons
            .push("the caller's transport did not authenticate the answering origin".to_string());
    }
    reasons
}

/// Why an unsupported row's check was never available — the schema the
/// receipt predates, or the identity fields it never carried, each
/// named.
fn unsupported_reasons(receipt: Option<&ExecutionReceipt>) -> Vec<String> {
    match receipt {
        Some(receipt) if receipt.v != SCHEMA => vec![format!(
            "receipt schema `{}` predates the fields the identity checks need",
            receipt.v
        )],
        Some(receipt) => [
            ("request identity", receipt.request.is_empty()),
            ("request digest", receipt.request_digest.is_empty()),
            ("attempt identity", receipt.attempt_id.is_empty()),
        ]
        .into_iter()
        .filter(|(_, absent)| *absent)
        .map(|(field, _)| format!("the receipt names no {field}"))
        .collect(),
        None => vec!["the joined receipt was not among the supplied records".to_string()],
    }
}

/// A step's canonical sort key — session down to attempt — so the
/// report's ordering is stable however the records arrived.
fn step_key(step: &StepRef) -> (&str, &str, &str, &str, &str, &str, &str, u32) {
    (
        step.session.as_str(),
        step.turn.as_str(),
        step.program.as_str(),
        step.step.as_str(),
        step.function.as_str(),
        step.work_item.as_str(),
        step.request.as_str(),
        step.attempt,
    )
}

/// Every digest a receipt carries, with its field name — the bindings
/// the schema declares, checked for shape rather than content.
fn digest_fields(receipt: &ExecutionReceipt) -> Vec<(&'static str, &str)> {
    let mut fields = vec![
        ("request_digest", receipt.request_digest.as_str()),
        ("digest", receipt.digest.as_str()),
        (
            "requested.artifact_signature",
            receipt.requested.artifact_signature.as_str(),
        ),
        (
            "served.artifact_signature",
            receipt.served.artifact_signature.as_str(),
        ),
    ];
    if let Some(result) = &receipt.result_digest {
        fields.push(("result_digest", result));
    }
    if let Some(registry) = &receipt.registry {
        fields.push(("registry.digest", registry.digest.as_str()));
    }
    if let Some(evaluation) = &receipt.evaluation {
        fields.push(("evaluation.suite_digest", evaluation.suite_digest.as_str()));
        if let Some(digest) = &evaluation.question_digest {
            fields.push(("evaluation.question_digest", digest));
        }
        if let Some(digest) = &evaluation.gate_digest {
            fields.push(("evaluation.gate_digest", digest));
        }
    }
    fields
}

/// Whether a digest is `sha256:` followed by sixty-four lowercase
/// alphanumeric characters — the shape every digest in this workspace
/// shares.
fn well_formed_digest(digest: &str) -> bool {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
}

fn registry_label(registry: &Option<Registry>) -> String {
    match registry {
        Some(registry) => format!("{}#{}", registry.digest, registry.sequence),
        None => String::new(),
    }
}

/// The seconds since the epoch an RFC 3339 instant names — all an
/// ordering check needs of `resolved_at`.
fn resolved_seconds(text: &str) -> Option<i64> {
    // `YYYY-MM-DD` `T` `HH:MM:SS`[`.fraction`][`Z` | `±HH:MM`]
    let (date, clock) = text.split_once(['T', 't', ' '])?;
    if date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return None;
    }
    let year: i64 = date[..4].parse().ok()?;
    let month: i64 = date[5..7].parse().ok()?;
    let day: i64 = date[8..10].parse().ok()?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }

    let (clock, offset) =
        if let Some(clock) = clock.strip_suffix('Z').or_else(|| clock.strip_suffix('z')) {
            (clock, 0)
        } else {
            let at = clock.rfind(['+', '-'])?;
            let zone = &clock[at..];
            if zone.len() != 6 || zone.as_bytes()[3] != b':' {
                return None;
            }
            let hours: i64 = zone[1..3].parse().ok()?;
            let minutes: i64 = zone[4..6].parse().ok()?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            let seconds = hours * 3600 + minutes * 60;
            (
                &clock[..at],
                if zone.starts_with('-') {
                    -seconds
                } else {
                    seconds
                },
            )
        };

    let (hms, fraction) = match clock.split_once('.') {
        Some((hms, fraction)) => (hms, Some(fraction)),
        None => (clock, None),
    };
    if fraction.is_some_and(|fraction| {
        fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return None;
    }
    if hms.len() != 8 || hms.as_bytes()[2] != b':' || hms.as_bytes()[5] != b':' {
        return None;
    }
    let hour: i64 = hms[..2].parse().ok()?;
    let minute: i64 = hms[3..5].parse().ok()?;
    let second: i64 = hms[6..8].parse().ok()?;
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    Some(days_from_civil(year, month, day) * 86400 + hour * 3600 + minute * 60 + second - offset)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since the epoch for a civil date — the usual proleptic
/// Gregorian conversion, stated rather than assumed.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::{Evaluation, Served, Timing};
    use crate::join::{Cost, Join};

    fn digest_of(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    /// A receipt the way a gateway writes one for an answered call.
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

    /// The step that believes it made `answered()`'s call.
    fn step() -> StepRef {
        StepRef {
            session: "ses-1".to_string(),
            turn: "turn-7".to_string(),
            program: "triage".to_string(),
            step: "step-3".to_string(),
            function: "classify".to_string(),
            work_item: "item-9".to_string(),
            request: "req-1".to_string(),
            attempt: 1,
            request_digest: digest_of('q'),
            result_digest: Some(digest_of('s')),
            job: None,
            origin_authenticated: true,
            cost: Cost::Known {
                millionths: 42,
                currency: "USD".to_string(),
            },
        }
    }

    fn joined(receipt: &ExecutionReceipt) -> Row {
        Join::of(vec![step()], vec![receipt.clone()])
            .rows
            .into_iter()
            .next()
            .expect("the pair joins")
    }

    fn missing(field: &str) -> Fault {
        Fault::MissingField {
            field: field.to_string(),
        }
    }

    #[test]
    fn a_well_formed_receipt_passes_schema_binding_and_report() {
        let receipt = answered();
        assert!(Validate::receipt(&receipt).is_empty());
        assert!(Validate::binding(&receipt, &joined(&receipt)).is_empty());

        let joined = Join::of(vec![step()], vec![receipt.clone()]);
        let report = Validate::report(&joined, &[receipt]);
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].support, Support::Verified);
        assert!(report.steps[0].faults.is_empty());
        assert_eq!(report.counts.verified, 1);
        assert_eq!(report.counts.faults, 0);
    }

    /// `answered()` with one required field emptied, sealed unless the
    /// emptied field is the digest itself.
    fn without(field: &str) -> ExecutionReceipt {
        let mut receipt = answered();
        match field {
            "request" => receipt.request = String::new(),
            "attempt_id" => receipt.attempt_id = String::new(),
            "transport" => receipt.transport = String::new(),
            "request_digest" => receipt.request_digest = String::new(),
            "digest" => receipt.digest = String::new(),
            _ => unreachable!("the test names a required field"),
        }
        if field != "digest" {
            receipt.seal();
        }
        receipt
    }

    #[test]
    fn each_missing_field_faults_by_name() {
        for field in [
            "request",
            "attempt_id",
            "transport",
            "request_digest",
            "digest",
        ] {
            let faults = Validate::receipt(&without(field));
            assert!(faults.contains(&missing(field)), "{field}: {faults:?}");
        }

        // An answered call binds the result it produced; a refusal or a
        // transport failure names its cause.
        let mut unanswered = answered();
        unanswered.result_digest = None;
        unanswered.seal();
        assert!(Validate::receipt(&unanswered).contains(&missing("result_digest")));

        for outcome in [Outcome::Refused, Outcome::Unavailable] {
            let mut receipt = answered();
            receipt.outcome = outcome;
            receipt.cause = None;
            receipt.result_digest = None;
            receipt.seal();
            assert!(Validate::receipt(&receipt).contains(&missing("cause")));
        }
    }

    #[test]
    fn each_malformed_field_faults_by_name() {
        let mut receipt = answered();
        receipt.v = "openagents.receipt.execution.v0".to_string();
        receipt.seal();
        assert_eq!(
            Validate::receipt(&receipt)[0],
            Fault::UnknownSchema {
                found: "openagents.receipt.execution.v0".to_string()
            }
        );

        for field in ["request_digest", "result_digest"] {
            let mut receipt = answered();
            if field == "request_digest" {
                receipt.request_digest = "sha256:tooshort".to_string();
            } else {
                receipt.result_digest = Some("sha256:tooshort".to_string());
            }
            receipt.seal();
            assert!(
                Validate::receipt(&receipt).contains(&Fault::MalformedDigest {
                    field: field.to_string()
                })
            );
        }

        let mut tampered = answered();
        tampered.served.artifact_signature = digest_of('f');
        assert!(Validate::receipt(&tampered).contains(&Fault::InconsistentDigest));

        let mut zero = answered();
        zero.attempt = 0;
        zero.seal();
        assert!(
            Validate::receipt(&zero)
                .iter()
                .any(|fault| matches!(fault, Fault::InconsistentIdentity { .. }))
        );

        let mut clockless = answered();
        clockless.timing.resolved_at = Some("not a time".to_string());
        clockless.seal();
        assert!(
            Validate::receipt(&clockless)
                .iter()
                .any(|fault| matches!(fault, Fault::IncoherentTiming { .. }))
        );
    }

    #[test]
    fn a_retry_with_a_divergent_request_identity_faults() {
        // A retry serves the same request envelope — a second attempt
        // carrying another request digest left the request behind.
        let mut second = answered();
        second.attempt = 2;
        second.attempt_id = "att-2".to_string();
        second.request_digest = digest_of('z');
        second.seal();
        let faults = Validate::request(&[answered(), second]);
        assert!(
            faults
                .iter()
                .any(|fault| matches!(fault, Fault::InconsistentIdentity { attempt: 2, .. }))
        );

        // One attempt identity cannot be claimed under two requests.
        let mut other = answered();
        other.request = "req-2".to_string();
        other.seal();
        let faults = Validate::request(&[answered(), other]);
        assert!(
            faults
                .iter()
                .any(|fault| matches!(fault, Fault::InconsistentIdentity { attempt: 1, .. }))
        );

        // A retry joined to a step of another request is an identity
        // mix, not a mismatch.
        let row = joined(&answered());
        let mut foreign = answered();
        foreign.request = "req-9".to_string();
        foreign.attempt = 2;
        foreign.attempt_id = "att-9".to_string();
        foreign.seal();
        let faults = Validate::binding(&foreign, &row);
        assert!(faults.iter().any(|fault| matches!(
            fault,
            Fault::IdentityMix { field, .. } if field == "request"
        )));
        assert!(!faults.iter().any(|fault| matches!(
            fault,
            Fault::Mismatch { field, .. } if field == "request"
        )));
    }

    #[test]
    fn a_later_attempt_resolving_before_an_earlier_one_faults() {
        let mut first = answered();
        first.timing.resolved_at = Some("2026-09-21T00:05:00Z".to_string());
        first.seal();
        let mut second = answered();
        second.attempt = 2;
        second.attempt_id = "att-2".to_string();
        second.timing.resolved_at = Some("2026-09-21T00:01:00Z".to_string());
        second.seal();
        let faults = Validate::request(&[first, second]);
        assert!(
            faults
                .iter()
                .any(|fault| matches!(fault, Fault::IncoherentTiming { attempt: 2, .. }))
        );
    }

    #[test]
    fn a_cross_identity_join_faults_as_a_mix_never_a_mismatch() {
        let row = joined(&answered());
        // A receipt issued under another tenant joined to this step.
        let mut foreign = answered();
        foreign.tenant = Some("key-ref:other/2026-09".to_string());
        foreign.seal();
        let faults = Validate::binding(&foreign, &row);
        assert!(faults.iter().any(|fault| matches!(
            fault,
            Fault::IdentityMix { field, .. } if field == "tenant"
        )));
        assert!(
            !faults
                .iter()
                .any(|fault| matches!(fault, Fault::Mismatch { .. }))
        );
    }

    #[test]
    fn a_digest_mismatch_between_receipt_and_step_is_a_typed_fault() {
        let row = joined(&answered());
        let mut drifted = answered();
        drifted.request_digest = digest_of('z');
        drifted.seal();
        let faults = Validate::binding(&drifted, &row);
        assert!(faults.iter().any(|fault| matches!(
            fault,
            Fault::Mismatch { field, .. } if field == "request_digest"
        )));
    }

    #[test]
    fn the_receipts_references_must_name_the_steps() {
        let mut row = joined(&answered());
        row.references.policy = Some("other-program".to_string());
        let faults = Validate::binding(&answered(), &row);
        assert!(faults.iter().any(|fault| matches!(
            fault,
            Fault::Mismatch { field, .. } if field == "policy"
        )));

        let mut evaluated = answered();
        evaluated.evaluation = Some(Evaluation {
            suite: "caller-v1".to_string(),
            suite_digest: digest_of('e'),
            question_digest: None,
            gate_digest: None,
            item: Some("item-x".to_string()),
            partition: None,
        });
        evaluated.seal();
        let faults = Validate::binding(&evaluated, &joined(&evaluated));
        assert!(faults.iter().any(|fault| matches!(
            fault,
            Fault::Mismatch { field, .. } if field == "work_item"
        )));
    }

    #[test]
    fn a_pre_identity_receipt_reports_legacy_with_the_fields_named() {
        let mut legacy = answered();
        legacy.attempt_id = String::new();
        legacy.request_digest = String::new();
        legacy.seal();
        let joined = Join::of(vec![step()], vec![legacy.clone()]);
        let report = Validate::report(&joined, &[legacy]);
        let Support::Legacy { reasons } = &report.steps[0].support else {
            panic!("a receipt without identity fields is legacy");
        };
        assert!(reasons.iter().any(|reason| reason.contains("attempt_id")));
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("request_digest"))
        );
        assert_eq!(report.counts.legacy, 1);
    }

    #[test]
    fn a_self_consistent_receipt_is_legacy_not_verified() {
        // Every digest binds and nothing contradicts — but the format
        // carries no issuer authentication, so the claim is legacy with
        // the reason named, not verified.
        let mut caller = step();
        caller.origin_authenticated = false;
        let joined = Join::of(vec![caller], vec![answered()]);
        let report = Validate::report(&joined, &[answered()]);
        let Support::Legacy { reasons } = &report.steps[0].support else {
            panic!("a self-consistent digest alone is not verified");
        };
        assert!(reasons.iter().any(|reason| reason.contains("origin")));
    }

    #[test]
    fn a_step_no_receipt_claims_reports_absent() {
        let joined = Join::of(vec![step()], vec![]);
        let report = Validate::report(&joined, &[]);
        assert_eq!(report.steps[0].support, Support::Absent);
        assert_eq!(report.counts.absent, 1);
    }

    #[test]
    fn the_report_is_deterministic() {
        let mut other_step = step();
        other_step.step = "step-4".to_string();
        other_step.request = "req-2".to_string();
        other_step.request_digest = digest_of('w');
        other_step.result_digest = None;
        let mut refused = answered();
        refused.request = "req-2".to_string();
        refused.attempt_id = "att-2".to_string();
        refused.request_digest = digest_of('w');
        refused.outcome = Outcome::Refused;
        refused.cause = Some("the door said no".to_string());
        refused.result_digest = None;
        refused.seal();

        let first = Join::of(
            vec![step(), other_step.clone()],
            vec![answered(), refused.clone()],
        );
        let second = Join::of(vec![other_step, step()], vec![refused.clone(), answered()]);
        let one = Validate::report(&first, &[answered(), refused.clone()]);
        let two = Validate::report(&second, &[refused, answered()]);
        assert_eq!(
            serde_json::to_string(&one).expect("a report serializes"),
            serde_json::to_string(&two).expect("a report serializes")
        );
    }
}
