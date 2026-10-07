use std::{env, fs, path::PathBuf, process::ExitCode, time::Instant};

use psionic_models::{
    MedPsyModelSize, MedPsyQwen3CandleConfig, MedPsyQwen3CandleGenerator,
    MedPsyQwen3GgufGenerator, MedPsyQwen3RuntimeBackend, TokenId,
};
use serde::Serialize;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    configure_cuda_quantized_kernel_policy();
    let config = BenchConfig::parse(env::args().skip(1))?;
    let report = run_benchmark(&config)?;
    if let Some(path) = config.json_out.as_ref() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("failed to create benchmark output directory {}: {error}", parent.display())
            })?;
        }
        let body = serde_json::to_vec_pretty(&report)
            .map_err(|error| format!("failed to serialize benchmark report: {error}"))?;
        fs::write(path, body)
            .map_err(|error| format!("failed to write benchmark report {}: {error}", path.display()))?;
    }
    if config.json_stdout {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| format!("failed to serialize report: {error}"))?
        );
    } else {
        println!("{}", render_human_report(&report));
    }
    Ok(())
}

fn configure_cuda_quantized_kernel_policy() {
    #[cfg(feature = "medpsy-cuda")]
    {
        if std::env::var("PSIONIC_MEDPSY_FORCE_DMMV")
            .ok()
            .is_some_and(|value| !value.is_empty() && value != "0")
        {
            candle::quantized::cuda::set_force_dmmv(true);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ArtifactKind {
    Safetensors,
    Gguf,
}

impl ArtifactKind {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "safetensors" => Ok(Self::Safetensors),
            "gguf" => Ok(Self::Gguf),
            other => Err(format!(
                "unsupported --artifact-kind `{other}`; expected safetensors or gguf"
            )),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Safetensors => "safetensors",
            Self::Gguf => "gguf",
        }
    }
}

#[derive(Clone, Debug)]
struct BenchConfig {
    model_path: PathBuf,
    artifact_kind: ArtifactKind,
    model_size: MedPsyModelSize,
    backend: BenchBackend,
    prompt_tokens: Vec<TokenId>,
    max_new_tokens: usize,
    repeats: usize,
    json_stdout: bool,
    json_out: Option<PathBuf>,
}

impl BenchConfig {
    fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut model_path = None::<PathBuf>;
        let mut artifact_kind = ArtifactKind::Gguf;
        let mut model_size = MedPsyModelSize::OnePointSevenB;
        let mut backend = BenchBackend::Cpu;
        let mut prompt_tokens = vec![TokenId(151644)];
        let mut max_new_tokens = 1usize;
        let mut repeats = 1usize;
        let mut json_stdout = false;
        let mut json_out = None::<PathBuf>;

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--model-path" => {
                    model_path = Some(PathBuf::from(
                        args.next().ok_or_else(|| String::from("missing value for --model-path"))?,
                    ));
                }
                "--artifact-kind" => {
                    artifact_kind = ArtifactKind::parse(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --artifact-kind"))?
                            .as_str(),
                    )?;
                }
                "--model-size" => {
                    model_size = parse_model_size(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --model-size"))?
                            .as_str(),
                    )?;
                }
                "--backend" => {
                    backend = BenchBackend::parse(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --backend"))?
                            .as_str(),
                    )?;
                }
                "--prompt-token-ids" => {
                    prompt_tokens = parse_token_ids(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --prompt-token-ids"))?
                            .as_str(),
                    )?;
                }
                "--max-new-tokens" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --max-new-tokens"))?;
                    max_new_tokens = value
                        .parse::<usize>()
                        .map_err(|error| format!("invalid --max-new-tokens `{value}`: {error}"))?;
                }
                "--repeats" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --repeats"))?;
                    repeats = value
                        .parse::<usize>()
                        .map_err(|error| format!("invalid --repeats `{value}`: {error}"))?;
                }
                "--json" => json_stdout = true,
                "--json-out" => {
                    json_out = Some(PathBuf::from(
                        args.next().ok_or_else(|| String::from("missing value for --json-out"))?,
                    ));
                }
                "--help" | "-h" => return Err(usage().to_string()),
                other => return Err(format!("unexpected argument `{other}`\n\n{}", usage())),
            }
        }

        let model_path = model_path.ok_or_else(|| String::from("missing required --model-path"))?;
        if prompt_tokens.is_empty() {
            return Err(String::from("--prompt-token-ids must contain at least one token id"));
        }
        if max_new_tokens == 0 {
            return Err(String::from("--max-new-tokens must be greater than zero"));
        }
        if repeats == 0 {
            return Err(String::from("--repeats must be greater than zero"));
        }
        Ok(Self {
            model_path,
            artifact_kind,
            model_size,
            backend,
            prompt_tokens,
            max_new_tokens,
            repeats,
            json_stdout,
            json_out,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum BenchBackend {
    Cpu,
    Cuda,
}

impl BenchBackend {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "cpu" => Ok(Self::Cpu),
            "cuda" => Ok(Self::Cuda),
            other => Err(format!("unsupported --backend `{other}`; expected cpu or cuda")),
        }
    }

    fn runtime_backend(self) -> Result<MedPsyQwen3RuntimeBackend, String> {
        match self {
            Self::Cpu => Ok(MedPsyQwen3RuntimeBackend::Cpu),
            Self::Cuda => {
                #[cfg(feature = "medpsy-cuda")]
                {
                    Ok(MedPsyQwen3RuntimeBackend::Cuda { device_ordinal: 0 })
                }
                #[cfg(not(feature = "medpsy-cuda"))]
                {
                    Err(String::from(
                        "--backend cuda requires building psionic-models with --features medpsy-cuda",
                    ))
                }
            }
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct MedPsyBenchReport {
    schema: &'static str,
    model_path: String,
    model_size: &'static str,
    artifact_kind: &'static str,
    backend: &'static str,
    execution_engine: String,
    prompt_tokens: Vec<u32>,
    max_new_tokens: usize,
    repeats: usize,
    runs: Vec<MedPsyBenchRun>,
    mean_total_ms: f64,
    mean_decode_tokens_per_second: f64,
    medical_policy: MedPsyBenchMedicalPolicy,
}

#[derive(Clone, Debug, Serialize)]
struct MedPsyBenchRun {
    run_index: usize,
    generated_tokens: Vec<u32>,
    output_tokens: usize,
    stopped_on_eos: bool,
    total_ms: f64,
    decode_tokens_per_second: f64,
    model_artifact_sha256: String,
    execution_engine: String,
}

#[derive(Clone, Debug, Serialize)]
struct MedPsyBenchMedicalPolicy {
    policy_id: &'static str,
    default_classification: &'static str,
    disclaimer_required: bool,
    direct_diagnosis_allowed: bool,
    prescribing_or_treatment_authority_allowed: bool,
    benchmark_claim_boundary: &'static str,
}

fn run_benchmark(config: &BenchConfig) -> Result<MedPsyBenchReport, String> {
    let mut runs = Vec::with_capacity(config.repeats);
    let mut execution_engine = String::new();
    match config.artifact_kind {
        ArtifactKind::Safetensors => {
            let qwen_config = MedPsyQwen3CandleConfig::from_size(config.model_size);
            let mut generator = MedPsyQwen3CandleGenerator::from_safetensors_file_with_backend(
                qwen_config,
                &config.model_path,
                None,
                config.backend.runtime_backend()?,
            )
            .map_err(|error| error.to_string())?;
            for run_index in 0..config.repeats {
                let started = Instant::now();
                let report = generator
                    .generate_greedy_token_ids(
                        &config.prompt_tokens,
                        config.max_new_tokens,
                        &[TokenId(151645)],
                    )
                    .map_err(|error| error.to_string())?;
                push_run(&mut runs, run_index, report, started.elapsed().as_secs_f64() * 1000.0);
            }
        }
        ArtifactKind::Gguf => {
            let mut generator = MedPsyQwen3GgufGenerator::from_gguf_file_with_backend(
                &config.model_path,
                None,
                config.backend.runtime_backend()?,
            )
            .map_err(|error| error.to_string())?;
            for run_index in 0..config.repeats {
                let started = Instant::now();
                let report = generator
                    .generate_greedy_token_ids(
                        &config.prompt_tokens,
                        config.max_new_tokens,
                        &[TokenId(151645)],
                    )
                    .map_err(|error| error.to_string())?;
                push_run(&mut runs, run_index, report, started.elapsed().as_secs_f64() * 1000.0);
            }
        }
    }
    if let Some(first) = runs.first() {
        execution_engine = first.execution_engine.clone();
    }
    let mean_total_ms = runs.iter().map(|run| run.total_ms).sum::<f64>() / runs.len() as f64;
    let mean_decode_tokens_per_second = runs
        .iter()
        .map(|run| run.decode_tokens_per_second)
        .sum::<f64>()
        / runs.len() as f64;
    Ok(MedPsyBenchReport {
        schema: "psionic.medpsy.bench.v1",
        model_path: config.model_path.display().to_string(),
        model_size: model_size_label(config.model_size),
        artifact_kind: config.artifact_kind.label(),
        backend: config.backend.label(),
        execution_engine,
        prompt_tokens: config.prompt_tokens.iter().map(|token| token.as_u32()).collect(),
        max_new_tokens: config.max_new_tokens,
        repeats: config.repeats,
        runs,
        mean_total_ms,
        mean_decode_tokens_per_second,
        medical_policy: MedPsyBenchMedicalPolicy {
            policy_id: "medical_model_use.medpsy.v1",
            default_classification: "medical_information_not_diagnosis",
            disclaimer_required: true,
            direct_diagnosis_allowed: false,
            prescribing_or_treatment_authority_allowed: false,
            benchmark_claim_boundary: "local_runtime_benchmark_only_not_clinical_quality_claim",
        },
    })
}

fn push_run(
    runs: &mut Vec<MedPsyBenchRun>,
    run_index: usize,
    report: psionic_models::MedPsyQwen3GenerationReport,
    elapsed: f64,
) {
    let output_tokens = report.generated_tokens.len();
    runs.push(MedPsyBenchRun {
        run_index,
        generated_tokens: report
            .generated_tokens
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        output_tokens,
        stopped_on_eos: report.stopped_on_eos,
        total_ms: elapsed,
        decode_tokens_per_second: if elapsed > 0.0 {
            (output_tokens as f64) / (elapsed / 1000.0)
        } else {
            0.0
        },
        model_artifact_sha256: report.model_artifact_sha256,
        execution_engine: report.execution_engine,
    });
}

fn parse_model_size(value: &str) -> Result<MedPsyModelSize, String> {
    match value {
        "1.7b" | "1_7b" | "one_point_seven_b" => Ok(MedPsyModelSize::OnePointSevenB),
        "4b" | "four_b" => Ok(MedPsyModelSize::FourB),
        other => Err(format!("unsupported --model-size `{other}`; expected 1.7b or 4b")),
    }
}

fn model_size_label(value: MedPsyModelSize) -> &'static str {
    match value {
        MedPsyModelSize::OnePointSevenB => "1.7b",
        MedPsyModelSize::FourB => "4b",
    }
}

fn parse_token_ids(value: &str) -> Result<Vec<TokenId>, String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<u32>()
                .map(TokenId)
                .map_err(|error| format!("invalid token id `{part}`: {error}"))
        })
        .collect()
}

fn render_human_report(report: &MedPsyBenchReport) -> String {
    format!(
        "MedPsy bench {artifact} {size}: {runs} runs, mean {ms:.2} ms, mean {tok:.2} tok/s, engine {engine}",
        artifact = report.artifact_kind,
        size = report.model_size,
        runs = report.repeats,
        ms = report.mean_total_ms,
        tok = report.mean_decode_tokens_per_second,
        engine = report.execution_engine,
    )
}

fn usage() -> &'static str {
    "usage: cargo run -p psionic-models --example medpsy_bench -- --model-path <path> [--artifact-kind safetensors|gguf] [--model-size 1.7b|4b] [--backend cpu|cuda] [--prompt-token-ids 151644] [--max-new-tokens 1] [--repeats 1] [--json|--json-out path]"
}
