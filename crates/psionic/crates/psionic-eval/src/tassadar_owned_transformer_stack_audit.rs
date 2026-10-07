use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_equivalence_blocker_matrix_report,
    build_tassadar_article_transformer_forward_pass_closure_report,
    build_tassadar_article_transformer_model_closure_report,
    build_tassadar_article_transformer_training_closure_report,
    build_tassadar_attention_primitive_mask_closure_report,
    build_tassadar_canonical_transformer_stack_boundary_report,
    build_tassadar_existing_substrate_inventory_report,
    build_tassadar_transformer_block_closure_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleEquivalenceBlockerCategory,
    TassadarArticleEquivalenceBlockerMatrixReport,
    TassadarArticleEquivalenceBlockerMatrixReportError,
    TassadarArticleTransformerForwardPassClosureReport,
    TassadarArticleTransformerForwardPassClosureReportError,
    TassadarArticleTransformerModelClosureReport,
    TassadarArticleTransformerModelClosureReportError,
    TassadarArticleTransformerTrainingClosureReport,
    TassadarArticleTransformerTrainingClosureReportError,
    TassadarAttentionPrimitiveMaskClosureReport, TassadarAttentionPrimitiveMaskClosureReportError,
    TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError, TassadarExistingSubstrateInventoryReport,
    TassadarExistingSubstrateInventoryReportError, TassadarTransformerBlockClosureReport,
    TassadarTransformerBlockClosureReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLOSURE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_TRAINING_CLOSURE_REPORT_REF,
    TASSADAR_ATTENTION_PRIMITIVE_MASK_CLOSURE_REPORT_REF,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
    TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF,
    TASSADAR_TRANSFORMER_BLOCK_CLOSURE_REPORT_REF,
};

pub const TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_owned_transformer_stack_audit_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-166";
const BOUNDARY_DOC_REF: &str = "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const CORE_MODULE_REF: &str = "crates/psionic-core/src/lib.rs";
const ARRAY_MODULE_REF: &str = "crates/psionic-array/src/lib.rs";
const NN_MODULE_REF: &str = "crates/psionic-nn/src/lib.rs";
const TRANSFORMER_LIB_REF: &str = "crates/psionic-transformer/src/lib.rs";
const TRANSFORMER_ATTENTION_REF: &str = "crates/psionic-transformer/src/attention.rs";
const TRANSFORMER_BLOCKS_REF: &str = "crates/psionic-transformer/src/blocks.rs";
const TRANSFORMER_ENCODER_DECODER_REF: &str = "crates/psionic-transformer/src/encoder_decoder.rs";
const MODELS_ARTICLE_TRANSFORMER_REF: &str =
    "crates/psionic-models/src/tassadar_article_transformer.rs";
const MODELS_EXECUTOR_TRANSFORMER_REF: &str =
    "crates/psionic-models/src/tassadar_executor_transformer.rs";
const MODELS_FIXTURE_LANE_REF: &str = "crates/psionic-models/src/tassadar.rs";
const TRAIN_ARTICLE_TRANSFORMER_REF: &str =
    "crates/psionic-train/src/tassadar_article_transformer_training.rs";
const RUNTIME_FORWARD_PASS_REF: &str =
    "crates/psionic-runtime/src/tassadar_article_transformer_forward_pass.rs";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarOwnedTransformerStackSurfaceStatus {
    OwnedStackBacked,
    FixtureBacked,
    ResearchOnly,
    SubstrateOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarOwnedTransformerStackSurfaceRow {
    pub surface_id: String,
    pub status: TassadarOwnedTransformerStackSurfaceStatus,
    pub crate_id: String,
    pub item_refs: Vec<String>,
    pub article_route_role: String,
    pub current_truth: String,
    pub non_proof_boundary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarOwnedTransformerStackExtractionAspect {
    ReusableArchitectureInPsionicTransformer,
    CanonicalArticleWrapperInPsionicModels,
    LowerSubstrateInCoreArrayNn,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarOwnedTransformerStackExtractionRow {
    pub aspect: TassadarOwnedTransformerStackExtractionAspect,
    pub owner_crate_ids: Vec<String>,
    pub item_refs: Vec<String>,
    pub current_truth: String,
    pub non_implication: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarOwnedTransformerStackProvenanceRow {
    pub provenance_id: String,
    pub source_refs: Vec<String>,
    pub artifact_ref: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarOwnedTransformerStackRemainingBlockerRow {
    pub blocker_id: String,
    pub category: TassadarArticleEquivalenceBlockerCategory,
    pub title: String,
    pub current_gap_summary: String,
    pub open_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarOwnedTransformerStackBoundaryReview {
    pub boundary_doc_ref: String,
    pub transformer_lib_ref: String,
    pub transformer_encoder_decoder_ref: String,
    pub models_article_transformer_ref: String,
    pub models_executor_transformer_ref: String,
    pub models_fixture_lane_ref: String,
    pub core_ref: String,
    pub array_ref: String,
    pub nn_ref: String,
    pub boundary_doc_names_psionic_transformer_as_anchor: bool,
    pub boundary_doc_marks_executor_transformer_noncanonical: bool,
    pub transformer_exposes_attention_blocks_encoder_decoder: bool,
    pub models_define_canonical_article_wrapper: bool,
    pub executor_transformer_uses_programmatic_fixture_weights: bool,
    pub fixture_lane_uses_programmatic_fixture_weights: bool,
    pub substrate_rows_cover_core_array_nn_only: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarOwnedTransformerStackAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub blocker_matrix_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
    pub open_blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarOwnedTransformerStackAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarOwnedTransformerStackAcceptanceGateTie,
    pub boundary_doc_ref: String,
    pub existing_substrate_inventory_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub attention_primitive_mask_closure_report_ref: String,
    pub transformer_block_closure_report_ref: String,
    pub article_transformer_model_closure_report_ref: String,
    pub article_transformer_training_closure_report_ref: String,
    pub article_transformer_forward_pass_closure_report_ref: String,
    pub surface_rows: Vec<TassadarOwnedTransformerStackSurfaceRow>,
    pub extraction_rows: Vec<TassadarOwnedTransformerStackExtractionRow>,
    pub provenance_rows: Vec<TassadarOwnedTransformerStackProvenanceRow>,
    pub remaining_blocker_rows: Vec<TassadarOwnedTransformerStackRemainingBlockerRow>,
    pub boundary_review: TassadarOwnedTransformerStackBoundaryReview,
    pub surface_count: usize,
    pub owned_stack_backed_surface_count: usize,
    pub fixture_backed_surface_count: usize,
    pub research_only_surface_count: usize,
    pub substrate_only_surface_count: usize,
    pub remaining_blocker_count: usize,
    pub all_required_surface_statuses_present: bool,
    pub all_required_extraction_aspects_present: bool,
    pub all_provenance_rows_pass: bool,
    pub remaining_delta_explicit: bool,
    pub actual_owned_transformer_stack_exists: bool,
    pub psionic_transformer_extraction_explicit: bool,
    pub owned_transformer_stack_audit_green: bool,
    pub article_equivalence_green: bool,
    pub current_truth_boundary: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarOwnedTransformerStackAuditReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    BlockerMatrix(#[from] TassadarArticleEquivalenceBlockerMatrixReportError),
    #[error(transparent)]
    Inventory(#[from] TassadarExistingSubstrateInventoryReportError),
    #[error(transparent)]
    CanonicalBoundary(#[from] TassadarCanonicalTransformerStackBoundaryReportError),
    #[error(transparent)]
    AttentionPrimitive(#[from] TassadarAttentionPrimitiveMaskClosureReportError),
    #[error(transparent)]
    TransformerBlock(#[from] TassadarTransformerBlockClosureReportError),
    #[error(transparent)]
    ArticleTransformerModel(#[from] TassadarArticleTransformerModelClosureReportError),
    #[error(transparent)]
    ArticleTransformerTraining(#[from] TassadarArticleTransformerTrainingClosureReportError),
    #[error(transparent)]
    ArticleTransformerForwardPass(#[from] TassadarArticleTransformerForwardPassClosureReportError),
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

pub fn build_tassadar_owned_transformer_stack_audit_report(
) -> Result<TassadarOwnedTransformerStackAuditReport, TassadarOwnedTransformerStackAuditReportError>
{
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let blocker_matrix_report = build_tassadar_article_equivalence_blocker_matrix_report()?;
    let inventory_report = build_tassadar_existing_substrate_inventory_report()?;
    let canonical_boundary_report = build_tassadar_canonical_transformer_stack_boundary_report()?;
    let attention_report = build_tassadar_attention_primitive_mask_closure_report()?;
    let transformer_block_report = build_tassadar_transformer_block_closure_report()?;
    let article_transformer_model_report =
        build_tassadar_article_transformer_model_closure_report()?;
    let article_transformer_training_report =
        build_tassadar_article_transformer_training_closure_report()?;
    let article_transformer_forward_pass_report =
        build_tassadar_article_transformer_forward_pass_closure_report()?;
    let surface_rows = surface_rows();
    let extraction_rows = extraction_rows();
    let provenance_rows = provenance_rows(
        &inventory_report,
        &canonical_boundary_report,
        &attention_report,
        &transformer_block_report,
        &article_transformer_model_report,
        &article_transformer_training_report,
        &article_transformer_forward_pass_report,
    );
    let remaining_blocker_rows = remaining_blocker_rows(&blocker_matrix_report);
    let boundary_review = boundary_review(&surface_rows)?;
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        blocker_matrix_report,
        inventory_report,
        canonical_boundary_report,
        attention_report,
        transformer_block_report,
        article_transformer_model_report,
        article_transformer_training_report,
        article_transformer_forward_pass_report,
        surface_rows,
        extraction_rows,
        provenance_rows,
        remaining_blocker_rows,
        boundary_review,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    blocker_matrix_report: TassadarArticleEquivalenceBlockerMatrixReport,
    inventory_report: TassadarExistingSubstrateInventoryReport,
    canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    attention_report: TassadarAttentionPrimitiveMaskClosureReport,
    transformer_block_report: TassadarTransformerBlockClosureReport,
    article_transformer_model_report: TassadarArticleTransformerModelClosureReport,
    article_transformer_training_report: TassadarArticleTransformerTrainingClosureReport,
    article_transformer_forward_pass_report: TassadarArticleTransformerForwardPassClosureReport,
    surface_rows: Vec<TassadarOwnedTransformerStackSurfaceRow>,
    extraction_rows: Vec<TassadarOwnedTransformerStackExtractionRow>,
    provenance_rows: Vec<TassadarOwnedTransformerStackProvenanceRow>,
    remaining_blocker_rows: Vec<TassadarOwnedTransformerStackRemainingBlockerRow>,
    boundary_review: TassadarOwnedTransformerStackBoundaryReview,
) -> TassadarOwnedTransformerStackAuditReport {
    let acceptance_gate_tie = TassadarOwnedTransformerStackAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        blocker_matrix_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate_report
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate_report.acceptance_status,
        blocked_issue_ids: acceptance_gate_report.blocked_issue_ids.clone(),
        open_blocker_ids: blocker_matrix_report.open_blocker_ids.clone(),
    };
    let observed_surface_statuses = surface_rows
        .iter()
        .map(|row| row.status)
        .collect::<BTreeSet<_>>();
    let observed_extraction_aspects = extraction_rows
        .iter()
        .map(|row| row.aspect)
        .collect::<BTreeSet<_>>();
    let owned_stack_backed_surface_count = surface_rows
        .iter()
        .filter(|row| row.status == TassadarOwnedTransformerStackSurfaceStatus::OwnedStackBacked)
        .count();
    let fixture_backed_surface_count = surface_rows
        .iter()
        .filter(|row| row.status == TassadarOwnedTransformerStackSurfaceStatus::FixtureBacked)
        .count();
    let research_only_surface_count = surface_rows
        .iter()
        .filter(|row| row.status == TassadarOwnedTransformerStackSurfaceStatus::ResearchOnly)
        .count();
    let substrate_only_surface_count = surface_rows
        .iter()
        .filter(|row| row.status == TassadarOwnedTransformerStackSurfaceStatus::SubstrateOnly)
        .count();
    let all_required_surface_statuses_present =
        observed_surface_statuses == required_surface_statuses();
    let all_required_extraction_aspects_present =
        observed_extraction_aspects == required_extraction_aspects();
    let all_provenance_rows_pass = provenance_rows.iter().all(|row| row.passed);
    let remaining_delta_explicit = remaining_blocker_rows.len()
        == blocker_matrix_report.open_blocker_ids.len()
        && remaining_blocker_rows.iter().all(|row| {
            !row.current_gap_summary.trim().is_empty() && !row.open_issue_ids.is_empty()
        });
    let actual_owned_transformer_stack_exists = inventory_report.inventory_contract_green
        && canonical_boundary_report.boundary_contract_green
        && attention_report.attention_primitive_contract_green
        && transformer_block_report.transformer_block_contract_green
        && article_transformer_model_report.article_transformer_contract_green
        && article_transformer_training_report.article_transformer_training_contract_green
        && article_transformer_forward_pass_report.article_transformer_forward_pass_contract_green;
    let psionic_transformer_extraction_explicit = all_required_extraction_aspects_present
        && extraction_rows.iter().all(|row| !row.item_refs.is_empty());
    let owned_transformer_stack_audit_green = acceptance_gate_tie.tied_requirement_satisfied
        && all_required_surface_statuses_present
        && all_required_extraction_aspects_present
        && all_provenance_rows_pass
        && remaining_delta_explicit
        && actual_owned_transformer_stack_exists
        && psionic_transformer_extraction_explicit
        && boundary_review.passed;
    let article_equivalence_green =
        owned_transformer_stack_audit_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarOwnedTransformerStackAuditReport {
        schema_version: 1,
        report_id: String::from("tassadar.owned_transformer_stack_audit.report.v1"),
        acceptance_gate_tie,
        boundary_doc_ref: String::from(BOUNDARY_DOC_REF),
        existing_substrate_inventory_report_ref: String::from(
            TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF,
        ),
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        attention_primitive_mask_closure_report_ref: String::from(
            TASSADAR_ATTENTION_PRIMITIVE_MASK_CLOSURE_REPORT_REF,
        ),
        transformer_block_closure_report_ref: String::from(
            TASSADAR_TRANSFORMER_BLOCK_CLOSURE_REPORT_REF,
        ),
        article_transformer_model_closure_report_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF,
        ),
        article_transformer_training_closure_report_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_TRAINING_CLOSURE_REPORT_REF,
        ),
        article_transformer_forward_pass_closure_report_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLOSURE_REPORT_REF,
        ),
        surface_count: surface_rows.len(),
        owned_stack_backed_surface_count,
        fixture_backed_surface_count,
        research_only_surface_count,
        substrate_only_surface_count,
        remaining_blocker_count: remaining_blocker_rows.len(),
        surface_rows,
        extraction_rows,
        provenance_rows,
        remaining_blocker_rows,
        boundary_review,
        all_required_surface_statuses_present,
        all_required_extraction_aspects_present,
        all_provenance_rows_pass,
        remaining_delta_explicit,
        actual_owned_transformer_stack_exists,
        psionic_transformer_extraction_explicit,
        owned_transformer_stack_audit_green,
        article_equivalence_green,
        current_truth_boundary: String::from(
            "the repo now has a real canonical owned Transformer stack boundary with reusable architecture in `psionic-transformer`, the canonical article wrapper in `psionic-models`, a bounded article-Transformer training lane in `psionic-train`, and a runtime receipt lane in `psionic-runtime`. That is stronger than generic substrate overlap alone, but it still stops short of final article-equivalent closure because fixture-backed legacy lanes, research/comparison lanes, and multiple blocker categories remain open.",
        ),
        claim_boundary: String::from(
            "this audit freezes the boundary that the actual owned Transformer stack now exists in Psionic. It does not claim that the canonical article model artifact, weight lineage, reference-linear exactness gate, fast-route closure, frontend/compiler breadth, interpreter breadth, benchmark parity, single-run no-spill closure, clean-room weight causality, reproducibility, route-minimality, or final article-equivalence verdict are already complete.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Owned Transformer stack audit now records surface_count={}, owned_stack_backed_surface_count={}, fixture_backed_surface_count={}, research_only_surface_count={}, substrate_only_surface_count={}, remaining_blocker_count={}, actual_owned_transformer_stack_exists={}, owned_transformer_stack_audit_green={}, and article_equivalence_green={}.",
        report.surface_count,
        report.owned_stack_backed_surface_count,
        report.fixture_backed_surface_count,
        report.research_only_surface_count,
        report.substrate_only_surface_count,
        report.remaining_blocker_count,
        report.actual_owned_transformer_stack_exists,
        report.owned_transformer_stack_audit_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_owned_transformer_stack_audit_report|",
        &report,
    );
    report
}

fn required_surface_statuses() -> BTreeSet<TassadarOwnedTransformerStackSurfaceStatus> {
    BTreeSet::from([
        TassadarOwnedTransformerStackSurfaceStatus::OwnedStackBacked,
        TassadarOwnedTransformerStackSurfaceStatus::FixtureBacked,
        TassadarOwnedTransformerStackSurfaceStatus::ResearchOnly,
        TassadarOwnedTransformerStackSurfaceStatus::SubstrateOnly,
    ])
}

fn required_extraction_aspects() -> BTreeSet<TassadarOwnedTransformerStackExtractionAspect> {
    BTreeSet::from([
        TassadarOwnedTransformerStackExtractionAspect::ReusableArchitectureInPsionicTransformer,
        TassadarOwnedTransformerStackExtractionAspect::CanonicalArticleWrapperInPsionicModels,
        TassadarOwnedTransformerStackExtractionAspect::LowerSubstrateInCoreArrayNn,
    ])
}

fn surface_rows() -> Vec<TassadarOwnedTransformerStackSurfaceRow> {
    vec![
        surface_row(
            "psionic_core_tensor_contracts",
            TassadarOwnedTransformerStackSurfaceStatus::SubstrateOnly,
            "psionic-core",
            &[CORE_MODULE_REF],
            "tensor metadata and refusal vocabulary below the canonical article route",
            "the canonical stack still depends on `psionic-core` for lower-level tensor truth",
            "this crate does not itself prove paper-faithful Transformer architecture or article-equivalent behavior",
        ),
        surface_row(
            "psionic_array_bounded_array_ops",
            TassadarOwnedTransformerStackSurfaceStatus::SubstrateOnly,
            "psionic-array",
            &[ARRAY_MODULE_REF],
            "bounded array and materialization substrate below the canonical article route",
            "the canonical stack still depends on `psionic-array` for reusable matrix and array execution substrate",
            "this crate does not itself prove article-route ownership or article-equivalent closure",
        ),
        surface_row(
            "psionic_nn_module_state",
            TassadarOwnedTransformerStackSurfaceStatus::SubstrateOnly,
            "psionic-nn",
            &[NN_MODULE_REF],
            "module state and primitive layer substrate below `psionic-transformer`",
            "the canonical stack still depends on `psionic-nn` for reusable layer/state substrate",
            "this crate does not itself own the extracted article-Transformer architecture",
        ),
        surface_row(
            "psionic_transformer_reusable_architecture",
            TassadarOwnedTransformerStackSurfaceStatus::OwnedStackBacked,
            "psionic-transformer",
            &[
                TRANSFORMER_LIB_REF,
                TRANSFORMER_ATTENTION_REF,
                TRANSFORMER_BLOCKS_REF,
                TRANSFORMER_ENCODER_DECODER_REF,
            ],
            "reusable attention, block, and encoder-decoder architecture anchor for the canonical article route",
            "the extracted `psionic-transformer` crate now owns the reusable paper-faithful Transformer architecture path",
            "this reusable architecture anchor alone does not prove the final article artifact, benchmark parity, or final article-equivalence verdict",
        ),
        surface_row(
            "psionic_models_canonical_article_wrapper",
            TassadarOwnedTransformerStackSurfaceStatus::OwnedStackBacked,
            "psionic-models",
            &[MODELS_ARTICLE_TRANSFORMER_REF],
            "canonical article-model descriptor, wrapper, and trace-hook surface",
            "the canonical article wrapper now lives in `psionic-models` on top of the extracted `psionic-transformer` route",
            "this wrapper alone does not prove clean-room weights, artifact lineage, or final exactness closure",
        ),
        surface_row(
            "psionic_train_bounded_article_transformer_lane",
            TassadarOwnedTransformerStackSurfaceStatus::OwnedStackBacked,
            "psionic-train",
            &[TRAIN_ARTICLE_TRANSFORMER_REF],
            "bounded article-Transformer training lane rooted in the owned stack",
            "the repo now has one bounded article-Transformer training lane above the canonical wrapper",
            "this lane does not yet prove final article-model training, benchmark parity, or weight-production closure",
        ),
        surface_row(
            "psionic_runtime_article_transformer_receipt_lane",
            TassadarOwnedTransformerStackSurfaceStatus::OwnedStackBacked,
            "psionic-runtime",
            &[RUNTIME_FORWARD_PASS_REF],
            "runtime evidence and proof-bundle lane for the canonical article route",
            "the repo now has one runtime-owned forward-pass receipt lane above the owned stack",
            "this receipt lane does not yet prove article trace-vocabulary closure, fast-route closure, or final article equivalence",
        ),
        surface_row(
            "psionic_models_fixture_backed_article_executor_lane",
            TassadarOwnedTransformerStackSurfaceStatus::FixtureBacked,
            "psionic-models",
            &[MODELS_FIXTURE_LANE_REF],
            "legacy bounded article executor truth carried by programmatic fixtures",
            "the older fixture-backed article executor lane remains committed and citeable",
            "this lane cannot be treated as proof that the canonical Transformer-backed article route or weight lineage is already closed",
        ),
        surface_row(
            "psionic_models_executor_transformer_comparison_lane",
            TassadarOwnedTransformerStackSurfaceStatus::ResearchOnly,
            "psionic-models",
            &[MODELS_EXECUTOR_TRANSFORMER_REF],
            "legacy executor-transformer scaffold and comparison lane beside the canonical article wrapper",
            "the older executor-transformer scaffold remains a separate research and comparison lane",
            "this lane is not the canonical article route and must not be substituted for the owned paper-faithful stack",
        ),
    ]
}

fn extraction_rows() -> Vec<TassadarOwnedTransformerStackExtractionRow> {
    vec![
        TassadarOwnedTransformerStackExtractionRow {
            aspect:
                TassadarOwnedTransformerStackExtractionAspect::ReusableArchitectureInPsionicTransformer,
            owner_crate_ids: vec![String::from("psionic-transformer")],
            item_refs: vec![
                String::from(TRANSFORMER_ATTENTION_REF),
                String::from(TRANSFORMER_BLOCKS_REF),
                String::from(TRANSFORMER_ENCODER_DECODER_REF),
            ],
            current_truth: String::from(
                "reusable scaled dot-product attention, Transformer blocks, and the paper-faithful encoder-decoder stack now live in `psionic-transformer` as the extracted architecture anchor",
            ),
            non_implication: String::from(
                "this does not move article descriptors, fixture lanes, or runtime proof receipt ownership into `psionic-transformer`",
            ),
        },
        TassadarOwnedTransformerStackExtractionRow {
            aspect:
                TassadarOwnedTransformerStackExtractionAspect::CanonicalArticleWrapperInPsionicModels,
            owner_crate_ids: vec![String::from("psionic-models")],
            item_refs: vec![String::from(MODELS_ARTICLE_TRANSFORMER_REF)],
            current_truth: String::from(
                "the canonical article wrapper, paper reference, embedding-sharing surface, model descriptor, and forward-pass trace hook surface remain in `psionic-models` above the extracted architecture crate",
            ),
            non_implication: String::from(
                "this does not justify rebuilding reusable attention or encoder-decoder architecture back inside `psionic-models`",
            ),
        },
        TassadarOwnedTransformerStackExtractionRow {
            aspect: TassadarOwnedTransformerStackExtractionAspect::LowerSubstrateInCoreArrayNn,
            owner_crate_ids: vec![
                String::from("psionic-core"),
                String::from("psionic-array"),
                String::from("psionic-nn"),
            ],
            item_refs: vec![
                String::from(CORE_MODULE_REF),
                String::from(ARRAY_MODULE_REF),
                String::from(NN_MODULE_REF),
            ],
            current_truth: String::from(
                "tensor metadata, bounded array execution, and primitive layer/module-state substrate remain below the extracted Transformer architecture boundary in `psionic-core`, `psionic-array`, and `psionic-nn`",
            ),
            non_implication: String::from(
                "these lower crates are still substrate only for the article route and do not by themselves prove article-model ownership or article-equivalent closure",
            ),
        },
    ]
}

fn provenance_rows(
    inventory_report: &TassadarExistingSubstrateInventoryReport,
    canonical_boundary_report: &TassadarCanonicalTransformerStackBoundaryReport,
    attention_report: &TassadarAttentionPrimitiveMaskClosureReport,
    transformer_block_report: &TassadarTransformerBlockClosureReport,
    article_transformer_model_report: &TassadarArticleTransformerModelClosureReport,
    article_transformer_training_report: &TassadarArticleTransformerTrainingClosureReport,
    article_transformer_forward_pass_report: &TassadarArticleTransformerForwardPassClosureReport,
) -> Vec<TassadarOwnedTransformerStackProvenanceRow> {
    vec![
        provenance_row(
            "existing_substrate_inventory",
            &[MODELS_FIXTURE_LANE_REF, MODELS_EXECUTOR_TRANSFORMER_REF],
            TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF,
            inventory_report.inventory_contract_green,
            format!(
                "existing substrate inventory remains green and keeps fixture-backed plus research/comparison rows explicit at surface_count={}",
                inventory_report.surface_count
            ),
        ),
        provenance_row(
            "canonical_transformer_boundary",
            &[BOUNDARY_DOC_REF, TRANSFORMER_LIB_REF, MODELS_ARTICLE_TRANSFORMER_REF],
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
            canonical_boundary_report.boundary_contract_green,
            format!(
                "canonical boundary report remains green with interface_rows={} and dependency_checks={}",
                canonical_boundary_report.interface_rows.len(),
                canonical_boundary_report.dependency_checks.len()
            ),
        ),
        provenance_row(
            "owned_attention_primitive",
            &[TRANSFORMER_ATTENTION_REF],
            TASSADAR_ATTENTION_PRIMITIVE_MASK_CLOSURE_REPORT_REF,
            attention_report.attention_primitive_contract_green,
            format!(
                "attention closure remains green with case_rows={}",
                attention_report.case_rows.len()
            ),
        ),
        provenance_row(
            "owned_transformer_blocks",
            &[TRANSFORMER_BLOCKS_REF],
            TASSADAR_TRANSFORMER_BLOCK_CLOSURE_REPORT_REF,
            transformer_block_report.transformer_block_contract_green,
            format!(
                "Transformer block closure remains green with case_rows={}",
                transformer_block_report.case_rows.len()
            ),
        ),
        provenance_row(
            "canonical_article_wrapper",
            &[TRANSFORMER_ENCODER_DECODER_REF, MODELS_ARTICLE_TRANSFORMER_REF],
            TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF,
            article_transformer_model_report.article_transformer_contract_green,
            format!(
                "article-Transformer model closure remains green with case_rows={}",
                article_transformer_model_report.case_rows.len()
            ),
        ),
        provenance_row(
            "bounded_article_training_lane",
            &[MODELS_ARTICLE_TRANSFORMER_REF, TRAIN_ARTICLE_TRANSFORMER_REF],
            TASSADAR_ARTICLE_TRANSFORMER_TRAINING_CLOSURE_REPORT_REF,
            article_transformer_training_report.article_transformer_training_contract_green,
            format!(
                "article-Transformer training closure remains green with case_rows={}",
                article_transformer_training_report.case_rows.len()
            ),
        ),
        provenance_row(
            "runtime_receipt_lane",
            &[
                MODELS_ARTICLE_TRANSFORMER_REF,
                TRANSFORMER_ENCODER_DECODER_REF,
                RUNTIME_FORWARD_PASS_REF,
            ],
            TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLOSURE_REPORT_REF,
            article_transformer_forward_pass_report.article_transformer_forward_pass_contract_green,
            format!(
                "article-Transformer forward-pass closure remains green with case_rows={}",
                article_transformer_forward_pass_report.case_rows.len()
            ),
        ),
    ]
}

fn remaining_blocker_rows(
    blocker_matrix_report: &TassadarArticleEquivalenceBlockerMatrixReport,
) -> Vec<TassadarOwnedTransformerStackRemainingBlockerRow> {
    blocker_matrix_report
        .blockers
        .iter()
        .filter(|row| {
            blocker_matrix_report
                .open_blocker_ids
                .iter()
                .any(|blocker_id| blocker_id == &row.blocker_id)
        })
        .map(|row| {
            let open_issue_ids = row
                .covered_by_issue_ids
                .iter()
                .filter(|issue_id| {
                    blocker_matrix_report
                        .issue_coverage_rows
                        .iter()
                        .any(|issue_row| {
                            issue_row.issue_id == **issue_id && issue_row.issue_state == "open"
                        })
                })
                .cloned()
                .collect::<Vec<_>>();
            TassadarOwnedTransformerStackRemainingBlockerRow {
                blocker_id: row.blocker_id.clone(),
                category: row.category,
                title: row.title.clone(),
                current_gap_summary: row.current_gap_summary.clone(),
                open_issue_ids,
            }
        })
        .collect()
}

fn boundary_review(
    surface_rows: &[TassadarOwnedTransformerStackSurfaceRow],
) -> Result<
    TassadarOwnedTransformerStackBoundaryReview,
    TassadarOwnedTransformerStackAuditReportError,
> {
    let boundary_doc = read_repo_text(BOUNDARY_DOC_REF)?;
    let transformer_lib = read_repo_text(TRANSFORMER_LIB_REF)?;
    let models_article_transformer = read_repo_text(MODELS_ARTICLE_TRANSFORMER_REF)?;
    let models_executor_transformer = read_repo_text(MODELS_EXECUTOR_TRANSFORMER_REF)?;
    let models_fixture_lane = read_repo_text(MODELS_FIXTURE_LANE_REF)?;
    let substrate_only_crates = surface_rows
        .iter()
        .filter(|row| row.status == TassadarOwnedTransformerStackSurfaceStatus::SubstrateOnly)
        .map(|row| row.crate_id.as_str())
        .collect::<BTreeSet<_>>();
    let boundary_doc_names_psionic_transformer_as_anchor = boundary_doc
        .contains("`psionic-transformer` as the")
        && boundary_doc.contains("architecture anchor");
    let boundary_doc_marks_executor_transformer_noncanonical = boundary_doc
        .contains("`crates/psionic-models/src/tassadar_executor_transformer.rs`")
        && boundary_doc.contains("remains a separate research and comparison lane");
    let transformer_exposes_attention_blocks_encoder_decoder = contains_all(
        &transformer_lib,
        &["mod attention;", "mod blocks;", "mod encoder_decoder;"],
    );
    let models_define_canonical_article_wrapper = contains_all(
        &models_article_transformer,
        &[
            "pub struct TassadarArticleTransformer",
            "Attention Is All You Need",
            "forward_with_runtime_evidence",
        ],
    );
    let executor_transformer_uses_programmatic_fixture_weights =
        models_executor_transformer.contains("WeightFormat::ProgrammaticFixture");
    let fixture_lane_uses_programmatic_fixture_weights = contains_all(
        &models_fixture_lane,
        &[
            "pub struct TassadarExecutorFixture",
            "WeightFormat::ProgrammaticFixture",
        ],
    );
    let substrate_rows_cover_core_array_nn_only =
        substrate_only_crates == BTreeSet::from(["psionic-array", "psionic-core", "psionic-nn"]);
    let passed = boundary_doc_names_psionic_transformer_as_anchor
        && boundary_doc_marks_executor_transformer_noncanonical
        && transformer_exposes_attention_blocks_encoder_decoder
        && models_define_canonical_article_wrapper
        && executor_transformer_uses_programmatic_fixture_weights
        && fixture_lane_uses_programmatic_fixture_weights
        && substrate_rows_cover_core_array_nn_only;
    let detail = if passed {
        String::from(
            "the boundary doc, extracted transformer crate, canonical article wrapper, legacy executor-transformer scaffold, legacy fixture lane, and lower substrate rows all agree on the owned-stack split: reusable architecture in `psionic-transformer`, canonical wrapper in `psionic-models`, fixture-backed legacy lane still explicit, and `psionic-core` plus `psionic-array` plus `psionic-nn` still treated as lower substrate only",
        )
    } else {
        String::from(
            "the boundary review is incomplete: one or more of the extracted transformer anchor, canonical article wrapper, legacy fixture/comparison lane markers, or lower-substrate-only classifications drifted from the declared owned-stack split",
        )
    };
    Ok(TassadarOwnedTransformerStackBoundaryReview {
        boundary_doc_ref: String::from(BOUNDARY_DOC_REF),
        transformer_lib_ref: String::from(TRANSFORMER_LIB_REF),
        transformer_encoder_decoder_ref: String::from(TRANSFORMER_ENCODER_DECODER_REF),
        models_article_transformer_ref: String::from(MODELS_ARTICLE_TRANSFORMER_REF),
        models_executor_transformer_ref: String::from(MODELS_EXECUTOR_TRANSFORMER_REF),
        models_fixture_lane_ref: String::from(MODELS_FIXTURE_LANE_REF),
        core_ref: String::from(CORE_MODULE_REF),
        array_ref: String::from(ARRAY_MODULE_REF),
        nn_ref: String::from(NN_MODULE_REF),
        boundary_doc_names_psionic_transformer_as_anchor,
        boundary_doc_marks_executor_transformer_noncanonical,
        transformer_exposes_attention_blocks_encoder_decoder,
        models_define_canonical_article_wrapper,
        executor_transformer_uses_programmatic_fixture_weights,
        fixture_lane_uses_programmatic_fixture_weights,
        substrate_rows_cover_core_array_nn_only,
        passed,
        detail,
    })
}

pub fn tassadar_owned_transformer_stack_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_REPORT_REF)
}

pub fn write_tassadar_owned_transformer_stack_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarOwnedTransformerStackAuditReport, TassadarOwnedTransformerStackAuditReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarOwnedTransformerStackAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_owned_transformer_stack_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarOwnedTransformerStackAuditReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn surface_row(
    surface_id: &str,
    status: TassadarOwnedTransformerStackSurfaceStatus,
    crate_id: &str,
    item_refs: &[&str],
    article_route_role: &str,
    current_truth: &str,
    non_proof_boundary: &str,
) -> TassadarOwnedTransformerStackSurfaceRow {
    TassadarOwnedTransformerStackSurfaceRow {
        surface_id: String::from(surface_id),
        status,
        crate_id: String::from(crate_id),
        item_refs: item_refs.iter().map(|value| String::from(*value)).collect(),
        article_route_role: String::from(article_route_role),
        current_truth: String::from(current_truth),
        non_proof_boundary: String::from(non_proof_boundary),
    }
}

fn provenance_row(
    provenance_id: &str,
    source_refs: &[&str],
    artifact_ref: &str,
    passed: bool,
    detail: String,
) -> TassadarOwnedTransformerStackProvenanceRow {
    TassadarOwnedTransformerStackProvenanceRow {
        provenance_id: String::from(provenance_id),
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        artifact_ref: String::from(artifact_ref),
        passed,
        detail,
    }
}

fn contains_all(value: &str, required_fragments: &[&str]) -> bool {
    required_fragments
        .iter()
        .all(|fragment| value.contains(fragment))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn read_repo_text(
    relative_path: &str,
) -> Result<String, TassadarOwnedTransformerStackAuditReportError> {
    let path = repo_root().join(relative_path);
    fs::read_to_string(&path).map_err(
        |error| TassadarOwnedTransformerStackAuditReportError::Read {
            path: path.display().to_string(),
            error,
        },
    )
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarOwnedTransformerStackAuditReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarOwnedTransformerStackAuditReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarOwnedTransformerStackAuditReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        boundary_review, build_report_from_inputs,
        build_tassadar_owned_transformer_stack_audit_report, extraction_rows, provenance_rows,
        read_json, remaining_blocker_rows, surface_rows,
        tassadar_owned_transformer_stack_audit_report_path,
        write_tassadar_owned_transformer_stack_audit_report,
        TassadarOwnedTransformerStackAuditReport,
        TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_REPORT_REF,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_article_equivalence_blocker_matrix_report,
        build_tassadar_article_transformer_forward_pass_closure_report,
        build_tassadar_article_transformer_model_closure_report,
        build_tassadar_article_transformer_training_closure_report,
        build_tassadar_attention_primitive_mask_closure_report,
        build_tassadar_canonical_transformer_stack_boundary_report,
        build_tassadar_existing_substrate_inventory_report,
        build_tassadar_transformer_block_closure_report,
    };

    #[test]
    fn owned_transformer_stack_audit_tracks_real_stack_without_final_article_green() {
        let report = build_tassadar_owned_transformer_stack_audit_report().expect("audit");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(
            report.acceptance_gate_tie.acceptance_status,
            crate::TassadarArticleEquivalenceAcceptanceStatus::Green
        );
        assert!(report.actual_owned_transformer_stack_exists);
        assert!(report.all_required_surface_statuses_present);
        assert!(report.all_required_extraction_aspects_present);
        assert!(report.all_provenance_rows_pass);
        assert!(report.remaining_delta_explicit);
        assert!(report.boundary_review.passed);
        assert_eq!(report.owned_stack_backed_surface_count, 4);
        assert_eq!(report.fixture_backed_surface_count, 1);
        assert_eq!(report.research_only_surface_count, 1);
        assert_eq!(report.substrate_only_surface_count, 3);
        assert_eq!(report.remaining_blocker_count, 7);
        assert!(report.owned_transformer_stack_audit_green);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn failed_provenance_keeps_owned_transformer_stack_audit_red() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let blocker_matrix_report =
            build_tassadar_article_equivalence_blocker_matrix_report().expect("matrix");
        let inventory_report =
            build_tassadar_existing_substrate_inventory_report().expect("inventory");
        let canonical_boundary_report =
            build_tassadar_canonical_transformer_stack_boundary_report().expect("boundary");
        let attention_report =
            build_tassadar_attention_primitive_mask_closure_report().expect("attention");
        let transformer_block_report =
            build_tassadar_transformer_block_closure_report().expect("block");
        let article_transformer_model_report =
            build_tassadar_article_transformer_model_closure_report().expect("model");
        let article_transformer_training_report =
            build_tassadar_article_transformer_training_closure_report().expect("training");
        let article_transformer_forward_pass_report =
            build_tassadar_article_transformer_forward_pass_closure_report().expect("forward");
        let surface_rows = surface_rows();
        let extraction_rows = extraction_rows();
        let mut provenance_rows = provenance_rows(
            &inventory_report,
            &canonical_boundary_report,
            &attention_report,
            &transformer_block_report,
            &article_transformer_model_report,
            &article_transformer_training_report,
            &article_transformer_forward_pass_report,
        );
        provenance_rows[0].passed = false;
        let remaining_blocker_rows = remaining_blocker_rows(&blocker_matrix_report);
        let boundary_review = boundary_review(&surface_rows).expect("boundary review");

        let report = build_report_from_inputs(
            acceptance_gate_report,
            blocker_matrix_report,
            inventory_report,
            canonical_boundary_report,
            attention_report,
            transformer_block_report,
            article_transformer_model_report,
            article_transformer_training_report,
            article_transformer_forward_pass_report,
            surface_rows,
            extraction_rows,
            provenance_rows,
            remaining_blocker_rows,
            boundary_review,
        );

        assert!(!report.owned_transformer_stack_audit_green);
        assert!(!report.article_equivalence_green);
    }

    #[test]
    fn owned_transformer_stack_audit_matches_committed_truth() {
        let generated = build_tassadar_owned_transformer_stack_audit_report().expect("audit");
        let committed: TassadarOwnedTransformerStackAuditReport =
            read_json(tassadar_owned_transformer_stack_audit_report_path()).expect("committed");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_owned_transformer_stack_audit_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_owned_transformer_stack_audit_report.json");
        let written = write_tassadar_owned_transformer_stack_audit_report(&output_path)
            .expect("write report");
        let persisted: TassadarOwnedTransformerStackAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_owned_transformer_stack_audit_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_owned_transformer_stack_audit_report.json")
        );
        assert_eq!(
            TASSADAR_OWNED_TRANSFORMER_STACK_AUDIT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_owned_transformer_stack_audit_report.json"
        );
    }
}
