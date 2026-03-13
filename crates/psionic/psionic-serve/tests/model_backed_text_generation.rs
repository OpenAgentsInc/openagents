#![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

use psionic_backend_cpu::CpuBackend;
use psionic_models::ModelLoadError;
use psionic_provider::{
    KvCacheMode, ProviderReadiness, ReceiptStatus, TextGenerationCapabilityEnvelope,
    TextGenerationReceipt,
};
use psionic_serve::{
    ArtifactWordDecoder, CpuModelTextGenerationService, GenerationOptions, GenerationRequest,
    ReferenceTextGenerationError, ReferenceWordDecoder, SessionId, TerminationReason,
    TextGenerationExecutor, default_text_generation_execution_profile,
};
use tempfile::tempdir;

#[test]
fn model_backed_text_generation_flow_returns_response_capability_and_receipt()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let path = temp.path().join("wordpiece_decoder.safetensors");
    ArtifactWordDecoder::write_default_safetensors_artifact(&path)?;

    let mut service = CpuModelTextGenerationService::from_safetensors_artifact(&path)?;
    let session = service.create_session(ArtifactWordDecoder::MODEL_ID)?;
    let request = GenerationRequest::new_text(
        "gen-model-1",
        service.model_descriptor().clone(),
        Some(session.session_id.clone()),
        "hello",
        GenerationOptions::greedy(4),
    );

    let response = service.generate(&request)?;
    let loaded_view = service
        .loaded_model_views()
        .into_iter()
        .find(|view| view.summary.model == ArtifactWordDecoder::MODEL_ID)
        .expect("loaded model view");
    let capability = TextGenerationCapabilityEnvelope::from_decoder_model(
        cpu_backend_selection()?,
        service.model_descriptor(),
        loaded_view.memory_plan.clone(),
        loaded_view.residency_policy.clone(),
        KvCacheMode::Paged,
        default_text_generation_execution_profile(),
        ProviderReadiness::ready("cpu backend ready"),
    );
    let receipt = TextGenerationReceipt::succeeded_for_response(
        cpu_backend_selection()?,
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
    assert_eq!(capability.runtime_backend, "cpu");
    assert_eq!(
        capability.validation.claim_id,
        "cpu.text_generation.reference"
    );
    assert_eq!(capability.backend_selection.requested_backend, "cpu");
    assert!(capability.backend_selection.fallback_reason.is_none());
    assert_eq!(capability.model_id, ArtifactWordDecoder::MODEL_ID);
    assert_eq!(capability.model_family, ArtifactWordDecoder::MODEL_FAMILY);
    assert_eq!(capability.model_revision, "v1");
    assert_eq!(
        capability.weight_bundle.digest,
        request.model.weights.digest
    );
    assert_eq!(capability.weight_bundle.artifacts.len(), 1);
    assert!(capability.memory_plan.resident_host_bytes > 0);
    assert_eq!(capability.kv_cache_mode, KvCacheMode::Paged);
    assert!(capability.kv_cache_policy.is_some());
    assert_eq!(
        capability.execution_profile,
        default_text_generation_execution_profile()
    );
    let capability_json = serde_json::to_string_pretty(&capability)?;
    assert!(capability_json.contains("\"model_revision\": \"v1\""));
    assert!(capability_json.contains("\"weight_bundle\""));

    assert_eq!(receipt.status, ReceiptStatus::Succeeded);
    assert_eq!(receipt.validation.claim_id, "cpu.text_generation.reference");
    assert_eq!(receipt.backend_selection.effective_backend, "cpu");
    assert_eq!(receipt.model_id, ArtifactWordDecoder::MODEL_ID);
    assert_eq!(receipt.model_family, ArtifactWordDecoder::MODEL_FAMILY);
    assert_eq!(receipt.model_revision, "v1");
    assert_eq!(receipt.weight_bundle.digest, request.model.weights.digest);
    assert!(receipt.memory_plan.is_some());
    assert!(receipt.residency_policy.is_some());
    assert!(receipt.residency_snapshot.is_some());
    assert_eq!(receipt.session_id, Some(session.session_id.clone()));
    assert_eq!(receipt.input_tokens, 2);
    assert_eq!(receipt.output_tokens, 2);
    assert_eq!(receipt.cache_tokens, 4);
    assert!(receipt.kv_cache_policy.is_some());
    assert_eq!(
        receipt.kv_cache.as_ref().map(|value| value.current.pages),
        Some(1)
    );
    assert_eq!(receipt.termination, Some(TerminationReason::EndOfSequence));
    assert!(receipt.execution_plan_digest.is_some());
    let receipt_json = serde_json::to_string_pretty(&receipt)?;
    assert!(receipt_json.contains("\"weight_bundle\""));

    let follow_up = GenerationRequest::new_text(
        "gen-model-2",
        service.model_descriptor().clone(),
        Some(session.session_id.clone()),
        "rusty",
        GenerationOptions::greedy(1),
    );
    let continued = service.generate(&follow_up)?;
    assert_eq!(continued.output.text, "grad");
    assert!(continued.usage.cache_tokens > response.usage.cache_tokens);

    let reset = GenerationRequest::new_text(
        "gen-model-3",
        service.model_descriptor().clone(),
        Some(session.session_id.clone()),
        "rusty",
        GenerationOptions::greedy(1),
    )
    .with_reset_session(true);
    let reset_response = service.generate(&reset)?;
    assert_eq!(reset_response.output.text, "grad");
    assert_eq!(reset_response.usage.cache_tokens, 3);
    Ok(())
}

#[test]
fn model_backed_text_generation_service_reports_missing_artifact() {
    let error = CpuModelTextGenerationService::from_safetensors_artifact(
        "/tmp/definitely-missing-psionic-wordpiece-decoder.safetensors",
    )
    .expect_err("missing artifact should fail");

    assert!(matches!(
        error,
        ReferenceTextGenerationError::Model(ModelLoadError::ArtifactRead { .. })
    ));
    let diagnostic = error.diagnostic();
    assert_eq!(
        diagnostic.code,
        psionic_runtime::LocalRuntimeErrorCode::ArtifactMissing
    );
    assert_eq!(diagnostic.status, 404);
}

#[test]
fn model_backed_text_generation_rejects_reference_descriptor_without_fallback()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let path = temp.path().join("wordpiece_decoder.safetensors");
    ArtifactWordDecoder::write_default_safetensors_artifact(&path)?;

    let mut service = CpuModelTextGenerationService::from_safetensors_artifact(&path)?;
    let request = GenerationRequest::new_text(
        "gen-model-bad-model",
        ReferenceWordDecoder::new().descriptor().clone(),
        None,
        "hello",
        GenerationOptions::greedy(2),
    );

    let error = service
        .generate(&request)
        .expect_err("reference model should not silently run");
    assert!(matches!(
        error,
        ReferenceTextGenerationError::UnsupportedModel(model_id)
            if model_id == ReferenceWordDecoder::MODEL_ID
    ));
    Ok(())
}

fn cpu_backend_selection()
-> Result<psionic_runtime::BackendSelection, psionic_runtime::RuntimeError> {
    CpuBackend::new().backend_selection(&["input", "constant", "matmul", "add"])
}

#[test]
fn model_backed_text_generation_rejects_unknown_session() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempdir()?;
    let path = temp.path().join("wordpiece_decoder.safetensors");
    ArtifactWordDecoder::write_default_safetensors_artifact(&path)?;

    let mut service = CpuModelTextGenerationService::from_safetensors_artifact(&path)?;
    let request = GenerationRequest::new_text(
        "gen-model-missing-session",
        service.model_descriptor().clone(),
        Some(SessionId::new("sess-missing")),
        "hello",
        GenerationOptions::greedy(2),
    );

    let error = service
        .generate(&request)
        .expect_err("missing session should fail");
    assert!(matches!(
        error,
        ReferenceTextGenerationError::Session(
            psionic_serve::SessionStoreError::SessionNotFound(ref session_id)
        ) if session_id == "sess-missing"
    ));
    let diagnostic = error.diagnostic_for_request(&request);
    assert_eq!(
        diagnostic.code,
        psionic_runtime::LocalRuntimeErrorCode::SessionNotFound
    );
    assert_eq!(diagnostic.status, 404);
    assert_eq!(
        diagnostic.product_id.as_deref(),
        Some("psionic.text_generation")
    );
    assert_eq!(
        diagnostic.model_id.as_deref(),
        Some(ArtifactWordDecoder::MODEL_ID)
    );
    Ok(())
}
