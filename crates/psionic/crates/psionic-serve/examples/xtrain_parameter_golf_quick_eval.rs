use std::{
    env, fs,
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
use psionic_train::{
    ParameterGolfLocalReferenceFixture, ParameterGolfPromotedProfileAssumption,
    ParameterGolfReferenceTrainingConfig,
    build_parameter_golf_promoted_inference_promotion_receipt,
    run_parameter_golf_promoted_reference_run, run_parameter_golf_promoted_xtrain_reference_run,
    write_parameter_golf_promoted_reference_run,
};
use serde::Serialize;

const REPORT_SCHEMA_VERSION: &str = "psionic.xtrain_parameter_golf_quick_eval_report.v1";

#[derive(Clone, Debug, Serialize)]
struct QualityLaneReport {
    label: String,
    bundle_dir: String,
    run_id: String,
    final_checkpoint_ref: String,
    receipt_digest: String,
    initial_validation_mean_loss: f64,
    final_validation_mean_loss: f64,
    final_validation_bits_per_byte: f64,
    prompt_text: String,
    prompt_tokens: Vec<u32>,
    expected_tokens: Vec<u32>,
    generated_tokens: Vec<u32>,
    generated_text: String,
    exact_prefix_match_tokens: usize,
    exact_cycle_match: bool,
}

#[derive(Clone, Debug, Serialize)]
struct QualityComparisonReport {
    proof: QualityLaneReport,
    xtrain: QualityLaneReport,
    validation_loss_improvement: f64,
    bits_per_byte_improvement: f64,
    exact_prefix_match_gain: isize,
}

#[derive(Clone, Debug, Serialize)]
struct ThroughputVariantReport {
    variant: String,
    repetitions: usize,
    generated_tokens_per_run: usize,
    total_elapsed_ms: u128,
    tokens_per_second: f64,
    output_tokens: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
struct ThroughputComparisonReport {
    prompt_tokens: Vec<u32>,
    direct_history_ref: ThroughputVariantReport,
    served_runtime: ThroughputVariantReport,
    direct_and_served_match: bool,
}

#[derive(Clone, Debug, Serialize)]
struct XtrainParameterGolfQuickEvalReport {
    schema_version: String,
    proof_bundle_dir: String,
    xtrain_bundle_dir: String,
    comparison: QualityComparisonReport,
    throughput: ThroughputComparisonReport,
    detail: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let output_root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/psionic_xtrain_parameter_golf_quick_eval"));
    let proof_bundle_dir = output_root.join("proof_bundle");
    let xtrain_bundle_dir = output_root.join("xtrain_bundle");
    let report_path = output_root.join("xtrain_parameter_golf_quick_eval_report.json");
    fs::create_dir_all(&output_root)?;
    remove_dir_if_exists(&proof_bundle_dir)?;
    remove_dir_if_exists(&xtrain_bundle_dir)?;

    let fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let prompt_tokens = token_sequence(&[1, 2, 3, 4]);
    let expected_tokens = vec![5, 6, 7, 8, 1, 2, 3, 4];

    let proof = run_lane(
        String::from("proof"),
        &fixture,
        &ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder(),
        &proof_bundle_dir,
        &prompt_tokens,
        expected_tokens.as_slice(),
        false,
    )?;
    let xtrain = run_lane(
        String::from("xtrain"),
        &fixture,
        &ParameterGolfReferenceTrainingConfig::xtrain_promoted_general_small_decoder_baseline(),
        &xtrain_bundle_dir,
        &prompt_tokens,
        expected_tokens.as_slice(),
        true,
    )?;

    let comparison = QualityComparisonReport {
        validation_loss_improvement: proof.final_validation_mean_loss
            - xtrain.final_validation_mean_loss,
        bits_per_byte_improvement: proof.final_validation_bits_per_byte
            - xtrain.final_validation_bits_per_byte,
        exact_prefix_match_gain: xtrain.exact_prefix_match_tokens as isize
            - proof.exact_prefix_match_tokens as isize,
        proof,
        xtrain,
    };

    let xtrain_bundle = ParameterGolfPromotedRuntimeBundle::load_dir(&xtrain_bundle_dir)?;
    let throughput_tokens = 16;
    let repetitions = 2;
    let direct_history_ref = benchmark_direct_runtime(
        &xtrain_bundle,
        &prompt_tokens,
        throughput_tokens,
        repetitions,
    )?;
    let served_runtime = benchmark_served_runtime(
        &xtrain_bundle_dir,
        &prompt_tokens,
        throughput_tokens,
        repetitions,
    )?;
    let throughput = ThroughputComparisonReport {
        prompt_tokens: prompt_tokens
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        direct_and_served_match: direct_history_ref.output_tokens == served_runtime.output_tokens,
        direct_history_ref,
        served_runtime,
    };

    let report = XtrainParameterGolfQuickEvalReport {
        schema_version: String::from(REPORT_SCHEMA_VERSION),
        proof_bundle_dir: proof_bundle_dir.display().to_string(),
        xtrain_bundle_dir: xtrain_bundle_dir.display().to_string(),
        comparison,
        throughput,
        detail: String::from(
            "This quick report compares the original promoted proof config against the stronger bounded XTRAIN PGOLF baseline on the same repo-owned cycle fixture, then benchmarks only the direct-history and served runtime paths so XTRAIN tuning can iterate without paying the full legacy-clone benchmark cost every time.",
        ),
    };
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "xtrain PGOLF quick eval completed: report={} xtrain_loss={:.6} direct_tps={:.2}",
        report_path.display(),
        report.comparison.xtrain.final_validation_mean_loss,
        report.throughput.direct_history_ref.tokens_per_second,
    );
    Ok(())
}

fn run_lane(
    label: String,
    fixture: &ParameterGolfLocalReferenceFixture,
    config: &ParameterGolfReferenceTrainingConfig,
    bundle_dir: &Path,
    prompt_tokens: &TokenSequence,
    expected_tokens: &[u32],
    allow_resume_divergence: bool,
) -> Result<QualityLaneReport, Box<dyn std::error::Error>> {
    let run = if allow_resume_divergence {
        run_parameter_golf_promoted_xtrain_reference_run(fixture, config)?
    } else {
        run_parameter_golf_promoted_reference_run(fixture, config)?
    };
    write_parameter_golf_promoted_reference_run(&run, bundle_dir)?;
    let receipt = build_parameter_golf_promoted_inference_promotion_receipt(
        bundle_dir,
        ParameterGolfPromotedProfileAssumption::GeneralPsionSmallDecoder,
    )?;
    let bundle = ParameterGolfPromotedRuntimeBundle::load_dir(bundle_dir)?;
    let mut options = bundle.default_greedy_generation_options();
    options.max_new_tokens = expected_tokens.len();
    let output = bundle.generate_tokens(prompt_tokens.clone(), &options)?;
    let generated_tokens = output
        .generated_tokens
        .as_slice()
        .iter()
        .map(|token| token.as_u32())
        .collect::<Vec<_>>();
    Ok(QualityLaneReport {
        label,
        bundle_dir: bundle_dir.display().to_string(),
        run_id: config.run_id.clone(),
        final_checkpoint_ref: run.summary.final_checkpoint_ref.clone(),
        receipt_digest: receipt.receipt_digest,
        initial_validation_mean_loss: run.training_outcome.summary.initial_validation_mean_loss,
        final_validation_mean_loss: run.training_outcome.summary.final_validation_mean_loss,
        final_validation_bits_per_byte: run.training_outcome.summary.final_validation_bits_per_byte,
        prompt_text: bundle.tokenizer().decode(prompt_tokens.as_slice()),
        prompt_tokens: prompt_tokens
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        expected_tokens: expected_tokens.to_vec(),
        exact_prefix_match_tokens: prefix_match_len(&generated_tokens, expected_tokens),
        exact_cycle_match: generated_tokens == expected_tokens,
        generated_tokens,
        generated_text: output.text,
    })
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
        output_tokens: final_tokens,
    })
}

fn benchmark_served_runtime(
    bundle_dir: &Path,
    prompt_tokens: &TokenSequence,
    max_new_tokens: usize,
    repetitions: usize,
) -> Result<ThroughputVariantReport, Box<dyn std::error::Error>> {
    let mut service = CpuPromotedParameterGolfTextGenerationService::from_bundle_dir(bundle_dir)?;
    let descriptor = service.model_descriptor().clone();
    let mut total_eval_ns = 0_u128;
    let mut final_tokens = Vec::new();
    for repetition in 0..repetitions {
        let request = GenerationRequest::new_tokens(
            format!("xtrain-served-quick-{repetition}"),
            descriptor.clone(),
            None,
            prompt_tokens.clone(),
            GenerationOptions::greedy(max_new_tokens),
        );
        let response = service.generate(&request)?;
        total_eval_ns += u128::from(response.metrics.eval_duration_ns.unwrap_or_default());
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
            bundle
                .model()
                .forward_logits(std::slice::from_ref(&history))?
        };
        let width = logits.width();
        let sequence_length = logits.sequence_length();
        let last_row_start = sequence_length
            .checked_sub(1)
            .ok_or("missing logits row")?
            .saturating_mul(width);
        let last_logits = logits
            .values()
            .get(last_row_start..last_row_start.saturating_add(width))
            .ok_or("missing logits slice")?;
        let next_token = sampler
            .select_next_token_with_allowed(
                last_logits,
                history.as_slice(),
                generation_allowed_token_ids,
            )
            .ok_or("missing next token")?;
        let next_token = TokenId(next_token);
        history.push(next_token.as_u32());
        generated.push(next_token.as_u32());
    }
    Ok(generated)
}

fn token_sequence(token_ids: &[u32]) -> TokenSequence {
    TokenSequence::new(token_ids.iter().copied().map(TokenId).collect::<Vec<_>>())
}

fn prefix_match_len(actual: &[u32], expected: &[u32]) -> usize {
    actual
        .iter()
        .zip(expected.iter())
        .take_while(|(left, right)| left == right)
        .count()
}

fn remove_dir_if_exists(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn tokens_per_second(total_tokens: usize, duration: Duration) -> f64 {
    let seconds = duration.as_secs_f64();
    if seconds <= f64::EPSILON {
        0.0
    } else {
        total_tokens as f64 / seconds
    }
}
