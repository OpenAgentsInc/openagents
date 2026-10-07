use std::{
    env,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use psionic_models::{
    ParameterGolfPromotedRuntimeBundle, TokenId, TokenSequence, TokenizerBoundary,
};
use psionic_runtime::TokenSampler;
use psionic_serve::{
    CpuPromotedParameterGolfTextGenerationService, GenerationOptions, GenerationRequest,
    TextGenerationExecutor,
};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
struct ThroughputVariantReport {
    variant: String,
    repetitions: usize,
    generated_tokens_per_run: usize,
    total_elapsed_ms: u128,
    tokens_per_second: f64,
    output_tokens: Vec<u32>,
    output_text: String,
}

#[derive(Clone, Debug, Serialize)]
struct ParameterGolfPromotedRuntimeBenchmarkReport {
    bundle_dir: String,
    prompt_text: String,
    prompt_tokens: Vec<u32>,
    max_new_tokens: usize,
    repetitions: usize,
    direct_history_ref: ThroughputVariantReport,
    served_runtime: ThroughputVariantReport,
    direct_and_served_match: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let bundle_dir = PathBuf::from(args.next().ok_or("usage: parameter_golf_promoted_runtime_benchmark <bundle_dir> [prompt] [max_new_tokens] [repetitions]")?);
    let prompt_text = args.next().unwrap_or_else(|| String::from("abcd"));
    let max_new_tokens = args
        .next()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(16);
    let repetitions = args
        .next()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(4);

    let bundle = ParameterGolfPromotedRuntimeBundle::load_dir(bundle_dir.as_path())?;
    let prompt_tokens = bundle.tokenizer().encode_with_defaults(prompt_text.as_str());
    let direct_history_ref =
        benchmark_direct_runtime(&bundle, &prompt_tokens, max_new_tokens, repetitions)?;
    let served_runtime = benchmark_served_runtime(
        bundle_dir.as_path(),
        &bundle,
        &prompt_tokens,
        max_new_tokens,
        repetitions,
    )?;
    let report = ParameterGolfPromotedRuntimeBenchmarkReport {
        bundle_dir: bundle_dir.display().to_string(),
        prompt_text,
        prompt_tokens: prompt_tokens
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        max_new_tokens,
        repetitions,
        direct_and_served_match: direct_history_ref.output_tokens == served_runtime.output_tokens,
        direct_history_ref,
        served_runtime,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn benchmark_direct_runtime(
    bundle: &ParameterGolfPromotedRuntimeBundle,
    prompt_tokens: &TokenSequence,
    max_new_tokens: usize,
    repetitions: usize,
) -> Result<ThroughputVariantReport, Box<dyn std::error::Error>> {
    let _ = decode_direct_runtime(bundle, prompt_tokens, max_new_tokens)?;
    let started = Instant::now();
    let mut final_tokens = Vec::new();
    for _ in 0..repetitions {
        final_tokens = decode_direct_runtime(bundle, prompt_tokens, max_new_tokens)?;
    }
    let elapsed = started.elapsed();
    Ok(ThroughputVariantReport {
        variant: String::from("current_runtime"),
        repetitions,
        generated_tokens_per_run: final_tokens.len(),
        total_elapsed_ms: elapsed.as_millis(),
        tokens_per_second: tokens_per_second(final_tokens.len() * repetitions, elapsed),
        output_text: decode_text(bundle, final_tokens.as_slice()),
        output_tokens: final_tokens,
    })
}

fn benchmark_served_runtime(
    bundle_dir: &Path,
    _bundle: &ParameterGolfPromotedRuntimeBundle,
    prompt_tokens: &TokenSequence,
    max_new_tokens: usize,
    repetitions: usize,
) -> Result<ThroughputVariantReport, Box<dyn std::error::Error>> {
    let mut service = CpuPromotedParameterGolfTextGenerationService::from_bundle_dir(bundle_dir)?;
    let descriptor = service.model_descriptor().clone();
    let mut total_eval_ns = 0_u128;
    let mut final_tokens = Vec::new();
    let mut final_text = String::new();
    for repetition in 0..repetitions {
        let request = GenerationRequest::new_tokens(
            format!("pgolf-bench-{repetition}"),
            descriptor.clone(),
            None,
            prompt_tokens.clone(),
            GenerationOptions::greedy(max_new_tokens),
        );
        let response = service.generate(&request)?;
        total_eval_ns += u128::from(response.metrics.eval_duration_ns.unwrap_or_default());
        final_text = response.output.text;
        final_tokens = response
            .output
            .tokens
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect();
    }
    let total_eval_duration = Duration::from_nanos(total_eval_ns.min(u128::from(u64::MAX)) as u64);
    Ok(ThroughputVariantReport {
        variant: String::from("served_runtime"),
        repetitions,
        generated_tokens_per_run: final_tokens.len(),
        total_elapsed_ms: total_eval_duration.as_millis(),
        tokens_per_second: tokens_per_second(final_tokens.len() * repetitions, total_eval_duration),
        output_tokens: final_tokens,
        output_text: final_text,
    })
}

fn decode_direct_runtime(
    bundle: &ParameterGolfPromotedRuntimeBundle,
    prompt_tokens: &TokenSequence,
    max_new_tokens: usize,
) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
    let total_history_capacity = prompt_tokens
        .len()
        .saturating_add(max_new_tokens)
        .min(bundle.generation_config().max_context);
    let mut history = Vec::with_capacity(total_history_capacity);
    history.extend(prompt_tokens.as_slice().iter().map(|token| token.as_u32()));
    let mut bounded_history = Vec::with_capacity(
        bundle
            .generation_config()
            .bounded_attention_window_tokens
            .unwrap_or(bundle.generation_config().max_context),
    );
    let mut generated = Vec::with_capacity(max_new_tokens);
    let mut sampler =
        TokenSampler::new(&bundle.default_greedy_generation_options().sampling_policy);
    let generation_allowed_token_ids = bundle.tokenizer().generation_allowed_token_ids();
    while generated.len() < max_new_tokens && history.len() < bundle.generation_config().max_context
    {
        let logits = if let Some(window_tokens) =
            bundle.generation_config().bounded_attention_window_tokens
        {
            let start = history.len().saturating_sub(window_tokens);
            bounded_history.clear();
            bounded_history.extend_from_slice(&history[start..]);
            bundle.model().forward_logits_with_attention_window(
                std::slice::from_ref(&bounded_history),
                bounded_history.len(),
            )?
        } else {
            bundle.model().forward_logits(std::slice::from_ref(&history))?
        };
        let width = logits.width();
        let sequence_length = logits.sequence_length();
        let last_row_start = sequence_length
            .checked_sub(1)
            .ok_or("missing logits")?
            .saturating_mul(width);
        let last_logits = logits
            .values()
            .get(last_row_start..last_row_start.saturating_add(width))
            .ok_or("missing logits")?;
        let next_token = sampler
            .select_next_token_with_allowed(
                last_logits,
                history.as_slice(),
                generation_allowed_token_ids,
            )
            .ok_or("missing logits")?;
        history.push(next_token);
        generated.push(next_token);
    }
    Ok(generated)
}

fn decode_text(bundle: &ParameterGolfPromotedRuntimeBundle, token_ids: &[u32]) -> String {
    let tokens = token_ids
        .iter()
        .copied()
        .map(TokenId)
        .collect::<Vec<_>>();
    bundle.tokenizer().decode(tokens.as_slice())
}

fn tokens_per_second(token_count: usize, elapsed: Duration) -> f64 {
    let elapsed_seconds = elapsed.as_secs_f64();
    if elapsed_seconds == 0.0 {
        0.0
    } else {
        token_count as f64 / elapsed_seconds
    }
}
