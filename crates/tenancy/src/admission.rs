//! Immutable candidate admission records produced by replaying Gym evidence.
//!
//! A checksum binds bytes but does not establish that their claimed ruling is
//! true. Public callers construct records through [`Record::evaluate`] or
//! [`Record::verify`], both of which run the native Gym evaluator. Loading a
//! serialized decision alone cannot authorize activation. The evaluator still
//! relies on the operator to supply authentic, protected measurement stores.

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

/// An immutable admission record constructed by native evidence replay.
#[derive(Clone, Debug)]
pub struct Record {
    base: Candidate,
    /// The plan's id.
    plan: String,
    /// The plan's digest — the identity the locked read was spent under.
    plan_digest: String,
    /// What the evidence concluded.
    ruling: Ruling,
    /// The exact artifact identity the activation binds.
    candidate: Candidate,
    /// The family scope the admission covered, which the binding carries.
    scope: Vec<String>,
    /// When the decision was taken, as the recorder dated it.
    decided_at: String,
    /// `admission:<sha256>` over the document's other fields.
    digest: String,
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
                write!(
                    f,
                    "the admission record is tagged {found}, which is not {DECISION_SCHEMA}"
                )
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
    /// Evaluate a frozen plan against native evidence before constructing a record.
    pub fn evaluate(
        plan: &gym::admission::Plan,
        evidence: &gym::admission::Evidence<'_>,
    ) -> Result<Self, Fault> {
        let decision = plan
            .decide(evidence)
            .map_err(|error| Fault::Malformed(error.to_string()))?;
        let text = serde_json::to_string(&decision)
            .map_err(|error| Fault::Malformed(error.to_string()))?;
        Self::parse_evaluated(
            &text,
            Candidate {
                model: plan.base.identity.model.clone(),
                adapter: (!plan.base.identity.adapter.is_empty())
                    .then(|| plan.base.identity.adapter.clone()),
                artifact_signature: plan.base.identity.artifact_signature.clone(),
                execution: plan.base.identity.execution.clone(),
            },
        )
    }

    /// Reload a stored decision only when replay produces the exact same record.
    pub fn verify(
        text: &str,
        plan: &gym::admission::Plan,
        evidence: &gym::admission::Evidence<'_>,
    ) -> Result<Self, Fault> {
        let evaluated = Self::evaluate(plan, evidence)?;
        let supplied = Self::parse_evaluated(text, evaluated.base.clone())?;
        if supplied.digest != evaluated.digest || supplied.document != evaluated.document {
            return Err(Fault::Denied(
                "stored admission does not match native evidence replay".into(),
            ));
        }
        Ok(evaluated)
    }

    /// The candidate identity produced by evaluation.
    pub fn candidate(&self) -> &Candidate {
        &self.candidate
    }

    /// The family scope produced by evaluation.
    pub fn scope(&self) -> &[String] {
        &self.scope
    }

    /// The pinned plan and its digest.
    pub fn plan(&self) -> (&str, &str) {
        (&self.plan, &self.plan_digest)
    }

    /// The recorded evidence evaluation time.
    pub fn decided_at(&self) -> &str {
        &self.decided_at
    }

    /// The base identity whose measured replacement was authorized.
    pub fn base(&self) -> &Candidate {
        &self.base
    }

    fn parse_evaluated(text: &str, base: Candidate) -> Result<Self, Fault> {
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
        let computed = format!(
            "admission:{:x}",
            Sha256::digest(canonicalize(&document).as_bytes())
        );
        if recorded != computed {
            return Err(Fault::Tampered { recorded, computed });
        }
        let wire: Wire = serde_json::from_value(document.clone()).map_err(|error| {
            Fault::Malformed(format!("the record is not the decision's shape: {error}"))
        })?;
        Ok(Self {
            base,
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

#[cfg(test)]
mod tests {
    use super::*;
    use gym::ab::{Metric, MetricFloor, Rule};
    use gym::admission::*;
    use gym::gate::{Basis, Bound, Budget, DeploymentRule, GatedPercentile};
    use gym::suite::{Partition, Suite};

    fn plan(suite: &Suite) -> Plan {
        let unknown = Bound {
            value: None,
            basis: Basis::Unmeasured,
            evidence: vec![],
            why: "Fixture has no measurements.".into(),
        };
        let floor = |metric| MetricFloor {
            metric,
            block_sigma: unknown.clone(),
        };
        let base = gym::row::DoorIdentity {
            model: "fixture".into(),
            base_model_signature: String::new(),
            adapter: String::new(),
            artifact_signature: format!("sha256:{}", "a".repeat(64)),
            execution: BTreeMap::new(),
            verified: true,
        };
        let mut candidate = base.clone();
        candidate.artifact_signature = format!("sha256:{}", "b".repeat(64));
        let mut plan = Plan {
            schema: PLAN_SCHEMA.into(),
            id: "fixture-v1".into(),
            question: "Does the measured candidate improve?".into(),
            base: Pinned {
                door: "base".into(),
                identity: base,
            },
            candidate: Pinned {
                door: "candidate".into(),
                identity: candidate,
            },
            differences: vec!["artifact_signature".into()],
            workload: Workload {
                suite: suite.name.clone(),
                suite_digest: suite.digest.clone(),
                question_set: None,
                question_digest: None,
                partitions: vec![Partition::Development],
                gate_digest: None,
            },
            instrument: Instrument {
                estimator: "fixture".into(),
                ..Instrument::default()
            },
            rule: Rule {
                id: "fixture-rule".into(),
                question: "Fixture only".into(),
                metric_order: vec![floor(Metric::Accuracy)],
                effect_size_sigmas: unknown.clone(),
                family_regression_sigmas: unknown.clone(),
                min_blocks_per_side: unknown.clone(),
                requeue_limit: 0,
                covers: "Nothing measured".into(),
                does_not_cover: "Quality".into(),
                pending_measurements: vec![],
            },
            guards: Guards {
                max_new_refusals: unknown.clone(),
                max_new_confident_errors: unknown.clone(),
                calibration: [Metric::Ece, Metric::Brier, Metric::Nll]
                    .into_iter()
                    .map(floor)
                    .collect(),
                transfer: TransferGuard {
                    suite: suite.name.clone(),
                    suite_digest: suite.digest.clone(),
                    max_regression_sigmas: unknown.clone(),
                },
                deployment: DeploymentGuard {
                    rule: DeploymentRule {
                        min_calls: unknown.clone(),
                        gated_percentile: GatedPercentile::P95,
                        latency_block_sigma_relative: unknown.clone(),
                        regression_sigmas: unknown,
                        pending_measurement: None,
                    },
                    budget: Budget::new("fixture", "Unmeasured"),
                },
            },
            scope: suite.families(),
            digest: String::new(),
        };
        plan.seal();
        plan
    }

    #[test]
    fn replay_rejects_a_resealed_success_claim_over_missing_evidence() {
        let suite = Suite::load(include_str!(
            "../../gym/tests/fixtures/caller-v1/suite.json"
        ))
        .unwrap();
        let plan = plan(&suite);
        let evidence = Evidence {
            suite: &suite,
            development: Side {
                base: &[],
                candidate: &[],
                store_head: None,
            },
            locked: None,
            transfer: None,
            deployment: None,
            decided_at: "fixture".into(),
            commitment: None,
        };
        let original = Record::evaluate(&plan, &evidence).unwrap();
        assert!(original.admitted().is_err());
        let mut forged = plan.decide(&evidence).unwrap();
        forged.ruling = gym::admission::Ruling::Passed;
        forged.seal();
        assert!(
            Record::verify(&serde_json::to_string(&forged).unwrap(), &plan, &evidence).is_err()
        );
        let valid = plan.decide(&evidence).unwrap();
        assert!(
            Record::verify(&serde_json::to_string(&valid).unwrap(), &plan, &evidence)
                .unwrap()
                .admitted()
                .is_err()
        );
    }

    #[test]
    fn frozen_plan_cannot_omit_calibration_or_change_winning_metric_between_phases() {
        let suite = Suite::load(include_str!(
            "../../gym/tests/fixtures/caller-v1/suite.json"
        ))
        .unwrap();
        let mut plan = plan(&suite);
        plan.guards.calibration.clear();
        plan.seal();
        assert!(plan.validate().is_err());
        let mut plan = self::plan(&suite);
        plan.rule
            .metric_order
            .push(plan.rule.metric_order[0].clone());
        plan.seal();
        assert!(plan.validate().is_err());
    }
}
