use rustygrad_backend_cpu::CpuBackend;
use rustygrad_provider::{CapabilityEnvelope, ExecutionReceipt, ProviderReadiness, ReceiptStatus};
use rustygrad_serve::{
    ByteProjectionEmbedder, CpuModelEmbeddingsService, EmbeddingRequest, EmbeddingsExecutor,
    ModelEmbeddingsError, SmokeByteEmbedder,
};
use tempfile::tempdir;

#[test]
fn model_backed_embeddings_flow_returns_response_capability_and_receipt(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let path = temp.path().join("byte_projection.safetensors");
    ByteProjectionEmbedder::write_default_safetensors_artifact(&path)?;

    let mut service = CpuModelEmbeddingsService::from_safetensors_artifact(&path)?;
    let request = EmbeddingRequest::new(
        "req-model-1",
        service.model_descriptor().clone(),
        vec![String::from("open agents"), String::from("open agents")],
    );

    let response = service.embed(&request)?;
    let capability = CapabilityEnvelope::from_embedding_model(
        cpu_backend_selection()?,
        service.model_descriptor(),
        ProviderReadiness::ready("cpu backend ready"),
    );
    let receipt = ExecutionReceipt::succeeded_for_response(
        cpu_backend_selection()?,
        &request,
        &response,
        100,
        120,
    );

    assert_eq!(response.metadata.model_id, ByteProjectionEmbedder::MODEL_ID);
    assert_eq!(response.metadata.dimensions, 8);
    assert_eq!(
        response.metadata.normalization,
        rustygrad_serve::EmbeddingNormalization::UnitLength
    );
    assert_eq!(response.embeddings.len(), 2);
    assert_eq!(response.embeddings[0].values, response.embeddings[1].values);
    let norm = response.embeddings[0]
        .values
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    assert!((norm - 1.0).abs() < 1.0e-5);

    assert_eq!(capability.product_id, "rustygrad.embeddings");
    assert_eq!(capability.runtime_backend, "cpu");
    assert_eq!(capability.backend_selection.requested_backend, "cpu");
    assert!(capability.backend_selection.fallback_reason.is_none());
    assert_eq!(capability.model_id, ByteProjectionEmbedder::MODEL_ID);
    assert_eq!(
        capability.model_family,
        ByteProjectionEmbedder::MODEL_FAMILY
    );
    assert_eq!(capability.model_revision, "v1");
    assert_eq!(
        capability.weight_bundle.digest,
        request.model.weights.digest
    );
    assert_eq!(
        capability.weight_bundle.quantization,
        rustygrad_serve::QuantizationMode::None
    );
    assert_eq!(capability.weight_bundle.artifacts.len(), 1);
    let capability_json = serde_json::to_string_pretty(&capability)?;
    assert!(capability_json.contains("\"model_revision\": \"v1\""));
    assert!(capability_json.contains("\"weight_bundle\""));

    assert_eq!(receipt.status, ReceiptStatus::Succeeded);
    assert_eq!(receipt.backend_selection.effective_backend, "cpu");
    assert_eq!(receipt.model_family, ByteProjectionEmbedder::MODEL_FAMILY);
    assert_eq!(receipt.model_revision, "v1");
    assert_eq!(receipt.weight_bundle.digest, request.model.weights.digest);
    assert_eq!(receipt.output_dimensions, 8);
    assert_eq!(receipt.output_vector_count, 2);
    assert!(receipt.failure_reason.is_none());
    let receipt_json = serde_json::to_string_pretty(&receipt)?;
    assert!(receipt_json.contains("\"weight_bundle\""));
    Ok(())
}

#[test]
fn model_backed_embeddings_service_reports_missing_artifact() {
    let error = CpuModelEmbeddingsService::from_safetensors_artifact(
        "/tmp/definitely-missing-rustygrad-byte-projection.safetensors",
    )
    .expect_err("missing artifact should fail");

    assert!(matches!(
        error,
        ModelEmbeddingsError::Model(rustygrad_serve::ModelLoadError::ArtifactRead { .. })
    ));
}

#[test]
fn model_backed_embeddings_reject_reference_descriptor_without_fallback(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let path = temp.path().join("byte_projection.safetensors");
    ByteProjectionEmbedder::write_default_safetensors_artifact(&path)?;

    let mut service = CpuModelEmbeddingsService::from_safetensors_artifact(&path)?;
    let request = EmbeddingRequest::new(
        "req-model-bad-model",
        SmokeByteEmbedder::new().descriptor().clone(),
        vec![String::from("hello world")],
    );

    let error = service
        .embed(&request)
        .expect_err("wrong model should fail");
    assert!(matches!(
        error,
        ModelEmbeddingsError::UnsupportedModel(model_id)
            if model_id == SmokeByteEmbedder::MODEL_ID
    ));
    Ok(())
}

fn cpu_backend_selection(
) -> Result<rustygrad_runtime::BackendSelection, rustygrad_runtime::RuntimeError> {
    CpuBackend::new().backend_selection(&["input", "constant", "matmul", "add"])
}

#[test]
fn model_backed_embeddings_reject_wrong_product() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let path = temp.path().join("byte_projection.safetensors");
    ByteProjectionEmbedder::write_default_safetensors_artifact(&path)?;

    let mut service = CpuModelEmbeddingsService::from_safetensors_artifact(&path)?;
    let mut request = EmbeddingRequest::new(
        "req-model-bad-product",
        service.model_descriptor().clone(),
        vec![String::from("hello world")],
    );
    request.product_id = String::from("rustygrad.text_generation");

    let error = service
        .embed(&request)
        .expect_err("wrong product should fail");
    assert!(matches!(
        error,
        ModelEmbeddingsError::UnsupportedProduct(product_id)
            if product_id == "rustygrad.text_generation"
    ));
    Ok(())
}
