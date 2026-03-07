use mox_provider::{
    CapabilityEnvelope, ExecutionReceipt, ProviderReadiness, ReceiptStatus,
};
use mox_serve::{EmbeddingRequest, EmbeddingsExecutor, SmokeEmbeddingsService};

#[test]
fn smoke_embeddings_flow_returns_response_capability_and_receipt(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut service = SmokeEmbeddingsService::new()?;
    let request = EmbeddingRequest::new(
        "req-smoke-1",
        service.model_descriptor().clone(),
        vec![String::from("hello world"), String::from("hello world")],
    );

    let response = service.embed(&request)?;
    let capability = CapabilityEnvelope::from_embedding_model(
        "cpu",
        service.model_descriptor(),
        ProviderReadiness::ready("cpu backend ready"),
    );
    let receipt = ExecutionReceipt::succeeded_for_response("cpu", &request, &response, 100, 120);

    assert_eq!(response.metadata.model_id, "smoke-byte-embed-v0");
    assert_eq!(response.metadata.dimensions, 8);
    assert_eq!(response.embeddings.len(), 2);
    assert_eq!(response.embeddings[0].values, response.embeddings[1].values);

    assert_eq!(capability.product_id, "mox.embeddings");
    assert_eq!(capability.runtime_backend, "cpu");
    assert_eq!(capability.dimensions, 8);

    assert_eq!(receipt.status, ReceiptStatus::Succeeded);
    assert_eq!(receipt.output_dimensions, 8);
    assert_eq!(receipt.output_vector_count, 2);
    assert!(receipt.failure_reason.is_none());
    assert_eq!(receipt.request_id, request.request_id);
    Ok(())
}
