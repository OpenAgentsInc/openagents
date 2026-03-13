#![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

use psionic_provider::{CapabilityEnvelope, ExecutionReceipt, ProviderReadiness, ReceiptStatus};
use psionic_runtime::HealthStatus;
use psionic_serve::{
    ByteProjectionEmbedder, EmbeddingRequest, EmbeddingsExecutor, MetalEmbeddingsError,
    MetalModelEmbeddingsService,
};
use tempfile::tempdir;

#[test]
fn metal_model_backed_embeddings_flow_returns_response_capability_and_receipt_or_explicit_unavailability()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let path = temp.path().join("byte_projection.safetensors");
    ByteProjectionEmbedder::write_default_safetensors_artifact(&path)?;

    match MetalModelEmbeddingsService::from_safetensors_artifact(&path) {
        Ok(mut service) => {
            let request = EmbeddingRequest::new(
                "req-metal-model-1",
                service.model_descriptor().clone(),
                vec![String::from("open agents"), String::from("open agents")],
            );

            let response = service.embed(&request)?;
            let capability = CapabilityEnvelope::from_embedding_model(
                service.backend_selection().clone(),
                service.model_descriptor(),
                ProviderReadiness::ready("metal backend ready"),
            );
            let receipt = ExecutionReceipt::succeeded_for_response(
                service.backend_selection().clone(),
                &request,
                &response,
                100,
                120,
            );

            assert_eq!(response.metadata.model_id, ByteProjectionEmbedder::MODEL_ID);
            assert_eq!(
                response.metadata.model_family,
                ByteProjectionEmbedder::MODEL_FAMILY
            );
            assert_eq!(response.metadata.model_revision, "v1");
            assert_eq!(response.metadata.dimensions, 8);
            assert_eq!(response.metadata.input_count, 2);
            assert_eq!(response.metadata.requested_output_dimensions, None);
            assert_eq!(
                response.metadata.normalization,
                psionic_serve::EmbeddingNormalization::UnitLength
            );
            assert_eq!(response.embeddings.len(), 2);
            assert_eq!(response.embeddings[0].values, response.embeddings[1].values);

            assert_eq!(capability.product_id, "psionic.embeddings");
            assert_eq!(capability.runtime_backend, "metal");
            assert_eq!(
                capability.validation.claim_id,
                "metal.embeddings.apple_silicon"
            );
            assert_eq!(capability.backend_selection.requested_backend, "metal");
            assert_eq!(capability.backend_selection.effective_backend, "metal");
            assert!(capability.backend_selection.selected_device.is_some());
            assert!(capability.backend_selection.fallback_reason.is_none());
            assert_eq!(capability.model_id, ByteProjectionEmbedder::MODEL_ID);
            assert_eq!(
                capability.model_family,
                ByteProjectionEmbedder::MODEL_FAMILY
            );
            assert_eq!(capability.model_revision, "v1");
            assert_eq!(
                capability.normalization,
                psionic_serve::EmbeddingNormalization::UnitLength
            );
            assert!(capability.preserves_input_order);
            assert!(capability.empty_batch_returns_empty);
            assert!(capability.supports_output_dimensions);
            assert!(!capability.supports_input_truncation);
            let capability_json = serde_json::to_string_pretty(&capability)?;
            assert!(capability_json.contains("\"runtime_backend\": \"metal\""));
            assert!(capability_json.contains("\"effective_backend\": \"metal\""));

            assert_eq!(receipt.status, ReceiptStatus::Succeeded);
            assert_eq!(receipt.runtime_backend, "metal");
            assert_eq!(
                receipt.validation.claim_id,
                "metal.embeddings.apple_silicon"
            );
            assert_eq!(receipt.backend_selection.effective_backend, "metal");
            assert_eq!(receipt.model_family, ByteProjectionEmbedder::MODEL_FAMILY);
            assert_eq!(receipt.model_revision, "v1");
            assert_eq!(receipt.weight_bundle.digest, request.model.weights.digest);
            assert_eq!(receipt.output_dimensions, 8);
            assert_eq!(receipt.input_count, 2);
            assert_eq!(receipt.output_vector_count, 2);
            assert_eq!(
                receipt.normalization,
                psionic_serve::EmbeddingNormalization::UnitLength
            );
            assert_eq!(receipt.requested_output_dimensions, None);
            assert!(receipt.failure_reason.is_none());
        }
        Err(MetalEmbeddingsError::BackendUnavailable { status, message }) => {
            assert!(matches!(
                status,
                HealthStatus::Offline | HealthStatus::Degraded
            ));
            assert!(!message.is_empty());
            let diagnostic =
                MetalEmbeddingsError::BackendUnavailable { status, message }.diagnostic();
            let expected_code = if status == HealthStatus::Degraded {
                psionic_runtime::LocalRuntimeErrorCode::BackendDegraded
            } else {
                psionic_runtime::LocalRuntimeErrorCode::BackendUnavailable
            };
            assert_eq!(diagnostic.code, expected_code);
            assert_eq!(diagnostic.status, 503);
            assert_eq!(diagnostic.backend.as_deref(), Some("metal"));
            assert_eq!(diagnostic.backend_health, Some(status));
        }
        Err(error) => return Err(error.into()),
    }

    Ok(())
}
