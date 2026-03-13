#![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

use psionic_backend_cpu::CpuBackend;
use psionic_provider::{CapabilityEnvelope, ExecutionReceipt, ProviderReadiness, ReceiptStatus};
use psionic_serve::{EmbeddingRequest, EmbeddingsExecutor, SmokeEmbeddingsService};

#[test]
fn smoke_embeddings_flow_returns_response_capability_and_receipt()
-> Result<(), Box<dyn std::error::Error>> {
    let mut service = SmokeEmbeddingsService::new()?;
    let request = EmbeddingRequest::new(
        "req-smoke-1",
        service.model_descriptor().clone(),
        vec![String::from("hello world"), String::from("hello world")],
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

    assert_eq!(response.metadata.model_id, "smoke-byte-embed-v0");
    assert_eq!(response.metadata.model_family, "smoke");
    assert_eq!(response.metadata.model_revision, "v0");
    assert_eq!(response.metadata.dimensions, 8);
    assert_eq!(response.metadata.input_count, 2);
    assert_eq!(response.metadata.requested_output_dimensions, None);
    assert_eq!(response.embeddings.len(), 2);
    assert_eq!(response.embeddings[0].values, response.embeddings[1].values);

    assert_eq!(capability.product_id, "psionic.embeddings");
    assert_eq!(capability.runtime_backend, "cpu");
    assert_eq!(capability.backend_selection.requested_backend, "cpu");
    assert!(capability.backend_selection.fallback_reason.is_none());
    assert_eq!(capability.model_id, "smoke-byte-embed-v0");
    assert_eq!(capability.model_family, "smoke");
    assert_eq!(capability.model_revision, "v0");
    assert_eq!(
        capability.weight_bundle.digest,
        request.model.weights.digest
    );
    assert!(capability.weight_bundle.artifacts.is_empty());
    assert_eq!(capability.dimensions, 8);
    assert_eq!(
        capability.normalization,
        psionic_serve::EmbeddingNormalization::None
    );
    assert!(capability.preserves_input_order);
    assert!(capability.empty_batch_returns_empty);
    assert!(capability.supports_output_dimensions);
    assert!(!capability.supports_input_truncation);

    assert_eq!(receipt.status, ReceiptStatus::Succeeded);
    assert_eq!(receipt.backend_selection.effective_backend, "cpu");
    assert_eq!(receipt.model_id, "smoke-byte-embed-v0");
    assert_eq!(receipt.model_family, "smoke");
    assert_eq!(receipt.model_revision, "v0");
    assert_eq!(receipt.weight_bundle.digest, request.model.weights.digest);
    assert_eq!(receipt.output_dimensions, 8);
    assert_eq!(receipt.input_count, 2);
    assert_eq!(receipt.output_vector_count, 2);
    assert_eq!(
        receipt.normalization,
        psionic_serve::EmbeddingNormalization::None
    );
    assert_eq!(receipt.requested_output_dimensions, None);
    assert!(receipt.failure_reason.is_none());
    assert_eq!(receipt.request_id, request.request_id);
    Ok(())
}

#[test]
fn smoke_embeddings_empty_batch_returns_empty_success() -> Result<(), Box<dyn std::error::Error>> {
    let mut service = SmokeEmbeddingsService::new()?;
    let request = EmbeddingRequest::new(
        "req-smoke-empty",
        service.model_descriptor().clone(),
        vec![],
    );

    let response = service.embed(&request)?;
    assert!(response.embeddings.is_empty());
    assert_eq!(response.metadata.vector_count, 0);
    assert_eq!(response.metadata.input_count, 0);
    assert_eq!(response.metadata.dimensions, 8);
    Ok(())
}

fn cpu_backend_selection()
-> Result<psionic_runtime::BackendSelection, psionic_runtime::RuntimeError> {
    CpuBackend::new().backend_selection(&["input", "constant", "matmul", "add"])
}
