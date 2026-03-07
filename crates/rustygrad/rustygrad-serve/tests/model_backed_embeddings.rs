use rustygrad_provider::{CapabilityEnvelope, ExecutionReceipt, ProviderReadiness, ReceiptStatus};
use rustygrad_serve::{
    ByteProjectionEmbedder, CpuModelEmbeddingsService, EmbeddingRequest, EmbeddingsExecutor,
};
use tempfile::tempdir;

#[test]
fn model_backed_embeddings_flow_returns_response_capability_and_receipt()
-> Result<(), Box<dyn std::error::Error>> {
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
        "cpu",
        service.model_descriptor(),
        ProviderReadiness::ready("cpu backend ready"),
    );
    let receipt = ExecutionReceipt::succeeded_for_response("cpu", &request, &response, 100, 120);

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

    assert_eq!(receipt.status, ReceiptStatus::Succeeded);
    assert_eq!(receipt.model_family, ByteProjectionEmbedder::MODEL_FAMILY);
    assert_eq!(receipt.model_revision, "v1");
    assert_eq!(receipt.weight_bundle.digest, request.model.weights.digest);
    assert_eq!(receipt.output_dimensions, 8);
    assert_eq!(receipt.output_vector_count, 2);
    assert!(receipt.failure_reason.is_none());
    Ok(())
}
