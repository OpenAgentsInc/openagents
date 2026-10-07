use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    compiled_agent_baseline_revision_set, evaluate_compiled_agent_grounded_answer,
    evaluate_compiled_agent_route, predict_compiled_agent_grounded_answer,
    predict_compiled_agent_route, CompiledAgentEvidenceClass,
    CompiledAgentGroundedAnswerPrediction, CompiledAgentModuleKind,
    CompiledAgentModuleRevisionSet, CompiledAgentPublicOutcomeKind, CompiledAgentRoute,
    CompiledAgentRoutePrediction, CompiledAgentToolResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_compiled_agent_learning_receipt_ledger,
    canonical_compiled_agent_promoted_artifact_contract,
    canonical_compiled_agent_runtime_source_receipts,
    canonical_compiled_agent_xtrain_cycle_receipt, repo_relative_path,
    CompiledAgentArtifactContractEntry, CompiledAgentArtifactContractError,
    CompiledAgentArtifactLifecycleState, CompiledAgentArtifactPayload,
    CompiledAgentCorpusSplit, CompiledAgentLearningReceipt, CompiledAgentPromotedArtifactContract,
    CompiledAgentReceiptError, CompiledAgentSourceReceipt, CompiledAgentXtrainError,
};

pub const COMPILED_AGENT_CONFIDENCE_POLICY_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_confidence_policy_v1.json";
pub const COMPILED_AGENT_SHADOW_DISAGREEMENT_RECEIPTS_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_shadow_disagreement_receipts_v1.json";

const COMPILED_AGENT_CONFIDENCE_POLICY_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.confidence_policy.v1";
const COMPILED_AGENT_SHADOW_DISAGREEMENT_RECEIPTS_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.shadow_disagreement_receipts.v1";

#[derive(Debug, Error)]
pub enum CompiledAgentShadowGovernanceError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error("missing promoted artifact entry for module `{module}`")]
    MissingPromotedEntry { module: String },
    #[error("missing candidate artifact entry for module `{module}`")]
    MissingCandidateEntry { module: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    ArtifactContract(#[from] CompiledAgentArtifactContractError),
    #[error(transparent)]
    Receipts(#[from] CompiledAgentReceiptError),
    #[error(transparent)]
    Xtrain(#[from] CompiledAgentXtrainError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentConfidenceBand {
    High,
    Watch,
    Review,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentDisagreementReason {
    CandidateImprovement,
    CandidateRegression,
    AmbiguousRegression,
    LowConfidenceDisagreement,
    RollbackRegression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentReviewDisposition {
    ShadowOnly,
    HumanReviewRequired,
    RollbackReady,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentConfidenceThresholds {
    pub high_confidence_min: f32,
    pub review_below: f32,
    pub promoted_confidence_floor: f32,
    pub rollback_on_heldout_regression_count: u32,
    pub rollback_on_runtime_regression_count: u32,
    pub human_review_on_low_confidence_disagreement: bool,
    pub human_review_on_ambiguous_regression: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentConfidenceMetrics {
    pub disagreement_count: u32,
    pub runtime_disagreement_count: u32,
    pub low_confidence_disagreement_count: u32,
    pub human_review_count: u32,
    pub rollback_ready_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned_correct_mean_confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned_incorrect_mean_confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned_min_confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned_max_confidence: Option<f32>,
    pub disagreement_receipt_ids: Vec<String>,
    pub human_review_receipt_ids: Vec<String>,
    pub rollback_ready_receipt_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentModuleConfidencePolicy {
    pub module: CompiledAgentModuleKind,
    pub promoted_artifact_id: String,
    pub promoted_artifact_digest: String,
    pub candidate_artifact_id: String,
    pub candidate_artifact_digest: String,
    pub candidate_label: String,
    pub measured_artifact_id: String,
    pub thresholds: CompiledAgentConfidenceThresholds,
    pub metrics: CompiledAgentConfidenceMetrics,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentConfidencePolicy {
    pub schema_version: String,
    pub policy_id: String,
    pub row_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub promoted_artifact_contract_digest: String,
    pub xtrain_cycle_receipt_digest: String,
    pub policies: Vec<CompiledAgentModuleConfidencePolicy>,
    pub summary: String,
    pub policy_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentShadowDisagreementReceipt {
    pub disagreement_id: String,
    pub module: CompiledAgentModuleKind,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub source_receipt_id: String,
    pub source_fixture_ref: String,
    pub source_receipt_digest: String,
    pub corpus_split: CompiledAgentCorpusSplit,
    pub promoted_artifact_id: String,
    pub promoted_artifact_digest: String,
    pub candidate_artifact_id: String,
    pub candidate_artifact_digest: String,
    pub candidate_label: String,
    pub promoted_output: Value,
    pub candidate_output: Value,
    pub expected_output: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promoted_confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_confidence: Option<f32>,
    pub promoted_band: CompiledAgentConfidenceBand,
    pub candidate_band: CompiledAgentConfidenceBand,
    pub reason: CompiledAgentDisagreementReason,
    pub disposition: CompiledAgentReviewDisposition,
    pub promoted_manifest_ids: Vec<String>,
    pub candidate_manifest_ids: Vec<String>,
    pub promoted_artifact_contract_digest: String,
    pub xtrain_cycle_receipt_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentShadowDisagreementReceipts {
    pub schema_version: String,
    pub ledger_id: String,
    pub row_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub promoted_artifact_contract_digest: String,
    pub xtrain_cycle_receipt_digest: String,
    pub disagreement_count: u32,
    pub human_review_count: u32,
    pub rollback_ready_count: u32,
    pub low_confidence_disagreement_count: u32,
    pub receipts: Vec<CompiledAgentShadowDisagreementReceipt>,
    pub summary: String,
    pub receipts_digest: String,
}

#[must_use]
pub fn compiled_agent_confidence_policy_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_CONFIDENCE_POLICY_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_shadow_disagreement_receipts_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_SHADOW_DISAGREEMENT_RECEIPTS_FIXTURE_PATH)
}

pub fn canonical_compiled_agent_confidence_policy(
) -> Result<CompiledAgentConfidencePolicy, CompiledAgentShadowGovernanceError> {
    let contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let xtrain_cycle = canonical_compiled_agent_xtrain_cycle_receipt()?;
    let receipts = canonical_compiled_agent_shadow_disagreement_receipts()?;

    let route_policy = build_module_confidence_policy(
        CompiledAgentModuleKind::Route,
        &contract,
        &receipts,
        &xtrain_cycle.receipt_digest,
    )?;
    let grounded_policy = build_module_confidence_policy(
        CompiledAgentModuleKind::GroundedAnswer,
        &contract,
        &receipts,
        &xtrain_cycle.receipt_digest,
    )?;

    let mut policy = CompiledAgentConfidencePolicy {
        schema_version: String::from(COMPILED_AGENT_CONFIDENCE_POLICY_SCHEMA_VERSION),
        policy_id: String::from("compiled_agent.confidence_policy.v1"),
        row_id: contract.row_id.clone(),
        evidence_class: contract.evidence_class,
        promoted_artifact_contract_digest: contract.contract_digest.clone(),
        xtrain_cycle_receipt_digest: xtrain_cycle.receipt_digest.clone(),
        policies: vec![route_policy, grounded_policy],
        summary: String::new(),
        policy_digest: String::new(),
    };
    policy.summary = format!(
        "Compiled-agent confidence policy retains {} module policies and {} disagreement receipts. Human review is queued for {} disagreements and rollback is codified for {} promoted regressions.",
        policy.policies.len(),
        receipts.disagreement_count,
        receipts.human_review_count,
        receipts.rollback_ready_count
    );
    policy.policy_digest = stable_digest(b"compiled_agent_confidence_policy|", &policy);
    Ok(policy)
}

pub fn canonical_compiled_agent_shadow_disagreement_receipts(
) -> Result<CompiledAgentShadowDisagreementReceipts, CompiledAgentShadowGovernanceError> {
    let ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let xtrain_cycle = canonical_compiled_agent_xtrain_cycle_receipt()?;

    let route_receipts = disagreement_receipts_for_module(
        CompiledAgentModuleKind::Route,
        &ledger.receipts,
        &contract,
        &xtrain_cycle.receipt_digest,
    )?;
    let grounded_receipts = disagreement_receipts_for_module(
        CompiledAgentModuleKind::GroundedAnswer,
        &ledger.receipts,
        &contract,
        &xtrain_cycle.receipt_digest,
    )?;
    let mut receipts = route_receipts
        .into_iter()
        .chain(grounded_receipts)
        .collect::<Vec<_>>();
    receipts.sort_by(|left, right| left.disagreement_id.cmp(&right.disagreement_id));

    let disagreement_count = receipts.len() as u32;
    let human_review_count = receipts
        .iter()
        .filter(|receipt| receipt.disposition == CompiledAgentReviewDisposition::HumanReviewRequired)
        .count() as u32;
    let rollback_ready_count = receipts
        .iter()
        .filter(|receipt| receipt.disposition == CompiledAgentReviewDisposition::RollbackReady)
        .count() as u32;
    let low_confidence_disagreement_count = receipts
        .iter()
        .filter(|receipt| receipt.reason == CompiledAgentDisagreementReason::LowConfidenceDisagreement)
        .count() as u32;

    let mut bundle = CompiledAgentShadowDisagreementReceipts {
        schema_version: String::from(COMPILED_AGENT_SHADOW_DISAGREEMENT_RECEIPTS_SCHEMA_VERSION),
        ledger_id: ledger.ledger_id.clone(),
        row_id: ledger.row_id.clone(),
        evidence_class: ledger.evidence_class,
        promoted_artifact_contract_digest: contract.contract_digest.clone(),
        xtrain_cycle_receipt_digest: xtrain_cycle.receipt_digest.clone(),
        disagreement_count,
        human_review_count,
        rollback_ready_count,
        low_confidence_disagreement_count,
        receipts,
        summary: String::new(),
        receipts_digest: String::new(),
    };
    bundle.summary = format!(
        "Compiled-agent shadow disagreement receipts retain {} promoted-versus-candidate disagreements for the current bounded family, including {} human-review triggers and {} rollback-ready regressions.",
        bundle.disagreement_count, bundle.human_review_count, bundle.rollback_ready_count
    );
    bundle.receipts_digest = stable_digest(
        b"compiled_agent_shadow_disagreement_receipts|",
        &bundle,
    );
    Ok(bundle)
}

pub fn write_compiled_agent_confidence_policy(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentConfidencePolicy, CompiledAgentShadowGovernanceError> {
    write_json_fixture(output_path, &canonical_compiled_agent_confidence_policy()?)
}

pub fn write_compiled_agent_shadow_disagreement_receipts(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentShadowDisagreementReceipts, CompiledAgentShadowGovernanceError> {
    write_json_fixture(output_path, &canonical_compiled_agent_shadow_disagreement_receipts()?)
}

pub fn verify_compiled_agent_shadow_governance_fixtures(
) -> Result<(), CompiledAgentShadowGovernanceError> {
    let expected_policy = canonical_compiled_agent_confidence_policy()?;
    let committed_policy: CompiledAgentConfidencePolicy = serde_json::from_slice(
        &fs::read(compiled_agent_confidence_policy_fixture_path()).map_err(|error| {
            CompiledAgentShadowGovernanceError::Read {
                path: compiled_agent_confidence_policy_fixture_path()
                    .display()
                    .to_string(),
                error,
            }
        })?,
    )?;
    if committed_policy != expected_policy {
        return Err(CompiledAgentShadowGovernanceError::FixtureDrift {
            path: compiled_agent_confidence_policy_fixture_path()
                .display()
                .to_string(),
        });
    }

    let expected_receipts = canonical_compiled_agent_shadow_disagreement_receipts()?;
    let committed_receipts: CompiledAgentShadowDisagreementReceipts = serde_json::from_slice(
        &fs::read(compiled_agent_shadow_disagreement_receipts_fixture_path()).map_err(|error| {
            CompiledAgentShadowGovernanceError::Read {
                path: compiled_agent_shadow_disagreement_receipts_fixture_path()
                    .display()
                    .to_string(),
                error,
            }
        })?,
    )?;
    if committed_receipts != expected_receipts {
        return Err(CompiledAgentShadowGovernanceError::FixtureDrift {
            path: compiled_agent_shadow_disagreement_receipts_fixture_path()
                .display()
                .to_string(),
        });
    }
    Ok(())
}

fn build_module_confidence_policy(
    module: CompiledAgentModuleKind,
    contract: &CompiledAgentPromotedArtifactContract,
    disagreement_receipts: &CompiledAgentShadowDisagreementReceipts,
    xtrain_cycle_receipt_digest: &str,
) -> Result<CompiledAgentModuleConfidencePolicy, CompiledAgentShadowGovernanceError> {
    let promoted_entry = contract
        .promoted_entry(module)
        .ok_or_else(|| CompiledAgentShadowGovernanceError::MissingPromotedEntry {
            module: format!("{module:?}"),
        })?;
    let candidate_entry = module_candidate_entry(module, contract)?;
    let thresholds = thresholds_for_module(module, promoted_entry.confidence_floor);
    let learned_revision = learned_revision_for_module(module, promoted_entry, candidate_entry);
    let ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let (correct_confidences, incorrect_confidences) =
        learned_confidence_samples(module, &ledger.receipts, &learned_revision);
    let module_receipts = disagreement_receipts
        .receipts
        .iter()
        .filter(|receipt| receipt.module == module)
        .collect::<Vec<_>>();

    Ok(CompiledAgentModuleConfidencePolicy {
        module,
        promoted_artifact_id: promoted_entry.artifact_id.clone(),
        promoted_artifact_digest: promoted_entry.artifact_digest.clone(),
        candidate_artifact_id: candidate_entry.artifact_id.clone(),
        candidate_artifact_digest: candidate_entry.artifact_digest.clone(),
        candidate_label: candidate_entry
            .candidate_label
            .clone()
            .unwrap_or_else(|| String::from("candidate")),
        measured_artifact_id: learned_revision.revision_id.clone(),
        thresholds,
        metrics: CompiledAgentConfidenceMetrics {
            disagreement_count: module_receipts.len() as u32,
            runtime_disagreement_count: module_receipts
                .iter()
                .filter(|receipt| receipt.source_fixture_ref.contains("/runtime/"))
                .count() as u32,
            low_confidence_disagreement_count: module_receipts
                .iter()
                .filter(|receipt| {
                    receipt.reason == CompiledAgentDisagreementReason::LowConfidenceDisagreement
                })
                .count() as u32,
            human_review_count: module_receipts
                .iter()
                .filter(|receipt| {
                    receipt.disposition == CompiledAgentReviewDisposition::HumanReviewRequired
                })
                .count() as u32,
            rollback_ready_count: module_receipts
                .iter()
                .filter(|receipt| {
                    receipt.disposition == CompiledAgentReviewDisposition::RollbackReady
                })
                .count() as u32,
            learned_correct_mean_confidence: mean(&correct_confidences),
            learned_incorrect_mean_confidence: mean(&incorrect_confidences),
            learned_min_confidence: min_value(&correct_confidences, &incorrect_confidences),
            learned_max_confidence: max_value(&correct_confidences, &incorrect_confidences),
            disagreement_receipt_ids: module_receipts
                .iter()
                .map(|receipt| receipt.disagreement_id.clone())
                .collect(),
            human_review_receipt_ids: module_receipts
                .iter()
                .filter(|receipt| {
                    receipt.disposition == CompiledAgentReviewDisposition::HumanReviewRequired
                })
                .map(|receipt| receipt.disagreement_id.clone())
                .collect(),
            rollback_ready_receipt_ids: module_receipts
                .iter()
                .filter(|receipt| {
                    receipt.disposition == CompiledAgentReviewDisposition::RollbackReady
                })
                .map(|receipt| receipt.disagreement_id.clone())
                .collect(),
        },
        detail: format!(
            "Confidence policy for {:?} is regenerated from the promoted-artifact contract, learning ledger, and XTRAIN receipt {} so shadow disagreements, low-confidence bands, and rollback-ready regressions remain machine-auditable instead of ad hoc.",
            module, xtrain_cycle_receipt_digest
        ),
    })
}

fn disagreement_receipts_for_module(
    module: CompiledAgentModuleKind,
    learning_receipts: &[CompiledAgentLearningReceipt],
    contract: &CompiledAgentPromotedArtifactContract,
    xtrain_cycle_receipt_digest: &str,
) -> Result<Vec<CompiledAgentShadowDisagreementReceipt>, CompiledAgentShadowGovernanceError> {
    let promoted_entry = contract
        .promoted_entry(module)
        .ok_or_else(|| CompiledAgentShadowGovernanceError::MissingPromotedEntry {
            module: format!("{module:?}"),
        })?;
    let candidate_entry = module_candidate_entry(module, contract)?;
    let promoted_revision = revision_from_entry(promoted_entry);
    let candidate_revision = revision_from_entry(candidate_entry);
    let thresholds = thresholds_for_module(module, promoted_entry.confidence_floor);
    let runtime_receipts = canonical_compiled_agent_runtime_source_receipts()
        .into_iter()
        .map(|(fixture_name, receipt)| {
            (
                format!("fixtures/compiled_agent/runtime/{fixture_name}"),
                receipt,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    let receipts = learning_receipts
        .iter()
        .filter_map(|receipt| {
            disagreement_for_receipt(
                module,
                receipt,
                promoted_entry,
                candidate_entry,
                &promoted_revision,
                &candidate_revision,
                &thresholds,
                &contract.contract_digest,
                xtrain_cycle_receipt_digest,
                runtime_receipts.get(receipt.source_fixture_ref.as_str()),
            )
        })
        .collect();
    Ok(receipts)
}

#[allow(clippy::too_many_arguments)]
fn disagreement_for_receipt(
    module: CompiledAgentModuleKind,
    receipt: &CompiledAgentLearningReceipt,
    promoted_entry: &CompiledAgentArtifactContractEntry,
    candidate_entry: &CompiledAgentArtifactContractEntry,
    promoted_revision: &CompiledAgentModuleRevisionSet,
    candidate_revision: &CompiledAgentModuleRevisionSet,
    thresholds: &CompiledAgentConfidenceThresholds,
    promoted_artifact_contract_digest: &str,
    xtrain_cycle_receipt_digest: &str,
    runtime_source_receipt: Option<&CompiledAgentSourceReceipt>,
) -> Option<CompiledAgentShadowDisagreementReceipt> {
    let evaluated = match module {
        CompiledAgentModuleKind::Route => runtime_source_receipt
            .and_then(|source| {
                evaluate_runtime_route_disagreement(
                    receipt,
                    source,
                    promoted_entry,
                    candidate_entry,
                    thresholds,
                )
            })
            .or_else(|| {
                evaluate_route_disagreement(
                    receipt,
                    promoted_entry,
                    candidate_entry,
                    promoted_revision,
                    candidate_revision,
                    thresholds,
                )
            }),
        CompiledAgentModuleKind::GroundedAnswer => runtime_source_receipt
            .and_then(|source| {
                evaluate_runtime_grounded_disagreement(
                    receipt,
                    source,
                    promoted_entry,
                    candidate_entry,
                    thresholds,
                )
            })
            .or_else(|| {
                evaluate_grounded_disagreement(
                    receipt,
                    promoted_entry,
                    candidate_entry,
                    promoted_revision,
                    candidate_revision,
                    thresholds,
                )
            }),
        _ => None,
    }?;

    Some(CompiledAgentShadowDisagreementReceipt {
        disagreement_id: format!(
            "disagreement.compiled_agent.{}.{}",
            module_slug(module),
            fixture_slug(receipt.receipt_id.as_str())
        ),
        module,
        evidence_class: receipt.evidence_class,
        source_receipt_id: receipt.receipt_id.clone(),
        source_fixture_ref: receipt.source_fixture_ref.clone(),
        source_receipt_digest: receipt.source_receipt_digest.clone(),
        corpus_split: receipt.corpus_split,
        promoted_artifact_id: promoted_entry.artifact_id.clone(),
        promoted_artifact_digest: promoted_entry.artifact_digest.clone(),
        candidate_artifact_id: candidate_entry.artifact_id.clone(),
        candidate_artifact_digest: candidate_entry.artifact_digest.clone(),
        candidate_label: candidate_entry
            .candidate_label
            .clone()
            .unwrap_or_else(|| String::from("candidate")),
        promoted_output: evaluated.promoted_output,
        candidate_output: evaluated.candidate_output,
        expected_output: evaluated.expected_output,
        promoted_confidence: evaluated.promoted_confidence,
        candidate_confidence: evaluated.candidate_confidence,
        promoted_band: evaluated.promoted_band,
        candidate_band: evaluated.candidate_band,
        reason: evaluated.reason,
        disposition: evaluated.disposition,
        promoted_manifest_ids: receipt.authority_manifest_ids.clone(),
        candidate_manifest_ids: receipt.shadow_manifest_ids.clone(),
        promoted_artifact_contract_digest: promoted_artifact_contract_digest.to_string(),
        xtrain_cycle_receipt_digest: xtrain_cycle_receipt_digest.to_string(),
        detail: evaluated.detail,
    })
}

fn evaluate_runtime_route_disagreement(
    receipt: &CompiledAgentLearningReceipt,
    source_receipt: &CompiledAgentSourceReceipt,
    promoted_entry: &CompiledAgentArtifactContractEntry,
    candidate_entry: &CompiledAgentArtifactContractEntry,
    thresholds: &CompiledAgentConfidenceThresholds,
) -> Option<EvaluatedDisagreement> {
    let candidate_phase = source_receipt
        .run
        .internal_trace
        .shadow_phases
        .iter()
        .find(|phase| phase.phase == "intent_route")?;
    let candidate_route: CompiledAgentRoute =
        serde_json::from_value(candidate_phase.output.get("route")?.clone()).ok()?;
    let promoted_route = receipt.observed_route;
    if promoted_route == candidate_route {
        return None;
    }
    let promoted_confidence = receipt.primary_phase_confidences.get("intent_route").copied();
    let candidate_confidence = Some(candidate_phase.confidence);
    let promoted_band = band_for_confidence(promoted_confidence, thresholds);
    let candidate_band = band_for_confidence(candidate_confidence, thresholds);
    let promoted_matches_expected = promoted_route == receipt.expected_route;
    let candidate_matches_expected = candidate_route == receipt.expected_route;
    let low_confidence = promoted_band == CompiledAgentConfidenceBand::Review
        || candidate_band == CompiledAgentConfidenceBand::Review;
    let (reason, disposition) = if !promoted_matches_expected && candidate_matches_expected {
        (
            CompiledAgentDisagreementReason::LowConfidenceDisagreement,
            CompiledAgentReviewDisposition::HumanReviewRequired,
        )
    } else if promoted_matches_expected && !candidate_matches_expected {
        if low_confidence {
            (
                CompiledAgentDisagreementReason::LowConfidenceDisagreement,
                CompiledAgentReviewDisposition::HumanReviewRequired,
            )
        } else {
            (
                CompiledAgentDisagreementReason::CandidateRegression,
                CompiledAgentReviewDisposition::ShadowOnly,
            )
        }
    } else {
        (
            CompiledAgentDisagreementReason::AmbiguousRegression,
            CompiledAgentReviewDisposition::HumanReviewRequired,
        )
    };

    Some(EvaluatedDisagreement {
        promoted_output: json!({ "route": promoted_route }),
        candidate_output: json!({ "route": candidate_route }),
        expected_output: json!({ "route": receipt.expected_route }),
        promoted_confidence,
        candidate_confidence,
        promoted_band,
        candidate_band,
        reason,
        disposition,
        detail: format!(
            "Runtime route disagreement for `{}` retained the promoted artifact `{}` versus shadow candidate `{}` exactly as captured in the admitted-family runtime trace.",
            receipt.receipt_id, promoted_entry.artifact_id, candidate_entry.artifact_id
        ),
    })
}

fn evaluate_runtime_grounded_disagreement(
    receipt: &CompiledAgentLearningReceipt,
    source_receipt: &CompiledAgentSourceReceipt,
    promoted_entry: &CompiledAgentArtifactContractEntry,
    candidate_entry: &CompiledAgentArtifactContractEntry,
    thresholds: &CompiledAgentConfidenceThresholds,
) -> Option<EvaluatedDisagreement> {
    let candidate_phase = source_receipt
        .run
        .internal_trace
        .shadow_phases
        .iter()
        .find(|phase| phase.phase == "grounded_answer")?;
    let candidate_kind: CompiledAgentPublicOutcomeKind = serde_json::from_value(
        candidate_phase
            .output
            .get("response_kind")
            .cloned()
            .unwrap_or_else(|| json!(CompiledAgentPublicOutcomeKind::GroundedAnswer)),
    )
    .ok()?;
    let candidate_response = candidate_phase
        .output
        .get("answer")
        .and_then(Value::as_str)?
        .to_string();
    let promoted_kind = receipt.observed_public_response.kind;
    let promoted_response = receipt.observed_public_response.response.clone();
    if promoted_kind == candidate_kind && promoted_response == candidate_response {
        return None;
    }
    let promoted_confidence = receipt.primary_phase_confidences.get("grounded_answer").copied();
    let candidate_confidence = Some(candidate_phase.confidence);
    let promoted_band = band_for_confidence(promoted_confidence, thresholds);
    let candidate_band = band_for_confidence(candidate_confidence, thresholds);
    let promoted_matches_expected = promoted_kind == receipt.expected_public_response.kind
        && promoted_response == receipt.expected_public_response.response;
    let candidate_matches_expected = candidate_kind == receipt.expected_public_response.kind
        && candidate_response == receipt.expected_public_response.response;
    let low_confidence = promoted_band == CompiledAgentConfidenceBand::Review
        || candidate_band == CompiledAgentConfidenceBand::Review;
    let (reason, disposition) = if !promoted_matches_expected && candidate_matches_expected {
        (
            CompiledAgentDisagreementReason::LowConfidenceDisagreement,
            CompiledAgentReviewDisposition::HumanReviewRequired,
        )
    } else if promoted_matches_expected && !candidate_matches_expected {
        if low_confidence {
            (
                CompiledAgentDisagreementReason::LowConfidenceDisagreement,
                CompiledAgentReviewDisposition::HumanReviewRequired,
            )
        } else {
            (
                CompiledAgentDisagreementReason::CandidateRegression,
                CompiledAgentReviewDisposition::ShadowOnly,
            )
        }
    } else {
        (
            CompiledAgentDisagreementReason::AmbiguousRegression,
            CompiledAgentReviewDisposition::HumanReviewRequired,
        )
    };

    Some(EvaluatedDisagreement {
        promoted_output: json!({
            "kind": promoted_kind,
            "response": promoted_response,
        }),
        candidate_output: json!({
            "kind": candidate_kind,
            "response": candidate_response,
        }),
        expected_output: json!({
            "kind": receipt.expected_public_response.kind,
            "response": receipt.expected_public_response.response,
        }),
        promoted_confidence,
        candidate_confidence,
        promoted_band,
        candidate_band,
        reason,
        disposition,
        detail: format!(
            "Runtime grounded-answer disagreement for `{}` retained the promoted artifact `{}` versus shadow candidate `{}` exactly as captured in the admitted-family runtime trace.",
            receipt.receipt_id, promoted_entry.artifact_id, candidate_entry.artifact_id
        ),
    })
}

struct EvaluatedDisagreement {
    promoted_output: Value,
    candidate_output: Value,
    expected_output: Value,
    promoted_confidence: Option<f32>,
    candidate_confidence: Option<f32>,
    promoted_band: CompiledAgentConfidenceBand,
    candidate_band: CompiledAgentConfidenceBand,
    reason: CompiledAgentDisagreementReason,
    disposition: CompiledAgentReviewDisposition,
    detail: String,
}

fn evaluate_route_disagreement(
    receipt: &CompiledAgentLearningReceipt,
    promoted_entry: &CompiledAgentArtifactContractEntry,
    candidate_entry: &CompiledAgentArtifactContractEntry,
    promoted_revision: &CompiledAgentModuleRevisionSet,
    candidate_revision: &CompiledAgentModuleRevisionSet,
    thresholds: &CompiledAgentConfidenceThresholds,
) -> Option<EvaluatedDisagreement> {
    let promoted_prediction =
        route_prediction_for_revision(promoted_revision, receipt.user_request.as_str(), receipt);
    let candidate_prediction =
        route_prediction_for_revision(candidate_revision, receipt.user_request.as_str(), receipt);
    if promoted_prediction.route == candidate_prediction.route {
        return None;
    }

    let promoted_band = band_for_confidence(promoted_prediction.confidence, thresholds);
    let candidate_band = band_for_confidence(candidate_prediction.confidence, thresholds);
    let promoted_matches_expected = promoted_prediction.route == receipt.expected_route;
    let candidate_matches_expected = candidate_prediction.route == receipt.expected_route;
    let low_confidence = promoted_band == CompiledAgentConfidenceBand::Review
        || candidate_band == CompiledAgentConfidenceBand::Review;
    let (reason, disposition) = if !promoted_matches_expected && candidate_matches_expected {
        if low_confidence || receipt.source_fixture_ref.contains("/runtime/") {
            (
                CompiledAgentDisagreementReason::LowConfidenceDisagreement,
                CompiledAgentReviewDisposition::HumanReviewRequired,
            )
        } else {
            (
                CompiledAgentDisagreementReason::CandidateImprovement,
                CompiledAgentReviewDisposition::ShadowOnly,
            )
        }
    } else if promoted_matches_expected && !candidate_matches_expected {
        if low_confidence {
            (
                CompiledAgentDisagreementReason::LowConfidenceDisagreement,
                CompiledAgentReviewDisposition::HumanReviewRequired,
            )
        } else {
            (
                CompiledAgentDisagreementReason::CandidateRegression,
                CompiledAgentReviewDisposition::ShadowOnly,
            )
        }
    } else {
        (
            CompiledAgentDisagreementReason::AmbiguousRegression,
            CompiledAgentReviewDisposition::HumanReviewRequired,
        )
    };

    Some(EvaluatedDisagreement {
        promoted_output: json!({
            "route": promoted_prediction.route,
            "score_margin": promoted_prediction.score_margin,
        }),
        candidate_output: json!({
            "route": candidate_prediction.route,
            "score_margin": candidate_prediction.score_margin,
        }),
        expected_output: json!({
            "route": receipt.expected_route,
        }),
        promoted_confidence: promoted_prediction.confidence,
        candidate_confidence: candidate_prediction.confidence,
        promoted_band,
        candidate_band,
        reason,
        disposition,
        detail: format!(
            "Route disagreement for `{}` retained the promoted artifact `{}` versus candidate `{}`. Promoted route {:?}, candidate route {:?}, expected {:?}.",
            receipt.receipt_id,
            promoted_entry.artifact_id,
            candidate_entry.artifact_id,
            promoted_prediction.route,
            candidate_prediction.route,
            receipt.expected_route
        ),
    })
}

fn evaluate_grounded_disagreement(
    receipt: &CompiledAgentLearningReceipt,
    promoted_entry: &CompiledAgentArtifactContractEntry,
    candidate_entry: &CompiledAgentArtifactContractEntry,
    promoted_revision: &CompiledAgentModuleRevisionSet,
    candidate_revision: &CompiledAgentModuleRevisionSet,
    thresholds: &CompiledAgentConfidenceThresholds,
) -> Option<EvaluatedDisagreement> {
    let promoted_prediction = grounded_prediction_for_revision(
        promoted_revision,
        receipt.expected_route,
        receipt.observed_tool_results.as_slice(),
    );
    let candidate_prediction = grounded_prediction_for_revision(
        candidate_revision,
        receipt.expected_route,
        receipt.observed_tool_results.as_slice(),
    );
    if promoted_prediction.response == candidate_prediction.response
        && promoted_prediction.kind == candidate_prediction.kind
    {
        return None;
    }

    let promoted_band = band_for_confidence(promoted_prediction.confidence, thresholds);
    let candidate_band = band_for_confidence(candidate_prediction.confidence, thresholds);
    let promoted_matches_expected = promoted_prediction.kind == receipt.expected_public_response.kind
        && promoted_prediction.response == receipt.expected_public_response.response;
    let candidate_matches_expected = candidate_prediction.kind == receipt.expected_public_response.kind
        && candidate_prediction.response == receipt.expected_public_response.response;
    let low_confidence = promoted_band == CompiledAgentConfidenceBand::Review
        || candidate_band == CompiledAgentConfidenceBand::Review;
    let (reason, disposition) = if !promoted_matches_expected
        && candidate_matches_expected
        && candidate_entry.candidate_label.as_deref() == Some("last_known_good")
    {
        (
            CompiledAgentDisagreementReason::RollbackRegression,
            CompiledAgentReviewDisposition::RollbackReady,
        )
    } else if !promoted_matches_expected && candidate_matches_expected {
        if low_confidence {
            (
                CompiledAgentDisagreementReason::LowConfidenceDisagreement,
                CompiledAgentReviewDisposition::HumanReviewRequired,
            )
        } else {
            (
                CompiledAgentDisagreementReason::CandidateImprovement,
                CompiledAgentReviewDisposition::ShadowOnly,
            )
        }
    } else if promoted_matches_expected && !candidate_matches_expected {
        if low_confidence {
            (
                CompiledAgentDisagreementReason::LowConfidenceDisagreement,
                CompiledAgentReviewDisposition::HumanReviewRequired,
            )
        } else {
            (
                CompiledAgentDisagreementReason::CandidateRegression,
                CompiledAgentReviewDisposition::ShadowOnly,
            )
        }
    } else {
        (
            CompiledAgentDisagreementReason::AmbiguousRegression,
            CompiledAgentReviewDisposition::HumanReviewRequired,
        )
    };

    Some(EvaluatedDisagreement {
        promoted_output: json!({
            "kind": promoted_prediction.kind,
            "response": promoted_prediction.response,
            "fallback_reason": promoted_prediction.fallback_reason,
            "score_margin": promoted_prediction.score_margin,
        }),
        candidate_output: json!({
            "kind": candidate_prediction.kind,
            "response": candidate_prediction.response,
            "fallback_reason": candidate_prediction.fallback_reason,
            "score_margin": candidate_prediction.score_margin,
        }),
        expected_output: json!({
            "kind": receipt.expected_public_response.kind,
            "response": receipt.expected_public_response.response,
        }),
        promoted_confidence: promoted_prediction.confidence,
        candidate_confidence: candidate_prediction.confidence,
        promoted_band,
        candidate_band,
        reason,
        disposition,
        detail: format!(
            "Grounded-answer disagreement for `{}` retained the promoted artifact `{}` versus candidate `{}`. Promoted kind {:?}, candidate kind {:?}.",
            receipt.receipt_id,
            promoted_entry.artifact_id,
            candidate_entry.artifact_id,
            promoted_prediction.kind,
            candidate_prediction.kind
        ),
    })
}

fn module_candidate_entry<'a>(
    module: CompiledAgentModuleKind,
    contract: &'a CompiledAgentPromotedArtifactContract,
) -> Result<&'a CompiledAgentArtifactContractEntry, CompiledAgentShadowGovernanceError> {
    let preferred_labels: &[&str] = match module {
        CompiledAgentModuleKind::Route => &["psionic_candidate", "last_known_good"],
        CompiledAgentModuleKind::GroundedAnswer => &["last_known_good", "psionic_candidate"],
        _ => &["candidate", "last_known_good"],
    };
    preferred_labels
        .iter()
        .find_map(|label| contract.candidate_entry(module, label))
        .or_else(|| {
            contract.artifacts.iter().find(|entry| {
                entry.module == module
                    && entry.lifecycle_state == CompiledAgentArtifactLifecycleState::Candidate
            })
        })
        .ok_or_else(|| CompiledAgentShadowGovernanceError::MissingCandidateEntry {
            module: format!("{module:?}"),
        })
}

fn thresholds_for_module(
    module: CompiledAgentModuleKind,
    promoted_confidence_floor: f32,
) -> CompiledAgentConfidenceThresholds {
    match module {
        CompiledAgentModuleKind::Route => CompiledAgentConfidenceThresholds {
            high_confidence_min: 0.80,
            review_below: 0.60,
            promoted_confidence_floor,
            rollback_on_heldout_regression_count: 1,
            rollback_on_runtime_regression_count: 1,
            human_review_on_low_confidence_disagreement: true,
            human_review_on_ambiguous_regression: true,
        },
        CompiledAgentModuleKind::GroundedAnswer => CompiledAgentConfidenceThresholds {
            high_confidence_min: 0.85,
            review_below: 0.65,
            promoted_confidence_floor,
            rollback_on_heldout_regression_count: 1,
            rollback_on_runtime_regression_count: 1,
            human_review_on_low_confidence_disagreement: true,
            human_review_on_ambiguous_regression: true,
        },
        _ => CompiledAgentConfidenceThresholds {
            high_confidence_min: 0.80,
            review_below: 0.60,
            promoted_confidence_floor,
            rollback_on_heldout_regression_count: 1,
            rollback_on_runtime_regression_count: 1,
            human_review_on_low_confidence_disagreement: true,
            human_review_on_ambiguous_regression: true,
        },
    }
}

fn learned_revision_for_module(
    module: CompiledAgentModuleKind,
    promoted_entry: &CompiledAgentArtifactContractEntry,
    candidate_entry: &CompiledAgentArtifactContractEntry,
) -> CompiledAgentModuleRevisionSet {
    match module {
        CompiledAgentModuleKind::Route => revision_from_entry(candidate_entry),
        CompiledAgentModuleKind::GroundedAnswer => revision_from_entry(promoted_entry),
        _ => revision_from_entry(promoted_entry),
    }
}

fn learned_confidence_samples(
    module: CompiledAgentModuleKind,
    receipts: &[CompiledAgentLearningReceipt],
    learned_revision: &CompiledAgentModuleRevisionSet,
) -> (Vec<f32>, Vec<f32>) {
    let mut correct = Vec::new();
    let mut incorrect = Vec::new();
    for receipt in receipts {
        let observed = match module {
            CompiledAgentModuleKind::Route => {
                let prediction =
                    route_prediction_for_revision(learned_revision, receipt.user_request.as_str(), receipt);
                prediction.confidence.map(|confidence| {
                    (
                        prediction.route == receipt.expected_route,
                        confidence,
                    )
                })
            }
            CompiledAgentModuleKind::GroundedAnswer => {
                let prediction = grounded_prediction_for_revision(
                    learned_revision,
                    receipt.expected_route,
                    receipt.observed_tool_results.as_slice(),
                );
                prediction.confidence.map(|confidence| {
                    (
                        prediction.kind == receipt.expected_public_response.kind
                            && prediction.response == receipt.expected_public_response.response,
                        confidence,
                    )
                })
            }
            _ => None,
        };
        if let Some((is_correct, confidence)) = observed {
            if is_correct {
                correct.push(confidence);
            } else {
                incorrect.push(confidence);
            }
        }
    }
    (correct, incorrect)
}

fn revision_from_entry(entry: &CompiledAgentArtifactContractEntry) -> CompiledAgentModuleRevisionSet {
    match &entry.payload {
        CompiledAgentArtifactPayload::RevisionSet { revision } => revision.clone(),
        CompiledAgentArtifactPayload::RouteModel { artifact } => {
            let mut revision = compiled_agent_baseline_revision_set();
            revision.revision_id = artifact.artifact_id.clone();
            revision.route_model_artifact = Some(artifact.clone());
            revision
        }
        // A coordinator-head candidate is not a route/grounded module revision —
        // it is a frozen-backbone routing head packaged under
        // `AdapterArtifactKind::CoordinatorHead`. The compiled-agent
        // revision-set shadow surface does not model it, so it carries the
        // baseline revision identity here (its real shadow gate is the
        // coordinator `ShadowComparison`, not this revision-set comparison).
        CompiledAgentArtifactPayload::CoordinatorHead { artifact } => {
            let mut revision = compiled_agent_baseline_revision_set();
            revision.revision_id = artifact.artifact_id();
            revision
        }
    }
}

fn route_prediction_for_revision(
    revision: &CompiledAgentModuleRevisionSet,
    prompt: &str,
    receipt: &CompiledAgentLearningReceipt,
) -> RouteShadowPrediction {
    if let Some(artifact) = revision.route_model_artifact.as_ref() {
        let prediction: CompiledAgentRoutePrediction = predict_compiled_agent_route(artifact, prompt);
        RouteShadowPrediction {
            route: prediction.route,
            confidence: Some(prediction.confidence),
            score_margin: Some(prediction.score_margin),
        }
    } else {
        RouteShadowPrediction {
            route: evaluate_compiled_agent_route(prompt, revision),
            confidence: receipt.primary_phase_confidences.get("intent_route").copied(),
            score_margin: None,
        }
    }
}

fn grounded_prediction_for_revision(
    revision: &CompiledAgentModuleRevisionSet,
    route: CompiledAgentRoute,
    tool_results: &[CompiledAgentToolResult],
) -> GroundedShadowPrediction {
    if let Some(artifact) = revision.grounded_answer_model_artifact.as_ref() {
        let prediction: CompiledAgentGroundedAnswerPrediction =
            predict_compiled_agent_grounded_answer(artifact, route, tool_results);
        GroundedShadowPrediction {
            kind: prediction.outcome_kind,
            response: prediction.response,
            confidence: Some(prediction.confidence),
            score_margin: Some(prediction.score_margin),
            fallback_reason: prediction.fallback_reason,
        }
    } else {
        let response = evaluate_compiled_agent_grounded_answer(route, tool_results, revision);
        GroundedShadowPrediction {
            kind: match route {
                CompiledAgentRoute::Unsupported => CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                CompiledAgentRoute::ProviderStatus | CompiledAgentRoute::WalletStatus => {
                    CompiledAgentPublicOutcomeKind::GroundedAnswer
                }
            },
            response,
            confidence: None,
            score_margin: None,
            fallback_reason: None,
        }
    }
}

#[derive(Clone, Copy)]
struct RouteShadowPrediction {
    route: CompiledAgentRoute,
    confidence: Option<f32>,
    score_margin: Option<f32>,
}

struct GroundedShadowPrediction {
    kind: CompiledAgentPublicOutcomeKind,
    response: String,
    confidence: Option<f32>,
    score_margin: Option<f32>,
    fallback_reason: Option<String>,
}

fn band_for_confidence(
    confidence: Option<f32>,
    thresholds: &CompiledAgentConfidenceThresholds,
) -> CompiledAgentConfidenceBand {
    match confidence {
        None => CompiledAgentConfidenceBand::Unavailable,
        Some(confidence) if confidence < thresholds.review_below => {
            CompiledAgentConfidenceBand::Review
        }
        Some(confidence) if confidence < thresholds.high_confidence_min => {
            CompiledAgentConfidenceBand::Watch
        }
        Some(_) => CompiledAgentConfidenceBand::High,
    }
}

fn module_slug(module: CompiledAgentModuleKind) -> &'static str {
    match module {
        CompiledAgentModuleKind::Route => "route",
        CompiledAgentModuleKind::ToolPolicy => "tool_policy",
        CompiledAgentModuleKind::ToolArguments => "tool_arguments",
        CompiledAgentModuleKind::GroundedAnswer => "grounded_answer",
        CompiledAgentModuleKind::Verify => "verify",
    }
}

fn fixture_slug(value: &str) -> String {
    value
        .replace("receipt.compiled_agent.learning.", "")
        .replace(['/', ':'], "_")
        .replace(".json", "")
}

fn mean(values: &[f32]) -> Option<f32> {
    (!values.is_empty()).then(|| values.iter().sum::<f32>() / values.len() as f32)
}

fn min_value(first: &[f32], second: &[f32]) -> Option<f32> {
    first
        .iter()
        .chain(second.iter())
        .copied()
        .min_by(|left, right| left.total_cmp(right))
}

fn max_value(first: &[f32], second: &[f32]) -> Option<f32> {
    first
        .iter()
        .chain(second.iter())
        .copied()
        .max_by(|left, right| left.total_cmp(right))
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn write_json_fixture<T: Serialize + Clone>(
    output_path: impl AsRef<Path>,
    value: &T,
) -> Result<T, CompiledAgentShadowGovernanceError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CompiledAgentShadowGovernanceError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentShadowGovernanceError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(value.clone())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_compiled_agent_confidence_policy,
        canonical_compiled_agent_shadow_disagreement_receipts,
        compiled_agent_confidence_policy_fixture_path,
        compiled_agent_shadow_disagreement_receipts_fixture_path,
        verify_compiled_agent_shadow_governance_fixtures, CompiledAgentModuleKind,
        CompiledAgentReviewDisposition,
    };

    #[test]
    fn compiled_agent_shadow_governance_retains_reviewable_disagreements(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let receipts = canonical_compiled_agent_shadow_disagreement_receipts()?;
        assert!(receipts.disagreement_count >= 4);
        assert!(receipts.human_review_count >= 1);
        assert!(receipts
            .receipts
            .iter()
            .any(|receipt| receipt.module == CompiledAgentModuleKind::Route));
        assert!(receipts.receipts.iter().any(|receipt| {
            receipt.disposition == CompiledAgentReviewDisposition::HumanReviewRequired
        }));
        Ok(())
    }

    #[test]
    fn compiled_agent_confidence_policy_tracks_module_disagreement_metrics(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let policy = canonical_compiled_agent_confidence_policy()?;
        assert_eq!(policy.policies.len(), 2);
        assert!(policy
            .policies
            .iter()
            .any(|module| module.module == CompiledAgentModuleKind::GroundedAnswer));
        assert!(policy
            .policies
            .iter()
            .flat_map(|module| module.metrics.human_review_receipt_ids.iter())
            .count()
            >= 1);
        Ok(())
    }

    #[test]
    fn compiled_agent_shadow_governance_fixtures_match_the_canonical_generator(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert!(compiled_agent_confidence_policy_fixture_path().exists());
        assert!(compiled_agent_shadow_disagreement_receipts_fixture_path().exists());
        verify_compiled_agent_shadow_governance_fixtures()?;
        Ok(())
    }
}
