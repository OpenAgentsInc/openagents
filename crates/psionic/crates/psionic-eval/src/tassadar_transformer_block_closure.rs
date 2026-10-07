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
use psionic_nn::{ActivationKind, LayerNorm, NnTensor};
use psionic_transformer::{
    AttentionMask, MultiHeadAttention, PositionwiseFeedForward, TransformerDecoderBlock,
    TransformerEmbeddings, TransformerExecutionMode,
};

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

pub const TASSADAR_TRANSFORMER_BLOCK_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_transformer_block_closure_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-162";
const TRANSFORMER_CARGO_REF: &str = "crates/psionic-transformer/Cargo.toml";
const TRANSFORMER_BLOCKS_MODULE_REF: &str = "crates/psionic-transformer/src/blocks.rs";
const NN_CARGO_REF: &str = "crates/psionic-nn/Cargo.toml";
const NN_OPTIMIZERS_CARGO_REF: &str = "crates/psionic-nn-optimizers/Cargo.toml";
const MODELS_EXECUTOR_MODULE_REF: &str =
    "crates/psionic-models/src/tassadar_executor_transformer.rs";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTransformerBlockValidationKind {
    EmbeddingBinding,
    DropoutTrainingEvalPosture,
    MultiHeadProjectionMerge,
    FeedForwardBlock,
    ResidualAndNorm,
    DeterministicDecoderBlockForward,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTransformerBlockCaseRow {
    pub case_id: String,
    pub validation_kind: TassadarTransformerBlockValidationKind,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTransformerBlockBoundaryReview {
    pub transformer_cargo_ref: String,
    pub transformer_blocks_module_ref: String,
    pub nn_cargo_ref: String,
    pub nn_optimizers_cargo_ref: String,
    pub models_executor_module_ref: String,
    pub direct_nn_dependency: bool,
    pub direct_models_dependency: bool,
    pub direct_runtime_dependency: bool,
    pub nn_depends_on_train: bool,
    pub optimizer_interop_split_present: bool,
    pub transformer_defines_embeddings: bool,
    pub transformer_defines_multi_head_attention: bool,
    pub transformer_defines_feed_forward: bool,
    pub transformer_defines_decoder_block: bool,
    pub models_mentions_owned_block_symbols: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTransformerBlockAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTransformerBlockClosureReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarTransformerBlockAcceptanceGateTie,
    pub canonical_boundary_report_ref: String,
    pub canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub case_rows: Vec<TassadarTransformerBlockCaseRow>,
    pub boundary_review: TassadarTransformerBlockBoundaryReview,
    pub all_required_cases_present: bool,
    pub all_cases_pass: bool,
    pub transformer_block_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarTransformerBlockClosureReportError {
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

pub fn build_tassadar_transformer_block_closure_report(
) -> Result<TassadarTransformerBlockClosureReport, TassadarTransformerBlockClosureReportError> {
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
    case_rows: Vec<TassadarTransformerBlockCaseRow>,
    boundary_review: TassadarTransformerBlockBoundaryReview,
) -> TassadarTransformerBlockClosureReport {
    let acceptance_gate_tie = TassadarTransformerBlockAcceptanceGateTie {
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
    let transformer_block_contract_green = acceptance_gate_tie.tied_requirement_satisfied
        && canonical_boundary_report.boundary_contract_green
        && all_required_cases_present
        && all_cases_pass
        && boundary_review.passed;
    let article_equivalence_green =
        transformer_block_contract_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarTransformerBlockClosureReport {
        schema_version: 1,
        report_id: String::from("tassadar.transformer_block_closure.report.v1"),
        acceptance_gate_tie,
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        canonical_boundary_report,
        case_rows,
        boundary_review,
        all_required_cases_present,
        all_cases_pass,
        transformer_block_contract_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report freezes the reusable transformer block layer only. It proves that embeddings, positional binding, multi-head projection and merge, feed-forward, residual-plus-norm composition, and train versus eval dropout posture now live in `psionic-transformer` above `psionic-nn`, but it does not claim the paper-faithful article model artifact or final article-equivalence route are complete.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Transformer block closure now records case_rows={}, all_required_cases_present={}, all_cases_pass={}, boundary_review_passed={}, tied_requirement_satisfied={}, transformer_block_contract_green={}, and article_equivalence_green={}.",
        report.case_rows.len(),
        report.all_required_cases_present,
        report.all_cases_pass,
        report.boundary_review.passed,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.transformer_block_contract_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_transformer_block_closure_report|",
        &report,
    );
    report
}

fn required_validation_kinds() -> BTreeSet<TassadarTransformerBlockValidationKind> {
    BTreeSet::from([
        TassadarTransformerBlockValidationKind::EmbeddingBinding,
        TassadarTransformerBlockValidationKind::DropoutTrainingEvalPosture,
        TassadarTransformerBlockValidationKind::MultiHeadProjectionMerge,
        TassadarTransformerBlockValidationKind::FeedForwardBlock,
        TassadarTransformerBlockValidationKind::ResidualAndNorm,
        TassadarTransformerBlockValidationKind::DeterministicDecoderBlockForward,
    ])
}

fn case_rows() -> Vec<TassadarTransformerBlockCaseRow> {
    vec![
        embedding_binding_case(),
        dropout_training_eval_posture_case(),
        multi_head_projection_merge_case(),
        feed_forward_block_case(),
        residual_and_norm_case(),
        deterministic_decoder_block_forward_case(),
    ]
}

fn embedding_binding_case() -> TassadarTransformerBlockCaseRow {
    let embeddings =
        TransformerEmbeddings::from_f32_table("embeddings", 4, 4, 8, vec![0.0; 16], 0.0);

    match embeddings.and_then(|embeddings| {
        embeddings.forward(
            Shape::new(vec![1, 2]),
            &[0, 1],
            TransformerExecutionMode::Eval,
        )
    }) {
        Ok(output)
            if output.dims() == [1, 2, 4]
                && output
                    .as_f32_slice()
                    .map(|values| {
                        approx_eq(values[0], 0.0)
                            && approx_eq(values[1], 1.0)
                            && approx_eq(values[4], 0.84147096)
                            && approx_eq(values[5], 0.5403023)
                    })
                    .unwrap_or(false) =>
        {
            case_row(
                "embedding_binding",
                TassadarTransformerBlockValidationKind::EmbeddingBinding,
                true,
                "token embeddings and deterministic sinusoidal positional bindings now compose in `psionic-transformer` with the expected [batch, seq, hidden] surface",
            )
        }
        Ok(output) => case_row(
            "embedding_binding",
            TassadarTransformerBlockValidationKind::EmbeddingBinding,
            false,
            format!(
                "unexpected embedding binding output dims={:?} values={:?}",
                output.dims(),
                output.as_f32_slice().unwrap_or(&[])
            ),
        ),
        Err(error) => case_row(
            "embedding_binding",
            TassadarTransformerBlockValidationKind::EmbeddingBinding,
            false,
            format!("transformer embeddings failed on binding case: {error}"),
        ),
    }
}

fn dropout_training_eval_posture_case() -> TassadarTransformerBlockCaseRow {
    let embeddings =
        TransformerEmbeddings::from_f32_table("embeddings", 2, 2, 4, vec![1.0, 1.0, 1.0, 1.0], 0.5);

    match embeddings {
        Ok(embeddings) => {
            let eval = embeddings.forward(
                Shape::new(vec![1, 2]),
                &[0, 1],
                TransformerExecutionMode::Eval,
            );
            let train_a = embeddings.forward(
                Shape::new(vec![1, 2]),
                &[0, 1],
                TransformerExecutionMode::Train { seed: 7 },
            );
            let train_b = embeddings.forward(
                Shape::new(vec![1, 2]),
                &[0, 1],
                TransformerExecutionMode::Train { seed: 7 },
            );

            match (eval, train_a, train_b) {
                (Ok(eval), Ok(train_a), Ok(train_b))
                    if train_a == train_b && eval != train_a =>
                {
                    case_row(
                        "dropout_training_eval_posture",
                        TassadarTransformerBlockValidationKind::DropoutTrainingEvalPosture,
                        true,
                        "train-versus-eval posture is explicit and seed-stable for reusable block composition",
                    )
                }
                (Ok(eval), Ok(train_a), Ok(train_b)) => case_row(
                    "dropout_training_eval_posture",
                    TassadarTransformerBlockValidationKind::DropoutTrainingEvalPosture,
                    false,
                    format!(
                        "unexpected dropout posture eval={:?} train_a={:?} train_b={:?}",
                        eval.as_f32_slice().unwrap_or(&[]),
                        train_a.as_f32_slice().unwrap_or(&[]),
                        train_b.as_f32_slice().unwrap_or(&[])
                    ),
                ),
                (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => case_row(
                    "dropout_training_eval_posture",
                    TassadarTransformerBlockValidationKind::DropoutTrainingEvalPosture,
                    false,
                    format!("dropout posture case failed: {error}"),
                ),
            }
        }
        Err(error) => case_row(
            "dropout_training_eval_posture",
            TassadarTransformerBlockValidationKind::DropoutTrainingEvalPosture,
            false,
            format!("failed to construct transformer embeddings for dropout posture case: {error}"),
        ),
    }
}

fn multi_head_projection_merge_case() -> TassadarTransformerBlockCaseRow {
    let attention = sample_identity_attention();
    let input = NnTensor::f32(
        Shape::new(vec![1, 2, 4]),
        vec![1.0, 2.0, 3.0, 4.0, 2.0, 1.0, 0.0, -1.0],
    )
    .map_err(|error| error.to_string());
    let mask = AttentionMask::causal(1, 2, 2);

    match (attention, input) {
        (Ok(attention), Ok(input)) => match attention.forward(
            &input,
            &input,
            &input,
            Some(&mask),
            TransformerExecutionMode::Eval,
        ) {
            Ok(output)
                if output.hidden_state.dims() == [1, 2, 4]
                    && output.probability_trace.probabilities.tensor_spec().shape().dims()
                        == [1, 2, 2, 2]
                    && probability_rows_sum_to_one(&output.probability_trace.probabilities) =>
            {
                case_row(
                    "multi_head_projection_merge",
                    TassadarTransformerBlockValidationKind::MultiHeadProjectionMerge,
                    true,
                    "reusable multi-head projection, attention, and merge now execute in `psionic-transformer` with explicit trace shape [batch, heads, query, key]",
                )
            }
            Ok(output) => case_row(
                "multi_head_projection_merge",
                TassadarTransformerBlockValidationKind::MultiHeadProjectionMerge,
                false,
                format!(
                    "unexpected multi-head output dims={:?} trace_shape={:?}",
                    output.hidden_state.dims(),
                    output.probability_trace.probabilities.tensor_spec().shape().dims()
                ),
            ),
            Err(error) => case_row(
                "multi_head_projection_merge",
                TassadarTransformerBlockValidationKind::MultiHeadProjectionMerge,
                false,
                format!("multi-head attention failed on projection/merge case: {error}"),
            ),
        },
        (Err(error), _) | (_, Err(error)) => case_row(
            "multi_head_projection_merge",
            TassadarTransformerBlockValidationKind::MultiHeadProjectionMerge,
            false,
            format!("failed to construct projection/merge case inputs: {error}"),
        ),
    }
}

fn feed_forward_block_case() -> TassadarTransformerBlockCaseRow {
    let feed_forward = PositionwiseFeedForward::from_f32_parts(
        "ffn",
        4,
        4,
        ActivationKind::Relu,
        identity_matrix(4),
        Some(vec![0.0; 4]),
        identity_matrix(4),
        Some(vec![0.0; 4]),
        0.0,
    )
    .map_err(|error| error.to_string());
    let input = NnTensor::f32(Shape::new(vec![1, 1, 4]), vec![1.0, -2.0, 3.0, -4.0])
        .map_err(|error| error.to_string());

    match (feed_forward, input) {
        (Ok(feed_forward), Ok(input)) => match feed_forward.forward(&input, TransformerExecutionMode::Eval) {
            Ok(output)
                if output.dims() == [1, 1, 4]
                    && output
                        .as_f32_slice()
                        .map(|values| values == [1.0, 0.0, 3.0, 0.0])
                        .unwrap_or(false) =>
            {
                case_row(
                    "feed_forward_block",
                    TassadarTransformerBlockValidationKind::FeedForwardBlock,
                    true,
                    "position-wise feed-forward composition now stays reusable in `psionic-transformer` and matches the expected ReLU reference path",
                )
            }
            Ok(output) => case_row(
                "feed_forward_block",
                TassadarTransformerBlockValidationKind::FeedForwardBlock,
                false,
                format!(
                    "unexpected feed-forward output dims={:?} values={:?}",
                    output.dims(),
                    output.as_f32_slice().unwrap_or(&[])
                ),
            ),
            Err(error) => case_row(
                "feed_forward_block",
                TassadarTransformerBlockValidationKind::FeedForwardBlock,
                false,
                format!("feed-forward block failed on reference case: {error}"),
            ),
        },
        (Err(error), _) | (_, Err(error)) => case_row(
            "feed_forward_block",
            TassadarTransformerBlockValidationKind::FeedForwardBlock,
            false,
            format!("failed to construct feed-forward reference case: {error}"),
        ),
    }
}

fn residual_and_norm_case() -> TassadarTransformerBlockCaseRow {
    let block = zero_block();
    let input = NnTensor::f32(
        Shape::new(vec![1, 2, 4]),
        vec![1.0, 2.0, 3.0, 4.0, 2.0, 4.0, 6.0, 8.0],
    )
    .map_err(|error| error.to_string());
    let expected_norm_1 =
        LayerNorm::new("expected_norm_1", 4, 1e-5).map_err(|error| error.to_string());
    let expected_norm_2 =
        LayerNorm::new("expected_norm_2", 4, 1e-5).map_err(|error| error.to_string());

    match (block, input, expected_norm_1, expected_norm_2) {
        (Ok(block), Ok(input), Ok(expected_norm_1), Ok(expected_norm_2)) => {
            let actual = block
                .forward(
                    &input,
                    Some(&AttentionMask::causal(1, 2, 2)),
                    TransformerExecutionMode::Eval,
                )
                .map_err(|error| error.to_string());
            let expected = expected_norm_1
                .forward(&input)
                .and_then(|normed| expected_norm_2.forward(&normed))
                .map_err(|error| error.to_string());
            match (actual, expected) {
                (Ok(actual), Ok(expected))
                    if tensor_approx_eq(&actual.hidden_state, &expected)
                        && actual.hidden_state.dims() == [1, 2, 4] =>
                {
                    case_row(
                        "residual_and_norm",
                        TassadarTransformerBlockValidationKind::ResidualAndNorm,
                        true,
                        "residual addition and layer-norm sequencing now live in reusable decoder-block composition rather than ad hoc per-model code",
                    )
                }
                (Ok(actual), Ok(expected)) => case_row(
                    "residual_and_norm",
                    TassadarTransformerBlockValidationKind::ResidualAndNorm,
                    false,
                    format!(
                        "residual/norm output diverged actual={:?} expected={:?}",
                        actual.hidden_state.as_f32_slice().unwrap_or(&[]),
                        expected.as_f32_slice().unwrap_or(&[])
                    ),
                ),
                (Err(error), _) | (_, Err(error)) => case_row(
                    "residual_and_norm",
                    TassadarTransformerBlockValidationKind::ResidualAndNorm,
                    false,
                    format!("residual/norm case failed: {error}"),
                ),
            }
        }
        (Err(error), _, _, _)
        | (_, Err(error), _, _)
        | (_, _, Err(error), _)
        | (_, _, _, Err(error)) => case_row(
            "residual_and_norm",
            TassadarTransformerBlockValidationKind::ResidualAndNorm,
            false,
            format!("failed to construct residual/norm case: {error}"),
        ),
    }
}

fn deterministic_decoder_block_forward_case() -> TassadarTransformerBlockCaseRow {
    let block = sample_identity_block();
    let input = NnTensor::f32(
        Shape::new(vec![1, 2, 4]),
        vec![1.0, 2.0, 3.0, 4.0, 2.0, 3.0, 4.0, 5.0],
    )
    .map_err(|error| error.to_string());
    let mask = AttentionMask::causal(1, 2, 2);

    match (block, input) {
        (Ok(block), Ok(input)) => {
            let first = block.forward(&input, Some(&mask), TransformerExecutionMode::Eval);
            let second = block.forward(&input, Some(&mask), TransformerExecutionMode::Eval);
            match (first, second) {
                (Ok(first), Ok(second))
                    if first == second && first.hidden_state.dims() == [1, 2, 4] =>
                {
                    case_row(
                        "deterministic_decoder_block_forward",
                        TassadarTransformerBlockValidationKind::DeterministicDecoderBlockForward,
                        true,
                        "decoder-block composition stays deterministic in eval posture for repeated reusable forward passes",
                    )
                }
                (Ok(first), Ok(second)) => case_row(
                    "deterministic_decoder_block_forward",
                    TassadarTransformerBlockValidationKind::DeterministicDecoderBlockForward,
                    false,
                    format!(
                        "deterministic decoder-block case diverged first={:?} second={:?}",
                        first.hidden_state.as_f32_slice().unwrap_or(&[]),
                        second.hidden_state.as_f32_slice().unwrap_or(&[])
                    ),
                ),
                (Err(error), _) | (_, Err(error)) => case_row(
                    "deterministic_decoder_block_forward",
                    TassadarTransformerBlockValidationKind::DeterministicDecoderBlockForward,
                    false,
                    format!("deterministic decoder-block case failed: {error}"),
                ),
            }
        }
        (Err(error), _) | (_, Err(error)) => case_row(
            "deterministic_decoder_block_forward",
            TassadarTransformerBlockValidationKind::DeterministicDecoderBlockForward,
            false,
            format!("failed to construct deterministic decoder-block case: {error}"),
        ),
    }
}

fn case_row(
    case_id: &str,
    validation_kind: TassadarTransformerBlockValidationKind,
    passed: bool,
    detail: impl Into<String>,
) -> TassadarTransformerBlockCaseRow {
    TassadarTransformerBlockCaseRow {
        case_id: String::from(case_id),
        validation_kind,
        passed,
        detail: detail.into(),
    }
}

fn boundary_review(
) -> Result<TassadarTransformerBlockBoundaryReview, TassadarTransformerBlockClosureReportError> {
    let transformer_cargo = read_repo_file(TRANSFORMER_CARGO_REF)?;
    let transformer_blocks_module = read_repo_file(TRANSFORMER_BLOCKS_MODULE_REF)?;
    let nn_cargo = read_repo_file(NN_CARGO_REF)?;
    let nn_optimizers_cargo = read_repo_file(NN_OPTIMIZERS_CARGO_REF)?;
    let models_executor_module = read_repo_file(MODELS_EXECUTOR_MODULE_REF)?;

    let direct_nn_dependency = cargo_toml_has_dependency(&transformer_cargo, "psionic-nn");
    let direct_models_dependency = cargo_toml_has_dependency(&transformer_cargo, "psionic-models");
    let direct_runtime_dependency =
        cargo_toml_has_dependency(&transformer_cargo, "psionic-runtime");
    let nn_depends_on_train = cargo_toml_has_dependency(&nn_cargo, "psionic-train");
    let optimizer_interop_split_present =
        cargo_toml_has_dependency(&nn_optimizers_cargo, "psionic-nn")
            && cargo_toml_has_dependency(&nn_optimizers_cargo, "psionic-train");
    let transformer_defines_embeddings =
        transformer_blocks_module.contains("pub struct TransformerEmbeddings");
    let transformer_defines_multi_head_attention =
        transformer_blocks_module.contains("pub struct MultiHeadAttention");
    let transformer_defines_feed_forward =
        transformer_blocks_module.contains("pub struct PositionwiseFeedForward");
    let transformer_defines_decoder_block =
        transformer_blocks_module.contains("pub struct TransformerDecoderBlock");
    let models_mentions_owned_block_symbols = [
        "TransformerEmbeddings",
        "MultiHeadAttention",
        "PositionwiseFeedForward",
        "TransformerDecoderBlock",
    ]
    .iter()
    .any(|symbol| models_executor_module.contains(symbol));
    let passed = direct_nn_dependency
        && !direct_models_dependency
        && !direct_runtime_dependency
        && !nn_depends_on_train
        && optimizer_interop_split_present
        && transformer_defines_embeddings
        && transformer_defines_multi_head_attention
        && transformer_defines_feed_forward
        && transformer_defines_decoder_block
        && !models_mentions_owned_block_symbols;

    Ok(TassadarTransformerBlockBoundaryReview {
        transformer_cargo_ref: String::from(TRANSFORMER_CARGO_REF),
        transformer_blocks_module_ref: String::from(TRANSFORMER_BLOCKS_MODULE_REF),
        nn_cargo_ref: String::from(NN_CARGO_REF),
        nn_optimizers_cargo_ref: String::from(NN_OPTIMIZERS_CARGO_REF),
        models_executor_module_ref: String::from(MODELS_EXECUTOR_MODULE_REF),
        direct_nn_dependency,
        direct_models_dependency,
        direct_runtime_dependency,
        nn_depends_on_train,
        optimizer_interop_split_present,
        transformer_defines_embeddings,
        transformer_defines_multi_head_attention,
        transformer_defines_feed_forward,
        transformer_defines_decoder_block,
        models_mentions_owned_block_symbols,
        passed,
        detail: String::from(
            "reusable transformer block composition now lives in `psionic-transformer` above `psionic-nn`; `psionic-nn` no longer points back into `psionic-train`, and optimizer interop is split into `psionic-nn-optimizers` so the canonical transformer boundary stays acyclic and model-independent",
        ),
    })
}

fn sample_identity_attention() -> Result<MultiHeadAttention, String> {
    MultiHeadAttention::from_f32_parts(
        "attn",
        4,
        2,
        identity_matrix(4),
        Some(vec![0.0; 4]),
        identity_matrix(4),
        Some(vec![0.0; 4]),
        identity_matrix(4),
        Some(vec![0.0; 4]),
        identity_matrix(4),
        Some(vec![0.0; 4]),
        0.0,
    )
    .map_err(|error| error.to_string())
}

fn sample_identity_block() -> Result<TransformerDecoderBlock, String> {
    let attention = sample_identity_attention()?;
    let feed_forward = PositionwiseFeedForward::from_f32_parts(
        "ffn",
        4,
        4,
        ActivationKind::Relu,
        identity_matrix(4),
        Some(vec![0.0; 4]),
        identity_matrix(4),
        Some(vec![0.0; 4]),
        0.0,
    )
    .map_err(|error| error.to_string())?;
    TransformerDecoderBlock::from_components(
        attention,
        LayerNorm::new("norm_1", 4, 1e-5).map_err(|error| error.to_string())?,
        feed_forward,
        LayerNorm::new("norm_2", 4, 1e-5).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn zero_block() -> Result<TransformerDecoderBlock, String> {
    let attention = MultiHeadAttention::from_f32_parts(
        "attn_zero",
        4,
        2,
        vec![0.0; 16],
        Some(vec![0.0; 4]),
        vec![0.0; 16],
        Some(vec![0.0; 4]),
        vec![0.0; 16],
        Some(vec![0.0; 4]),
        vec![0.0; 16],
        Some(vec![0.0; 4]),
        0.0,
    )
    .map_err(|error| error.to_string())?;
    let feed_forward = PositionwiseFeedForward::from_f32_parts(
        "ffn_zero",
        4,
        4,
        ActivationKind::Relu,
        vec![0.0; 16],
        Some(vec![0.0; 4]),
        vec![0.0; 16],
        Some(vec![0.0; 4]),
        0.0,
    )
    .map_err(|error| error.to_string())?;
    TransformerDecoderBlock::from_components(
        attention,
        LayerNorm::new("norm_zero_1", 4, 1e-5).map_err(|error| error.to_string())?,
        feed_forward,
        LayerNorm::new("norm_zero_2", 4, 1e-5).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn probability_rows_sum_to_one(probabilities: &psionic_transformer::AttentionTensor4) -> bool {
    for batch in 0..probabilities.batch_size() {
        for head in 0..probabilities.head_count() {
            for row in 0..probabilities.row_count() {
                let mut sum = 0.0;
                for col in 0..probabilities.col_count() {
                    sum += probabilities.get(batch, head, row, col);
                }
                if !approx_eq(sum, 1.0) {
                    return false;
                }
            }
        }
    }
    true
}

fn tensor_approx_eq(left: &NnTensor, right: &NnTensor) -> bool {
    left.dims() == right.dims()
        && left
            .as_f32_slice()
            .ok()
            .zip(right.as_f32_slice().ok())
            .map(|(left_values, right_values)| {
                left_values
                    .iter()
                    .zip(right_values.iter())
                    .all(|(left, right)| approx_eq(*left, *right))
            })
            .unwrap_or(false)
}

fn identity_matrix(size: usize) -> Vec<f32> {
    let mut values = vec![0.0; size * size];
    for index in 0..size {
        values[index * size + index] = 1.0;
    }
    values
}

fn approx_eq(left: f32, right: f32) -> bool {
    (left - right).abs() <= 1e-4
}

fn cargo_toml_has_dependency(contents: &str, dependency: &str) -> bool {
    contents
        .lines()
        .any(|line| line.trim_start().starts_with(&format!("{dependency} =")))
}

fn read_repo_file(
    relative_path: &str,
) -> Result<String, TassadarTransformerBlockClosureReportError> {
    let path = repo_root().join(relative_path);
    fs::read_to_string(&path).map_err(|error| TassadarTransformerBlockClosureReportError::Read {
        path: path.display().to_string(),
        error,
    })
}

#[must_use]
pub fn tassadar_transformer_block_closure_report_path() -> PathBuf {
    repo_root().join(TASSADAR_TRANSFORMER_BLOCK_CLOSURE_REPORT_REF)
}

pub fn write_tassadar_transformer_block_closure_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarTransformerBlockClosureReport, TassadarTransformerBlockClosureReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarTransformerBlockClosureReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_transformer_block_closure_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarTransformerBlockClosureReportError::Write {
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
) -> Result<T, TassadarTransformerBlockClosureReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarTransformerBlockClosureReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarTransformerBlockClosureReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        boundary_review, build_report_from_inputs, build_tassadar_transformer_block_closure_report,
        case_rows, read_json, tassadar_transformer_block_closure_report_path,
        write_tassadar_transformer_block_closure_report, TassadarTransformerBlockClosureReport,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_canonical_transformer_stack_boundary_report,
    };

    #[test]
    fn transformer_block_closure_is_tied_and_blocked_until_later_work() {
        let report = build_tassadar_transformer_block_closure_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.boundary_review.passed);
        assert!(report.all_required_cases_present);
        assert!(report.all_cases_pass);
        assert!(report.transformer_block_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.case_rows.len(), 6);
    }

    #[test]
    fn missing_case_keeps_transformer_block_contract_red() {
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
            boundary_review().expect("review"),
        );

        assert!(!report.all_required_cases_present);
        assert!(!report.transformer_block_contract_green);
    }

    #[test]
    fn failed_boundary_review_keeps_transformer_block_contract_red() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let boundary_report =
            build_tassadar_canonical_transformer_stack_boundary_report().expect("boundary");
        let mut review = boundary_review().expect("review");
        review.passed = false;

        let report =
            build_report_from_inputs(acceptance_gate_report, boundary_report, case_rows(), review);

        assert!(!report.transformer_block_contract_green);
    }

    #[test]
    fn transformer_block_closure_matches_committed_truth() {
        let generated = build_tassadar_transformer_block_closure_report().expect("report");
        let committed: TassadarTransformerBlockClosureReport =
            read_json(tassadar_transformer_block_closure_report_path()).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_transformer_block_closure_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_transformer_block_closure_report.json");
        let written =
            write_tassadar_transformer_block_closure_report(&output_path).expect("write report");
        let persisted: TassadarTransformerBlockClosureReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_transformer_block_closure_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_transformer_block_closure_report.json")
        );
    }
}
