use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    build_compiled_agent_module_eval_report, compiled_agent_baseline_revision_set,
    evaluate_compiled_agent_grounded_answer, evaluate_compiled_agent_route,
    train_compiled_agent_grounded_answer_model,
    train_compiled_agent_grounded_answer_tfidf_centroid_model, train_compiled_agent_route_model,
    train_compiled_agent_route_tfidf_centroid_model, CompiledAgentEvidenceClass,
    CompiledAgentGroundedAnswerModelArtifact, CompiledAgentGroundedAnswerTrainingSample,
    CompiledAgentModuleEvalReport, CompiledAgentModuleKind, CompiledAgentModuleRevisionSet,
    CompiledAgentPublicOutcomeKind, CompiledAgentRoute, CompiledAgentRouteModelArtifact,
    CompiledAgentRouteTrainingSample, CompiledAgentToolResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_compiled_agent_learning_receipt_ledger, canonical_compiled_agent_replay_bundle,
    repo_relative_path, CompiledAgentCorpusSplit, CompiledAgentReceiptError,
    CompiledAgentReplayBundle,
};

pub const COMPILED_AGENT_ROUTE_MODEL_ARTIFACT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_route_model_v1.json";
pub const COMPILED_AGENT_GROUNDED_MODEL_ARTIFACT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_grounded_answer_model_v1.json";
pub const COMPILED_AGENT_ROUTE_CANDIDATE_REPORT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_route_candidate_module_eval_report_v1.json";
pub const COMPILED_AGENT_GROUNDED_CANDIDATE_REPORT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_grounded_candidate_module_eval_report_v1.json";
pub const COMPILED_AGENT_ROUTE_STRONGER_MODEL_ARTIFACT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_route_tfidf_centroid_model_v1.json";
pub const COMPILED_AGENT_GROUNDED_STRONGER_MODEL_ARTIFACT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_grounded_answer_tfidf_centroid_model_v1.json";
pub const COMPILED_AGENT_ROUTE_STRONGER_CANDIDATE_REPORT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_route_tfidf_centroid_candidate_module_eval_report_v1.json";
pub const COMPILED_AGENT_GROUNDED_STRONGER_CANDIDATE_REPORT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_grounded_tfidf_centroid_candidate_module_eval_report_v1.json";
pub const COMPILED_AGENT_STRONGER_CANDIDATE_FAMILY_REPORT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_stronger_candidate_family_report_v1.json";
pub const COMPILED_AGENT_XTRAIN_CYCLE_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_xtrain_cycle_receipt_v1.json";

const XTRAIN_CYCLE_SCHEMA_VERSION: &str = "psionic.compiled_agent.xtrain_cycle.v1";

#[derive(Debug, Error)]
pub enum CompiledAgentXtrainError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("module summary for `{module}` missing from eval report")]
    MissingModuleSummary { module: String },
    #[error("route replay sample `{sample_id}` is missing `user_request`")]
    MissingRoutePrompt { sample_id: String },
    #[error("grounded replay sample `{sample_id}` is missing `route`")]
    MissingGroundedRoute { sample_id: String },
    #[error("grounded replay sample `{sample_id}` has invalid tool results")]
    InvalidGroundedToolResults { sample_id: String },
    #[error(
        "compiled-agent evidence class drifted in `{context}`: expected `{expected:?}` but found `{actual:?}`"
    )]
    MixedEvidenceClass {
        context: String,
        expected: CompiledAgentEvidenceClass,
        actual: CompiledAgentEvidenceClass,
    },
    #[error(transparent)]
    Receipts(#[from] CompiledAgentReceiptError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentPromotionDecision {
    Promote,
    Hold,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentCandidateDelta {
    pub module: CompiledAgentModuleKind,
    pub base_revision_id: String,
    pub candidate_revision_id: String,
    pub replay_bundle_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_artifact_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_artifact_digest: Option<String>,
    pub targeted_failure_classes: Vec<String>,
    pub delta_summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentModuleValidation {
    pub module: CompiledAgentModuleKind,
    pub baseline_passed_cases: u32,
    pub candidate_passed_cases: u32,
    pub improvement_case_ids: Vec<String>,
    pub regression_case_ids: Vec<String>,
    pub baseline_replay_match_count: u32,
    pub candidate_replay_match_count: u32,
    pub baseline_heldout_match_count: u32,
    pub candidate_heldout_match_count: u32,
    pub replay_improvement_sample_ids: Vec<String>,
    pub replay_regression_sample_ids: Vec<String>,
    pub heldout_improvement_receipt_ids: Vec<String>,
    pub heldout_regression_receipt_ids: Vec<String>,
}

impl CompiledAgentModuleValidation {
    fn stronger_than(&self, other: &Self) -> bool {
        self.candidate_passed_cases > other.candidate_passed_cases
            || self.candidate_replay_match_count > other.candidate_replay_match_count
            || self.candidate_heldout_match_count > other.candidate_heldout_match_count
    }

    fn equivalent_to(&self, other: &Self) -> bool {
        self.candidate_passed_cases == other.candidate_passed_cases
            && self.candidate_replay_match_count == other.candidate_replay_match_count
            && self.candidate_heldout_match_count == other.candidate_heldout_match_count
            && self.regression_case_ids == other.regression_case_ids
            && self.replay_regression_sample_ids == other.replay_regression_sample_ids
            && self.heldout_regression_receipt_ids == other.heldout_regression_receipt_ids
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentXtrainModuleOutcome {
    pub module: CompiledAgentModuleKind,
    pub decision: CompiledAgentPromotionDecision,
    pub reason: String,
    pub delta: CompiledAgentCandidateDelta,
    pub validation: CompiledAgentModuleValidation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentXtrainCycleReceipt {
    pub schema_version: String,
    pub cycle_id: String,
    pub row_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub source_ledger_digest: String,
    pub replay_bundle_digest: String,
    pub route_outcome: CompiledAgentXtrainModuleOutcome,
    pub grounded_answer_outcome: CompiledAgentXtrainModuleOutcome,
    pub summary: String,
    pub receipt_digest: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentCandidateFamilyDecision {
    KeepIncumbent,
    PromoteStronger,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentCandidateFamilyComparison {
    pub module: CompiledAgentModuleKind,
    pub incumbent_artifact_id: String,
    pub incumbent_model_family: String,
    pub stronger_artifact_id: String,
    pub stronger_model_family: String,
    pub incumbent_eval_passed_cases: u32,
    pub stronger_eval_passed_cases: u32,
    pub incumbent_replay_match_count: u32,
    pub stronger_replay_match_count: u32,
    pub incumbent_heldout_match_count: u32,
    pub stronger_heldout_match_count: u32,
    pub decision: CompiledAgentCandidateFamilyDecision,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentStrongerCandidateFamilyReport {
    pub schema_version: String,
    pub report_id: String,
    pub row_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub comparisons: Vec<CompiledAgentCandidateFamilyComparison>,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn compiled_agent_route_model_artifact_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_ROUTE_MODEL_ARTIFACT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_grounded_model_artifact_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_GROUNDED_MODEL_ARTIFACT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_route_stronger_model_artifact_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_ROUTE_STRONGER_MODEL_ARTIFACT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_grounded_stronger_model_artifact_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_GROUNDED_STRONGER_MODEL_ARTIFACT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_route_candidate_report_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_ROUTE_CANDIDATE_REPORT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_grounded_candidate_report_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_GROUNDED_CANDIDATE_REPORT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_route_stronger_candidate_report_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_ROUTE_STRONGER_CANDIDATE_REPORT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_grounded_stronger_candidate_report_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_GROUNDED_STRONGER_CANDIDATE_REPORT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_stronger_candidate_family_report_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_STRONGER_CANDIDATE_FAMILY_REPORT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_xtrain_cycle_receipt_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_XTRAIN_CYCLE_RECEIPT_FIXTURE_PATH)
}

pub fn canonical_compiled_agent_route_model_artifact(
) -> Result<CompiledAgentRouteModelArtifact, CompiledAgentXtrainError> {
    let learning_ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let replay_bundle = canonical_compiled_agent_replay_bundle()?;
    let route_samples = replay_bundle
        .samples
        .iter()
        .filter(|sample| sample.module == CompiledAgentModuleKind::Route)
        .map(|sample| {
            Ok(CompiledAgentRouteTrainingSample {
                sample_id: sample.sample_id.clone(),
                user_request: sample
                    .input
                    .get("user_request")
                    .and_then(Value::as_str)
                    .ok_or_else(|| CompiledAgentXtrainError::MissingRoutePrompt {
                        sample_id: sample.sample_id.clone(),
                    })?
                    .to_string(),
                expected_route: parse_route(
                    sample.expected_output.get("route"),
                    sample.sample_id.as_str(),
                )?,
                tags: sample.tags.clone(),
            })
        })
        .collect::<Result<Vec<_>, CompiledAgentXtrainError>>()?;
    let heldout_route_samples = learning_ledger
        .receipts
        .iter()
        .filter(|receipt| receipt.corpus_split == CompiledAgentCorpusSplit::HeldOut)
        .map(|receipt| CompiledAgentRouteTrainingSample {
            sample_id: format!("heldout.route.{}", receipt.receipt_id),
            user_request: receipt.user_request.clone(),
            expected_route: receipt.expected_route,
            tags: receipt.tags.clone(),
        })
        .collect::<Vec<_>>();
    Ok(train_compiled_agent_route_model(
        "compiled_agent.route.multinomial_nb_v1",
        "compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1",
        replay_bundle.bundle_digest,
        &route_samples,
        &heldout_route_samples,
    ))
}

pub fn canonical_compiled_agent_grounded_model_artifact(
) -> Result<CompiledAgentGroundedAnswerModelArtifact, CompiledAgentXtrainError> {
    let learning_ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let replay_bundle = canonical_compiled_agent_replay_bundle()?;
    let grounded_samples = replay_bundle
        .samples
        .iter()
        .filter(|sample| sample.module == CompiledAgentModuleKind::GroundedAnswer)
        .map(|sample| {
            Ok(CompiledAgentGroundedAnswerTrainingSample {
                sample_id: sample.sample_id.clone(),
                route: parse_route(sample.input.get("route"), &sample.sample_id)?,
                tool_results: parse_tool_results(
                    sample.input.get("tool_results"),
                    &sample.sample_id,
                )?,
                expected_kind: parse_public_kind(
                    sample.expected_output.get("kind"),
                    &sample.sample_id,
                )?,
                expected_response: sample
                    .expected_output
                    .get("response")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                tags: sample.tags.clone(),
            })
        })
        .collect::<Result<Vec<_>, CompiledAgentXtrainError>>()?;
    let heldout_grounded_samples = learning_ledger
        .receipts
        .iter()
        .filter(|receipt| receipt.corpus_split == CompiledAgentCorpusSplit::HeldOut)
        .map(|receipt| CompiledAgentGroundedAnswerTrainingSample {
            sample_id: format!("heldout.grounded_answer.{}", receipt.receipt_id),
            route: receipt.expected_route,
            tool_results: receipt.observed_tool_results.clone(),
            expected_kind: receipt.expected_public_response.kind,
            expected_response: receipt.expected_public_response.response.clone(),
            tags: receipt.tags.clone(),
        })
        .collect::<Vec<_>>();
    Ok(train_compiled_agent_grounded_answer_model(
        "compiled_agent.grounded_answer.multinomial_nb_v1",
        "compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1",
        replay_bundle.bundle_digest,
        &grounded_samples,
        &heldout_grounded_samples,
    ))
}

pub fn canonical_compiled_agent_route_stronger_model_artifact(
) -> Result<CompiledAgentRouteModelArtifact, CompiledAgentXtrainError> {
    let learning_ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let replay_bundle = canonical_compiled_agent_replay_bundle()?;
    let route_samples = replay_bundle
        .samples
        .iter()
        .filter(|sample| sample.module == CompiledAgentModuleKind::Route)
        .map(|sample| {
            Ok(CompiledAgentRouteTrainingSample {
                sample_id: sample.sample_id.clone(),
                user_request: sample
                    .input
                    .get("user_request")
                    .and_then(Value::as_str)
                    .ok_or_else(|| CompiledAgentXtrainError::MissingRoutePrompt {
                        sample_id: sample.sample_id.clone(),
                    })?
                    .to_string(),
                expected_route: parse_route(
                    sample.expected_output.get("route"),
                    sample.sample_id.as_str(),
                )?,
                tags: sample.tags.clone(),
            })
        })
        .collect::<Result<Vec<_>, CompiledAgentXtrainError>>()?;
    let heldout_route_samples = learning_ledger
        .receipts
        .iter()
        .filter(|receipt| receipt.corpus_split == CompiledAgentCorpusSplit::HeldOut)
        .map(|receipt| CompiledAgentRouteTrainingSample {
            sample_id: format!("heldout.route.{}", receipt.receipt_id),
            user_request: receipt.user_request.clone(),
            expected_route: receipt.expected_route,
            tags: receipt.tags.clone(),
        })
        .collect::<Vec<_>>();
    Ok(train_compiled_agent_route_tfidf_centroid_model(
        "compiled_agent.route.tfidf_centroid_v1",
        "compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1",
        replay_bundle.bundle_digest,
        &route_samples,
        &heldout_route_samples,
    ))
}

pub fn canonical_compiled_agent_grounded_stronger_model_artifact(
) -> Result<CompiledAgentGroundedAnswerModelArtifact, CompiledAgentXtrainError> {
    let learning_ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let replay_bundle = canonical_compiled_agent_replay_bundle()?;
    let grounded_samples = replay_bundle
        .samples
        .iter()
        .filter(|sample| sample.module == CompiledAgentModuleKind::GroundedAnswer)
        .map(|sample| {
            Ok(CompiledAgentGroundedAnswerTrainingSample {
                sample_id: sample.sample_id.clone(),
                route: parse_route(sample.input.get("route"), &sample.sample_id)?,
                tool_results: parse_tool_results(
                    sample.input.get("tool_results"),
                    &sample.sample_id,
                )?,
                expected_kind: parse_public_kind(
                    sample.expected_output.get("kind"),
                    &sample.sample_id,
                )?,
                expected_response: sample
                    .expected_output
                    .get("response")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                tags: sample.tags.clone(),
            })
        })
        .collect::<Result<Vec<_>, CompiledAgentXtrainError>>()?;
    let heldout_grounded_samples = learning_ledger
        .receipts
        .iter()
        .filter(|receipt| receipt.corpus_split == CompiledAgentCorpusSplit::HeldOut)
        .map(|receipt| CompiledAgentGroundedAnswerTrainingSample {
            sample_id: format!("heldout.grounded_answer.{}", receipt.receipt_id),
            route: receipt.expected_route,
            tool_results: receipt.observed_tool_results.clone(),
            expected_kind: receipt.expected_public_response.kind,
            expected_response: receipt.expected_public_response.response.clone(),
            tags: receipt.tags.clone(),
        })
        .collect::<Vec<_>>();
    Ok(train_compiled_agent_grounded_answer_tfidf_centroid_model(
        "compiled_agent.grounded_answer.tfidf_centroid_v1",
        "compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1",
        replay_bundle.bundle_digest,
        &grounded_samples,
        &heldout_grounded_samples,
    ))
}

pub fn compiled_agent_route_candidate_revision(
) -> Result<CompiledAgentModuleRevisionSet, CompiledAgentXtrainError> {
    let mut candidate = compiled_agent_baseline_revision_set();
    let route_model_artifact = canonical_compiled_agent_route_model_artifact()?;
    candidate.revision_id = route_model_artifact.artifact_id.clone();
    candidate.route_model_artifact = Some(route_model_artifact);
    Ok(candidate)
}

pub fn compiled_agent_grounded_candidate_revision(
) -> Result<CompiledAgentModuleRevisionSet, CompiledAgentXtrainError> {
    let mut candidate = compiled_agent_baseline_revision_set();
    let grounded_model_artifact = canonical_compiled_agent_grounded_model_artifact()?;
    candidate.revision_id = grounded_model_artifact.artifact_id.clone();
    candidate.grounded_answer_model_artifact = Some(grounded_model_artifact);
    candidate.verify_require_recent_earnings = true;
    Ok(candidate)
}

pub fn compiled_agent_route_stronger_candidate_revision(
) -> Result<CompiledAgentModuleRevisionSet, CompiledAgentXtrainError> {
    let mut candidate = compiled_agent_baseline_revision_set();
    let route_model_artifact = canonical_compiled_agent_route_stronger_model_artifact()?;
    candidate.revision_id = route_model_artifact.artifact_id.clone();
    candidate.route_model_artifact = Some(route_model_artifact);
    Ok(candidate)
}

pub fn compiled_agent_grounded_stronger_candidate_revision(
) -> Result<CompiledAgentModuleRevisionSet, CompiledAgentXtrainError> {
    let mut candidate = compiled_agent_baseline_revision_set();
    let grounded_model_artifact = canonical_compiled_agent_grounded_stronger_model_artifact()?;
    candidate.revision_id = grounded_model_artifact.artifact_id.clone();
    candidate.grounded_answer_model_artifact = Some(grounded_model_artifact);
    candidate.verify_require_recent_earnings = true;
    Ok(candidate)
}

pub fn canonical_compiled_agent_route_candidate_report(
) -> Result<CompiledAgentModuleEvalReport, CompiledAgentXtrainError> {
    Ok(build_compiled_agent_module_eval_report(
        &compiled_agent_route_candidate_revision()?,
    ))
}

pub fn canonical_compiled_agent_grounded_candidate_report(
) -> Result<CompiledAgentModuleEvalReport, CompiledAgentXtrainError> {
    Ok(build_compiled_agent_module_eval_report(
        &compiled_agent_grounded_candidate_revision()?,
    ))
}

pub fn canonical_compiled_agent_route_stronger_candidate_report(
) -> Result<CompiledAgentModuleEvalReport, CompiledAgentXtrainError> {
    Ok(build_compiled_agent_module_eval_report(
        &compiled_agent_route_stronger_candidate_revision()?,
    ))
}

pub fn canonical_compiled_agent_grounded_stronger_candidate_report(
) -> Result<CompiledAgentModuleEvalReport, CompiledAgentXtrainError> {
    Ok(build_compiled_agent_module_eval_report(
        &compiled_agent_grounded_stronger_candidate_revision()?,
    ))
}

pub fn canonical_compiled_agent_stronger_candidate_family_report(
) -> Result<CompiledAgentStrongerCandidateFamilyReport, CompiledAgentXtrainError> {
    let baseline = compiled_agent_baseline_revision_set();
    let replay_bundle = canonical_compiled_agent_replay_bundle()?;
    let evidence_class = replay_bundle.evidence_class;
    let incumbent_route = compiled_agent_route_candidate_revision()?;
    let stronger_route = compiled_agent_route_stronger_candidate_revision()?;
    let incumbent_grounded = compiled_agent_grounded_candidate_revision()?;
    let stronger_grounded = compiled_agent_grounded_stronger_candidate_revision()?;
    let baseline_report = build_compiled_agent_module_eval_report(&baseline);
    let incumbent_route_report = build_compiled_agent_module_eval_report(&incumbent_route);
    let stronger_route_report = build_compiled_agent_module_eval_report(&stronger_route);
    let incumbent_grounded_report = build_compiled_agent_module_eval_report(&incumbent_grounded);
    let stronger_grounded_report = build_compiled_agent_module_eval_report(&stronger_grounded);
    ensure_matching_evidence_class(
        evidence_class,
        baseline_report.evidence_class,
        "compiled_agent_xtrain.stronger_family.baseline_report",
    )?;
    ensure_matching_evidence_class(
        evidence_class,
        incumbent_route_report.evidence_class,
        "compiled_agent_xtrain.stronger_family.incumbent_route_report",
    )?;
    ensure_matching_evidence_class(
        evidence_class,
        stronger_route_report.evidence_class,
        "compiled_agent_xtrain.stronger_family.stronger_route_report",
    )?;
    ensure_matching_evidence_class(
        evidence_class,
        incumbent_grounded_report.evidence_class,
        "compiled_agent_xtrain.stronger_family.incumbent_grounded_report",
    )?;
    ensure_matching_evidence_class(
        evidence_class,
        stronger_grounded_report.evidence_class,
        "compiled_agent_xtrain.stronger_family.stronger_grounded_report",
    )?;

    let incumbent_route_validation = validate_route_candidate(
        &replay_bundle,
        &baseline_report,
        &incumbent_route_report,
        &baseline,
        &incumbent_route,
    )?;
    let stronger_route_validation = validate_route_candidate(
        &replay_bundle,
        &baseline_report,
        &stronger_route_report,
        &baseline,
        &stronger_route,
    )?;
    let incumbent_grounded_validation = validate_grounded_candidate(
        &replay_bundle,
        &baseline_report,
        &incumbent_grounded_report,
        &baseline,
        &incumbent_grounded,
    )?;
    let stronger_grounded_validation = validate_grounded_candidate(
        &replay_bundle,
        &baseline_report,
        &stronger_grounded_report,
        &baseline,
        &stronger_grounded,
    )?;

    let route_comparison = compare_candidate_families(
        CompiledAgentModuleKind::Route,
        incumbent_route.revision_id.as_str(),
        incumbent_route
            .route_model_artifact
            .as_ref()
            .map_or("", |artifact| artifact.model_family.as_str()),
        stronger_route.revision_id.as_str(),
        stronger_route
            .route_model_artifact
            .as_ref()
            .map_or("", |artifact| artifact.model_family.as_str()),
        &incumbent_route_validation,
        &stronger_route_validation,
    );
    let grounded_comparison = compare_candidate_families(
        CompiledAgentModuleKind::GroundedAnswer,
        incumbent_grounded.revision_id.as_str(),
        incumbent_grounded
            .grounded_answer_model_artifact
            .as_ref()
            .map_or("", |artifact| artifact.model_family.as_str()),
        stronger_grounded.revision_id.as_str(),
        stronger_grounded
            .grounded_answer_model_artifact
            .as_ref()
            .map_or("", |artifact| artifact.model_family.as_str()),
        &incumbent_grounded_validation,
        &stronger_grounded_validation,
    );

    let mut report = CompiledAgentStrongerCandidateFamilyReport {
        schema_version: String::from("psionic.compiled_agent.stronger_candidate_family_report.v1"),
        report_id: String::from("compiled_agent.stronger_candidate_family_report.v1"),
        row_id: baseline_report.row_id.clone(),
        evidence_class,
        comparisons: vec![route_comparison, grounded_comparison],
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = String::from(
        "Compared the incumbent multinomial Naive Bayes candidates against TF-IDF centroid candidates on the same bounded compiled-agent route and grounded-answer contracts. The stronger family did not clear a strict improvement bar, so the incumbent families remain authoritative candidates.",
    );
    report.report_digest =
        stable_digest(b"compiled_agent_stronger_candidate_family_report|", &report);
    Ok(report)
}

pub fn canonical_compiled_agent_xtrain_cycle_receipt(
) -> Result<CompiledAgentXtrainCycleReceipt, CompiledAgentXtrainError> {
    let baseline = compiled_agent_baseline_revision_set();
    let replay_bundle = canonical_compiled_agent_replay_bundle()?;
    let baseline_report = build_compiled_agent_module_eval_report(&baseline);
    let route_candidate = compiled_agent_route_candidate_revision()?;
    let route_candidate_report = build_compiled_agent_module_eval_report(&route_candidate);
    let grounded_candidate = compiled_agent_grounded_candidate_revision()?;
    let grounded_candidate_report = build_compiled_agent_module_eval_report(&grounded_candidate);
    let evidence_class = replay_bundle.evidence_class;
    ensure_matching_evidence_class(
        evidence_class,
        baseline_report.evidence_class,
        "compiled_agent_xtrain.baseline_report",
    )?;
    ensure_matching_evidence_class(
        evidence_class,
        route_candidate_report.evidence_class,
        "compiled_agent_xtrain.route_candidate_report",
    )?;
    ensure_matching_evidence_class(
        evidence_class,
        grounded_candidate_report.evidence_class,
        "compiled_agent_xtrain.grounded_candidate_report",
    )?;

    let route_outcome = build_route_outcome(
        &replay_bundle,
        &baseline_report,
        &route_candidate_report,
        &baseline,
        &route_candidate,
    )?;
    let grounded_answer_outcome = build_grounded_answer_outcome(
        &replay_bundle,
        &baseline_report,
        &grounded_candidate_report,
        &baseline,
        &grounded_candidate,
    )?;

    let mut receipt = CompiledAgentXtrainCycleReceipt {
        schema_version: String::from(XTRAIN_CYCLE_SCHEMA_VERSION),
        cycle_id: String::from("compiled_agent.xtrain.cycle.v1"),
        row_id: baseline_report.row_id.clone(),
        evidence_class,
        source_ledger_digest: replay_bundle.source_ledger_digest.clone(),
        replay_bundle_digest: replay_bundle.bundle_digest.clone(),
        route_outcome,
        grounded_answer_outcome,
        summary: String::new(),
        receipt_digest: String::new(),
    };
    receipt.summary = format!(
        "Compiled-agent XTRAIN cycle retained route decision={:?} and grounded_answer decision={:?} on replay bundle {}.",
        receipt.route_outcome.decision,
        receipt.grounded_answer_outcome.decision,
        receipt.replay_bundle_digest
    );
    receipt.receipt_digest = stable_digest(b"compiled_agent_xtrain_cycle_receipt|", &receipt);
    Ok(receipt)
}

fn ensure_matching_evidence_class(
    expected: CompiledAgentEvidenceClass,
    actual: CompiledAgentEvidenceClass,
    context: &str,
) -> Result<(), CompiledAgentXtrainError> {
    if actual == expected {
        Ok(())
    } else {
        Err(CompiledAgentXtrainError::MixedEvidenceClass {
            context: context.to_string(),
            expected,
            actual,
        })
    }
}

pub fn write_compiled_agent_route_candidate_report(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentModuleEvalReport, CompiledAgentXtrainError> {
    write_report(
        output_path,
        &canonical_compiled_agent_route_candidate_report()?,
    )
}

pub fn write_compiled_agent_route_model_artifact(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentRouteModelArtifact, CompiledAgentXtrainError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentXtrainError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let artifact = canonical_compiled_agent_route_model_artifact()?;
    let json = serde_json::to_string_pretty(&artifact)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentXtrainError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(artifact)
}

pub fn write_compiled_agent_grounded_model_artifact(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentGroundedAnswerModelArtifact, CompiledAgentXtrainError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentXtrainError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let artifact = canonical_compiled_agent_grounded_model_artifact()?;
    let json = serde_json::to_string_pretty(&artifact)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentXtrainError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(artifact)
}

pub fn write_compiled_agent_route_stronger_model_artifact(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentRouteModelArtifact, CompiledAgentXtrainError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentXtrainError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let artifact = canonical_compiled_agent_route_stronger_model_artifact()?;
    let json = serde_json::to_string_pretty(&artifact)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentXtrainError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(artifact)
}

pub fn write_compiled_agent_grounded_stronger_model_artifact(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentGroundedAnswerModelArtifact, CompiledAgentXtrainError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentXtrainError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let artifact = canonical_compiled_agent_grounded_stronger_model_artifact()?;
    let json = serde_json::to_string_pretty(&artifact)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentXtrainError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(artifact)
}

pub fn write_compiled_agent_grounded_candidate_report(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentModuleEvalReport, CompiledAgentXtrainError> {
    write_report(
        output_path,
        &canonical_compiled_agent_grounded_candidate_report()?,
    )
}

pub fn write_compiled_agent_route_stronger_candidate_report(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentModuleEvalReport, CompiledAgentXtrainError> {
    write_report(
        output_path,
        &canonical_compiled_agent_route_stronger_candidate_report()?,
    )
}

pub fn write_compiled_agent_grounded_stronger_candidate_report(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentModuleEvalReport, CompiledAgentXtrainError> {
    write_report(
        output_path,
        &canonical_compiled_agent_grounded_stronger_candidate_report()?,
    )
}

pub fn write_compiled_agent_stronger_candidate_family_report(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentStrongerCandidateFamilyReport, CompiledAgentXtrainError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentXtrainError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = canonical_compiled_agent_stronger_candidate_family_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentXtrainError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

pub fn write_compiled_agent_xtrain_cycle_receipt(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentXtrainCycleReceipt, CompiledAgentXtrainError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentXtrainError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let receipt = canonical_compiled_agent_xtrain_cycle_receipt()?;
    let json = serde_json::to_string_pretty(&receipt)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentXtrainError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(receipt)
}

pub fn verify_compiled_agent_xtrain_fixtures() -> Result<(), CompiledAgentXtrainError> {
    let expected_route_model = canonical_compiled_agent_route_model_artifact()?;
    let committed_route_model: CompiledAgentRouteModelArtifact = serde_json::from_slice(
        &fs::read(compiled_agent_route_model_artifact_fixture_path()).map_err(|error| {
            CompiledAgentXtrainError::Read {
                path: compiled_agent_route_model_artifact_fixture_path()
                    .display()
                    .to_string(),
                error,
            }
        })?,
    )?;
    if committed_route_model != expected_route_model {
        return Err(CompiledAgentXtrainError::Read {
            path: compiled_agent_route_model_artifact_fixture_path()
                .display()
                .to_string(),
            error: std::io::Error::other("route model artifact drift"),
        });
    }

    let expected_grounded_model = canonical_compiled_agent_grounded_model_artifact()?;
    let committed_grounded_model: CompiledAgentGroundedAnswerModelArtifact =
        serde_json::from_slice(
            &fs::read(compiled_agent_grounded_model_artifact_fixture_path()).map_err(|error| {
                CompiledAgentXtrainError::Read {
                    path: compiled_agent_grounded_model_artifact_fixture_path()
                        .display()
                        .to_string(),
                    error,
                }
            })?,
        )?;
    if committed_grounded_model != expected_grounded_model {
        return Err(CompiledAgentXtrainError::Read {
            path: compiled_agent_grounded_model_artifact_fixture_path()
                .display()
                .to_string(),
            error: std::io::Error::other("grounded model artifact drift"),
        });
    }

    let expected_route_stronger_model = canonical_compiled_agent_route_stronger_model_artifact()?;
    let committed_route_stronger_model: CompiledAgentRouteModelArtifact = serde_json::from_slice(
        &fs::read(compiled_agent_route_stronger_model_artifact_fixture_path()).map_err(
            |error| CompiledAgentXtrainError::Read {
                path: compiled_agent_route_stronger_model_artifact_fixture_path()
                    .display()
                    .to_string(),
                error,
            },
        )?,
    )?;
    if committed_route_stronger_model != expected_route_stronger_model {
        return Err(CompiledAgentXtrainError::Read {
            path: compiled_agent_route_stronger_model_artifact_fixture_path()
                .display()
                .to_string(),
            error: std::io::Error::other("route stronger model artifact drift"),
        });
    }

    let expected_grounded_stronger_model =
        canonical_compiled_agent_grounded_stronger_model_artifact()?;
    let committed_grounded_stronger_model: CompiledAgentGroundedAnswerModelArtifact =
        serde_json::from_slice(
            &fs::read(compiled_agent_grounded_stronger_model_artifact_fixture_path()).map_err(
                |error| CompiledAgentXtrainError::Read {
                    path: compiled_agent_grounded_stronger_model_artifact_fixture_path()
                        .display()
                        .to_string(),
                    error,
                },
            )?,
        )?;
    if committed_grounded_stronger_model != expected_grounded_stronger_model {
        return Err(CompiledAgentXtrainError::Read {
            path: compiled_agent_grounded_stronger_model_artifact_fixture_path()
                .display()
                .to_string(),
            error: std::io::Error::other("grounded stronger model artifact drift"),
        });
    }

    let expected_route = canonical_compiled_agent_route_candidate_report()?;
    let committed_route: CompiledAgentModuleEvalReport = serde_json::from_slice(
        &fs::read(compiled_agent_route_candidate_report_fixture_path()).map_err(|error| {
            CompiledAgentXtrainError::Read {
                path: compiled_agent_route_candidate_report_fixture_path()
                    .display()
                    .to_string(),
                error,
            }
        })?,
    )?;
    if committed_route != expected_route {
        return Err(CompiledAgentXtrainError::Read {
            path: compiled_agent_route_candidate_report_fixture_path()
                .display()
                .to_string(),
            error: std::io::Error::other("route candidate eval report drift"),
        });
    }

    let expected_grounded = canonical_compiled_agent_grounded_candidate_report()?;
    let committed_grounded: CompiledAgentModuleEvalReport = serde_json::from_slice(
        &fs::read(compiled_agent_grounded_candidate_report_fixture_path()).map_err(|error| {
            CompiledAgentXtrainError::Read {
                path: compiled_agent_grounded_candidate_report_fixture_path()
                    .display()
                    .to_string(),
                error,
            }
        })?,
    )?;
    if committed_grounded != expected_grounded {
        return Err(CompiledAgentXtrainError::Read {
            path: compiled_agent_grounded_candidate_report_fixture_path()
                .display()
                .to_string(),
            error: std::io::Error::other("grounded candidate eval report drift"),
        });
    }

    let expected_route_stronger = canonical_compiled_agent_route_stronger_candidate_report()?;
    let committed_route_stronger: CompiledAgentModuleEvalReport = serde_json::from_slice(
        &fs::read(compiled_agent_route_stronger_candidate_report_fixture_path()).map_err(
            |error| CompiledAgentXtrainError::Read {
                path: compiled_agent_route_stronger_candidate_report_fixture_path()
                    .display()
                    .to_string(),
                error,
            },
        )?,
    )?;
    if committed_route_stronger != expected_route_stronger {
        return Err(CompiledAgentXtrainError::Read {
            path: compiled_agent_route_stronger_candidate_report_fixture_path()
                .display()
                .to_string(),
            error: std::io::Error::other("route stronger candidate eval report drift"),
        });
    }

    let expected_grounded_stronger = canonical_compiled_agent_grounded_stronger_candidate_report()?;
    let committed_grounded_stronger: CompiledAgentModuleEvalReport = serde_json::from_slice(
        &fs::read(compiled_agent_grounded_stronger_candidate_report_fixture_path()).map_err(
            |error| CompiledAgentXtrainError::Read {
                path: compiled_agent_grounded_stronger_candidate_report_fixture_path()
                    .display()
                    .to_string(),
                error,
            },
        )?,
    )?;
    if committed_grounded_stronger != expected_grounded_stronger {
        return Err(CompiledAgentXtrainError::Read {
            path: compiled_agent_grounded_stronger_candidate_report_fixture_path()
                .display()
                .to_string(),
            error: std::io::Error::other("grounded stronger candidate eval report drift"),
        });
    }

    let expected_stronger_family_report =
        canonical_compiled_agent_stronger_candidate_family_report()?;
    let committed_stronger_family_report: CompiledAgentStrongerCandidateFamilyReport =
        serde_json::from_slice(
            &fs::read(compiled_agent_stronger_candidate_family_report_fixture_path()).map_err(
                |error| CompiledAgentXtrainError::Read {
                    path: compiled_agent_stronger_candidate_family_report_fixture_path()
                        .display()
                        .to_string(),
                    error,
                },
            )?,
        )?;
    if committed_stronger_family_report != expected_stronger_family_report {
        return Err(CompiledAgentXtrainError::Read {
            path: compiled_agent_stronger_candidate_family_report_fixture_path()
                .display()
                .to_string(),
            error: std::io::Error::other("stronger candidate family report drift"),
        });
    }

    let expected_cycle = canonical_compiled_agent_xtrain_cycle_receipt()?;
    let committed_cycle: CompiledAgentXtrainCycleReceipt = serde_json::from_slice(
        &fs::read(compiled_agent_xtrain_cycle_receipt_fixture_path()).map_err(|error| {
            CompiledAgentXtrainError::Read {
                path: compiled_agent_xtrain_cycle_receipt_fixture_path()
                    .display()
                    .to_string(),
                error,
            }
        })?,
    )?;
    if committed_cycle != expected_cycle {
        return Err(CompiledAgentXtrainError::Read {
            path: compiled_agent_xtrain_cycle_receipt_fixture_path()
                .display()
                .to_string(),
            error: std::io::Error::other("xtrain cycle receipt drift"),
        });
    }
    Ok(())
}

fn build_route_outcome(
    replay_bundle: &CompiledAgentReplayBundle,
    baseline_report: &CompiledAgentModuleEvalReport,
    candidate_report: &CompiledAgentModuleEvalReport,
    baseline_revision: &CompiledAgentModuleRevisionSet,
    candidate_revision: &CompiledAgentModuleRevisionSet,
) -> Result<CompiledAgentXtrainModuleOutcome, CompiledAgentXtrainError> {
    let validation = validate_route_candidate(
        replay_bundle,
        baseline_report,
        candidate_report,
        baseline_revision,
        candidate_revision,
    )?;
    let decision = if validation.regression_case_ids.is_empty()
        && validation.replay_regression_sample_ids.is_empty()
        && validation.heldout_regression_receipt_ids.is_empty()
        && validation.candidate_passed_cases > validation.baseline_passed_cases
        && validation.candidate_replay_match_count > validation.baseline_replay_match_count
        && validation.candidate_heldout_match_count >= validation.baseline_heldout_match_count
    {
        CompiledAgentPromotionDecision::Promote
    } else {
        CompiledAgentPromotionDecision::Hold
    };
    let reason = match decision {
        CompiledAgentPromotionDecision::Promote => String::from(
            "Trained route candidate fixes the retained negated-route false positive with no module-eval, replay, or held-out regressions.",
        ),
        CompiledAgentPromotionDecision::Hold => String::from(
            "Candidate did not clear the route validator gate cleanly enough to promote.",
        ),
    };
    let route_model_artifact = candidate_revision.route_model_artifact.as_ref();
    Ok(CompiledAgentXtrainModuleOutcome {
        module: CompiledAgentModuleKind::Route,
        decision,
        reason,
        delta: CompiledAgentCandidateDelta {
            module: CompiledAgentModuleKind::Route,
            base_revision_id: baseline_revision.revision_id.clone(),
            candidate_revision_id: candidate_revision.revision_id.clone(),
            replay_bundle_digest: replay_bundle.bundle_digest.clone(),
            candidate_artifact_ref: route_model_artifact
                .map(|_| String::from(COMPILED_AGENT_ROUTE_MODEL_ARTIFACT_FIXTURE_PATH)),
            candidate_artifact_digest: route_model_artifact
                .map(|artifact| artifact.artifact_digest.clone()),
            targeted_failure_classes: vec![String::from("negated_route_false_positive")],
            delta_summary: String::from(
                "Trains a multinomial Naive Bayes route model from replay-backed route samples so the negated wallet correction becomes a learned unsupported classification instead of a hand-authored keyword patch.",
            ),
        },
        validation,
    })
}

fn build_grounded_answer_outcome(
    replay_bundle: &CompiledAgentReplayBundle,
    baseline_report: &CompiledAgentModuleEvalReport,
    candidate_report: &CompiledAgentModuleEvalReport,
    baseline_revision: &CompiledAgentModuleRevisionSet,
    candidate_revision: &CompiledAgentModuleRevisionSet,
) -> Result<CompiledAgentXtrainModuleOutcome, CompiledAgentXtrainError> {
    let validation = validate_grounded_candidate(
        replay_bundle,
        baseline_report,
        candidate_report,
        baseline_revision,
        candidate_revision,
    )?;
    let decision = if validation.regression_case_ids.is_empty()
        && validation.replay_regression_sample_ids.is_empty()
        && validation.heldout_regression_receipt_ids.is_empty()
        && validation.candidate_replay_match_count > validation.baseline_replay_match_count
        && validation.candidate_passed_cases >= validation.baseline_passed_cases
        && validation.candidate_heldout_match_count >= validation.baseline_heldout_match_count
    {
        CompiledAgentPromotionDecision::Promote
    } else {
        CompiledAgentPromotionDecision::Hold
    };
    let reason = match decision {
        CompiledAgentPromotionDecision::Promote => String::from(
            "Trained grounded-answer candidate improves replay and held-out grounding while correctly falling back on missing and conflicting fact rows.",
        ),
        CompiledAgentPromotionDecision::Hold => String::from(
            "Candidate did not clear the grounded-answer validator gate cleanly enough to promote.",
        ),
    };
    let grounded_model_artifact = candidate_revision.grounded_answer_model_artifact.as_ref();
    Ok(CompiledAgentXtrainModuleOutcome {
        module: CompiledAgentModuleKind::GroundedAnswer,
        decision,
        reason,
        delta: CompiledAgentCandidateDelta {
            module: CompiledAgentModuleKind::GroundedAnswer,
            base_revision_id: baseline_revision.revision_id.clone(),
            candidate_revision_id: candidate_revision.revision_id.clone(),
            replay_bundle_digest: replay_bundle.bundle_digest.clone(),
            candidate_artifact_ref: grounded_model_artifact
                .map(|_| String::from(COMPILED_AGENT_GROUNDED_MODEL_ARTIFACT_FIXTURE_PATH)),
            candidate_artifact_digest: grounded_model_artifact
                .map(|artifact| artifact.artifact_digest.clone()),
            targeted_failure_classes: vec![String::from("grounded_answer_mismatch")],
            delta_summary: String::from(
                "Trains a fact-only grounded-answer model from replay-backed grounded samples so supported answers and unsupported refusals are retained as a learned artifact, while missing or conflicting facts fall back cleanly instead of being guessed.",
            ),
        },
        validation,
    })
}

fn compare_candidate_families(
    module: CompiledAgentModuleKind,
    incumbent_artifact_id: &str,
    incumbent_model_family: &str,
    stronger_artifact_id: &str,
    stronger_model_family: &str,
    incumbent: &CompiledAgentModuleValidation,
    stronger: &CompiledAgentModuleValidation,
) -> CompiledAgentCandidateFamilyComparison {
    let stronger_is_clean = stronger.regression_case_ids.is_empty()
        && stronger.replay_regression_sample_ids.is_empty()
        && stronger.heldout_regression_receipt_ids.is_empty();
    let stronger_improves = stronger.stronger_than(incumbent);
    let (decision, reason) = if stronger_is_clean && stronger_improves {
        (
            CompiledAgentCandidateFamilyDecision::PromoteStronger,
            String::from(
                "The TF-IDF centroid family cleared the same bounded validator surfaces and improved at least one retained metric without any eval, replay, or held-out regressions.",
            ),
        )
    } else if stronger.equivalent_to(incumbent) {
        (
            CompiledAgentCandidateFamilyDecision::KeepIncumbent,
            String::from(
                "The stronger family only tied the incumbent on the retained bounded surfaces, so the loop kept the incumbent candidate instead of churning authority on architecture preference.",
            ),
        )
    } else {
        (
            CompiledAgentCandidateFamilyDecision::KeepIncumbent,
            String::from(
                "The stronger family did not beat the incumbent cleanly enough on the retained bounded surfaces to justify replacing it.",
            ),
        )
    };
    CompiledAgentCandidateFamilyComparison {
        module,
        incumbent_artifact_id: incumbent_artifact_id.to_string(),
        incumbent_model_family: incumbent_model_family.to_string(),
        stronger_artifact_id: stronger_artifact_id.to_string(),
        stronger_model_family: stronger_model_family.to_string(),
        incumbent_eval_passed_cases: incumbent.candidate_passed_cases,
        stronger_eval_passed_cases: stronger.candidate_passed_cases,
        incumbent_replay_match_count: incumbent.candidate_replay_match_count,
        stronger_replay_match_count: stronger.candidate_replay_match_count,
        incumbent_heldout_match_count: incumbent.candidate_heldout_match_count,
        stronger_heldout_match_count: stronger.candidate_heldout_match_count,
        decision,
        reason,
    }
}

fn validate_route_candidate(
    replay_bundle: &CompiledAgentReplayBundle,
    baseline_report: &CompiledAgentModuleEvalReport,
    candidate_report: &CompiledAgentModuleEvalReport,
    baseline_revision: &CompiledAgentModuleRevisionSet,
    candidate_revision: &CompiledAgentModuleRevisionSet,
) -> Result<CompiledAgentModuleValidation, CompiledAgentXtrainError> {
    let learning_ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let baseline_summary = module_summary(baseline_report, CompiledAgentModuleKind::Route)?;
    let candidate_summary = module_summary(candidate_report, CompiledAgentModuleKind::Route)?;
    let improvement_case_ids = improvement_case_ids(
        baseline_report,
        candidate_report,
        CompiledAgentModuleKind::Route,
    );
    let regression_case_ids = regression_case_ids(
        baseline_report,
        candidate_report,
        CompiledAgentModuleKind::Route,
    );
    let (baseline_replay_match_count, baseline_replay_matches) =
        route_replay_matches(replay_bundle, baseline_revision)?;
    let (candidate_replay_match_count, candidate_replay_matches) =
        route_replay_matches(replay_bundle, candidate_revision)?;
    let (baseline_heldout_match_count, baseline_heldout_matches) =
        route_heldout_matches(&learning_ledger, baseline_revision);
    let (candidate_heldout_match_count, candidate_heldout_matches) =
        route_heldout_matches(&learning_ledger, candidate_revision);
    Ok(CompiledAgentModuleValidation {
        module: CompiledAgentModuleKind::Route,
        baseline_passed_cases: baseline_summary.passed_cases,
        candidate_passed_cases: candidate_summary.passed_cases,
        improvement_case_ids,
        regression_case_ids,
        baseline_replay_match_count,
        candidate_replay_match_count,
        baseline_heldout_match_count,
        candidate_heldout_match_count,
        replay_improvement_sample_ids: candidate_replay_matches
            .iter()
            .filter(|sample_id| !baseline_replay_matches.contains(sample_id))
            .cloned()
            .collect(),
        replay_regression_sample_ids: baseline_replay_matches
            .iter()
            .filter(|sample_id| !candidate_replay_matches.contains(sample_id))
            .cloned()
            .collect(),
        heldout_improvement_receipt_ids: candidate_heldout_matches
            .iter()
            .filter(|receipt_id| !baseline_heldout_matches.contains(receipt_id))
            .cloned()
            .collect(),
        heldout_regression_receipt_ids: baseline_heldout_matches
            .iter()
            .filter(|receipt_id| !candidate_heldout_matches.contains(receipt_id))
            .cloned()
            .collect(),
    })
}

fn validate_grounded_candidate(
    replay_bundle: &CompiledAgentReplayBundle,
    baseline_report: &CompiledAgentModuleEvalReport,
    candidate_report: &CompiledAgentModuleEvalReport,
    baseline_revision: &CompiledAgentModuleRevisionSet,
    candidate_revision: &CompiledAgentModuleRevisionSet,
) -> Result<CompiledAgentModuleValidation, CompiledAgentXtrainError> {
    let learning_ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let baseline_summary =
        module_summary(baseline_report, CompiledAgentModuleKind::GroundedAnswer)?;
    let candidate_summary =
        module_summary(candidate_report, CompiledAgentModuleKind::GroundedAnswer)?;
    let improvement_case_ids = improvement_case_ids(
        baseline_report,
        candidate_report,
        CompiledAgentModuleKind::GroundedAnswer,
    );
    let regression_case_ids = regression_case_ids(
        baseline_report,
        candidate_report,
        CompiledAgentModuleKind::GroundedAnswer,
    );
    let (baseline_replay_match_count, baseline_replay_matches) =
        grounded_replay_matches(replay_bundle, baseline_revision)?;
    let (candidate_replay_match_count, candidate_replay_matches) =
        grounded_replay_matches(replay_bundle, candidate_revision)?;
    let (baseline_heldout_match_count, baseline_heldout_matches) =
        grounded_heldout_matches(&learning_ledger, baseline_revision);
    let (candidate_heldout_match_count, candidate_heldout_matches) =
        grounded_heldout_matches(&learning_ledger, candidate_revision);
    Ok(CompiledAgentModuleValidation {
        module: CompiledAgentModuleKind::GroundedAnswer,
        baseline_passed_cases: baseline_summary.passed_cases,
        candidate_passed_cases: candidate_summary.passed_cases,
        improvement_case_ids,
        regression_case_ids,
        baseline_replay_match_count,
        candidate_replay_match_count,
        baseline_heldout_match_count,
        candidate_heldout_match_count,
        replay_improvement_sample_ids: candidate_replay_matches
            .iter()
            .filter(|sample_id| !baseline_replay_matches.contains(sample_id))
            .cloned()
            .collect(),
        replay_regression_sample_ids: baseline_replay_matches
            .iter()
            .filter(|sample_id| !candidate_replay_matches.contains(sample_id))
            .cloned()
            .collect(),
        heldout_improvement_receipt_ids: candidate_heldout_matches
            .iter()
            .filter(|receipt_id| !baseline_heldout_matches.contains(receipt_id))
            .cloned()
            .collect(),
        heldout_regression_receipt_ids: baseline_heldout_matches
            .iter()
            .filter(|receipt_id| !candidate_heldout_matches.contains(receipt_id))
            .cloned()
            .collect(),
    })
}

fn route_replay_matches(
    replay_bundle: &CompiledAgentReplayBundle,
    revision: &CompiledAgentModuleRevisionSet,
) -> Result<(u32, Vec<String>), CompiledAgentXtrainError> {
    let mut matches = Vec::new();
    for sample in replay_bundle
        .samples
        .iter()
        .filter(|sample| sample.module == CompiledAgentModuleKind::Route)
    {
        let Some(prompt) = sample.input.get("user_request").and_then(Value::as_str) else {
            return Err(CompiledAgentXtrainError::MissingRoutePrompt {
                sample_id: sample.sample_id.clone(),
            });
        };
        let observed = evaluate_compiled_agent_route(prompt, revision);
        let expected = parse_route(sample.expected_output.get("route"), &sample.sample_id)?;
        if observed == expected {
            matches.push(sample.sample_id.clone());
        }
    }
    Ok((matches.len() as u32, matches))
}

fn grounded_replay_matches(
    replay_bundle: &CompiledAgentReplayBundle,
    revision: &CompiledAgentModuleRevisionSet,
) -> Result<(u32, Vec<String>), CompiledAgentXtrainError> {
    let mut matches = Vec::new();
    for sample in replay_bundle
        .samples
        .iter()
        .filter(|sample| sample.module == CompiledAgentModuleKind::GroundedAnswer)
    {
        let route = parse_route(sample.input.get("route"), &sample.sample_id)?;
        let tool_results = parse_tool_results(sample.input.get("tool_results"), &sample.sample_id)?;
        let observed = evaluate_compiled_agent_grounded_answer(route, &tool_results, revision);
        let expected_kind =
            parse_public_kind(sample.expected_output.get("kind"), &sample.sample_id)?;
        let expected_response = sample
            .expected_output
            .get("response")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let observed_kind = public_kind_for_route(route);
        if observed_kind == expected_kind && observed == expected_response {
            matches.push(sample.sample_id.clone());
        }
    }
    Ok((matches.len() as u32, matches))
}

fn route_heldout_matches(
    learning_ledger: &crate::CompiledAgentLearningReceiptLedger,
    revision: &CompiledAgentModuleRevisionSet,
) -> (u32, Vec<String>) {
    let matches = learning_ledger
        .receipts
        .iter()
        .filter(|receipt| receipt.corpus_split == CompiledAgentCorpusSplit::HeldOut)
        .filter(|receipt| {
            evaluate_compiled_agent_route(receipt.user_request.as_str(), revision)
                == receipt.expected_route
        })
        .map(|receipt| receipt.receipt_id.clone())
        .collect::<Vec<_>>();
    (matches.len() as u32, matches)
}

fn grounded_heldout_matches(
    learning_ledger: &crate::CompiledAgentLearningReceiptLedger,
    revision: &CompiledAgentModuleRevisionSet,
) -> (u32, Vec<String>) {
    let matches = learning_ledger
        .receipts
        .iter()
        .filter(|receipt| receipt.corpus_split == CompiledAgentCorpusSplit::HeldOut)
        .filter(|receipt| {
            let observed = evaluate_compiled_agent_grounded_answer(
                receipt.expected_route,
                expected_tool_results_for_receipt(receipt),
                revision,
            );
            observed == receipt.expected_public_response.response
                && receipt.expected_public_response.kind
                    == match receipt.expected_route {
                        CompiledAgentRoute::Unsupported => {
                            CompiledAgentPublicOutcomeKind::UnsupportedRefusal
                        }
                        CompiledAgentRoute::ProviderStatus | CompiledAgentRoute::WalletStatus => {
                            CompiledAgentPublicOutcomeKind::GroundedAnswer
                        }
                    }
        })
        .map(|receipt| receipt.receipt_id.clone())
        .collect::<Vec<_>>();
    (matches.len() as u32, matches)
}

fn expected_tool_results_for_receipt(
    receipt: &crate::CompiledAgentLearningReceipt,
) -> &[CompiledAgentToolResult] {
    receipt.observed_tool_results.as_slice()
}

fn write_report(
    output_path: impl AsRef<Path>,
    report: &CompiledAgentModuleEvalReport,
) -> Result<CompiledAgentModuleEvalReport, CompiledAgentXtrainError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentXtrainError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let json = serde_json::to_string_pretty(report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentXtrainError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report.clone())
}

fn module_summary<'a>(
    report: &'a CompiledAgentModuleEvalReport,
    module: CompiledAgentModuleKind,
) -> Result<&'a psionic_eval::CompiledAgentModuleEvalSummary, CompiledAgentXtrainError> {
    report
        .module_summaries
        .iter()
        .find(|summary| summary.module == module)
        .ok_or_else(|| CompiledAgentXtrainError::MissingModuleSummary {
            module: format!("{module:?}"),
        })
}

fn improvement_case_ids(
    baseline_report: &CompiledAgentModuleEvalReport,
    candidate_report: &CompiledAgentModuleEvalReport,
    module: CompiledAgentModuleKind,
) -> Vec<String> {
    candidate_report
        .case_reports
        .iter()
        .filter(|case| case.module == module && case.pass)
        .filter(|candidate_case| {
            baseline_report
                .case_reports
                .iter()
                .find(|baseline_case| baseline_case.case_id == candidate_case.case_id)
                .map(|baseline_case| !baseline_case.pass)
                .unwrap_or(false)
        })
        .map(|case| case.case_id.clone())
        .collect()
}

fn regression_case_ids(
    baseline_report: &CompiledAgentModuleEvalReport,
    candidate_report: &CompiledAgentModuleEvalReport,
    module: CompiledAgentModuleKind,
) -> Vec<String> {
    baseline_report
        .case_reports
        .iter()
        .filter(|case| case.module == module && case.pass)
        .filter(|baseline_case| {
            candidate_report
                .case_reports
                .iter()
                .find(|candidate_case| candidate_case.case_id == baseline_case.case_id)
                .map(|candidate_case| !candidate_case.pass)
                .unwrap_or(false)
        })
        .map(|case| case.case_id.clone())
        .collect()
}

fn parse_route(
    value: Option<&Value>,
    sample_id: &str,
) -> Result<CompiledAgentRoute, CompiledAgentXtrainError> {
    serde_json::from_value(value.cloned().unwrap_or(Value::Null)).map_err(|_| {
        CompiledAgentXtrainError::MissingGroundedRoute {
            sample_id: sample_id.to_string(),
        }
    })
}

fn parse_tool_results(
    value: Option<&Value>,
    sample_id: &str,
) -> Result<Vec<CompiledAgentToolResult>, CompiledAgentXtrainError> {
    serde_json::from_value(value.cloned().unwrap_or(Value::Array(Vec::new()))).map_err(|_| {
        CompiledAgentXtrainError::InvalidGroundedToolResults {
            sample_id: sample_id.to_string(),
        }
    })
}

fn parse_public_kind(
    value: Option<&Value>,
    sample_id: &str,
) -> Result<CompiledAgentPublicOutcomeKind, CompiledAgentXtrainError> {
    serde_json::from_value(value.cloned().unwrap_or(Value::Null)).map_err(|_| {
        CompiledAgentXtrainError::MissingGroundedRoute {
            sample_id: sample_id.to_string(),
        }
    })
}

fn public_kind_for_route(route: CompiledAgentRoute) -> CompiledAgentPublicOutcomeKind {
    match route {
        CompiledAgentRoute::Unsupported => CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
        CompiledAgentRoute::ProviderStatus | CompiledAgentRoute::WalletStatus => {
            CompiledAgentPublicOutcomeKind::GroundedAnswer
        }
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}
