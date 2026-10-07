use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{ParameterGolfBatchGeometry, ParameterGolfSingleH100TrainingReport};

/// Current disposition of the same-node parity harness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSameNodeParityDisposition {
    Blocked,
    Ready,
}

/// Comparison direction for one parity metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSameNodeParityJudgment {
    Faster,
    Slower,
    Better,
    Worse,
    Equivalent,
    Divergent,
    Missing,
}

/// One normalized upstream `train_gpt.py` run receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfTrainGptReferenceRunReceipt {
    pub schema_version: u16,
    pub report_id: String,
    pub run_id: String,
    pub source_log_path: PathBuf,
    pub source_log_digest: String,
    pub device_name: String,
    pub dataset_manifest_digest: String,
    pub tokenizer_digest: String,
    pub geometry: ParameterGolfBatchGeometry,
    pub train_step_observed_ms: Option<u64>,
    pub final_validation_observed_ms: Option<u64>,
    pub final_roundtrip_eval_ms: Option<u64>,
    pub peak_memory_allocated_mib: Option<u64>,
    pub peak_memory_reserved_mib: Option<u64>,
    pub final_validation_loss: Option<f64>,
    pub final_validation_bpb: Option<f64>,
    pub final_roundtrip_val_loss: Option<f64>,
    pub final_roundtrip_val_bpb: Option<f64>,
    pub compressed_model_bytes: Option<u64>,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl ParameterGolfTrainGptReferenceRunReceipt {
    fn new(
        config: &ParameterGolfTrainGptReferenceRunConfig,
        source_log_digest: String,
        parsed: &ParsedTrainGptLogMetrics,
    ) -> Self {
        let mut receipt = Self {
            schema_version: 1,
            report_id: String::from("parameter_golf.train_gpt_reference_run_receipt.v1"),
            run_id: config.run_id.clone(),
            source_log_path: config.source_log_path.clone(),
            source_log_digest,
            device_name: config.device_name.clone(),
            dataset_manifest_digest: config.dataset_manifest_digest.clone(),
            tokenizer_digest: config.tokenizer_digest.clone(),
            geometry: config.geometry.clone(),
            train_step_observed_ms: parsed.train_step_observed_ms,
            final_validation_observed_ms: config.final_validation_observed_ms,
            final_roundtrip_eval_ms: parsed
                .final_roundtrip_eval_ms
                .or(config.final_roundtrip_eval_ms),
            peak_memory_allocated_mib: parsed
                .peak_memory_allocated_mib
                .or(config.peak_memory_allocated_mib),
            peak_memory_reserved_mib: parsed
                .peak_memory_reserved_mib
                .or(config.peak_memory_reserved_mib),
            final_validation_loss: parsed.final_validation_loss,
            final_validation_bpb: parsed.final_validation_bpb,
            final_roundtrip_val_loss: parsed.final_roundtrip_val_loss,
            final_roundtrip_val_bpb: parsed.final_roundtrip_val_bpb,
            compressed_model_bytes: parsed.compressed_model_bytes,
            claim_boundary: String::from(
                "This receipt normalizes one upstream train_gpt.py run into the same machine-readable parity surface used by Psionic. It does not by itself prove same-node H100 parity until the operator supplies matched hardware/input identity and the parity report binds both sides together.",
            ),
            report_digest: String::new(),
        };
        receipt.report_digest = stable_digest(
            b"psionic_parameter_golf_train_gpt_reference_run_receipt|",
            &receipt,
        );
        receipt
    }
}

/// Operator-supplied identity and geometry for one upstream `train_gpt.py`
/// run. The upstream log does not carry enough information to infer these
/// fields safely, so the harness requires them explicitly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfTrainGptReferenceRunConfig {
    pub run_id: String,
    pub source_log_path: PathBuf,
    pub device_name: String,
    pub dataset_manifest_digest: String,
    pub tokenizer_digest: String,
    pub geometry: ParameterGolfBatchGeometry,
    pub final_validation_observed_ms: Option<u64>,
    pub final_roundtrip_eval_ms: Option<u64>,
    pub peak_memory_allocated_mib: Option<u64>,
    pub peak_memory_reserved_mib: Option<u64>,
}

/// One parity comparison row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSameNodeParityMetricComparison {
    pub metric_id: String,
    pub better_direction: String,
    pub psionic_value: Option<f64>,
    pub upstream_value: Option<f64>,
    pub judgment: ParameterGolfSameNodeParityJudgment,
    pub detail: String,
}

/// Same-node parity report comparing the Psionic single-H100 receipt against
/// one normalized upstream `train_gpt.py` receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSameNodeParityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub benchmark_ref: String,
    pub disposition: ParameterGolfSameNodeParityDisposition,
    pub psionic_run_id: String,
    pub psionic_report_digest: String,
    pub upstream_run_id: String,
    pub upstream_report_digest: String,
    pub device_names_match: bool,
    pub dataset_manifest_matches: bool,
    pub tokenizer_matches: bool,
    pub geometry_matches: bool,
    pub blockers: Vec<String>,
    pub comparisons: Vec<ParameterGolfSameNodeParityMetricComparison>,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl ParameterGolfSameNodeParityReport {
    fn new(
        psionic: &ParameterGolfSingleH100TrainingReport,
        upstream: &ParameterGolfTrainGptReferenceRunReceipt,
        blockers: Vec<String>,
        comparisons: Vec<ParameterGolfSameNodeParityMetricComparison>,
    ) -> Self {
        let psionic_device_name = psionic
            .observed_cuda_devices
            .iter()
            .find_map(|device| device.device_name.clone())
            .unwrap_or_default();
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("parameter_golf.same_node_parity_report.v1"),
            benchmark_ref: String::from(
                "benchmark://openagents/parameter_golf/same_node_single_h100",
            ),
            disposition: if blockers.is_empty() {
                ParameterGolfSameNodeParityDisposition::Ready
            } else {
                ParameterGolfSameNodeParityDisposition::Blocked
            },
            psionic_run_id: psionic.run_id.clone(),
            psionic_report_digest: psionic.report_digest.clone(),
            upstream_run_id: upstream.run_id.clone(),
            upstream_report_digest: upstream.report_digest.clone(),
            device_names_match: psionic_device_name == upstream.device_name,
            dataset_manifest_matches: psionic.dataset_manifest_digest
                == upstream.dataset_manifest_digest,
            tokenizer_matches: psionic.tokenizer_digest.tokenizer_digest
                == upstream.tokenizer_digest,
            geometry_matches: psionic.geometry == upstream.geometry,
            blockers,
            comparisons,
            claim_boundary: String::from(
                "This report compares one Psionic single-H100 receipt against one normalized upstream train_gpt.py receipt on an intended same-node surface. It stays blocked until matched hardware/input identity and the required metrics are all present, and it stays explicit when Psionic is slower, larger, or numerically divergent.",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_parameter_golf_same_node_parity_report|", &report);
        report
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct ParsedTrainGptLogMetrics {
    train_step_observed_ms: Option<u64>,
    final_roundtrip_eval_ms: Option<u64>,
    peak_memory_allocated_mib: Option<u64>,
    peak_memory_reserved_mib: Option<u64>,
    final_validation_loss: Option<f64>,
    final_validation_bpb: Option<f64>,
    final_roundtrip_val_loss: Option<f64>,
    final_roundtrip_val_bpb: Option<f64>,
    compressed_model_bytes: Option<u64>,
}

/// Failure while parsing or persisting the same-node parity surfaces.
#[derive(Debug, Error)]
pub enum ParameterGolfSameNodeParityError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to parse upstream log: {message}")]
    ParseLog { message: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Parses one upstream `train_gpt.py` log and binds it to the explicit
/// identity/geometry config supplied by the operator.
pub fn build_parameter_golf_train_gpt_reference_run_receipt(
    config: &ParameterGolfTrainGptReferenceRunConfig,
) -> Result<ParameterGolfTrainGptReferenceRunReceipt, ParameterGolfSameNodeParityError> {
    let log_bytes = fs::read(&config.source_log_path).map_err(|error| {
        ParameterGolfSameNodeParityError::Read {
            path: config.source_log_path.display().to_string(),
            error,
        }
    })?;
    let log_text = String::from_utf8_lossy(&log_bytes);
    let parsed = parse_train_gpt_log(log_text.as_ref())?;
    Ok(ParameterGolfTrainGptReferenceRunReceipt::new(
        config,
        sha256_hex(log_bytes.as_slice()),
        &parsed,
    ))
}

/// Builds one same-node parity report from the current Psionic single-H100
/// receipt and one normalized upstream receipt.
pub fn build_parameter_golf_same_node_parity_report(
    psionic: &ParameterGolfSingleH100TrainingReport,
    upstream: &ParameterGolfTrainGptReferenceRunReceipt,
) -> ParameterGolfSameNodeParityReport {
    let psionic_device_name = psionic
        .observed_cuda_devices
        .iter()
        .find_map(|device| device.device_name.clone())
        .unwrap_or_default();
    let mut blockers = Vec::new();
    if !psionic.training_executed() {
        blockers.push(String::from(
            "psionic receipt is not an executed single-H100 trainer run",
        ));
    }
    if !psionic.machine_contract_satisfied || psionic.matching_h100_device_count == 0 {
        blockers.push(String::from(
            "psionic receipt did not satisfy the single-H100 machine contract",
        ));
    }
    if psionic_device_name != upstream.device_name {
        blockers.push(format!(
            "device mismatch: psionic=`{}` upstream=`{}`",
            psionic_device_name, upstream.device_name
        ));
    }
    if psionic.dataset_manifest_digest != upstream.dataset_manifest_digest {
        blockers.push(String::from(
            "dataset manifest digests do not match between Psionic and upstream receipts",
        ));
    }
    if psionic.tokenizer_digest.tokenizer_digest != upstream.tokenizer_digest {
        blockers.push(String::from(
            "tokenizer digests do not match between Psionic and upstream receipts",
        ));
    }
    if psionic.geometry != upstream.geometry {
        blockers.push(String::from(
            "batch geometry does not match between Psionic and upstream receipts",
        ));
    }

    let psionic_train_step_ms = psionic
        .step_metrics
        .first()
        .map(|step| step.observed_wallclock_ms as f64);
    let psionic_validation_ms = psionic
        .final_roundtrip_receipt
        .as_ref()
        .map(|receipt| receipt.observed_eval_ms as f64);
    let psionic_roundtrip_loss = psionic
        .final_roundtrip_receipt
        .as_ref()
        .map(|receipt| receipt.validation.mean_loss);
    let psionic_roundtrip_bpb = psionic
        .final_roundtrip_receipt
        .as_ref()
        .map(|receipt| receipt.validation.bits_per_byte);
    let psionic_model_bytes = psionic
        .final_roundtrip_receipt
        .as_ref()
        .map(|receipt| receipt.compressed_model_bytes as f64)
        .or_else(|| psionic.compressed_model_bytes.map(|value| value as f64));

    let comparisons = vec![
        lower_is_better_metric(
            "train_step_wallclock_ms",
            psionic_train_step_ms,
            upstream.train_step_observed_ms.map(|value| value as f64),
        ),
        lower_is_better_metric(
            "validation_wallclock_ms",
            psionic_validation_ms,
            upstream.final_roundtrip_eval_ms.map(|value| value as f64),
        ),
        lower_is_better_metric(
            "peak_memory_allocated_mib",
            None,
            upstream.peak_memory_allocated_mib.map(|value| value as f64),
        ),
        equal_or_divergent_metric(
            "final_roundtrip_val_loss",
            psionic_roundtrip_loss,
            upstream.final_roundtrip_val_loss,
            1e-6,
        ),
        equal_or_divergent_metric(
            "final_roundtrip_val_bpb",
            psionic_roundtrip_bpb,
            upstream.final_roundtrip_val_bpb,
            1e-6,
        ),
        lower_is_better_metric(
            "compressed_model_bytes",
            psionic_model_bytes,
            upstream.compressed_model_bytes.map(|value| value as f64),
        ),
    ];

    for comparison in &comparisons {
        if comparison.judgment == ParameterGolfSameNodeParityJudgment::Missing {
            blockers.push(format!(
                "required parity metric `{}` is missing on one side of the receipt pair",
                comparison.metric_id
            ));
        }
    }

    ParameterGolfSameNodeParityReport::new(psionic, upstream, blockers, comparisons)
}

/// Writes one upstream normalized receipt to disk.
pub fn write_parameter_golf_train_gpt_reference_run_receipt(
    output_path: impl AsRef<Path>,
    config: &ParameterGolfTrainGptReferenceRunConfig,
) -> Result<ParameterGolfTrainGptReferenceRunReceipt, ParameterGolfSameNodeParityError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfSameNodeParityError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let receipt = build_parameter_golf_train_gpt_reference_run_receipt(config)?;
    fs::write(
        output_path,
        format!("{}\n", serde_json::to_string_pretty(&receipt)?),
    )
    .map_err(|error| ParameterGolfSameNodeParityError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(receipt)
}

/// Writes one same-node parity report to disk.
pub fn write_parameter_golf_same_node_parity_report(
    output_path: impl AsRef<Path>,
    psionic_report: &ParameterGolfSingleH100TrainingReport,
    upstream_receipt: &ParameterGolfTrainGptReferenceRunReceipt,
) -> Result<ParameterGolfSameNodeParityReport, ParameterGolfSameNodeParityError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfSameNodeParityError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_parameter_golf_same_node_parity_report(psionic_report, upstream_receipt);
    fs::write(
        output_path,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )
    .map_err(|error| ParameterGolfSameNodeParityError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(report)
}

fn lower_is_better_metric(
    metric_id: &str,
    psionic_value: Option<f64>,
    upstream_value: Option<f64>,
) -> ParameterGolfSameNodeParityMetricComparison {
    let (judgment, detail) = match (psionic_value, upstream_value) {
        (Some(left), Some(right)) if (left - right).abs() <= f64::EPSILON => (
            ParameterGolfSameNodeParityJudgment::Equivalent,
            String::from("both sides reported the same value"),
        ),
        (Some(left), Some(right)) if left < right => (
            ParameterGolfSameNodeParityJudgment::Better,
            String::from("Psionic is lower on a lower-is-better metric"),
        ),
        (Some(_), Some(_)) => (
            ParameterGolfSameNodeParityJudgment::Worse,
            String::from("Psionic is higher on a lower-is-better metric"),
        ),
        _ => (
            ParameterGolfSameNodeParityJudgment::Missing,
            String::from("one side is missing this metric"),
        ),
    };
    ParameterGolfSameNodeParityMetricComparison {
        metric_id: String::from(metric_id),
        better_direction: String::from("lower_is_better"),
        psionic_value,
        upstream_value,
        judgment,
        detail,
    }
}

fn equal_or_divergent_metric(
    metric_id: &str,
    psionic_value: Option<f64>,
    upstream_value: Option<f64>,
    tolerance: f64,
) -> ParameterGolfSameNodeParityMetricComparison {
    let (judgment, detail) = match (psionic_value, upstream_value) {
        (Some(left), Some(right)) if (left - right).abs() <= tolerance => (
            ParameterGolfSameNodeParityJudgment::Equivalent,
            format!("values matched within tolerance {tolerance}"),
        ),
        (Some(_), Some(_)) => (
            ParameterGolfSameNodeParityJudgment::Divergent,
            format!("values differ by more than tolerance {tolerance}"),
        ),
        _ => (
            ParameterGolfSameNodeParityJudgment::Missing,
            String::from("one side is missing this metric"),
        ),
    };
    ParameterGolfSameNodeParityMetricComparison {
        metric_id: String::from(metric_id),
        better_direction: String::from("equal_or_divergent"),
        psionic_value,
        upstream_value,
        judgment,
        detail,
    }
}

fn parse_train_gpt_log(
    log_text: &str,
) -> Result<ParsedTrainGptLogMetrics, ParameterGolfSameNodeParityError> {
    let mut parsed = ParsedTrainGptLogMetrics::default();
    for line in log_text.lines().map(str::trim) {
        if parsed.train_step_observed_ms.is_none()
            && line.starts_with("step:")
            && line.contains("train_loss:")
        {
            parsed.train_step_observed_ms = extract_u64_after(line, "train_time:", "ms");
        }
        if line.starts_with("step:") && line.contains(" val_loss:") && line.contains(" val_bpb:") {
            parsed.final_validation_loss = extract_f64_after(line, "val_loss:", " ");
            parsed.final_validation_bpb = extract_f64_after(line, "val_bpb:", " ");
        }
        if line.starts_with("peak memory allocated:") {
            parsed.peak_memory_allocated_mib =
                extract_u64_after(line, "peak memory allocated:", "MiB");
            parsed.peak_memory_reserved_mib = extract_u64_after(line, "reserved:", "MiB");
        }
        if line.starts_with("Serialized model int8+zlib:") {
            parsed.compressed_model_bytes =
                extract_u64_after(line, "Serialized model int8+zlib:", "bytes");
        }
        if line.starts_with("final_int8_zlib_roundtrip ") {
            parsed.final_roundtrip_eval_ms = extract_u64_after(line, "eval_time:", "ms");
        }
        if line.starts_with("final_int8_zlib_roundtrip_exact") {
            parsed.final_roundtrip_val_loss = extract_f64_after(line, "val_loss:", " ");
            parsed.final_roundtrip_val_bpb = extract_f64_after(line, "val_bpb:", "");
        }
    }
    if parsed.train_step_observed_ms.is_none() {
        return Err(ParameterGolfSameNodeParityError::ParseLog {
            message: String::from("missing first train-step timing line in train_gpt.py log"),
        });
    }
    Ok(parsed)
}

fn extract_u64_after(line: &str, prefix: &str, suffix: &str) -> Option<u64> {
    extract_token_after(line, prefix, suffix)?.parse().ok()
}

fn extract_f64_after(line: &str, prefix: &str, suffix: &str) -> Option<f64> {
    extract_token_after(line, prefix, suffix)?.parse().ok()
}

fn extract_token_after<'a>(line: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    let start = line.find(prefix)? + prefix.len();
    let tail = line[start..].trim_start();
    if suffix.is_empty() {
        return Some(tail.trim());
    }
    let end = tail.find(suffix)?;
    Some(tail[..end].trim())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("serialize parity report"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ParameterGolfSingleH100GroupPrecisionReceipt, ParameterGolfSingleH100PrecisionReceipt,
        ParameterGolfSingleH100TrainingDisposition,
    };
    use psionic_core::{DType, Device, DeviceKind};
    use psionic_data::{DatasetKey, TokenizerDigest, TokenizerFamily};
    use psionic_runtime::{
        DeliveredExecutionContext, DeviceDescriptor, HealthStatus, RuntimeHealth,
    };

    fn sample_geometry() -> ParameterGolfBatchGeometry {
        ParameterGolfBatchGeometry {
            world_size: 1,
            train_batch_tokens: 524_288,
            validation_batch_tokens: 524_288,
            train_sequence_length: 1024,
            grad_accum_steps: 8,
        }
    }

    fn sample_non_h100_psionic_report() -> ParameterGolfSingleH100TrainingReport {
        ParameterGolfSingleH100TrainingReport {
            schema_version: 2,
            scope_window: String::from("parameter_golf_single_h100_training_v2"),
            run_id: String::from("psionic-sample"),
            dataset_root: PathBuf::from("/tmp/dataset"),
            tokenizer_path: PathBuf::from("/tmp/tokenizer.model"),
            dataset_key: DatasetKey::new("fineweb-edu", "2026.03.18"),
            variant: String::from("sp1024"),
            tokenizer_digest: TokenizerDigest::new(
                TokenizerFamily::SentencePiece,
                "tokenizer-digest",
                1_024,
            ),
            dataset_manifest_digest: String::from("dataset-digest"),
            train_shard_count: 1,
            validation_shard_count: 1,
            train_token_count: 524_288,
            validation_token_count: 524_288,
            geometry: sample_geometry(),
            hyperparameters: crate::ParameterGolfTrainingHyperparameters::baseline_defaults(),
            max_steps: 1,
            warmup_steps: 0,
            completed_warmup_steps: 0,
            validation_loss_every: 0,
            train_log_every: 1,
            final_validation_mode: crate::ParameterGolfSingleH100ValidationMode::Both,
            validation_eval_mode: crate::ParameterGolfValidationEvalMode::NonOverlapping,
            validation_batch_sequences: 64,
            matrix_execution_mode: crate::ParameterGolfMatrixExecutionMode::DirectBanked,
            score_first_ttt: None,
            ema: None,
            swa: None,
            final_model_surface: crate::ParameterGolfFinalModelSurface::Raw,
            final_artifact_config: crate::ParameterGolfFinalArtifactConfig::default(),
            executed_steps: 0,
            stop_reason: None,
            delivered_execution: DeliveredExecutionContext::new("cuda", None, Vec::new()),
            machine_thresholds: crate::ParameterGolfSingleH100ChallengeThresholds::challenge_h100(),
            observed_cuda_health: RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("ready"),
            },
            cuda_discovery_error: None,
            observed_cuda_devices: vec![DeviceDescriptor {
                backend: String::from("cuda"),
                device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
                device_name: Some(String::from("NVIDIA GeForce RTX 4080")),
                supported_dtypes: vec![DType::F32, DType::BF16, DType::I32],
                supported_quantization: Vec::new(),
                memory_capacity_bytes: Some(16_u64 * 1024 * 1024 * 1024),
                unified_memory: Some(false),
                feature_flags: Vec::new(),
                amd_metadata: None,
                nvidia_metadata: None,
            }],
            matching_h100_device_count: 0,
            machine_contract_satisfied: false,
            model_variant: crate::ParameterGolfSingleH100ModelVariant::BaselineSp1024_9x512,
            machine_profile: crate::ParameterGolfSingleDeviceMachineProfile::SingleH100,
            model_config: psionic_models::ParameterGolfConfig::baseline_sp1024_9x512(),
            baseline_model_id: String::from("baseline-model"),
            baseline_model_revision: String::from("baseline-revision"),
            baseline_model_descriptor_digest: String::from("model-digest"),
            optimizer_plan_digest: String::from("optimizer-digest"),
            precision_receipt: ParameterGolfSingleH100PrecisionReceipt {
                graph_parameter_upload_precision: crate::TrainingPrecisionMode::Bf16,
                graph_execution_precision: crate::TrainingPrecisionMode::Bf16,
                retained_activation_precision: crate::TrainingPrecisionMode::Fp32,
                group_receipts: vec![ParameterGolfSingleH100GroupPrecisionReceipt {
                    group_id: String::from("token_embedding"),
                    parameter_precision: crate::TrainingPrecisionMode::Bf16,
                    gradient_precision: crate::TrainingPrecisionMode::Fp32,
                    optimizer_state_precision: crate::TrainingPrecisionMode::Fp32,
                    master_weight_precision: Some(crate::TrainingPrecisionMode::Fp32),
                }],
                notes: vec![String::from("test fixture")],
            },
            cuda_training_capability_report_digest: String::from("capability-digest"),
            challenge_kernel_blockers: Vec::new(),
            validation_checkpoints: Vec::new(),
            initial_validation: None,
            pre_export_final_validation: None,
            final_validation: None,
            warmup_observed_ms: 0,
            observed_training_time_ms: 0,
            pre_export_final_validation_observed_ms: None,
            final_validation_observed_ms: None,
            final_swa_receipt: None,
            final_roundtrip_receipt: None,
            compressed_model_bytes: None,
            compressed_model_artifact_ref: None,
            compressed_model_artifact_digest: None,
            compressed_model_artifact_path: None,
            step_metrics: Vec::new(),
            aggregate_phase_timings: None,
            final_training_cursor: None,
            started_at_ms: 0,
            finished_at_ms: 0,
            observed_wallclock_ms: 0,
            disposition: ParameterGolfSingleH100TrainingDisposition::RefusedMachineContract,
            refusal: None,
            claim_boundary: String::from("test fixture"),
            summary: String::from("not an H100 run"),
            report_digest: String::from("psionic-digest"),
        }
    }

    #[test]
    fn train_gpt_reference_receipt_parses_expected_metrics(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let log_path = temp_dir.path().join("train_gpt.log");
        fs::write(
            &log_path,
            "step:1/20000 train_loss:8.1234 train_time:456ms step_avg:456.00ms\n\
             step:1/20000 val_loss:8.4321 val_bpb:9.8765 train_time:456ms step_avg:456.00ms\n\
             peak memory allocated: 22345 MiB reserved: 22784 MiB\n\
             Serialized model int8+zlib: 78958 bytes (payload:123 raw_torch:456 payload_ratio:7.89x)\n\
             final_int8_zlib_roundtrip val_loss:8.4000 val_bpb:9.8000 eval_time:6432ms\n\
             final_int8_zlib_roundtrip_exact val_loss:8.40000000 val_bpb:9.80000000\n",
        )?;
        let config = ParameterGolfTrainGptReferenceRunConfig {
            run_id: String::from("upstream-sample"),
            source_log_path: log_path,
            device_name: String::from("NVIDIA H100 80GB HBM3"),
            dataset_manifest_digest: String::from("dataset-digest"),
            tokenizer_digest: String::from("tokenizer-digest"),
            geometry: sample_geometry(),
            final_validation_observed_ms: Some(12_345),
            final_roundtrip_eval_ms: None,
            peak_memory_allocated_mib: None,
            peak_memory_reserved_mib: None,
        };

        let receipt = build_parameter_golf_train_gpt_reference_run_receipt(&config)?;
        assert_eq!(receipt.train_step_observed_ms, Some(456));
        assert_eq!(receipt.final_validation_loss, Some(8.4321));
        assert_eq!(receipt.final_validation_bpb, Some(9.8765));
        assert_eq!(receipt.final_roundtrip_val_loss, Some(8.4));
        assert_eq!(receipt.final_roundtrip_val_bpb, Some(9.8));
        assert_eq!(receipt.final_roundtrip_eval_ms, Some(6_432));
        assert_eq!(receipt.peak_memory_allocated_mib, Some(22_345));
        assert_eq!(receipt.compressed_model_bytes, Some(78_958));
        Ok(())
    }

    #[test]
    fn same_node_parity_report_fails_closed_when_psionic_receipt_is_not_h100() {
        let psionic = sample_non_h100_psionic_report();
        let upstream = ParameterGolfTrainGptReferenceRunReceipt {
            schema_version: 1,
            report_id: String::from("parameter_golf.train_gpt_reference_run_receipt.v1"),
            run_id: String::from("upstream"),
            source_log_path: PathBuf::from("/tmp/train_gpt.log"),
            source_log_digest: String::from("digest"),
            device_name: String::from("NVIDIA H100 80GB HBM3"),
            dataset_manifest_digest: psionic.dataset_manifest_digest.clone(),
            tokenizer_digest: psionic.tokenizer_digest.tokenizer_digest.clone(),
            geometry: psionic.geometry.clone(),
            train_step_observed_ms: Some(456),
            final_validation_observed_ms: Some(1234),
            final_roundtrip_eval_ms: Some(5678),
            peak_memory_allocated_mib: Some(22345),
            peak_memory_reserved_mib: Some(22784),
            final_validation_loss: Some(8.4),
            final_validation_bpb: Some(9.8),
            final_roundtrip_val_loss: Some(8.4),
            final_roundtrip_val_bpb: Some(9.8),
            compressed_model_bytes: Some(78_958),
            claim_boundary: String::from("test"),
            report_digest: String::from("upstream-digest"),
        };

        let report = build_parameter_golf_same_node_parity_report(&psionic, &upstream);
        assert_eq!(
            report.disposition,
            ParameterGolfSameNodeParityDisposition::Blocked
        );
        assert!(report
            .blockers
            .iter()
            .any(|blocker| blocker.contains("single-H100 machine contract")));
    }
}
