#![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

use std::{fs, io, path::PathBuf};

use psionic_models::{
    ParameterGolfPromotedGenerationOptions, ParameterGolfPromotedGenerationOutput,
    ParameterGolfPromotedGenerationTermination, ParameterGolfPromotedRuntimeBundle,
    ParameterGolfReferenceModel, TokenId, TokenizerBoundary,
};
use psionic_runtime::TokenSampler;
use psionic_serve::{
    CpuPromotedParameterGolfTextGenerationService, GenerationEventStream, GenerationOptions,
    GenerationRequest, GenerationStreamEvent, GenerationStreamStatus,
    StreamingTextGenerationExecutor, TextGenerationExecutor,
};
use psionic_train::{
    restore_parameter_golf_local_reference_checkpoint, run_parameter_golf_promoted_reference_run,
    write_parameter_golf_promoted_reference_run, ParameterGolfLocalReferenceFixture,
    ParameterGolfReferenceTrainingConfig,
};
use serde::Deserialize;
use tempfile::{tempdir, TempDir};

#[derive(Clone, Debug, Deserialize)]
struct GoldenPromptCase {
    prompt_id: String,
    prompt: String,
    max_new_tokens: usize,
    mode: String,
    #[serde(default)]
    seed: Option<u64>,
    expected_text: String,
    detail: String,
}

fn write_repo_owned_promoted_bundle() -> Result<
    (
        ParameterGolfLocalReferenceFixture,
        psionic_train::ParameterGolfPromotedReferenceRun,
        TempDir,
    ),
    Box<dyn std::error::Error>,
> {
    let fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
    let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;
    let output_dir = tempdir()?;
    write_parameter_golf_promoted_reference_run(&run, output_dir.path())?;
    Ok((fixture, run, output_dir))
}

fn golden_prompt_suite() -> Result<Vec<GoldenPromptCase>, Box<dyn std::error::Error>> {
    let suite_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../fixtures/parameter_golf/inference/parameter_golf_promoted_golden_prompts.json",
    );
    Ok(serde_json::from_slice(&fs::read(suite_path)?)?)
}

fn bundle_options_for_case(
    bundle: &ParameterGolfPromotedRuntimeBundle,
    case: &GoldenPromptCase,
) -> ParameterGolfPromotedGenerationOptions {
    let mut options = if case.mode == "sample" {
        bundle.default_seeded_sampling_options(case.seed.unwrap_or(42))
    } else {
        bundle.default_greedy_generation_options()
    };
    options.max_new_tokens = case.max_new_tokens;
    options
}

fn serve_options_for_case(
    bundle: &ParameterGolfPromotedRuntimeBundle,
    case: &GoldenPromptCase,
) -> GenerationOptions {
    let bundle_options = bundle_options_for_case(bundle, case);
    let mut options = if case.mode == "sample" {
        GenerationOptions::sample(case.max_new_tokens)
    } else {
        GenerationOptions::greedy(case.max_new_tokens)
    };
    options.temperature = bundle_options.sampling_policy.temperature;
    options.top_k = bundle_options.sampling_policy.top_k;
    options.top_p = bundle_options.sampling_policy.top_p;
    options.repeat_penalty = bundle_options.sampling_policy.repeat_penalty;
    options.presence_penalty = bundle_options.sampling_policy.presence_penalty;
    options.frequency_penalty = bundle_options.sampling_policy.frequency_penalty;
    options.seed = bundle_options.sampling_policy.seed;
    options
}

fn generate_from_restored_training_model(
    model: &ParameterGolfReferenceModel,
    bundle: &ParameterGolfPromotedRuntimeBundle,
    prompt: &str,
    options: &ParameterGolfPromotedGenerationOptions,
) -> Result<ParameterGolfPromotedGenerationOutput, Box<dyn std::error::Error>> {
    let prompt_tokens = bundle.tokenizer().encode_with_defaults(prompt);
    let mut history = prompt_tokens
        .as_slice()
        .iter()
        .map(|token| token.as_u32())
        .collect::<Vec<_>>();
    let mut generated_tokens = Vec::new();
    let mut sampler = TokenSampler::new(&options.sampling_policy);
    let termination = loop {
        if generated_tokens.len() >= options.max_new_tokens {
            break ParameterGolfPromotedGenerationTermination::MaxNewTokens;
        }
        if history.len() >= bundle.generation_config().max_context {
            break ParameterGolfPromotedGenerationTermination::ContextLimit;
        }
        let logits = model.forward_logits(&[history.clone()])?;
        let width = logits.width();
        let sequence_length = logits.sequence_length();
        let last_row_start = sequence_length
            .checked_sub(1)
            .ok_or_else(|| io::Error::other("training-side logits were empty"))?
            .saturating_mul(width);
        let last_logits = logits
            .values()
            .get(last_row_start..last_row_start.saturating_add(width))
            .ok_or_else(|| io::Error::other("training-side logits row was missing"))?;
        let next_token = sampler
            .select_next_token(last_logits, history.as_slice())
            .ok_or_else(|| io::Error::other("training-side sampler could not pick a token"))?;
        let next_token = TokenId(next_token);
        history.push(next_token.as_u32());
        generated_tokens.push(next_token);
        if options.stop_on_eos && bundle.tokenizer().is_end_of_sequence(next_token) {
            break ParameterGolfPromotedGenerationTermination::EndOfSequence;
        }
    };
    let all_tokens = prompt_tokens
        .as_slice()
        .iter()
        .copied()
        .chain(generated_tokens.iter().copied())
        .collect::<Vec<_>>();
    Ok(ParameterGolfPromotedGenerationOutput {
        prompt_tokens,
        generated_tokens: psionic_models::TokenSequence::new(generated_tokens.clone()),
        all_tokens: psionic_models::TokenSequence::new(all_tokens),
        text: bundle.tokenizer().decode(generated_tokens.as_slice()),
        termination,
    })
}

#[test]
fn promoted_parameter_golf_train_runtime_and_serve_stay_in_conformance(
) -> Result<(), Box<dyn std::error::Error>> {
    let (fixture, run, bundle_dir) = write_repo_owned_promoted_bundle()?;
    let restored = restore_parameter_golf_local_reference_checkpoint(
        &fixture,
        &run.training_outcome.final_checkpoint,
    )?;
    let bundle = ParameterGolfPromotedRuntimeBundle::load_dir(bundle_dir.path())?;
    let mut service =
        CpuPromotedParameterGolfTextGenerationService::from_bundle_dir(bundle_dir.path())?;
    let suite = golden_prompt_suite()?;

    assert_eq!(
        bundle.descriptor().stable_digest(),
        run.model_descriptor.stable_digest()
    );
    assert_eq!(bundle.model(), restored.current_model());
    assert_eq!(bundle.manifest().profile_id, run.summary.profile_id);

    for case in suite {
        let encoded_defaults = bundle
            .tokenizer()
            .encode_with_defaults(case.prompt.as_str());
        let encoded_plain = bundle.tokenizer().encode(case.prompt.as_str());
        assert_eq!(
            bundle.tokenizer().decode(encoded_defaults.as_slice()),
            case.prompt,
            "default encode/decode drifted for `{}` ({})",
            case.prompt_id,
            case.detail
        );
        assert_eq!(
            bundle.tokenizer().decode(encoded_plain.as_slice()),
            case.prompt,
            "plain encode/decode drifted for `{}` ({})",
            case.prompt_id,
            case.detail
        );
        if bundle.tokenizer().asset().add_bos {
            assert_eq!(
                encoded_defaults
                    .as_slice()
                    .first()
                    .map(|token| token.as_u32()),
                bundle.tokenizer().asset().bos_token_id,
                "BOS handling drifted for `{}`",
                case.prompt_id
            );
        }
        if bundle.tokenizer().asset().add_eos {
            assert!(
                encoded_defaults
                    .as_slice()
                    .last()
                    .is_some_and(|token| bundle
                        .tokenizer()
                        .asset()
                        .eos_token_ids
                        .contains(&token.as_u32())),
                "EOS handling drifted for `{}`",
                case.prompt_id
            );
        }

        let prompt_ids = encoded_defaults
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect::<Vec<_>>();
        let training_logits = restored
            .current_model()
            .forward_logits(&[prompt_ids.clone()])?;
        let runtime_logits = bundle.model().forward_logits(&[prompt_ids])?;
        assert_eq!(
            training_logits.values(),
            runtime_logits.values(),
            "training-side restore logits drifted from runtime loader for `{}`",
            case.prompt_id
        );

        let bundle_options = bundle_options_for_case(&bundle, &case);
        let training_output = generate_from_restored_training_model(
            restored.current_model(),
            &bundle,
            case.prompt.as_str(),
            &bundle_options,
        )?;
        let runtime_output = bundle.generate_text(case.prompt.as_str(), &bundle_options)?;

        let serve_request = GenerationRequest::new_text(
            format!("promoted-pgolf-conformance-{}", case.prompt_id),
            service.model_descriptor().clone(),
            None,
            case.prompt.clone(),
            serve_options_for_case(&bundle, &case),
        );
        let serve_response = service.generate(&serve_request)?;

        let stream_request = GenerationRequest::new_text(
            format!("promoted-pgolf-conformance-stream-{}", case.prompt_id),
            service.model_descriptor().clone(),
            None,
            case.prompt.clone(),
            serve_options_for_case(&bundle, &case),
        );
        let mut stream = service.generate_stream(&stream_request)?;
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
            GenerationStreamStatus::Succeeded,
            "serve stream failed for `{}`",
            case.prompt_id
        );
        drop(stream);

        assert_eq!(
            training_output.text, case.expected_text,
            "training-side restore output drifted for `{}`",
            case.prompt_id
        );
        assert_eq!(
            runtime_output.text, case.expected_text,
            "runtime-bundle output drifted for `{}`",
            case.prompt_id
        );
        assert_eq!(
            serve_response.output.text, case.expected_text,
            "serve generate output drifted for `{}`",
            case.prompt_id
        );
        assert_eq!(
            terminal.response.output.text, case.expected_text,
            "serve stream output drifted for `{}`",
            case.prompt_id
        );
        assert_eq!(
            streamed_text, case.expected_text,
            "stream chunk accumulation drifted for `{}`",
            case.prompt_id
        );
        assert_eq!(training_output.text, runtime_output.text);
        assert_eq!(runtime_output.text, serve_response.output.text);
        assert_eq!(serve_response.output.text, terminal.response.output.text);
    }

    Ok(())
}
