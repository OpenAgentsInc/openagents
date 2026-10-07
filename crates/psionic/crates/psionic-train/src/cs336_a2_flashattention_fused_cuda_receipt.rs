use std::{collections::BTreeMap, fs, path::Path, time::Instant};

use psionic_backend_cuda::CudaBackend;
use psionic_core::{DType, Shape, TensorData, TensorId};
use psionic_ir::{
    AutodiffContext, AutodiffError, AutodiffGraphBuilder, GraphError, ReferenceEvaluationError,
    evaluate_graph,
};
use psionic_models::{
    Cs336A2FlashAttentionReferenceConfig, Cs336A2FlashAttentionReferenceError,
    cs336_a2_flash_attention_reference_backward, cs336_a2_flash_attention_reference_forward,
    cs336_a2_naive_attention_forward,
};
use psionic_runtime::{
    CompilePathEvidence, DeviceDescriptor, DeviceDiscovery, RuntimeError, RuntimeHealth,
};
use psionic_transformer::{AttentionMask, AttentionTensor4, AttentionTensorError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH,
    CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH, CS336_A2_REFERENCE_LANE_DOC_PATH,
};

pub const CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a2_flashattention_fused_cuda_receipt_v1.json";
pub const CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_SCHEMA_VERSION: &str =
    "psion.cs336_a2.flashattention_fused_cuda_receipt.v1";

#[derive(Debug, Error)]
pub enum Cs336A2FlashAttentionFusedCudaReceiptError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Model(#[from] Cs336A2FlashAttentionReferenceError),
    #[error(transparent)]
    AttentionTensor(#[from] AttentionTensorError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error(transparent)]
    Autodiff(#[from] AutodiffError),
    #[error(transparent)]
    ReferenceEvaluation(#[from] ReferenceEvaluationError),
    #[error("missing graph output `{0}`")]
    MissingOutput(&'static str),
    #[error("invalid CS336 A2 fused CUDA receipt: {0}")]
    InvalidReceipt(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionFusedCudaConfig {
    pub batch_size: usize,
    pub head_count: usize,
    pub sequence_length: usize,
    pub head_dim: usize,
    pub causal: bool,
    pub query_block_rows: usize,
    pub key_block_rows: usize,
    pub benchmark_iterations: usize,
    pub max_abs_tolerance: f32,
}

impl Default for Cs336A2FlashAttentionFusedCudaConfig {
    fn default() -> Self {
        let tiled = Cs336A2FlashAttentionReferenceConfig::bounded_default();
        Self {
            batch_size: 1,
            head_count: 2,
            sequence_length: 4,
            head_dim: 8,
            causal: true,
            query_block_rows: tiled.query_block_rows,
            key_block_rows: tiled.key_block_rows,
            benchmark_iterations: 32,
            max_abs_tolerance: 3e-2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionFusedCudaCapability {
    pub backend: String,
    pub supports_bounded_fused_attention: bool,
    pub health: RuntimeHealth,
    pub selected_device: Option<DeviceDescriptor>,
    pub refusal_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionRouteBenchmark {
    pub route_id: String,
    pub benchmark_iterations: usize,
    pub average_elapsed_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionFusedRouteBenchmark {
    pub route_id: String,
    pub benchmark_iterations: usize,
    pub average_elapsed_ms: f64,
    pub kernel_count: usize,
    pub bytes_moved: u64,
    pub plan_cache_hits: usize,
    pub plan_cache_misses: usize,
    pub execution_plan_digest: Option<String>,
    pub compile_path: Option<CompilePathEvidence>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionFusedCudaBenchmark {
    pub naive_forward: Cs336A2FlashAttentionRouteBenchmark,
    pub reference_forward: Cs336A2FlashAttentionRouteBenchmark,
    pub fused_forward: Option<Cs336A2FlashAttentionFusedRouteBenchmark>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionFusedCudaCorrectness {
    pub output_max_abs_diff: f32,
    pub d_query_max_abs_diff: f32,
    pub d_key_max_abs_diff: f32,
    pub d_value_max_abs_diff: f32,
    pub reference_output_digest: String,
    pub fused_output_digest: String,
    pub reference_d_query_digest: String,
    pub fused_d_query_digest: String,
    pub reference_d_key_digest: String,
    pub fused_d_key_digest: String,
    pub reference_d_value_digest: String,
    pub fused_d_value_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionFusedCudaReceipt {
    pub schema_version: String,
    pub reference_lane_doc_path: String,
    pub baseline_profile_bundle_path: String,
    pub reference_receipt_path: String,
    pub config: Cs336A2FlashAttentionFusedCudaConfig,
    pub capability: Cs336A2FlashAttentionFusedCudaCapability,
    pub benchmark: Cs336A2FlashAttentionFusedCudaBenchmark,
    pub correctness: Option<Cs336A2FlashAttentionFusedCudaCorrectness>,
    pub claim_boundary: String,
}

pub fn build_cs336_a2_flashattention_fused_cuda_receipt()
-> Result<Cs336A2FlashAttentionFusedCudaReceipt, Cs336A2FlashAttentionFusedCudaReceiptError> {
    let config = Cs336A2FlashAttentionFusedCudaConfig::default();
    let tensor_shape = [
        config.batch_size,
        config.head_count,
        config.sequence_length,
        config.head_dim,
    ];
    let query = deterministic_attention_tensor(tensor_shape, -0.25, 0.0175)?;
    let key = deterministic_attention_tensor(tensor_shape, 0.125, -0.0125)?;
    let value = deterministic_attention_tensor(tensor_shape, -0.4, 0.0225)?;
    let grad_output = deterministic_attention_tensor(tensor_shape, 0.05, 0.00625)?;
    let mask = config.causal.then(|| {
        AttentionMask::causal(
            config.batch_size,
            config.sequence_length,
            config.sequence_length,
        )
    });
    let reference_config = Cs336A2FlashAttentionReferenceConfig {
        query_block_rows: config.query_block_rows,
        key_block_rows: config.key_block_rows,
    };
    let reference_forward = cs336_a2_flash_attention_reference_forward(
        &query,
        &key,
        &value,
        mask.as_ref(),
        reference_config,
    )?;
    let reference_backward = cs336_a2_flash_attention_reference_backward(
        &query,
        &key,
        &value,
        &reference_forward,
        &grad_output,
        mask.as_ref(),
        reference_config,
    )?;
    let iterations = config.benchmark_iterations.max(1);
    let benchmark = Cs336A2FlashAttentionFusedCudaBenchmark {
        naive_forward: Cs336A2FlashAttentionRouteBenchmark {
            route_id: String::from("naive_attention_forward_cpu"),
            benchmark_iterations: iterations,
            average_elapsed_ms: benchmark_route_ms(iterations, || {
                let _ = cs336_a2_naive_attention_forward(&query, &key, &value, mask.as_ref())?;
                Ok(())
            })?,
        },
        reference_forward: Cs336A2FlashAttentionRouteBenchmark {
            route_id: String::from("flashattention_reference_forward_cpu"),
            benchmark_iterations: iterations,
            average_elapsed_ms: benchmark_route_ms(iterations, || {
                let _ = cs336_a2_flash_attention_reference_forward(
                    &query,
                    &key,
                    &value,
                    mask.as_ref(),
                    reference_config,
                )?;
                Ok(())
            })?,
        },
        fused_forward: None,
    };

    let mut backend = CudaBackend::new();
    let health = backend.health();
    let selected_device = backend.selected_device().cloned();
    let supports_bounded_fused_attention = backend.supports_bounded_scaled_dot_product_attention();
    let capability = Cs336A2FlashAttentionFusedCudaCapability {
        backend: String::from(backend.backend_name()),
        supports_bounded_fused_attention,
        health: health.clone(),
        selected_device: selected_device.clone(),
        refusal_reason: backend.bounded_scaled_dot_product_attention_refusal_reason(),
    };
    let claim_boundary = String::from(
        "This receipt covers the bounded CS336 A2 fused CUDA attention tranche inside psionic. On admitted CUDA hardware it records correctness and bounded benchmark comparisons against the owned tiled reference path. On hosts without that CUDA path it records an explicit refusal instead of pretending the fused path ran. It does not claim actual-lane broader pretraining closure.",
    );
    if !supports_bounded_fused_attention {
        let receipt = Cs336A2FlashAttentionFusedCudaReceipt {
            schema_version: String::from(CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_SCHEMA_VERSION),
            reference_lane_doc_path: String::from(CS336_A2_REFERENCE_LANE_DOC_PATH),
            baseline_profile_bundle_path: String::from(
                CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH,
            ),
            reference_receipt_path: String::from(
                CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH,
            ),
            config,
            capability,
            benchmark,
            correctness: None,
            claim_boundary,
        };
        validate_receipt(&receipt)?;
        return Ok(receipt);
    }

    let selected_device = selected_device.ok_or_else(|| {
        Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
            "CUDA receipt marked available without a selected device".into(),
        )
    })?;
    let shape = Shape::new(vec![
        config.batch_size,
        config.head_count,
        config.sequence_length,
        config.head_dim,
    ]);
    let scale = 1.0_f32 / (config.head_dim as f32).sqrt();
    let query_values = query.values().to_vec();
    let key_values = key.values().to_vec();
    let value_values = value.values().to_vec();
    let grad_output_values = grad_output.values().to_vec();

    let mut builder = AutodiffGraphBuilder::with_context(
        selected_device.device.clone(),
        AutodiffContext::training(),
    );
    let query_tensor = builder.input("query", shape.clone(), DType::BF16, true);
    let key_tensor = builder.input("key", shape.clone(), DType::BF16, true);
    let value_tensor = builder.input("value", shape.clone(), DType::BF16, true);
    let attended = builder.scaled_dot_product_attention(
        &query_tensor,
        &key_tensor,
        &value_tensor,
        scale,
        config.causal,
    )?;
    let autodiff_graph = builder.finish(vec![attended.clone()]);
    let forward_inputs = BTreeMap::from([
        (
            query_tensor.id(),
            backend.input_bf16_buffer(shape.clone(), query_values.clone())?,
        ),
        (
            key_tensor.id(),
            backend.input_bf16_buffer(shape.clone(), key_values.clone())?,
        ),
        (
            value_tensor.id(),
            backend.input_bf16_buffer(shape.clone(), value_values.clone())?,
        ),
    ]);
    let _warm_forward = backend.compile_and_execute(autodiff_graph.graph(), &forward_inputs)?;
    let (fused_average_elapsed_ms, fused_forward_result) = benchmark_cuda_forward_ms(
        &mut backend,
        autodiff_graph.graph(),
        &forward_inputs,
        iterations,
    )?;
    let fused_output = fused_forward_result
        .outputs
        .get(&attended.id())
        .ok_or(Cs336A2FlashAttentionFusedCudaReceiptError::MissingOutput(
            "forward_attention_output",
        ))?
        .read_bf16_to_f32()?;

    let backward_plan = autodiff_graph.backward_plan(attended.id())?;
    let primal_inputs = BTreeMap::from([
        (query_tensor.id(), TensorData::BF16(query_values.clone())),
        (key_tensor.id(), TensorData::BF16(key_values.clone())),
        (value_tensor.id(), TensorData::BF16(value_values.clone())),
    ]);
    let forward_values = evaluate_graph(autodiff_graph.graph(), &primal_inputs)?;
    let mut backward_inputs = BTreeMap::new();
    for binding in &backward_plan.primal_bindings {
        let value = forward_values.get(&binding.primal_tensor).ok_or(
            Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
                "missing forward value for backward binding".into(),
            ),
        )?;
        let spec = backward_plan
            .gradient_graph
            .node(binding.gradient_graph_input)
            .ok_or(Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
                "missing backward graph input node".into(),
            ))?
            .tensor()
            .spec()
            .clone();
        let buffer = match value {
            TensorData::F32(values) => {
                backend.input_buffer(spec.shape().clone(), values.clone())?
            }
            TensorData::BF16(values) => {
                backend.input_bf16_buffer(spec.shape().clone(), values.clone())?
            }
            TensorData::I32(values) => {
                backend.input_i32_buffer(spec.shape().clone(), values.clone())?
            }
            TensorData::QuantizedBlocks(_) => {
                return Err(Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
                    "unexpected quantized backward binding value".into(),
                ));
            }
        };
        backward_inputs.insert(binding.gradient_graph_input, buffer);
    }
    let seed_shape = backward_plan
        .gradient_graph
        .node(backward_plan.seed_input)
        .ok_or(Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
            "missing backward seed node".into(),
        ))?
        .tensor()
        .spec()
        .shape()
        .clone();
    backward_inputs.insert(
        backward_plan.seed_input,
        backend.input_bf16_buffer(seed_shape, grad_output_values)?,
    );
    let backward_result =
        backend.compile_and_execute(&backward_plan.gradient_graph, &backward_inputs)?;
    let d_query = read_bf16_output(
        &backward_result.outputs,
        backward_plan.gradient_for(query_tensor.id()).ok_or(
            Cs336A2FlashAttentionFusedCudaReceiptError::MissingOutput("query_gradient_id"),
        )?,
        "query_gradient",
    )?;
    let d_key = read_bf16_output(
        &backward_result.outputs,
        backward_plan.gradient_for(key_tensor.id()).ok_or(
            Cs336A2FlashAttentionFusedCudaReceiptError::MissingOutput("key_gradient_id"),
        )?,
        "key_gradient",
    )?;
    let d_value = read_bf16_output(
        &backward_result.outputs,
        backward_plan.gradient_for(value_tensor.id()).ok_or(
            Cs336A2FlashAttentionFusedCudaReceiptError::MissingOutput("value_gradient_id"),
        )?,
        "value_gradient",
    )?;
    let correctness = Cs336A2FlashAttentionFusedCudaCorrectness {
        output_max_abs_diff: max_abs_diff(&fused_output, reference_forward.output.values()),
        d_query_max_abs_diff: max_abs_diff(&d_query, reference_backward.d_query.values()),
        d_key_max_abs_diff: max_abs_diff(&d_key, reference_backward.d_key.values()),
        d_value_max_abs_diff: max_abs_diff(&d_value, reference_backward.d_value.values()),
        reference_output_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention.reference.output",
            &reference_forward.output,
        ),
        fused_output_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention.fused_cuda.output",
            &fused_output,
        ),
        reference_d_query_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention.reference.d_query",
            &reference_backward.d_query,
        ),
        fused_d_query_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention.fused_cuda.d_query",
            &d_query,
        ),
        reference_d_key_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention.reference.d_key",
            &reference_backward.d_key,
        ),
        fused_d_key_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention.fused_cuda.d_key",
            &d_key,
        ),
        reference_d_value_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention.reference.d_value",
            &reference_backward.d_value,
        ),
        fused_d_value_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention.fused_cuda.d_value",
            &d_value,
        ),
    };
    let benchmark = Cs336A2FlashAttentionFusedCudaBenchmark {
        fused_forward: Some(Cs336A2FlashAttentionFusedRouteBenchmark {
            route_id: String::from("cuda_backend_bf16_scaled_dot_product_attention"),
            benchmark_iterations: iterations,
            average_elapsed_ms: fused_average_elapsed_ms,
            kernel_count: fused_forward_result.metrics.kernel_count,
            bytes_moved: fused_forward_result.metrics.bytes_moved,
            plan_cache_hits: fused_forward_result.metrics.plan_cache_hits,
            plan_cache_misses: fused_forward_result.metrics.plan_cache_misses,
            execution_plan_digest: fused_forward_result.metrics.execution_plan_digest.clone(),
            compile_path: fused_forward_result.metrics.compile_path.clone(),
        }),
        ..benchmark
    };
    let receipt = Cs336A2FlashAttentionFusedCudaReceipt {
        schema_version: String::from(CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_SCHEMA_VERSION),
        reference_lane_doc_path: String::from(CS336_A2_REFERENCE_LANE_DOC_PATH),
        baseline_profile_bundle_path: String::from(CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH),
        reference_receipt_path: String::from(
            CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH,
        ),
        config,
        capability,
        benchmark,
        correctness: Some(correctness),
        claim_boundary,
    };
    validate_receipt(&receipt)?;
    Ok(receipt)
}

pub fn write_cs336_a2_flashattention_fused_cuda_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2FlashAttentionFusedCudaReceipt, Cs336A2FlashAttentionFusedCudaReceiptError> {
    let receipt = build_cs336_a2_flashattention_fused_cuda_receipt()?;
    let receipt_path = repo_root
        .as_ref()
        .join(CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_FIXTURE_PATH);
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt)
}

fn validate_receipt(
    receipt: &Cs336A2FlashAttentionFusedCudaReceipt,
) -> Result<(), Cs336A2FlashAttentionFusedCudaReceiptError> {
    if receipt.schema_version != CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_SCHEMA_VERSION {
        return Err(Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
            format!(
                "expected schema version `{CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_SCHEMA_VERSION}`, got `{}`",
                receipt.schema_version
            ),
        ));
    }
    if receipt.benchmark.naive_forward.average_elapsed_ms < 0.0
        || receipt.benchmark.reference_forward.average_elapsed_ms < 0.0
    {
        return Err(Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
            "CPU benchmark timings must be non-negative".into(),
        ));
    }
    if receipt.capability.supports_bounded_fused_attention {
        let correctness = receipt.correctness.as_ref().ok_or_else(|| {
            Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
                "CUDA-capable receipt must include correctness data".into(),
            )
        })?;
        let fused = receipt.benchmark.fused_forward.as_ref().ok_or_else(|| {
            Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
                "CUDA-capable receipt must include fused benchmark data".into(),
            )
        })?;
        for (label, diff) in [
            ("output", correctness.output_max_abs_diff),
            ("d_query", correctness.d_query_max_abs_diff),
            ("d_key", correctness.d_key_max_abs_diff),
            ("d_value", correctness.d_value_max_abs_diff),
        ] {
            if diff > receipt.config.max_abs_tolerance {
                return Err(Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
                    format!(
                        "{label} diff {diff} exceeds tolerance {}",
                        receipt.config.max_abs_tolerance
                    ),
                ));
            }
        }
        if fused.average_elapsed_ms < 0.0 {
            return Err(Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
                "fused benchmark timing must be non-negative".into(),
            ));
        }
    } else {
        if receipt.capability.refusal_reason.is_none() {
            return Err(Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
                "refused receipt must include a refusal reason".into(),
            ));
        }
        if receipt.correctness.is_some() || receipt.benchmark.fused_forward.is_some() {
            return Err(Cs336A2FlashAttentionFusedCudaReceiptError::InvalidReceipt(
                "refused receipt must not claim fused correctness or fused benchmark data".into(),
            ));
        }
    }
    Ok(())
}

fn benchmark_route_ms<F>(
    iterations: usize,
    mut route: F,
) -> Result<f64, Cs336A2FlashAttentionFusedCudaReceiptError>
where
    F: FnMut() -> Result<(), Cs336A2FlashAttentionFusedCudaReceiptError>,
{
    let mut total_ms = 0.0;
    for _ in 0..iterations.max(1) {
        let started = Instant::now();
        route()?;
        total_ms += started.elapsed().as_secs_f64() * 1_000.0;
    }
    Ok(total_ms / iterations.max(1) as f64)
}

fn benchmark_cuda_forward_ms(
    backend: &mut CudaBackend,
    graph: &psionic_ir::Graph,
    inputs: &BTreeMap<TensorId, psionic_backend_cuda::CudaBuffer>,
    iterations: usize,
) -> Result<
    (
        f64,
        psionic_runtime::ExecutionResult<psionic_backend_cuda::CudaBuffer>,
    ),
    Cs336A2FlashAttentionFusedCudaReceiptError,
> {
    let mut total_ms = 0.0;
    let mut last_result = backend.compile_and_execute(graph, inputs)?;
    for _ in 0..iterations.max(1) {
        let started = Instant::now();
        last_result = backend.compile_and_execute(graph, inputs)?;
        total_ms += started.elapsed().as_secs_f64() * 1_000.0;
    }
    Ok((total_ms / iterations.max(1) as f64, last_result))
}

fn deterministic_attention_tensor(
    shape: [usize; 4],
    start: f32,
    step: f32,
) -> Result<AttentionTensor4, Cs336A2FlashAttentionFusedCudaReceiptError> {
    let mut values = Vec::with_capacity(shape.iter().product());
    for index in 0..shape.iter().product::<usize>() {
        values.push(start + index as f32 * step);
    }
    Ok(AttentionTensor4::new(shape, values)?)
}

fn read_bf16_output(
    outputs: &BTreeMap<TensorId, psionic_backend_cuda::CudaBuffer>,
    tensor_id: TensorId,
    label: &'static str,
) -> Result<Vec<f32>, Cs336A2FlashAttentionFusedCudaReceiptError> {
    outputs
        .get(&tensor_id)
        .ok_or(Cs336A2FlashAttentionFusedCudaReceiptError::MissingOutput(
            label,
        ))?
        .read_bf16_to_f32()
        .map_err(Into::into)
}

fn max_abs_diff(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| (left - right).abs())
        .fold(0.0f32, f32::max)
}

fn stable_json_digest<T: Serialize>(domain: &[u8], value: &T) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(serde_json::to_vec(value).expect("serializing stable digest input must succeed"));
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_FIXTURE_PATH,
        build_cs336_a2_flashattention_fused_cuda_receipt,
        write_cs336_a2_flashattention_fused_cuda_receipt,
    };

    #[test]
    fn fused_cuda_receipt_is_executed_or_refused_honestly() -> Result<(), Box<dyn std::error::Error>>
    {
        let receipt = build_cs336_a2_flashattention_fused_cuda_receipt()?;
        if receipt.capability.supports_bounded_fused_attention {
            let correctness = receipt.correctness.as_ref().ok_or("missing correctness")?;
            assert!(correctness.output_max_abs_diff <= receipt.config.max_abs_tolerance);
            assert!(correctness.d_query_max_abs_diff <= receipt.config.max_abs_tolerance);
            assert!(receipt.benchmark.fused_forward.is_some());
        } else {
            assert!(receipt.capability.refusal_reason.is_some());
            assert!(receipt.correctness.is_none());
            assert!(receipt.benchmark.fused_forward.is_none());
        }
        assert!(receipt.benchmark.naive_forward.average_elapsed_ms >= 0.0);
        assert!(receipt.benchmark.reference_forward.average_elapsed_ms >= 0.0);
        Ok(())
    }

    #[test]
    fn fused_cuda_writer_emits_json_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let receipt = write_cs336_a2_flashattention_fused_cuda_receipt(temp.path())?;
        let fixture_path = temp
            .path()
            .join(CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_FIXTURE_PATH);
        assert!(fixture_path.exists());
        let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&fixture_path)?)?;
        assert_eq!(
            written["schema_version"].as_str(),
            Some(receipt.schema_version.as_str())
        );
        Ok(())
    }
}
