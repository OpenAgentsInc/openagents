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

use psionic_transformer::{
    scaled_dot_product_attention, AttentionMask, AttentionTensor4, ScaledDotProductAttentionOutput,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_canonical_transformer_stack_boundary_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus,
    TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_ATTENTION_PRIMITIVE_MASK_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_attention_primitive_mask_closure_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-161";
const TRANSFORMER_CARGO_REF: &str = "crates/psionic-transformer/Cargo.toml";
const TRANSFORMER_ATTENTION_MODULE_REF: &str = "crates/psionic-transformer/src/attention.rs";
const MODELS_EXECUTOR_MODULE_REF: &str = "crates/psionic-models/src/tassadar_executor_transformer.rs";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAttentionPrimitiveValidationKind {
    StableSoftmax,
    CausalMask,
    PaddingMask,
    CombinedMask,
    DeterministicForward,
    ProbabilityTraceExport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAttentionPrimitiveCaseRow {
    pub case_id: String,
    pub validation_kind: TassadarAttentionPrimitiveValidationKind,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAttentionPrimitiveBoundaryReview {
    pub transformer_cargo_ref: String,
    pub transformer_attention_module_ref: String,
    pub models_executor_module_ref: String,
    pub direct_core_dependency: bool,
    pub direct_array_dependency: bool,
    pub direct_models_dependency: bool,
    pub direct_runtime_dependency: bool,
    pub transformer_defines_owned_primitive: bool,
    pub models_mentions_owned_symbol: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAttentionPrimitiveAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAttentionPrimitiveMaskClosureReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarAttentionPrimitiveAcceptanceGateTie,
    pub canonical_boundary_report_ref: String,
    pub canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub case_rows: Vec<TassadarAttentionPrimitiveCaseRow>,
    pub boundary_review: TassadarAttentionPrimitiveBoundaryReview,
    pub all_required_cases_present: bool,
    pub all_cases_pass: bool,
    pub attention_primitive_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarAttentionPrimitiveMaskClosureReportError {
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

pub fn build_tassadar_attention_primitive_mask_closure_report() -> Result<
    TassadarAttentionPrimitiveMaskClosureReport,
    TassadarAttentionPrimitiveMaskClosureReportError,
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
    case_rows: Vec<TassadarAttentionPrimitiveCaseRow>,
    boundary_review: TassadarAttentionPrimitiveBoundaryReview,
) -> TassadarAttentionPrimitiveMaskClosureReport {
    let acceptance_gate_tie = TassadarAttentionPrimitiveAcceptanceGateTie {
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
    let attention_primitive_contract_green = acceptance_gate_tie.tied_requirement_satisfied
        && canonical_boundary_report.boundary_contract_green
        && all_required_cases_present
        && all_cases_pass
        && boundary_review.passed;
    let article_equivalence_green =
        attention_primitive_contract_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarAttentionPrimitiveMaskClosureReport {
        schema_version: 1,
        report_id: String::from("tassadar.attention_primitive_mask_closure.report.v1"),
        acceptance_gate_tie,
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        canonical_boundary_report,
        case_rows,
        boundary_review,
        all_required_cases_present,
        all_cases_pass,
        attention_primitive_contract_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report freezes the owned scaled dot-product attention and masking primitive only. It proves that reusable attention math, mask composition, deterministic trace export, and crate ownership now live in `psionic-transformer`, but it does not claim full article-equivalent Transformer closure.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Attention primitive closure now records case_rows={}, all_required_cases_present={}, all_cases_pass={}, boundary_review_passed={}, tied_requirement_satisfied={}, attention_primitive_contract_green={}, and article_equivalence_green={}.",
        report.case_rows.len(),
        report.all_required_cases_present,
        report.all_cases_pass,
        report.boundary_review.passed,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.attention_primitive_contract_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_attention_primitive_mask_closure_report|",
        &report,
    );
    report
}

fn required_validation_kinds() -> BTreeSet<TassadarAttentionPrimitiveValidationKind> {
    BTreeSet::from([
        TassadarAttentionPrimitiveValidationKind::StableSoftmax,
        TassadarAttentionPrimitiveValidationKind::CausalMask,
        TassadarAttentionPrimitiveValidationKind::PaddingMask,
        TassadarAttentionPrimitiveValidationKind::CombinedMask,
        TassadarAttentionPrimitiveValidationKind::DeterministicForward,
        TassadarAttentionPrimitiveValidationKind::ProbabilityTraceExport,
    ])
}

fn case_rows() -> Vec<TassadarAttentionPrimitiveCaseRow> {
    vec![
        stable_softmax_case(),
        causal_mask_case(),
        padding_mask_case(),
        combined_mask_case(),
        deterministic_forward_case(),
        probability_trace_export_case(),
    ]
}

fn stable_softmax_case() -> TassadarAttentionPrimitiveCaseRow {
    let query = AttentionTensor4::from_nested(vec![vec![vec![vec![1000.0, 0.0]]]]);
    let key = AttentionTensor4::from_nested(vec![vec![vec![
        vec![1000.0, 0.0],
        vec![-1000.0, 0.0],
    ]]]);
    let value = AttentionTensor4::from_nested(vec![vec![vec![
        vec![5.0, 1.0],
        vec![1.0, 7.0],
    ]]]);

    match run_attention_case(query, key, value, None) {
        Ok(output)
            if output
                .probability_trace
                .probabilities
                .values()
                .iter()
                .all(|value| value.is_finite())
                && approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 0), 1.0)
                && approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 1), 0.0) =>
        {
            case_row(
                "stable_softmax",
                TassadarAttentionPrimitiveValidationKind::StableSoftmax,
                true,
                "stable softmax stayed finite under extreme logits and produced the expected [1, 0] probability split",
            )
        }
        Ok(output) => case_row(
            "stable_softmax",
            TassadarAttentionPrimitiveValidationKind::StableSoftmax,
            false,
            format!(
                "unexpected stable-softmax output probabilities={:?}",
                output.probability_trace.probabilities.values()
            ),
        ),
        Err(error) => case_row(
            "stable_softmax",
            TassadarAttentionPrimitiveValidationKind::StableSoftmax,
            false,
            format!("attention primitive failed on stable-softmax case: {error}"),
        ),
    }
}

fn causal_mask_case() -> TassadarAttentionPrimitiveCaseRow {
    let query = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![1.0]]]]);
    let key = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![2.0]]]]);
    let value = AttentionTensor4::from_nested(vec![vec![vec![vec![10.0], vec![20.0]]]]);
    let mask = Some(AttentionMask::causal(1, 2, 2));

    match run_attention_case(query, key, value, mask) {
        Ok(output)
            if approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 0), 1.0)
                && approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 1), 0.0)
                && approx_eq(output.context.get(0, 0, 0, 0), 10.0) =>
        {
            case_row(
                "causal_mask",
                TassadarAttentionPrimitiveValidationKind::CausalMask,
                true,
                "causal masking zeroed future attention while preserving the deterministic prefix row",
            )
        }
        Ok(output) => case_row(
            "causal_mask",
            TassadarAttentionPrimitiveValidationKind::CausalMask,
            false,
            format!(
                "unexpected causal-mask output probabilities={:?} context={:?}",
                output.probability_trace.probabilities.values(),
                output.context.values()
            ),
        ),
        Err(error) => case_row(
            "causal_mask",
            TassadarAttentionPrimitiveValidationKind::CausalMask,
            false,
            format!("attention primitive failed on causal-mask case: {error}"),
        ),
    }
}

fn padding_mask_case() -> TassadarAttentionPrimitiveCaseRow {
    let query = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![1.0]]]]);
    let key = AttentionTensor4::from_nested(vec![vec![vec![
        vec![1.0],
        vec![100.0],
        vec![2.0],
    ]]]);
    let value = AttentionTensor4::from_nested(vec![vec![vec![
        vec![10.0],
        vec![999.0],
        vec![30.0],
    ]]]);
    let mask = AttentionMask::from_padding_tokens(vec![vec![true, false, true]], 2).ok();

    match run_attention_case(query, key, value, mask) {
        Ok(output)
            if approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 1), 0.0)
                && approx_eq(output.probability_trace.probabilities.get(0, 0, 1, 1), 0.0) =>
        {
            case_row(
                "padding_mask",
                TassadarAttentionPrimitiveValidationKind::PaddingMask,
                true,
                "padding masking zeroed padded keys and kept the masked-out high-score column from dominating the output",
            )
        }
        Ok(output) => case_row(
            "padding_mask",
            TassadarAttentionPrimitiveValidationKind::PaddingMask,
            false,
            format!(
                "unexpected padding-mask output probabilities={:?}",
                output.probability_trace.probabilities.values()
            ),
        ),
        Err(error) => case_row(
            "padding_mask",
            TassadarAttentionPrimitiveValidationKind::PaddingMask,
            false,
            format!("attention primitive failed on padding-mask case: {error}"),
        ),
    }
}

fn combined_mask_case() -> TassadarAttentionPrimitiveCaseRow {
    let query = AttentionTensor4::from_nested(vec![vec![vec![
        vec![1.0],
        vec![1.0],
        vec![1.0],
    ]]]);
    let key = AttentionTensor4::from_nested(vec![vec![vec![
        vec![1.0],
        vec![2.0],
        vec![3.0],
    ]]]);
    let value = AttentionTensor4::from_nested(vec![vec![vec![
        vec![10.0],
        vec![20.0],
        vec![999.0],
    ]]]);
    let mask = AttentionMask::from_padding_tokens(vec![vec![true, true, false]], 3)
        .and_then(|padding| AttentionMask::causal(1, 3, 3).combine(&padding))
        .ok();

    match run_attention_case(query, key, value, mask) {
        Ok(output)
            if approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 1), 0.0)
                && approx_eq(output.probability_trace.probabilities.get(0, 0, 0, 2), 0.0)
                && approx_eq(output.probability_trace.probabilities.get(0, 0, 1, 2), 0.0)
                && approx_eq(output.probability_trace.probabilities.get(0, 0, 2, 2), 0.0) =>
        {
            case_row(
                "combined_mask",
                TassadarAttentionPrimitiveValidationKind::CombinedMask,
                true,
                "combined masking enforced both causal and padding boundaries in one reusable path",
            )
        }
        Ok(output) => case_row(
            "combined_mask",
            TassadarAttentionPrimitiveValidationKind::CombinedMask,
            false,
            format!(
                "unexpected combined-mask output probabilities={:?}",
                output.probability_trace.probabilities.values()
            ),
        ),
        Err(error) => case_row(
            "combined_mask",
            TassadarAttentionPrimitiveValidationKind::CombinedMask,
            false,
            format!("attention primitive failed on combined-mask case: {error}"),
        ),
    }
}

fn deterministic_forward_case() -> TassadarAttentionPrimitiveCaseRow {
    let query = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![1.0]]]]);
    let key = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![2.0]]]]);
    let value = AttentionTensor4::from_nested(vec![vec![vec![vec![10.0], vec![20.0]]]]);
    let mask = Some(AttentionMask::causal(1, 2, 2));

    match (
        run_attention_case(query.clone(), key.clone(), value.clone(), mask.clone()),
        run_attention_case(query, key, value, mask),
    ) {
        (Ok(first), Ok(second))
            if first.context == second.context
                && first.probability_trace.probabilities
                    == second.probability_trace.probabilities =>
        {
            case_row(
                "deterministic_forward",
                TassadarAttentionPrimitiveValidationKind::DeterministicForward,
                true,
                "the owned attention primitive produced byte-stable repeated outputs for the same inputs and mask",
            )
        }
        (Ok(first), Ok(second)) => case_row(
            "deterministic_forward",
            TassadarAttentionPrimitiveValidationKind::DeterministicForward,
            false,
            format!(
                "repeated attention passes diverged: first_context={:?} second_context={:?} first_probabilities={:?} second_probabilities={:?}",
                first.context.values(),
                second.context.values(),
                first.probability_trace.probabilities.values(),
                second.probability_trace.probabilities.values(),
            ),
        ),
        (Err(error), _) | (_, Err(error)) => case_row(
            "deterministic_forward",
            TassadarAttentionPrimitiveValidationKind::DeterministicForward,
            false,
            format!("attention primitive failed on deterministic-forward case: {error}"),
        ),
    }
}

fn probability_trace_export_case() -> TassadarAttentionPrimitiveCaseRow {
    let query = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![0.0]]]]);
    let key = AttentionTensor4::from_nested(vec![vec![vec![vec![1.0], vec![0.0]]]]);
    let value = AttentionTensor4::from_nested(vec![vec![vec![vec![3.0], vec![4.0]]]]);

    match run_attention_case(query, key, value, None) {
        Ok(output) => {
            let spec = output.probability_trace.tensor_spec();
            let data = output.probability_trace.tensor_data();
            match data.as_f32_slice() {
                Some(values)
                    if spec.shape().dims() == [1, 1, 2, 2]
                        && format!("{:?}", spec.dtype()) == "F32"
                        && format!("{:?}", spec.device().kind()) == "Cpu"
                        && values.len() == 4
                        && approx_eq(values[0] + values[1], 1.0)
                        && approx_eq(values[2] + values[3], 1.0) =>
                {
                    case_row(
                        "probability_trace_export",
                        TassadarAttentionPrimitiveValidationKind::ProbabilityTraceExport,
                        true,
                        "attention probability traces now export through psionic-core tensor spec and dense f32 tensor data",
                    )
                }
                Some(values) => case_row(
                    "probability_trace_export",
                    TassadarAttentionPrimitiveValidationKind::ProbabilityTraceExport,
                    false,
                    format!(
                        "unexpected probability-trace export spec_shape={:?} values={values:?}",
                        spec.shape().dims()
                    ),
                ),
                None => case_row(
                    "probability_trace_export",
                    TassadarAttentionPrimitiveValidationKind::ProbabilityTraceExport,
                    false,
                    "probability trace did not export dense f32 tensor data".to_string(),
                ),
            }
        }
        Err(error) => case_row(
            "probability_trace_export",
            TassadarAttentionPrimitiveValidationKind::ProbabilityTraceExport,
            false,
            format!("attention primitive failed on probability-trace export case: {error}"),
        ),
    }
}

fn case_row(
    case_id: &str,
    validation_kind: TassadarAttentionPrimitiveValidationKind,
    passed: bool,
    detail: impl Into<String>,
) -> TassadarAttentionPrimitiveCaseRow {
    TassadarAttentionPrimitiveCaseRow {
        case_id: String::from(case_id),
        validation_kind,
        passed,
        detail: detail.into(),
    }
}

fn run_attention_case(
    query: Result<AttentionTensor4, psionic_transformer::AttentionTensorError>,
    key: Result<AttentionTensor4, psionic_transformer::AttentionTensorError>,
    value: Result<AttentionTensor4, psionic_transformer::AttentionTensorError>,
    mask: Option<AttentionMask>,
) -> Result<ScaledDotProductAttentionOutput, String> {
    let query = query.map_err(|error| format!("query construction failed: {error}"))?;
    let key = key.map_err(|error| format!("key construction failed: {error}"))?;
    let value = value.map_err(|error| format!("value construction failed: {error}"))?;
    scaled_dot_product_attention(&query, &key, &value, mask.as_ref())
        .map_err(|error| error.to_string())
}

fn approx_eq(left: f32, right: f32) -> bool {
    (left - right).abs() <= 1e-4
}

fn boundary_review() -> Result<
    TassadarAttentionPrimitiveBoundaryReview,
    TassadarAttentionPrimitiveMaskClosureReportError,
> {
    let transformer_cargo = read_repo_file(TRANSFORMER_CARGO_REF)?;
    let transformer_attention_module = read_repo_file(TRANSFORMER_ATTENTION_MODULE_REF)?;
    let models_executor_module = read_repo_file(MODELS_EXECUTOR_MODULE_REF)?;

    let direct_core_dependency = cargo_toml_has_dependency(&transformer_cargo, "psionic-core");
    let direct_array_dependency = cargo_toml_has_dependency(&transformer_cargo, "psionic-array");
    let direct_models_dependency = cargo_toml_has_dependency(&transformer_cargo, "psionic-models");
    let direct_runtime_dependency = cargo_toml_has_dependency(&transformer_cargo, "psionic-runtime");
    let transformer_defines_owned_primitive =
        transformer_attention_module.contains("pub fn scaled_dot_product_attention");
    let models_mentions_owned_symbol =
        models_executor_module.contains("scaled_dot_product_attention");
    let passed = direct_core_dependency
        && direct_array_dependency
        && !direct_models_dependency
        && !direct_runtime_dependency
        && transformer_defines_owned_primitive
        && !models_mentions_owned_symbol;

    Ok(TassadarAttentionPrimitiveBoundaryReview {
        transformer_cargo_ref: String::from(TRANSFORMER_CARGO_REF),
        transformer_attention_module_ref: String::from(TRANSFORMER_ATTENTION_MODULE_REF),
        models_executor_module_ref: String::from(MODELS_EXECUTOR_MODULE_REF),
        direct_core_dependency,
        direct_array_dependency,
        direct_models_dependency,
        direct_runtime_dependency,
        transformer_defines_owned_primitive,
        models_mentions_owned_symbol,
        passed,
        detail: String::from(
            "the reusable owned attention primitive now lives in `psionic-transformer`, depends directly on `psionic-core` and `psionic-array`, and does not drift back into `psionic-models` or direct runtime ownership",
        ),
    })
}

fn cargo_toml_has_dependency(contents: &str, dependency: &str) -> bool {
    contents
        .lines()
        .any(|line| line.trim_start().starts_with(&format!("{dependency} =")))
}

fn read_repo_file(
    relative_path: &str,
) -> Result<String, TassadarAttentionPrimitiveMaskClosureReportError> {
    let path = repo_root().join(relative_path);
    fs::read_to_string(&path).map_err(|error| {
        TassadarAttentionPrimitiveMaskClosureReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })
}

#[must_use]
pub fn tassadar_attention_primitive_mask_closure_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ATTENTION_PRIMITIVE_MASK_CLOSURE_REPORT_REF)
}

pub fn write_tassadar_attention_primitive_mask_closure_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarAttentionPrimitiveMaskClosureReport,
    TassadarAttentionPrimitiveMaskClosureReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarAttentionPrimitiveMaskClosureReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_attention_primitive_mask_closure_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarAttentionPrimitiveMaskClosureReportError::Write {
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
) -> Result<T, TassadarAttentionPrimitiveMaskClosureReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarAttentionPrimitiveMaskClosureReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarAttentionPrimitiveMaskClosureReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        boundary_review, build_report_from_inputs,
        build_tassadar_attention_primitive_mask_closure_report, case_rows, read_json,
        tassadar_attention_primitive_mask_closure_report_path,
        write_tassadar_attention_primitive_mask_closure_report,
        TassadarAttentionPrimitiveMaskClosureReport,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_canonical_transformer_stack_boundary_report,
    };

    #[test]
    fn attention_primitive_mask_closure_is_tied_and_blocked_until_later_work() {
        let report = build_tassadar_attention_primitive_mask_closure_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.boundary_review.passed);
        assert!(report.all_required_cases_present);
        assert!(report.all_cases_pass);
        assert!(report.attention_primitive_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.case_rows.len(), 6);
    }

    #[test]
    fn missing_case_keeps_attention_contract_red() {
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
        assert!(!report.attention_primitive_contract_green);
    }

    #[test]
    fn failed_boundary_review_keeps_attention_contract_red() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let boundary_report =
            build_tassadar_canonical_transformer_stack_boundary_report().expect("boundary");
        let mut review = boundary_review().expect("review");
        review.passed = false;

        let report = build_report_from_inputs(
            acceptance_gate_report,
            boundary_report,
            case_rows(),
            review,
        );

        assert!(!report.attention_primitive_contract_green);
    }

    #[test]
    fn attention_primitive_mask_closure_matches_committed_truth() {
        let generated = build_tassadar_attention_primitive_mask_closure_report().expect("report");
        let committed: TassadarAttentionPrimitiveMaskClosureReport =
            read_json(tassadar_attention_primitive_mask_closure_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_attention_primitive_mask_closure_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_attention_primitive_mask_closure_report.json");
        let written = write_tassadar_attention_primitive_mask_closure_report(&output_path)
            .expect("write report");
        let persisted: TassadarAttentionPrimitiveMaskClosureReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_attention_primitive_mask_closure_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_attention_primitive_mask_closure_report.json")
        );
    }
}
