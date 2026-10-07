use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{CompiledAgentEvidenceClass, CompiledAgentModuleKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_compiled_agent_tailnet_governed_run, build_compiled_agent_tailnet_node_bundle,
    canonical_compiled_agent_confidence_policy,
    canonical_compiled_agent_external_quarantine_report,
    canonical_compiled_agent_external_submission_staging_ledger,
    canonical_compiled_agent_learning_receipt_ledger,
    canonical_compiled_agent_promoted_artifact_contract,
    canonical_compiled_agent_route_model_artifact,
    canonical_compiled_agent_shadow_disagreement_receipts,
    canonical_compiled_agent_xtrain_cycle_receipt, repo_relative_path,
    CompiledAgentArtifactContractError, CompiledAgentConfidenceBand,
    CompiledAgentExternalContributorProfile, CompiledAgentExternalContributorTrustHistory,
    CompiledAgentExternalIntakeError, CompiledAgentPromotionDecision, CompiledAgentReceiptError,
    CompiledAgentShadowGovernanceError, CompiledAgentTailnetPilotError, CompiledAgentXtrainError,
};

pub const COMPILED_AGENT_PHASE_SIX_OPERATIONAL_REPORT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_phase_six_operational_report_v1.json";
pub const COMPILED_AGENT_PHASE_SIX_DOC_PATH: &str = "docs/COMPILED_AGENT_PHASE_SIX.md";

const COMPILED_AGENT_PHASE_SIX_OPERATIONAL_REPORT_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.phase_six_operational_report.v1";

const PHASE_FIVE_GOVERNED_RUN_DIGEST: &str =
    "dc9ab99b00fa05ae990693b5e758cc728d7d06dcef36bb51b86bf769c7f18b37";
const PHASE_FIVE_STAGING_LEDGER_DIGEST: &str =
    "5d05f3500e0ca5bdfd8291e1b0ffd3bfd95a4d99b3bc854d03db6636183151b2";
const PHASE_FIVE_QUARANTINE_REPORT_DIGEST: &str =
    "90492578580c808d8639f7a920b641fe8e717446509c74e9126d5e0c6d91c6c4";
const PHASE_FIVE_XTRAIN_DIGEST: &str =
    "4f7655b1b65931c538c3fbea643452a8a16e1ad7738ae4a9e12896ef722cef45";
const PHASE_FIVE_ROUTE_REPLAY_MATCH_COUNT: u32 = 25;
const PHASE_FIVE_ROUTE_HELDOUT_MATCH_COUNT: u32 = 12;
const PHASE_FIVE_ROUTE_REGRESSION_COUNT: u32 = 0;
const PHASE_FIVE_TAILNET_REVIEW_REQUIRED_SUBMISSION_COUNT: u32 = 6;
const PHASE_FIVE_TAILNET_REJECTED_SUBMISSION_COUNT: u32 = 0;

#[derive(Debug, Error)]
pub enum CompiledAgentPhaseSixError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error("phase-six operational report is invalid: {detail}")]
    InvalidReport { detail: String },
    #[error(transparent)]
    Receipts(#[from] CompiledAgentReceiptError),
    #[error(transparent)]
    ArtifactContract(#[from] CompiledAgentArtifactContractError),
    #[error(transparent)]
    ExternalIntake(#[from] CompiledAgentExternalIntakeError),
    #[error(transparent)]
    ShadowGovernance(#[from] CompiledAgentShadowGovernanceError),
    #[error(transparent)]
    Tailnet(#[from] CompiledAgentTailnetPilotError),
    #[error(transparent)]
    Xtrain(#[from] CompiledAgentXtrainError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentOperationalAlertKind {
    HeldoutRegression,
    QuarantineSpike,
    ValidatorBoundaryViolation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentOperationalAlert {
    pub kind: CompiledAgentOperationalAlertKind,
    pub triggered: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentTailnetCadenceSection {
    pub previous_governed_run_digest: String,
    pub current_governed_run_digest: String,
    pub previous_staging_ledger_digest: String,
    pub current_staging_ledger_digest: String,
    pub previous_quarantine_report_digest: String,
    pub current_quarantine_report_digest: String,
    pub previous_xtrain_cycle_digest: String,
    pub current_xtrain_cycle_digest: String,
    pub previous_route_decision: CompiledAgentPromotionDecision,
    pub current_route_decision: CompiledAgentPromotionDecision,
    pub previous_grounded_decision: CompiledAgentPromotionDecision,
    pub current_grounded_decision: CompiledAgentPromotionDecision,
    pub current_review_required_submission_count: u32,
    pub current_rejected_submission_count: u32,
    pub alerts: Vec<CompiledAgentOperationalAlert>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRouteOperationalSection {
    pub promoted_artifact_id: String,
    pub promoted_artifact_digest: String,
    pub decision: CompiledAgentPromotionDecision,
    pub baseline_replay_match_count: u32,
    pub candidate_replay_match_count: u32,
    pub baseline_heldout_match_count: u32,
    pub candidate_heldout_match_count: u32,
    pub retained_regression_count: u32,
    pub previous_replay_match_count: u32,
    pub previous_heldout_match_count: u32,
    pub previous_regression_count: u32,
    pub permanent_regression_trap_receipt_ids: Vec<String>,
    pub ambiguous_check_receipt_ids: Vec<String>,
    pub human_review_receipt_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned_correct_mean_confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned_incorrect_mean_confidence: Option<f32>,
    pub low_confidence_disagreement_count: u32,
    pub confidence_watch_receipt_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentGroundedOperationalSection {
    pub promoted_artifact_id: String,
    pub promoted_artifact_digest: String,
    pub decision: CompiledAgentPromotionDecision,
    pub baseline_replay_match_count: u32,
    pub candidate_replay_match_count: u32,
    pub baseline_heldout_match_count: u32,
    pub candidate_heldout_match_count: u32,
    pub retained_regression_count: u32,
    pub disagreement_receipt_ids: Vec<String>,
    pub human_review_receipt_ids: Vec<String>,
    pub new_failure_class_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalEvidenceBoundarySection {
    pub staging_ledger_digest: String,
    pub quarantine_report_digest: String,
    pub anomaly_submission_ids: Vec<String>,
    pub fail_closed_submission_ids: Vec<String>,
    pub trusted_signal_contributor_ids: Vec<String>,
    pub watch_contributor_ids: Vec<String>,
    pub contributor_trust_histories: Vec<CompiledAgentExternalContributorTrustHistory>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentPhaseSixOperationalReport {
    pub schema_version: String,
    pub report_id: String,
    pub row_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub promoted_artifact_contract_digest: String,
    pub tailnet_cadence: CompiledAgentTailnetCadenceSection,
    pub route: CompiledAgentRouteOperationalSection,
    pub grounded_answer: CompiledAgentGroundedOperationalSection,
    pub external_evidence_boundary: CompiledAgentExternalEvidenceBoundarySection,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn compiled_agent_phase_six_operational_report_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_PHASE_SIX_OPERATIONAL_REPORT_FIXTURE_PATH)
}

pub fn canonical_compiled_agent_phase_six_operational_report(
) -> Result<CompiledAgentPhaseSixOperationalReport, CompiledAgentPhaseSixError> {
    let local_bundle = build_compiled_agent_tailnet_node_bundle(
        CompiledAgentExternalContributorProfile::TailnetM5Mlx,
    )?;
    let remote_bundle = build_compiled_agent_tailnet_node_bundle(
        CompiledAgentExternalContributorProfile::TailnetArchlinuxRtx4080Cuda,
    )?;
    let (tailnet_staging, tailnet_quarantine, tailnet_run) =
        build_compiled_agent_tailnet_governed_run(&local_bundle, &remote_bundle)?;
    let xtrain_cycle = canonical_compiled_agent_xtrain_cycle_receipt()?;
    let contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let confidence_policy = canonical_compiled_agent_confidence_policy()?;
    let disagreement_receipts = canonical_compiled_agent_shadow_disagreement_receipts()?;
    let external_staging = canonical_compiled_agent_external_submission_staging_ledger()?;
    let external_quarantine = canonical_compiled_agent_external_quarantine_report()?;
    let learning_ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let route_artifact = canonical_compiled_agent_route_model_artifact()?;

    let route_policy = confidence_policy
        .policies
        .iter()
        .find(|policy| policy.module == CompiledAgentModuleKind::Route)
        .expect("canonical confidence policy must retain route module");
    let grounded_policy = confidence_policy
        .policies
        .iter()
        .find(|policy| policy.module == CompiledAgentModuleKind::GroundedAnswer)
        .expect("canonical confidence policy must retain grounded module");

    let grounded_disagreement_receipt_ids = disagreement_receipts
        .receipts
        .iter()
        .filter(|receipt| receipt.module == CompiledAgentModuleKind::GroundedAnswer)
        .map(|receipt| receipt.source_receipt_id.clone())
        .collect::<Vec<_>>();
    let permanent_regression_trap_receipt_ids = learning_ledger
        .receipts
        .iter()
        .filter(|receipt| {
            receipt.corpus_split == crate::CompiledAgentCorpusSplit::HeldOut
                && receipt.tags.iter().any(|tag| {
                    matches!(
                        tag.as_str(),
                        "route_ambiguity"
                            | "negated"
                            | "exclusion"
                            | "compare_exclusion"
                            | "unsupported_provider_question"
                            | "unsupported_wallet_question"
                    )
                })
        })
        .map(|receipt| receipt.receipt_id.clone())
        .collect::<Vec<_>>();
    let ambiguous_check_receipt_ids = learning_ledger
        .receipts
        .iter()
        .filter(|receipt| {
            receipt.tags.iter().any(|tag| {
                matches!(
                    tag.as_str(),
                    "route_ambiguity" | "compare_exclusion" | "confidence_edge"
                )
            })
        })
        .map(|receipt| receipt.receipt_id.clone())
        .collect::<Vec<_>>();
    let confidence_watch_receipt_ids = disagreement_receipts
        .receipts
        .iter()
        .filter(|receipt| {
            receipt.module == CompiledAgentModuleKind::Route
                && matches!(
                    receipt.promoted_band,
                    CompiledAgentConfidenceBand::Watch | CompiledAgentConfidenceBand::Review
                )
        })
        .map(|receipt| receipt.source_receipt_id.clone())
        .collect::<Vec<_>>();
    let fail_closed_submission_ids = external_staging
        .submissions
        .iter()
        .filter(|record| {
            record.review_state == crate::CompiledAgentExternalReviewState::Rejected
                && record.anomaly_flags.iter().any(|anomaly| {
                    matches!(
                        anomaly,
                        crate::CompiledAgentExternalAnomalyKind::SchemaMismatch
                            | crate::CompiledAgentExternalAnomalyKind::ContractMismatch
                            | crate::CompiledAgentExternalAnomalyKind::DigestMismatch
                            | crate::CompiledAgentExternalAnomalyKind::MissingEnvironmentMetadata
                    )
                })
        })
        .map(|record| record.submission_id.clone())
        .collect::<Vec<_>>();
    let validator_boundary_violation_ids = tailnet_staging
        .submissions
        .iter()
        .filter(|record| {
            record.review_state != crate::CompiledAgentExternalReviewState::Rejected
                && (!record.validation_checks.schema_conformant
                    || !record.validation_checks.contract_version_match
                    || !record.validation_checks.digest_integrity)
        })
        .map(|record| record.submission_id.clone())
        .collect::<Vec<_>>();

    let tailnet_cadence = CompiledAgentTailnetCadenceSection {
        previous_governed_run_digest: String::from(PHASE_FIVE_GOVERNED_RUN_DIGEST),
        current_governed_run_digest: tailnet_run.run_digest.clone(),
        previous_staging_ledger_digest: String::from(PHASE_FIVE_STAGING_LEDGER_DIGEST),
        current_staging_ledger_digest: tailnet_staging.ledger_digest.clone(),
        previous_quarantine_report_digest: String::from(PHASE_FIVE_QUARANTINE_REPORT_DIGEST),
        current_quarantine_report_digest: tailnet_quarantine.report_digest.clone(),
        previous_xtrain_cycle_digest: String::from(PHASE_FIVE_XTRAIN_DIGEST),
        current_xtrain_cycle_digest: xtrain_cycle.receipt_digest.clone(),
        previous_route_decision: CompiledAgentPromotionDecision::Promote,
        current_route_decision: xtrain_cycle.route_outcome.decision,
        previous_grounded_decision: CompiledAgentPromotionDecision::Promote,
        current_grounded_decision: xtrain_cycle.grounded_answer_outcome.decision,
        current_review_required_submission_count: tailnet_staging.review_required_submission_count,
        current_rejected_submission_count: tailnet_staging.rejected_submission_count,
        alerts: vec![
            CompiledAgentOperationalAlert {
                kind: CompiledAgentOperationalAlertKind::HeldoutRegression,
                triggered: !xtrain_cycle
                    .route_outcome
                    .validation
                    .heldout_regression_receipt_ids
                    .is_empty()
                    || !xtrain_cycle
                        .grounded_answer_outcome
                        .validation
                        .heldout_regression_receipt_ids
                        .is_empty(),
                detail: String::from(
                    "Held-out regression stays fail-closed for both route and grounded-answer promotion decisions.",
                ),
            },
            CompiledAgentOperationalAlert {
                kind: CompiledAgentOperationalAlertKind::QuarantineSpike,
                triggered: tailnet_staging.review_required_submission_count
                    > PHASE_FIVE_TAILNET_REVIEW_REQUIRED_SUBMISSION_COUNT + 2
                    || tailnet_staging.rejected_submission_count
                        > PHASE_FIVE_TAILNET_REJECTED_SUBMISSION_COUNT,
                detail: format!(
                    "Tailnet review-required={} rejected={} against prior review-required={} rejected={}.",
                    tailnet_staging.review_required_submission_count,
                    tailnet_staging.rejected_submission_count,
                    PHASE_FIVE_TAILNET_REVIEW_REQUIRED_SUBMISSION_COUNT,
                    PHASE_FIVE_TAILNET_REJECTED_SUBMISSION_COUNT
                ),
            },
            CompiledAgentOperationalAlert {
                kind: CompiledAgentOperationalAlertKind::ValidatorBoundaryViolation,
                triggered: !validator_boundary_violation_ids.is_empty(),
                detail: if validator_boundary_violation_ids.is_empty() {
                    String::from(
                        "No submission crossed the validator boundary with schema, contract, or digest drift while remaining non-rejected.",
                    )
                } else {
                    format!(
                        "Validator-boundary violations detected for submissions {:?}.",
                        validator_boundary_violation_ids
                    )
                },
            },
        ],
    };

    let route = CompiledAgentRouteOperationalSection {
        promoted_artifact_id: route_artifact.artifact_id.clone(),
        promoted_artifact_digest: route_artifact.artifact_digest.clone(),
        decision: xtrain_cycle.route_outcome.decision,
        baseline_replay_match_count: xtrain_cycle
            .route_outcome
            .validation
            .baseline_replay_match_count,
        candidate_replay_match_count: xtrain_cycle
            .route_outcome
            .validation
            .candidate_replay_match_count,
        baseline_heldout_match_count: xtrain_cycle
            .route_outcome
            .validation
            .baseline_heldout_match_count,
        candidate_heldout_match_count: xtrain_cycle
            .route_outcome
            .validation
            .candidate_heldout_match_count,
        retained_regression_count: xtrain_cycle
            .route_outcome
            .validation
            .heldout_regression_receipt_ids
            .len() as u32,
        previous_replay_match_count: PHASE_FIVE_ROUTE_REPLAY_MATCH_COUNT,
        previous_heldout_match_count: PHASE_FIVE_ROUTE_HELDOUT_MATCH_COUNT,
        previous_regression_count: PHASE_FIVE_ROUTE_REGRESSION_COUNT,
        permanent_regression_trap_receipt_ids,
        ambiguous_check_receipt_ids,
        human_review_receipt_ids: route_policy.metrics.human_review_receipt_ids.clone(),
        learned_correct_mean_confidence: route_policy.metrics.learned_correct_mean_confidence,
        learned_incorrect_mean_confidence: route_policy.metrics.learned_incorrect_mean_confidence,
        low_confidence_disagreement_count: route_policy.metrics.low_confidence_disagreement_count,
        confidence_watch_receipt_ids,
    };

    let grounded_answer = CompiledAgentGroundedOperationalSection {
        promoted_artifact_id: xtrain_cycle
            .grounded_answer_outcome
            .delta
            .candidate_revision_id
            .clone(),
        promoted_artifact_digest: xtrain_cycle
            .grounded_answer_outcome
            .delta
            .candidate_artifact_digest
            .clone()
            .unwrap_or_default(),
        decision: xtrain_cycle.grounded_answer_outcome.decision,
        baseline_replay_match_count: xtrain_cycle
            .grounded_answer_outcome
            .validation
            .baseline_replay_match_count,
        candidate_replay_match_count: xtrain_cycle
            .grounded_answer_outcome
            .validation
            .candidate_replay_match_count,
        baseline_heldout_match_count: xtrain_cycle
            .grounded_answer_outcome
            .validation
            .baseline_heldout_match_count,
        candidate_heldout_match_count: xtrain_cycle
            .grounded_answer_outcome
            .validation
            .candidate_heldout_match_count,
        retained_regression_count: xtrain_cycle
            .grounded_answer_outcome
            .validation
            .heldout_regression_receipt_ids
            .len() as u32,
        disagreement_receipt_ids: grounded_disagreement_receipt_ids,
        human_review_receipt_ids: grounded_policy.metrics.human_review_receipt_ids.clone(),
        new_failure_class_count: learning_ledger
            .receipts
            .iter()
            .filter(|receipt| {
                receipt.corpus_split == crate::CompiledAgentCorpusSplit::HeldOut
                    && receipt
                        .assessment
                        .failure_classes
                        .iter()
                        .any(|failure_class| failure_class == "grounded_answer_mismatch")
            })
            .count() as u32,
    };

    let external_evidence_boundary = CompiledAgentExternalEvidenceBoundarySection {
        staging_ledger_digest: external_staging.ledger_digest.clone(),
        quarantine_report_digest: external_quarantine.report_digest.clone(),
        anomaly_submission_ids: external_quarantine.anomaly_submission_ids.clone(),
        fail_closed_submission_ids,
        trusted_signal_contributor_ids: external_quarantine.trusted_signal_contributor_ids.clone(),
        watch_contributor_ids: external_quarantine.watch_contributor_ids.clone(),
        contributor_trust_histories: external_staging.contributor_trust_histories.clone(),
    };

    let mut report = CompiledAgentPhaseSixOperationalReport {
        schema_version: String::from(COMPILED_AGENT_PHASE_SIX_OPERATIONAL_REPORT_SCHEMA_VERSION),
        report_id: String::from("compiled_agent.phase_six.operational_report.v1"),
        row_id: contract.row_id,
        evidence_class: contract.evidence_class,
        promoted_artifact_contract_digest: contract.contract_digest,
        tailnet_cadence,
        route,
        grounded_answer,
        external_evidence_boundary,
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Phase six keeps the Tailnet-first compiled-agent loop repeatable with route decision={:?}, grounded decision={:?}, {} permanent route traps, and {} anomaly-gated external submissions.",
        report.tailnet_cadence.current_route_decision,
        report.tailnet_cadence.current_grounded_decision,
        report.route.permanent_regression_trap_receipt_ids.len(),
        report.external_evidence_boundary.anomaly_submission_ids.len(),
    );
    report.report_digest = stable_digest(
        b"compiled_agent_phase_six_operational_report|",
        &report_without_digest(&report),
    );
    report.validate()?;
    Ok(report)
}

impl CompiledAgentPhaseSixOperationalReport {
    fn validate(&self) -> Result<(), CompiledAgentPhaseSixError> {
        if self.schema_version != COMPILED_AGENT_PHASE_SIX_OPERATIONAL_REPORT_SCHEMA_VERSION {
            return Err(CompiledAgentPhaseSixError::InvalidReport {
                detail: String::from("schema_version drifted"),
            });
        }
        if self.evidence_class != CompiledAgentEvidenceClass::LearnedLane {
            return Err(CompiledAgentPhaseSixError::InvalidReport {
                detail: String::from("evidence class drifted"),
            });
        }
        if self.route.retained_regression_count > 0
            && self.tailnet_cadence.current_route_decision != CompiledAgentPromotionDecision::Hold
        {
            return Err(CompiledAgentPhaseSixError::InvalidReport {
                detail: String::from("route retained regression must force hold"),
            });
        }
        if self.report_digest
            != stable_digest(
                b"compiled_agent_phase_six_operational_report|",
                &report_without_digest(self),
            )
        {
            return Err(CompiledAgentPhaseSixError::InvalidReport {
                detail: String::from("report digest drifted"),
            });
        }
        Ok(())
    }
}

pub fn write_compiled_agent_phase_six_operational_report(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentPhaseSixOperationalReport, CompiledAgentPhaseSixError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentPhaseSixError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = canonical_compiled_agent_phase_six_operational_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentPhaseSixError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

pub fn verify_compiled_agent_phase_six_operational_report_fixture(
) -> Result<(), CompiledAgentPhaseSixError> {
    let path = compiled_agent_phase_six_operational_report_fixture_path();
    let expected = canonical_compiled_agent_phase_six_operational_report()?;
    let committed: CompiledAgentPhaseSixOperationalReport =
        serde_json::from_slice(&fs::read(&path).map_err(|error| {
            CompiledAgentPhaseSixError::Read {
                path: path.display().to_string(),
                error,
            }
        })?)?;
    if committed != expected {
        return Err(CompiledAgentPhaseSixError::FixtureDrift {
            path: path.display().to_string(),
        });
    }
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn report_without_digest(
    report: &CompiledAgentPhaseSixOperationalReport,
) -> CompiledAgentPhaseSixOperationalReport {
    let mut clone = report.clone();
    clone.report_digest.clear();
    clone
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_compiled_agent_phase_six_operational_report,
        verify_compiled_agent_phase_six_operational_report_fixture,
    };

    #[test]
    fn phase_six_operational_report_is_valid() -> Result<(), Box<dyn std::error::Error>> {
        let report = canonical_compiled_agent_phase_six_operational_report()?;
        assert_eq!(
            report.schema_version,
            "psionic.compiled_agent.phase_six_operational_report.v1"
        );
        assert!(!report
            .route
            .permanent_regression_trap_receipt_ids
            .is_empty());
        Ok(())
    }

    #[test]
    fn phase_six_operational_report_fixture_matches_canonical(
    ) -> Result<(), Box<dyn std::error::Error>> {
        verify_compiled_agent_phase_six_operational_report_fixture()?;
        Ok(())
    }
}
