//! Candidate admission: a frozen plan, the evidence it is judged on, and
//! the digested decision a registry activates.
//!
//! `gym regress` answers whether a door moved against itself, and it stays
//! the same-identity guard. This module answers the neighbouring question
//! the regression guard refuses on purpose: whether a *different* artifact
//! — a trained adapter, a new checkpoint — may take a door's place. The two
//! are different accusations, so they are different code. The regression
//! guard holds the identity fixed and reads the difference; an admission
//! declares the difference up front and holds everything else fixed.
//!
//! # The plan is frozen before the comparison
//!
//! A [`Plan`] pins, before a number is read:
//!
//! - the two identities under comparison — [`Pinned::identity`] is the
//!   `door_identity` every evidence row must carry, and
//!   [`Plan::differences`] names the identity fields the two pins may
//!   disagree on. A field that moved without being declared is an
//!   unapproved change, and the decision is [`Ruling::Refused`];
//! - the workload — suite name and digest, question-set id and digest, the
//!   development partitions the selection is read from, and the gate digest
//!   when one is pinned;
//! - the instrument — estimator, draws, seed block, option order. These are
//!   never in `differences`: the same items asked under a different trial
//!   are two measurements, so instrument drift is refused outright;
//! - the [`Rule`]: the metrics a win may be earned on, each with the
//!   block-to-block spread *measured on this suite* it is judged against,
//!   and the effect size a win must clear. Nothing here imports a universal
//!   floor: a bound nobody measured is [`Basis::Unmeasured`], and a win on
//!   such a metric is [`Verdict::Unverifiable`], never passed on a
//!   historical number borrowed from a different workload;
//! - the guards — family regression, confident errors, new refusals,
//!   declared-selection coverage, calibration, transfer, and deployment —
//!   each with its own stated bound;
//! - `scope`, the families the admission covers, which is what the registry
//!   binds the activated door to.
//!
//! # Development chooses, locked confirms
//!
//! The two phases are not interchangeable. Development rows — the
//! partitions [`Workload::partitions`] names, which never include
//! `locked` — are the selection: they may be read as often as anyone likes
//! and they choose which candidate, if any, earns a confirmation. The
//! locked partition is the confirmation, and it is spent exactly once,
//! through [`LockedLedger`]. [`Plan::decide`] verifies that spend: the
//! ledger must hold one read of the suite's digest whose subject is this
//! plan's [`Plan::ledger_subject`] and whose adapter is the candidate's.
//! A locked read spent on anything else cannot confirm this candidate —
//! the evidence was consumed by another admission and the decision is
//! [`Ruling::Refused`] — and a suite that has not been read leaves the
//! decision [`Ruling::Unverifiable`], because an unconfirmed candidate
//! does not activate.
//!
//! # The decision is the record
//!
//! [`Plan::decide`] emits a [`Decision`]: the ruling, the criteria behind
//! it per phase, the evidence references it rests on (store chain heads,
//! the locked-read record, a report commitment), and the candidate identity
//! and scope the activation binds. The record is self-digested as
//! `admission:<sha256>` over the same canonicalization the tenancy manifest
//! uses. `tenancy::admission` replays this evaluator before activation; a
//! self-digested serialized claim alone is not admission evidence.
//! `docs/decision-models/candidate-admission.md` states the contract.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::ab::{Metric, MetricFloor, Rule, RuleError};
use crate::calibrate::score;
use crate::coverage::{Coverage, Expected};
use crate::eval::observations;
use crate::gate::{Bound, Budget, Criterion, Deployment, DeploymentRule, Gate, Scores, Verdict};
use crate::row::{DoorIdentity, Row};
use crate::suite::{LockedLedger, LockedRead, Partition, Suite};

/// The schema a frozen admission plan carries.
pub const PLAN_SCHEMA: &str = "openagents.gym.admission_plan.v1";

/// The schema a recorded admission decision carries.
pub const DECISION_SCHEMA: &str = "openagents.gym.admission_decision.v1";

/// The development-selection phase, as the decision names it.
pub const DEVELOPMENT_PHASE: &str = "development_selection";
/// The one-shot locked-confirmation phase.
pub const LOCKED_PHASE: &str = "locked_confirmation";
/// The transfer check's phase.
pub const TRANSFER_PHASE: &str = "transfer";
/// The deployment guard's phase.
pub const DEPLOYMENT_PHASE: &str = "deployment";

/// The `DoorIdentity` fields a plan may declare different between base and
/// candidate. Anything else on the identity is an instrument of the
/// comparison, and a difference there is refused, not declared.
const DIFFERABLE: [&str; 6] = [
    "model",
    "base_model_signature",
    "adapter",
    "artifact_signature",
    "execution",
    "verified",
];

/// What the admission decision concluded.
///
/// [`Verdict`] answers "what could be told"; this answers "may the
/// candidate activate". Only [`Ruling::Passed`] may. `Refused` is the plan
/// having been violated — drifted instruments, an undeclared identity
/// change, locked evidence spent on another admission — which is a
/// different accusation from a measured loss and is recorded as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ruling {
    /// Every criterion held; the registry may activate the candidate.
    Passed,
    /// A measurable loss, or a tie where the plan required improvement.
    Failed,
    /// The evidence could not answer: incomplete coverage, an unmeasured
    /// floor, or a confirmation that has not run.
    Unverifiable,
    /// The evidence or the spend is not the plan's: instrument drift, an
    /// undeclared identity change, or locked evidence consumed by another
    /// admission. The plan was violated rather than the candidate beaten.
    Refused,
}

impl Ruling {
    /// The word used in records.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Unverifiable => "unverifiable",
            Self::Refused => "refused",
        }
    }

    /// Whether the candidate may activate.
    #[must_use]
    pub const fn admitted(self) -> bool {
        matches!(self, Self::Passed)
    }
}

impl std::fmt::Display for Ruling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One side of the comparison, pinned before any number is read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pinned {
    /// The door name the evidence rows carry.
    pub door: String,
    /// The identity every row of this side must pin. A row carrying any
    /// other identity is evidence of a different door, and the decision
    /// refuses it.
    pub identity: DoorIdentity,
}

/// The workload the comparison is run on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workload {
    /// The suite's name.
    pub suite: String,
    /// The suite's content digest. Rows pinning another digest scored a
    /// different measurement.
    pub suite_digest: String,
    /// The question set the runs served, when the suite names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_set: Option<String>,
    /// That set's content digest. `None` pins the suite's own text — a row
    /// that records a set where none was pinned drifted, and one that
    /// records none where one was pinned did too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_digest: Option<String>,
    /// The partitions the development selection is read from. `locked` is
    /// never one of them: the confirmation spends that partition through
    /// the ledger, once.
    pub partitions: Vec<Partition>,
    /// The gate digest every row must pin, when the plan pins one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_digest: Option<String>,
}

/// The instrument both sides ran under. None of these may differ between
/// the sides or between the pin and a row: that is drift, not a difference.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instrument {
    /// The estimator that produced the raw signal.
    pub estimator: String,
    /// The draws one estimate rests on. `None` pins unreported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub samples: Option<u64>,
    /// The seed block drawn. `None` pins a door that takes no seed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_base: Option<u64>,
    /// Distinct seed blocks for a repeated comparison. When present, this
    /// replaces `seed_base`; every block must cover the full selection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seed_blocks: Vec<u64>,
    /// The option order served. `None` pins the suite's own order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permutation: Option<Vec<usize>>,
}

impl Instrument {
    fn blocks(&self) -> Vec<Option<u64>> {
        if self.seed_blocks.is_empty() {
            vec![self.seed_base]
        } else {
            self.seed_blocks.iter().copied().map(Some).collect()
        }
    }

    fn coverage(&self, rows: &[Row], expected: &Expected) -> Vec<Coverage> {
        self.blocks()
            .iter()
            .map(|seed| {
                let block: Vec<Row> = rows
                    .iter()
                    .filter(|row| row.seed_base == *seed)
                    .cloned()
                    .collect();
                Coverage::of(&block, expected.items())
            })
            .collect()
    }
}

/// The candidate must hold on a second suite, named and digested here, so
/// the win is not read only on the items the development ran over.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferGuard {
    /// The transfer suite's name.
    pub suite: String,
    /// The transfer suite's content digest.
    pub suite_digest: String,
    /// The complete non-locked selection required on the transfer suite.
    pub partitions: Vec<Partition>,
    /// The transfer question set and exact wording digest.
    pub question_set: Option<String>,
    pub question_digest: Option<String>,
    /// How many standard deviations of the winning metric's spread the
    /// transfer comparison may lose before the admission fails.
    /// Measured spread of the winning metric on this transfer suite.
    /// Development-suite spread cannot stand in for this measurement.
    pub block_sigma: Bound,
    pub max_regression_sigmas: Bound,
}

/// What the deployment can afford, stated with the rule that judges it.
///
/// The budget is part of the plan, not the rule: a batch job tolerates what
/// a router does not, and a ceiling that is absent reads as
/// [`Verdict::Unverifiable`], never as a pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentGuard {
    /// The deployment rule the profiles are judged under, with this plan's
    /// own measured latency spread rather than a shared floor.
    pub rule: DeploymentRule,
    /// The workload's ceilings.
    pub budget: Budget,
}

/// The guards every admission carries. Each is mandatory: a guard whose
/// bound is unmeasured reports [`Verdict::Unverifiable`] rather than
/// passing silently.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guards {
    /// How many more items the candidate may decline than the base did.
    pub max_new_refusals: Bound,
    /// How many more confident errors the candidate may carry. A count,
    /// judged exactly — there is no imported floor under it.
    pub max_new_confident_errors: Bound,
    /// The calibration metrics the candidate must hold — ECE, Brier, log
    /// loss — each with this suite's measured spread. A metric whose spread
    /// nobody measured cannot be judged in size: an adverse move is
    /// [`Verdict::Unverifiable`], never excused.
    pub calibration: Vec<MetricFloor>,
    /// The transfer check.
    pub transfer: TransferGuard,
    /// The deployment check.
    pub deployment: DeploymentGuard,
}

/// The frozen terms of a candidate admission.
///
/// `digest` is `admission-plan:<sha256>` over every field but itself,
/// canonicalized the way the tenancy manifest is, so a plan that cannot
/// recompute its own digest is refused before it is read — the same
/// discipline [`crate::suite::Suite::load`] keeps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// The schema tag.
    pub schema: String,
    /// The plan's id. Versioned: changing a term produces a new plan.
    pub id: String,
    /// The product question this admission answers, in one line.
    pub question: String,
    /// The door in service.
    pub base: Pinned,
    /// The door proposed to replace it.
    pub candidate: Pinned,
    /// The identity fields the two pins may disagree on — a subset of
    /// [`DIFFERABLE`]. A pinned field that differs without being declared
    /// makes the plan invalid: the difference was not approved.
    pub differences: Vec<String>,
    /// What was measured.
    pub workload: Workload,
    /// What the measurements were taken with.
    pub instrument: Instrument,
    /// The metrics a win may be earned on, this suite's own measured
    /// spreads for them, and the effect a win must clear.
    pub rule: Rule,
    /// The guards every admission carries.
    pub guards: Guards,
    /// The families the admission covers — the scope the registry binds.
    /// A family outside the suite is a plan that names evidence it cannot
    /// have.
    pub scope: Vec<String>,
    /// `admission-plan:<sha256>` over the rest of the document.
    pub digest: String,
}

/// What is wrong with a plan that contradicts itself or says nothing.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    /// The document is not a plan.
    #[error("the plan is not readable: {0}")]
    Malformed(String),
    /// The document is tagged for another schema.
    #[error("the plan is tagged {found}, which is not {PLAN_SCHEMA}")]
    Schema {
        /// The tag it carried.
        found: String,
    },
    /// The recorded digest does not recompute over the contents.
    #[error(
        "the plan's digest does not recompute over its contents: recorded {recorded}, computed {computed}"
    )]
    Tampered {
        /// The digest the file claims.
        recorded: String,
        /// The digest its contents produce.
        computed: String,
    },
    /// The plan has no id, so a decision could not name the plan it ran.
    #[error("the plan has no id; a decision has to name the plan that produced it")]
    NoId,
    /// The plan does not say what it decides.
    #[error("the plan has no question; a plan that cannot say what it decides decides too much")]
    NoQuestion,
    /// The plan admits no family scope.
    #[error("the plan's scope is empty; an admission binds the families it covered")]
    NoScope,
    /// The development selection names no open partition, or names the
    /// locked one.
    #[error(
        "the development selection must name open partitions only; the locked partition is \
         spent once, through the ledger, and `partition` never hands it out"
    )]
    BadPartitions,
    /// A declared difference is not an identity field that may move.
    #[error(
        "`{field}` is not a door-identity field a plan may declare different; the differable \
         fields are {}",
        DIFFERABLE.join(", ")
    )]
    BadDifference {
        /// The field the plan named.
        field: String,
    },
    /// The two pinned identities differ on a field the plan did not
    /// declare.
    #[error(
        "the base and candidate pins differ on `{field}`, which the plan does not declare; \
         an undeclared difference is an unapproved change"
    )]
    UndeclaredDifference {
        /// The field that moved.
        field: &'static str,
    },
    /// The winning rule contradicts itself.
    #[error("the plan's rule is not usable: {0}")]
    Rule(#[from] RuleError),
    /// A guard's bound says something impossible.
    #[error("{name}: {problem}")]
    Bound {
        /// The bound that is wrong.
        name: String,
        /// What is wrong with it.
        problem: String,
    },
}

impl Plan {
    /// Fill in `digest` over the plan's other fields.
    pub fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// `admission-plan:<sha256>` over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a plan serializes");
        value
            .as_object_mut()
            .expect("a plan is an object")
            .remove("digest");
        format!(
            "admission-plan:{}",
            hex_digest(canonicalize(&value).as_bytes())
        )
    }

    /// Read and validate a plan.
    ///
    /// A plan whose digest does not recompute is refused rather than read
    /// partially — a changed bound is a different plan, not a drifted one.
    pub fn parse(text: &str) -> Result<Self, PlanError> {
        let plan: Self =
            serde_json::from_str(text).map_err(|error| PlanError::Malformed(error.to_string()))?;
        plan.validate()?;
        Ok(plan)
    }

    /// Read a plan from a file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PlanError> {
        let text = std::fs::read_to_string(path.as_ref())
            .map_err(|error| PlanError::Malformed(error.to_string()))?;
        Self::parse(&text)
    }

    /// The checks [`Plan::parse`] runs.
    pub fn validate(&self) -> Result<(), PlanError> {
        if self.schema != PLAN_SCHEMA {
            return Err(PlanError::Schema {
                found: self.schema.clone(),
            });
        }
        let computed = self.compute_digest();
        if computed != self.digest {
            return Err(PlanError::Tampered {
                recorded: self.digest.clone(),
                computed,
            });
        }
        if self.id.trim().is_empty() {
            return Err(PlanError::NoId);
        }
        if self.question.trim().is_empty() {
            return Err(PlanError::NoQuestion);
        }
        if self.scope.is_empty() || self.scope.iter().any(|family| family.trim().is_empty()) {
            return Err(PlanError::NoScope);
        }
        if self.workload.partitions.is_empty()
            || self
                .workload
                .partitions
                .iter()
                .any(|partition| *partition == Partition::Locked)
        {
            return Err(PlanError::BadPartitions);
        }
        for field in &self.differences {
            if !DIFFERABLE.contains(&field.as_str()) {
                return Err(PlanError::BadDifference {
                    field: field.clone(),
                });
            }
        }
        if let Some(field) = identity_difference(&self.base.identity, &self.candidate.identity)
            .into_iter()
            .find(|field| !self.differences.iter().any(|declared| declared == field))
        {
            return Err(PlanError::UndeclaredDifference { field });
        }
        if self.guards.transfer.partitions.is_empty()
            || self.guards.transfer.partitions.contains(&Partition::Locked)
        {
            return Err(PlanError::BadPartitions);
        }
        if self.guards.transfer.suite_digest == self.workload.suite_digest {
            return Err(PlanError::Bound {
                name: "guards.transfer.suite_digest".into(),
                problem: "transfer requires a distinct pinned suite".into(),
            });
        }
        if !self.instrument.seed_blocks.is_empty()
            && (self.instrument.seed_base.is_some()
                || self
                    .instrument
                    .seed_blocks
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != self.instrument.seed_blocks.len())
        {
            return Err(PlanError::Bound {
                name: "instrument.seed_blocks".into(),
                problem: "declare distinct blocks without a separate seed_base".into(),
            });
        }
        self.rule.validate()?;
        if self.rule.metric_order.len() != 1 {
            return Err(PlanError::Bound {
                name: "rule.metric_order".into(),
                problem:
                    "freeze exactly one winning metric before development and locked confirmation"
                        .into(),
            });
        }
        let required_calibration = [Metric::Ece, Metric::Brier, Metric::Nll];
        if self.guards.calibration.len() != required_calibration.len()
            || required_calibration.iter().any(|metric| {
                self.guards
                    .calibration
                    .iter()
                    .filter(|floor| floor.metric == *metric)
                    .count()
                    != 1
            })
        {
            return Err(PlanError::Bound {
                name: "guards.calibration".into(),
                problem: "declare each ECE, Brier, and NLL guard exactly once".into(),
            });
        }
        check_bound(&self.guards.max_new_refusals, "guards.max_new_refusals")?;
        check_bound(
            &self.guards.max_new_confident_errors,
            "guards.max_new_confident_errors",
        )?;
        for floor in &self.guards.calibration {
            check_bound(
                &floor.block_sigma,
                &format!("guards.calibration.{} block_sigma", floor.metric),
            )?;
        }
        check_bound(
            &self.guards.transfer.block_sigma,
            "guards.transfer.block_sigma",
        )?;
        check_bound(
            &self.guards.transfer.max_regression_sigmas,
            "guards.transfer.max_regression_sigmas",
        )?;
        let deployment = &self.guards.deployment;
        for (bound, name) in [
            (
                &deployment.rule.min_calls,
                "guards.deployment.rule.min_calls",
            ),
            (
                &deployment.rule.latency_block_sigma_relative,
                "guards.deployment.rule.latency_block_sigma_relative",
            ),
            (
                &deployment.rule.regression_sigmas,
                "guards.deployment.rule.regression_sigmas",
            ),
        ] {
            check_bound(bound, name)?;
        }
        if deployment.budget.workload.trim().is_empty() {
            return Err(PlanError::Bound {
                name: "guards.deployment.budget.workload".to_string(),
                problem: "the budget must name the workload it is for".to_string(),
            });
        }
        Ok(())
    }

    /// The subject a locked read is spent under for this plan.
    ///
    /// The ledger binds a read to this string, which binds the read to this
    /// plan's digest: a read spent on another subject is evidence for
    /// another admission, and cannot confirm this candidate.
    #[must_use]
    pub fn ledger_subject(&self) -> String {
        format!("admission {} ({})", self.id, self.digest)
    }

    /// Judge the evidence and emit the decision record.
    ///
    /// The returned decision is sealed: its `digest` recomputes over its
    /// contents, which is what the registry verifies before it activates
    /// the binding the record names.
    ///
    /// # Errors
    ///
    /// Returns [`PlanError`] when the plan itself cannot stand. Problems in
    /// the evidence — drift, a spent read, missing coverage — are recorded
    /// in the decision, not raised, because the record is the point.
    pub fn decide(&self, evidence: &Evidence) -> Result<Decision, PlanError> {
        self.validate()?;

        let mut refusals: Vec<String> = Vec::new();
        let mut refs = EvidenceRefs::default();

        // The suite the evidence was measured on must be the suite the plan
        // froze, by digest: a same-named suite with moved items is a
        // different measurement wearing this plan's name.
        if evidence.suite.digest != self.workload.suite_digest
            || evidence.suite.name != self.workload.suite
        {
            refusals.push(format!(
                "the evidence was measured on `{}` at digest {}, which is not the suite the \
                 plan froze (`{}` at {})",
                evidence.suite.name,
                evidence.suite.digest,
                self.workload.suite,
                self.workload.suite_digest,
            ));
        }
        refusals.extend(check_suite_contract(
            evidence.suite,
            self.workload.question_set.as_deref(),
            self.workload.question_digest.as_deref(),
        ));
        for family in &self.scope {
            if !evidence.suite.families().contains(family) {
                refusals.push(format!(
                    "the scope names `{family}`, which suite `{}` does not hold; an admission \
                     cannot bind families the evidence never covered",
                    self.workload.suite,
                ));
            }
        }

        // Structural checks on the development evidence.
        for (side, pinned, rows) in [
            ("base", &self.base, evidence.development.base),
            ("candidate", &self.candidate, evidence.development.candidate),
        ] {
            refusals.extend(self.check_rows(side, pinned, rows, EvidenceKind::Development));
        }
        refs.development = Some(StoreRef {
            head: evidence.development.store_head.clone(),
            rows: evidence.development.base.len() + evidence.development.candidate.len(),
        });

        // The locked read must be this plan's spend, recorded for this
        // candidate's adapter — or it authorizes nothing here.
        let mut locked_read: Option<LockedRead> = None;
        if let Some(locked) = &evidence.locked {
            for (side, pinned, rows) in [
                ("base", &self.base, locked.base),
                ("candidate", &self.candidate, locked.candidate),
            ] {
                refusals.extend(self.check_rows(side, pinned, rows, EvidenceKind::Locked));
            }
            match locked.ledger.reads_of(&self.workload.suite_digest) {
                Err(error) => refusals.push(format!("the locked ledger cannot be read: {error}")),
                Ok(reads) => {
                    if reads.len() != 1 || reads.iter().any(|read| read.overrides.is_some()) {
                        refusals.push("locked confirmation requires exactly one original read; overridden or repeated exposure cannot confirm admission".into());
                    }
                    let subject = self.ledger_subject();
                    match reads.iter().find(|read| read.subject == subject) {
                        Some(read) => {
                            if read.adapter != self.candidate.identity.adapter {
                                refusals.push(format!(
                                    "the locked read of `{}` was spent for adapter `{}`, not \
                                     the candidate's `{}`; spent evidence cannot authorize a \
                                     different artifact",
                                    self.workload.suite,
                                    read.adapter,
                                    self.candidate.identity.adapter,
                                ));
                            }
                            locked_read = Some(read.clone());
                        }
                        None => {
                            let spent = reads
                                .first()
                                .map(|read| format!("`{}` at {}", read.subject, read.at))
                                .unwrap_or_else(|| "nothing yet".to_string());
                            refusals.push(format!(
                                "the locked partition of `{}` was spent on {spent}, not on \
                                 this admission ({subject}); locked evidence spent once cannot \
                                 be respent for a second candidate",
                                self.workload.suite,
                            ));
                        }
                    }
                }
            }
            refs.locked = locked_read.clone().map(|read| LockedRef {
                suite_digest: read.digest,
                subject: read.subject,
                at: read.at,
                items: read.items,
            });
        }

        // The transfer suite must be the one the plan froze.
        if let Some(transfer) = &evidence.transfer {
            refusals.extend(check_suite_contract(
                transfer.suite,
                self.guards.transfer.question_set.as_deref(),
                self.guards.transfer.question_digest.as_deref(),
            ));
            if transfer.suite.digest != self.guards.transfer.suite_digest
                || transfer.suite.name != self.guards.transfer.suite
            {
                refusals.push(format!(
                    "the transfer evidence ran `{}` at digest {}, which is not the transfer \
                     suite the plan froze (`{}` at {})",
                    transfer.suite.name,
                    transfer.suite.digest,
                    self.guards.transfer.suite,
                    self.guards.transfer.suite_digest,
                ));
            } else {
                for (side, pinned, rows) in [
                    ("base", &self.base, transfer.base),
                    ("candidate", &self.candidate, transfer.candidate),
                ] {
                    refusals.extend(self.check_rows(side, pinned, rows, EvidenceKind::Transfer));
                }
                refs.transfer = Some(StoreRef {
                    head: transfer.store_head.clone(),
                    rows: transfer.base.len() + transfer.candidate.len(),
                });
            }
        }
        refs.commitment = evidence.commitment.clone();

        if !refusals.is_empty() {
            let mut decision = Decision {
                schema: DECISION_SCHEMA.to_string(),
                plan: self.id.clone(),
                plan_digest: self.digest.clone(),
                ruling: Ruling::Refused,
                candidate: self.candidate.identity.clone(),
                scope: self.scope.clone(),
                decided_at: evidence.decided_at.clone(),
                evidence: refs,
                phases: Vec::new(),
                refusals,
                digest: String::new(),
            };
            decision.seal();
            return Ok(decision);
        }

        let mut phases = Vec::new();
        phases.push(self.judge_phase(
            DEVELOPMENT_PHASE,
            evidence.development.base,
            evidence.development.candidate,
            evidence.suite,
            &self.workload.partitions,
        ));
        phases.push(match &evidence.locked {
            Some(locked) if locked_read.is_some() => self.judge_phase(
                LOCKED_PHASE,
                locked.base,
                locked.candidate,
                evidence.suite,
                &[Partition::Locked],
            ),
            _ => PhaseOutcome {
                phase: LOCKED_PHASE.to_string(),
                verdict: Verdict::Unverifiable,
                criteria: vec![Criterion {
                    name: "the_locked_confirmation_ran".to_string(),
                    rank: 1,
                    verdict: Verdict::Unverifiable,
                    detail: "the locked partition has not been spent on this plan; \
                             development selects, and the one-shot confirmation is what \
                             admission waits on"
                        .to_string(),
                }],
            },
        });
        phases.push(self.judge_transfer(evidence));
        phases.push(self.judge_deployment(evidence));

        let verdict = Verdict::over(
            phases
                .iter()
                .flat_map(|phase| phase.criteria.iter().map(|c| c.verdict)),
        );
        let ruling = match verdict {
            Verdict::Passed => Ruling::Passed,
            Verdict::Failed => Ruling::Failed,
            Verdict::Unverifiable => Ruling::Unverifiable,
        };
        let mut decision = Decision {
            schema: DECISION_SCHEMA.to_string(),
            plan: self.id.clone(),
            plan_digest: self.digest.clone(),
            ruling,
            candidate: self.candidate.identity.clone(),
            scope: self.scope.clone(),
            decided_at: evidence.decided_at.clone(),
            evidence: refs,
            phases,
            refusals: Vec::new(),
            digest: String::new(),
        };
        decision.seal();
        Ok(decision)
    }

    /// Every structural check a side's rows must pass, each failure a
    /// refusal reason rather than a verdict.
    fn check_rows(
        &self,
        side: &str,
        pinned: &Pinned,
        rows: &[Row],
        kind: EvidenceKind,
    ) -> Vec<String> {
        let mut refusals = Vec::new();
        for row in rows {
            if let Err(error) = row.check() {
                refusals.push(format!(
                    "{side} evidence contains an invalid native row: {error}"
                ));
            }
            let at = format!("{side} row for item `{}`", row.item_id);
            if row.door != pinned.door {
                refusals.push(format!(
                    "{at} names door `{}`; the plan pins `{pinned_door}` for the {side} side",
                    row.door,
                    pinned_door = pinned.door,
                ));
            }
            if row.door_identity != pinned.identity {
                let moved = identity_difference(&pinned.identity, &row.door_identity);
                refusals.push(format!(
                    "{at} ran under a different identity ({}); a row that is not the pinned \
                     {side} door is evidence for another door",
                    moved.join(", "),
                ));
            }
            if row.estimator != self.instrument.estimator
                || row.samples != self.instrument.samples
                || !self.instrument.blocks().contains(&row.seed_base)
                || row.permutation != self.instrument.permutation
            {
                refusals.push(format!(
                    "{at} drew a different trial ({}, {} draws, seed {}, order {}) than the \
                     instrument the plan froze; the same items under a different trial are a \
                     different measurement, not evidence for this candidate",
                    row.estimator,
                    row.samples
                        .map(|draws| draws.to_string())
                        .unwrap_or_else(|| "an unreported number of".to_string()),
                    row.seed_base
                        .map(|seed| format!("block {seed}"))
                        .unwrap_or_else(|| "none".to_string()),
                    row.permutation
                        .as_ref()
                        .map(|order| format!("{order:?}"))
                        .unwrap_or_else(|| "the suite's own".to_string()),
                ));
            }
            let (suite_digest, question_set, question_digest, splits) = match kind {
                EvidenceKind::Transfer => (
                    self.guards.transfer.suite_digest.as_str(),
                    self.guards.transfer.question_set.as_deref(),
                    self.guards.transfer.question_digest.as_deref(),
                    Some(self.guards.transfer.partitions.as_slice()),
                ),
                EvidenceKind::Development => (
                    self.workload.suite_digest.as_str(),
                    self.workload.question_set.as_deref(),
                    self.workload.question_digest.as_deref(),
                    Some(self.workload.partitions.as_slice()),
                ),
                EvidenceKind::Locked => (
                    self.workload.suite_digest.as_str(),
                    self.workload.question_set.as_deref(),
                    self.workload.question_digest.as_deref(),
                    Some(&[Partition::Locked][..]),
                ),
            };
            if row.suite_digest != suite_digest {
                refusals.push(format!(
                    "{at} pins suite digest {}, which is not the digest the plan froze",
                    row.suite_digest,
                ));
            }
            if row.question_set.as_deref() != question_set
                || row.question_digest.as_deref() != question_digest
            {
                refusals.push(format!(
                    "{at} was served different question text than the plan froze; reworded \
                     text against the same items is a different instrument"
                ));
            }
            if kind != EvidenceKind::Transfer
                && let Some(gate) = &self.workload.gate_digest
                && row.gate_digest.as_deref() != Some(gate.as_str())
            {
                refusals.push(format!(
                    "{at} was judged under gate {}, not the gate the plan froze",
                    row.gate_digest.as_deref().unwrap_or("none recorded"),
                ));
            }
            if let Some(allowed) = splits
                && !allowed
                    .iter()
                    .any(|partition| partition.as_str() == row.split)
            {
                let expected = allowed
                    .iter()
                    .map(|partition| partition.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                refusals.push(format!(
                    "{at} sits in split `{}`, outside the declared {expected}; the locked \
                     partition is evidence only for the confirmation it was spent on",
                    row.split,
                ));
            }
        }
        refusals
    }

    /// Judge one phase's two row sets against the declared selection.
    ///
    /// The criteria are the shared ones: the selection was covered, both
    /// sides asked the same items, the candidate earned a win on a declared
    /// metric, no family regressed past its allowance, confident errors did
    /// not rise, and the candidate declined nothing the base answered.
    fn judge_phase(
        &self,
        phase: &str,
        base_rows: &[Row],
        candidate_rows: &[Row],
        suite: &Suite,
        partitions: &[Partition],
    ) -> PhaseOutcome {
        let expected = Expected::of(
            suite,
            partitions,
            None,
            None,
            vec![self.base.door.clone(), self.candidate.door.clone()],
        );
        let mut criteria = Vec::new();
        let expected = match expected {
            Ok(expected) => expected,
            Err(reason) => {
                criteria.push(Criterion {
                    name: "the_declared_selection_is_covered".to_string(),
                    rank: 1,
                    verdict: Verdict::Unverifiable,
                    detail: reason,
                });
                return PhaseOutcome {
                    phase: phase.to_string(),
                    verdict: Verdict::over(criteria.iter().map(|c| c.verdict)),
                    criteria,
                };
            }
        };

        let base = measure(base_rows);
        let candidate = measure(candidate_rows);

        // Coverage: the declared selection, recorded once per side.
        let (base_cov, cand_cov) = (
            self.instrument.coverage(base_rows, &expected),
            self.instrument.coverage(candidate_rows, &expected),
        );
        let covered = base_cov.iter().chain(&cand_cov).all(Coverage::complete);
        criteria.push(if covered {
            passed(
                "the_declared_selection_is_covered",
                1,
                format!(
                    "both sides recorded all {} expected items exactly once per declared seed block",
                    expected.items().len()
                ),
            )
        } else {
            let mut detail = String::new();
            for (side, coverage) in base_cov.iter().map(|c| ("base", c))
                .chain(cand_cov.iter().map(|c| ("candidate", c))) {
                if !coverage.complete() {
                    let _ = write!(
                        detail,
                        "{side}: {} missing{}, {} duplicated{}, {} unexpected{}; ",
                        coverage.missing.len(),
                        name_a_few(&coverage.missing),
                        coverage.duplicates.len(),
                        name_a_few(&coverage.duplicates),
                        coverage.unexpected.len(),
                        name_a_few(&coverage.unexpected),
                    );
                }
            }
            Criterion {
                name: "the_declared_selection_is_covered".to_string(),
                rank: 1,
                verdict: Verdict::Unverifiable,
                detail: format!(
                    "{detail}a missing item is missing, never a wrong answer, and an \
                     incomplete run cannot confirm a candidate"
                ),
            }
        });

        // Same items: the sides' denominators agree, or nothing below is
        // comparable. Completeness implies it; check it anyway so a partial
        // reading still says why nothing below is judged.
        let mut blocked = (!covered).then_some("the selection was not covered");
        if base.asked == candidate.asked {
            criteria.push(passed(
                "the_sides_asked_the_same_items",
                1,
                format!("both sides asked the same {} items", candidate.asked.len()),
            ));
        } else {
            let gone: Vec<String> = base.asked.difference(&candidate.asked).cloned().collect();
            let fresh: Vec<String> = candidate.asked.difference(&base.asked).cloned().collect();
            criteria.push(Criterion {
                name: "the_sides_asked_the_same_items".to_string(),
                rank: 1,
                verdict: Verdict::Unverifiable,
                detail: format!(
                    "the base asked {} items and the candidate {}: {:?} dropped, {:?} added; \
                     two runs over different items are two measurements",
                    base.asked.len(),
                    candidate.asked.len(),
                    gone,
                    fresh,
                ),
            });
            blocked = Some("the sides did not ask the same items");
        }

        // Seed blocks: each side must rest on at least the declared number
        // of distinct blocks, or a difference cannot be told from one draw.
        if self.rule.min_blocks_per_side.count().is_none() {
            criteria.push(Criterion {
                name: "each_side_drew_enough_seed_blocks".into(),
                rank: 1,
                verdict: Verdict::Unverifiable,
                detail: "the plan does not establish a minimum seed-block count".into(),
            });
            blocked = Some("the seed-block minimum is unmeasured");
        }
        let needed = self.rule.min_blocks_per_side.count().unwrap_or(0);
        if blocked.is_none() && (base.blocks < needed || candidate.blocks < needed) {
            criteria.push(Criterion {
                name: "each_side_drew_enough_seed_blocks".to_string(),
                rank: 1,
                verdict: Verdict::Unverifiable,
                detail: format!(
                    "the base drew {} block{} and the candidate {} against a declared minimum \
                     of {needed}; a difference over fewer blocks cannot be told from one draw",
                    base.blocks,
                    if base.blocks == 1 { "" } else { "s" },
                    candidate.blocks,
                ),
            });
            blocked = Some("a side drew too few seed blocks");
        }
        let blocked = blocked;

        // The winning metric: the first metric in the rule's order whose
        // gain clears the effect the plan declared for it. Under it, or
        // behind it, nothing else can rescue the round.
        criteria.push(self.winning_metric(&base, &candidate, blocked));
        // The family guard on the rule's first metric.
        criteria.push(self.family_guard(base_rows, candidate_rows, blocked));
        // The error guard: confident errors are a count, judged exactly.
        criteria.push(self.error_guard(&base, &candidate, blocked));
        // The refusal guard: items the base answered and the candidate
        // declined.
        criteria.push(self.refusal_guard(&base, &candidate, blocked));
        // The calibration guards, each against this suite's own spread.
        for floor in &self.guards.calibration {
            criteria.push(self.calibration_guard(floor, &base, &candidate, blocked));
        }

        PhaseOutcome {
            phase: phase.to_string(),
            verdict: Verdict::over(criteria.iter().map(|c| c.verdict)),
            criteria,
        }
    }

    /// The win the plan requires: the first metric whose gain clears its
    /// declared effect size. A tie earns nothing where improvement is
    /// required, and a positive move inside the floor is underpowered, not
    /// won.
    fn winning_metric(
        &self,
        base: &Measured,
        candidate: &Measured,
        blocked: Option<&str>,
    ) -> Criterion {
        let name = "the_candidate_earns_the_win";
        if let Some(reason) = blocked {
            return not_judged(name, 1, reason);
        }
        let mut short_of_floor: Option<(Metric, f64, f64)> = None;
        let mut unmeasured: Option<String> = None;
        for floor in &self.rule.metric_order {
            let metric = floor.metric;
            let (Some(before), Some(after)) =
                (metric.read(&base.scores), metric.read(&candidate.scores))
            else {
                continue;
            };
            let gain = metric.gain(before, after);
            let moved = format!("{metric} {before:.3} to {after:.3}, a move of {gain:+.3}");
            let Some(effect) =
                self.rule
                    .effect_size(metric, base.blocks.max(1), candidate.blocks.max(1))
            else {
                if gain > 0.0 {
                    unmeasured.get_or_insert(format!(
                        "{moved}, but nobody has measured this suite's block-to-block \
                         spread of {metric} ({}), so a win on it cannot be told from the \
                         seeds",
                        floor.block_sigma.why,
                    ));
                }
                continue;
            };
            if gain > effect {
                return passed(
                    name,
                    1,
                    format!(
                        "{moved}, clearing the declared effect of {effect:.3} on {metric}; the \
                         win is earned on the metric the plan named first"
                    ),
                );
            }
            if gain > 0.0 && short_of_floor.is_none() {
                short_of_floor = Some((metric, gain, effect));
            }
        }
        if let Some(detail) = unmeasured {
            return Criterion {
                name: name.to_string(),
                rank: 1,
                verdict: Verdict::Unverifiable,
                detail,
            };
        }
        if let Some((metric, gain, effect)) = short_of_floor {
            return Criterion {
                name: name.to_string(),
                rank: 1,
                verdict: Verdict::Unverifiable,
                detail: format!(
                    "the best move is {metric} by {gain:+.3}, inside the declared effect of \
                     {effect:.3}; the evidence is underpowered, and a positive move the seeds \
                     could have produced is not a win"
                ),
            };
        }
        Criterion {
            name: name.to_string(),
            rank: 1,
            verdict: Verdict::Failed,
            detail: "no metric moved the candidate's way; a candidate that loses or ties where \
                     the plan requires improvement does not activate"
                .to_string(),
        }
    }

    /// No family may lose more of the winning metric than the declared
    /// allowance, which is the suite's measured spread standing in for the
    /// family's — nobody has measured a per-family one, and over-refusing
    /// is the safe direction for a guard.
    fn family_guard(
        &self,
        base_rows: &[Row],
        candidate_rows: &[Row],
        blocked: Option<&str>,
    ) -> Criterion {
        let name = "no_family_regresses_beyond_the_allowance";
        if let Some(reason) = blocked {
            return not_judged(name, 2, reason);
        }
        let metric = self.rule.metric_order[0].metric;
        let mut families = Vec::new();
        for row in base_rows.iter().chain(candidate_rows) {
            if !families.contains(&row.family) {
                families.push(row.family.clone());
            }
        }
        let mut worst: Option<(String, f64, f64)> = None;
        let mut unmeasured: Option<String> = None;
        for family in &families {
            let was = measure(&of_family(base_rows, family));
            let now = measure(&of_family(candidate_rows, family));
            let (Some(before), Some(after)) = (metric.read(&was.scores), metric.read(&now.scores))
            else {
                continue;
            };
            let loss = -metric.gain(before, after);
            if loss <= 0.0 {
                continue;
            }
            let Some(allowance) =
                self.rule
                    .family_allowance(metric, was.blocks.max(1), now.blocks.max(1))
            else {
                unmeasured = Some(format!(
                    "`{family}` lost {loss:.3} of {metric} and nobody has measured this \
                     suite's spread of it, so the loss cannot be sized"
                ));
                continue;
            };
            if loss > allowance && worst.as_ref().is_none_or(|(_, worst, _)| loss > *worst) {
                worst = Some((family.clone(), loss, allowance));
            }
        }
        if let Some((family, loss, allowance)) = worst {
            return Criterion {
                name: name.to_string(),
                rank: 2,
                verdict: Verdict::Failed,
                detail: format!(
                    "`{family}` lost {loss:.3} of {metric} against an allowance of \
                     {allowance:.3}, which is the suite's measured spread standing in for the \
                     family's; a win that wrecks a family does not activate"
                ),
            };
        }
        if let Some(detail) = unmeasured {
            return Criterion {
                name: name.to_string(),
                rank: 2,
                verdict: Verdict::Unverifiable,
                detail,
            };
        }
        passed(
            name,
            2,
            format!("no family lost more of {metric} than the allowance"),
        )
    }

    /// Confident errors may not rise past the declared count. A count is
    /// judged exactly: at a fixed seed block the door was confidently wrong
    /// on that item or it was not.
    fn error_guard(
        &self,
        base: &Measured,
        candidate: &Measured,
        blocked: Option<&str>,
    ) -> Criterion {
        let name = "confident_errors_do_not_rise";
        if let Some(reason) = blocked {
            return not_judged(name, 2, reason);
        }
        let (Some(before), Some(after)) = (
            base.scores.confident_errors,
            candidate.scores.confident_errors,
        ) else {
            return Criterion {
                name: name.to_string(),
                rank: 2,
                verdict: Verdict::Unverifiable,
                detail:
                    "confident errors were not counted on both sides, and unknown is never zero"
                        .to_string(),
            };
        };
        let Some(bound) = self.guards.max_new_confident_errors.count() else {
            return not_judged(name, 2, "the guard limit has not been established");
        };
        let rise = after.saturating_sub(before);
        if rise > bound {
            return Criterion {
                name: name.to_string(),
                rank: 2,
                verdict: Verdict::Failed,
                detail: format!(
                    "confident errors {before} to {after}, a rise of {rise} against a declared \
                     bound of {bound}; no improvement anywhere buys this off"
                ),
            };
        }
        passed(
            name,
            2,
            format!("confident errors {before} to {after}, within the bound of {bound}"),
        )
    }

    /// The candidate may not decline items the base answered, past the
    /// declared count. A declined item leaves the numerator and stays in
    /// the denominator, which is the shape a guardrail change produces.
    fn refusal_guard(
        &self,
        base: &Measured,
        candidate: &Measured,
        blocked: Option<&str>,
    ) -> Criterion {
        let name = "the_candidate_declines_no_new_items";
        if let Some(reason) = blocked {
            return not_judged(name, 2, reason);
        }
        let new_refusals: Vec<_> = base
            .answered
            .iter()
            .filter(|item| candidate.refused.contains(*item))
            .cloned()
            .collect();
        let Some(bound) = self.guards.max_new_refusals.count() else {
            return not_judged(name, 2, "the guard limit has not been established");
        };
        if new_refusals.len() > bound {
            return Criterion {
                name: name.to_string(),
                rank: 2,
                verdict: Verdict::Failed,
                detail: format!(
                    "the candidate declined {} items the base answered{:?}, against a bound of \
                     {bound}; a door that starts refusing the hard items is a worse door, not \
                     a luckier one",
                    new_refusals.len(),
                    new_refusals.first(),
                ),
            };
        }
        passed(
            name,
            2,
            format!(
                "{} new refusals against a bound of {bound}",
                new_refusals.len()
            ),
        )
    }

    /// One calibration metric may not regress past the suite's own
    /// measured spread of it. Where nobody measured that spread, an adverse
    /// move is unverifiable rather than excused.
    fn calibration_guard(
        &self,
        floor: &MetricFloor,
        base: &Measured,
        candidate: &Measured,
        blocked: Option<&str>,
    ) -> Criterion {
        let metric = floor.metric;
        let name = format!("{metric}_holds_within_the_measured_spread");
        if let Some(reason) = blocked {
            return not_judged(&name, 3, reason);
        }
        let (Some(before), Some(after)) =
            (metric.read(&base.scores), metric.read(&candidate.scores))
        else {
            return Criterion {
                name,
                rank: 3,
                verdict: Verdict::Unverifiable,
                detail: format!("{metric} was not measured on both sides"),
            };
        };
        let gain = metric.gain(before, after);
        let moved = format!("{metric} {before:.3} to {after:.3}, a move of {gain:+.3}");
        let Some(sigma) = floor.block_sigma.value() else {
            return if gain >= 0.0 {
                passed(
                    &name,
                    3,
                    format!(
                        "{moved}, which is not the wrong way; nothing has measured this \
                         suite's spread of {metric}, so the size of a move is not judged"
                    ),
                )
            } else {
                Criterion {
                    name,
                    rank: 3,
                    verdict: Verdict::Unverifiable,
                    detail: format!(
                        "{moved}. Nothing has measured this suite's block-to-block spread of \
                         {metric}, so whether a loss of that size means anything cannot be told"
                    ),
                }
            };
        };
        let sigmas = self.rule.family_regression_sigmas.value().unwrap_or(0.0);
        let allowance = sigmas
            * sigma
            * (1.0 / base.blocks.max(1) as f64 + 1.0 / candidate.blocks.max(1) as f64).sqrt();
        if -gain > allowance {
            return Criterion {
                name,
                rank: 3,
                verdict: Verdict::Failed,
                detail: format!(
                    "{moved} against an allowance of {allowance:.3}: the candidate's \
                     calibration regressed further than the seeds move it"
                ),
            };
        }
        passed(
            &name,
            3,
            format!("{moved}, within the allowance of {allowance:.3}"),
        )
    }

    /// The transfer check: the candidate must hold on the second suite the
    /// plan froze, judged on the rule's first metric against the declared
    /// allowance.
    fn judge_transfer(&self, evidence: &Evidence) -> PhaseOutcome {
        let phase = TRANSFER_PHASE;
        let guard = &self.guards.transfer;
        let Some(transfer) = &evidence.transfer else {
            return PhaseOutcome {
                phase: phase.to_string(),
                verdict: Verdict::Unverifiable,
                criteria: vec![Criterion {
                    name: "the_candidate_holds_on_transfer".to_string(),
                    rank: 1,
                    verdict: Verdict::Unverifiable,
                    detail: "no transfer evidence was supplied; a win read only on the \
                             development items is not shown to transfer"
                        .to_string(),
                }],
            };
        };
        let base = measure(transfer.base);
        let candidate = measure(transfer.candidate);
        let mut criteria = Vec::new();
        let expected = Expected::of(
            transfer.suite,
            &guard.partitions,
            None,
            None,
            vec![self.base.door.clone(), self.candidate.door.clone()],
        );
        let covered = expected.as_ref().is_ok_and(|expected| {
            self.instrument
                .coverage(transfer.base, expected)
                .iter()
                .all(Coverage::complete)
                && self
                    .instrument
                    .coverage(transfer.candidate, expected)
                    .iter()
                    .all(Coverage::complete)
        });
        criteria.push(Criterion {
            name: "the_transfer_selection_is_covered".into(),
            rank: 1,
            verdict: if covered {
                Verdict::Passed
            } else {
                Verdict::Unverifiable
            },
            detail: if covered {
                "both transfer sides cover the complete declared selection".into()
            } else {
                "missing, duplicate, or unexpected transfer rows cannot establish transfer".into()
            },
        });
        let blocked = if covered && base.asked == candidate.asked && !base.asked.is_empty() {
            criteria.push(passed(
                "the_transfer_sides_asked_the_same_items",
                1,
                format!(
                    "both sides asked the same {} transfer items",
                    candidate.asked.len()
                ),
            ));
            None
        } else {
            criteria.push(Criterion {
                name: "the_transfer_sides_asked_the_same_items".to_string(),
                rank: 1,
                verdict: Verdict::Unverifiable,
                detail: format!(
                    "the base asked {} transfer items and the candidate {}; the transfer \
                     check needs one denominator",
                    base.asked.len(),
                    candidate.asked.len(),
                ),
            });
            Some("the transfer sides did not ask the same items")
        };
        let name = "the_candidate_holds_on_transfer";
        if let Some(reason) = blocked {
            criteria.push(not_judged(name, 1, reason));
        } else {
            let metric = self.rule.metric_order[0].metric;
            match (metric.read(&base.scores), metric.read(&candidate.scores)) {
                (Some(before), Some(after)) => {
                    let gain = metric.gain(before, after);
                    let moved = format!("{metric} {before:.3} to {after:.3}, a move of {gain:+.3}");
                    let allowance = guard
                        .block_sigma
                        .value()
                        .zip(guard.max_regression_sigmas.value())
                        .map(|(sigma, sigmas)| {
                            sigmas
                                * sigma
                                * (1.0 / base.blocks.max(1) as f64
                                    + 1.0 / candidate.blocks.max(1) as f64)
                                    .sqrt()
                        });
                    match allowance {
                        None if gain >= 0.0 => criteria.push(passed(
                            name,
                            1,
                            format!(
                                "{moved} on `{}`, which is not the wrong way; the transfer \
                                 bound is unmeasured, so the size is not judged",
                                guard.suite,
                            ),
                        )),
                        None => criteria.push(Criterion {
                            name: name.to_string(),
                            rank: 1,
                            verdict: Verdict::Unverifiable,
                            detail: format!(
                                "{moved} on `{}`. The transfer bound is unmeasured ({}), so \
                                 the loss cannot be sized",
                                guard.suite, guard.max_regression_sigmas.why,
                            ),
                        }),
                        Some(allowance) if -gain > allowance => criteria.push(Criterion {
                            name: name.to_string(),
                            rank: 1,
                            verdict: Verdict::Failed,
                            detail: format!(
                                "{moved} on `{}` against a transfer allowance of \
                                 {allowance:.3}: the win does not transfer",
                                guard.suite,
                            ),
                        }),
                        Some(allowance) => criteria.push(passed(
                            name,
                            1,
                            format!(
                                "{moved} on `{}`, within the transfer allowance of \
                                 {allowance:.3}",
                                guard.suite,
                            ),
                        )),
                    }
                }
                _ => criteria.push(Criterion {
                    name: name.to_string(),
                    rank: 1,
                    verdict: Verdict::Unverifiable,
                    detail: format!(
                        "{metric} was not measured on both transfer sides; unsupported \
                         evidence stays unverifiable"
                    ),
                }),
            }
        }
        PhaseOutcome {
            phase: phase.to_string(),
            verdict: Verdict::over(criteria.iter().map(|c| c.verdict)),
            criteria,
        }
    }

    /// The deployment check, judged by the plan's own deployment rule and
    /// budget — no shared latency floor is imported.
    fn judge_deployment(&self, evidence: &Evidence) -> PhaseOutcome {
        let phase = DEPLOYMENT_PHASE;
        let Some(deployment) = &evidence.deployment else {
            return PhaseOutcome {
                phase: phase.to_string(),
                verdict: Verdict::Unverifiable,
                criteria: vec![Criterion {
                    name: "the_workload_can_afford_the_candidate".to_string(),
                    rank: 1,
                    verdict: Verdict::Unverifiable,
                    detail: "no deployment evidence was supplied; a candidate the workload \
                             cannot afford does not activate, and an unmeasured price is not \
                             a price of zero"
                        .to_string(),
                }],
            };
        };
        // The plan's deployment rule stands in a gate of its own so the
        // outcome carries an id and a digest a reader can look up, and the
        // plan's budget is what the profiles are judged against.
        let gate = Gate {
            comment: None,
            schema: crate::gate::SCHEMA.to_string(),
            id: format!("{}-deployment", self.id),
            previously: Vec::new(),
            question: format!(
                "Can the {} workload afford this candidate?",
                deployment.group
            ),
            rule: crate::gate::Rule::Deployment(self.guards.deployment.rule.clone()),
        };
        let outcome = gate.judge_deployment(
            &deployment
                .clone()
                .under(self.guards.deployment.budget.clone()),
        );
        PhaseOutcome {
            phase: phase.to_string(),
            verdict: outcome.verdict,
            criteria: outcome.criteria,
        }
    }
}

/// What a side's rows measured: the items, the answers, the seed blocks
/// drawn, and the scores.
#[derive(Clone, Debug, Default)]
struct Measured {
    /// Every item the side recorded a row for.
    asked: BTreeSet<String>,
    /// Every item the door answered.
    answered: BTreeSet<(Option<u64>, String)>,
    /// Every item the door declined.
    refused: BTreeSet<(Option<u64>, String)>,
    /// How many distinct seed blocks the rows drew.
    blocks: usize,
    /// The panel of measures over the answered items.
    scores: Scores,
}

/// Reads a side's rows into a [`Measured`]. An empty or all-refused side
/// scores nothing: unknown stays unknown.
fn measure(rows: &[Row]) -> Measured {
    let mut measured = Measured {
        asked: rows.iter().map(|row| row.item_id.clone()).collect(),
        answered: rows
            .iter()
            .filter(|row| row.is_scored())
            .map(|row| (row.seed_base, row.item_id.clone()))
            .collect(),
        refused: rows
            .iter()
            .filter(|row| row.is_refused())
            .map(|row| (row.seed_base, row.item_id.clone()))
            .collect(),
        blocks: rows
            .iter()
            .filter_map(|row| row.seed_base)
            .collect::<BTreeSet<_>>()
            .len()
            .max(1),
        scores: Scores::default(),
    };
    let answered = rows.iter().filter(|row| row.is_scored()).count();
    if answered > 0 {
        measured.scores = score(&observations(rows)).scores();
    }
    measured
}

/// The rows of one family, borrowed in row order.
fn of_family<'a>(rows: &'a [Row], family: &str) -> Vec<Row> {
    rows.iter()
        .filter(|row| row.family == family)
        .cloned()
        .collect()
}

/// Which evidence a row set is for; the checks differ per kind.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EvidenceKind {
    /// The development selection.
    Development,
    /// The one-shot locked confirmation.
    Locked,
    /// The transfer check on the second suite.
    Transfer,
}

/// One side's rows for one phase.
#[derive(Clone, Debug)]
pub struct Side<'a> {
    /// The base door's rows.
    pub base: &'a [Row],
    /// The candidate door's rows.
    pub candidate: &'a [Row],
    /// The chain head of the store the rows were verified from, when they
    /// came from one. The decision records it rather than the rows.
    pub store_head: Option<String>,
}

/// The locked confirmation's evidence: the two sides' rows on the spent
/// partition and the ledger the spend was recorded in.
#[derive(Clone, Debug)]
pub struct Locked<'a> {
    /// The base door's locked rows.
    pub base: &'a [Row],
    /// The candidate door's locked rows.
    pub candidate: &'a [Row],
    /// The ledger the read was recorded in.
    pub ledger: &'a LockedLedger,
    /// The chain head of the store the rows came from, when there is one.
    pub store_head: Option<String>,
}

/// The transfer check's evidence: both sides over the plan's second suite.
#[derive(Clone, Debug)]
pub struct Transfer<'a> {
    /// The transfer suite, whose digest must be the frozen one.
    pub suite: &'a Suite,
    /// The base door's rows on it.
    pub base: &'a [Row],
    /// The candidate door's rows on it.
    pub candidate: &'a [Row],
    /// The chain head of the store the rows came from, when there is one.
    pub store_head: Option<String>,
}

/// Everything a decision is judged on. Evidence that is absent is not
/// failed — it is unverifiable, which does not activate either.
#[derive(Clone, Debug)]
pub struct Evidence<'a> {
    /// The suite the development and locked rows were scored on. Its digest
    /// must be the frozen one.
    pub suite: &'a Suite,
    /// The development selection's rows.
    pub development: Side<'a>,
    /// The locked confirmation's rows and ledger. `None` means the
    /// confirmation has not run, and an unconfirmed candidate does not
    /// activate.
    pub locked: Option<Locked<'a>>,
    /// The transfer check's rows and suite. `None` is unverifiable.
    pub transfer: Option<Transfer<'a>>,
    /// The deployment profiles and the budget they are judged against.
    /// `None` is unverifiable.
    pub deployment: Option<Deployment>,
    /// When the decision was taken, as the caller dates it.
    pub decided_at: String,
    /// The report commitment digest over the evidence stores, when one was
    /// recorded.
    pub commitment: Option<String>,
}

/// A receipt chain head and the row count it covers, as evidence.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreRef {
    /// The store's chain head at the time of the decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// How many rows the evidence held.
    pub rows: usize,
}

/// The locked read the confirmation rests on, as evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedRef {
    /// The suite digest the read was spent on.
    pub suite_digest: String,
    /// The subject the read was recorded under — this plan's.
    pub subject: String,
    /// When the read was recorded.
    pub at: String,
    /// How many items the read covered.
    pub items: usize,
}

/// What the decision rested on, by reference rather than by content.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRefs {
    /// The development store's chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub development: Option<StoreRef>,
    /// The locked read the confirmation consumed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<LockedRef>,
    /// The transfer store's chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<StoreRef>,
    /// The report commitment digest over the evidence, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment: Option<String>,
}

/// One phase's verdict and the criteria behind it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseOutcome {
    /// Which phase: `development_selection`, `locked_confirmation`,
    /// `transfer`, or `deployment`.
    pub phase: String,
    /// `failed` beats `unverifiable` beats `passed`, as every gate here.
    pub verdict: Verdict,
    /// Every criterion, in rank order.
    pub criteria: Vec<Criterion>,
}

impl PhaseOutcome {
    /// The criteria that kept the phase from passing.
    pub fn breaches(&self) -> impl Iterator<Item = &Criterion> {
        self.criteria
            .iter()
            .filter(|criterion| criterion.verdict != Verdict::Passed)
    }
}

/// The digested record a registry consumes before it activates a trained
/// binding.
///
/// `digest` is `admission:<sha256>` over every field but itself, under the
/// same canonicalization the tenancy manifest uses. The registry replays
/// the native evaluator before trusting a stored decision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// The schema tag.
    pub schema: String,
    /// The plan's id.
    pub plan: String,
    /// The plan's digest, which is what the locked read's subject binds.
    pub plan_digest: String,
    /// What the evidence concluded.
    pub ruling: Ruling,
    /// The exact artifact identity the activation binds — the pinned
    /// candidate, not a name for it.
    pub candidate: DoorIdentity,
    /// The family scope the admission covered.
    pub scope: Vec<String>,
    /// When the decision was taken.
    pub decided_at: String,
    /// The evidence the decision rests on, by reference.
    pub evidence: EvidenceRefs,
    /// One outcome per phase.
    pub phases: Vec<PhaseOutcome>,
    /// Why a refused decision was refused, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusals: Vec<String>,
    /// `admission:<sha256>` over the rest of the document.
    pub digest: String,
}

impl Decision {
    /// Fill in `digest` over the decision's other fields.
    pub fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// `admission:<sha256>` over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a decision serializes");
        value
            .as_object_mut()
            .expect("a decision is an object")
            .remove("digest");
        format!("admission:{}", hex_digest(canonicalize(&value).as_bytes()))
    }

    /// The reference a binding's `promotion` field carries.
    #[must_use]
    pub fn reference(&self) -> String {
        self.digest.clone()
    }
}

/// The identity fields two pins disagree on, as names.
fn identity_difference(before: &DoorIdentity, after: &DoorIdentity) -> Vec<&'static str> {
    let mut out = Vec::new();
    if before.model != after.model {
        out.push("model");
    }
    if before.base_model_signature != after.base_model_signature {
        out.push("base_model_signature");
    }
    if before.adapter != after.adapter {
        out.push("adapter");
    }
    if before.artifact_signature != after.artifact_signature {
        out.push("artifact_signature");
    }
    if before.execution != after.execution {
        out.push("execution");
    }
    if before.verified != after.verified {
        out.push("verified");
    }
    out
}

/// A bound's own validity, checked where a plan carries it: the value is
/// present exactly when the basis is measured or chosen, finite, and at or
/// above zero — a guard bound of zero is a real bound, unlike an effect
/// size of zero.
fn check_bound(bound: &Bound, name: &str) -> Result<(), PlanError> {
    let unmeasured = bound.basis == crate::gate::Basis::Unmeasured;
    let problem = match (bound.value, unmeasured) {
        (Some(_), true) => Some(
            "carries a value and an unmeasured basis; drop the value or name the basis that \
             backs it"
                .to_string(),
        ),
        (None, false) => Some(format!(
            "has no value and a {} basis; record it as unmeasured",
            bound.basis
        )),
        (Some(value), false) if !value.is_finite() || value < 0.0 => Some(format!(
            "must be a finite number at or above zero, got {value}"
        )),
        _ if bound.why.trim().is_empty() => {
            Some("has no provenance; say where the number came from".to_string())
        }
        _ => None,
    };
    match problem {
        Some(problem) => Err(PlanError::Bound {
            name: name.to_string(),
            problem,
        }),
        None => Ok(()),
    }
}

/// A passing criterion.
fn passed(name: &str, rank: u8, detail: String) -> Criterion {
    Criterion {
        name: name.to_string(),
        rank,
        verdict: Verdict::Passed,
        detail,
    }
}

/// A criterion nothing downstream of was judged.
fn not_judged(name: &str, rank: u8, reason: &str) -> Criterion {
    Criterion {
        name: name.to_string(),
        rank,
        verdict: Verdict::Unverifiable,
        detail: format!("not judged: {reason}"),
    }
}

/// A short suffix naming a few of the offending keys, or nothing.
fn name_a_few(items: &[(String, String)]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let names: Vec<String> = items
        .iter()
        .take(3)
        .map(|(partition, item)| format!("{partition}/{item}"))
        .collect();
    format!(" ({})", names.join(", "))
}

/// The digest of a byte string, lowercase hex.
fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest.iter() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Canonical JSON: keys sorted, whitespace gone.
///
/// This is the same canonicalization `crates/tenancy::manifest` applies to
/// the registry manifest, kept in agreement on purpose: the admission
/// decision's digest is computed by this crate when it is written and by
/// `tenancy::admission` when the registry activates it, and the two must
/// produce the same bytes for the same document. The keys are sorted here
/// rather than trusted to the map: `preserve_order` makes a `serde_json`
/// map insertion-ordered whenever a sibling crate enables it, and the
/// digest agreement must not depend on who wrote the bytes.
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

/// Revalidate public suite data at the admission boundary and bind its wording.
fn check_suite_contract(
    suite: &Suite,
    question_set: Option<&str>,
    question_digest: Option<&str>,
) -> Vec<String> {
    let mut refusals = Vec::new();
    let checked = serde_json::to_string(suite)
        .map_err(|error| error.to_string())
        .and_then(|document| Suite::load(&document).map_err(|error| error.to_string()));
    if let Err(error) = checked {
        refusals.push(format!("invalid admission suite `{}`: {error}", suite.name));
    }
    if suite.questions.as_deref() != question_set
        || question_set.is_some_and(|value| value.trim().is_empty())
        || question_set.is_some() != question_digest.is_some()
        || question_digest.is_some_and(|value| value.trim().is_empty())
    {
        refusals.push(format!(
            "suite `{}` requires its declared question set and a nonempty question digest",
            suite.name
        ));
    }
    refusals
}
