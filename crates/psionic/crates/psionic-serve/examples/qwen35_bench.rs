use std::{env, fs, path::PathBuf, process::ExitCode};

use psionic_models::{
    GgufDecoderAdapterLoader, GgufRuntimeTokenizer, PromptMessage, PromptMessageRole,
    TokenizerBoundary,
};
use psionic_serve::{
    CpuGgufTextGenerationService, CudaGgufQwen35TextGenerationService, GenerationOptions,
    GenerationRequest, GenerationResponse, MetalGgufQwen35TextGenerationService,
    Qwen35MetalRuntimeContract, TerminationReason, TextGenerationExecutor,
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
    let config = BenchConfig::parse(env::args().skip(1))?;
    let report = run_benchmark(&config)?;
    if let Some(json_out) = config.json_out.as_ref() {
        if let Some(parent) = json_out.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create benchmark output directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let body = serde_json::to_vec_pretty(&report)
            .map_err(|error| format!("failed to serialize benchmark report: {error}"))?;
        fs::write(json_out, body).map_err(|error| {
            format!(
                "failed to write benchmark report to {}: {error}",
                json_out.display()
            )
        })?;
    }
    if config.json_stdout {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| format!("failed to serialize benchmark report: {error}"))?
        );
    } else {
        println!("{}", render_human_report(&report));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BenchBackend {
    Cpu,
    Cuda,
    Metal,
}

impl BenchBackend {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "cpu" => Ok(Self::Cpu),
            "cuda" => Ok(Self::Cuda),
            "metal" => Ok(Self::Metal),
            other => Err(format!(
                "unsupported qwen35 backend `{other}`; expected one of: cpu, cuda, metal"
            )),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::Metal => "metal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum BenchPromptMode {
    ChatTemplate,
    RawText,
}

#[derive(Clone, Debug)]
struct BenchConfig {
    model_path: PathBuf,
    backend: BenchBackend,
    prompt: String,
    prompt_mode: BenchPromptMode,
    max_output_tokens: usize,
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
        let mut backend = BenchBackend::Cpu;
        let mut prompt = String::from("Write one short sentence about open source AI.");
        let mut prompt_mode = BenchPromptMode::ChatTemplate;
        let mut max_output_tokens = 64usize;
        let mut repeats = 1usize;
        let mut json_stdout = false;
        let mut json_out = None::<PathBuf>;

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--model-path" => {
                    model_path =
                        Some(PathBuf::from(args.next().ok_or_else(|| {
                            String::from("missing value for --model-path")
                        })?));
                }
                "--backend" => {
                    backend = BenchBackend::parse(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --backend"))?
                            .as_str(),
                    )?;
                }
                "--prompt" => {
                    prompt = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --prompt"))?;
                }
                "--raw-prompt" => {
                    prompt_mode = BenchPromptMode::RawText;
                }
                "--max-output-tokens" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --max-output-tokens"))?;
                    max_output_tokens = value.parse::<usize>().map_err(|error| {
                        format!("invalid --max-output-tokens `{value}`: {error}")
                    })?;
                }
                "--repeats" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --repeats"))?;
                    repeats = value
                        .parse::<usize>()
                        .map_err(|error| format!("invalid --repeats `{value}`: {error}"))?;
                }
                "--json" => {
                    json_stdout = true;
                }
                "--json-out" => {
                    json_out =
                        Some(PathBuf::from(args.next().ok_or_else(|| {
                            String::from("missing value for --json-out")
                        })?));
                }
                "--help" | "-h" => return Err(usage().to_string()),
                other => return Err(format!("unexpected argument `{other}`\n\n{}", usage())),
            }
        }

        let model_path = model_path.ok_or_else(|| String::from("missing required --model-path"))?;
        if !model_path.exists() {
            return Err(format!(
                "model path does not exist: {}",
                model_path.display()
            ));
        }
        if repeats == 0 {
            return Err(String::from("--repeats must be at least 1"));
        }
        Ok(Self {
            model_path,
            backend,
            prompt,
            prompt_mode,
            max_output_tokens,
            repeats,
            json_stdout,
            json_out,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
struct BenchReport {
    schema_version: u32,
    report_kind: String,
    backend: String,
    model_id: String,
    model_path: String,
    prompt: String,
    prompt_mode: BenchPromptMode,
    prompt_token_ids: Vec<u32>,
    max_output_tokens: usize,
    repeats: usize,
    load_s: f64,
    runs: Vec<BenchRunReport>,
    mean_output_tokens: f64,
    mean_total_s: f64,
    mean_ttft_s: Option<f64>,
    mean_decode_tok_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metal_runtime_contract: Option<Qwen35MetalRuntimeContract>,
}

#[derive(Clone, Debug, Serialize)]
struct BenchRunReport {
    run_index: usize,
    output_tokens: usize,
    output_token_ids: Vec<u32>,
    total_s: f64,
    prompt_s: Option<f64>,
    decode_s: Option<f64>,
    ttft_s: Option<f64>,
    decode_tok_s: Option<f64>,
    termination: String,
    output_text: String,
}

enum BenchRuntime {
    Cpu(CpuGgufTextGenerationService),
    Cuda(CudaGgufQwen35TextGenerationService),
    Metal(MetalGgufQwen35TextGenerationService),
}

impl BenchRuntime {
    fn descriptor(&self) -> psionic_models::DecoderModelDescriptor {
        match self {
            Self::Cpu(service) => service.model_descriptor().clone(),
            Self::Cuda(service) => service.model_descriptor().clone(),
            Self::Metal(service) => service.model_descriptor().clone(),
        }
    }

    fn model_id(&self) -> &str {
        match self {
            Self::Cpu(service) => service.model_descriptor().model.model_id.as_str(),
            Self::Cuda(service) => service.model_descriptor().model.model_id.as_str(),
            Self::Metal(service) => service.model_descriptor().model.model_id.as_str(),
        }
    }

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, String> {
        match self {
            Self::Cpu(service) => service.generate(request).map_err(|error| error.to_string()),
            Self::Cuda(service) => service.generate(request).map_err(|error| error.to_string()),
            Self::Metal(service) => service.generate(request).map_err(|error| error.to_string()),
        }
    }

    fn metal_runtime_contract(&self) -> Option<Qwen35MetalRuntimeContract> {
        match self {
            Self::Metal(service) => Some(service.metal_runtime_contract()),
            Self::Cpu(_) | Self::Cuda(_) => None,
        }
    }
}

fn run_benchmark(config: &BenchConfig) -> Result<BenchReport, String> {
    let adapter = GgufDecoderAdapterLoader
        .load_path(config.model_path.as_path())
        .map_err(|error| {
            format!(
                "failed to inspect GGUF at {}: {error}",
                config.model_path.display()
            )
        })?;
    let prompt_tokens = match config.prompt_mode {
        BenchPromptMode::ChatTemplate => {
            let rendered = adapter
                .render_prompt(
                    None,
                    &[PromptMessage::new(
                        PromptMessageRole::User,
                        config.prompt.clone(),
                    )],
                    true,
                )
                .map_err(|error| format!("failed to render Qwen prompt: {error}"))?;
            adapter
                .prompt_renderer()
                .tokenize_rendered_prompt(rendered.text.as_str())
                .map_err(|error| format!("failed to tokenize rendered prompt: {error}"))?
        }
        BenchPromptMode::RawText => GgufRuntimeTokenizer::from_gguf(adapter.tokenizer())
            .map_err(|error| format!("failed to build Qwen runtime tokenizer: {error}"))?
            .encode(config.prompt.as_str()),
    };

    let load_started = std::time::Instant::now();
    let mut runtime = match config.backend {
        BenchBackend::Cpu => BenchRuntime::Cpu(
            CpuGgufTextGenerationService::from_gguf_path(config.model_path.as_path()).map_err(
                |error| {
                    format!(
                        "failed to load qwen35 cpu service from {}: {error}",
                        config.model_path.display()
                    )
                },
            )?,
        ),
        BenchBackend::Cuda => BenchRuntime::Cuda(
            CudaGgufQwen35TextGenerationService::from_gguf_path(config.model_path.as_path())
                .map_err(|error| {
                    format!(
                        "failed to load qwen35 cuda service from {}: {error}",
                        config.model_path.display()
                    )
                })?,
        ),
        BenchBackend::Metal => BenchRuntime::Metal(
            MetalGgufQwen35TextGenerationService::from_gguf_path(config.model_path.as_path())
                .map_err(|error| {
                    format!(
                        "failed to load qwen35 metal service from {}: {error}",
                        config.model_path.display()
                    )
                })?,
        ),
    };
    let load_s = load_started.elapsed().as_secs_f64();
    let descriptor = runtime.descriptor();
    let metal_runtime_contract = runtime.metal_runtime_contract();
    let mut runs = Vec::with_capacity(config.repeats);
    for run_index in 0..config.repeats {
        let request = GenerationRequest::new_tokens(
            format!("qwen35-bench-{}", run_index + 1),
            descriptor.clone(),
            None,
            prompt_tokens.clone(),
            GenerationOptions::greedy(config.max_output_tokens),
        );
        let started = std::time::Instant::now();
        let response = runtime.generate(&request)?;
        let total_s = started.elapsed().as_secs_f64();
        runs.push(run_report_from_generation(run_index, &response, total_s));
    }
    Ok(finish_report(
        runtime.model_id().to_string(),
        descriptor.model.family.clone(),
        config,
        prompt_tokens
            .as_slice()
            .iter()
            .map(|token: &psionic_models::TokenId| token.as_u32())
            .collect(),
        load_s,
        runs,
        metal_runtime_contract,
    ))
}

fn run_report_from_generation(
    run_index: usize,
    response: &GenerationResponse,
    total_s: f64,
) -> BenchRunReport {
    let metrics = &response.metrics;
    let output_tokens = metrics
        .eval_count
        .unwrap_or_else(|| response.output.tokens.len());
    let decode_s = metrics.eval_duration_ns.map(nanos_to_seconds);
    BenchRunReport {
        run_index,
        output_tokens,
        output_token_ids: response
            .output
            .tokens
            .as_slice()
            .iter()
            .map(|token: &psionic_models::TokenId| token.as_u32())
            .collect(),
        total_s,
        prompt_s: metrics.prompt_eval_duration_ns.map(nanos_to_seconds),
        decode_s,
        ttft_s: metrics.time_to_first_token_ns.map(nanos_to_seconds),
        decode_tok_s: decode_s.and_then(|decode_s| {
            (decode_s > 0.0).then_some(output_tokens as f64 / decode_s.max(f64::MIN_POSITIVE))
        }),
        termination: render_termination(response.termination),
        output_text: response.output.text.clone(),
    }
}

fn finish_report(
    model_id: String,
    model_family: String,
    config: &BenchConfig,
    prompt_token_ids: Vec<u32>,
    load_s: f64,
    runs: Vec<BenchRunReport>,
    metal_runtime_contract: Option<Qwen35MetalRuntimeContract>,
) -> BenchReport {
    let mean_output_tokens = mean(
        runs.iter()
            .map(|run| run.output_tokens as f64)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let mean_total_s = mean(
        runs.iter()
            .map(|run| run.total_s)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let mean_ttft_s = mean_option(
        runs.iter()
            .map(|run| run.ttft_s)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let mean_decode_tok_s = mean_option(
        runs.iter()
            .map(|run| run.decode_tok_s)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    BenchReport {
        schema_version: 1,
        report_kind: format!("psionic_{model_family}_benchmark"),
        backend: config.backend.label().to_string(),
        model_id,
        model_path: config.model_path.display().to_string(),
        prompt: config.prompt.clone(),
        prompt_mode: config.prompt_mode,
        prompt_token_ids,
        max_output_tokens: config.max_output_tokens,
        repeats: config.repeats,
        load_s,
        runs,
        mean_output_tokens,
        mean_total_s,
        mean_ttft_s,
        mean_decode_tok_s,
        metal_runtime_contract,
    }
}

fn render_human_report(report: &BenchReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("backend: {}\n", report.backend));
    out.push_str(&format!("model: {}\n", report.model_id));
    out.push_str(&format!("path: {}\n", report.model_path));
    out.push_str(&format!("load: {:.3} s\n", report.load_s));
    out.push_str(&format!("prompt: {}\n", report.prompt));
    out.push_str(&format!("prompt_mode: {:?}\n", report.prompt_mode));
    out.push_str(&format!(
        "mean: output_tokens={:.2} total={:.3} s ttft={} tok/s={}\n",
        report.mean_output_tokens,
        report.mean_total_s,
        format_optional_seconds(report.mean_ttft_s),
        format_optional_rate(report.mean_decode_tok_s),
    ));
    for run in &report.runs {
        out.push_str(&format!(
            "run {}: output_tokens={} total={:.3} s ttft={} decode={} tok/s={} termination={}\n",
            run.run_index + 1,
            run.output_tokens,
            run.total_s,
            format_optional_seconds(run.ttft_s),
            format_optional_seconds(run.decode_s),
            format_optional_rate(run.decode_tok_s),
            run.termination,
        ));
    }
    out
}

fn render_termination(reason: TerminationReason) -> String {
    match reason {
        TerminationReason::EndOfSequence => String::from("end_of_sequence"),
        TerminationReason::MaxOutputTokens => String::from("max_output_tokens"),
        TerminationReason::ContextLimit => String::from("context_limit"),
        TerminationReason::Cancelled => String::from("cancelled"),
        TerminationReason::Disconnected => String::from("disconnected"),
        TerminationReason::Error => String::from("error"),
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().copied().sum::<f64>() / values.len() as f64
}

fn mean_option(values: &[Option<f64>]) -> Option<f64> {
    let collected = values.iter().copied().flatten().collect::<Vec<_>>();
    (!collected.is_empty()).then(|| mean(collected.as_slice()))
}

fn nanos_to_seconds(value: u64) -> f64 {
    value as f64 / 1_000_000_000.0
}

fn format_optional_seconds(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.3} s"))
        .unwrap_or_else(|| String::from("n/a"))
}

fn format_optional_rate(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.2} tok/s"))
        .unwrap_or_else(|| String::from("n/a"))
}

fn usage() -> &'static str {
    "Usage: cargo run -p psionic-serve --example qwen35_bench -- \\
  --model-path <path> [--backend cpu|cuda|metal] [--prompt <text>] \\
  [--raw-prompt] [--max-output-tokens <n>] [--repeats <n>] [--json] [--json-out <path>]"
}
