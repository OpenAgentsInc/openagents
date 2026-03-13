#![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

use psionic_provider::{
    KvCacheMode, ProviderReadiness, ReceiptStatus, TextGenerationCapabilityEnvelope,
    TextGenerationReceipt,
};
use psionic_runtime::{BackendSelectionState, HealthStatus, LocalRuntimeErrorCode};
use psionic_serve::{
    ArtifactWordDecoder, GenerationOptions, GenerationRequest, MetalModelTextGenerationService,
    MetalTextGenerationError, TerminationReason, TextGenerationExecutor,
    default_text_generation_execution_profile,
};
use tempfile::tempdir;

#[test]
fn metal_model_backed_text_generation_returns_response_capability_and_receipt_or_explicit_unavailability()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let path = temp.path().join("wordpiece_decoder.safetensors");
    ArtifactWordDecoder::write_default_safetensors_artifact(&path)?;
    let model_descriptor = ArtifactWordDecoder::from_safetensors_artifact(&path)?
        .descriptor()
        .clone();
    let unavailable_request = GenerationRequest::new_text(
        "gen-metal-model-unavailable",
        model_descriptor.clone(),
        None,
        "hello",
        GenerationOptions::greedy(2),
    );

    match MetalModelTextGenerationService::from_safetensors_artifact(&path) {
        Ok(mut service) => {
            let session = service.create_session(ArtifactWordDecoder::MODEL_ID)?;
            let request = GenerationRequest::new_text(
                "gen-metal-model-1",
                service.model_descriptor().clone(),
                Some(session.session_id.clone()),
                "hello",
                GenerationOptions::greedy(4),
            );

            let response = service.generate(&request)?;
            let selection = service.backend_selection().clone();
            let loaded_view = service
                .loaded_model_views()
                .into_iter()
                .find(|view| view.summary.model == ArtifactWordDecoder::MODEL_ID)
                .expect("loaded model view");
            let capability = TextGenerationCapabilityEnvelope::from_decoder_model(
                selection.clone(),
                service.model_descriptor(),
                loaded_view.memory_plan.clone(),
                loaded_view.residency_policy.clone(),
                KvCacheMode::Paged,
                default_text_generation_execution_profile(),
                ProviderReadiness::ready("metal backend ready"),
            );
            let receipt = TextGenerationReceipt::succeeded_for_response(
                selection.clone(),
                &request,
                &response,
                service
                    .plan_digest(ArtifactWordDecoder::MODEL_ID)
                    .expect("plan digest")
                    .to_string(),
                110,
                140,
            );

            assert_eq!(response.model_id, ArtifactWordDecoder::MODEL_ID);
            assert_eq!(response.output.text, "open agents");
            assert_eq!(response.termination, TerminationReason::EndOfSequence);
            assert_eq!(response.usage.input_tokens, 2);
            assert_eq!(response.usage.output_tokens, 2);
            assert_eq!(response.usage.cache_tokens, 4);

            assert_eq!(capability.product_id, "psionic.text_generation");
            assert_eq!(capability.runtime_backend, "metal");
            assert_eq!(
                capability.validation.claim_id,
                "metal.text_generation.apple_silicon"
            );
            assert_eq!(capability.backend_selection.requested_backend, "metal");
            assert_eq!(capability.backend_selection.effective_backend, "metal");
            assert!(capability.backend_selection.selected_device.is_some());
            assert_ne!(
                capability.backend_selection.selection_state,
                BackendSelectionState::CrossBackendFallback
            );
            assert_eq!(capability.model_id, ArtifactWordDecoder::MODEL_ID);
            assert_eq!(capability.model_family, ArtifactWordDecoder::MODEL_FAMILY);
            assert_eq!(capability.model_revision, "v1");
            assert_eq!(
                capability.weight_bundle.digest,
                request.model.weights.digest
            );
            assert_eq!(capability.kv_cache_mode, KvCacheMode::Paged);
            assert_eq!(
                capability.execution_profile,
                default_text_generation_execution_profile()
            );
            let capability_json = serde_json::to_string_pretty(&capability)?;
            assert!(capability_json.contains("\"runtime_backend\": \"metal\""));
            assert!(capability_json.contains("\"effective_backend\": \"metal\""));

            match capability.backend_selection.selection_state {
                BackendSelectionState::Direct => {
                    assert!(capability.backend_selection.fallback_reason.is_none());
                    assert!(capability.backend_selection.degraded_reason.is_none());
                }
                BackendSelectionState::SameBackendDegraded => {
                    assert!(capability.backend_selection.fallback_reason.is_none());
                    assert!(capability.backend_selection.degraded_reason.is_some());
                }
                BackendSelectionState::SameBackendSlowPath | BackendSelectionState::Retried => {
                    assert!(capability.backend_selection.fallback_reason.is_some());
                    assert!(capability.backend_selection.degraded_reason.is_none());
                }
                BackendSelectionState::CrossBackendFallback | BackendSelectionState::Refused => {
                    unreachable!()
                }
            }

            assert_eq!(receipt.status, ReceiptStatus::Succeeded);
            assert_eq!(receipt.runtime_backend, "metal");
            assert_eq!(
                receipt.validation.claim_id,
                "metal.text_generation.apple_silicon"
            );
            assert_eq!(receipt.backend_selection.requested_backend, "metal");
            assert_eq!(receipt.backend_selection.effective_backend, "metal");
            assert_eq!(receipt.model_id, ArtifactWordDecoder::MODEL_ID);
            assert_eq!(receipt.model_family, ArtifactWordDecoder::MODEL_FAMILY);
            assert_eq!(receipt.model_revision, "v1");
            assert_eq!(receipt.weight_bundle.digest, request.model.weights.digest);
            assert_eq!(receipt.session_id, Some(session.session_id));
            assert_eq!(receipt.input_tokens, 2);
            assert_eq!(receipt.output_tokens, 2);
            assert_eq!(receipt.cache_tokens, 4);
            assert_eq!(receipt.termination, Some(TerminationReason::EndOfSequence));
            assert!(receipt.execution_plan_digest.is_some());
            assert!(receipt.failure_reason.is_none());
            let receipt_json = serde_json::to_string_pretty(&receipt)?;
            assert!(receipt_json.contains("\"runtime_backend\": \"metal\""));
        }
        Err(MetalTextGenerationError::BackendUnavailable { status, message }) => {
            assert!(matches!(
                status,
                HealthStatus::Offline | HealthStatus::Degraded
            ));
            assert!(!message.is_empty());

            let diagnostic =
                MetalTextGenerationError::BackendUnavailable { status, message }.diagnostic();
            let expected_code = if status == HealthStatus::Degraded {
                LocalRuntimeErrorCode::BackendDegraded
            } else {
                LocalRuntimeErrorCode::BackendUnavailable
            };
            assert_eq!(diagnostic.code, expected_code);
            assert_eq!(diagnostic.status, 503);
            assert_eq!(diagnostic.backend.as_deref(), Some("metal"));
            assert_eq!(diagnostic.backend_health, Some(status));

            let diagnostic = MetalTextGenerationError::BackendUnavailable {
                status,
                message: String::from("metal backend unavailable"),
            }
            .diagnostic_for_request(&unavailable_request);
            assert_eq!(
                diagnostic.product_id.as_deref(),
                Some("psionic.text_generation")
            );
            assert_eq!(
                diagnostic.model_id.as_deref(),
                Some(ArtifactWordDecoder::MODEL_ID)
            );
        }
        Err(error) => return Err(error.into()),
    }

    Ok(())
}
