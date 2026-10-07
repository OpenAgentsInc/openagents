use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    TassadarArticleTransformer, TassadarArticleTransformerArchitectureVariant,
    TassadarArticleTransformerEmbeddingStrategy, TassadarArticleTransformerError,
};
use psionic_runtime::TassadarArticleTransformerModelArtifactBinding;
use psionic_transformer::EncoderDecoderTransformerConfig;

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_canonical_transformer_stack_boundary_report,
    read_tassadar_article_transformer_weight_production_evidence_bundle,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleTransformerCheckpointEvidence,
    TassadarArticleTransformerWeightProductionCaseEvidence,
    TassadarArticleTransformerWeightProductionError,
    TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF: &str =
    TassadarArticleTransformer::TRAINED_TRACE_BOUND_LINEAGE_CONTRACT_REF;
pub const TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_weight_lineage_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-169A";
const EXPECTED_SOURCE_CORPUS_ID: &str = "tassadar.article_class_corpus.v1";
const EXPECTED_SUITE_ID: &str = "tassadar.article_transformer.weight_production_suite.v1";
const EXPECTED_TRAIN_CASE_IDS: &[&str] = &["hungarian_matching"];
const EXPECTED_HELD_OUT_CASE_IDS: &[&str] = &[
    "micro_wasm_kernel",
    "branch_heavy_kernel",
    "memory_heavy_kernel",
];
const EXPECTED_TARGET_WINDOW_TOKENS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerWeightLineageSourceRole {
    ModelModule,
    TransformerModule,
    TrainModule,
    RuntimeModule,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightLineageSourceRecord {
    pub source_role: TassadarArticleTransformerWeightLineageSourceRole,
    pub source_ref: String,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerWeightLineageArtifactRole {
    EvidenceBundle,
    BaseDescriptor,
    BaseWeights,
    ProducedDescriptor,
    ProducedWeights,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightLineageArtifactRecord {
    pub artifact_role: TassadarArticleTransformerWeightLineageArtifactRole,
    pub artifact_ref: String,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightLineageTrainingConfigSnapshot {
    pub run_id: String,
    pub checkpoint_family: String,
    pub source_corpus_id: String,
    pub suite_id: String,
    pub suite_description: String,
    pub architecture_variant: TassadarArticleTransformerArchitectureVariant,
    pub transformer_config: EncoderDecoderTransformerConfig,
    pub embedding_strategy: TassadarArticleTransformerEmbeddingStrategy,
    pub trainable_parameter_ids: Vec<String>,
    pub trainable_parameter_scalar_count: usize,
    pub loss_kind: String,
    pub optimizer_kind: String,
    pub scheduler_kind: String,
    pub warmup_steps: u64,
    pub label_smoothing: f32,
    pub finite_difference_epsilon: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightLineageContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub tied_requirement_id: String,
    pub evidence_bundle_ref: String,
    pub evidence_bundle_digest: String,
    pub model_module_ref: String,
    pub transformer_module_ref: String,
    pub train_module_ref: String,
    pub runtime_module_ref: String,
    pub base_descriptor_ref: String,
    pub base_artifact_ref: String,
    pub produced_descriptor_ref: String,
    pub produced_artifact_ref: String,
    pub training_config: TassadarArticleTransformerWeightLineageTrainingConfigSnapshot,
    pub training_config_digest: String,
    pub training_cases: Vec<TassadarArticleTransformerWeightProductionCaseEvidence>,
    pub held_out_cases: Vec<TassadarArticleTransformerWeightProductionCaseEvidence>,
    pub workload_set_digest: String,
    pub source_inventory: Vec<TassadarArticleTransformerWeightLineageSourceRecord>,
    pub source_inventory_digest: String,
    pub artifact_inventory: Vec<TassadarArticleTransformerWeightLineageArtifactRecord>,
    pub artifact_inventory_digest: String,
    pub base_model_artifact_binding: TassadarArticleTransformerModelArtifactBinding,
    pub produced_model_artifact_binding: TassadarArticleTransformerModelArtifactBinding,
    pub base_descriptor_stable_digest: String,
    pub produced_descriptor_stable_digest: String,
    pub checkpoint: TassadarArticleTransformerCheckpointEvidence,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl TassadarArticleTransformerWeightLineageContract {
    #[must_use]
    pub fn with_contract_digest(mut self) -> Self {
        self.contract_digest.clear();
        self.contract_digest = stable_digest(
            b"psionic_tassadar_article_transformer_weight_lineage_contract|",
            &self,
        );
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightLineageManifestReview {
    pub lineage_contract_ref: String,
    pub contract_matches_current_truth: bool,
    pub contract_ref_under_fixtures_tassadar_models: bool,
    pub source_inventory_complete: bool,
    pub artifact_inventory_complete: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightLineageBoundaryReview {
    pub model_module_ref: String,
    pub transformer_module_ref: String,
    pub train_module_ref: String,
    pub runtime_module_ref: String,
    pub canonical_boundary_green: bool,
    pub evidence_roots_in_expected_modules: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerWeightLineageValidationKind {
    LineageManifestFrozen,
    DatasetLineageFrozen,
    TrainingConfigFrozen,
    ExactWorkloadSetFrozen,
    ArtifactInventoryAndDigestBinding,
    SourceToArtifactAudit,
    BoundaryRootedInTransformer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightLineageCaseRow {
    pub case_id: String,
    pub validation_kind: TassadarArticleTransformerWeightLineageValidationKind,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightLineageAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub evidence_bundle_ref: String,
    pub lineage_contract_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerWeightLineageReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarArticleTransformerWeightLineageAcceptanceGateTie,
    pub canonical_boundary_report_ref: String,
    pub canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub evidence_bundle_ref: String,
    pub evidence_bundle_digest: String,
    pub lineage_contract_ref: String,
    pub lineage_contract: TassadarArticleTransformerWeightLineageContract,
    pub manifest_review: TassadarArticleTransformerWeightLineageManifestReview,
    pub boundary_review: TassadarArticleTransformerWeightLineageBoundaryReview,
    pub case_rows: Vec<TassadarArticleTransformerWeightLineageCaseRow>,
    pub all_required_cases_present: bool,
    pub all_cases_pass: bool,
    pub weight_lineage_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerWeightLineageError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    CanonicalBoundary(#[from] TassadarCanonicalTransformerStackBoundaryReportError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
    #[error(transparent)]
    WeightProduction(#[from] TassadarArticleTransformerWeightProductionError),
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
pub fn tassadar_article_transformer_weight_lineage_contract_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF)
}

pub fn read_tassadar_article_transformer_weight_lineage_contract(
    relative_path: &str,
) -> Result<
    TassadarArticleTransformerWeightLineageContract,
    TassadarArticleTransformerWeightLineageError,
> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarArticleTransformerWeightLineageError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerWeightLineageError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

pub fn build_tassadar_article_transformer_weight_lineage_contract(
) -> Result<TassadarArticleTransformerWeightLineageContract, TassadarArticleTransformerWeightLineageError>
{
    let evidence_bundle = read_tassadar_article_transformer_weight_production_evidence_bundle(
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF,
    )?;
    let base_model = TassadarArticleTransformer::article_trace_domain_reference()?;
    let produced_model = TassadarArticleTransformer::trained_trace_domain_reference()?;
    build_tassadar_article_transformer_weight_lineage_contract_for_bundle(
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF,
        &evidence_bundle,
        &base_model,
        &produced_model,
        "tassadar.article_transformer_weight_lineage.contract.v1",
        TIED_REQUIREMENT_ID,
        "this lineage contract freezes only the first real trained trace-bound article-model weight artifact. It binds the exact workload set, training-config snapshot, source inventory, descriptor digests, checkpoint lineage, and committed artifact digests into one challengeable manifest without implying reference-linear exactness, fast-route closure, benchmark parity, or final article-equivalence green status.",
    )
}

pub fn build_tassadar_article_transformer_weight_lineage_contract_for_bundle(
    evidence_bundle_ref: impl Into<String>,
    evidence_bundle: &crate::TassadarArticleTransformerWeightProductionEvidenceBundle,
    base_model: &TassadarArticleTransformer,
    produced_model: &TassadarArticleTransformer,
    contract_id: impl Into<String>,
    tied_requirement_id: impl Into<String>,
    claim_boundary: impl Into<String>,
) -> Result<TassadarArticleTransformerWeightLineageContract, TassadarArticleTransformerWeightLineageError>
{
    let evidence_bundle_ref = evidence_bundle_ref.into();
    let training_config = training_config_snapshot(evidence_bundle);
    let source_inventory = source_inventory(evidence_bundle)?;
    let artifact_inventory = artifact_inventory(evidence_bundle_ref.as_str(), evidence_bundle)?;

    Ok(TassadarArticleTransformerWeightLineageContract {
        schema_version: 1,
        contract_id: contract_id.into(),
        tied_requirement_id: tied_requirement_id.into(),
        evidence_bundle_ref,
        evidence_bundle_digest: evidence_bundle.bundle_digest.clone(),
        model_module_ref: evidence_bundle.model_module_ref.clone(),
        transformer_module_ref: evidence_bundle.transformer_module_ref.clone(),
        train_module_ref: evidence_bundle.train_module_ref.clone(),
        runtime_module_ref: evidence_bundle.runtime_module_ref.clone(),
        base_descriptor_ref: evidence_bundle.base_descriptor_ref.clone(),
        base_artifact_ref: evidence_bundle.base_artifact_ref.clone(),
        produced_descriptor_ref: evidence_bundle.produced_descriptor_ref.clone(),
        produced_artifact_ref: evidence_bundle.produced_artifact_ref.clone(),
        training_config_digest: training_config_digest(&training_config),
        training_config,
        training_cases: evidence_bundle.training_cases.clone(),
        held_out_cases: evidence_bundle.held_out_cases.clone(),
        workload_set_digest: workload_set_digest(
            &evidence_bundle.training_cases,
            &evidence_bundle.held_out_cases,
        ),
        source_inventory_digest: source_inventory_digest(&source_inventory),
        source_inventory,
        artifact_inventory_digest: artifact_inventory_digest(&artifact_inventory),
        artifact_inventory,
        base_model_artifact_binding: evidence_bundle.base_model_artifact_binding.clone(),
        produced_model_artifact_binding: evidence_bundle.produced_model_artifact_binding.clone(),
        base_descriptor_stable_digest: base_model.descriptor().stable_digest(),
        produced_descriptor_stable_digest: produced_model.descriptor().stable_digest(),
        checkpoint: evidence_bundle.checkpoint.clone(),
        claim_boundary: claim_boundary.into(),
        contract_digest: String::new(),
    }
    .with_contract_digest())
}

pub fn write_tassadar_article_transformer_weight_lineage_contract(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerWeightLineageContract,
    TassadarArticleTransformerWeightLineageError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerWeightLineageError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = build_tassadar_article_transformer_weight_lineage_contract()?;
    let json = serde_json::to_string_pretty(&contract)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerWeightLineageError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(contract)
}

pub fn build_tassadar_article_transformer_weight_lineage_report() -> Result<
    TassadarArticleTransformerWeightLineageReport,
    TassadarArticleTransformerWeightLineageError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let canonical_boundary_report = build_tassadar_canonical_transformer_stack_boundary_report()?;
    let generated_contract = build_tassadar_article_transformer_weight_lineage_contract()?;
    let committed_contract = read_tassadar_article_transformer_weight_lineage_contract(
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
    )?;
    let manifest_review = manifest_review(&generated_contract, &committed_contract);
    let boundary_review = boundary_review(&committed_contract, &canonical_boundary_report);
    let case_rows = case_rows(&committed_contract, &manifest_review, &boundary_review);
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        canonical_boundary_report,
        committed_contract,
        manifest_review,
        boundary_review,
        case_rows,
    ))
}

#[must_use]
pub fn tassadar_article_transformer_weight_lineage_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF)
}

pub fn write_tassadar_article_transformer_weight_lineage_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerWeightLineageReport,
    TassadarArticleTransformerWeightLineageError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerWeightLineageError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_weight_lineage_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerWeightLineageError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    lineage_contract: TassadarArticleTransformerWeightLineageContract,
    manifest_review: TassadarArticleTransformerWeightLineageManifestReview,
    boundary_review: TassadarArticleTransformerWeightLineageBoundaryReview,
    case_rows: Vec<TassadarArticleTransformerWeightLineageCaseRow>,
) -> TassadarArticleTransformerWeightLineageReport {
    let acceptance_gate_tie = TassadarArticleTransformerWeightLineageAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        evidence_bundle_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF,
        ),
        lineage_contract_ref: String::from(TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF),
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
    let weight_lineage_contract_green = acceptance_gate_tie.tied_requirement_satisfied
        && canonical_boundary_report.boundary_contract_green
        && manifest_review.contract_matches_current_truth
        && manifest_review.contract_ref_under_fixtures_tassadar_models
        && manifest_review.source_inventory_complete
        && manifest_review.artifact_inventory_complete
        && all_required_cases_present
        && all_cases_pass
        && boundary_review.passed;
    let article_equivalence_green =
        weight_lineage_contract_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleTransformerWeightLineageReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_transformer_weight_lineage.report.v1"),
        acceptance_gate_tie,
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        canonical_boundary_report,
        evidence_bundle_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF,
        ),
        evidence_bundle_digest: lineage_contract.evidence_bundle_digest.clone(),
        lineage_contract_ref: String::from(TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF),
        lineage_contract,
        manifest_review,
        boundary_review,
        case_rows,
        all_required_cases_present,
        all_cases_pass,
        weight_lineage_contract_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report closes only the lineage and artifact-contract tranche around the first real trained trace-bound article-model weights. It proves the exact workload set, training-config snapshot, source inventory, checkpoint lineage, and artifact digests are frozen and challengeable. It does not claim reference-linear exactness, fast-route closure, benchmark parity, single-run closure, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article Transformer weight lineage contract now records case_rows={}, all_required_cases_present={}, all_cases_pass={}, contract_matches_current_truth={}, tied_requirement_satisfied={}, weight_lineage_contract_green={}, and article_equivalence_green={}.",
        report.case_rows.len(),
        report.all_required_cases_present,
        report.all_cases_pass,
        report.manifest_review.contract_matches_current_truth,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.weight_lineage_contract_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_transformer_weight_lineage_report|",
        &report,
    );
    report
}

fn required_validation_kinds() -> BTreeSet<TassadarArticleTransformerWeightLineageValidationKind> {
    BTreeSet::from([
        TassadarArticleTransformerWeightLineageValidationKind::LineageManifestFrozen,
        TassadarArticleTransformerWeightLineageValidationKind::DatasetLineageFrozen,
        TassadarArticleTransformerWeightLineageValidationKind::TrainingConfigFrozen,
        TassadarArticleTransformerWeightLineageValidationKind::ExactWorkloadSetFrozen,
        TassadarArticleTransformerWeightLineageValidationKind::ArtifactInventoryAndDigestBinding,
        TassadarArticleTransformerWeightLineageValidationKind::SourceToArtifactAudit,
        TassadarArticleTransformerWeightLineageValidationKind::BoundaryRootedInTransformer,
    ])
}

fn training_config_snapshot(
    evidence_bundle: &crate::TassadarArticleTransformerWeightProductionEvidenceBundle,
) -> TassadarArticleTransformerWeightLineageTrainingConfigSnapshot {
    TassadarArticleTransformerWeightLineageTrainingConfigSnapshot {
        run_id: evidence_bundle.run_id.clone(),
        checkpoint_family: evidence_bundle.checkpoint_family.clone(),
        source_corpus_id: evidence_bundle.source_corpus_id.clone(),
        suite_id: evidence_bundle.suite_id.clone(),
        suite_description: evidence_bundle.suite_description.clone(),
        architecture_variant: evidence_bundle.architecture_variant,
        transformer_config: evidence_bundle.config.clone(),
        embedding_strategy: evidence_bundle.embedding_strategy,
        trainable_parameter_ids: evidence_bundle.trainable_parameter_ids.clone(),
        trainable_parameter_scalar_count: evidence_bundle.trainable_parameter_scalar_count,
        loss_kind: evidence_bundle.loss_kind.clone(),
        optimizer_kind: evidence_bundle.optimizer_kind.clone(),
        scheduler_kind: evidence_bundle.scheduler_kind.clone(),
        warmup_steps: evidence_bundle.warmup_steps,
        label_smoothing: evidence_bundle.label_smoothing,
        finite_difference_epsilon: evidence_bundle.finite_difference_epsilon,
    }
}

fn source_inventory(
    evidence_bundle: &crate::TassadarArticleTransformerWeightProductionEvidenceBundle,
) -> Result<Vec<TassadarArticleTransformerWeightLineageSourceRecord>, TassadarArticleTransformerWeightLineageError>
{
    Ok(vec![
        source_record(
            TassadarArticleTransformerWeightLineageSourceRole::ModelModule,
            &evidence_bundle.model_module_ref,
        )?,
        source_record(
            TassadarArticleTransformerWeightLineageSourceRole::TransformerModule,
            &evidence_bundle.transformer_module_ref,
        )?,
        source_record(
            TassadarArticleTransformerWeightLineageSourceRole::TrainModule,
            &evidence_bundle.train_module_ref,
        )?,
        source_record(
            TassadarArticleTransformerWeightLineageSourceRole::RuntimeModule,
            &evidence_bundle.runtime_module_ref,
        )?,
    ])
}

fn artifact_inventory(
    evidence_bundle_ref: &str,
    evidence_bundle: &crate::TassadarArticleTransformerWeightProductionEvidenceBundle,
) -> Result<Vec<TassadarArticleTransformerWeightLineageArtifactRecord>, TassadarArticleTransformerWeightLineageError>
{
    Ok(vec![
        artifact_record(
            TassadarArticleTransformerWeightLineageArtifactRole::EvidenceBundle,
            evidence_bundle_ref,
        )?,
        artifact_record(
            TassadarArticleTransformerWeightLineageArtifactRole::BaseDescriptor,
            &evidence_bundle.base_descriptor_ref,
        )?,
        artifact_record(
            TassadarArticleTransformerWeightLineageArtifactRole::BaseWeights,
            &evidence_bundle.base_artifact_ref,
        )?,
        artifact_record(
            TassadarArticleTransformerWeightLineageArtifactRole::ProducedDescriptor,
            &evidence_bundle.produced_descriptor_ref,
        )?,
        artifact_record(
            TassadarArticleTransformerWeightLineageArtifactRole::ProducedWeights,
            &evidence_bundle.produced_artifact_ref,
        )?,
    ])
}

fn source_record(
    source_role: TassadarArticleTransformerWeightLineageSourceRole,
    relative_ref: &str,
) -> Result<TassadarArticleTransformerWeightLineageSourceRecord, TassadarArticleTransformerWeightLineageError>
{
    let (byte_length, sha256) = file_inventory(relative_ref)?;
    Ok(TassadarArticleTransformerWeightLineageSourceRecord {
        source_role,
        source_ref: String::from(relative_ref),
        byte_length,
        sha256,
    })
}

fn artifact_record(
    artifact_role: TassadarArticleTransformerWeightLineageArtifactRole,
    relative_ref: &str,
) -> Result<TassadarArticleTransformerWeightLineageArtifactRecord, TassadarArticleTransformerWeightLineageError>
{
    let (byte_length, sha256) = file_inventory(relative_ref)?;
    Ok(TassadarArticleTransformerWeightLineageArtifactRecord {
        artifact_role,
        artifact_ref: String::from(relative_ref),
        byte_length,
        sha256,
    })
}

fn manifest_review(
    generated_contract: &TassadarArticleTransformerWeightLineageContract,
    committed_contract: &TassadarArticleTransformerWeightLineageContract,
) -> TassadarArticleTransformerWeightLineageManifestReview {
    let source_roles = committed_contract
        .source_inventory
        .iter()
        .map(|row| row.source_role)
        .collect::<BTreeSet<_>>();
    let artifact_roles = committed_contract
        .artifact_inventory
        .iter()
        .map(|row| row.artifact_role)
        .collect::<BTreeSet<_>>();
    let source_inventory_complete = source_roles == expected_source_roles();
    let artifact_inventory_complete = artifact_roles == expected_artifact_roles();
    TassadarArticleTransformerWeightLineageManifestReview {
        lineage_contract_ref: String::from(TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF),
        contract_matches_current_truth: generated_contract == committed_contract,
        contract_ref_under_fixtures_tassadar_models:
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF
                .starts_with("fixtures/tassadar/models/"),
        source_inventory_complete,
        artifact_inventory_complete,
        detail: format!(
            "contract_matches_current_truth={}; source_inventory_complete={}; artifact_inventory_complete={}; contract_digest={}",
            generated_contract == committed_contract,
            source_inventory_complete,
            artifact_inventory_complete,
            committed_contract.contract_digest
        ),
    }
}

fn boundary_review(
    contract: &TassadarArticleTransformerWeightLineageContract,
    canonical_boundary_report: &TassadarCanonicalTransformerStackBoundaryReport,
) -> TassadarArticleTransformerWeightLineageBoundaryReview {
    let evidence_roots_in_expected_modules = contract.model_module_ref
        == TassadarArticleTransformer::MODEL_MODULE_REF
        && contract.transformer_module_ref == TassadarArticleTransformer::TRANSFORMER_MODULE_REF
        && contract.train_module_ref
            == "crates/psionic-train/src/tassadar_article_transformer_weight_production.rs"
        && contract.runtime_module_ref == "crates/psionic-runtime/src/tassadar.rs";
    TassadarArticleTransformerWeightLineageBoundaryReview {
        model_module_ref: contract.model_module_ref.clone(),
        transformer_module_ref: contract.transformer_module_ref.clone(),
        train_module_ref: contract.train_module_ref.clone(),
        runtime_module_ref: contract.runtime_module_ref.clone(),
        canonical_boundary_green: canonical_boundary_report.boundary_contract_green,
        evidence_roots_in_expected_modules,
        passed: canonical_boundary_report.boundary_contract_green
            && evidence_roots_in_expected_modules,
        detail: format!(
            "canonical_boundary_green={}; evidence_roots_in_expected_modules={}",
            canonical_boundary_report.boundary_contract_green, evidence_roots_in_expected_modules
        ),
    }
}

fn case_rows(
    contract: &TassadarArticleTransformerWeightLineageContract,
    manifest_review: &TassadarArticleTransformerWeightLineageManifestReview,
    boundary_review: &TassadarArticleTransformerWeightLineageBoundaryReview,
) -> Vec<TassadarArticleTransformerWeightLineageCaseRow> {
    vec![
        lineage_manifest_frozen_case(manifest_review),
        dataset_lineage_frozen_case(contract),
        training_config_frozen_case(contract),
        exact_workload_set_frozen_case(contract),
        artifact_inventory_and_digest_binding_case(contract),
        source_to_artifact_audit_case(contract),
        boundary_rooted_in_transformer_case(boundary_review),
    ]
}

fn lineage_manifest_frozen_case(
    manifest_review: &TassadarArticleTransformerWeightLineageManifestReview,
) -> TassadarArticleTransformerWeightLineageCaseRow {
    case_row(
        "lineage_manifest_frozen",
        TassadarArticleTransformerWeightLineageValidationKind::LineageManifestFrozen,
        manifest_review.contract_matches_current_truth
            && manifest_review.contract_ref_under_fixtures_tassadar_models,
        manifest_review.detail.clone(),
    )
}

fn dataset_lineage_frozen_case(
    contract: &TassadarArticleTransformerWeightLineageContract,
) -> TassadarArticleTransformerWeightLineageCaseRow {
    let cases_nonempty = contract
        .training_cases
        .iter()
        .chain(contract.held_out_cases.iter())
        .all(|case| {
            !case.case_id.trim().is_empty()
                && !case.profile_id.trim().is_empty()
                && !case.source_token_digest.trim().is_empty()
                && !case.target_token_digest.trim().is_empty()
                && !case.sequence_digest.trim().is_empty()
        });
    let recomputed_workload_digest =
        workload_set_digest(&contract.training_cases, &contract.held_out_cases);
    let passed = contract.training_config.source_corpus_id == EXPECTED_SOURCE_CORPUS_ID
        && contract.training_config.suite_id == EXPECTED_SUITE_ID
        && cases_nonempty
        && recomputed_workload_digest == contract.workload_set_digest;
    case_row(
        "dataset_lineage_frozen",
        TassadarArticleTransformerWeightLineageValidationKind::DatasetLineageFrozen,
        passed,
        format!(
            "source_corpus_id={}; suite_id={}; cases_nonempty={}; workload_set_digest_match={}",
            contract.training_config.source_corpus_id,
            contract.training_config.suite_id,
            cases_nonempty,
            recomputed_workload_digest == contract.workload_set_digest
        ),
    )
}

fn training_config_frozen_case(
    contract: &TassadarArticleTransformerWeightLineageContract,
) -> TassadarArticleTransformerWeightLineageCaseRow {
    let recomputed_training_config_digest = training_config_digest(&contract.training_config);
    let trainable_surface_matches = contract.training_config.trainable_parameter_ids
        == vec![String::from(
            TassadarArticleTransformer::LOGITS_PROJECTION_BIAS_PARAMETER_ID,
        )];
    let passed = recomputed_training_config_digest == contract.training_config_digest
        && contract.training_config.architecture_variant
            == TassadarArticleTransformerArchitectureVariant::AttentionIsAllYouNeedEncoderDecoder
        && contract.training_config.embedding_strategy
            == TassadarArticleTransformerEmbeddingStrategy::SharedSourceTargetAndOutput
        && contract.produced_model_artifact_binding.model_id
            == TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
        && contract.training_config.trainable_parameter_scalar_count == 303
        && trainable_surface_matches
        && contract.training_config.loss_kind == "label_smoothed_cross_entropy"
        && contract.training_config.optimizer_kind == "adam"
        && contract.training_config.scheduler_kind == "inverse_square_root_warmup"
        && contract.training_config.warmup_steps == 1;
    case_row(
        "training_config_frozen",
        TassadarArticleTransformerWeightLineageValidationKind::TrainingConfigFrozen,
        passed,
        format!(
            "training_config_digest_match={}; trainable_surface_matches={}; optimizer_kind={}; scheduler_kind={}; scalar_count={}",
            recomputed_training_config_digest == contract.training_config_digest,
            trainable_surface_matches,
            contract.training_config.optimizer_kind,
            contract.training_config.scheduler_kind,
            contract.training_config.trainable_parameter_scalar_count
        ),
    )
}

fn exact_workload_set_frozen_case(
    contract: &TassadarArticleTransformerWeightLineageContract,
) -> TassadarArticleTransformerWeightLineageCaseRow {
    let train_case_ids = contract
        .training_cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<BTreeSet<_>>();
    let held_out_case_ids = contract
        .held_out_cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<BTreeSet<_>>();
    let expected_train_case_ids = EXPECTED_TRAIN_CASE_IDS
        .iter()
        .map(|value| String::from(*value))
        .collect::<BTreeSet<_>>();
    let expected_held_out_case_ids = EXPECTED_HELD_OUT_CASE_IDS
        .iter()
        .map(|value| String::from(*value))
        .collect::<BTreeSet<_>>();
    let all_cases_use_expected_window = contract
        .training_cases
        .iter()
        .chain(contract.held_out_cases.iter())
        .all(|case| case.target_token_count == EXPECTED_TARGET_WINDOW_TOKENS);
    let passed = train_case_ids == expected_train_case_ids
        && held_out_case_ids == expected_held_out_case_ids
        && all_cases_use_expected_window;
    case_row(
        "exact_workload_set_frozen",
        TassadarArticleTransformerWeightLineageValidationKind::ExactWorkloadSetFrozen,
        passed,
        format!(
            "train_case_ids={:?}; held_out_case_ids={:?}; all_cases_use_expected_window={}",
            train_case_ids, held_out_case_ids, all_cases_use_expected_window
        ),
    )
}

fn artifact_inventory_and_digest_binding_case(
    contract: &TassadarArticleTransformerWeightLineageContract,
) -> TassadarArticleTransformerWeightLineageCaseRow {
    let artifact_roles = contract
        .artifact_inventory
        .iter()
        .map(|row| row.artifact_role)
        .collect::<BTreeSet<_>>();
    let artifact_digests_match = contract
        .artifact_inventory
        .iter()
        .all(|row| match file_inventory(&row.artifact_ref) {
            Ok((byte_length, sha256)) => byte_length == row.byte_length && sha256 == row.sha256,
            Err(_) => false,
        });
    let recomputed_inventory_digest = artifact_inventory_digest(&contract.artifact_inventory);
    let expected_roles_match = artifact_roles == expected_artifact_roles();
    let produced_weights_match_binding = artifact_record_by_role(
        &contract.artifact_inventory,
        TassadarArticleTransformerWeightLineageArtifactRole::ProducedWeights,
    )
    .map(|record| {
        record.sha256 == contract.produced_model_artifact_binding.primary_artifact_sha256
            && contract.produced_model_artifact_binding.weight_bundle_digest
                == contract.produced_model_artifact_binding.artifact_id
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
    })
    .unwrap_or(false);
    let base_weights_match_binding = artifact_record_by_role(
        &contract.artifact_inventory,
        TassadarArticleTransformerWeightLineageArtifactRole::BaseWeights,
    )
    .map(|record| {
        record.sha256 == contract.base_model_artifact_binding.primary_artifact_sha256
            && contract.base_model_artifact_binding.weight_bundle_digest
                == contract.base_model_artifact_binding.artifact_id
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
    })
    .unwrap_or(false);
    let descriptor_digests_match = contract.base_descriptor_stable_digest
        == contract.base_model_artifact_binding.descriptor_digest
        && contract.produced_descriptor_stable_digest
            == contract.produced_model_artifact_binding.descriptor_digest;
    let refs_under_expected_roots = contract
        .artifact_inventory
        .iter()
        .all(|row| row.artifact_ref.starts_with("fixtures/tassadar/"));
    let passed = expected_roles_match
        && artifact_digests_match
        && recomputed_inventory_digest == contract.artifact_inventory_digest
        && base_weights_match_binding
        && produced_weights_match_binding
        && descriptor_digests_match
        && refs_under_expected_roots;
    case_row(
        "artifact_inventory_and_digest_binding",
        TassadarArticleTransformerWeightLineageValidationKind::ArtifactInventoryAndDigestBinding,
        passed,
        format!(
            "expected_roles_match={}; artifact_digests_match={}; artifact_inventory_digest_match={}; base_weights_match_binding={}; produced_weights_match_binding={}; descriptor_digests_match={}; refs_under_expected_roots={}",
            expected_roles_match,
            artifact_digests_match,
            recomputed_inventory_digest == contract.artifact_inventory_digest,
            base_weights_match_binding,
            produced_weights_match_binding,
            descriptor_digests_match,
            refs_under_expected_roots
        ),
    )
}

fn source_to_artifact_audit_case(
    contract: &TassadarArticleTransformerWeightLineageContract,
) -> TassadarArticleTransformerWeightLineageCaseRow {
    let source_roles = contract
        .source_inventory
        .iter()
        .map(|row| row.source_role)
        .collect::<BTreeSet<_>>();
    let source_digests_match = contract
        .source_inventory
        .iter()
        .all(|row| match file_inventory(&row.source_ref) {
            Ok((byte_length, sha256)) => byte_length == row.byte_length && sha256 == row.sha256,
            Err(_) => false,
        });
    let recomputed_source_inventory_digest = source_inventory_digest(&contract.source_inventory);
    let expected_source_refs = BTreeSet::from([
        contract.model_module_ref.clone(),
        contract.transformer_module_ref.clone(),
        contract.train_module_ref.clone(),
        contract.runtime_module_ref.clone(),
    ]);
    let actual_source_refs = contract
        .source_inventory
        .iter()
        .map(|row| row.source_ref.clone())
        .collect::<BTreeSet<_>>();
    let checkpoint_matches_binding = contract.checkpoint.restore_matches_trained_state
        && contract.checkpoint.trained_trainable_parameter_digest
            == contract.produced_model_artifact_binding.trainable_parameter_digest
        && contract.checkpoint.restored_trainable_parameter_digest
            == contract.produced_model_artifact_binding.trainable_parameter_digest;
    let produced_weights_match_binding = artifact_record_by_role(
        &contract.artifact_inventory,
        TassadarArticleTransformerWeightLineageArtifactRole::ProducedWeights,
    )
    .map(|row| row.sha256 == contract.produced_model_artifact_binding.primary_artifact_sha256)
    .unwrap_or(false);
    let evidence_bundle_row_matches = artifact_record_by_role(
        &contract.artifact_inventory,
        TassadarArticleTransformerWeightLineageArtifactRole::EvidenceBundle,
    )
    .map(|row| row.artifact_ref == contract.evidence_bundle_ref)
    .unwrap_or(false);
    let passed = source_roles == expected_source_roles()
        && source_digests_match
        && recomputed_source_inventory_digest == contract.source_inventory_digest
        && actual_source_refs == expected_source_refs
        && checkpoint_matches_binding
        && produced_weights_match_binding
        && evidence_bundle_row_matches
        && !contract.evidence_bundle_digest.trim().is_empty();
    case_row(
        "source_to_artifact_audit",
        TassadarArticleTransformerWeightLineageValidationKind::SourceToArtifactAudit,
        passed,
        format!(
            "source_roles_complete={}; source_digests_match={}; source_inventory_digest_match={}; source_refs_match={}; checkpoint_matches_binding={}; produced_weights_match_binding={}; evidence_bundle_row_matches={}; evidence_bundle_digest_present={}",
            source_roles == expected_source_roles(),
            source_digests_match,
            recomputed_source_inventory_digest == contract.source_inventory_digest,
            actual_source_refs == expected_source_refs,
            checkpoint_matches_binding,
            produced_weights_match_binding,
            evidence_bundle_row_matches,
            !contract.evidence_bundle_digest.trim().is_empty()
        ),
    )
}

fn boundary_rooted_in_transformer_case(
    boundary_review: &TassadarArticleTransformerWeightLineageBoundaryReview,
) -> TassadarArticleTransformerWeightLineageCaseRow {
    case_row(
        "boundary_rooted_in_transformer",
        TassadarArticleTransformerWeightLineageValidationKind::BoundaryRootedInTransformer,
        boundary_review.passed,
        boundary_review.detail.clone(),
    )
}

fn case_row(
    case_id: &str,
    validation_kind: TassadarArticleTransformerWeightLineageValidationKind,
    passed: bool,
    detail: String,
) -> TassadarArticleTransformerWeightLineageCaseRow {
    TassadarArticleTransformerWeightLineageCaseRow {
        case_id: String::from(case_id),
        validation_kind,
        passed,
        detail,
    }
}

fn expected_source_roles() -> BTreeSet<TassadarArticleTransformerWeightLineageSourceRole> {
    BTreeSet::from([
        TassadarArticleTransformerWeightLineageSourceRole::ModelModule,
        TassadarArticleTransformerWeightLineageSourceRole::TransformerModule,
        TassadarArticleTransformerWeightLineageSourceRole::TrainModule,
        TassadarArticleTransformerWeightLineageSourceRole::RuntimeModule,
    ])
}

fn expected_artifact_roles() -> BTreeSet<TassadarArticleTransformerWeightLineageArtifactRole> {
    BTreeSet::from([
        TassadarArticleTransformerWeightLineageArtifactRole::EvidenceBundle,
        TassadarArticleTransformerWeightLineageArtifactRole::BaseDescriptor,
        TassadarArticleTransformerWeightLineageArtifactRole::BaseWeights,
        TassadarArticleTransformerWeightLineageArtifactRole::ProducedDescriptor,
        TassadarArticleTransformerWeightLineageArtifactRole::ProducedWeights,
    ])
}

fn artifact_record_by_role(
    records: &[TassadarArticleTransformerWeightLineageArtifactRecord],
    artifact_role: TassadarArticleTransformerWeightLineageArtifactRole,
) -> Option<&TassadarArticleTransformerWeightLineageArtifactRecord> {
    records.iter().find(|row| row.artifact_role == artifact_role)
}

fn file_inventory(
    relative_ref: &str,
) -> Result<(u64, String), TassadarArticleTransformerWeightLineageError> {
    let path = repo_root().join(relative_ref);
    let bytes = fs::read(&path).map_err(|error| TassadarArticleTransformerWeightLineageError::Read {
        path: path.display().to_string(),
        error,
    })?;
    Ok((bytes.len() as u64, sha256_bytes(&bytes)))
}

fn training_config_digest(
    training_config: &TassadarArticleTransformerWeightLineageTrainingConfigSnapshot,
) -> String {
    stable_digest(
        b"psionic_tassadar_article_transformer_weight_lineage_training_config|",
        training_config,
    )
}

fn workload_set_digest(
    training_cases: &[TassadarArticleTransformerWeightProductionCaseEvidence],
    held_out_cases: &[TassadarArticleTransformerWeightProductionCaseEvidence],
) -> String {
    stable_digest(
        b"psionic_tassadar_article_transformer_weight_lineage_workload_set|",
        &(training_cases, held_out_cases),
    )
}

fn source_inventory_digest(
    source_inventory: &[TassadarArticleTransformerWeightLineageSourceRecord],
) -> String {
    stable_digest(
        b"psionic_tassadar_article_transformer_weight_lineage_source_inventory|",
        source_inventory,
    )
}

fn artifact_inventory_digest(
    artifact_inventory: &[TassadarArticleTransformerWeightLineageArtifactRecord],
) -> String {
    stable_digest(
        b"psionic_tassadar_article_transformer_weight_lineage_artifact_inventory|",
        artifact_inventory,
    )
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize + ?Sized>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        artifact_inventory_and_digest_binding_case,
        build_report_from_inputs,
        build_tassadar_article_transformer_weight_lineage_contract,
        build_tassadar_article_transformer_weight_lineage_report,
        case_rows, manifest_review, source_to_artifact_audit_case,
        tassadar_article_transformer_weight_lineage_contract_path,
        tassadar_article_transformer_weight_lineage_report_path,
        write_tassadar_article_transformer_weight_lineage_contract,
        write_tassadar_article_transformer_weight_lineage_report,
        boundary_review, TassadarArticleTransformerWeightLineageContract,
        TassadarArticleTransformerWeightLineageReport,
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_canonical_transformer_stack_boundary_report,
    };
    use serde::de::DeserializeOwned;

    fn read_repo_json<T: DeserializeOwned>(
        relative_path: &str,
        artifact_kind: &str,
    ) -> Result<T, super::TassadarArticleTransformerWeightLineageError> {
        let path = super::repo_root().join(relative_path);
        let bytes = std::fs::read(&path)
            .map_err(|error| super::TassadarArticleTransformerWeightLineageError::Read {
                path: path.display().to_string(),
                error,
            })?;
        serde_json::from_slice(&bytes).map_err(|error| {
            super::TassadarArticleTransformerWeightLineageError::Decode {
                path: format!("{} ({artifact_kind})", path.display()),
                error,
            }
        })
    }

    #[test]
    fn article_transformer_weight_lineage_contract_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_weight_lineage_contract().expect("contract");
        let committed: TassadarArticleTransformerWeightLineageContract = read_repo_json(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
            "article_transformer_weight_lineage_contract",
        )
        .expect("committed contract");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_transformer_weight_lineage_contract_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_weight_lineage_contract.json");
        let written = write_tassadar_article_transformer_weight_lineage_contract(&output_path)
            .expect("write contract");
        let persisted: TassadarArticleTransformerWeightLineageContract =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_weight_lineage_contract_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_trace_bound_trained_v0_lineage_contract.json")
        );
    }

    #[test]
    fn article_transformer_weight_lineage_report_is_green_only_as_a_contract() {
        let report = build_tassadar_article_transformer_weight_lineage_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(report.acceptance_gate_tie.acceptance_status, crate::TassadarArticleEquivalenceAcceptanceStatus::Green);
        assert!(report.manifest_review.contract_matches_current_truth);
        assert!(report.weight_lineage_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.case_rows.len(), 7);
        assert_eq!(report.case_rows.iter().filter(|row| row.passed).count(), 7);
    }

    #[test]
    fn drifted_weight_lineage_contract_keeps_report_red() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let canonical_boundary_report =
            build_tassadar_canonical_transformer_stack_boundary_report().expect("boundary");
        let generated_contract =
            build_tassadar_article_transformer_weight_lineage_contract().expect("contract");
        let mut drifted_contract = generated_contract.clone();
        drifted_contract.produced_model_artifact_binding.primary_artifact_sha256 =
            String::from("drifted");
        let manifest_review = manifest_review(&generated_contract, &drifted_contract);
        let boundary_review = boundary_review(&drifted_contract, &canonical_boundary_report);
        let case_rows = case_rows(&drifted_contract, &manifest_review, &boundary_review);
        let report = build_report_from_inputs(
            acceptance_gate_report,
            canonical_boundary_report,
            drifted_contract.clone(),
            manifest_review,
            boundary_review,
            case_rows,
        );

        assert!(!report.weight_lineage_contract_green);
        assert!(!artifact_inventory_and_digest_binding_case(&drifted_contract).passed);
        assert!(!source_to_artifact_audit_case(&drifted_contract).passed);
    }

    #[test]
    fn article_transformer_weight_lineage_report_matches_committed_truth() {
        let generated = build_tassadar_article_transformer_weight_lineage_report().expect("report");
        let committed: TassadarArticleTransformerWeightLineageReport = read_repo_json(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
            "article_transformer_weight_lineage_report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_transformer_weight_lineage_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let contract_path = directory
            .path()
            .join("tassadar_article_transformer_weight_lineage_contract.json");
        let report_path = directory
            .path()
            .join("tassadar_article_transformer_weight_lineage_report.json");
        write_tassadar_article_transformer_weight_lineage_contract(&contract_path)
            .expect("write contract");
        let written = write_tassadar_article_transformer_weight_lineage_report(&report_path)
            .expect("write report");
        let persisted: TassadarArticleTransformerWeightLineageReport =
            serde_json::from_slice(&std::fs::read(&report_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_weight_lineage_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_weight_lineage_report.json")
        );
    }
}
