//! The receipt join: where a trace's steps meet the service's claims.
//!
//! A step records that a decision call happened; a receipt is what the
//! service says it did. Neither alone is accountable — the step is the
//! caller's word, the receipt the issuer's. The join is where the two
//! become one claim a reader can check: this step, that attempt, these
//! digests.
//!
//! What does not pair is evidence too. A step no receipt claims is
//! `unreceipted` — a call the service never owned up to. A receipt no
//! step claims is `orphaned` — work or a charge against a call nobody
//! recorded. An unmatched receipt is a question, not noise, and the
//! join reports both kinds rather than dropping them.
//!
//! The module does no I/O and reads no format: the caller supplies step
//! references and receipts already parsed, and the join is pure data.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::execution::{
    Evaluation, ExecutionReceipt, Outcome, ReceiptError, Registry, SCHEMA, Served, Timing,
};

/// What the caller's trace knows about one step's decision call.
///
/// The identity tuple says where in the session the call happened; the
/// rest is what the caller itself observed — the request it sent, the
/// digests it computed, whether its transport authenticated the
/// answering side, and what its accounting settled. The join checks the
/// caller's word against the receipt's; it never fills in what the
/// caller did not record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StepRef {
    /// The session the step belongs to.
    pub session: String,
    /// The turn inside the session.
    pub turn: String,
    /// The program the step ran under — empty when no program ran.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub program: String,
    /// The step's own identity inside its program or turn.
    pub step: String,
    /// The decision function the step invoked — empty when the call
    /// was not a named function.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub function: String,
    /// The work item the step served — empty when it served none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub work_item: String,
    /// The logical request id the step's call carried — the same
    /// identity the receipt's `request` field claims.
    pub request: String,
    /// Which attempt of that request this step records, one-based.
    pub attempt: u32,
    /// The digest the caller computed over its own request envelope —
    /// the caller's half of the request binding the receipt's
    /// `request_digest` claims. Empty means the caller kept no digest,
    /// and the binding cannot be corroborated.
    pub request_digest: String,
    /// The digest over the response body the caller received, when it
    /// received one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_digest: Option<String>,
    /// The job reference the caller published, when the transport has
    /// one — on the relay lane, the request event's id, which the
    /// receipt records as `attempt_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    /// Whether the caller's transport authenticated the answering
    /// origin — a relay result's verified signature, a keyed endpoint's
    /// authenticated channel. The join does not infer it; a receipt's
    /// own self-consistent digest is not issuer authentication.
    pub origin_authenticated: bool,
    /// What the call cost the caller, when anyone can say.
    pub cost: Cost,
}

/// What a call cost, said exactly.
///
/// Amounts are unsigned integer millionths of an explicitly named
/// currency unit — the monetary ledger's own unit, so a joined cost and
/// a settled charge agree without conversion. `Unknown` is a statement,
/// not a zero: a cost nobody settled is not a call that was free.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cost {
    /// A settled, accounted amount.
    Known {
        /// Millionths of the named currency unit.
        millionths: u64,
        /// The currency the amount is denominated in.
        currency: String,
    },
    /// No settled amount exists — unpriced, unsettled, or the service
    /// could not say. Never scored as free.
    Unknown,
}

/// Every reference a receipt carries, copied into the row.
///
/// The caller's own side lives on [`StepRef`]; this side is the
/// service's word — which request and attempt answered, under which
/// admission, bound to which identities.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct References {
    /// The logical request the attempt belongs to.
    pub request: String,
    /// The digest of the canonical request envelope the attempt served.
    pub request_digest: String,
    /// Which attempt of the request, one-based.
    pub attempt: u32,
    /// The attempt's own identity.
    pub attempt_id: String,
    /// The transport that carried the call.
    pub transport: String,
    /// The job reference, when the lane carries one. On `relay` the
    /// attempt's published request event is the job, and the receipt
    /// records its id as `attempt_id`; a direct call has no separate
    /// job identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    /// The identity the caller asked for.
    pub requested: Served,
    /// The identity that answered — model, adapter, artifact digest,
    /// and execution settings as the service claims them.
    pub served: Served,
    /// The authorized tenant reference, when the call had one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    /// The decision policy the call ran under, when the receipt names
    /// one. The v1 receipt carries no separate policy identity — a
    /// declared policy is inside the request envelope `request_digest`
    /// covers — so this stays empty until a schema carries it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    /// The registry revision the call was admitted under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<Registry>,
    /// The accounting reference the attempt settled against — the
    /// quota reservation or usage record the ledger knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
    /// The evaluation context, when the call carried one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<Evaluation>,
}

/// How far a joined row's evidence goes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verification {
    /// Everything the join can check, checked: the schema is this
    /// version's, the receipt's digest recomputes over its contents,
    /// the caller's request and result digests bind, the identities
    /// agree, and the caller's transport authenticated the origin.
    /// Still an attributable claim — never remote attestation.
    Verified,
    /// The receipt is self-consistent — its digest recomputes and no
    /// digest the caller supplied contradicts it — but the row cannot
    /// say more: no issuer authentication was available, or a binding
    /// the caller never recorded cannot be corroborated. A
    /// self-consistent digest is not issuer authentication and not
    /// remote attestation.
    SelfConsistent,
    /// A legacy receipt that cannot carry what this join validates —
    /// another schema version, or the identity fields absent. Nothing
    /// failed; the check was never available, and the row says so
    /// rather than pretending a verified receipt.
    Unsupported,
}

/// One matched pair: the step and what the receipt claims for it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Row {
    /// The step the receipt paired with, as the caller recorded it.
    pub step: StepRef,
    /// The receipt's own digest — the identity a reader quotes for
    /// this call.
    pub receipt_digest: String,
    /// The references the receipt carries.
    pub references: References,
    /// What the attempt came to — `answered`, `refused`,
    /// `unavailable`, `unattempted`, or `unknown`, preserved exactly.
    /// A row never smudges one outcome into another.
    pub outcome: Outcome,
    /// The refusal or failure cause, when the outcome carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    /// The timing the attempt carried.
    pub timing: Timing,
    /// What the call cost — the caller's accounting, kept exact.
    pub cost: Cost,
    /// How far the row's evidence goes.
    pub verification: Verification,
}

/// Why a record could not pair, or a pairing could not be trusted.
///
/// Every problem names the offending identity — a join that cannot say
/// which records disagree is noise, not evidence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Problem {
    /// The receipt cannot support a join — it names no request, so no
    /// step can claim it and nothing can be checked.
    MalformedReceipt {
        /// The attempt identity the receipt claims, when it claims one.
        attempt_id: String,
        /// The request the receipt claims, when it claims one.
        request: String,
        /// The attempt number the receipt claims.
        attempt: u32,
        /// Which required identity the receipt lacks.
        cause: String,
    },
    /// The receipt's own digest does not recompute over its contents —
    /// self-consistency failed, so every claim in it is suspect.
    InconsistentDigest {
        /// The request the suspect receipt claims.
        request: String,
        /// The attempt the suspect receipt claims.
        attempt: u32,
        /// The attempt identity the suspect receipt claims.
        attempt_id: String,
    },
    /// A step's record and the receipt claiming the same attempt
    /// disagree — same pair, different claim.
    MismatchedIdentity {
        /// The step whose record disagrees.
        step: Box<StepRef>,
        /// The request both sides claim.
        request: String,
        /// The attempt both sides claim.
        attempt: u32,
        /// The receipt field the disagreement is on.
        field: String,
    },
    /// A request/attempt pair claimed more than once — on either side
    /// the pairing is ambiguous, and an ambiguous join is a wrong join.
    DuplicateRequestAttempt {
        /// The request claimed more than once.
        request: String,
        /// The attempt claimed more than once.
        attempt: u32,
    },
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedReceipt {
                attempt_id,
                request,
                cause,
                ..
            } => write!(
                f,
                "receipt `{attempt_id}` ({request}) is malformed: it names no {cause}"
            ),
            Self::InconsistentDigest {
                request,
                attempt,
                attempt_id,
            } => write!(
                f,
                "receipt `{attempt_id}` ({request} attempt {attempt}) does not recompute \
                 over its contents"
            ),
            Self::MismatchedIdentity {
                step,
                request,
                attempt,
                field,
            } => write!(
                f,
                "step `{}/{}/{}` and the receipt for {request} attempt {attempt} disagree \
                 on `{field}`",
                step.session, step.turn, step.step
            ),
            Self::DuplicateRequestAttempt { request, attempt } => {
                write!(f, "{request} attempt {attempt} was claimed more than once")
            }
        }
    }
}

/// The join itself: caller-supplied steps against the receipts a
/// service emitted, once.
pub struct Join;

impl Join {
    /// Pair step references with receipts on the request/attempt
    /// identity both sides carry.
    ///
    /// Everything supplied is accounted for in the returned [`Joined`]:
    /// matched pairs become rows, steps no receipt claimed are
    /// `unreceipted`, receipts no step claimed are `orphaned`, and
    /// records that could not pair — or whose pairing cannot be
    /// trusted — are named in `problems()`.
    #[must_use]
    pub fn of(steps: Vec<StepRef>, receipts: Vec<ExecutionReceipt>) -> Joined {
        // Index both sides by (request, attempt). A record with no
        // request identity cannot be claimed at all.
        let mut step_keys: BTreeMap<(String, u32), Vec<usize>> = BTreeMap::new();
        let mut receipt_keys: BTreeMap<(String, u32), Vec<usize>> = BTreeMap::new();
        for (index, step) in steps.iter().enumerate() {
            if !step.request.is_empty() {
                step_keys
                    .entry((step.request.clone(), step.attempt))
                    .or_default()
                    .push(index);
            }
        }
        for (index, receipt) in receipts.iter().enumerate() {
            if !receipt.request.is_empty() {
                receipt_keys
                    .entry((receipt.request.clone(), receipt.attempt))
                    .or_default()
                    .push(index);
            }
        }

        // A pair claimed more than once — by either side — is ambiguous
        // and joins nothing.
        let mut problems = Vec::new();
        let mut duplicated = BTreeSet::new();
        for (key, _) in step_keys
            .iter()
            .chain(receipt_keys.iter())
            .filter(|(_, indexes)| indexes.len() > 1)
        {
            if duplicated.insert(key.clone()) {
                problems.push(Problem::DuplicateRequestAttempt {
                    request: key.0.clone(),
                    attempt: key.1,
                });
            }
        }

        let mut rows = Vec::new();
        let mut unreceipted = Vec::new();
        let mut claimed = BTreeSet::new();
        let mut reported = BTreeSet::new();
        for step in &steps {
            let key = (step.request.clone(), step.attempt);
            let joinable = !step.request.is_empty()
                && !duplicated.contains(&key)
                && receipt_keys
                    .get(&key)
                    .is_some_and(|indexes| indexes.len() == 1);
            if !joinable {
                unreceipted.push(step.clone());
                continue;
            }
            let index = receipt_keys[&key][0];
            match pair(step, &receipts[index]) {
                Ok(row) => {
                    rows.push(row);
                    claimed.insert(index);
                }
                Err(problem) => {
                    problems.push(problem);
                    unreceipted.push(step.clone());
                    reported.insert(index);
                }
            }
        }

        let mut orphaned = Vec::new();
        for (index, receipt) in receipts.iter().enumerate() {
            if claimed.contains(&index) {
                continue;
            }
            if receipt.request.is_empty() {
                problems.push(Problem::MalformedReceipt {
                    attempt_id: receipt.attempt_id.clone(),
                    request: receipt.request.clone(),
                    attempt: receipt.attempt,
                    cause: "request identity".to_string(),
                });
            } else if !reported.contains(&index)
                && receipt.v == SCHEMA
                && receipt.digest != receipt.compute_digest()
            {
                problems.push(Problem::InconsistentDigest {
                    request: receipt.request.clone(),
                    attempt: receipt.attempt,
                    attempt_id: receipt.attempt_id.clone(),
                });
            }
            orphaned.push(receipt.clone());
        }

        Joined {
            rows,
            unreceipted,
            orphaned,
            problems,
        }
    }
}

/// What one step and the receipt claiming its attempt come to: a row,
/// or a problem that explains why no row exists.
fn pair(step: &StepRef, receipt: &ExecutionReceipt) -> Result<Row, Problem> {
    let references = references_of(receipt);
    match receipt.verify() {
        Err(ReceiptError::Tampered) => {
            return Err(Problem::InconsistentDigest {
                request: receipt.request.clone(),
                attempt: receipt.attempt,
                attempt_id: receipt.attempt_id.clone(),
            });
        }
        // Another schema's receipt, or one missing the fields this
        // version checks, still pairs — the row reports that the check
        // was never available.
        Err(ReceiptError::UnknownSchema(_)) | Err(ReceiptError::Missing(_)) => {
            return Ok(row(step, receipt, references, Verification::Unsupported));
        }
        Err(ReceiptError::Malformed(_)) | Ok(()) => {}
    }

    // The caller's recorded digests either corroborate the receipt or
    // contradict it. A contradiction is a problem, not a join; a digest
    // the caller never recorded simply cannot be checked.
    if !step.request_digest.is_empty() && step.request_digest != receipt.request_digest {
        return Err(mismatch(step, receipt, "request_digest"));
    }
    match (&step.result_digest, &receipt.result_digest) {
        (Some(caller), Some(service)) if caller != service => {
            return Err(mismatch(step, receipt, "result_digest"));
        }
        (Some(_), None) => return Err(mismatch(step, receipt, "result_digest")),
        _ => {}
    }
    if step
        .job
        .as_ref()
        .is_some_and(|job| job != &receipt.attempt_id)
    {
        return Err(mismatch(step, receipt, "attempt_id"));
    }

    let verified = !step.request_digest.is_empty()
        && step.request_digest == receipt.request_digest
        && step.result_digest == receipt.result_digest
        && step.origin_authenticated;
    let verification = if verified {
        Verification::Verified
    } else {
        Verification::SelfConsistent
    };
    Ok(row(step, receipt, references, verification))
}

fn mismatch(step: &StepRef, receipt: &ExecutionReceipt, field: &'static str) -> Problem {
    Problem::MismatchedIdentity {
        step: Box::new(step.clone()),
        request: receipt.request.clone(),
        attempt: receipt.attempt,
        field: field.to_string(),
    }
}

fn row(
    step: &StepRef,
    receipt: &ExecutionReceipt,
    references: References,
    verification: Verification,
) -> Row {
    Row {
        step: step.clone(),
        receipt_digest: receipt.digest.clone(),
        references,
        outcome: receipt.outcome,
        cause: receipt.cause.clone(),
        timing: receipt.timing.clone(),
        cost: step.cost.clone(),
        verification,
    }
}

/// Copy every reference the receipt carries into the row's own record.
fn references_of(receipt: &ExecutionReceipt) -> References {
    References {
        request: receipt.request.clone(),
        request_digest: receipt.request_digest.clone(),
        attempt: receipt.attempt,
        attempt_id: receipt.attempt_id.clone(),
        transport: receipt.transport.clone(),
        job: (receipt.transport == "relay").then(|| receipt.attempt_id.clone()),
        requested: receipt.requested.clone(),
        served: receipt.served.clone(),
        tenant: receipt.tenant.clone(),
        policy: None,
        registry: receipt.registry.clone(),
        usage: receipt.usage.clone(),
        evaluation: receipt.evaluation.clone(),
    }
}

/// The result of one join: what paired, what did not, and why not.
///
/// Every record the caller supplied lands somewhere — in `rows`, in
/// `unreceipted`, or in `orphaned` — and every reason a record could
/// not pair is a [`Problem`]. Nothing is dropped, because the absence
/// of a pair is itself the evidence a later accounting needs.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Joined {
    /// The pairs that matched — one row per claimed attempt, in the
    /// order the steps were supplied.
    pub rows: Vec<Row>,
    /// Steps no receipt claimed — calls the service never owned up to.
    pub unreceipted: Vec<StepRef>,
    /// Receipts no step claimed — work or charges against calls the
    /// trace never recorded.
    pub orphaned: Vec<ExecutionReceipt>,
    problems: Vec<Problem>,
}

impl Joined {
    /// The problems the join found: malformed receipts, mismatched
    /// identities, duplicate request-attempt pairs, and digests that
    /// fail self-consistency — each typed, each naming the offending
    /// identity.
    #[must_use]
    pub fn problems(&self) -> Vec<Problem> {
        self.problems.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_matched_step_and_receipt_verify() {
        let joined = Join::of(vec![step()], vec![answered()]);
        assert!(joined.unreceipted.is_empty());
        assert!(joined.orphaned.is_empty());
        assert!(joined.problems().is_empty());
        let row = &joined.rows[0];
        assert_eq!(row.verification, Verification::Verified);
        assert_eq!(row.step.session, "ses-1");
        assert_eq!(row.references.request, "req-1");
        assert_eq!(row.references.attempt, 1);
        assert_eq!(row.references.attempt_id, "att-1");
        assert_eq!(
            row.references.registry,
            Some(Registry {
                digest: digest_of('r'),
                sequence: 3
            })
        );
        assert_eq!(
            row.references.tenant.as_deref(),
            Some("key-ref:acme/2026-09")
        );
        assert_eq!(row.references.usage.as_deref(), Some("resv-881"));
        assert_eq!(row.references.served.artifact_signature, digest_of('a'));
        assert_eq!(row.outcome, Outcome::Answered);
        assert_eq!(row.timing.latency_ms, Some(87));
    }

    #[test]
    fn a_relay_receipt_names_its_job() {
        let mut receipt = answered();
        receipt.transport = "relay".to_string();
        receipt.seal();
        let joined = Join::of(vec![step()], vec![receipt]);
        assert_eq!(joined.rows[0].references.job.as_deref(), Some("att-1"));
    }

    #[test]
    fn a_mismatched_request_digest_is_a_problem_not_a_join() {
        let mut claiming = step();
        claiming.request_digest = digest_of('z');
        let joined = Join::of(vec![claiming], vec![answered()]);
        assert!(joined.rows.is_empty());
        assert_eq!(joined.unreceipted.len(), 1);
        assert_eq!(joined.orphaned.len(), 1);
        assert!(matches!(
            joined.problems().as_slice(),
            [Problem::MismatchedIdentity { field, .. }] if field == "request_digest"
        ));
    }

    #[test]
    fn a_legacy_receipt_is_unsupported_not_verified() {
        let mut legacy = answered();
        legacy.v = "openagents.receipt.execution.v0".to_string();
        let joined = Join::of(vec![step()], vec![legacy]);
        assert_eq!(joined.rows.len(), 1);
        assert_eq!(joined.rows[0].verification, Verification::Unsupported);
        assert!(joined.problems().is_empty());
    }

    #[test]
    fn a_receipt_without_identity_fields_is_unsupported() {
        let mut legacy = answered();
        legacy.attempt_id = String::new();
        legacy.request_digest = String::new();
        legacy.seal();
        let joined = Join::of(vec![step()], vec![legacy]);
        assert_eq!(joined.rows.len(), 1);
        assert_eq!(joined.rows[0].verification, Verification::Unsupported);
    }

    #[test]
    fn an_unreceipted_step_and_an_orphaned_receipt_both_report() {
        let mut other = answered();
        other.request = "req-2".to_string();
        other.seal();
        let joined = Join::of(vec![step()], vec![other]);
        assert!(joined.rows.is_empty());
        assert_eq!(joined.unreceipted.len(), 1);
        assert_eq!(joined.unreceipted[0].request, "req-1");
        assert_eq!(joined.orphaned.len(), 1);
        assert_eq!(joined.orphaned[0].request, "req-2");
        assert!(joined.problems().is_empty());
    }

    #[test]
    fn a_duplicate_request_attempt_pair_reports() {
        let mut twin = answered();
        twin.attempt_id = "att-1b".to_string();
        twin.seal();
        let joined = Join::of(vec![step()], vec![answered(), twin]);
        assert!(joined.rows.is_empty());
        assert_eq!(joined.unreceipted.len(), 1);
        assert_eq!(joined.orphaned.len(), 2);
        assert!(matches!(
            joined.problems().as_slice(),
            [Problem::DuplicateRequestAttempt { request, attempt }]
                if request == "req-1" && *attempt == 1
        ));
    }

    #[test]
    fn a_tampered_receipt_fails_self_consistency() {
        let mut receipt = answered();
        receipt.served.artifact_signature = digest_of('f');
        let joined = Join::of(vec![step()], vec![receipt]);
        assert!(joined.rows.is_empty());
        assert_eq!(joined.unreceipted.len(), 1);
        assert_eq!(joined.orphaned.len(), 1);
        assert!(matches!(
            joined.problems().as_slice(),
            [Problem::InconsistentDigest { .. }]
        ));
    }

    #[test]
    fn outcomes_survive_the_join_exactly() {
        for outcome in [
            Outcome::Refused,
            Outcome::Unavailable,
            Outcome::Unattempted,
            Outcome::Unknown,
        ] {
            let mut receipt = answered();
            receipt.outcome = outcome;
            if outcome != Outcome::Answered {
                receipt.cause = Some("the door said no".to_string());
                receipt.result_digest = None;
            }
            receipt.seal();
            let mut caller = step();
            caller.result_digest = receipt.result_digest.clone();
            let joined = Join::of(vec![caller], vec![receipt]);
            assert_eq!(joined.rows[0].outcome, outcome);
            if outcome != Outcome::Answered {
                assert_eq!(joined.rows[0].cause.as_deref(), Some("the door said no"));
            }
        }
    }

    #[test]
    fn an_unknown_cost_stays_unknown() {
        let mut caller = step();
        caller.cost = Cost::Unknown;
        let joined = Join::of(vec![caller], vec![answered()]);
        assert_eq!(joined.rows[0].cost, Cost::Unknown);

        let joined = Join::of(vec![step()], vec![answered()]);
        assert_eq!(
            joined.rows[0].cost,
            Cost::Known {
                millionths: 42,
                currency: "USD".to_string()
            }
        );
    }

    #[test]
    fn a_self_consistent_receipt_is_not_issuer_authentication() {
        let mut caller = step();
        caller.origin_authenticated = false;
        let joined = Join::of(vec![caller], vec![answered()]);
        // Every digest binds and nothing contradicts — but no transport
        // authentication exists, and the row says so rather than
        // claiming verified.
        assert_eq!(joined.rows[0].verification, Verification::SelfConsistent);
    }
}
