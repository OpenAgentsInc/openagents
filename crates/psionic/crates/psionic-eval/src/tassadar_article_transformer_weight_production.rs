use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    TassadarArticleTransformerArchitectureVariant, TassadarArticleTransformerEmbeddingStrategy,
};
use psionic_runtime::TassadarArticleTransformerModelArtifactBinding;
use psionic_transformer::EncoderDecoderTransformerConfig;

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_canonical_transformer_stack_boundary_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleTransformerCheckpointEvidence,
    TassadarArticleTransformerGradientCheckEvidence,
    TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_article_transformer_weight_production_v1/article_transformer_weight_production_bundle.json";
pub const TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_weight_production_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-169";
const EXPECTED_MODEL_MODULE_REF: &str = "crates/psionic-models/src/tassadar_article_transformer.rs";
const EXPECTED_TRANSFORMER_MODULE_REF: &str = "crates/psionic-transformer/src/encoder_decoder.rs";
const EXPECTED_TRAIN_MODULE_REF: &str =
    "crates/psionic-train/src/tassadar_article_transformer_weight_production.rs";
const EXPECTED_RUNTIME_MODULE_REF: &str = "crates/psionic-runtime/src/tassadar.rs";
const MODELS_CARGO_TOML_REF: &str = "crates/psionic-models/Cargo.toml";
const TRAIN_CARGO_TOML_REF: &str = "crates/psionic-train/Cargo.toml";
const EXPECTED_CASE_IDS: &[&str] = &[
    "micro_wasm_kernel",
    "branch_heavy_kernel",
    "memory_heavy_kernel",
    "hungarian_matching",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerWeightProductionSplit {
    Train,
    HeldOut,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionCaseEvidence {
    pub case_id: String,
    pub split: TassadarArticleTransformerWeightProductionSplit,
    pub summary: String,
    pub profile_id: String,
    pub trace_step_count: usize,
    pub expected_output_count: usize,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub full_target_token_count: usize,
    pub source_token_digest: String,
    pub target_token_digest: String,
    pub sequence_digest: String,
    pub halt_reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionCaseMetric {
    pub case_id: String,
    pub split: TassadarArticleTransformerWeightProductionSplit,
    pub initial_mean_loss: f32,
    pub final_mean_loss: f32,
    pub initial_token_exactness_bps: u32,
    pub final_token_exactness_bps: u32,
    pub initial_exact_target_match: bool,
    pub final_exact_target_match: bool,
    pub improved: bool,
    pub initial_prediction_digest: String,
    pub final_prediction_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionStepEvidence {
    pub global_step: u64,
    pub batch_id: String,
    pub training_mean_loss: f32,
    pub training_token_exactness_bps: u32,
    pub training_exact_match_count: usize,
    pub held_out_mean_loss: f32,
    pub held_out_token_exactness_bps: u32,
    pub held_out_exact_match_count: usize,
    pub effective_learning_rates: BTreeMap<String, f32>,
    pub scheduler_kinds: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionEvidenceBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub tied_requirement_id: String,
    pub model_module_ref: String,
    pub transformer_module_ref: String,
    pub train_module_ref: String,
    pub runtime_module_ref: String,
    pub run_id: String,
    pub checkpoint_family: String,
    pub suite_id: String,
    pub suite_description: String,
    pub source_corpus_id: String,
    pub architecture_variant: TassadarArticleTransformerArchitectureVariant,
    pub config: EncoderDecoderTransformerConfig,
    pub embedding_strategy: TassadarArticleTransformerEmbeddingStrategy,
    pub trainable_parameter_ids: Vec<String>,
    pub trainable_parameter_scalar_count: usize,
    pub loss_kind: String,
    pub optimizer_kind: String,
    pub scheduler_kind: String,
    pub warmup_steps: u64,
    pub label_smoothing: f32,
    pub finite_difference_epsilon: f32,
    pub base_descriptor_ref: String,
    pub base_artifact_ref: String,
    pub produced_descriptor_ref: String,
    pub produced_artifact_ref: String,
    pub training_cases: Vec<TassadarArticleTransformerWeightProductionCaseEvidence>,
    pub held_out_cases: Vec<TassadarArticleTransformerWeightProductionCaseEvidence>,
    pub gradient_checks: Vec<TassadarArticleTransformerGradientCheckEvidence>,
    pub step_evidence: Vec<TassadarArticleTransformerWeightProductionStepEvidence>,
    pub case_metrics: Vec<TassadarArticleTransformerWeightProductionCaseMetric>,
    pub base_model_artifact_binding: TassadarArticleTransformerModelArtifactBinding,
    pub produced_model_artifact_binding: TassadarArticleTransformerModelArtifactBinding,
    pub initial_training_mean_loss: f32,
    pub final_training_mean_loss: f32,
    pub initial_training_token_exactness_bps: u32,
    pub final_training_token_exactness_bps: u32,
    pub initial_training_exact_match_count: usize,
    pub final_training_exact_match_count: usize,
    pub final_held_out_mean_loss: f32,
    pub final_held_out_token_exactness_bps: u32,
    pub final_held_out_exact_match_count: usize,
    pub produced_artifact_differs_from_base: bool,
    pub checkpoint: TassadarArticleTransformerCheckpointEvidence,
    pub artifact_reload_matches_trained_state: bool,
    pub artifact_reload_descriptor_digest: String,
    pub claim_boundary: String,
    pub bundle_digest: String,
}

impl TassadarArticleTransformerWeightProductionEvidenceBundle {
    #[must_use]
    pub fn with_bundle_digest(mut self) -> Self {
        self.bundle_digest.clear();
        self.bundle_digest = stable_digest(
            b"psionic_tassadar_article_transformer_weight_production_bundle|",
            &self,
        );
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerWeightProductionValidationKind {
    BoundedCanonicalSuite,
    ProducedArtifactWritten,
    ArtifactReloadAndCheckpointRestore,
    NontrivialImitationTargets,
    BoundaryRootedInTransformer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionCaseRow {
    pub case_id: String,
    pub validation_kind: TassadarArticleTransformerWeightProductionValidationKind,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionArtifactReview {
    pub base_descriptor_ref: String,
    pub base_artifact_ref: String,
    pub produced_descriptor_ref: String,
    pub produced_artifact_ref: String,
    pub all_committed_refs_exist: bool,
    pub produced_refs_under_fixtures_tassadar_models: bool,
    pub produced_artifact_differs_from_base: bool,
    pub checkpoint_restore_matches_trained_state: bool,
    pub artifact_reload_matches_trained_state: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionBoundaryReview {
    pub model_module_ref: String,
    pub transformer_module_ref: String,
    pub train_module_ref: String,
    pub runtime_module_ref: String,
    pub models_cargo_toml_ref: String,
    pub train_cargo_toml_ref: String,
    pub evidence_roots_in_expected_modules: bool,
    pub models_depend_on_psionic_transformer: bool,
    pub train_depends_on_psionic_transformer: bool,
    pub canonical_boundary_green: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub evidence_bundle_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightProductionReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarArticleTransformerWeightProductionAcceptanceGateTie,
    pub canonical_boundary_report_ref: String,
    pub canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub evidence_bundle_ref: String,
    pub evidence_bundle: TassadarArticleTransformerWeightProductionEvidenceBundle,
    pub artifact_review: TassadarArticleTransformerWeightProductionArtifactReview,
    pub boundary_review: TassadarArticleTransformerWeightProductionBoundaryReview,
    pub case_rows: Vec<TassadarArticleTransformerWeightProductionCaseRow>,
    pub all_required_cases_present: bool,
    pub all_cases_pass: bool,
    pub weight_production_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerWeightProductionError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    CanonicalBoundary(#[from] TassadarCanonicalTransformerStackBoundaryReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn tassadar_article_transformer_weight_production_evidence_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF)
}

pub fn write_tassadar_article_transformer_weight_production_evidence_bundle(
    output_path: impl AsRef<Path>,
    bundle: &TassadarArticleTransformerWeightProductionEvidenceBundle,
) -> Result<
    TassadarArticleTransformerWeightProductionEvidenceBundle,
    TassadarArticleTransformerWeightProductionError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerWeightProductionError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = bundle.clone().with_bundle_digest();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerWeightProductionError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn read_tassadar_article_transformer_weight_production_evidence_bundle(
    relative_path: &str,
) -> Result<
    TassadarArticleTransformerWeightProductionEvidenceBundle,
    TassadarArticleTransformerWeightProductionError,
> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarArticleTransformerWeightProductionError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerWeightProductionError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

pub fn build_tassadar_article_transformer_weight_production_report() -> Result<
    TassadarArticleTransformerWeightProductionReport,
    TassadarArticleTransformerWeightProductionError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let canonical_boundary_report = build_tassadar_canonical_transformer_stack_boundary_report()?;
    let evidence_bundle = read_tassadar_article_transformer_weight_production_evidence_bundle(
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF,
    )?;
    let artifact_review = artifact_review(&evidence_bundle);
    let boundary_review = boundary_review(&evidence_bundle, &canonical_boundary_report)?;
    let case_rows = case_rows(&evidence_bundle, &artifact_review, &boundary_review);
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        canonical_boundary_report,
        evidence_bundle,
        artifact_review,
        boundary_review,
        case_rows,
    ))
}

#[must_use]
pub fn tassadar_article_transformer_weight_production_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_REPORT_REF)
}

pub fn write_tassadar_article_transformer_weight_production_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerWeightProductionReport,
    TassadarArticleTransformerWeightProductionError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerWeightProductionError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_weight_production_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerWeightProductionError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    evidence_bundle: TassadarArticleTransformerWeightProductionEvidenceBundle,
    artifact_review: TassadarArticleTransformerWeightProductionArtifactReview,
    boundary_review: TassadarArticleTransformerWeightProductionBoundaryReview,
    case_rows: Vec<TassadarArticleTransformerWeightProductionCaseRow>,
) -> TassadarArticleTransformerWeightProductionReport {
    let acceptance_gate_tie = TassadarArticleTransformerWeightProductionAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        evidence_bundle_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate_report
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate_report.acceptance_status,
        blocked_issue_ids: acceptance_gate_report.blocked_issue_ids.clone(),
    };
    let all_required_cases_present = case_rows
        .iter()
        .map(|row| row.validation_kind)
        .collect::<BTreeSet<_>>()
        == required_validation_kinds();
    let all_cases_pass = case_rows.iter().all(|row| row.passed);
    let weight_production_contract_green = acceptance_gate_tie.tied_requirement_satisfied
        && canonical_boundary_report.boundary_contract_green
        && artifact_review.all_committed_refs_exist
        && artifact_review.produced_refs_under_fixtures_tassadar_models
        && artifact_review.produced_artifact_differs_from_base
        && artifact_review.checkpoint_restore_matches_trained_state
        && artifact_review.artifact_reload_matches_trained_state
        && all_required_cases_present
        && all_cases_pass
        && boundary_review.passed;
    let article_equivalence_green =
        weight_production_contract_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleTransformerWeightProductionReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_transformer_weight_production.report.v1"),
        acceptance_gate_tie,
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        canonical_boundary_report,
        evidence_bundle_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF,
        ),
        evidence_bundle,
        artifact_review,
        boundary_review,
        case_rows,
        all_required_cases_present,
        all_cases_pass,
        weight_production_contract_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report closes only the first real Transformer-backed article weight-production run on a bounded article-class slice. It proves that the owned stack now emits a committed trained trace-bound artifact with explicit imitation metrics, checkpoint restore parity, and artifact reload parity. It does not claim full article-class exactness, fast-route closure, benchmark parity, contamination independence, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article Transformer weight production now records case_rows={}, all_required_cases_present={}, all_cases_pass={}, tied_requirement_satisfied={}, weight_production_contract_green={}, and article_equivalence_green={}.",
        report.case_rows.len(),
        report.all_required_cases_present,
        report.all_cases_pass,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.weight_production_contract_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_transformer_weight_production_report|",
        &report,
    );
    report
}

fn required_validation_kinds() -> BTreeSet<TassadarArticleTransformerWeightProductionValidationKind>
{
    BTreeSet::from([
        TassadarArticleTransformerWeightProductionValidationKind::BoundedCanonicalSuite,
        TassadarArticleTransformerWeightProductionValidationKind::ProducedArtifactWritten,
        TassadarArticleTransformerWeightProductionValidationKind::ArtifactReloadAndCheckpointRestore,
        TassadarArticleTransformerWeightProductionValidationKind::NontrivialImitationTargets,
        TassadarArticleTransformerWeightProductionValidationKind::BoundaryRootedInTransformer,
    ])
}

fn case_rows(
    evidence_bundle: &TassadarArticleTransformerWeightProductionEvidenceBundle,
    artifact_review: &TassadarArticleTransformerWeightProductionArtifactReview,
    boundary_review: &TassadarArticleTransformerWeightProductionBoundaryReview,
) -> Vec<TassadarArticleTransformerWeightProductionCaseRow> {
    vec![
        bounded_canonical_suite_case(evidence_bundle),
        produced_artifact_written_case(artifact_review),
        artifact_reload_and_checkpoint_restore_case(artifact_review),
        nontrivial_imitation_targets_case(evidence_bundle),
        boundary_rooted_in_transformer_case(boundary_review),
    ]
}

fn bounded_canonical_suite_case(
    evidence_bundle: &TassadarArticleTransformerWeightProductionEvidenceBundle,
) -> TassadarArticleTransformerWeightProductionCaseRow {
    let case_ids = evidence_bundle
        .training_cases
        .iter()
        .chain(evidence_bundle.held_out_cases.iter())
        .map(|case| case.case_id.clone())
        .collect::<BTreeSet<_>>();
    let expected_case_ids = EXPECTED_CASE_IDS
        .iter()
        .map(|value| String::from(*value))
        .collect::<BTreeSet<_>>();
    let all_cases_nonempty = evidence_bundle
        .training_cases
        .iter()
        .chain(evidence_bundle.held_out_cases.iter())
        .all(|case| {
            case.prompt_token_count > 0
                && case.target_token_count > 0
                && case.trace_step_count > 0
                && !case.sequence_digest.trim().is_empty()
        });
    let passed = evidence_bundle.source_corpus_id == "tassadar.article_class_corpus.v1"
        && !evidence_bundle.training_cases.is_empty()
        && !evidence_bundle.held_out_cases.is_empty()
        && case_ids == expected_case_ids
        && all_cases_nonempty;
    case_row(
        "bounded_canonical_suite",
        TassadarArticleTransformerWeightProductionValidationKind::BoundedCanonicalSuite,
        passed,
        format!(
            "source_corpus_id={} training_cases={} held_out_cases={} case_ids_match_expected={} all_cases_nonempty={}",
            evidence_bundle.source_corpus_id,
            evidence_bundle.training_cases.len(),
            evidence_bundle.held_out_cases.len(),
            case_ids == expected_case_ids,
            all_cases_nonempty,
        ),
    )
}

fn produced_artifact_written_case(
    artifact_review: &TassadarArticleTransformerWeightProductionArtifactReview,
) -> TassadarArticleTransformerWeightProductionCaseRow {
    let passed = artifact_review.all_committed_refs_exist
        && artifact_review.produced_refs_under_fixtures_tassadar_models
        && artifact_review.produced_artifact_differs_from_base;
    case_row(
        "produced_artifact_written",
        TassadarArticleTransformerWeightProductionValidationKind::ProducedArtifactWritten,
        passed,
        artifact_review.detail.clone(),
    )
}

fn artifact_reload_and_checkpoint_restore_case(
    artifact_review: &TassadarArticleTransformerWeightProductionArtifactReview,
) -> TassadarArticleTransformerWeightProductionCaseRow {
    let passed = artifact_review.checkpoint_restore_matches_trained_state
        && artifact_review.artifact_reload_matches_trained_state;
    case_row(
        "artifact_reload_and_checkpoint_restore",
        TassadarArticleTransformerWeightProductionValidationKind::ArtifactReloadAndCheckpointRestore,
        passed,
        format!(
            "checkpoint_restore_matches_trained_state={} artifact_reload_matches_trained_state={}",
            artifact_review.checkpoint_restore_matches_trained_state,
            artifact_review.artifact_reload_matches_trained_state,
        ),
    )
}

fn nontrivial_imitation_targets_case(
    evidence_bundle: &TassadarArticleTransformerWeightProductionEvidenceBundle,
) -> TassadarArticleTransformerWeightProductionCaseRow {
    let training_improved = evidence_bundle.final_training_mean_loss
        < evidence_bundle.initial_training_mean_loss
        || evidence_bundle.final_training_token_exactness_bps
            > evidence_bundle.initial_training_token_exactness_bps;
    let all_case_metrics_present = evidence_bundle.case_metrics.len()
        == evidence_bundle.training_cases.len() + evidence_bundle.held_out_cases.len();
    let any_case_improved = evidence_bundle
        .case_metrics
        .iter()
        .any(|metric| metric.improved);
    let metrics_are_nontrivial = evidence_bundle.case_metrics.iter().all(|metric| {
        !metric.initial_prediction_digest.trim().is_empty()
            && !metric.final_prediction_digest.trim().is_empty()
    });
    let passed = training_improved
        && all_case_metrics_present
        && any_case_improved
        && metrics_are_nontrivial
        && !evidence_bundle.step_evidence.is_empty();
    case_row(
        "nontrivial_imitation_targets",
        TassadarArticleTransformerWeightProductionValidationKind::NontrivialImitationTargets,
        passed,
        format!(
            "initial_training_mean_loss={} final_training_mean_loss={} initial_training_token_exactness_bps={} final_training_token_exactness_bps={} final_held_out_token_exactness_bps={} case_metrics_present={} any_case_improved={}",
            evidence_bundle.initial_training_mean_loss,
            evidence_bundle.final_training_mean_loss,
            evidence_bundle.initial_training_token_exactness_bps,
            evidence_bundle.final_training_token_exactness_bps,
            evidence_bundle.final_held_out_token_exactness_bps,
            all_case_metrics_present,
            any_case_improved,
        ),
    )
}

fn boundary_rooted_in_transformer_case(
    boundary_review: &TassadarArticleTransformerWeightProductionBoundaryReview,
) -> TassadarArticleTransformerWeightProductionCaseRow {
    case_row(
        "boundary_rooted_in_transformer",
        TassadarArticleTransformerWeightProductionValidationKind::BoundaryRootedInTransformer,
        boundary_review.passed,
        boundary_review.detail.clone(),
    )
}

fn artifact_review(
    evidence_bundle: &TassadarArticleTransformerWeightProductionEvidenceBundle,
) -> TassadarArticleTransformerWeightProductionArtifactReview {
    let refs = [
        evidence_bundle.base_descriptor_ref.as_str(),
        evidence_bundle.base_artifact_ref.as_str(),
        evidence_bundle.produced_descriptor_ref.as_str(),
        evidence_bundle.produced_artifact_ref.as_str(),
    ];
    let all_committed_refs_exist = refs
        .iter()
        .all(|relative_path| repo_root().join(relative_path).exists());
    let produced_refs_under_fixtures_tassadar_models = evidence_bundle
        .produced_descriptor_ref
        .starts_with("fixtures/tassadar/models/")
        && evidence_bundle
            .produced_artifact_ref
            .starts_with("fixtures/tassadar/models/");
    TassadarArticleTransformerWeightProductionArtifactReview {
        base_descriptor_ref: evidence_bundle.base_descriptor_ref.clone(),
        base_artifact_ref: evidence_bundle.base_artifact_ref.clone(),
        produced_descriptor_ref: evidence_bundle.produced_descriptor_ref.clone(),
        produced_artifact_ref: evidence_bundle.produced_artifact_ref.clone(),
        all_committed_refs_exist,
        produced_refs_under_fixtures_tassadar_models,
        produced_artifact_differs_from_base: evidence_bundle.produced_artifact_differs_from_base,
        checkpoint_restore_matches_trained_state: evidence_bundle
            .checkpoint
            .restore_matches_trained_state,
        artifact_reload_matches_trained_state: evidence_bundle.artifact_reload_matches_trained_state,
        detail: format!(
            "base_descriptor_ref={} produced_descriptor_ref={} all_committed_refs_exist={} produced_refs_under_fixtures_tassadar_models={} produced_artifact_differs_from_base={} checkpoint_restore_matches_trained_state={} artifact_reload_matches_trained_state={}",
            evidence_bundle.base_descriptor_ref,
            evidence_bundle.produced_descriptor_ref,
            all_committed_refs_exist,
            produced_refs_under_fixtures_tassadar_models,
            evidence_bundle.produced_artifact_differs_from_base,
            evidence_bundle.checkpoint.restore_matches_trained_state,
            evidence_bundle.artifact_reload_matches_trained_state,
        ),
    }
}

fn boundary_review(
    evidence_bundle: &TassadarArticleTransformerWeightProductionEvidenceBundle,
    canonical_boundary_report: &TassadarCanonicalTransformerStackBoundaryReport,
) -> Result<
    TassadarArticleTransformerWeightProductionBoundaryReview,
    TassadarArticleTransformerWeightProductionError,
> {
    let models_cargo =
        fs::read_to_string(repo_root().join(MODELS_CARGO_TOML_REF)).map_err(|error| {
            TassadarArticleTransformerWeightProductionError::Read {
                path: String::from(MODELS_CARGO_TOML_REF),
                error,
            }
        })?;
    let train_cargo =
        fs::read_to_string(repo_root().join(TRAIN_CARGO_TOML_REF)).map_err(|error| {
            TassadarArticleTransformerWeightProductionError::Read {
                path: String::from(TRAIN_CARGO_TOML_REF),
                error,
            }
        })?;
    let evidence_roots_in_expected_modules = evidence_bundle.model_module_ref
        == EXPECTED_MODEL_MODULE_REF
        && evidence_bundle.transformer_module_ref == EXPECTED_TRANSFORMER_MODULE_REF
        && evidence_bundle.train_module_ref == EXPECTED_TRAIN_MODULE_REF
        && evidence_bundle.runtime_module_ref == EXPECTED_RUNTIME_MODULE_REF;
    let models_depend_on_psionic_transformer = models_cargo.contains("psionic-transformer");
    let train_depends_on_psionic_transformer = train_cargo.contains("psionic-transformer");
    let canonical_boundary_green = canonical_boundary_report.boundary_contract_green;
    let passed = evidence_roots_in_expected_modules
        && models_depend_on_psionic_transformer
        && train_depends_on_psionic_transformer
        && canonical_boundary_green;
    Ok(TassadarArticleTransformerWeightProductionBoundaryReview {
        model_module_ref: evidence_bundle.model_module_ref.clone(),
        transformer_module_ref: evidence_bundle.transformer_module_ref.clone(),
        train_module_ref: evidence_bundle.train_module_ref.clone(),
        runtime_module_ref: evidence_bundle.runtime_module_ref.clone(),
        models_cargo_toml_ref: String::from(MODELS_CARGO_TOML_REF),
        train_cargo_toml_ref: String::from(TRAIN_CARGO_TOML_REF),
        evidence_roots_in_expected_modules,
        models_depend_on_psionic_transformer,
        train_depends_on_psionic_transformer,
        canonical_boundary_green,
        passed,
        detail: format!(
            "expected_refs_match={} models_depend_on_psionic_transformer={} train_depends_on_psionic_transformer={} canonical_boundary_green={}",
            evidence_roots_in_expected_modules,
            models_depend_on_psionic_transformer,
            train_depends_on_psionic_transformer,
            canonical_boundary_green,
        ),
    })
}

fn case_row(
    case_id: &str,
    validation_kind: TassadarArticleTransformerWeightProductionValidationKind,
    passed: bool,
    detail: String,
) -> TassadarArticleTransformerWeightProductionCaseRow {
    TassadarArticleTransformerWeightProductionCaseRow {
        case_id: String::from(case_id),
        validation_kind,
        passed,
        detail,
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleTransformerWeightProductionError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarArticleTransformerWeightProductionError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerWeightProductionError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_transformer_weight_production_report, read_repo_json,
        tassadar_article_transformer_weight_production_report_path,
        write_tassadar_article_transformer_weight_production_report,
        TassadarArticleEquivalenceAcceptanceStatus,
        TassadarArticleTransformerWeightProductionReport,
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_REPORT_REF,
    };

    #[test]
    fn article_transformer_weight_production_report_tracks_first_trained_artifact_without_final_green(
    ) {
        let report = build_tassadar_article_transformer_weight_production_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(
            report.acceptance_gate_tie.acceptance_status,
            TassadarArticleEquivalenceAcceptanceStatus::Green
        );
        assert!(report.artifact_review.all_committed_refs_exist);
        assert!(report.artifact_review.produced_artifact_differs_from_base);
        assert!(
            report
                .artifact_review
                .checkpoint_restore_matches_trained_state
        );
        assert!(report.artifact_review.artifact_reload_matches_trained_state);
        assert!(report.weight_production_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.case_rows.len(), 5);
        assert_eq!(report.evidence_bundle.training_cases.len(), 1);
        assert_eq!(report.evidence_bundle.held_out_cases.len(), 3);
    }

    #[test]
    fn article_transformer_weight_production_report_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_weight_production_report().expect("report");
        let committed: TassadarArticleTransformerWeightProductionReport = read_repo_json(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_REPORT_REF,
            "article_transformer_weight_production_report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_transformer_weight_production_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_weight_production_report.json");
        let written = write_tassadar_article_transformer_weight_production_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleTransformerWeightProductionReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_weight_production_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_weight_production_report.json")
        );
    }
}
