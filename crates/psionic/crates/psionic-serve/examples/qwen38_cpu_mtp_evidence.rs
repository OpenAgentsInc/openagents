use std::{env, error::Error, fs, path::PathBuf, process::ExitCode, time::Instant};

use psionic_serve::{
    CpuGgufQwen35TextGenerationService, GenerationOptions, GenerationRequest, Qwen38MtpConfig,
    Qwen38MtpExecutionReport, TextGenerationExecutor, TokenId, TokenSequence,
};
use serde::Serialize;

const SCHEMA_VERSION: &str = "psionic.qwen38.mtp_real_artifact_evidence.v1";
const ARTIFACT_SHA256: &str = "00cf92e666c6af6566996c38c89a44ccdb6449ea25ef0f112a452c853b2a71e2";
const ARTIFACT_BYTES: u64 = 13_441_059_904;
const PROMPT_TOKENS: [u32; 2] = [9419, 11];
const OUTPUT_TOKENS: usize = 2;

#[derive(Debug, Serialize)]
struct SourceIdentity {
    revision: String,
    dirty: bool,
}

#[derive(Debug, Serialize)]
struct ArtifactIdentity {
    path: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct RunObservation {
    plan_digest: String,
    output_token_ids: Vec<u32>,
    output_text: String,
    prompt_eval_duration_ns: u64,
    decode_duration_ns: u64,
    decode_tokens_per_second: f64,
    wall_duration_ns: u64,
}

#[derive(Debug, Serialize)]
struct CorrectnessObservation {
    output_token_parity: bool,
    output_text_parity: bool,
    restored_state_parity: bool,
    passed: bool,
}

#[derive(Debug, Serialize)]
struct PerformanceObservation {
    baseline_decode_tokens_per_second: f64,
    mtp_decode_tokens_per_second: f64,
    mtp_to_baseline_ratio: f64,
    observed_outcome: String,
    acceleration_claimed: bool,
}

#[derive(Debug, Serialize)]
struct EvidenceReport {
    schema_version: String,
    source: SourceIdentity,
    artifact: ArtifactIdentity,
    backend: String,
    prompt_token_ids: Vec<u32>,
    max_output_tokens: usize,
    baseline: RunObservation,
    mtp: RunObservation,
    mtp_execution: Qwen38MtpExecutionReport,
    correctness: CorrectnessObservation,
    performance: PerformanceObservation,
    all_passed: bool,
    claim_boundary: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let model_path = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: qwen38_cpu_mtp_evidence MODEL_GGUF REPORT_JSON")?;
    let report_path = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: qwen38_cpu_mtp_evidence MODEL_GGUF REPORT_JSON")?;
    if args.next().is_some() {
        return Err("usage: qwen38_cpu_mtp_evidence MODEL_GGUF REPORT_JSON".into());
    }
    let byte_length = fs::metadata(&model_path)?.len();
    if byte_length != ARTIFACT_BYTES {
        return Err(format!(
            "selected Qwen3.8 artifact byte length mismatch: expected {ARTIFACT_BYTES}, actual {byte_length}"
        )
        .into());
    }
    let prompt = TokenSequence::new(PROMPT_TOKENS.into_iter().map(TokenId).collect::<Vec<_>>());

    let mut baseline = CpuGgufQwen35TextGenerationService::from_gguf_path(&model_path)?;
    let descriptor = baseline.model_descriptor().clone();
    let baseline_request = GenerationRequest::new_tokens(
        "qwen38-mtp-real-artifact-baseline",
        descriptor,
        None,
        prompt.clone(),
        GenerationOptions::greedy(OUTPUT_TOKENS),
    );
    let baseline_started = Instant::now();
    let baseline_response = baseline.generate(&baseline_request)?;
    let baseline_wall_ns = duration_ns(baseline_started.elapsed());
    let baseline_plan = baseline
        .plan_digest(baseline.model_descriptor().model.model_id.as_str())
        .unwrap_or("missing")
        .to_string();
    let baseline_observation = run_observation(baseline_plan, &baseline_response, baseline_wall_ns);
    drop(baseline);

    let mut mtp = CpuGgufQwen35TextGenerationService::from_gguf_path_with_qwen38_mtp(
        &model_path,
        Qwen38MtpConfig::single_token(),
    )?;
    let mtp_request = GenerationRequest::new_tokens(
        "qwen38-mtp-real-artifact-enabled",
        mtp.model_descriptor().clone(),
        None,
        prompt,
        GenerationOptions::greedy(OUTPUT_TOKENS),
    );
    let mtp_started = Instant::now();
    let mtp_response = mtp.generate(&mtp_request)?;
    let mtp_wall_ns = duration_ns(mtp_started.elapsed());
    let mtp_plan = mtp
        .plan_digest(mtp.model_descriptor().model.model_id.as_str())
        .unwrap_or("missing")
        .to_string();
    let mtp_observation = run_observation(mtp_plan, &mtp_response, mtp_wall_ns);
    let mtp_execution = mtp
        .last_qwen38_mtp_report()
        .ok_or("MTP generation did not retain execution accounting")?
        .clone();

    let output_token_parity = baseline_response.output.tokens == mtp_response.output.tokens;
    let output_text_parity = baseline_response.output.text == mtp_response.output.text;
    let correctness = CorrectnessObservation {
        output_token_parity,
        output_text_parity,
        restored_state_parity: mtp_execution.restored_state_parity,
        passed: output_token_parity && output_text_parity && mtp_execution.restored_state_parity,
    };
    let baseline_tps = baseline_observation.decode_tokens_per_second;
    let mtp_tps = mtp_observation.decode_tokens_per_second;
    let ratio = if baseline_tps == 0.0 {
        0.0
    } else {
        mtp_tps / baseline_tps
    };
    let performance = PerformanceObservation {
        baseline_decode_tokens_per_second: baseline_tps,
        mtp_decode_tokens_per_second: mtp_tps,
        mtp_to_baseline_ratio: ratio,
        observed_outcome: if ratio > 1.0 {
            String::from("single_run_speedup_observed_not_claimed")
        } else {
            String::from("slowdown_observed")
        },
        acceleration_claimed: false,
    };
    let all_passed = correctness.passed
        && mtp_execution.draft_count > 0
        && mtp_execution.mtp_forward_count
            == mtp_execution
                .draft_count
                .saturating_add(mtp_execution.mtp_alignment_forward_count)
        && mtp_execution.mtp_alignment_forward_count == mtp_execution.accepted_count
        && mtp_execution.accepted_count + mtp_execution.rejected_count == mtp_execution.draft_count
        && mtp_execution.mtp_weight_residency_bytes > 0
        && mtp_execution.mtp_kv_cache_peak_bytes > 0
        && mtp_execution.rollback_snapshot_peak_bytes > 0;
    let report = EvidenceReport {
        schema_version: String::from(SCHEMA_VERSION),
        source: SourceIdentity {
            revision: env::var("PSIONIC_SOURCE_REVISION")
                .unwrap_or_else(|_| String::from("unknown")),
            dirty: env::var("PSIONIC_SOURCE_DIRTY").map_or(true, |value| value != "false"),
        },
        artifact: ArtifactIdentity {
            path: model_path.display().to_string(),
            byte_length,
            sha256: String::from(ARTIFACT_SHA256),
        },
        backend: String::from("native_psionic_cpu"),
        prompt_token_ids: PROMPT_TOKENS.to_vec(),
        max_output_tokens: OUTPUT_TOKENS,
        baseline: baseline_observation,
        mtp: mtp_observation,
        mtp_execution,
        correctness,
        performance,
        all_passed,
        claim_boundary: String::from(
            "This retained CPU run proves optional NextN loading, aligned draft execution, target-state rollback, disabled-path output parity, memory accounting, and measured performance on the selected Qwen3.8 artifact. Token-at-a-time target verification is a correctness implementation and does not claim MTP acceleration, CUDA MTP, sampling MTP, or production speculative throughput.",
        ),
    };
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "passed={} drafts={} accepted={} rollbacks={} baseline_tps={:.6} mtp_tps={:.6} report={}",
        report.all_passed,
        report.mtp_execution.draft_count,
        report.mtp_execution.accepted_count,
        report.mtp_execution.rollback_count,
        report.performance.baseline_decode_tokens_per_second,
        report.performance.mtp_decode_tokens_per_second,
        report_path.display(),
    );
    Ok(report.all_passed)
}

fn run_observation(
    plan_digest: String,
    response: &psionic_serve::GenerationResponse,
    wall_duration_ns: u64,
) -> RunObservation {
    let decode_duration_ns = response.metrics.eval_duration_ns.unwrap_or_default();
    let decode_tokens_per_second = if decode_duration_ns == 0 {
        0.0
    } else {
        response.output.tokens.len() as f64 * 1_000_000_000.0 / decode_duration_ns as f64
    };
    RunObservation {
        plan_digest,
        output_token_ids: response
            .output
            .tokens
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        output_text: response.output.text.clone(),
        prompt_eval_duration_ns: response.metrics.prompt_eval_duration_ns.unwrap_or_default(),
        decode_duration_ns,
        decode_tokens_per_second,
        wall_duration_ns,
    }
}

fn duration_ns(duration: std::time::Duration) -> u64 {
    duration.as_nanos().try_into().unwrap_or(u64::MAX)
}
