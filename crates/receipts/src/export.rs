//! The export: a claim about a run that leaves the machine.
//!
//! Everything upstream of this module stays home. A receipt is
//! digests by birth, but the joined trace still names sessions,
//! turns, steps, and the work items they served — the machine's
//! business until an operator says otherwise. An export is the
//! deliberate act of letting a claim leave: a [`Redaction`] policy
//! states which field classes travel verbatim, which leave as salted
//! digests, and which never leave at all, and a [`Consent`] names
//! what was allowed to leave rather than assuming it.
//!
//! The bundle is built so a reader can tell what it can and cannot
//! support. It carries the policy's digest — two exports under one
//! policy are comparable — the counts of what each class gave up
//! rather than the content itself, every joined record the policy let
//! through, the retention marks the source still holds under, and a
//! completeness mark that says how far the export carries the claim
//! it was made for.
//!
//! Three rules govern the format:
//!
//! - **Local by default.** [`Redaction::local`] keeps every class on
//!   the machine and exports nothing — raw repository content and
//!   local traces stay local unless an operator builds a consented
//!   bundle on purpose.
//! - **Consent names the classes.** A class the consent does not
//!   cover does not leave; the refusal names the class and no bundle
//!   degrades silently to sharing anyway.
//! - **Counts, not content.** What a class redacted is counted per
//!   class; the redacted bytes never enter the bundle.
//!
//! The module does no I/O: no filesystem, no clock, no network. The
//! same trace, policy, and consent produce the byte-identical bundle.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::execution::{Evaluation, ExecutionReceipt, Served, Timing};
use crate::join::{Joined, Problem, References, Row, StepRef};

/// The schema tag an export policy carries.
pub const POLICY_SCHEMA: &str = "openagents.receipt.export-policy.v1";

/// The schema tag an export bundle carries.
pub const BUNDLE_SCHEMA: &str = "openagents.receipt.export.v1";

/// A field class: one kind of thing a joined record can carry.
///
/// The classes partition every field the join produces — a field that
/// fits no class does not exist to the export. Consent and retention
/// speak in classes too, so what may leave and what the source still
/// holds are said in the same words as what left.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// The ids that locate a call — session, turn, step, request,
    /// attempt, attempt identity, transport, tenant, and the model
    /// and suite names the record carries.
    Identities,
    /// Every digest a record carries — request, result, artifact,
    /// registry, and evaluation digests, and the receipt's own.
    Digests,
    /// Queue, latency, and resolution fields.
    Timing,
    /// What the call came to — the outcome word, its cause, the
    /// verification mark, and the caller's origin-authentication flag.
    Outcomes,
    /// What the call cost — the caller's accounting and the usage
    /// reference it settled against.
    Costs,
    /// The trace's locator fields — program, function, work item, the
    /// evaluated item, and execution settings — which can name
    /// repository paths or task files. The class that hashes.
    Paths,
    /// Raw caller or model content — prompt text, state, file
    /// contents. The joined schema carries none, and a bundle never
    /// carries any: the class exists so a policy states the rule, not
    /// because the records can violate it.
    Content,
}

impl Class {
    /// Every class, in canonical order — what a complete policy states.
    const ALL: [Self; 7] = [
        Self::Identities,
        Self::Digests,
        Self::Timing,
        Self::Outcomes,
        Self::Costs,
        Self::Paths,
        Self::Content,
    ];
}

/// What a policy does to one class.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Treatment {
    /// The class's fields leave as they stand.
    Keep,
    /// The class's fields leave as salted digests — the same field
    /// still correlates across records and across bundles under one
    /// policy, and no plaintext crosses.
    Hash,
    /// The class does not leave. The bundle records the count, never
    /// the fields.
    Drop,
}

/// What the source still holds of a class, stated so a reader knows.
///
/// A retention mark travels in the bundle unchanged — the export
/// records the source's declaration at bundle time; keeping to it is
/// the source's business.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Retention {
    /// The source keeps the class's records.
    Keep,
    /// The source keeps only the digests — the fields behind them are
    /// gone once this bundle is built.
    DigestOnly,
    /// The source keeps the class's records until this many exports
    /// have carried them, then drops them.
    DropAfter {
        /// The exports after which the source drops the class.
        exports: u32,
    },
}

/// The redaction policy: a stated, versioned, self-digested record of
/// what may leave a joined trace.
///
/// A policy is a claim about itself before it is a rule — its digest
/// covers the name, salt, treatments, and retention marks, so two
/// exports under one policy digest are comparable and a policy that
/// was edited after sealing refuses. A class the policy does not
/// name is undeclared, and the export refuses rather than guessing: a
/// stated policy states every class.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Redaction {
    /// The schema tag.
    pub v: String,
    /// The policy's name — `local` names the identity export.
    pub name: String,
    /// The salt every hashed field digests under. Empty is legal only
    /// when no class hashes — a salted digest with no salt is an
    /// unstated digest.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub salt: String,
    /// The treatment each class gets. Every class is named; a class
    /// absent from the map refuses the bundle.
    pub classes: BTreeMap<Class, Treatment>,
    /// The retention mark each class carries into the bundle — what
    /// the source still holds, per class.
    pub retention: BTreeMap<Class, Retention>,
    /// The digest over every field above.
    pub digest: String,
}

impl Redaction {
    /// The identity export: every class dropped, every class kept at
    /// the source.
    ///
    /// Raw repository content and local traces stay local unless an
    /// operator builds a consented bundle on purpose — `local` is the
    /// policy that says so. It needs no consent, because nothing
    /// leaves; its bundle is a sealed statement of what stayed home.
    #[must_use]
    pub fn local() -> Self {
        let mut policy = Self {
            v: POLICY_SCHEMA.to_string(),
            name: "local".to_string(),
            salt: String::new(),
            classes: Class::ALL
                .into_iter()
                .map(|class| (class, Treatment::Drop))
                .collect(),
            retention: Class::ALL
                .into_iter()
                .map(|class| (class, Retention::Keep))
                .collect(),
            digest: String::new(),
        };
        policy.seal();
        policy
    }

    /// Fill in `digest` over the policy's other fields.
    pub fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// The digest over every field but `digest` — the same
    /// canonicalized key-sorted JSON over SHA-256 the receipts seal
    /// with, so a policy and a receipt are read the same way.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a policy serializes");
        value
            .as_object_mut()
            .expect("a policy is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Serialize the sealed policy as canonical JSON — the document
    /// two exports compare digests over.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a policy serializes")
    }
}

/// The consent token: the explicit statement of what may leave.
///
/// Consent is supplied, never assumed — it names the classes that may
/// cross, and a class it does not name does not cross. The bundle
/// keeps the token so a reader checks the grant rather than taking
/// the export's word for it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Consent {
    /// The classes the grant covers.
    pub classes: BTreeSet<Class>,
    /// Who or what granted the export, when the grant names anyone —
    /// an operator reference, never the credential itself.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub grantor: String,
}

impl Consent {
    /// Consent covering the named classes — the token a consented
    /// export needs.
    pub fn covering(grantor: impl Into<String>, classes: impl IntoIterator<Item = Class>) -> Self {
        Self {
            classes: classes.into_iter().collect(),
            grantor: grantor.into(),
        }
    }

    /// Consent to nothing — the token a local export needs, supplied
    /// on purpose rather than omitted.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}

/// How far the bundle supports the claim it was made for.
///
/// The mark is computed over the trace and the policy, not over the
/// records that happened to leave — a complete export of a
/// half-evidenced trace is still `partial`, and an export that cannot
/// carry the claim says `insufficient` with the reason named rather
/// than passing as thinner evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mark", rename_all = "kebab-case")]
pub enum Completeness {
    /// Every step the trace recorded joined a receipt and left under
    /// the policy.
    Complete,
    /// Some steps had no receipt — the export carries what paired and
    /// says so.
    Partial,
    /// The export cannot support the claim it was made for — the
    /// reason is named, never smudged into partial.
    Insufficient {
        /// Why the export cannot carry the claim.
        reason: String,
    },
}

/// Which side of the join a record came from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecordKind {
    /// A step and the receipt that claimed it.
    Row,
    /// A step no receipt claimed.
    Unreceipted,
    /// A receipt no step claimed.
    Orphaned,
}

/// One joined record as the policy let it leave.
///
/// The record's classes are field maps: under `keep` the values are
/// verbatim, under `hash` each is the field's salted digest, and a
/// dropped class is absent. The field names stay — they are the
/// schema's own labels, never the operator's data.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Record {
    /// Which side of the join the record came from.
    pub kind: RecordKind,
    /// The classes that left, each a field map under the policy's
    /// treatment.
    pub classes: BTreeMap<Class, BTreeMap<String, Value>>,
}

/// What one class gave up to the policy — counts, never content.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClassCount {
    /// Fields that left verbatim.
    pub kept: usize,
    /// Fields that left as salted digests.
    pub hashed: usize,
    /// Fields that did not leave at all.
    pub dropped: usize,
}

/// Why an export refused to build a bundle.
///
/// A refusal names what it refuses by class — an export that cannot
/// say which class may not leave is noise, and a bundle that shares
/// anyway is worse.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Refusal {
    /// The policy's schema tag is not this version's.
    UnknownPolicy {
        /// The tag the policy carried.
        found: String,
    },
    /// The policy's digest does not recompute over its contents — an
    /// unsealed or edited policy is not a stated policy.
    InconsistentPolicy,
    /// The policy states no treatment for a class — a class it does
    /// not name is undeclared, and undeclared is not a policy.
    Undeclared {
        /// The classes the policy never named.
        classes: Vec<Class>,
    },
    /// The policy states no retention mark for a class — the reader
    /// could not know what the source still holds.
    Unmarked {
        /// The classes with no mark.
        classes: Vec<Class>,
    },
    /// The policy hashes a class but carries no salt — a digest with
    /// no salt is an unstated digest.
    Unsalted {
        /// The classes left to hash without a salt.
        classes: Vec<Class>,
    },
    /// The policy lets content leave — prompt text, state, and file
    /// contents never export at all, under any policy.
    ContentLeaves,
    /// The consent does not cover every class the policy lets leave —
    /// each uncovered class is named, and nothing degrades to sharing
    /// anyway.
    Unconsented {
        /// The classes the consent never covered.
        classes: Vec<Class>,
    },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPolicy { found } => {
                write!(f, "export policy schema `{found}` is not `{POLICY_SCHEMA}`")
            }
            Self::InconsistentPolicy => write!(
                f,
                "the policy's digest does not recompute over its contents"
            ),
            Self::Undeclared { classes } => write!(
                f,
                "the policy states no treatment for {}",
                class_list(classes)
            ),
            Self::Unmarked { classes } => write!(
                f,
                "the policy states no retention mark for {}",
                class_list(classes)
            ),
            Self::Unsalted { classes } => write!(
                f,
                "the policy hashes {} but carries no salt",
                class_list(classes)
            ),
            Self::ContentLeaves => {
                write!(f, "the policy lets `content` leave — content never exports")
            }
            Self::Unconsented { classes } => {
                write!(f, "the consent does not cover {}", class_list(classes))
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// The bundle: the export itself, sealed.
///
/// Everything in it is what a reader needs to judge the claim — the
/// policy's digest, the consent as supplied, what each class gave up
/// in counts, the retention the source still holds under, the records
/// that left, and the completeness mark. The digest covers every
/// field but itself, so a bundle quotes its own identity.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Bundle {
    /// The schema tag.
    pub v: String,
    /// The digest of the policy the export ran under — two bundles
    /// under one digest are comparable.
    pub policy_digest: String,
    /// The policy's name.
    pub policy: String,
    /// The consent as supplied — what was allowed to leave, recorded
    /// rather than assumed.
    pub consent: Consent,
    /// What each class gave up — counts, never content.
    pub redacted: BTreeMap<Class, ClassCount>,
    /// The retention marks applied at bundle time — what the source
    /// still holds, per class.
    pub retention: BTreeMap<Class, Retention>,
    /// How far the export supports the claim it was made for.
    pub completeness: Completeness,
    /// Every joined record under the policy.
    pub records: Vec<Record>,
    /// The digest over every field above.
    pub digest: String,
}

impl Bundle {
    /// The digest over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a bundle serializes");
        value
            .as_object_mut()
            .expect("a bundle is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Serialize the sealed bundle as canonical JSON — the bytes that
    /// leave the machine.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a bundle serializes")
    }
}

/// The export itself: one joined trace, one stated policy, one
/// supplied consent — a bundle, or a refusal that names its class.
pub struct Export;

impl Export {
    /// Build a bundle from a joined trace under a stated policy and a
    /// supplied consent — or refuse, naming the class.
    ///
    /// The checks run in order: the policy must be this schema's, must
    /// recompute its own digest, must state a treatment and a
    /// retention mark for every class, must keep `content` dropped,
    /// and must carry a salt if it hashes; then the consent must
    /// cover every class the policy lets leave. Only then does the
    /// bundle build — records, counts, retention, completeness — and
    /// seal.
    pub fn bundle(
        trace: &Joined,
        policy: &Redaction,
        consent: &Consent,
    ) -> Result<Bundle, Refusal> {
        if policy.v != POLICY_SCHEMA {
            return Err(Refusal::UnknownPolicy {
                found: policy.v.clone(),
            });
        }
        if policy.digest != policy.compute_digest() {
            return Err(Refusal::InconsistentPolicy);
        }
        let undeclared: Vec<Class> = Class::ALL
            .into_iter()
            .filter(|class| !policy.classes.contains_key(class))
            .collect();
        if !undeclared.is_empty() {
            return Err(Refusal::Undeclared {
                classes: undeclared,
            });
        }
        let unmarked: Vec<Class> = Class::ALL
            .into_iter()
            .filter(|class| !policy.retention.contains_key(class))
            .collect();
        if !unmarked.is_empty() {
            return Err(Refusal::Unmarked { classes: unmarked });
        }
        if policy.classes[&Class::Content] != Treatment::Drop {
            return Err(Refusal::ContentLeaves);
        }
        let unsalted: Vec<Class> = Class::ALL
            .into_iter()
            .filter(|class| policy.classes[class] == Treatment::Hash && policy.salt.is_empty())
            .collect();
        if !unsalted.is_empty() {
            return Err(Refusal::Unsalted { classes: unsalted });
        }
        let uncovered: Vec<Class> = Class::ALL
            .into_iter()
            .filter(|class| {
                policy.classes[class] != Treatment::Drop && !consent.classes.contains(class)
            })
            .collect();
        if !uncovered.is_empty() {
            return Err(Refusal::Unconsented { classes: uncovered });
        }

        let mut redacted: BTreeMap<Class, ClassCount> = Class::ALL
            .into_iter()
            .map(|class| (class, ClassCount::default()))
            .collect();
        let mut records = Vec::new();
        for row in &trace.rows {
            push_record(
                &mut records,
                &mut redacted,
                RecordKind::Row,
                row_fields(row),
                policy,
            );
        }
        for step in &trace.unreceipted {
            push_record(
                &mut records,
                &mut redacted,
                RecordKind::Unreceipted,
                step_only_fields(step),
                policy,
            );
        }
        for receipt in &trace.orphaned {
            push_record(
                &mut records,
                &mut redacted,
                RecordKind::Orphaned,
                receipt_fields(receipt),
                policy,
            );
        }

        let mut bundle = Bundle {
            v: BUNDLE_SCHEMA.to_string(),
            policy_digest: policy.digest.clone(),
            policy: policy.name.clone(),
            consent: consent.clone(),
            redacted,
            retention: policy.retention.clone(),
            completeness: completeness(trace, policy),
            records,
            digest: String::new(),
        };
        bundle.digest = bundle.compute_digest();
        Ok(bundle)
    }
}

/// One record's fields, sorted by class before treatment speaks.
#[derive(Default)]
struct Fields {
    identities: BTreeMap<String, Value>,
    digests: BTreeMap<String, Value>,
    timing: BTreeMap<String, Value>,
    outcomes: BTreeMap<String, Value>,
    costs: BTreeMap<String, Value>,
    paths: BTreeMap<String, Value>,
}

impl Fields {
    /// The fields one class holds — content has no fields and never
    /// reaches this map.
    fn get(&self, class: Class) -> &BTreeMap<String, Value> {
        match class {
            Class::Identities => &self.identities,
            Class::Digests => &self.digests,
            Class::Timing => &self.timing,
            Class::Outcomes => &self.outcomes,
            Class::Costs => &self.costs,
            Class::Paths => &self.paths,
            Class::Content => unreachable!("content carries no fields"),
        }
    }
}

/// Insert a field — empty text stays absent, because an absent field
/// is a statement an empty string would smudge.
fn put(map: &mut BTreeMap<String, Value>, name: &str, value: impl Into<Value>) {
    map.insert(name.to_string(), value.into());
}

fn put_text(map: &mut BTreeMap<String, Value>, name: &str, text: &str) {
    if !text.is_empty() {
        put(map, name, text);
    }
}

fn put_opt(map: &mut BTreeMap<String, Value>, name: &str, value: &Option<String>) {
    if let Some(text) = value {
        put_text(map, name, text);
    }
}

/// The caller's side of a step — its locating ids, recorded digests,
/// locator fields, origin flag, and cost.
fn step_fields(fields: &mut Fields, step: &StepRef) {
    put_text(&mut fields.identities, "session", &step.session);
    put_text(&mut fields.identities, "turn", &step.turn);
    put_text(&mut fields.identities, "step", &step.step);
    put_text(&mut fields.identities, "request", &step.request);
    put(&mut fields.identities, "attempt", step.attempt);
    put_opt(&mut fields.identities, "job", &step.job);
    put_text(
        &mut fields.digests,
        "step.request_digest",
        &step.request_digest,
    );
    put_opt(
        &mut fields.digests,
        "step.result_digest",
        &step.result_digest,
    );
    put(
        &mut fields.outcomes,
        "origin_authenticated",
        step.origin_authenticated,
    );
    put_text(&mut fields.paths, "program", &step.program);
    put_text(&mut fields.paths, "function", &step.function);
    put_text(&mut fields.paths, "work_item", &step.work_item);
    put(
        &mut fields.costs,
        "cost",
        serde_json::to_value(&step.cost).expect("a cost serializes"),
    );
}

/// A matched row — both sides of the pair, plus the row's own
/// receipt digest, outcome, verification, and timing.
fn row_fields(row: &Row) -> Fields {
    let mut fields = Fields::default();
    step_fields(&mut fields, &row.step);
    reference_fields(&mut fields, &row.references);
    put_text(&mut fields.digests, "receipt_digest", &row.receipt_digest);
    put(
        &mut fields.outcomes,
        "outcome",
        serde_json::to_value(row.outcome).expect("an outcome serializes"),
    );
    put_opt(&mut fields.outcomes, "cause", &row.cause);
    put(
        &mut fields.outcomes,
        "verification",
        serde_json::to_value(row.verification).expect("a verification serializes"),
    );
    timing_fields(&mut fields, &row.timing);
    fields
}

/// An unreceipted step — the caller's side only, because the service
/// never owned up to the call.
fn step_only_fields(step: &StepRef) -> Fields {
    let mut fields = Fields::default();
    step_fields(&mut fields, step);
    fields
}

/// The receipt's side, from the references a row copied.
fn reference_fields(fields: &mut Fields, references: &References) {
    put_text(&mut fields.identities, "request", &references.request);
    put(&mut fields.identities, "attempt", references.attempt);
    put_text(&mut fields.identities, "attempt_id", &references.attempt_id);
    put_text(&mut fields.identities, "transport", &references.transport);
    put_opt(&mut fields.identities, "job", &references.job);
    put_opt(&mut fields.identities, "tenant", &references.tenant);
    put_opt(&mut fields.identities, "policy", &references.policy);
    if let Some(registry) = &references.registry {
        put_text(&mut fields.digests, "registry.digest", &registry.digest);
        put(
            &mut fields.identities,
            "registry.sequence",
            registry.sequence,
        );
    }
    served_fields(fields, "requested", &references.requested);
    served_fields(fields, "served", &references.served);
    if let Some(evaluation) = &references.evaluation {
        evaluation_fields(fields, evaluation);
    }
    put_text(
        &mut fields.digests,
        "request_digest",
        &references.request_digest,
    );
    put_opt(&mut fields.costs, "usage", &references.usage);
}

/// An orphaned receipt — the service's side only, because the trace
/// never recorded the call.
fn receipt_fields(receipt: &ExecutionReceipt) -> Fields {
    let mut fields = Fields::default();
    put_text(&mut fields.identities, "request", &receipt.request);
    put(&mut fields.identities, "attempt", receipt.attempt);
    put_text(&mut fields.identities, "attempt_id", &receipt.attempt_id);
    put_text(&mut fields.identities, "transport", &receipt.transport);
    put_opt(&mut fields.identities, "tenant", &receipt.tenant);
    if let Some(registry) = &receipt.registry {
        put_text(&mut fields.digests, "registry.digest", &registry.digest);
        put(
            &mut fields.identities,
            "registry.sequence",
            registry.sequence,
        );
    }
    served_fields(&mut fields, "requested", &receipt.requested);
    served_fields(&mut fields, "served", &receipt.served);
    if let Some(evaluation) = &receipt.evaluation {
        evaluation_fields(&mut fields, evaluation);
    }
    put_text(&mut fields.digests, "digest", &receipt.digest);
    put_text(
        &mut fields.digests,
        "request_digest",
        &receipt.request_digest,
    );
    put_opt(&mut fields.digests, "result_digest", &receipt.result_digest);
    put(
        &mut fields.outcomes,
        "outcome",
        serde_json::to_value(receipt.outcome).expect("an outcome serializes"),
    );
    put_opt(&mut fields.outcomes, "cause", &receipt.cause);
    timing_fields(&mut fields, &receipt.timing);
    put_opt(&mut fields.costs, "usage", &receipt.usage);
    fields
}

/// One identity side of a call — names go to identities, the
/// artifact digest to digests, and execution settings to paths
/// because a setting can name a file.
fn served_fields(fields: &mut Fields, side: &str, served: &Served) {
    put_text(
        &mut fields.identities,
        &format!("{side}.model"),
        &served.model,
    );
    put_opt(
        &mut fields.identities,
        &format!("{side}.adapter"),
        &served.adapter,
    );
    put_text(
        &mut fields.digests,
        &format!("{side}.artifact_signature"),
        &served.artifact_signature,
    );
    for (key, value) in &served.execution {
        put_text(&mut fields.paths, &format!("{side}.execution.{key}"), value);
    }
}

/// The evaluation context a call carried — the suite and partition
/// name identities, the pinned digests are digests, and the item is a
/// locator.
fn evaluation_fields(fields: &mut Fields, evaluation: &Evaluation) {
    put_text(
        &mut fields.identities,
        "evaluation.suite",
        &evaluation.suite,
    );
    put_opt(
        &mut fields.identities,
        "evaluation.partition",
        &evaluation.partition,
    );
    put_text(
        &mut fields.digests,
        "evaluation.suite_digest",
        &evaluation.suite_digest,
    );
    put_opt(
        &mut fields.digests,
        "evaluation.question_digest",
        &evaluation.question_digest,
    );
    put_opt(
        &mut fields.digests,
        "evaluation.gate_digest",
        &evaluation.gate_digest,
    );
    put_opt(&mut fields.paths, "evaluation.item", &evaluation.item);
}

/// The timing an attempt carried — each phase named separately, as
/// the receipt keeps it.
fn timing_fields(fields: &mut Fields, timing: &Timing) {
    if let Some(queued) = timing.queued_ms {
        put(&mut fields.timing, "queued_ms", queued);
    }
    if let Some(latency) = timing.latency_ms {
        put(&mut fields.timing, "latency_ms", latency);
    }
    put_opt(&mut fields.timing, "resolved_at", &timing.resolved_at);
}

/// Apply the policy to one record's fields: kept maps travel as they
/// stand, hashed maps travel as salted digests, dropped maps never
/// appear — and every class's giving is counted. A record nothing was
/// left of does not appear; absence is the drop's own record.
fn push_record(
    records: &mut Vec<Record>,
    redacted: &mut BTreeMap<Class, ClassCount>,
    kind: RecordKind,
    fields: Fields,
    policy: &Redaction,
) {
    let mut classes = BTreeMap::new();
    for class in Class::ALL {
        if class == Class::Content {
            continue;
        }
        let map = fields.get(class);
        if map.is_empty() {
            continue;
        }
        let count = redacted.entry(class).or_default();
        match policy.classes[&class] {
            Treatment::Keep => {
                count.kept += map.len();
                classes.insert(class, map.clone());
            }
            Treatment::Hash => {
                count.hashed += map.len();
                classes.insert(
                    class,
                    map.iter()
                        .map(|(name, value)| {
                            (
                                name.clone(),
                                Value::String(salted(&policy.salt, name, value)),
                            )
                        })
                        .collect(),
                );
            }
            Treatment::Drop => {
                count.dropped += map.len();
            }
        }
    }
    if classes.is_empty() {
        return;
    }
    records.push(Record { kind, classes });
}

/// How far the export supports the claim it was made for — computed
/// over the trace and the policy, before any single record speaks.
fn completeness(trace: &Joined, policy: &Redaction) -> Completeness {
    if trace.rows.is_empty() && trace.unreceipted.is_empty() {
        return Completeness::Insufficient {
            reason: "the trace records no steps — an export over no calls \
                     supports no claim"
                .to_string(),
        };
    }
    if policy
        .classes
        .values()
        .all(|treatment| *treatment == Treatment::Drop)
    {
        return Completeness::Insufficient {
            reason: format!(
                "the policy `{}` lets no class leave — the trace stays on the machine",
                policy.name
            ),
        };
    }
    let problems = trace.problems();
    if !problems.is_empty() {
        let mut kinds: Vec<&str> = problems.iter().map(problem_kind).collect();
        kinds.sort_unstable();
        kinds.dedup();
        return Completeness::Insufficient {
            reason: format!(
                "the join could not pair every record: {} ({})",
                problem_count(problems.len()),
                kinds.join(", ")
            ),
        };
    }
    let unnameable: Vec<Class> = [Class::Identities, Class::Digests]
        .into_iter()
        .filter(|class| policy.classes[class] == Treatment::Drop)
        .collect();
    if !unnameable.is_empty() {
        return Completeness::Insufficient {
            reason: format!(
                "the policy drops {} — the records cannot name or bind the \
                 calls they claim",
                class_list(&unnameable)
            ),
        };
    }
    if !trace.unreceipted.is_empty() {
        return Completeness::Partial;
    }
    Completeness::Complete
}

/// A join problem's kind name — counted in the completeness reason
/// without carrying the problem's identities.
fn problem_kind(problem: &Problem) -> &'static str {
    match problem {
        Problem::MalformedReceipt { .. } => "malformed-receipt",
        Problem::InconsistentDigest { .. } => "inconsistent-digest",
        Problem::MismatchedIdentity { .. } => "mismatched-identity",
        Problem::DuplicateRequestAttempt { .. } => "duplicate-request-attempt",
        Problem::OrphanRevision { .. } => "orphan-revision",
    }
}

fn problem_count(count: usize) -> String {
    if count == 1 {
        "1 problem".to_string()
    } else {
        format!("{count} problems")
    }
}

/// A class's serialized name — the same kebab word the bundle and the
/// refusal speak.
fn class_name(class: Class) -> &'static str {
    match class {
        Class::Identities => "identities",
        Class::Digests => "digests",
        Class::Timing => "timing",
        Class::Outcomes => "outcomes",
        Class::Costs => "costs",
        Class::Paths => "paths",
        Class::Content => "content",
    }
}

/// The classes a refusal or a reason names, backticked and joined.
fn class_list(classes: &[Class]) -> String {
    classes
        .iter()
        .map(|class| format!("`{}`", class_name(*class)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One field's salted digest — the salt, the field's own name, and
/// the canonical value, so the same value under a different field
/// digests differently while the same field still correlates across
/// records and across bundles under one policy.
fn salted(salt: &str, field: &str, value: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update([0]);
    hasher.update(field.as_bytes());
    hasher.update([0]);
    hasher.update(canonicalize(value).as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// Canonical JSON: keys sorted, whitespace gone — the same
/// canonicalization every digest in this workspace shares. The keys
/// are sorted here rather than trusted to the map: `preserve_order`
/// makes a `serde_json` map insertion-ordered whenever a sibling
/// crate enables it, and the digest agreement must not depend on who
/// wrote the bytes.
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
    use crate::execution::{Outcome, Registry};
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
            work_item: "src/bin/oak.rs".to_string(),
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
            lane: None,
            revises: None,
        }
    }

    fn trace() -> Joined {
        Join::of(vec![step()], vec![answered()])
    }

    /// A shareable policy: the evidence classes keep, the locators
    /// hash, and content drops — the shape an operator consents to.
    fn shareable() -> Redaction {
        let mut policy = Redaction::local();
        policy.name = "shareable".to_string();
        policy.salt = "test-salt".to_string();
        for class in [
            Class::Identities,
            Class::Digests,
            Class::Timing,
            Class::Outcomes,
            Class::Costs,
        ] {
            policy.classes.insert(class, Treatment::Keep);
        }
        policy.classes.insert(Class::Paths, Treatment::Hash);
        policy.seal();
        policy
    }

    /// Consent covering every class `shareable` lets leave.
    fn consent() -> Consent {
        Consent::covering(
            "operator:chris",
            [
                Class::Identities,
                Class::Digests,
                Class::Timing,
                Class::Outcomes,
                Class::Costs,
                Class::Paths,
            ],
        )
    }

    #[test]
    fn the_local_policy_exports_nothing() {
        let bundle = Export::bundle(&trace(), &Redaction::local(), &Consent::none()).unwrap();
        assert!(bundle.records.is_empty());
        assert_eq!(bundle.policy, "local");
        // What stayed home is counted, never carried.
        assert!(bundle.redacted[&Class::Digests].dropped > 0);
        assert!(bundle.redacted[&Class::Identities].dropped > 0);
        let json = bundle.to_json();
        for stayed in ["ses-1", "req-1", "att-1", "kev-0.6b", "src/bin/oak.rs"] {
            assert!(!json.contains(stayed), "{stayed} left the machine");
        }
        // Nothing left, so nothing can carry the claim — the mark
        // says so rather than pretending a thinner complete.
        assert!(matches!(
            bundle.completeness,
            Completeness::Insufficient { .. }
        ));
        // The identity export keeps everything at the source.
        assert!(
            bundle
                .retention
                .values()
                .all(|mark| *mark == Retention::Keep)
        );
    }

    #[test]
    fn missing_consent_refuses_by_class() {
        let consent = Consent::covering("operator:chris", [Class::Digests]);
        let refusal = Export::bundle(&trace(), &shareable(), &consent).unwrap_err();
        let Refusal::Unconsented { classes } = refusal else {
            panic!("expected an unconsented refusal, got {refusal:?}");
        };
        assert!(classes.contains(&Class::Identities));
        assert!(classes.contains(&Class::Paths));
        assert!(!classes.contains(&Class::Digests));
        // A dropped class needs no consent — content never leaves
        // anyway.
        assert!(!classes.contains(&Class::Content));
    }

    #[test]
    fn a_dropped_class_never_appears() {
        let mut policy = shareable();
        policy.classes.insert(Class::Paths, Treatment::Drop);
        policy.seal();
        let bundle = Export::bundle(&trace(), &policy, &consent()).unwrap();
        for record in &bundle.records {
            assert!(!record.classes.contains_key(&Class::Paths));
            assert!(!record.classes.contains_key(&Class::Content));
        }
        let json = bundle.to_json();
        for dropped in ["triage", "classify", "src/bin/oak.rs"] {
            assert!(!json.contains(dropped), "{dropped} left the machine");
        }
        assert!(bundle.redacted[&Class::Paths].dropped > 0);
    }

    #[test]
    fn hashed_fields_are_salted_digests_not_plaintext() {
        let bundle = Export::bundle(&trace(), &shareable(), &consent()).unwrap();
        let paths = &bundle.records[0].classes[&Class::Paths];
        assert!(!paths.is_empty());
        for (field, value) in paths {
            let digest = value.as_str().unwrap();
            assert!(
                digest.starts_with("sha256:") && digest.len() == 71,
                "{field} left as plaintext"
            );
        }
        assert_ne!(
            paths["work_item"],
            Value::String("src/bin/oak.rs".to_string())
        );
        // The salt is the policy's — another salt digests the same
        // field differently, so a leaked digest binds to its policy.
        let mut other = shareable();
        other.salt = "other-salt".to_string();
        other.seal();
        let bundle = Export::bundle(&trace(), &other, &consent()).unwrap();
        assert_ne!(
            paths["work_item"],
            bundle.records[0].classes[&Class::Paths]["work_item"]
        );
    }

    #[test]
    fn completeness_marks_partial_when_steps_lack_receipts() {
        let mut second = step();
        second.step = "step-4".to_string();
        second.request = "req-2".to_string();
        let joined = Join::of(vec![step(), second], vec![answered()]);
        let bundle = Export::bundle(&joined, &shareable(), &consent()).unwrap();
        assert_eq!(bundle.completeness, Completeness::Partial);
        // The unreceipted step still left under the policy, marked
        // for what it is.
        assert!(
            bundle
                .records
                .iter()
                .any(|record| record.kind == RecordKind::Unreceipted)
        );
    }

    #[test]
    fn completeness_marks_insufficient_when_the_claim_cannot_hold() {
        // An empty trace supports no claim.
        let empty = Join::of(Vec::new(), Vec::new());
        let bundle = Export::bundle(&empty, &shareable(), &consent()).unwrap();
        assert!(matches!(
            bundle.completeness,
            Completeness::Insufficient { .. }
        ));

        // A policy that drops identities leaves records that cannot
        // name what they claim.
        let mut nameless = shareable();
        nameless.classes.insert(Class::Identities, Treatment::Drop);
        nameless.seal();
        let bundle = Export::bundle(&trace(), &nameless, &consent()).unwrap();
        let Completeness::Insufficient { reason } = &bundle.completeness else {
            panic!("expected insufficient, got {:?}", bundle.completeness);
        };
        assert!(reason.contains("identities"));
    }

    #[test]
    fn retention_marks_are_recorded_per_class() {
        let mut policy = shareable();
        policy
            .retention
            .insert(Class::Digests, Retention::DigestOnly);
        policy
            .retention
            .insert(Class::Identities, Retention::DropAfter { exports: 3 });
        policy.seal();
        let bundle = Export::bundle(&trace(), &policy, &consent()).unwrap();
        assert_eq!(bundle.retention[&Class::Digests], Retention::DigestOnly);
        assert_eq!(
            bundle.retention[&Class::Identities],
            Retention::DropAfter { exports: 3 }
        );
        assert_eq!(bundle.retention[&Class::Paths], Retention::Keep);
    }

    #[test]
    fn identical_inputs_give_byte_identical_bundles() {
        let first = Export::bundle(&trace(), &shareable(), &consent()).unwrap();
        let second = Export::bundle(&trace(), &shareable(), &consent()).unwrap();
        assert_eq!(first.to_json(), second.to_json());
        assert_eq!(first.digest, second.digest);
    }

    #[test]
    fn an_unsealed_or_edited_policy_refuses() {
        let mut policy = shareable();
        policy.digest = String::new();
        assert!(matches!(
            Export::bundle(&trace(), &policy, &consent()),
            Err(Refusal::InconsistentPolicy)
        ));
        let mut edited = shareable();
        edited.name = "renamed".to_string();
        assert!(matches!(
            Export::bundle(&trace(), &edited, &consent()),
            Err(Refusal::InconsistentPolicy)
        ));
    }

    #[test]
    fn a_policy_that_lets_content_leave_refuses() {
        let mut policy = shareable();
        policy.classes.insert(Class::Content, Treatment::Keep);
        policy.seal();
        assert!(matches!(
            Export::bundle(&trace(), &policy, &consent()),
            Err(Refusal::ContentLeaves)
        ));
    }

    #[test]
    fn orphaned_receipts_leave_under_the_policy_too() {
        let mut orphan = answered();
        orphan.request = "req-9".to_string();
        orphan.seal();
        let joined = Join::of(vec![step()], vec![answered(), orphan]);
        let bundle = Export::bundle(&joined, &shareable(), &consent()).unwrap();
        let orphan = bundle
            .records
            .iter()
            .find(|record| record.kind == RecordKind::Orphaned)
            .unwrap();
        assert_eq!(
            orphan.classes[&Class::Identities]["request"],
            Value::String("req-9".to_string())
        );
        assert_eq!(
            orphan.classes[&Class::Digests]["request_digest"],
            Value::String(digest_of('q'))
        );
    }
}
