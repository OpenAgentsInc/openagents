#![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

use psionic_models::{ParameterGolfPromotedGenerationOptions, ParameterGolfPromotedRuntimeBundle};
use psionic_serve::{
    CpuPromotedParameterGolfTextGenerationService, GenerationEventStream, GenerationOptions,
    GenerationRequest, GenerationStreamEvent, ReferenceTextGenerationError, ReferenceWordDecoder,
    StreamingTextGenerationExecutor, TerminationReason, TextGenerationExecutor,
};
use psionic_train::{
    run_parameter_golf_promoted_reference_run, write_parameter_golf_promoted_reference_run,
    ParameterGolfLocalReferenceFixture, ParameterGolfReferenceTrainingConfig,
};
use tempfile::{tempdir, TempDir};

fn write_repo_owned_promoted_bundle() -> Result<TempDir, Box<dyn std::error::Error>> {
    let fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
    let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;
    let output_dir = tempdir()?;
    write_parameter_golf_promoted_reference_run(&run, output_dir.path())?;
    Ok(output_dir)
}

#[test]
fn promoted_parameter_golf_service_serves_repo_owned_bundle_via_generate_and_stream(
) -> Result<(), Box<dyn std::error::Error>> {
    let bundle_dir = write_repo_owned_promoted_bundle()?;
    let bundle = ParameterGolfPromotedRuntimeBundle::load_dir(bundle_dir.path())?;
    let greedy_options = ParameterGolfPromotedGenerationOptions::greedy(4);
    let expected = bundle.generate_text("abcd", &greedy_options)?;

    let mut service =
        CpuPromotedParameterGolfTextGenerationService::from_bundle_dir(bundle_dir.path())?;
    let model_id = service.model_descriptor().model.model_id.clone();
    let session = service.create_session(model_id.as_str())?;
    let request = GenerationRequest::new_text(
        "promoted-pgolf-generate",
        service.model_descriptor().clone(),
        Some(session.session_id.clone()),
        "abcd",
        GenerationOptions::greedy(4),
    );
    let response = service.generate(&request)?;

    assert_eq!(response.model_id, service.model_descriptor().model.model_id);
    assert_eq!(response.output.text, expected.text);
    assert_eq!(response.termination, TerminationReason::MaxOutputTokens);
    assert_eq!(response.usage.output_tokens, 4);
    assert_eq!(response.usage.input_tokens, expected.prompt_tokens.len());
    assert!(response.provenance.is_some());

    let follow_up = GenerationRequest::new_text(
        "promoted-pgolf-follow-up",
        service.model_descriptor().clone(),
        Some(session.session_id.clone()),
        "efgh",
        GenerationOptions::greedy(2),
    );
    let follow_up_response = service.generate(&follow_up)?;
    assert!(follow_up_response.usage.cache_tokens > response.usage.cache_tokens);

    let streaming_request = GenerationRequest::new_text(
        "promoted-pgolf-stream",
        service.model_descriptor().clone(),
        None,
        "abcd",
        GenerationOptions::greedy(4),
    );
    let mut stream = service.generate_stream(&streaming_request)?;
    let mut streamed_text = String::new();
    let mut terminal = None;
    while let Some(event) = stream.next_event() {
        match event {
            GenerationStreamEvent::Chunk(chunk) => {
                streamed_text.push_str(chunk.output.text.as_str())
            }
            GenerationStreamEvent::Terminal(value) => {
                streamed_text = value.response.output.text.clone();
                terminal = Some(value);
            }
        }
    }
    let terminal = terminal.expect("stream should terminate");
    assert_eq!(
        terminal.status,
        psionic_serve::GenerationStreamStatus::Succeeded
    );
    assert_eq!(terminal.response.output.text, expected.text);
    assert_eq!(streamed_text, expected.text);
    drop(stream);

    let unsupported_request = GenerationRequest::new_text(
        "promoted-pgolf-wrong-model",
        ReferenceWordDecoder::new().descriptor().clone(),
        None,
        "abcd",
        GenerationOptions::greedy(2),
    );
    let error = service
        .generate(&unsupported_request)
        .expect_err("wrong descriptor should refuse");
    assert!(matches!(
        error,
        ReferenceTextGenerationError::UnsupportedModel(model_id)
            if model_id == ReferenceWordDecoder::MODEL_ID
    ));
    Ok(())
}

#[test]
fn promoted_parameter_golf_service_reports_missing_bundle() {
    let error = CpuPromotedParameterGolfTextGenerationService::from_bundle_dir(
        "/tmp/definitely-missing-promoted-parameter-golf-bundle",
    )
    .expect_err("missing bundle should fail");

    assert!(matches!(
        error,
        ReferenceTextGenerationError::PromotedBundleLoad(_)
    ));
    let diagnostic = error.diagnostic();
    assert_eq!(
        diagnostic.code,
        psionic_runtime::LocalRuntimeErrorCode::ArtifactMissing
    );
    assert_eq!(diagnostic.status, 404);
}
