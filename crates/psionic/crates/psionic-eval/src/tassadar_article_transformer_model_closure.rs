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

use psionic_core::Shape;
use psionic_models::{
    TassadarArticleTransformer, TassadarArticleTransformerArchitectureVariant,
    TassadarArticleTransformerEmbeddingStrategy,
};
use psionic_transformer::TransformerExecutionMode;

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_canonical_transformer_stack_boundary_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_model_closure_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-163";
const TRANSFORMER_ENCODER_DECODER_MODULE_REF: &str =
    "crates/psionic-transformer/src/encoder_decoder.rs";
const MODELS_ARTICLE_TRANSFORMER_MODULE_REF: &str =
    "crates/psionic-models/src/tassadar_article_transformer.rs";
const MODELS_EXECUTOR_TRANSFORMER_MODULE_REF: &str =
    "crates/psionic-models/src/tassadar_executor_transformer.rs";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerValidationKind {
    PaperFaithfulDescriptor,
    EncoderDecoderForward,
    DecoderMaskEnforcement,
    CrossAttentionExecution,
    LogitsProjection,
    EmbeddingSharingOptions,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerCaseRow {
    pub case_id: String,
    pub validation_kind: TassadarArticleTransformerValidationKind,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerBoundaryReview {
    pub transformer_encoder_decoder_module_ref: String,
    pub models_article_transformer_module_ref: String,
    pub models_executor_transformer_module_ref: String,
    pub transformer_defines_encoder_layer: bool,
    pub transformer_defines_decoder_layer: bool,
    pub transformer_defines_encoder_decoder_transformer: bool,
    pub models_define_article_transformer: bool,
    pub models_use_owned_encoder_decoder_transformer: bool,
    pub models_reference_attention_is_all_you_need: bool,
    pub executor_scaffold_mentions_article_transformer: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerModelClosureReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarArticleTransformerAcceptanceGateTie,
    pub canonical_boundary_report_ref: String,
    pub canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub case_rows: Vec<TassadarArticleTransformerCaseRow>,
    pub boundary_review: TassadarArticleTransformerBoundaryReview,
    pub all_required_cases_present: bool,
    pub all_cases_pass: bool,
    pub article_transformer_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerModelClosureReportError {
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

pub fn build_tassadar_article_transformer_model_closure_report(
) -> Result<
    TassadarArticleTransformerModelClosureReport,
    TassadarArticleTransformerModelClosureReportError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let boundary_report = build_tassadar_canonical_transformer_stack_boundary_report()?;
    let case_rows = case_rows();
    let boundary_review = boundary_review()?;
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        boundary_report,
        case_rows,
        boundary_review,
    ))
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    case_rows: Vec<TassadarArticleTransformerCaseRow>,
    boundary_review: TassadarArticleTransformerBoundaryReview,
) -> TassadarArticleTransformerModelClosureReport {
    let acceptance_gate_tie = TassadarArticleTransformerAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
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
    let article_transformer_contract_green = acceptance_gate_tie.tied_requirement_satisfied
        && canonical_boundary_report.boundary_contract_green
        && all_required_cases_present
        && all_cases_pass
        && boundary_review.passed;
    let article_equivalence_green =
        article_transformer_contract_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleTransformerModelClosureReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_transformer_model_closure.report.v1"),
        acceptance_gate_tie,
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        canonical_boundary_report,
        case_rows,
        boundary_review,
        all_required_cases_present,
        all_cases_pass,
        article_transformer_contract_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report freezes the canonical paper-faithful article-Transformer model path only. It proves that `psionic-transformer` now owns the reusable encoder-decoder stack and that `psionic-models` now owns one canonical article wrapper on top of that stack. It does not claim trace-vocabulary closure, artifact-backed lineage, training success, exactness, benchmark parity, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article Transformer model closure now records case_rows={}, all_required_cases_present={}, all_cases_pass={}, boundary_review_passed={}, tied_requirement_satisfied={}, article_transformer_contract_green={}, and article_equivalence_green={}.",
        report.case_rows.len(),
        report.all_required_cases_present,
        report.all_cases_pass,
        report.boundary_review.passed,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.article_transformer_contract_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_transformer_model_closure_report|",
        &report,
    );
    report
}

fn required_validation_kinds() -> BTreeSet<TassadarArticleTransformerValidationKind> {
    BTreeSet::from([
        TassadarArticleTransformerValidationKind::PaperFaithfulDescriptor,
        TassadarArticleTransformerValidationKind::EncoderDecoderForward,
        TassadarArticleTransformerValidationKind::DecoderMaskEnforcement,
        TassadarArticleTransformerValidationKind::CrossAttentionExecution,
        TassadarArticleTransformerValidationKind::LogitsProjection,
        TassadarArticleTransformerValidationKind::EmbeddingSharingOptions,
    ])
}

fn case_rows() -> Vec<TassadarArticleTransformerCaseRow> {
    vec![
        paper_faithful_descriptor_case(),
        encoder_decoder_forward_case(),
        decoder_mask_enforcement_case(),
        cross_attention_execution_case(),
        logits_projection_case(),
        embedding_sharing_options_case(),
    ]
}

fn paper_faithful_descriptor_case() -> TassadarArticleTransformerCaseRow {
    match TassadarArticleTransformer::canonical_reference() {
        Ok(model)
            if model.descriptor().architecture_variant
                == TassadarArticleTransformerArchitectureVariant::AttentionIsAllYouNeedEncoderDecoder
                && model.descriptor().paper_faithful
                && model.descriptor().substitution_justification.is_none() =>
        {
            case_row(
                "paper_faithful_descriptor",
                TassadarArticleTransformerValidationKind::PaperFaithfulDescriptor,
                true,
                "the canonical article wrapper in `psionic-models` now names `Attention Is All You Need` explicitly and records a literal paper-faithful encoder-decoder route with no substitution justification",
            )
        }
        Ok(model) => case_row(
            "paper_faithful_descriptor",
            TassadarArticleTransformerValidationKind::PaperFaithfulDescriptor,
            false,
            format!(
                "unexpected descriptor variant={:?} paper_faithful={} substitution_justification={:?}",
                model.descriptor().architecture_variant,
                model.descriptor().paper_faithful,
                model.descriptor().substitution_justification
            ),
        ),
        Err(error) => case_row(
            "paper_faithful_descriptor",
            TassadarArticleTransformerValidationKind::PaperFaithfulDescriptor,
            false,
            format!("failed to build canonical article transformer: {error}"),
        ),
    }
}

fn encoder_decoder_forward_case() -> TassadarArticleTransformerCaseRow {
    match TassadarArticleTransformer::canonical_reference().and_then(|model| {
        model.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            Shape::new(vec![1, 3]),
            &[0, 1, 2],
            TransformerExecutionMode::Eval,
        )
        .map(|output| (model, output))
    }) {
        Ok((model, output))
            if output.encoder_hidden_state.dims() == [1, 2, 4]
                && output.decoder_hidden_state.dims() == [1, 3, 4]
                && output.logits.dims()
                    == [
                        1,
                        3,
                        model.descriptor().config.target_vocab_size,
                    ]
                && output.encoder_layer_outputs.len()
                    == model.descriptor().config.encoder_layer_count
                && output.decoder_layer_outputs.len()
                    == model.descriptor().config.decoder_layer_count =>
        {
            case_row(
                "encoder_decoder_forward",
                TassadarArticleTransformerValidationKind::EncoderDecoderForward,
                true,
                "the canonical article route now runs one owned encoder stack plus one owned decoder stack and emits logits from the decoder hidden state with explicit layer-count parity",
            )
        }
        Ok((model, output)) => case_row(
            "encoder_decoder_forward",
            TassadarArticleTransformerValidationKind::EncoderDecoderForward,
            false,
            format!(
                "unexpected forward surface encoder_dims={:?} decoder_dims={:?} logits_dims={:?} encoder_layers={} decoder_layers={} expected_encoder_layers={} expected_decoder_layers={}",
                output.encoder_hidden_state.dims(),
                output.decoder_hidden_state.dims(),
                output.logits.dims(),
                output.encoder_layer_outputs.len(),
                output.decoder_layer_outputs.len(),
                model.descriptor().config.encoder_layer_count,
                model.descriptor().config.decoder_layer_count,
            ),
        ),
        Err(error) => case_row(
            "encoder_decoder_forward",
            TassadarArticleTransformerValidationKind::EncoderDecoderForward,
            false,
            format!("encoder-decoder forward case failed: {error}"),
        ),
    }
}

fn decoder_mask_enforcement_case() -> TassadarArticleTransformerCaseRow {
    match TassadarArticleTransformer::canonical_reference() {
        Ok(model) => {
            let left = model.forward(
                Shape::new(vec![1, 2]),
                &[0, 1],
                Shape::new(vec![1, 3]),
                &[0, 1, 2],
                TransformerExecutionMode::Eval,
            );
            let right = model.forward(
                Shape::new(vec![1, 2]),
                &[0, 1],
                Shape::new(vec![1, 3]),
                &[0, 1, 3],
                TransformerExecutionMode::Eval,
            );
            match (left, right) {
                (Ok(left), Ok(right)) => {
                    let left_logits = left.logits.as_f32_slice();
                    let right_logits = right.logits.as_f32_slice();
                    match (left_logits, right_logits) {
                        (Ok(left_logits), Ok(right_logits))
                            if left_logits[..16] == right_logits[..16]
                                && left_logits[16..24] != right_logits[16..24] =>
                        {
                            case_row(
                                "decoder_mask_enforcement",
                                TassadarArticleTransformerValidationKind::DecoderMaskEnforcement,
                                true,
                                "masked self-attention is now enforced on the canonical article route: changing a future token leaves earlier logits unchanged while the final position still moves",
                            )
                        }
                        (Ok(left_logits), Ok(right_logits)) => case_row(
                            "decoder_mask_enforcement",
                            TassadarArticleTransformerValidationKind::DecoderMaskEnforcement,
                            false,
                            format!(
                                "unexpected decoder mask behavior left_logits={left_logits:?} right_logits={right_logits:?}",
                            ),
                        ),
                        (Err(error), _) | (_, Err(error)) => case_row(
                            "decoder_mask_enforcement",
                            TassadarArticleTransformerValidationKind::DecoderMaskEnforcement,
                            false,
                            format!("failed to inspect logits for decoder mask case: {error}"),
                        ),
                    }
                }
                (Err(error), _) | (_, Err(error)) => case_row(
                    "decoder_mask_enforcement",
                    TassadarArticleTransformerValidationKind::DecoderMaskEnforcement,
                    false,
                    format!("decoder mask case failed: {error}"),
                ),
            }
        }
        Err(error) => case_row(
            "decoder_mask_enforcement",
            TassadarArticleTransformerValidationKind::DecoderMaskEnforcement,
            false,
            format!("failed to construct canonical article transformer: {error}"),
        ),
    }
}

fn cross_attention_execution_case() -> TassadarArticleTransformerCaseRow {
    match TassadarArticleTransformer::canonical_reference() {
        Ok(model) => {
            let left = model.forward(
                Shape::new(vec![1, 2]),
                &[0, 1],
                Shape::new(vec![1, 2]),
                &[0, 1],
                TransformerExecutionMode::Eval,
            );
            let right = model.forward(
                Shape::new(vec![1, 2]),
                &[2, 3],
                Shape::new(vec![1, 2]),
                &[0, 1],
                TransformerExecutionMode::Eval,
            );
            match (left, right) {
                (Ok(left), Ok(right))
                    if left.decoder_hidden_state != right.decoder_hidden_state
                        && left.decoder_layer_outputs[0]
                            .cross_attention_trace
                            .probabilities
                            .shape()
                            == [1, 2, 2, 2] =>
                {
                    case_row(
                        "cross_attention_execution",
                        TassadarArticleTransformerValidationKind::CrossAttentionExecution,
                        true,
                        "cross-attention is now part of the canonical article route and the decoder hidden state changes when the encoder sequence changes",
                    )
                }
                (Ok(left), Ok(right)) => case_row(
                    "cross_attention_execution",
                    TassadarArticleTransformerValidationKind::CrossAttentionExecution,
                    false,
                    format!(
                        "unexpected cross-attention behavior left_decoder={:?} right_decoder={:?}",
                        left.decoder_hidden_state.as_f32_slice().unwrap_or(&[]),
                        right.decoder_hidden_state.as_f32_slice().unwrap_or(&[])
                    ),
                ),
                (Err(error), _) | (_, Err(error)) => case_row(
                    "cross_attention_execution",
                    TassadarArticleTransformerValidationKind::CrossAttentionExecution,
                    false,
                    format!("cross-attention case failed: {error}"),
                ),
            }
        }
        Err(error) => case_row(
            "cross_attention_execution",
            TassadarArticleTransformerValidationKind::CrossAttentionExecution,
            false,
            format!("failed to construct canonical article transformer: {error}"),
        ),
    }
}

fn logits_projection_case() -> TassadarArticleTransformerCaseRow {
    match TassadarArticleTransformer::canonical_reference().and_then(|model| {
        model.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            Shape::new(vec![1, 3]),
            &[0, 1, 2],
            TransformerExecutionMode::Eval,
        )
        .map(|output| (model, output))
    }) {
        Ok((model, output))
            if output.logits.dims()
                == [1, 3, model.descriptor().config.target_vocab_size]
                && output
                    .decoder_layer_outputs
                    .iter()
                    .all(|layer| layer.hidden_state.dims() == [1, 3, 4]) =>
        {
            case_row(
                "logits_projection",
                TassadarArticleTransformerValidationKind::LogitsProjection,
                true,
                "the canonical article route now projects decoder hidden states into explicit vocabulary logits inside the owned encoder-decoder model path",
            )
        }
        Ok((model, output)) => case_row(
            "logits_projection",
            TassadarArticleTransformerValidationKind::LogitsProjection,
            false,
            format!(
                "unexpected logits projection dims={:?} expected_target_vocab={}",
                output.logits.dims(),
                model.descriptor().config.target_vocab_size
            ),
        ),
        Err(error) => case_row(
            "logits_projection",
            TassadarArticleTransformerValidationKind::LogitsProjection,
            false,
            format!("logits projection case failed: {error}"),
        ),
    }
}

fn embedding_sharing_options_case() -> TassadarArticleTransformerCaseRow {
    let config = TassadarArticleTransformer::tiny_reference_config();
    let unshared = TassadarArticleTransformer::paper_faithful_reference(
        config.clone(),
        TassadarArticleTransformerEmbeddingStrategy::Unshared,
    );
    let tied = TassadarArticleTransformer::paper_faithful_reference(
        config.clone(),
        TassadarArticleTransformerEmbeddingStrategy::DecoderInputOutputTied,
    );
    let shared = TassadarArticleTransformer::paper_faithful_reference(
        config,
        TassadarArticleTransformerEmbeddingStrategy::SharedSourceTargetAndOutput,
    );
    match (unshared, tied, shared) {
        (Ok(unshared), Ok(tied), Ok(shared))
            if unshared.target_embedding_table() != unshared.logits_projection_weight()
                && tied.target_embedding_table() == tied.logits_projection_weight()
                && shared.source_embedding_table() == shared.target_embedding_table()
                && shared.target_embedding_table() == shared.logits_projection_weight() =>
        {
            case_row(
                "embedding_sharing_options",
                TassadarArticleTransformerValidationKind::EmbeddingSharingOptions,
                true,
                "the canonical article wrapper now supports explicit unshared, decoder-tied, and fully shared source/target/output weight-tying options",
            )
        }
        (Ok(unshared), Ok(tied), Ok(shared)) => case_row(
            "embedding_sharing_options",
            TassadarArticleTransformerValidationKind::EmbeddingSharingOptions,
            false,
            format!(
                "unexpected sharing surfaces unshared_equal={} tied_equal={} shared_source_target_equal={} shared_target_output_equal={}",
                unshared.target_embedding_table() == unshared.logits_projection_weight(),
                tied.target_embedding_table() == tied.logits_projection_weight(),
                shared.source_embedding_table() == shared.target_embedding_table(),
                shared.target_embedding_table() == shared.logits_projection_weight(),
            ),
        ),
        (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => case_row(
            "embedding_sharing_options",
            TassadarArticleTransformerValidationKind::EmbeddingSharingOptions,
            false,
            format!("embedding sharing case failed: {error}"),
        ),
    }
}

fn case_row(
    case_id: &str,
    validation_kind: TassadarArticleTransformerValidationKind,
    passed: bool,
    detail: impl Into<String>,
) -> TassadarArticleTransformerCaseRow {
    TassadarArticleTransformerCaseRow {
        case_id: String::from(case_id),
        validation_kind,
        passed,
        detail: detail.into(),
    }
}

fn boundary_review(
) -> Result<TassadarArticleTransformerBoundaryReview, TassadarArticleTransformerModelClosureReportError>
{
    let transformer_encoder_decoder_module = read_repo_file(TRANSFORMER_ENCODER_DECODER_MODULE_REF)?;
    let models_article_transformer_module = read_repo_file(MODELS_ARTICLE_TRANSFORMER_MODULE_REF)?;
    let models_executor_transformer_module =
        read_repo_file(MODELS_EXECUTOR_TRANSFORMER_MODULE_REF)?;

    let transformer_defines_encoder_layer =
        transformer_encoder_decoder_module.contains("pub struct TransformerEncoderLayer");
    let transformer_defines_decoder_layer =
        transformer_encoder_decoder_module.contains("pub struct TransformerDecoderLayer");
    let transformer_defines_encoder_decoder_transformer =
        transformer_encoder_decoder_module.contains("pub struct EncoderDecoderTransformer");
    let models_define_article_transformer =
        models_article_transformer_module.contains("pub struct TassadarArticleTransformer");
    let models_use_owned_encoder_decoder_transformer =
        models_article_transformer_module.contains("EncoderDecoderTransformer");
    let models_reference_attention_is_all_you_need =
        models_article_transformer_module.contains("Attention Is All You Need");
    let executor_scaffold_mentions_article_transformer =
        models_executor_transformer_module.contains("TassadarArticleTransformer");
    let passed = transformer_defines_encoder_layer
        && transformer_defines_decoder_layer
        && transformer_defines_encoder_decoder_transformer
        && models_define_article_transformer
        && models_use_owned_encoder_decoder_transformer
        && models_reference_attention_is_all_you_need
        && !executor_scaffold_mentions_article_transformer;
    let detail = if passed {
        String::from(
            "the owned encoder-decoder stack now lives in `psionic-transformer/src/encoder_decoder.rs`, the canonical article wrapper now lives in `psionic-models/src/tassadar_article_transformer.rs`, and the older executor scaffold remains a separate non-canonical lane",
        )
    } else {
        format!(
            "boundary review failed encoder_layer={} decoder_layer={} encoder_decoder_transformer={} article_transformer={} models_use_owned_stack={} references_source_paper={} executor_scaffold_mentions_article_transformer={}",
            transformer_defines_encoder_layer,
            transformer_defines_decoder_layer,
            transformer_defines_encoder_decoder_transformer,
            models_define_article_transformer,
            models_use_owned_encoder_decoder_transformer,
            models_reference_attention_is_all_you_need,
            executor_scaffold_mentions_article_transformer,
        )
    };

    Ok(TassadarArticleTransformerBoundaryReview {
        transformer_encoder_decoder_module_ref: String::from(
            TRANSFORMER_ENCODER_DECODER_MODULE_REF,
        ),
        models_article_transformer_module_ref: String::from(
            MODELS_ARTICLE_TRANSFORMER_MODULE_REF,
        ),
        models_executor_transformer_module_ref: String::from(
            MODELS_EXECUTOR_TRANSFORMER_MODULE_REF,
        ),
        transformer_defines_encoder_layer,
        transformer_defines_decoder_layer,
        transformer_defines_encoder_decoder_transformer,
        models_define_article_transformer,
        models_use_owned_encoder_decoder_transformer,
        models_reference_attention_is_all_you_need,
        executor_scaffold_mentions_article_transformer,
        passed,
        detail,
    })
}

fn read_repo_file(
    relative_path: &str,
) -> Result<String, TassadarArticleTransformerModelClosureReportError> {
    let path = repo_root().join(relative_path);
    fs::read_to_string(&path).map_err(|error| {
        TassadarArticleTransformerModelClosureReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })
}

pub fn tassadar_article_transformer_model_closure_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF)
}

pub fn write_tassadar_article_transformer_model_closure_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerModelClosureReport,
    TassadarArticleTransformerModelClosureReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerModelClosureReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_model_closure_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerModelClosureReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
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
) -> Result<T, TassadarArticleTransformerModelClosureReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarArticleTransformerModelClosureReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerModelClosureReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        boundary_review, build_report_from_inputs,
        build_tassadar_article_transformer_model_closure_report, case_rows, read_json,
        tassadar_article_transformer_model_closure_report_path,
        write_tassadar_article_transformer_model_closure_report,
        TassadarArticleTransformerModelClosureReport,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_canonical_transformer_stack_boundary_report,
    };

    #[test]
    fn article_transformer_model_closure_is_tied_and_blocked_until_later_work() {
        let report =
            build_tassadar_article_transformer_model_closure_report().expect("closure report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(
            report.acceptance_gate_tie.acceptance_status,
            crate::TassadarArticleEquivalenceAcceptanceStatus::Green
        );
        assert!(report.boundary_review.passed);
        assert!(report.article_transformer_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.case_rows.len(), 6);
        assert!(report.all_cases_pass);
    }

    #[test]
    fn missing_case_keeps_article_transformer_contract_red() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let boundary_report =
            build_tassadar_canonical_transformer_stack_boundary_report().expect("boundary");
        let mut rows = case_rows();
        rows.pop();

        let report = build_report_from_inputs(
            acceptance_gate_report,
            boundary_report,
            rows,
            boundary_review().expect("boundary review"),
        );

        assert!(!report.article_transformer_contract_green);
        assert!(!report.all_required_cases_present);
    }

    #[test]
    fn failed_boundary_review_keeps_article_transformer_contract_red() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let boundary_report =
            build_tassadar_canonical_transformer_stack_boundary_report().expect("boundary");
        let mut review = boundary_review().expect("boundary review");
        review.passed = false;

        let report = build_report_from_inputs(
            acceptance_gate_report,
            boundary_report,
            case_rows(),
            review,
        );

        assert!(!report.article_transformer_contract_green);
    }

    #[test]
    fn article_transformer_model_closure_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_model_closure_report().expect("closure report");
        let committed: TassadarArticleTransformerModelClosureReport =
            read_json(tassadar_article_transformer_model_closure_report_path())
                .expect("committed closure report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_transformer_model_closure_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_model_closure_report.json");
        let written = write_tassadar_article_transformer_model_closure_report(&output_path)
            .expect("write closure report");
        let persisted: TassadarArticleTransformerModelClosureReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_model_closure_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_model_closure_report.json")
        );
    }
}
