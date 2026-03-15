use std::collections::BTreeMap;

use crate::authority::{
    AppendComputeEvaluationSamplesRequest, CreateComputeEvaluationRunRequest,
    FinalizeComputeEvaluationRunRequest,
};
use crate::compute::{
    ComputeAppleAdapterSampleKind, ComputeEnvironmentBinding, ComputeEvaluationArtifact,
    ComputeEvaluationMetric, ComputeEvaluationRun, ComputeEvaluationRunStatus,
    ComputeEvaluationSample, ComputeEvaluationSampleStatus,
};
use crate::receipts::{PolicyContext, ReceiptHints, TraceContext};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeBenchmarkAdapterKind {
    MmluMultipleChoiceV1,
    AppleAdapterEvalV1,
}

impl ComputeBenchmarkAdapterKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::MmluMultipleChoiceV1 => "mmlu_multiple_choice_v1",
            Self::AppleAdapterEvalV1 => "apple_adapter_eval_v1",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "mmlu_multiple_choice_v1" => Some(Self::MmluMultipleChoiceV1),
            "apple_adapter_eval_v1" => Some(Self::AppleAdapterEvalV1),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ComputeBenchmarkCaseImport {
    pub sample_id: String,
    #[serde(default)]
    pub ordinal: Option<u64>,
    #[serde(default)]
    pub input_ref: Option<String>,
    #[serde(default)]
    pub output_ref: Option<String>,
    #[serde(default)]
    pub expected_output_ref: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<ComputeEvaluationArtifact>,
    #[serde(default)]
    pub metadata: Value,
    pub recorded_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ComputeBenchmarkImportRequest {
    pub idempotency_prefix: String,
    pub trace: TraceContext,
    pub policy: PolicyContext,
    pub adapter_kind: ComputeBenchmarkAdapterKind,
    pub benchmark_family: String,
    #[serde(default)]
    pub benchmark_suite_ref: Option<String>,
    pub eval_run_id: String,
    pub environment_binding: ComputeEnvironmentBinding,
    #[serde(default)]
    pub product_id: Option<String>,
    #[serde(default)]
    pub capacity_lot_id: Option<String>,
    #[serde(default)]
    pub instrument_id: Option<String>,
    #[serde(default)]
    pub delivery_proof_id: Option<String>,
    #[serde(default)]
    pub model_ref: Option<String>,
    #[serde(default)]
    pub source_ref: Option<String>,
    pub created_at_ms: i64,
    pub finalized_at_ms: i64,
    #[serde(default)]
    pub cases: Vec<ComputeBenchmarkCaseImport>,
    #[serde(default)]
    pub run_artifacts: Vec<ComputeEvaluationArtifact>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub hints: ReceiptHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ComputeBenchmarkAdaptedRun {
    pub adapter_kind: ComputeBenchmarkAdapterKind,
    pub benchmark_family: String,
    #[serde(default)]
    pub benchmark_suite_ref: Option<String>,
    pub create_eval_run: CreateComputeEvaluationRunRequest,
    pub append_samples: AppendComputeEvaluationSamplesRequest,
    pub finalize_eval_run: FinalizeComputeEvaluationRunRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MmluMultipleChoiceCaseMetadata {
    pub subject: String,
    pub choices: Vec<String>,
    pub correct_choice_index: u32,
    pub predicted_choice_index: u32,
    #[serde(default)]
    pub prompt_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppleAdapterObservedToolCallImport {
    pub tool_name: String,
    pub succeeded: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppleAdapterBenchmarkCaseMetadata {
    pub sample_kind: ComputeAppleAdapterSampleKind,
    pub expected_output_digest: String,
    pub expected_output_text: String,
    pub observed_output_text: String,
    #[serde(default)]
    pub expected_structured_output: Option<Value>,
    #[serde(default)]
    pub observed_structured_output: Option<Value>,
    #[serde(default)]
    pub required_tool_names: Vec<String>,
    #[serde(default)]
    pub observed_tool_calls: Vec<AppleAdapterObservedToolCallImport>,
}

pub fn adapt_compute_benchmark_import(
    mut request: ComputeBenchmarkImportRequest,
) -> Result<ComputeBenchmarkAdaptedRun, String> {
    request.idempotency_prefix = normalize_required(
        request.idempotency_prefix.as_str(),
        "compute_benchmark_idempotency_prefix_missing",
    )?;
    request.benchmark_family = normalize_required(
        request.benchmark_family.as_str(),
        "compute_benchmark_family_missing",
    )?;
    request.eval_run_id =
        normalize_required(request.eval_run_id.as_str(), "compute_eval_run_id_missing")?;
    if request
        .environment_binding
        .environment_ref
        .trim()
        .is_empty()
    {
        return Err("compute_environment_binding_ref_missing".to_string());
    }
    if request.created_at_ms <= 0 || request.finalized_at_ms < request.created_at_ms {
        return Err("compute_benchmark_timestamps_invalid".to_string());
    }
    if request.cases.is_empty() {
        return Err("compute_benchmark_cases_missing".to_string());
    }

    match request.adapter_kind {
        ComputeBenchmarkAdapterKind::MmluMultipleChoiceV1 => adapt_mmlu_multiple_choice(request),
        ComputeBenchmarkAdapterKind::AppleAdapterEvalV1 => adapt_apple_adapter_eval(request),
    }
}

fn adapt_mmlu_multiple_choice(
    request: ComputeBenchmarkImportRequest,
) -> Result<ComputeBenchmarkAdaptedRun, String> {
    let samples = request
        .cases
        .iter()
        .map(|case| adapt_mmlu_case(case, &request))
        .collect::<Result<Vec<_>, _>>()?;

    let run_metadata = benchmark_run_metadata(&request);
    let finalize_metadata = json!({
        "benchmark_adapter_kind": request.adapter_kind.label(),
        "benchmark_family": request.benchmark_family,
        "benchmark_suite_ref": request.benchmark_suite_ref,
        "adapter_metadata": request.metadata,
    });

    Ok(ComputeBenchmarkAdaptedRun {
        adapter_kind: request.adapter_kind,
        benchmark_family: request.benchmark_family.clone(),
        benchmark_suite_ref: request.benchmark_suite_ref.clone(),
        create_eval_run: CreateComputeEvaluationRunRequest {
            idempotency_key: format!("{}.create_eval_run", request.idempotency_prefix),
            trace: request.trace.clone(),
            policy: request.policy.clone(),
            eval_run: ComputeEvaluationRun {
                eval_run_id: request.eval_run_id.clone(),
                environment_binding: request.environment_binding.clone(),
                product_id: request.product_id.clone(),
                capacity_lot_id: request.capacity_lot_id.clone(),
                instrument_id: request.instrument_id.clone(),
                delivery_proof_id: request.delivery_proof_id.clone(),
                model_ref: request.model_ref.clone(),
                source_ref: request.source_ref.clone(),
                created_at_ms: request.created_at_ms,
                expected_sample_count: Some(samples.len() as u64),
                status: ComputeEvaluationRunStatus::Queued,
                started_at_ms: None,
                finalized_at_ms: None,
                summary: None,
                run_artifacts: Vec::new(),
                metadata: run_metadata,
            },
            evidence: Vec::new(),
            hints: request.hints.clone(),
        },
        append_samples: AppendComputeEvaluationSamplesRequest {
            idempotency_key: format!("{}.append_samples", request.idempotency_prefix),
            trace: request.trace.clone(),
            policy: request.policy.clone(),
            eval_run_id: request.eval_run_id.clone(),
            samples,
            evidence: Vec::new(),
            hints: request.hints.clone(),
        },
        finalize_eval_run: FinalizeComputeEvaluationRunRequest {
            idempotency_key: format!("{}.finalize_eval_run", request.idempotency_prefix),
            trace: request.trace,
            policy: request.policy,
            eval_run_id: request.eval_run_id,
            status: ComputeEvaluationRunStatus::Finalized,
            finalized_at_ms: request.finalized_at_ms,
            artifacts: request.run_artifacts,
            metadata: finalize_metadata,
            evidence: Vec::new(),
            hints: request.hints,
        },
    })
}

fn adapt_mmlu_case(
    case: &ComputeBenchmarkCaseImport,
    request: &ComputeBenchmarkImportRequest,
) -> Result<ComputeEvaluationSample, String> {
    let sample_id = normalize_required(case.sample_id.as_str(), "compute_eval_sample_id_missing")?;
    if case.recorded_at_ms <= 0 {
        return Err("compute_benchmark_case_recorded_at_invalid".to_string());
    }
    let metadata: MmluMultipleChoiceCaseMetadata = serde_json::from_value(case.metadata.clone())
        .map_err(|error| format!("compute_benchmark_mmlu_metadata_invalid:{error}"))?;
    if metadata.subject.trim().is_empty() {
        return Err("compute_benchmark_mmlu_subject_missing".to_string());
    }
    if metadata.choices.len() < 2 {
        return Err("compute_benchmark_mmlu_choices_invalid".to_string());
    }
    let correct_choice_index = metadata.correct_choice_index as usize;
    let predicted_choice_index = metadata.predicted_choice_index as usize;
    if correct_choice_index >= metadata.choices.len()
        || predicted_choice_index >= metadata.choices.len()
    {
        return Err("compute_benchmark_mmlu_choice_index_invalid".to_string());
    }
    let passed = metadata.correct_choice_index == metadata.predicted_choice_index;
    Ok(ComputeEvaluationSample {
        eval_run_id: request.eval_run_id.clone(),
        sample_id,
        ordinal: case.ordinal,
        status: if passed {
            ComputeEvaluationSampleStatus::Passed
        } else {
            ComputeEvaluationSampleStatus::Failed
        },
        input_ref: normalize_optional(case.input_ref.as_deref()),
        output_ref: normalize_optional(case.output_ref.as_deref()),
        expected_output_ref: normalize_optional(case.expected_output_ref.as_deref()),
        score_bps: Some(if passed { 10_000 } else { 0 }),
        metrics: vec![ComputeEvaluationMetric {
            metric_id: "accuracy".to_string(),
            metric_value: if passed { 1.0 } else { 0.0 },
            unit: Some("fraction".to_string()),
            metadata: json!({
                "benchmark_adapter_kind": request.adapter_kind.label(),
                "benchmark_family": request.benchmark_family,
                "subject": metadata.subject,
            }),
        }],
        artifacts: case.artifacts.clone(),
        error_reason: None,
        recorded_at_ms: case.recorded_at_ms,
        metadata: json!({
            "benchmark_adapter_kind": request.adapter_kind.label(),
            "benchmark_family": request.benchmark_family,
            "benchmark_suite_ref": request.benchmark_suite_ref,
            "benchmark_case": metadata,
        }),
    })
}

fn adapt_apple_adapter_eval(
    request: ComputeBenchmarkImportRequest,
) -> Result<ComputeBenchmarkAdaptedRun, String> {
    let samples = request
        .cases
        .iter()
        .map(|case| adapt_apple_adapter_case(case, &request))
        .collect::<Result<Vec<_>, _>>()?;

    let run_metadata = benchmark_run_metadata(&request);
    let finalize_metadata = json!({
        "benchmark_adapter_kind": request.adapter_kind.label(),
        "benchmark_family": request.benchmark_family,
        "benchmark_suite_ref": request.benchmark_suite_ref,
        "adapter_metadata": request.metadata,
    });

    Ok(ComputeBenchmarkAdaptedRun {
        adapter_kind: request.adapter_kind,
        benchmark_family: request.benchmark_family.clone(),
        benchmark_suite_ref: request.benchmark_suite_ref.clone(),
        create_eval_run: CreateComputeEvaluationRunRequest {
            idempotency_key: format!("{}.create_eval_run", request.idempotency_prefix),
            trace: request.trace.clone(),
            policy: request.policy.clone(),
            eval_run: ComputeEvaluationRun {
                eval_run_id: request.eval_run_id.clone(),
                environment_binding: request.environment_binding.clone(),
                product_id: request.product_id.clone(),
                capacity_lot_id: request.capacity_lot_id.clone(),
                instrument_id: request.instrument_id.clone(),
                delivery_proof_id: request.delivery_proof_id.clone(),
                model_ref: request.model_ref.clone(),
                source_ref: request.source_ref.clone(),
                created_at_ms: request.created_at_ms,
                expected_sample_count: Some(samples.len() as u64),
                status: ComputeEvaluationRunStatus::Queued,
                started_at_ms: None,
                finalized_at_ms: None,
                summary: None,
                run_artifacts: Vec::new(),
                metadata: run_metadata,
            },
            evidence: Vec::new(),
            hints: request.hints.clone(),
        },
        append_samples: AppendComputeEvaluationSamplesRequest {
            idempotency_key: format!("{}.append_samples", request.idempotency_prefix),
            trace: request.trace.clone(),
            policy: request.policy.clone(),
            eval_run_id: request.eval_run_id.clone(),
            samples,
            evidence: Vec::new(),
            hints: request.hints.clone(),
        },
        finalize_eval_run: FinalizeComputeEvaluationRunRequest {
            idempotency_key: format!("{}.finalize_eval_run", request.idempotency_prefix),
            trace: request.trace,
            policy: request.policy,
            eval_run_id: request.eval_run_id,
            status: ComputeEvaluationRunStatus::Finalized,
            finalized_at_ms: request.finalized_at_ms,
            artifacts: request.run_artifacts,
            metadata: finalize_metadata,
            evidence: Vec::new(),
            hints: request.hints,
        },
    })
}

fn adapt_apple_adapter_case(
    case: &ComputeBenchmarkCaseImport,
    request: &ComputeBenchmarkImportRequest,
) -> Result<ComputeEvaluationSample, String> {
    let sample_id = normalize_required(case.sample_id.as_str(), "compute_eval_sample_id_missing")?;
    if case.recorded_at_ms <= 0 {
        return Err("compute_benchmark_case_recorded_at_invalid".to_string());
    }
    let metadata: AppleAdapterBenchmarkCaseMetadata = serde_json::from_value(case.metadata.clone())
        .map_err(|error| format!("compute_benchmark_apple_adapter_metadata_invalid:{error}"))?;
    if metadata.expected_output_digest.trim().is_empty() {
        return Err("compute_benchmark_apple_adapter_expected_output_digest_missing".to_string());
    }
    if metadata.expected_output_text.trim().is_empty() {
        return Err("compute_benchmark_apple_adapter_expected_output_text_missing".to_string());
    }
    if metadata.observed_output_text.trim().is_empty() {
        return Err("compute_benchmark_apple_adapter_observed_output_text_missing".to_string());
    }
    if matches!(
        metadata.sample_kind,
        ComputeAppleAdapterSampleKind::GuidedGenerationWithSchema
    ) && (metadata.expected_structured_output.is_none()
        || metadata.observed_structured_output.is_none())
    {
        return Err("compute_benchmark_apple_adapter_structured_output_missing".to_string());
    }
    if matches!(
        metadata.sample_kind,
        ComputeAppleAdapterSampleKind::ToolCalling
    ) && metadata.required_tool_names.is_empty()
    {
        return Err("compute_benchmark_apple_adapter_required_tool_names_missing".to_string());
    }
    for tool_name in &metadata.required_tool_names {
        if tool_name.trim().is_empty() {
            return Err("compute_benchmark_apple_adapter_required_tool_name_invalid".to_string());
        }
    }
    for observed_call in &metadata.observed_tool_calls {
        if observed_call.tool_name.trim().is_empty() {
            return Err("compute_benchmark_apple_adapter_observed_tool_name_invalid".to_string());
        }
    }

    let text_match = normalized_text(metadata.expected_output_text.as_str())
        == normalized_text(metadata.observed_output_text.as_str());
    let mut metrics = vec![ComputeEvaluationMetric {
        metric_id: "apple_adapter.text_match".to_string(),
        metric_value: if text_match { 1.0 } else { 0.0 },
        unit: Some("fraction".to_string()),
        metadata: json!({
            "benchmark_adapter_kind": request.adapter_kind.label(),
            "benchmark_family": request.benchmark_family,
            "sample_kind": metadata.sample_kind.label(),
        }),
    }];

    let mut passed = text_match;
    if let (
        ComputeAppleAdapterSampleKind::GuidedGenerationWithSchema,
        Some(expected_structured_output),
        Some(observed_structured_output),
    ) = (
        metadata.sample_kind,
        metadata.expected_structured_output.as_ref(),
        metadata.observed_structured_output.as_ref(),
    ) {
        let structured_match = expected_structured_output == observed_structured_output;
        metrics.push(ComputeEvaluationMetric {
            metric_id: "apple_adapter.structured_output_match".to_string(),
            metric_value: if structured_match { 1.0 } else { 0.0 },
            unit: Some("fraction".to_string()),
            metadata: json!({
                "benchmark_adapter_kind": request.adapter_kind.label(),
                "benchmark_family": request.benchmark_family,
                "sample_kind": metadata.sample_kind.label(),
            }),
        });
        passed &= structured_match;
    }

    if matches!(
        metadata.sample_kind,
        ComputeAppleAdapterSampleKind::ToolCalling
    ) {
        let observed_tools = metadata
            .observed_tool_calls
            .iter()
            .map(|call| (call.tool_name.as_str(), call.succeeded))
            .collect::<BTreeMap<_, _>>();
        let matched_tools = metadata
            .required_tool_names
            .iter()
            .filter(|tool_name| observed_tools.get(tool_name.as_str()) == Some(&true))
            .count();
        let coverage = matched_tools as f64 / metadata.required_tool_names.len() as f64;
        metrics.push(ComputeEvaluationMetric {
            metric_id: "apple_adapter.tool_call_coverage".to_string(),
            metric_value: coverage,
            unit: Some("fraction".to_string()),
            metadata: json!({
                "benchmark_adapter_kind": request.adapter_kind.label(),
                "benchmark_family": request.benchmark_family,
                "required_tool_names": metadata.required_tool_names,
            }),
        });
        passed &= (coverage - 1.0).abs() < f64::EPSILON;
    }

    Ok(ComputeEvaluationSample {
        eval_run_id: request.eval_run_id.clone(),
        sample_id,
        ordinal: case.ordinal,
        status: if passed {
            ComputeEvaluationSampleStatus::Passed
        } else {
            ComputeEvaluationSampleStatus::Failed
        },
        input_ref: normalize_optional(case.input_ref.as_deref()),
        output_ref: normalize_optional(case.output_ref.as_deref()),
        expected_output_ref: normalize_optional(case.expected_output_ref.as_deref())
            .or_else(|| Some(metadata.expected_output_digest.clone())),
        score_bps: Some(average_metric_score_bps(metrics.as_slice())),
        metrics,
        artifacts: case.artifacts.clone(),
        error_reason: None,
        recorded_at_ms: case.recorded_at_ms,
        metadata: json!({
            "benchmark_adapter_kind": request.adapter_kind.label(),
            "benchmark_family": request.benchmark_family,
            "benchmark_suite_ref": request.benchmark_suite_ref,
            "benchmark_case": metadata,
        }),
    })
}

fn benchmark_run_metadata(request: &ComputeBenchmarkImportRequest) -> Value {
    json!({
        "benchmark_adapter_kind": request.adapter_kind.label(),
        "benchmark_family": request.benchmark_family,
        "benchmark_suite_ref": request.benchmark_suite_ref,
        "adapter_metadata": request.metadata,
    })
}

fn average_metric_score_bps(metrics: &[ComputeEvaluationMetric]) -> u32 {
    if metrics.is_empty() {
        return 0;
    }
    let total = metrics
        .iter()
        .map(|metric| (metric.metric_value.clamp(0.0, 1.0) * 10_000.0).round() as u32)
        .sum::<u32>();
    total / metrics.len() as u32
}

fn normalized_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn normalize_required(value: &str, reason: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(reason.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{
        AppleAdapterBenchmarkCaseMetadata, AppleAdapterObservedToolCallImport,
        ComputeBenchmarkAdapterKind, ComputeBenchmarkCaseImport, ComputeBenchmarkImportRequest,
        MmluMultipleChoiceCaseMetadata, adapt_compute_benchmark_import,
    };
    use crate::compute::{
        ComputeAppleAdapterSampleKind, ComputeEnvironmentBinding, ComputeEvaluationSampleStatus,
    };
    use crate::receipts::{PolicyContext, ReceiptHints, TraceContext};
    use serde_json::{Value, json};

    fn mmlu_import_request() -> ComputeBenchmarkImportRequest {
        ComputeBenchmarkImportRequest {
            idempotency_prefix: "benchmark.mmlu.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            adapter_kind: ComputeBenchmarkAdapterKind::MmluMultipleChoiceV1,
            benchmark_family: "mmlu".to_string(),
            benchmark_suite_ref: Some("benchmark://mmlu/pro".to_string()),
            eval_run_id: "eval.benchmark.mmlu.alpha".to_string(),
            environment_binding: ComputeEnvironmentBinding {
                environment_ref: "env.openagents.math.basic".to_string(),
                environment_version: None,
                dataset_ref: None,
                rubric_ref: None,
                evaluator_policy_ref: None,
            },
            product_id: Some("ollama.text_generation".to_string()),
            capacity_lot_id: None,
            instrument_id: None,
            delivery_proof_id: None,
            model_ref: Some("model://llama3.3".to_string()),
            source_ref: Some("artifact://benchmarks/mmlu/input".to_string()),
            created_at_ms: 1_762_000_700_000,
            finalized_at_ms: 1_762_000_710_000,
            cases: vec![
                ComputeBenchmarkCaseImport {
                    sample_id: "sample.alpha".to_string(),
                    ordinal: Some(1),
                    input_ref: Some("artifact://benchmarks/mmlu/input/alpha".to_string()),
                    output_ref: Some("artifact://benchmarks/mmlu/output/alpha".to_string()),
                    expected_output_ref: Some(
                        "artifact://benchmarks/mmlu/expected/alpha".to_string(),
                    ),
                    artifacts: Vec::new(),
                    metadata: serde_json::to_value(MmluMultipleChoiceCaseMetadata {
                        subject: "biology".to_string(),
                        choices: vec![
                            "A".to_string(),
                            "B".to_string(),
                            "C".to_string(),
                            "D".to_string(),
                        ],
                        correct_choice_index: 1,
                        predicted_choice_index: 1,
                        prompt_id: Some("bio-1".to_string()),
                    })
                    .expect("metadata"),
                    recorded_at_ms: 1_762_000_705_000,
                },
                ComputeBenchmarkCaseImport {
                    sample_id: "sample.beta".to_string(),
                    ordinal: Some(2),
                    input_ref: Some("artifact://benchmarks/mmlu/input/beta".to_string()),
                    output_ref: Some("artifact://benchmarks/mmlu/output/beta".to_string()),
                    expected_output_ref: Some(
                        "artifact://benchmarks/mmlu/expected/beta".to_string(),
                    ),
                    artifacts: Vec::new(),
                    metadata: serde_json::to_value(MmluMultipleChoiceCaseMetadata {
                        subject: "history".to_string(),
                        choices: vec![
                            "A".to_string(),
                            "B".to_string(),
                            "C".to_string(),
                            "D".to_string(),
                        ],
                        correct_choice_index: 2,
                        predicted_choice_index: 0,
                        prompt_id: Some("hist-1".to_string()),
                    })
                    .expect("metadata"),
                    recorded_at_ms: 1_762_000_705_100,
                },
            ],
            run_artifacts: Vec::new(),
            metadata: json!({"split": "test"}),
            hints: ReceiptHints::default(),
        }
    }

    fn apple_adapter_import_request() -> ComputeBenchmarkImportRequest {
        ComputeBenchmarkImportRequest {
            idempotency_prefix: "benchmark.apple_adapter.alpha".to_string(),
            trace: TraceContext::default(),
            policy: PolicyContext::default(),
            adapter_kind: ComputeBenchmarkAdapterKind::AppleAdapterEvalV1,
            benchmark_family: "apple_adapter.helpdesk".to_string(),
            benchmark_suite_ref: Some("benchmark://apple_adapter/helpdesk/reference".to_string()),
            eval_run_id: "eval.benchmark.apple_adapter.alpha".to_string(),
            environment_binding: ComputeEnvironmentBinding {
                environment_ref: "env.openagents.apple_adapter.helpdesk.benchmark".to_string(),
                environment_version: None,
                dataset_ref: None,
                rubric_ref: None,
                evaluator_policy_ref: None,
            },
            product_id: Some("psionic.local.inference.apple_foundation_models.single_node".into()),
            capacity_lot_id: None,
            instrument_id: None,
            delivery_proof_id: None,
            model_ref: Some("model://apple.foundation".to_string()),
            source_ref: Some("artifact://benchmarks/apple_adapter/input".to_string()),
            created_at_ms: 1_762_000_720_000,
            finalized_at_ms: 1_762_000_730_000,
            cases: vec![
                ComputeBenchmarkCaseImport {
                    sample_id: "sample.chat".to_string(),
                    ordinal: Some(1),
                    input_ref: Some("artifact://benchmarks/apple_adapter/input/chat".to_string()),
                    output_ref: Some("artifact://benchmarks/apple_adapter/output/chat".to_string()),
                    expected_output_ref: None,
                    artifacts: Vec::new(),
                    metadata: serde_json::to_value(AppleAdapterBenchmarkCaseMetadata {
                        sample_kind: ComputeAppleAdapterSampleKind::SupervisedFineTune,
                        expected_output_digest: "sha256:sample-chat".to_string(),
                        expected_output_text:
                            "A mutex allows one thread at a time to access a shared resource."
                                .to_string(),
                        observed_output_text:
                            "A mutex allows one thread at a time to access a shared resource."
                                .to_string(),
                        expected_structured_output: None,
                        observed_structured_output: None,
                        required_tool_names: Vec::new(),
                        observed_tool_calls: Vec::new(),
                    })
                    .expect("metadata"),
                    recorded_at_ms: 1_762_000_725_000,
                },
                ComputeBenchmarkCaseImport {
                    sample_id: "sample.schema".to_string(),
                    ordinal: Some(2),
                    input_ref: Some("artifact://benchmarks/apple_adapter/input/schema".to_string()),
                    output_ref: Some(
                        "artifact://benchmarks/apple_adapter/output/schema".to_string(),
                    ),
                    expected_output_ref: None,
                    artifacts: Vec::new(),
                    metadata: serde_json::to_value(AppleAdapterBenchmarkCaseMetadata {
                        sample_kind: ComputeAppleAdapterSampleKind::GuidedGenerationWithSchema,
                        expected_output_digest: "sha256:sample-schema".to_string(),
                        expected_output_text: "{\"priority\":\"high\"}".to_string(),
                        observed_output_text: "{\"priority\":\"low\"}".to_string(),
                        expected_structured_output: Some(json!({"priority": "high"})),
                        observed_structured_output: Some(json!({"priority": "low"})),
                        required_tool_names: Vec::new(),
                        observed_tool_calls: Vec::new(),
                    })
                    .expect("metadata"),
                    recorded_at_ms: 1_762_000_725_050,
                },
                ComputeBenchmarkCaseImport {
                    sample_id: "sample.tool".to_string(),
                    ordinal: Some(3),
                    input_ref: Some("artifact://benchmarks/apple_adapter/input/tool".to_string()),
                    output_ref: Some("artifact://benchmarks/apple_adapter/output/tool".to_string()),
                    expected_output_ref: None,
                    artifacts: Vec::new(),
                    metadata: serde_json::to_value(AppleAdapterBenchmarkCaseMetadata {
                        sample_kind: ComputeAppleAdapterSampleKind::ToolCalling,
                        expected_output_digest: "sha256:sample-tool".to_string(),
                        expected_output_text: "I checked the refund policy.".to_string(),
                        observed_output_text: "I checked the refund policy.".to_string(),
                        expected_structured_output: None,
                        observed_structured_output: None,
                        required_tool_names: vec!["lookup_order".to_string()],
                        observed_tool_calls: vec![AppleAdapterObservedToolCallImport {
                            tool_name: "lookup_order".to_string(),
                            succeeded: true,
                        }],
                    })
                    .expect("metadata"),
                    recorded_at_ms: 1_762_000_725_100,
                },
            ],
            run_artifacts: Vec::new(),
            metadata: json!({"split": "held_out"}),
            hints: ReceiptHints::default(),
        }
    }

    #[test]
    fn mmlu_adapter_builds_eval_lifecycle_and_preserves_metadata() {
        let adapted = adapt_compute_benchmark_import(mmlu_import_request())
            .expect("mmlu import should adapt");
        assert_eq!(
            adapted.adapter_kind,
            ComputeBenchmarkAdapterKind::MmluMultipleChoiceV1
        );
        assert_eq!(
            adapted.create_eval_run.eval_run.expected_sample_count,
            Some(2)
        );
        assert_eq!(adapted.append_samples.samples.len(), 2);
        assert_eq!(
            adapted.append_samples.samples[0].status,
            ComputeEvaluationSampleStatus::Passed
        );
        assert_eq!(
            adapted.append_samples.samples[1].status,
            ComputeEvaluationSampleStatus::Failed
        );
        assert_eq!(
            adapted.append_samples.samples[0]
                .metadata
                .get("benchmark_case")
                .and_then(|value| value.get("subject"))
                .and_then(Value::as_str),
            Some("biology")
        );
        assert_eq!(
            adapted
                .finalize_eval_run
                .metadata
                .get("benchmark_adapter_kind")
                .and_then(Value::as_str),
            Some("mmlu_multiple_choice_v1")
        );
    }

    #[test]
    fn mmlu_adapter_rejects_invalid_choice_indices() {
        let mut request = mmlu_import_request();
        request.cases[0].metadata = json!({
            "subject": "biology",
            "choices": ["A", "B"],
            "correct_choice_index": 3,
            "predicted_choice_index": 1,
        });
        let error = adapt_compute_benchmark_import(request)
            .expect_err("invalid choice indices should fail");
        assert_eq!(error, "compute_benchmark_mmlu_choice_index_invalid");
    }

    #[test]
    fn apple_adapter_eval_builds_eval_lifecycle_with_mixed_case_metrics() {
        let adapted = adapt_compute_benchmark_import(apple_adapter_import_request())
            .expect("apple adapter import should adapt");
        assert_eq!(
            adapted.adapter_kind,
            ComputeBenchmarkAdapterKind::AppleAdapterEvalV1
        );
        assert_eq!(adapted.append_samples.samples.len(), 3);
        assert_eq!(
            adapted.append_samples.samples[0].status,
            ComputeEvaluationSampleStatus::Passed
        );
        assert_eq!(
            adapted.append_samples.samples[1].status,
            ComputeEvaluationSampleStatus::Failed
        );
        assert_eq!(
            adapted.append_samples.samples[2]
                .metrics
                .iter()
                .find(|metric| metric.metric_id == "apple_adapter.tool_call_coverage")
                .map(|metric| metric.metric_value),
            Some(1.0)
        );
        assert_eq!(
            adapted
                .finalize_eval_run
                .metadata
                .get("benchmark_adapter_kind")
                .and_then(Value::as_str),
            Some("apple_adapter_eval_v1")
        );
    }

    #[test]
    fn apple_adapter_eval_rejects_tool_case_without_required_tools() {
        let mut request = apple_adapter_import_request();
        request.cases[2].metadata = json!({
            "sample_kind": "tool_calling",
            "expected_output_digest": "sha256:sample-tool",
            "expected_output_text": "I checked the refund policy.",
            "observed_output_text": "I checked the refund policy.",
            "required_tool_names": [],
            "observed_tool_calls": [],
        });
        let error = adapt_compute_benchmark_import(request)
            .expect_err("tool cases should require explicit tool names");
        assert_eq!(
            error,
            "compute_benchmark_apple_adapter_required_tool_names_missing"
        );
    }
}
