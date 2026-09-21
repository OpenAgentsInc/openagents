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
                question_set: suite.questions.clone(),
                question_digest: suite
                    .questions
                    .as_ref()
                    .map(|_| "fixture-question-digest".into()),
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
                    suite: "fixture-transfer".into(),
                    suite_digest: format!("sha256:{}", "c".repeat(64)),
                    partitions: vec![Partition::Development],
                    question_set: suite.questions.clone(),
                    question_digest: suite
                        .questions
                        .as_ref()
                        .map(|_| "fixture-question-digest".into()),
                    block_sigma: unknown.clone(),
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
    fn complete_retained_evidence_admits_a_measured_winner() {
        use gym::gate::{Cost, Deployment, Profile};
        use gym::suite::{LockedLedger, Spend};
        let suite = Suite::load(include_str!(
            "../../gym/tests/fixtures/caller-v1/suite.json"
        ))
        .unwrap();
        let mut transfer = suite.clone();
        transfer.name = "fixture-transfer".into();
        transfer.items[0].state = serde_json::json!("Independent transfer fixture");
        transfer.digest = transfer.compute_digest().unwrap();
        let mut plan = plan(&suite);
        let bound = |value| Bound {
            value: Some(value),
            basis: Basis::Derived,
            evidence: vec![],
            why: "Synthetic acceptance fixture only.".into(),
        };
        plan.rule.metric_order[0].block_sigma = bound(0.01);
        plan.rule.effect_size_sigmas = bound(1.0);
        plan.rule.family_regression_sigmas = bound(1.0);
        plan.rule.min_blocks_per_side = bound(1.0);
        plan.guards.max_new_refusals = bound(0.0);
        plan.guards.max_new_confident_errors = bound(0.0);
        for floor in &mut plan.guards.calibration {
            floor.block_sigma = bound(0.01);
        }
        plan.guards.transfer.suite_digest = transfer.digest.clone();
        plan.guards.transfer.block_sigma = bound(0.01);
        plan.guards.transfer.max_regression_sigmas = bound(1.0);
        plan.guards.deployment.rule.min_calls = bound(1.0);
        plan.guards.deployment.rule.latency_block_sigma_relative = bound(0.01);
        plan.guards.deployment.rule.regression_sigmas = bound(1.0);
        plan.guards.deployment.budget.max_latency_ms = Some(100.0);
        plan.guards.deployment.budget.max_cost_per_decision_usd = Some(1.0);
        plan.guards.deployment.budget.max_refusal_rate = Some(0.0);
        plan.seal();
        let dir = tempfile::tempdir().unwrap();
        let ledger = LockedLedger::at(dir.path().join("locked.jsonl"));
        let subject = plan.ledger_subject();
        ledger
            .read_locked(
                &suite,
                &Spend {
                    subject: &subject,
                    reason: "Synthetic acceptance fixture",
                    at: "2026-09-21T00:00:00Z",
                    adapter: "",
                },
            )
            .unwrap();
        let report = |suite: &Suite, partition: Partition, file: &str| {
            let path = dir.path().join(file);
            let store = gym::store::Store::at(&path);
            for pin in [&plan.base, &plan.candidate] {
                for item in suite
                    .items
                    .iter()
                    .filter(|item| item.partition == partition)
                {
                    let mut row =
                        gym::row::Row::new(&suite.name, &suite.digest, &item.id, &pin.door).scored(
                            [("yes".into(), 0.8), ("no".into(), 0.2)]
                                .into_iter()
                                .collect(),
                            pin.door == plan.candidate.door,
                        );
                    row.recorded_at = "2026-09-21T00:00:00Z".into();
                    row.split = partition.as_str().into();
                    row.family = item.family.clone();
                    row.estimator = plan.instrument.estimator.clone();
                    row.door_identity = pin.identity.clone();
                    row.question_set = plan.workload.question_set.clone();
                    row.question_digest = plan.workload.question_digest.clone();
                    row.check().unwrap();
                    store.append(&row).unwrap();
                }
            }
            let rows: Vec<gym::row::Row> = store
                .verified_rows()
                .unwrap()
                .into_iter()
                .map(|v| serde_json::from_value(v).unwrap())
                .collect();
            let doors = vec![plan.base.door.clone(), plan.candidate.door.clone()];
            let expected =
                gym::coverage::Expected::of(suite, &[partition], None, None, doors.clone())
                    .unwrap();
            let commitment = gym::commitment::Commitment::of(
                suite,
                &expected,
                gym::commitment::Selection {
                    partitions: vec![partition.as_str().into()],
                    family: None,
                    items: None,
                    doors,
                },
                &rows,
                store.head().unwrap(),
                None,
            );
            let base: Vec<_> = rows
                .iter()
                .filter(|r| r.door == plan.base.door)
                .cloned()
                .collect();
            let candidate: Vec<_> = rows
                .iter()
                .filter(|r| r.door == plan.candidate.door)
                .cloned()
                .collect();
            (rows, commitment, base, candidate)
        };
        let dev = report(&suite, Partition::Development, "development.jsonl");
        let locked = report(&suite, Partition::Locked, "confirmation.jsonl");
        let transfer_rows = report(&transfer, Partition::Development, "transfer.jsonl");
        let profile = Profile::timed(&[10.0, 10.0])
            .refusing(0)
            .costing(Cost::Metered {
                usd_per_decision: 0.01,
            });
        let evidence = Evidence {
            reports: Reports {
                development: Some(ReportEvidence {
                    commitment: &dev.1,
                    rows: &dev.0,
                }),
                locked: Some(ReportEvidence {
                    commitment: &locked.1,
                    rows: &locked.0,
                }),
                transfer: Some(ReportEvidence {
                    commitment: &transfer_rows.1,
                    rows: &transfer_rows.0,
                }),
            },
            suite: &suite,
            development: Side {
                base: &dev.2,
                candidate: &dev.3,
                store_head: dev.1.head.clone(),
            },
            locked: Some(Locked {
                base: &locked.2,
                candidate: &locked.3,
                ledger: &ledger,
                store_head: locked.1.head.clone(),
            }),
            transfer: Some(Transfer {
                suite: &transfer,
                base: &transfer_rows.2,
                candidate: &transfer_rows.3,
                store_head: transfer_rows.1.head.clone(),
            }),
            deployment: Some(Deployment::new("fixture", profile, profile)),
            decided_at: "2026-09-21T00:00:00Z".into(),
            commitment: None,
        };
        let decision = plan.decide(&evidence).unwrap();
        assert_eq!(
            decision.ruling,
            gym::admission::Ruling::Passed,
            "{decision:#?}"
        );
        assert!(
            Record::evaluate(&plan, &evidence)
                .unwrap()
                .admitted()
                .is_ok()
        );
        let record = Record::evaluate(&plan, &evidence).unwrap();
        let base = record.base();
        let binding = crate::Binding {
            lane: crate::Lane::Dedicated,
            artifact: crate::Expected {
                model: base.model.clone(),
                adapter: base.adapter.clone(),
                artifact_signature: base.artifact_signature.clone(),
                execution: base.execution.clone(),
            },
            capacity: Some(crate::Capacity {
                concurrency: Some(3),
                requests_per_minute: Some(60),
            }),
            promotion: None,
            scope: vec![],
        };
        let tenant = crate::Tenant {
            credential: "key-ref:fixture/key".into(),
            principals: vec!["fixture-owner".into()],
            doors: [("fixture-door".into(), binding.clone())].into(),
            quota: None,
        };
        let manifest = crate::Manifest {
            v: crate::SCHEMA.into(),
            sequence: 0,
            supersedes: None,
            shared: BTreeMap::new(),
            tenants: [("fixture-tenant".into(), tenant.clone())].into(),
            digest: String::new(),
        };
        let registry_dir = dir.path().join("registry");
        let installed = crate::Registry::install(&registry_dir, manifest).unwrap();
        let original_digest = installed.manifest().digest.clone();
        let promoted =
            crate::Registry::activate(&registry_dir, "fixture-tenant", "fixture-door", &record)
                .unwrap();
        let current = &promoted.manifest().tenants["fixture-tenant"];
        assert_eq!(current.credential, tenant.credential);
        assert_eq!(current.principals, tenant.principals);
        assert_eq!(current.doors["fixture-door"].capacity, binding.capacity);
        assert_eq!(current.doors["fixture-door"].scope, plan.scope);
        assert_eq!(
            current.doors["fixture-door"].promotion.as_deref(),
            Some(record.reference())
        );
        assert!(
            crate::Registry::activate(&registry_dir, "fixture-tenant", "fixture-door", &record)
                .is_err()
        );
        let rolled_back = crate::Registry::rollback(&registry_dir).unwrap();
        assert_eq!(rolled_back.manifest().sequence, 2);
        assert_eq!(
            rolled_back.manifest().tenants["fixture-tenant"].doors["fixture-door"],
            binding
        );
        assert_eq!(
            crate::Registry::revision(&registry_dir, &original_digest)
                .unwrap()
                .digest,
            original_digest
        );
        assert_eq!(
            crate::Registry::revision(&registry_dir, &promoted.manifest().digest)
                .unwrap()
                .sequence,
            1
        );

        let mut missing = evidence.clone();
        missing.reports.locked = None;
        assert_ne!(
            plan.decide(&missing).unwrap().ruling,
            gym::admission::Ruling::Passed
        );
        assert!(
            Record::evaluate(&plan, &missing)
                .unwrap()
                .admitted()
                .is_err()
        );
        let mut wrong_workload = evidence.clone();
        wrong_workload.deployment.as_mut().unwrap().group = "another-workload".into();
        assert_eq!(
            plan.decide(&wrong_workload).unwrap().ruling,
            gym::admission::Ruling::Refused
        );
        let mut expensive = evidence.clone();
        expensive.deployment.as_mut().unwrap().candidate.cost = Some(Cost::Metered {
            usd_per_decision: 2.0,
        });
        assert_eq!(
            plan.decide(&expensive).unwrap().ruling,
            gym::admission::Ruling::Failed
        );
        let mut edited_rows = dev.3.clone();
        edited_rows[0].latency_ms = Some(0.1);
        let mut changed = evidence.clone();
        changed.development.candidate = &edited_rows;
        let changed_decision = plan.decide(&changed).unwrap();
        assert_eq!(changed_decision.ruling, gym::admission::Ruling::Refused);
        assert!(
            changed_decision
                .refusals
                .iter()
                .any(|r| r.contains("evaluated rows differ"))
        );
        let mut identity_rows = dev.3.clone();
        identity_rows[0].door_identity.artifact_signature = format!("sha256:{}", "f".repeat(64));
        let mut identity_drift = evidence.clone();
        identity_drift.development.candidate = &identity_rows;
        let refused = plan.decide(&identity_drift).unwrap();
        assert_eq!(refused.ruling, gym::admission::Ruling::Refused);
        assert!(
            refused
                .refusals
                .iter()
                .any(|reason| reason.contains("different identity"))
        );
    }

    #[test]
    fn replay_rejects_a_resealed_success_claim_over_missing_evidence() {
        let suite = Suite::load(include_str!(
            "../../gym/tests/fixtures/caller-v1/suite.json"
        ))
        .unwrap();
        let plan = plan(&suite);
        let evidence = Evidence {
            reports: Reports::default(),
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
        let evidence_decision = plan.decide(&evidence).unwrap();
        let retained = evidence_decision
            .phases
            .iter()
            .find(|p| p.phase == "retained_evidence")
            .unwrap();
        assert_eq!(retained.criteria.len(), 3);
        assert!(
            retained
                .criteria
                .iter()
                .all(|c| c.verdict == gym::gate::Verdict::Unverifiable)
        );
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
    fn admission_revalidates_suite_contents_and_declared_wording() {
        let suite = Suite::load(include_str!(
            "../../gym/tests/fixtures/caller-v1/suite.json"
        ))
        .unwrap();
        let original = plan(&suite);
        for mutation in 0..3 {
            let mut suite = suite.clone();
            let mut plan = original.clone();
            match mutation {
                0 => suite.items[0].state = serde_json::json!("Changed after loading"),
                1 => {
                    plan.workload.question_set = None;
                    plan.workload.question_digest = None;
                    plan.seal();
                }
                _ => {
                    plan.workload.question_digest = Some(String::new());
                    plan.seal();
                }
            }
            let evidence = Evidence {
                reports: Reports::default(),
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
            let decision = plan.decide(&evidence).unwrap();
            assert_eq!(decision.ruling, gym::admission::Ruling::Refused);
            assert!(
                Record::evaluate(&plan, &evidence)
                    .unwrap()
                    .admitted()
                    .is_err()
            );
        }
    }

    #[test]
    fn frozen_metric_distinguishes_winner_underpowered_tie_and_loser() {
        let suite = Suite::load(include_str!(
            "../../gym/tests/fixtures/caller-v1/suite.json"
        ))
        .unwrap();
        let mut plan = plan(&suite);
        let synthetic = |value| Bound {
            value: Some(value),
            basis: Basis::Derived,
            evidence: vec![],
            why: "Synthetic arithmetic fixture; not a deployment threshold.".into(),
        };
        plan.rule.min_blocks_per_side = synthetic(1.0);
        plan.rule.metric_order[0].block_sigma = synthetic(0.1);
        plan.rule.effect_size_sigmas = synthetic(1.0);
        plan.rule.family_regression_sigmas = synthetic(1.0);
        plan.seal();
        let items = suite.partition(Partition::Development).unwrap();
        let rows = |pin: &Pinned, correct_count: usize| -> Vec<_> {
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let mut row =
                        gym::row::Row::new(&suite.name, &suite.digest, &item.id, &pin.door).scored(
                            [("yes".into(), 0.8), ("no".into(), 0.2)]
                                .into_iter()
                                .collect(),
                            index < correct_count,
                        );
                    row.recorded_at = "2026-09-21T00:00:00Z".into();
                    row.split = "development".into();
                    row.family = item.family.clone();
                    row.estimator = plan.instrument.estimator.clone();
                    row.door_identity = pin.identity.clone();
                    row.question_set = plan.workload.question_set.clone();
                    row.question_digest = plan.workload.question_digest.clone();
                    row.check().unwrap();
                    row
                })
                .collect()
        };
        let base = rows(&plan.base, items.len() / 2);
        for (correct, sigma, expected) in [
            (items.len(), 0.1, gym::gate::Verdict::Passed),
            (items.len(), 1.0, gym::gate::Verdict::Unverifiable),
            (items.len() / 2, 0.1, gym::gate::Verdict::Failed),
            (0, 0.1, gym::gate::Verdict::Failed),
        ] {
            let candidate = rows(&plan.candidate, correct);
            let mut comparison = plan.clone();
            comparison.rule.metric_order[0].block_sigma = synthetic(sigma);
            comparison.seal();
            let evidence = Evidence {
                reports: Reports::default(),
                suite: &suite,
                development: Side {
                    base: &base,
                    candidate: &candidate,
                    store_head: None,
                },
                locked: None,
                transfer: None,
                deployment: None,
                decided_at: "fixture".into(),
                commitment: None,
            };
            let decision = comparison.decide(&evidence).unwrap();
            let phase = decision
                .phases
                .iter()
                .find(|p| p.phase == DEVELOPMENT_PHASE)
                .unwrap();
            let win = phase
                .criteria
                .iter()
                .find(|c| c.name == "the_candidate_earns_the_win")
                .unwrap();
            assert_eq!(win.verdict, expected, "{}", win.detail);
            if correct == 0 {
                let family = phase
                    .criteria
                    .iter()
                    .find(|c| c.name == "no_family_regresses_beyond_the_allowance")
                    .unwrap();
                assert_eq!(
                    family.verdict,
                    gym::gate::Verdict::Failed,
                    "{}",
                    family.detail
                );
            }

            // A winning metric alone never authorizes activation without
            // locked confirmation, transfer, deployment, and every guard.
            assert!(
                Record::evaluate(&comparison, &evidence)
                    .unwrap()
                    .admitted()
                    .is_err()
            );
            for name in [
                "confident_errors_do_not_rise",
                "the_candidate_declines_no_new_items",
            ] {
                assert_eq!(
                    phase
                        .criteria
                        .iter()
                        .find(|c| c.name == name)
                        .unwrap()
                        .verdict,
                    gym::gate::Verdict::Unverifiable
                );
            }
        }
    }

    #[test]
    fn repeated_blocks_require_complete_comparable_coverage() {
        let suite = Suite::load(include_str!(
            "../../gym/tests/fixtures/caller-v1/suite.json"
        ))
        .unwrap();
        let mut plan = plan(&suite);
        plan.instrument.seed_blocks = vec![11, 29];
        plan.seal();
        let mut base = Vec::new();
        for seed in &plan.instrument.seed_blocks {
            for item in suite.partition(Partition::Development).unwrap() {
                let mut row =
                    gym::row::Row::new(&suite.name, &suite.digest, &item.id, &plan.base.door)
                        .scored(
                            [("yes".into(), 0.8), ("no".into(), 0.2)]
                                .into_iter()
                                .collect(),
                            true,
                        );
                row.recorded_at = "2026-09-21T00:00:00Z".into();
                row.split = "development".into();
                row.family = item.family.clone();
                row.estimator = plan.instrument.estimator.clone();
                row.door_identity = plan.base.identity.clone();
                row.question_set = plan.workload.question_set.clone();
                row.question_digest = plan.workload.question_digest.clone();
                row.seed_base = Some(*seed);
                row.check().unwrap();
                base.push(row);
            }
        }
        let candidate: Vec<_> = base
            .iter()
            .cloned()
            .map(|mut row| {
                row.door = plan.candidate.door.clone();
                row.door_identity = plan.candidate.identity.clone();
                row
            })
            .collect();
        for mutation in 0..4 {
            let mut candidate = candidate.clone();
            match mutation {
                1 => {
                    candidate.pop();
                }
                2 => candidate.push(candidate[0].clone()),
                3 => candidate[0].seed_base = Some(99),
                _ => {}
            }
            let evidence = Evidence {
                reports: Reports::default(),
                suite: &suite,
                development: Side {
                    base: &base,
                    candidate: &candidate,
                    store_head: None,
                },
                locked: None,
                transfer: None,
                deployment: None,
                decided_at: "fixture".into(),
                commitment: None,
            };
            let decision = plan.decide(&evidence).unwrap();
            if mutation == 3 {
                assert_eq!(decision.ruling, gym::admission::Ruling::Refused);
            } else {
                assert_ne!(
                    decision.ruling,
                    gym::admission::Ruling::Refused,
                    "{:?}",
                    decision.refusals
                );
                let coverage = decision
                    .phases
                    .iter()
                    .find(|p| p.phase == DEVELOPMENT_PHASE)
                    .unwrap()
                    .criteria
                    .iter()
                    .find(|c| c.name == "the_declared_selection_is_covered")
                    .unwrap();
                assert_eq!(
                    coverage.verdict,
                    if mutation == 0 {
                        gym::gate::Verdict::Passed
                    } else {
                        gym::gate::Verdict::Unverifiable
                    }
                );
            }
        }
        plan.instrument.seed_blocks.push(11);
        plan.seal();
        assert!(plan.validate().is_err());
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
        let mut alternative = plan.rule.metric_order[0].clone();
        alternative.metric = Metric::Brier;
        plan.rule.metric_order.push(alternative);
        plan.seal();
        assert!(plan.validate().is_err());
    }
    #[test]
    fn overridden_locked_exposure_cannot_confirm_a_candidate() {
        use gym::suite::{LockedLedger, Spend};
        let suite = Suite::load(include_str!(
            "../../gym/tests/fixtures/caller-v1/suite.json"
        ))
        .unwrap();
        let plan = plan(&suite);
        let dir = tempfile::tempdir().unwrap();
        let ledger = LockedLedger::at(dir.path().join("locked.jsonl"));
        let subject = plan.ledger_subject();
        let spend = Spend {
            subject: &subject,
            reason: "Fixture confirmation",
            at: "2026-09-21T00:00:00Z",
            adapter: "",
        };
        ledger.read_locked(&suite, &spend).unwrap();
        let evidence = Evidence {
            reports: Reports::default(),
            suite: &suite,
            development: Side {
                base: &[],
                candidate: &[],
                store_head: None,
            },
            locked: Some(Locked {
                base: &[],
                candidate: &[],
                ledger: &ledger,
                store_head: None,
            }),
            transfer: None,
            deployment: None,
            decided_at: "fixture".into(),
            commitment: None,
        };
        assert_ne!(
            plan.decide(&evidence).unwrap().ruling,
            gym::admission::Ruling::Refused
        );
        ledger
            .read_locked_again(
                &suite,
                &spend,
                "fixture-operator",
                "Test repeat exposure refusal",
            )
            .unwrap();
        let decision = plan.decide(&evidence).unwrap();
        assert_eq!(decision.ruling, gym::admission::Ruling::Refused);
        assert!(
            decision
                .refusals
                .iter()
                .any(|reason| reason.contains("exactly one original read"))
        );
    }
    #[test]
    fn matching_transfer_subsets_do_not_establish_declared_coverage() {
        let suite = Suite::load(include_str!(
            "../../gym/tests/fixtures/caller-v1/suite.json"
        ))
        .unwrap();
        let mut transfer = suite.clone();
        transfer.name = "fixture-transfer".into();
        transfer.items[0].state = serde_json::json!("Different transfer state");
        transfer.digest = transfer.compute_digest().unwrap();
        let mut plan = plan(&suite);
        plan.guards.transfer.suite_digest = transfer.digest.clone();
        plan.seal();
        let item = transfer.partition(Partition::Development).unwrap()[0];
        let mut base =
            gym::row::Row::new(&transfer.name, &transfer.digest, &item.id, &plan.base.door).scored(
                [("yes".into(), 0.8), ("no".into(), 0.2)]
                    .into_iter()
                    .collect(),
                true,
            );
        base.recorded_at = "2026-09-21T00:00:00Z".into();
        base.split = "development".into();
        base.family = item.family.clone();
        base.question_set = plan.guards.transfer.question_set.clone();
        base.question_digest = plan.guards.transfer.question_digest.clone();
        base.estimator = plan.instrument.estimator.clone();
        base.door_identity = plan.base.identity.clone();
        base.check().unwrap();
        let mut candidate = base.clone();
        candidate.door = plan.candidate.door.clone();
        candidate.door_identity = plan.candidate.identity.clone();
        let evidence = Evidence {
            reports: Reports::default(),
            suite: &suite,
            development: Side {
                base: &[],
                candidate: &[],
                store_head: None,
            },
            locked: None,
            transfer: Some(Transfer {
                suite: &transfer,
                base: std::slice::from_ref(&base),
                candidate: std::slice::from_ref(&candidate),
                store_head: None,
            }),
            deployment: None,
            decided_at: "fixture".into(),
            commitment: None,
        };
        let decision = plan.decide(&evidence).unwrap();
        assert_ne!(
            decision.ruling,
            gym::admission::Ruling::Refused,
            "{:?}",
            decision.refusals
        );
        let phase = decision
            .phases
            .iter()
            .find(|phase| phase.phase == TRANSFER_PHASE)
            .unwrap();
        let coverage = phase
            .criteria
            .iter()
            .find(|criterion| criterion.name == "the_transfer_selection_is_covered")
            .unwrap();
        assert_eq!(coverage.verdict, gym::gate::Verdict::Unverifiable);

        // Complete transfer evidence must use transfer variance, even when
        // the development variance would excuse every possible loss.
        let synthetic = |value| Bound {
            value: Some(value),
            basis: Basis::Derived,
            evidence: vec![],
            why: "Synthetic variance fixture, not a deployment threshold.".into(),
        };
        plan.rule.metric_order[0].block_sigma = synthetic(10.0);
        plan.guards.transfer.max_regression_sigmas = synthetic(1.0);
        plan.guards.transfer.block_sigma = synthetic(0.01);
        plan.seal();
        let base_rows: Vec<_> = transfer
            .partition(Partition::Development)
            .unwrap()
            .iter()
            .map(|item| {
                let mut row = base.clone();
                row.item_id = item.id.clone();
                row.family = item.family.clone();
                row
            })
            .collect();
        let candidate_rows: Vec<_> = base_rows
            .iter()
            .cloned()
            .map(|mut row| {
                row.door = plan.candidate.door.clone();
                row.door_identity = plan.candidate.identity.clone();
                row.correct = Some(false);
                row
            })
            .collect();
        let evidence = Evidence {
            reports: Reports::default(),
            transfer: Some(Transfer {
                suite: &transfer,
                base: &base_rows,
                candidate: &candidate_rows,
                store_head: None,
            }),
            ..evidence
        };
        let decision = plan.decide(&evidence).unwrap();
        assert_ne!(
            decision.ruling,
            gym::admission::Ruling::Refused,
            "{:?}",
            decision.refusals
        );
        let criterion = decision
            .phases
            .iter()
            .find(|p| p.phase == TRANSFER_PHASE)
            .unwrap()
            .criteria
            .iter()
            .find(|c| c.name == "the_candidate_holds_on_transfer")
            .unwrap();
        assert_eq!(
            criterion.verdict,
            gym::gate::Verdict::Failed,
            "{}",
            criterion.detail
        );
    }
}
