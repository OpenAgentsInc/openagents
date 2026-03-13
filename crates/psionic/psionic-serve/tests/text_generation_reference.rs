#![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

use psionic_backend_cpu::CpuBackend;
use psionic_provider::{ReceiptStatus, TextGenerationReceipt};
use psionic_serve::{
    CpuReferenceTextGenerationService, GenerationOptions, GenerationRequest, ReferenceWordDecoder,
    TerminationReason, TextGenerationExecutor,
};

#[test]
fn cpu_reference_text_generation_flow_returns_response_and_receipt()
-> Result<(), Box<dyn std::error::Error>> {
    let mut service = CpuReferenceTextGenerationService::new()?;
    let session = service.create_session(ReferenceWordDecoder::MODEL_ID)?;
    let request = GenerationRequest::new_text(
        "gen-integration-1",
        service.model_descriptor().clone(),
        Some(session.session_id.clone()),
        "hello",
        GenerationOptions::greedy(4),
    );

    let response = service.generate(&request)?;
    let receipt = TextGenerationReceipt::succeeded_for_response(
        cpu_backend_selection()?,
        &request,
        &response,
        service
            .plan_digest(ReferenceWordDecoder::MODEL_ID)
            .expect("plan digest")
            .to_string(),
        10,
        20,
    );

    assert_eq!(response.output.text, "open agents");
    assert_eq!(response.termination, TerminationReason::EndOfSequence);
    assert_eq!(receipt.status, ReceiptStatus::Succeeded);
    assert_eq!(receipt.backend_selection.requested_backend, "cpu");
    assert_eq!(receipt.model_id, ReferenceWordDecoder::MODEL_ID);
    assert_eq!(receipt.model_family, "fixture_decoder");
    assert_eq!(receipt.model_revision, "v0");
    assert_eq!(receipt.weight_bundle.digest, request.model.weights.digest);
    assert!(receipt.weight_bundle.artifacts.is_empty());
    assert_eq!(receipt.input_tokens, 2);
    assert_eq!(receipt.output_tokens, 2);
    assert_eq!(receipt.cache_tokens, 4);
    assert_eq!(receipt.termination, Some(TerminationReason::EndOfSequence));
    assert!(receipt.execution_plan_digest.is_some());
    Ok(())
}

fn cpu_backend_selection()
-> Result<psionic_runtime::BackendSelection, psionic_runtime::RuntimeError> {
    CpuBackend::new().backend_selection(&["input", "constant", "matmul", "add"])
}
