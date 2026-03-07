use rustygrad_backend_cpu::CpuBackend;
use rustygrad_backend_metal::{MetalBackend, EMBEDDINGS_SUPPORTED_OPS};
use rustygrad_runtime::{DeviceDiscovery, HealthStatus};

#[cfg(target_os = "macos")]
use rustygrad_backend_metal::EMBEDDINGS_PARITY_ABS_TOLERANCE;
#[cfg(target_os = "macos")]
use rustygrad_core::{DType, Device, Shape, TensorId};
#[cfg(target_os = "macos")]
use rustygrad_ir::{Graph, GraphBuilder};
#[cfg(target_os = "macos")]
use rustygrad_serve::{
    ByteProjectionEmbedder, CpuModelEmbeddingsService, EmbeddingNormalization, EmbeddingRequest,
    EmbeddingsExecutor,
};
#[cfg(target_os = "macos")]
use std::collections::BTreeMap;
#[cfg(target_os = "macos")]
use tempfile::tempdir;

#[cfg(not(target_os = "macos"))]
#[test]
fn metal_model_backed_embeddings_parity_reports_explicit_offline_state(
) -> Result<(), Box<dyn std::error::Error>> {
    let metal = MetalBackend::new();
    assert_eq!(metal.health().status, HealthStatus::Offline);

    let fallback = metal.fallback_selection(&CpuBackend::new(), EMBEDDINGS_SUPPORTED_OPS)?;
    assert_eq!(fallback.requested_backend, "metal");
    assert_eq!(fallback.effective_backend, "cpu");
    assert!(fallback.fallback_reason.is_some());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn metal_model_backed_embeddings_match_cpu_baseline_within_tolerance_on_ready_hardware(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let path = temp.path().join("byte_projection.safetensors");
    ByteProjectionEmbedder::write_default_safetensors_artifact(&path)?;

    let mut cpu_service = CpuModelEmbeddingsService::from_safetensors_artifact(&path)?;
    let request = EmbeddingRequest::new(
        "req-metal-parity-1",
        cpu_service.model_descriptor().clone(),
        vec![
            String::from("open agents"),
            String::from("open agents"),
            String::from("rusty grad"),
        ],
    );
    let cpu_response = cpu_service.embed(&request)?;

    let model = ByteProjectionEmbedder::from_safetensors_artifact(&path)?;
    let mut metal = MetalBackend::new();
    let Some(selected_device) = metal.selected_device().cloned() else {
        assert_ne!(metal.health().status, HealthStatus::Ready);
        let fallback = metal.fallback_selection(&CpuBackend::new(), EMBEDDINGS_SUPPORTED_OPS)?;
        assert_eq!(fallback.requested_backend, "metal");
        assert_eq!(fallback.effective_backend, "cpu");
        assert!(fallback.fallback_reason.is_some());
        return Ok(());
    };

    let (graph, input_id, output_id) = build_embedding_graph(selected_device.device, &model)?;
    assert_eq!(cpu_response.embeddings.len(), request.inputs.len());
    for (index, input) in request.inputs.iter().enumerate() {
        let actual = run_metal_embedding(&mut metal, &model, &graph, input_id, output_id, input)?;
        let expected = &cpu_response.embeddings[index].values;
        assert_eq!(actual.len(), expected.len());
        assert_vectors_close(
            expected,
            &actual,
            EMBEDDINGS_PARITY_ABS_TOLERANCE,
            format!("input[{index}]={input}").as_str(),
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn run_metal_embedding(
    backend: &mut MetalBackend,
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
        .ok_or("missing metal embeddings output")?;
    Ok(normalize_embedding(
        output.read_f32()?,
        model.descriptor().normalization,
    ))
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn assert_vectors_close(expected: &[f32], actual: &[f32], tolerance: f32, label: &str) {
    assert_eq!(
        expected.len(),
        actual.len(),
        "{label}: vector length mismatch"
    );
    for (index, (expected, actual)) in expected.iter().zip(actual.iter()).enumerate() {
        let delta = (expected - actual).abs();
        assert!(
            delta <= tolerance,
            "{label}: element {index} drift {delta} exceeded tolerance {tolerance}; expected {expected}, actual {actual}"
        );
    }
}
