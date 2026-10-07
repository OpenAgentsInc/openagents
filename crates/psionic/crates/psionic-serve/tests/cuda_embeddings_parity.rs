#![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

use std::collections::BTreeMap;

use psionic_backend_cpu::CpuBackend;
use psionic_backend_cuda::{CudaBackend, EMBEDDINGS_SUPPORTED_OPS};
use psionic_core::{DType, Device, QuantizationMode, Shape, TensorId};
use psionic_ir::{Graph, GraphBuilder};
use psionic_runtime::{
    BackendParityPolicy, compare_embedding_vectors, validation_reference_for_served_product,
};
use psionic_serve::{
    ByteProjectionEmbedder, CpuModelEmbeddingsService, EmbeddingNormalization, EmbeddingRequest,
    EmbeddingsExecutor,
};
use tempfile::tempdir;

#[test]
fn cuda_model_backed_embeddings_match_cpu_baseline_within_tolerance_or_report_explicit_fallback()
-> Result<(), Box<dyn std::error::Error>> {
    let drift_budget = BackendParityPolicy::default().embedding_budget(QuantizationMode::None);
    let temp = tempdir()?;
    let path = temp.path().join("byte_projection.safetensors");
    ByteProjectionEmbedder::write_default_safetensors_artifact(&path)?;

    let mut cpu_service = CpuModelEmbeddingsService::from_safetensors_artifact(&path)?;
    let request = EmbeddingRequest::new(
        "req-cuda-parity-1",
        cpu_service.model_descriptor().clone(),
        vec![
            String::from("open agents"),
            String::from("open agents"),
            String::from("rusty grad"),
        ],
    );
    let cpu_response = cpu_service.embed(&request)?;

    let model = ByteProjectionEmbedder::from_safetensors_artifact(&path)?;
    let mut cuda = CudaBackend::new();
    let Some(selected_device) = cuda.selected_device().cloned() else {
        let fallback = cuda.fallback_selection(&CpuBackend::new(), EMBEDDINGS_SUPPORTED_OPS)?;
        assert_eq!(fallback.requested_backend, "cuda");
        assert_eq!(fallback.effective_backend, "cpu");
        assert!(fallback.fallback_reason.is_some());
        assert_eq!(
            validation_reference_for_served_product(&fallback, "psionic.embeddings").claim_id,
            "cuda.refusal.unavailable"
        );
        return Ok(());
    };

    let (graph, input_id, output_id) = build_embedding_graph(selected_device.device, &model)?;
    assert_eq!(cpu_response.embeddings.len(), request.inputs.len());
    for (index, input) in request.inputs.iter().enumerate() {
        let actual = run_cuda_embedding(&mut cuda, &model, &graph, input_id, output_id, input)?;
        let expected = &cpu_response.embeddings[index].values;
        let summary = compare_embedding_vectors(expected, &actual, drift_budget)?;
        assert!(
            summary.within_budget,
            "input[{index}]={input}: max_abs_delta={} max_rel_delta={} cosine_similarity={} budget={:?}",
            summary.max_abs_delta, summary.max_rel_delta, summary.cosine_similarity, drift_budget,
        );
    }
    Ok(())
}

fn build_embedding_graph(
    device: Device,
    model: &ByteProjectionEmbedder,
) -> Result<(Graph, TensorId, TensorId), Box<dyn std::error::Error>> {
    let mut builder = GraphBuilder::new(device);
    let input = builder.input(
        "features",
        Shape::new(vec![1, model.input_dimensions()]),
        DType::F32,
    );
    let weights = builder.constant_f32(
        Shape::new(vec![
            model.input_dimensions(),
            model.descriptor().dimensions,
        ]),
        model.weights().projection().to_vec(),
    )?;
    let bias = builder.constant_f32(
        Shape::new(vec![1, model.descriptor().dimensions]),
        model.weights().bias().to_vec(),
    )?;
    let projected = builder.matmul(&input, &weights)?;
    let shifted = builder.add(&projected, &bias)?;
    let output_id = shifted.id();
    Ok((builder.finish(vec![shifted]), input.id(), output_id))
}

fn run_cuda_embedding(
    backend: &mut CudaBackend,
    model: &ByteProjectionEmbedder,
    graph: &Graph,
    input_id: TensorId,
    output_id: TensorId,
    input: &str,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        input_id,
        backend.input_buffer(
            Shape::new(vec![1, model.input_dimensions()]),
            model.featurize(input),
        )?,
    );

    let result = backend.compile_and_execute(graph, &inputs)?;
    let output = result
        .outputs
        .get(&output_id)
        .ok_or("missing cuda embeddings output")?;
    Ok(normalize_embedding(
        output.read_f32()?,
        model.descriptor().normalization,
    ))
}

fn normalize_embedding(values: Vec<f32>, normalization: EmbeddingNormalization) -> Vec<f32> {
    if normalization != EmbeddingNormalization::UnitLength {
        return values;
    }

    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 {
        return values;
    }

    values.into_iter().map(|value| value / norm).collect()
}
