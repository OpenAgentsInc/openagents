use rustygrad_models::ModelLoadError;
use rustygrad_provider::{
    BatchPosture, KvCacheMode, ProviderReadiness, ReceiptStatus, TextGenerationCapabilityEnvelope,
    TextGenerationReceipt,
};
use rustygrad_serve::{
    ArtifactWordDecoder, CpuModelTextGenerationService, GenerationOptions, GenerationRequest,
    ReferenceTextGenerationError, TerminationReason, TextGenerationExecutor,
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
    let capability = TextGenerationCapabilityEnvelope::from_decoder_model(
        "cpu",
        service.model_descriptor(),
        KvCacheMode::InMemory,
        BatchPosture::SingleRequestOnly,
        ProviderReadiness::ready("cpu backend ready"),
    );
    let receipt = TextGenerationReceipt::succeeded_for_response(
        "cpu",
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

    assert_eq!(capability.product_id, "rustygrad.text_generation");
    assert_eq!(capability.runtime_backend, "cpu");
    assert_eq!(capability.model_id, ArtifactWordDecoder::MODEL_ID);
    assert_eq!(capability.model_family, ArtifactWordDecoder::MODEL_FAMILY);
    assert_eq!(capability.kv_cache_mode, KvCacheMode::InMemory);
    assert_eq!(capability.batch_posture, BatchPosture::SingleRequestOnly);

    assert_eq!(receipt.status, ReceiptStatus::Succeeded);
    assert_eq!(receipt.model_id, ArtifactWordDecoder::MODEL_ID);
    assert_eq!(receipt.session_id, Some(session.session_id.clone()));
    assert_eq!(receipt.input_tokens, 2);
    assert_eq!(receipt.output_tokens, 2);
    assert_eq!(receipt.cache_tokens, 4);
    assert_eq!(receipt.termination, Some(TerminationReason::EndOfSequence));
    assert!(receipt.execution_plan_digest.is_some());

    let follow_up = GenerationRequest::new_text(
        "gen-model-2",
        service.model_descriptor().clone(),
        Some(session.session_id.clone()),
        "rusty",
        GenerationOptions::greedy(4),
    );
    let continued = service.generate(&follow_up)?;
    assert_eq!(continued.output.text, "grad");
    assert!(continued.usage.cache_tokens > response.usage.cache_tokens);

    let reset = GenerationRequest::new_text(
        "gen-model-3",
        service.model_descriptor().clone(),
        Some(session.session_id.clone()),
        "rusty",
        GenerationOptions::greedy(4),
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
        "/tmp/definitely-missing-rustygrad-wordpiece-decoder.safetensors",
    )
    .expect_err("missing artifact should fail");

    assert!(matches!(
        error,
        ReferenceTextGenerationError::Model(ModelLoadError::ArtifactRead { .. })
    ));
}
