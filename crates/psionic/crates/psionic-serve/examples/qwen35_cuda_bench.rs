use std::{
    collections::BTreeSet,
    env, fs,
    io::Read,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, ExitCode, Stdio},
    time::{Duration, Instant},
};

use psionic_backend_cuda::{CudaAllocatorPoolTelemetry, CudaGemmTuningReport};
use psionic_models::{
    GgufDecoderAdapterLoader, GgufRuntimeTokenizer, PromptMessage, PromptMessageRole,
    PromptRenderOptions, TokenId, TokenSequence, TokenizerBoundary,
};
use psionic_runtime::{
    DEFAULT_PENALTY_LOOKBACK, PrefixCacheControl, PrefixCacheMode, StructuredOutputRequest,
    StructuredOutputValue,
};
use psionic_serve::{
    CudaGgufQwen35TextGenerationService, GenerationOptions, GenerationRequest, GenerationResponse,
    GenerationTerminationCause, Qwen35CudaDecodeOutputMetrics, Qwen35CudaRuntimeContract,
    TerminationReason, TextGenerationExecutor,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    match config.backend {
        BenchBackend::Psionic => run_psionic_benchmark(&config),
        BenchBackend::Ollama => run_ollama_benchmark(&config),
        BenchBackend::LlamaCpp => run_llama_cpp_benchmark(&config),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BenchBackend {
    Psionic,
    Ollama,
    LlamaCpp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BenchDecodeMode {
    Greedy,
    Sample,
}

#[derive(Clone, Debug)]
struct BenchConfig {
    backend: BenchBackend,
    model_path: PathBuf,
    ollama_model: Option<String>,
    ollama_base_url: String,
    llama_server_bin: PathBuf,
    llama_server_port: u16,
    llama_gpu_layers: i32,
    llama_context_size: usize,
    json_out: Option<PathBuf>,
    require_fallback_free_cuda: bool,
    prompt: String,
    raw_prompt: bool,
    prompt_token_ids: Option<Vec<u32>>,
    max_output_tokens: usize,
    repeats: usize,
    decode_mode: BenchDecodeMode,
    temperature: Option<f32>,
    top_k: Option<usize>,
    top_p: Option<f32>,
    min_p: Option<f32>,
    typical_p: Option<f32>,
    mirostat: Option<u8>,
    mirostat_tau: Option<f32>,
    mirostat_eta: Option<f32>,
    repeat_penalty: Option<f32>,
    repeat_last_n: Option<i32>,
    presence_penalty: Option<f32>,
    frequency_penalty: Option<f32>,
    seed: Option<u64>,
    structured_output: Option<BenchStructuredOutput>,
}

#[derive(Clone, Debug)]
enum BenchStructuredOutput {
    JsonObject,
    JsonSchema { name: Option<String>, schema: Value },
}

#[derive(Clone, Debug, Serialize)]
struct BenchReport {
    schema_version: u32,
    report_kind: String,
    run_status: String,
    refusal_reason: Option<String>,
    benchmark_class: String,
    generated_at_unix_s: u64,
    backend: String,
    model_path: String,
    ollama_model: Option<String>,
    ollama_base_url: Option<String>,
    prompt: String,
    prompt_mode: String,
    prompt_token_ids: Option<Vec<u32>>,
    rendered_prompt: String,
    stop_sequences: Vec<String>,
    decode_mode: String,
    max_output_tokens: usize,
    repeats: usize,
    steady_state_concurrency: usize,
    load_s: Option<f64>,
    temperature: Option<f32>,
    top_k: Option<usize>,
    top_p: Option<f32>,
    min_p: Option<f32>,
    typical_p: Option<f32>,
    mirostat: Option<u8>,
    mirostat_tau: Option<f32>,
    mirostat_eta: Option<f32>,
    repeat_penalty: Option<f32>,
    repeat_last_n: Option<i32>,
    presence_penalty: Option<f32>,
    frequency_penalty: Option<f32>,
    seed: Option<u64>,
    structured_output: BenchStructuredOutputConfigReport,
    llama_cpp_server: Option<BenchLlamaCppServerReport>,
    psionic_cuda_startup: Option<BenchPsionicCudaStartupReport>,
    psionic_cuda_fast_path: Option<BenchPsionicCudaFastPathReport>,
    runs: Vec<BenchRunReport>,
    mean_output_tokens: f64,
    mean_prompt_s: f64,
    mean_decode_s: f64,
    mean_total_s: f64,
    mean_ttft_s: Option<f64>,
    mean_itl_s: Option<f64>,
    mean_decode_tok_s: f64,
}

#[derive(Clone, Debug, Serialize)]
struct BenchRunReport {
    run_index: usize,
    decode_mode: String,
    prompt_tokens: usize,
    output_tokens: usize,
    prompt_s: f64,
    decode_s: f64,
    total_s: f64,
    ttft_s: Option<f64>,
    itl_s: Option<f64>,
    decode_tok_s: f64,
    qwen35_output_modes: Vec<String>,
    qwen35_readback_bytes: u64,
    qwen35_raw_logits: bool,
    qwen35_graph_hits: usize,
    qwen35_graph_misses: usize,
    qwen35_graph_captures: usize,
    qwen35_graph_shape_drifts: usize,
    qwen35_graph_cache_identity: Option<String>,
    cuda_allocator_resident_device_bytes_after_run: Option<u64>,
    cuda_allocator_peak_resident_device_bytes_after_run: Option<u64>,
    qwen35_attention_layer_invocations: usize,
    qwen35_attention_backends: Vec<psionic_serve::Qwen35CudaAttentionBackendExecution>,
    qwen35_host_fallback_evidence: BenchCudaHostFallbackEvidenceReport,
    termination: BenchTerminationReport,
    structured_output_mode: String,
    structured_output_parser: String,
    structured_output_kind: String,
    structured_output_value: Option<Value>,
    output_token_ids: Vec<u32>,
    output_text: String,
}

#[derive(Clone, Debug, Serialize)]
struct BenchStructuredOutputConfigReport {
    mode: String,
    schema_name: Option<String>,
    schema: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
struct BenchStructuredOutputRuntimeReport {
    mode: String,
    parser: String,
    kind: String,
    value: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
struct BenchQwen35OutputMetricsReport {
    output_modes: Vec<String>,
    readback_bytes: u64,
    raw_logits: bool,
    graph_hits: usize,
    graph_misses: usize,
    graph_captures: usize,
    graph_shape_drifts: usize,
    graph_cache_identity: Option<String>,
    attention_layer_invocations: usize,
    attention_backends: Vec<psionic_serve::Qwen35CudaAttentionBackendExecution>,
}

#[derive(Clone, Debug, Serialize)]
struct BenchLlamaCppServerReport {
    server_bin: String,
    server_version: String,
    launch_args: Vec<String>,
    base_url: String,
    gpu_layers: i32,
    context_size: usize,
    warmup_output_tokens: usize,
    gpu_memory_used_mib_after_load: Option<u64>,
    gpu_memory_used_mib_after_runs: Option<u64>,
    host_rss_kib_after_load: Option<u64>,
    host_rss_kib_after_runs: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
struct BenchTerminationReport {
    observed: String,
    classification: String,
    matched_stop_sequence: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct BenchPsionicCudaStartupReport {
    load_s: f64,
    runtime_contract: Qwen35CudaRuntimeContract,
    allocator_measurement_scope: String,
    allocator_resident_device_bytes_after_load: u64,
    allocator_peak_resident_device_bytes_after_load: u64,
    allocator_resident_device_bytes_after_warmup: u64,
    allocator_peak_resident_device_bytes_after_warmup: u64,
    allocator_resident_device_bytes_after_measurements: u64,
    allocator_peak_resident_device_bytes_after_measurements: u64,
    cublas_handle_scope: String,
    cublas_stream_binding: String,
    cublas_lt_tuning_status: String,
    cublas_lt_plan_cache_scope: String,
    cublas_lt_selected_plan_count: usize,
    cublas_lt_tuned_shape_count: usize,
    cublas_lt_fallback_shape_count: usize,
    cublas_lt_max_workspace_bytes: u64,
    cublas_lt_selected_plans: Vec<BenchCudaGemmTuningPlanReport>,
    warmup_status: String,
    warmup_prompt_s: f64,
    warmup_decode_s: f64,
    warmup_total_s: f64,
    warmup_output_tokens: usize,
    warmup_host_fallback_evidence: BenchCudaHostFallbackEvidenceReport,
    request_billed_to_user: bool,
}

#[derive(Clone, Debug, Serialize)]
struct BenchPsionicCudaFastPathReport {
    lane: String,
    status: String,
    refusal_reason: Option<String>,
    required_output_modes: Vec<String>,
    raw_logits_forbidden: bool,
    host_fallback_forbidden: bool,
    graph_capture_required: bool,
    env_guards: Vec<BenchEnvGuardReport>,
}

#[derive(Clone, Debug, Serialize)]
struct BenchCudaGemmTuningPlanReport {
    model_family: String,
    op_kind: String,
    rows: usize,
    inner: usize,
    cols: usize,
    input_dtype: String,
    output_dtype: String,
    backend_route: String,
    workspace_bytes: u64,
    mean_time_us: u64,
    algorithm_fingerprint: String,
}

#[derive(Clone, Debug, Default)]
struct BenchCudaGemmTuningStartupFields {
    tuning_status: String,
    plan_cache_scope: String,
    selected_plan_count: usize,
    tuned_shape_count: usize,
    fallback_shape_count: usize,
    max_workspace_bytes: u64,
    selected_plans: Vec<BenchCudaGemmTuningPlanReport>,
}

#[derive(Clone, Debug, Serialize)]
struct BenchEnvGuardReport {
    name: String,
    required_state: String,
    actual_state: String,
}

#[derive(Clone, Debug, Default, Serialize)]
struct BenchCudaHostFallbackEvidenceReport {
    report_count: usize,
    op_count: usize,
    case_count: usize,
    fallback_invocations: u64,
    total_host_fallback_ms: u64,
    op_labels: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct CudaHostFallbackProfileReport {
    #[serde(default)]
    total_host_fallback_ms: u64,
    #[serde(default)]
    ops: Vec<CudaHostFallbackOpReport>,
}

#[derive(Clone, Debug, Deserialize)]
struct CudaHostFallbackOpReport {
    label: String,
    #[serde(default)]
    count: u64,
    #[serde(default)]
    cases: Vec<CudaHostFallbackCaseReport>,
}

#[derive(Clone, Debug, Deserialize)]
struct CudaHostFallbackCaseReport {}

struct ScopedEnvVar {
    key: &'static str,
    previous: Option<String>,
}

impl ScopedEnvVar {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = env::var(key).ok();
        // Safety: this benchmark example mutates one process-local env var before
        // constructing the CUDA service and restores it before returning.
        unsafe {
            env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_deref() {
            // Safety: this restores the process-local override established in `set`.
            unsafe {
                env::set_var(self.key, previous);
            }
        } else {
            // Safety: this clears the process-local override established in `set`.
            unsafe {
                env::remove_var(self.key);
            }
        }
    }
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            backend: BenchBackend::Psionic,
            model_path: PathBuf::new(),
            ollama_model: None,
            ollama_base_url: String::from("http://127.0.0.1:11434"),
            llama_server_bin: default_llama_server_bin(),
            llama_server_port: 0,
            llama_gpu_layers: 99,
            llama_context_size: 4096,
            json_out: None,
            require_fallback_free_cuda: false,
            prompt: String::from("Explain what Psionic is in one sentence."),
            raw_prompt: false,
            prompt_token_ids: None,
            max_output_tokens: 256,
            repeats: 3,
            decode_mode: BenchDecodeMode::Greedy,
            temperature: None,
            top_k: None,
            top_p: None,
            min_p: None,
            typical_p: None,
            mirostat: None,
            mirostat_tau: None,
            mirostat_eta: None,
            repeat_penalty: None,
            repeat_last_n: None,
            presence_penalty: None,
            frequency_penalty: None,
            seed: None,
            structured_output: None,
        }
    }
}

impl BenchConfig {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut config = Self::default();
        let raw_args = args.collect::<Vec<_>>();
        let mut json_object = false;
        let mut json_schema_file: Option<PathBuf> = None;
        let mut json_schema_name: Option<String> = None;
        if raw_args.is_empty() {
            return Err(usage());
        }
        if !raw_args[0].starts_with("--") {
            return Self::parse_legacy(raw_args);
        }
        let mut index = 0;
        while index < raw_args.len() {
            let argument = &raw_args[index];
            match argument.as_str() {
                "--backend" => {
                    config.backend = match next_arg(&raw_args, &mut index, "--backend")?.as_str() {
                        "psionic" => BenchBackend::Psionic,
                        "ollama" => BenchBackend::Ollama,
                        "llama_cpp" => BenchBackend::LlamaCpp,
                        value => {
                            return Err(format!(
                                "invalid --backend `{value}`; expected `psionic`, `ollama`, or `llama_cpp`"
                            ));
                        }
                    };
                }
                "--model-path" => {
                    config.model_path =
                        PathBuf::from(next_arg(&raw_args, &mut index, "--model-path")?);
                }
                "--ollama-model" => {
                    config.ollama_model = Some(next_arg(&raw_args, &mut index, "--ollama-model")?);
                }
                "--ollama-base-url" => {
                    config.ollama_base_url = next_arg(&raw_args, &mut index, "--ollama-base-url")?;
                }
                "--llama-server-bin" => {
                    config.llama_server_bin =
                        PathBuf::from(next_arg(&raw_args, &mut index, "--llama-server-bin")?);
                }
                "--llama-server-port" => {
                    config.llama_server_port = parse_arg(
                        &next_arg(&raw_args, &mut index, "--llama-server-port")?,
                        "--llama-server-port",
                    )?;
                }
                "--llama-gpu-layers" => {
                    config.llama_gpu_layers = parse_arg(
                        &next_arg(&raw_args, &mut index, "--llama-gpu-layers")?,
                        "--llama-gpu-layers",
                    )?;
                }
                "--llama-context-size" => {
                    config.llama_context_size = parse_arg(
                        &next_arg(&raw_args, &mut index, "--llama-context-size")?,
                        "--llama-context-size",
                    )?;
                }
                "--json-out" => {
                    config.json_out = Some(PathBuf::from(next_arg(
                        &raw_args,
                        &mut index,
                        "--json-out",
                    )?));
                }
                "--require-fallback-free-cuda" => {
                    config.require_fallback_free_cuda = true;
                }
                "--prompt" => {
                    config.prompt = next_arg(&raw_args, &mut index, "--prompt")?;
                }
                "--raw-prompt" => {
                    config.raw_prompt = true;
                }
                "--prompt-token-ids" => {
                    config.prompt_token_ids = Some(parse_token_ids(&next_arg(
                        &raw_args,
                        &mut index,
                        "--prompt-token-ids",
                    )?)?);
                }
                "--max-output-tokens" => {
                    config.max_output_tokens = parse_arg(
                        &next_arg(&raw_args, &mut index, "--max-output-tokens")?,
                        "--max-output-tokens",
                    )?;
                }
                "--repeats" => {
                    config.repeats =
                        parse_arg(&next_arg(&raw_args, &mut index, "--repeats")?, "--repeats")?;
                }
                "--decode" => {
                    config.decode_mode = match next_arg(&raw_args, &mut index, "--decode")?.as_str()
                    {
                        "greedy" => BenchDecodeMode::Greedy,
                        "sample" => BenchDecodeMode::Sample,
                        value => {
                            return Err(format!(
                                "invalid --decode `{value}`; expected `greedy` or `sample`"
                            ));
                        }
                    };
                }
                "--temperature" => {
                    config.temperature = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--temperature")?,
                        "--temperature",
                    )?);
                }
                "--top-k" => {
                    config.top_k = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--top-k")?,
                        "--top-k",
                    )?);
                }
                "--top-p" => {
                    config.top_p = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--top-p")?,
                        "--top-p",
                    )?);
                }
                "--min-p" => {
                    config.min_p = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--min-p")?,
                        "--min-p",
                    )?);
                }
                "--typical-p" => {
                    config.typical_p = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--typical-p")?,
                        "--typical-p",
                    )?);
                }
                "--mirostat" => {
                    config.mirostat = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--mirostat")?,
                        "--mirostat",
                    )?);
                }
                "--mirostat-tau" => {
                    config.mirostat_tau = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--mirostat-tau")?,
                        "--mirostat-tau",
                    )?);
                }
                "--mirostat-eta" => {
                    config.mirostat_eta = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--mirostat-eta")?,
                        "--mirostat-eta",
                    )?);
                }
                "--repeat-penalty" => {
                    config.repeat_penalty = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--repeat-penalty")?,
                        "--repeat-penalty",
                    )?);
                }
                "--repeat-last-n" => {
                    config.repeat_last_n = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--repeat-last-n")?,
                        "--repeat-last-n",
                    )?);
                }
                "--presence-penalty" => {
                    config.presence_penalty = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--presence-penalty")?,
                        "--presence-penalty",
                    )?);
                }
                "--frequency-penalty" => {
                    config.frequency_penalty = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--frequency-penalty")?,
                        "--frequency-penalty",
                    )?);
                }
                "--seed" => {
                    config.seed = Some(parse_arg(
                        &next_arg(&raw_args, &mut index, "--seed")?,
                        "--seed",
                    )?);
                }
                "--json-object" => {
                    json_object = true;
                }
                "--json-schema-file" => {
                    json_schema_file = Some(PathBuf::from(next_arg(
                        &raw_args,
                        &mut index,
                        "--json-schema-file",
                    )?));
                }
                "--json-schema-name" => {
                    json_schema_name = Some(next_arg(&raw_args, &mut index, "--json-schema-name")?);
                }
                "--greedy" => {
                    config.decode_mode = BenchDecodeMode::Greedy;
                }
                "--sample" => {
                    config.decode_mode = BenchDecodeMode::Sample;
                }
                "--help" | "-h" => {
                    return Err(usage());
                }
                value => {
                    return Err(format!("unknown argument `{value}`\n\n{}", usage()));
                }
            }
            index += 1;
        }
        config.structured_output =
            parse_structured_output(json_object, json_schema_file, json_schema_name)?;
        config.validate()?;
        Ok(config)
    }

    fn parse_legacy(args: Vec<String>) -> Result<Self, String> {
        let mut config = Self::default();
        config.model_path = PathBuf::from(args.first().cloned().ok_or_else(usage)?);
        if let Some(prompt) = args.get(1) {
            config.prompt = prompt.clone();
        }
        if let Some(max_output_tokens) = args.get(2) {
            config.max_output_tokens = parse_arg(max_output_tokens, "max_output_tokens")?;
        }
        if let Some(repeats) = args.get(3) {
            config.repeats = parse_arg(repeats, "repeats")?;
        }
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        if self.model_path.as_os_str().is_empty() {
            return Err(format!("missing --model-path\n\n{}", usage()));
        }
        if self.repeats == 0 {
            return Err(String::from("--repeats must be at least 1"));
        }
        if matches!(self.backend, BenchBackend::Ollama) && self.ollama_model.is_none() {
            return Err(String::from(
                "missing --ollama-model for `--backend ollama`",
            ));
        }
        if self.require_fallback_free_cuda && !matches!(self.backend, BenchBackend::Psionic) {
            return Err(String::from(
                "`--require-fallback-free-cuda` is only available for `--backend psionic`",
            ));
        }
        if self.prompt_token_ids.is_some() && self.raw_prompt {
            return Err(String::from(
                "`--prompt-token-ids` and `--raw-prompt` are mutually exclusive",
            ));
        }
        if self.prompt_token_ids.is_some() && matches!(self.backend, BenchBackend::Ollama) {
            return Err(String::from(
                "`--prompt-token-ids` is only available for `--backend psionic` or `--backend llama_cpp`",
            ));
        }
        if matches!(self.backend, BenchBackend::LlamaCpp) && !self.llama_server_bin.is_file() {
            return Err(format!(
                "missing llama-server binary: {} (set --llama-server-bin or PSIONIC_LLAMA_SERVER_BIN)",
                self.llama_server_bin.display()
            ));
        }
        Ok(())
    }

    fn effective_temperature(&self) -> Option<f32> {
        match self.decode_mode {
            BenchDecodeMode::Greedy => self.temperature,
            BenchDecodeMode::Sample => Some(self.temperature.unwrap_or(0.8)),
        }
    }

    fn effective_top_k(&self) -> Option<usize> {
        match self.decode_mode {
            BenchDecodeMode::Greedy => self.top_k,
            BenchDecodeMode::Sample => Some(self.top_k.unwrap_or(40)),
        }
    }

    fn effective_top_p(&self) -> Option<f32> {
        match self.decode_mode {
            BenchDecodeMode::Greedy => self.top_p,
            BenchDecodeMode::Sample => Some(self.top_p.unwrap_or(0.9)),
        }
    }

    fn effective_min_p(&self) -> Option<f32> {
        self.min_p.filter(|min_p| min_p.is_finite() && *min_p > 0.0)
    }

    fn effective_typical_p(&self) -> Option<f32> {
        self.typical_p
            .filter(|typical_p| typical_p.is_finite() && *typical_p > 0.0 && *typical_p < 1.0)
    }

    fn effective_mirostat(&self) -> Option<u8> {
        self.mirostat.filter(|value| matches!(value, 1 | 2))
    }

    fn effective_mirostat_tau(&self) -> Option<f32> {
        self.effective_mirostat()
            .map(|_| self.mirostat_tau.unwrap_or(5.0).max(0.0))
    }

    fn effective_mirostat_eta(&self) -> Option<f32> {
        self.effective_mirostat()
            .map(|_| self.mirostat_eta.unwrap_or(0.1).max(0.0))
    }

    fn effective_repeat_penalty(&self) -> Option<f32> {
        match self.decode_mode {
            BenchDecodeMode::Greedy => self.repeat_penalty,
            BenchDecodeMode::Sample => Some(self.repeat_penalty.unwrap_or(1.0)),
        }
    }

    fn effective_repeat_last_n(&self) -> Option<i32> {
        match self.decode_mode {
            BenchDecodeMode::Greedy => self.repeat_last_n,
            BenchDecodeMode::Sample => Some(
                self.repeat_last_n
                    .unwrap_or(DEFAULT_PENALTY_LOOKBACK as i32),
            ),
        }
    }

    fn effective_presence_penalty(&self) -> Option<f32> {
        match self.decode_mode {
            BenchDecodeMode::Greedy => self.presence_penalty,
            BenchDecodeMode::Sample => Some(self.presence_penalty.unwrap_or(0.0)),
        }
    }

    fn effective_frequency_penalty(&self) -> Option<f32> {
        match self.decode_mode {
            BenchDecodeMode::Greedy => self.frequency_penalty,
            BenchDecodeMode::Sample => Some(self.frequency_penalty.unwrap_or(0.0)),
        }
    }

    fn effective_seed(&self) -> Option<u64> {
        match self.decode_mode {
            BenchDecodeMode::Greedy => self.seed,
            BenchDecodeMode::Sample => Some(self.seed.unwrap_or(42)),
        }
    }

    fn effective_temperature_for_backend(&self, backend: BenchBackend) -> Option<f32> {
        match (backend, self.decode_mode) {
            (BenchBackend::Ollama | BenchBackend::LlamaCpp, BenchDecodeMode::Greedy) => Some(0.0),
            _ => self.effective_temperature(),
        }
    }

    fn effective_top_k_for_backend(&self, backend: BenchBackend) -> Option<usize> {
        match (backend, self.decode_mode) {
            (BenchBackend::Ollama | BenchBackend::LlamaCpp, BenchDecodeMode::Greedy) => Some(1),
            _ => self.effective_top_k(),
        }
    }

    fn effective_top_p_for_backend(&self, backend: BenchBackend) -> Option<f32> {
        match (backend, self.decode_mode) {
            (BenchBackend::Ollama | BenchBackend::LlamaCpp, BenchDecodeMode::Greedy) => Some(1.0),
            _ => self.effective_top_p(),
        }
    }

    fn effective_min_p_for_backend(&self, backend: BenchBackend) -> Option<f32> {
        match (backend, self.decode_mode) {
            (BenchBackend::Ollama | BenchBackend::LlamaCpp, BenchDecodeMode::Greedy) => Some(0.0),
            _ => self.effective_min_p(),
        }
    }

    fn effective_repeat_penalty_for_backend(&self, backend: BenchBackend) -> Option<f32> {
        match (backend, self.decode_mode) {
            (BenchBackend::Ollama | BenchBackend::LlamaCpp, BenchDecodeMode::Greedy) => Some(1.0),
            _ => self.effective_repeat_penalty(),
        }
    }

    fn effective_repeat_last_n_for_backend(&self, backend: BenchBackend) -> Option<i32> {
        match (backend, self.decode_mode) {
            (BenchBackend::Ollama | BenchBackend::LlamaCpp, BenchDecodeMode::Greedy) => Some(0),
            _ => self.effective_repeat_last_n(),
        }
    }

    fn effective_presence_penalty_for_backend(&self, backend: BenchBackend) -> Option<f32> {
        match (backend, self.decode_mode) {
            (BenchBackend::Ollama | BenchBackend::LlamaCpp, BenchDecodeMode::Greedy) => Some(0.0),
            _ => self.effective_presence_penalty(),
        }
    }

    fn effective_frequency_penalty_for_backend(&self, backend: BenchBackend) -> Option<f32> {
        match (backend, self.decode_mode) {
            (BenchBackend::Ollama | BenchBackend::LlamaCpp, BenchDecodeMode::Greedy) => Some(0.0),
            _ => self.effective_frequency_penalty(),
        }
    }

    fn effective_seed_for_backend(&self, backend: BenchBackend) -> Option<u64> {
        match (backend, self.decode_mode) {
            (BenchBackend::Ollama | BenchBackend::LlamaCpp, BenchDecodeMode::Greedy) => {
                Some(self.seed.unwrap_or(42))
            }
            _ => self.effective_seed(),
        }
    }

    fn ollama_format_payload(&self) -> Option<Value> {
        match self.structured_output.as_ref() {
            Some(BenchStructuredOutput::JsonObject) => Some(Value::String(String::from("json"))),
            Some(BenchStructuredOutput::JsonSchema { schema, .. }) => Some(schema.clone()),
            None => None,
        }
    }
}

fn run_psionic_benchmark(config: &BenchConfig) -> Result<(), String> {
    let bench_model = load_bench_model(
        &config.model_path,
        &config.prompt,
        config.raw_prompt || config.prompt_token_ids.is_some(),
    )?;
    let mut fast_path_report = psionic_cuda_fast_path_report(config);
    if let Err(reason) = validate_psionic_cuda_fast_path_contract(config, &mut fast_path_report) {
        let report = build_bench_report(
            config,
            &bench_model.rendered,
            None,
            None,
            Some(fast_path_report),
            None,
            String::from("refused"),
            Some(reason.clone()),
            Vec::new(),
        );
        write_json_output(&report, config.json_out.as_ref())?;
        return Err(reason);
    }
    let fallback_profile_path = fallback_profile_capture_path();
    let _fallback_profile_env = ScopedEnvVar::set(
        "PSIONIC_CUDA_HOST_FALLBACK_PROFILE_PATH",
        &fallback_profile_path.display().to_string(),
    );
    let load_started_at = Instant::now();
    let mut service = CudaGgufQwen35TextGenerationService::from_gguf_path(&config.model_path)
        .map_err(|error| format!("failed to load qwen35 cuda service: {error}"))?;
    let load_s = load_started_at.elapsed().as_secs_f64();
    let runtime_contract = service.cuda_runtime_contract();
    let allocator_after_load = required_cuda_allocator_telemetry(&service)?;
    let descriptor = service.model_descriptor().clone();
    let cublas_lt_tuning = cuda_gemm_tuning_startup_fields(service.cuda_gemm_tuning_report());
    let prefix_cache_bypass = PrefixCacheControl {
        mode: PrefixCacheMode::Bypass,
        ..PrefixCacheControl::default()
    };

    let warmup = psionic_generation_request(
        config,
        String::from("warmup"),
        descriptor.clone(),
        &bench_model.rendered,
        min_warmup_tokens(config.max_output_tokens),
    )
    .with_prefix_cache_control(prefix_cache_bypass.clone());
    let warmup_response = service
        .generate(&warmup)
        .map_err(|error| format!("warmup generation failed: {error}"))?;
    let mut fallback_record_count = 0;
    let warmup_fallback_evidence = read_host_fallback_evidence_delta(
        fallback_profile_path.as_path(),
        &mut fallback_record_count,
    )?;
    let warmup_output_metrics =
        qwen35_output_metrics_report(warmup_response.metrics.qwen35_cuda_decode.as_ref());
    let allocator_after_warmup = required_cuda_allocator_telemetry(&service)?;
    let mut startup_report = BenchPsionicCudaStartupReport {
        load_s,
        runtime_contract,
        allocator_measurement_scope: String::from(
            "psionic_owned_cuda_malloc_buffers_including_allocator_pool_cache",
        ),
        allocator_resident_device_bytes_after_load: allocator_after_load.resident_device_bytes,
        allocator_peak_resident_device_bytes_after_load: allocator_after_load
            .peak_resident_device_bytes,
        allocator_resident_device_bytes_after_warmup: allocator_after_warmup.resident_device_bytes,
        allocator_peak_resident_device_bytes_after_warmup: allocator_after_warmup
            .peak_resident_device_bytes,
        allocator_resident_device_bytes_after_measurements: allocator_after_warmup
            .resident_device_bytes,
        allocator_peak_resident_device_bytes_after_measurements: allocator_after_warmup
            .peak_resident_device_bytes,
        cublas_handle_scope: String::from("per_device_runtime_owner"),
        cublas_stream_binding: String::from("bind_stream_per_submission"),
        cublas_lt_tuning_status: cublas_lt_tuning.tuning_status,
        cublas_lt_plan_cache_scope: cublas_lt_tuning.plan_cache_scope,
        cublas_lt_selected_plan_count: cublas_lt_tuning.selected_plan_count,
        cublas_lt_tuned_shape_count: cublas_lt_tuning.tuned_shape_count,
        cublas_lt_fallback_shape_count: cublas_lt_tuning.fallback_shape_count,
        cublas_lt_max_workspace_bytes: cublas_lt_tuning.max_workspace_bytes,
        cublas_lt_selected_plans: cublas_lt_tuning.selected_plans,
        warmup_status: String::from("explicit_warmup_completed"),
        warmup_prompt_s: nanos_to_seconds(
            warmup_response.metrics.prompt_eval_duration_ns.unwrap_or(0),
        ),
        warmup_decode_s: nanos_to_seconds(warmup_response.metrics.eval_duration_ns.unwrap_or(0)),
        warmup_total_s: nanos_to_seconds(warmup_response.metrics.total_duration_ns.unwrap_or(0)),
        warmup_output_tokens: warmup_response
            .metrics
            .eval_count
            .unwrap_or(warmup_response.output.tokens.len()),
        warmup_host_fallback_evidence: warmup_fallback_evidence.clone(),
        request_billed_to_user: false,
    };
    if let Err(reason) = validate_psionic_cuda_fast_path_warmup(
        config,
        &warmup_output_metrics,
        &warmup_fallback_evidence,
        &mut fast_path_report,
    ) {
        let report = build_bench_report(
            config,
            &bench_model.rendered,
            Some(startup_report),
            Some(load_s),
            Some(fast_path_report),
            None,
            String::from("refused"),
            Some(reason.clone()),
            Vec::new(),
        );
        write_json_output(&report, config.json_out.as_ref())?;
        return Err(reason);
    }
    println!(
        "backend=psionic load_s={:.6} startup_warmup_status={} cublas_handle_scope={} cublas_stream_binding={} cublaslt_tuning_status={} cublaslt_plan_cache_scope={} cublaslt_selected_plan_count={} cublaslt_fallback_shape_count={} cublaslt_max_workspace_bytes={} warmup_prompt_s={:.6} warmup_decode_s={:.6} warmup_total_s={:.6} warmup_output_tokens={} warmup_host_fallback_ops={} warmup_host_fallback_total_ms={}",
        startup_report.load_s,
        startup_report.warmup_status,
        startup_report.cublas_handle_scope,
        startup_report.cublas_stream_binding,
        startup_report.cublas_lt_tuning_status,
        startup_report.cublas_lt_plan_cache_scope,
        startup_report.cublas_lt_selected_plan_count,
        startup_report.cublas_lt_fallback_shape_count,
        startup_report.cublas_lt_max_workspace_bytes,
        startup_report.warmup_prompt_s,
        startup_report.warmup_decode_s,
        startup_report.warmup_total_s,
        startup_report.warmup_output_tokens,
        startup_report.warmup_host_fallback_evidence.op_count,
        startup_report
            .warmup_host_fallback_evidence
            .total_host_fallback_ms,
    );

    let mut runs = Vec::with_capacity(config.repeats);
    for run_index in 0..config.repeats {
        let request = psionic_generation_request(
            config,
            format!("bench-{run_index}"),
            descriptor.clone(),
            &bench_model.rendered,
            config.max_output_tokens,
        )
        .with_prefix_cache_control(prefix_cache_bypass.clone());
        let response = service
            .generate(&request)
            .map_err(|error| format!("benchmark generation failed: {error}"))?;
        let output_tokens = response
            .metrics
            .eval_count
            .unwrap_or(response.output.tokens.len());
        let decode_ns = response.metrics.eval_duration_ns.unwrap_or(0);
        let prompt_ns = response.metrics.prompt_eval_duration_ns.unwrap_or(0);
        let total_ns = response.metrics.total_duration_ns.unwrap_or(0);
        let decode_tok_s = tokens_per_second(output_tokens, decode_ns);
        let output_metrics =
            qwen35_output_metrics_report(response.metrics.qwen35_cuda_decode.as_ref());
        let fallback_evidence = read_host_fallback_evidence_delta(
            fallback_profile_path.as_path(),
            &mut fallback_record_count,
        )?;
        let allocator_after_run = required_cuda_allocator_telemetry(&service)?;
        startup_report.allocator_resident_device_bytes_after_measurements =
            allocator_after_run.resident_device_bytes;
        startup_report.allocator_peak_resident_device_bytes_after_measurements =
            allocator_after_run.peak_resident_device_bytes;
        let structured_output = structured_output_runtime_report(
            response.provenance.as_ref(),
            response.output.structured.as_ref(),
        );
        let termination = psionic_termination_report(
            &response,
            &bench_model.tokenizer,
            &bench_model.rendered.stop_sequences,
        );
        let output_token_ids = token_ids(response.output.tokens.as_slice());
        let prompt_s = nanos_to_seconds(prompt_ns);
        let decode_s = nanos_to_seconds(decode_ns);
        let total_s = nanos_to_seconds(total_ns);
        let ttft_s = response
            .metrics
            .time_to_first_token_ns
            .map(nanos_to_seconds);
        let itl_s = response
            .metrics
            .inter_token_latency_ns
            .map(nanos_to_seconds);
        let output_text = response.output.text;
        let printable_output_text = output_text.replace('\n', "\\n");
        let run_report = BenchRunReport {
            run_index: run_index + 1,
            decode_mode: String::from(bench_decode_mode_label(config.decode_mode)),
            prompt_tokens: response.metrics.prompt_eval_count.unwrap_or(0),
            output_tokens,
            prompt_s,
            decode_s,
            total_s,
            ttft_s,
            itl_s,
            decode_tok_s,
            qwen35_output_modes: output_metrics.output_modes.clone(),
            qwen35_readback_bytes: output_metrics.readback_bytes,
            qwen35_raw_logits: output_metrics.raw_logits,
            qwen35_graph_hits: output_metrics.graph_hits,
            qwen35_graph_misses: output_metrics.graph_misses,
            qwen35_graph_captures: output_metrics.graph_captures,
            qwen35_graph_shape_drifts: output_metrics.graph_shape_drifts,
            qwen35_graph_cache_identity: output_metrics.graph_cache_identity.clone(),
            cuda_allocator_resident_device_bytes_after_run: Some(
                allocator_after_run.resident_device_bytes,
            ),
            cuda_allocator_peak_resident_device_bytes_after_run: Some(
                allocator_after_run.peak_resident_device_bytes,
            ),
            qwen35_attention_layer_invocations: output_metrics.attention_layer_invocations,
            qwen35_attention_backends: output_metrics.attention_backends.clone(),
            qwen35_host_fallback_evidence: fallback_evidence.clone(),
            termination: termination.clone(),
            structured_output_mode: structured_output.mode.clone(),
            structured_output_parser: structured_output.parser.clone(),
            structured_output_kind: structured_output.kind.clone(),
            structured_output_value: structured_output.value.clone(),
            output_token_ids,
            output_text: output_text.clone(),
        };
        if let Err(reason) = validate_psionic_cuda_fast_path_run(
            config,
            &output_metrics,
            &fallback_evidence,
            &mut fast_path_report,
        ) {
            runs.push(run_report);
            let report = build_bench_report(
                config,
                &bench_model.rendered,
                Some(startup_report.clone()),
                Some(load_s),
                Some(fast_path_report),
                None,
                String::from("refused"),
                Some(reason.clone()),
                runs,
            );
            write_json_output(&report, config.json_out.as_ref())?;
            return Err(reason);
        }
        runs.push(run_report);
        println!(
            "backend=psionic run={} decode_mode={} prompt_tokens={} output_tokens={} prompt_s={:.6} decode_s={:.6} total_s={:.6} ttft_s={} itl_s={} decode_tok_s={:.2} termination_observed={} termination_classification={} matched_stop_sequence={} qwen35_host_fallback_ops={} qwen35_host_fallback_total_ms={} {} {} output={}",
            run_index + 1,
            bench_decode_mode_label(config.decode_mode),
            response.metrics.prompt_eval_count.unwrap_or(0),
            output_tokens,
            prompt_s,
            decode_s,
            total_s,
            format_optional_seconds(ttft_s),
            format_optional_seconds(itl_s),
            decode_tok_s,
            termination.observed,
            termination.classification,
            termination
                .matched_stop_sequence
                .as_deref()
                .unwrap_or("none"),
            fallback_evidence.op_count,
            fallback_evidence.total_host_fallback_ms,
            format_qwen35_output_metrics(&output_metrics),
            format_structured_output_report(&structured_output),
            printable_output_text,
        );
    }

    let report = build_bench_report(
        config,
        &bench_model.rendered,
        Some(startup_report.clone()),
        Some(startup_report.load_s),
        Some(fast_path_report),
        None,
        String::from("ok"),
        None,
        runs,
    );
    println!(
        "backend=psionic mean_decode_tok_s={:.2}",
        report.mean_decode_tok_s
    );
    write_json_output(&report, config.json_out.as_ref())?;
    Ok(())
}

fn run_ollama_benchmark(config: &BenchConfig) -> Result<(), String> {
    let bench_model = load_bench_model(&config.model_path, &config.prompt, config.raw_prompt)?;
    let client = Client::builder()
        .build()
        .map_err(|error| format!("failed to build Ollama HTTP client: {error}"))?;
    let ollama_model = config
        .ollama_model
        .as_ref()
        .ok_or_else(|| String::from("missing Ollama model alias"))?;

    let _ = ollama_generate(
        &client,
        &config.ollama_base_url,
        ollama_model,
        &bench_model.rendered,
        config,
        min_warmup_tokens(config.max_output_tokens),
    )?;

    let mut runs = Vec::with_capacity(config.repeats);
    for run_index in 0..config.repeats {
        let response = ollama_generate(
            &client,
            &config.ollama_base_url,
            ollama_model,
            &bench_model.rendered,
            config,
            config.max_output_tokens,
        )?;
        let output_tokens = response.eval_count.unwrap_or(0);
        let decode_ns = response.eval_duration.unwrap_or(0);
        let prompt_ns = response.prompt_eval_duration.unwrap_or(0);
        let total_ns = response.total_duration.unwrap_or(0);
        let decode_tok_s = tokens_per_second(output_tokens, decode_ns);
        let prompt_s = nanos_to_seconds(prompt_ns);
        let decode_s = nanos_to_seconds(decode_ns);
        let total_s = nanos_to_seconds(total_ns);
        let termination = ollama_termination_report(
            &response,
            &bench_model.rendered.stop_sequences,
            config.max_output_tokens,
        );
        let output_token_ids = token_ids(
            bench_model
                .tokenizer
                .encode(response.response.as_str())
                .as_slice(),
        );
        let output_text = response.response;
        let printable_output_text = output_text.replace('\n', "\\n");
        runs.push(BenchRunReport {
            run_index: run_index + 1,
            decode_mode: String::from(bench_decode_mode_label(config.decode_mode)),
            prompt_tokens: response.prompt_eval_count.unwrap_or(0),
            output_tokens,
            prompt_s,
            decode_s,
            total_s,
            ttft_s: None,
            itl_s: None,
            decode_tok_s,
            qwen35_output_modes: Vec::new(),
            qwen35_readback_bytes: 0,
            qwen35_raw_logits: false,
            qwen35_graph_hits: 0,
            qwen35_graph_misses: 0,
            qwen35_graph_captures: 0,
            qwen35_graph_shape_drifts: 0,
            qwen35_graph_cache_identity: None,
            cuda_allocator_resident_device_bytes_after_run: None,
            cuda_allocator_peak_resident_device_bytes_after_run: None,
            qwen35_attention_layer_invocations: 0,
            qwen35_attention_backends: Vec::new(),
            qwen35_host_fallback_evidence: BenchCudaHostFallbackEvidenceReport::default(),
            termination: termination.clone(),
            structured_output_mode: String::from("none"),
            structured_output_parser: String::from("none"),
            structured_output_kind: String::from("none"),
            structured_output_value: None,
            output_token_ids,
            output_text: output_text.clone(),
        });
        println!(
            "backend=ollama run={} decode_mode={} prompt_tokens={} output_tokens={} prompt_s={:.6} decode_s={:.6} total_s={:.6} decode_tok_s={:.2} termination_observed={} termination_classification={} matched_stop_sequence={} output={}",
            run_index + 1,
            bench_decode_mode_label(config.decode_mode),
            response.prompt_eval_count.unwrap_or(0),
            output_tokens,
            prompt_s,
            decode_s,
            total_s,
            decode_tok_s,
            termination.observed,
            termination.classification,
            termination
                .matched_stop_sequence
                .as_deref()
                .unwrap_or("none"),
            printable_output_text,
        );
    }

    let report = build_bench_report(
        config,
        &bench_model.rendered,
        None,
        None,
        None,
        None,
        String::from("ok"),
        None,
        runs,
    );
    println!(
        "backend=ollama mean_decode_tok_s={:.2}",
        report.mean_decode_tok_s
    );
    write_json_output(&report, config.json_out.as_ref())?;
    Ok(())
}

fn default_llama_server_bin() -> PathBuf {
    env::var("PSIONIC_LLAMA_SERVER_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if cfg!(target_os = "macos") {
                PathBuf::from("/Users/christopherdavid/code/llama.cpp/build/bin/llama-server")
            } else {
                PathBuf::from("/home/christopherdavid/code/llama.cpp/build/bin/llama-server")
            }
        })
}

struct LlamaCppServerGuard {
    child: Child,
}

impl Drop for LlamaCppServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct LlamaCppTimingsReport {
    #[serde(default)]
    prompt_n: usize,
    #[serde(default)]
    prompt_ms: f64,
    #[serde(default)]
    predicted_n: usize,
    #[serde(default)]
    predicted_ms: f64,
    #[serde(default)]
    predicted_per_second: f64,
    #[serde(default)]
    predicted_per_token_ms: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct LlamaCppCompletionResponse {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tokens: Vec<u32>,
    #[serde(default)]
    stop_type: Option<String>,
    #[serde(default)]
    stopping_word: Option<String>,
    #[serde(default)]
    tokens_predicted: usize,
    #[serde(default)]
    tokens_cached: usize,
    #[serde(default)]
    truncated: bool,
    #[serde(default)]
    timings: Option<LlamaCppTimingsReport>,
    #[serde(default)]
    error: Option<Value>,
}

fn run_llama_cpp_benchmark(config: &BenchConfig) -> Result<(), String> {
    // Token-id prompts skip chat-template rendering, matching the psionic
    // lane; a pure token array also keeps llama-server from inserting BOS.
    let (rendered, prompt_token_ids) = match config.prompt_token_ids.as_ref() {
        Some(token_ids) => (
            RenderedPrompt {
                text: String::new(),
                stop_sequences: Vec::new(),
            },
            token_ids.clone(),
        ),
        None => {
            let bench_model =
                load_bench_model(&config.model_path, &config.prompt, config.raw_prompt)?;
            let ids = token_ids(
                bench_model
                    .tokenizer
                    .encode(bench_model.rendered.text.as_str())
                    .as_slice(),
            );
            (bench_model.rendered, ids)
        }
    };

    let version_output = Command::new(&config.llama_server_bin)
        .arg("--version")
        .output()
        .map_err(|error| format!("failed to run llama-server --version: {error}"))?;
    let mut version_text = String::from_utf8_lossy(&version_output.stdout).into_owned();
    if version_text.trim().is_empty() {
        version_text = String::from_utf8_lossy(&version_output.stderr).into_owned();
    }
    let server_version = version_text.lines().next().unwrap_or_default().to_string();

    let port = if config.llama_server_port != 0 {
        config.llama_server_port
    } else {
        TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("failed to reserve a llama-server port: {error}"))?
            .local_addr()
            .map_err(|error| format!("failed to read the reserved port: {error}"))?
            .port()
    };
    let base_url = format!("http://127.0.0.1:{port}");
    let launch_args = vec![
        String::from("-m"),
        config.model_path.display().to_string(),
        String::from("--host"),
        String::from("127.0.0.1"),
        String::from("--port"),
        port.to_string(),
        String::from("-ngl"),
        config.llama_gpu_layers.to_string(),
        String::from("-c"),
        config.llama_context_size.to_string(),
        String::from("--no-webui"),
        String::from("--metrics"),
    ];
    let server_child = Command::new(&config.llama_server_bin)
        .args(&launch_args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("failed to spawn llama-server: {error}"))?;
    let server_pid = server_child.id();
    let mut server = LlamaCppServerGuard {
        child: server_child,
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|error| format!("failed to build llama.cpp HTTP client: {error}"))?;

    let load_started = Instant::now();
    loop {
        if let Some(status) = server
            .child
            .try_wait()
            .map_err(|error| format!("failed to poll llama-server status: {error}"))?
        {
            return Err(format!("llama-server exited during startup with {status}"));
        }
        let ready = client
            .get(format!("{base_url}/health"))
            .send()
            .map(|response| response.status().is_success())
            .unwrap_or(false);
        if ready {
            break;
        }
        if load_started.elapsed() > Duration::from_secs(600) {
            return Err(String::from(
                "llama-server did not become healthy within 600s",
            ));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let load_s = load_started.elapsed().as_secs_f64();
    let gpu_memory_after_load = nvidia_smi_memory_used_mib();
    let host_rss_after_load = process_rss_kib(server_pid);

    let warmup_tokens = min_warmup_tokens(config.max_output_tokens);
    let _ = llama_cpp_completion(
        &client,
        &base_url,
        &prompt_token_ids,
        warmup_tokens,
        config,
        &rendered,
    )?;

    let mut runs = Vec::with_capacity(config.repeats);
    for run_index in 0..config.repeats {
        let started = Instant::now();
        let response = llama_cpp_completion(
            &client,
            &base_url,
            &prompt_token_ids,
            config.max_output_tokens,
            config,
            &rendered,
        )?;
        let wall_s = started.elapsed().as_secs_f64();
        if response.truncated || response.tokens_cached > 0 {
            eprintln!(
                "backend=llama_cpp run={} warning: truncated={} tokens_cached={}",
                run_index + 1,
                response.truncated,
                response.tokens_cached,
            );
        }
        let timings = response.timings.clone().unwrap_or_default();
        let output_tokens = if response.tokens.is_empty() {
            timings.predicted_n.max(response.tokens_predicted)
        } else {
            response.tokens.len()
        };
        let prompt_s = timings.prompt_ms / 1000.0;
        let decode_s = timings.predicted_ms / 1000.0;
        let total_s = if prompt_s + decode_s > 0.0 {
            prompt_s + decode_s
        } else {
            wall_s
        };
        let decode_tok_s = if timings.predicted_per_second > 0.0 {
            timings.predicted_per_second
        } else {
            tokens_per_second(output_tokens, (decode_s * 1e9) as u64)
        };
        let itl_s =
            (timings.predicted_per_token_ms > 0.0).then(|| timings.predicted_per_token_ms / 1000.0);
        let termination = llama_cpp_termination_report(
            &response,
            &rendered.stop_sequences,
            config.max_output_tokens,
        );
        let output_text = response.content.clone();
        let printable_output_text = output_text.replace('\n', "\\n");
        runs.push(BenchRunReport {
            run_index: run_index + 1,
            decode_mode: String::from(bench_decode_mode_label(config.decode_mode)),
            prompt_tokens: timings.prompt_n.max(prompt_token_ids.len()),
            output_tokens,
            prompt_s,
            decode_s,
            total_s,
            ttft_s: None,
            itl_s,
            decode_tok_s,
            qwen35_output_modes: Vec::new(),
            qwen35_readback_bytes: 0,
            qwen35_raw_logits: false,
            qwen35_graph_hits: 0,
            qwen35_graph_misses: 0,
            qwen35_graph_captures: 0,
            qwen35_graph_shape_drifts: 0,
            qwen35_graph_cache_identity: None,
            cuda_allocator_resident_device_bytes_after_run: None,
            cuda_allocator_peak_resident_device_bytes_after_run: None,
            qwen35_attention_layer_invocations: 0,
            qwen35_attention_backends: Vec::new(),
            qwen35_host_fallback_evidence: BenchCudaHostFallbackEvidenceReport::default(),
            termination: termination.clone(),
            structured_output_mode: String::from("none"),
            structured_output_parser: String::from("none"),
            structured_output_kind: String::from("none"),
            structured_output_value: None,
            output_token_ids: response.tokens.clone(),
            output_text,
        });
        println!(
            "backend=llama_cpp run={} decode_mode={} prompt_tokens={} output_tokens={} prompt_s={:.6} decode_s={:.6} total_s={:.6} decode_tok_s={:.2} termination_observed={} termination_classification={} matched_stop_sequence={} output={}",
            run_index + 1,
            bench_decode_mode_label(config.decode_mode),
            timings.prompt_n.max(prompt_token_ids.len()),
            output_tokens,
            prompt_s,
            decode_s,
            total_s,
            decode_tok_s,
            termination.observed,
            termination.classification,
            termination
                .matched_stop_sequence
                .as_deref()
                .unwrap_or("none"),
            printable_output_text,
        );
    }

    let server_report = BenchLlamaCppServerReport {
        server_bin: config.llama_server_bin.display().to_string(),
        server_version,
        launch_args,
        base_url,
        gpu_layers: config.llama_gpu_layers,
        context_size: config.llama_context_size,
        warmup_output_tokens: warmup_tokens,
        gpu_memory_used_mib_after_load: gpu_memory_after_load,
        gpu_memory_used_mib_after_runs: nvidia_smi_memory_used_mib(),
        host_rss_kib_after_load: host_rss_after_load,
        host_rss_kib_after_runs: process_rss_kib(server_pid),
    };
    let report = build_bench_report(
        config,
        &rendered,
        None,
        Some(load_s),
        None,
        Some(server_report),
        String::from("ok"),
        None,
        runs,
    );
    println!(
        "backend=llama_cpp mean_decode_tok_s={:.2}",
        report.mean_decode_tok_s
    );
    write_json_output(&report, config.json_out.as_ref())?;
    Ok(())
}

fn llama_cpp_completion(
    client: &Client,
    base_url: &str,
    prompt_token_ids: &[u32],
    max_output_tokens: usize,
    config: &BenchConfig,
    rendered: &RenderedPrompt,
) -> Result<LlamaCppCompletionResponse, String> {
    let mut payload = serde_json::json!({
        "prompt": prompt_token_ids,
        "n_predict": max_output_tokens,
        "cache_prompt": false,
        "return_tokens": true,
        "stream": false,
    });
    if let Some(temperature) = config.effective_temperature_for_backend(BenchBackend::LlamaCpp) {
        payload["temperature"] = serde_json::json!(temperature);
    }
    if let Some(top_k) = config.effective_top_k_for_backend(BenchBackend::LlamaCpp) {
        payload["top_k"] = serde_json::json!(top_k);
    }
    if let Some(top_p) = config.effective_top_p_for_backend(BenchBackend::LlamaCpp) {
        payload["top_p"] = serde_json::json!(top_p);
    }
    if let Some(min_p) = config.effective_min_p_for_backend(BenchBackend::LlamaCpp) {
        payload["min_p"] = serde_json::json!(min_p);
    }
    if let Some(typical_p) = config.effective_typical_p() {
        payload["typical_p"] = serde_json::json!(typical_p);
    }
    if let Some(mirostat) = config.effective_mirostat() {
        payload["mirostat"] = serde_json::json!(mirostat);
    }
    if let Some(mirostat_tau) = config.effective_mirostat_tau() {
        payload["mirostat_tau"] = serde_json::json!(mirostat_tau);
    }
    if let Some(mirostat_eta) = config.effective_mirostat_eta() {
        payload["mirostat_eta"] = serde_json::json!(mirostat_eta);
    }
    if let Some(repeat_penalty) =
        config.effective_repeat_penalty_for_backend(BenchBackend::LlamaCpp)
    {
        payload["repeat_penalty"] = serde_json::json!(repeat_penalty);
    }
    if let Some(repeat_last_n) = config.effective_repeat_last_n_for_backend(BenchBackend::LlamaCpp)
    {
        payload["repeat_last_n"] = serde_json::json!(repeat_last_n);
    }
    if let Some(presence_penalty) =
        config.effective_presence_penalty_for_backend(BenchBackend::LlamaCpp)
    {
        payload["presence_penalty"] = serde_json::json!(presence_penalty);
    }
    if let Some(frequency_penalty) =
        config.effective_frequency_penalty_for_backend(BenchBackend::LlamaCpp)
    {
        payload["frequency_penalty"] = serde_json::json!(frequency_penalty);
    }
    if let Some(seed) = config.effective_seed_for_backend(BenchBackend::LlamaCpp) {
        payload["seed"] = serde_json::json!(seed);
    }
    if !rendered.stop_sequences.is_empty() {
        payload["stop"] = serde_json::json!(rendered.stop_sequences);
    }
    match config.structured_output.as_ref() {
        Some(BenchStructuredOutput::JsonObject) => {
            payload["json_schema"] = serde_json::json!({"type": "object"});
        }
        Some(BenchStructuredOutput::JsonSchema { schema, .. }) => {
            payload["json_schema"] = schema.clone();
        }
        None => {}
    }
    let url = format!("{}/completion", base_url.trim_end_matches('/'));
    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .map_err(|error| format!("failed to call llama-server completion endpoint: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| format!("failed to read llama-server response body: {error}"))?;
    if !status.is_success() {
        return Err(format!(
            "llama-server completion failed with status {status}: {body}"
        ));
    }
    let parsed: LlamaCppCompletionResponse = serde_json::from_str(&body)
        .map_err(|error| format!("failed to parse llama-server response: {error}"))?;
    if let Some(error) = parsed.error.as_ref() {
        return Err(format!(
            "llama-server completion returned an error: {error}"
        ));
    }
    Ok(parsed)
}

fn llama_cpp_termination_report(
    response: &LlamaCppCompletionResponse,
    stop_sequences: &[String],
    max_output_tokens: usize,
) -> BenchTerminationReport {
    let observed = response
        .stop_type
        .clone()
        .unwrap_or_else(|| String::from("unknown"));
    let classification = match response.stop_type.as_deref() {
        Some("eos") => String::from("eos_token"),
        Some("limit") => String::from("max_output_tokens"),
        Some("word") => String::from("stop_sequence"),
        Some(other) => other.replace('-', "_"),
        None if response.tokens_predicted >= max_output_tokens => String::from("max_output_tokens"),
        None => String::from("unknown"),
    };
    let matched_stop_sequence = response
        .stopping_word
        .clone()
        .filter(|word| !word.is_empty() && stop_sequences.iter().any(|stop| stop == word));
    BenchTerminationReport {
        observed,
        classification,
        matched_stop_sequence,
    }
}

fn nvidia_smi_memory_used_mib() -> Option<u64> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

fn process_rss_kib(pid: u32) -> Option<u64> {
    let mut status = String::new();
    fs::File::open(format!("/proc/{pid}/status"))
        .ok()?
        .read_to_string(&mut status)
        .ok()?;
    status.lines().find_map(|line| {
        line.strip_prefix("VmRSS:")
            .and_then(|value| value.trim().strip_suffix(" kB"))
            .and_then(|value| value.trim().parse().ok())
    })
}

fn build_generation_options(
    config: &BenchConfig,
    max_output_tokens: usize,
    stop_sequences: &[String],
) -> GenerationOptions {
    let mut options = match config.decode_mode {
        BenchDecodeMode::Greedy => GenerationOptions::greedy(max_output_tokens),
        BenchDecodeMode::Sample => GenerationOptions::sample(max_output_tokens),
    };
    options.stop_sequences = stop_sequences.to_vec();
    options.temperature = config.effective_temperature();
    options.top_k = config.effective_top_k();
    options.top_p = config.effective_top_p();
    options.min_p = config.effective_min_p();
    options.typical_p = config.effective_typical_p();
    options.mirostat = config.effective_mirostat();
    options.mirostat_tau = config.effective_mirostat_tau();
    options.mirostat_eta = config.effective_mirostat_eta();
    options.repeat_penalty = config.effective_repeat_penalty();
    options.repeat_last_n = config.effective_repeat_last_n();
    options.presence_penalty = config.effective_presence_penalty();
    options.frequency_penalty = config.effective_frequency_penalty();
    options.seed = config.effective_seed();
    options.structured_output = match config.structured_output.as_ref() {
        Some(BenchStructuredOutput::JsonObject) => Some(StructuredOutputRequest::JsonObject),
        Some(BenchStructuredOutput::JsonSchema { name, schema }) => {
            Some(StructuredOutputRequest::JsonSchema {
                name: name.clone(),
                schema: schema.clone(),
            })
        }
        None => None,
    };
    options
}

fn psionic_generation_request(
    config: &BenchConfig,
    request_id: String,
    descriptor: psionic_models::DecoderModelDescriptor,
    rendered: &RenderedPrompt,
    max_output_tokens: usize,
) -> GenerationRequest {
    let options = build_generation_options(config, max_output_tokens, &rendered.stop_sequences);
    match config.prompt_token_ids.as_ref() {
        Some(prompt_token_ids) => GenerationRequest::new_tokens(
            request_id,
            descriptor,
            None,
            TokenSequence::new(
                prompt_token_ids
                    .iter()
                    .copied()
                    .map(TokenId)
                    .collect::<Vec<_>>(),
            ),
            options,
        ),
        None => GenerationRequest::new_text(
            request_id,
            descriptor,
            None,
            rendered.text.clone(),
            options,
        ),
    }
}

fn load_bench_model(
    model_path: &Path,
    prompt: &str,
    raw_prompt: bool,
) -> Result<BenchModelContext, String> {
    let adapter = GgufDecoderAdapterLoader
        .load_path(model_path)
        .map_err(|error| format!("failed to load GGUF metadata: {error}"))?;
    let tokenizer = GgufRuntimeTokenizer::from_gguf(adapter.tokenizer())
        .map_err(|error| format!("failed to build GGUF runtime tokenizer: {error}"))?;
    let rendered = if raw_prompt {
        RenderedPrompt {
            text: prompt.to_string(),
            stop_sequences: Vec::new(),
        }
    } else {
        let renderer = adapter.prompt_renderer();
        let rendered = renderer
            .render_with_options(
                None,
                &[PromptMessage::new(
                    PromptMessageRole::User,
                    prompt.to_string(),
                )],
                true,
                &PromptRenderOptions::default(),
            )
            .map_err(|error| format!("failed to render qwen35 prompt: {error}"))?;
        RenderedPrompt {
            text: rendered.text,
            stop_sequences: rendered.stop_sequences,
        }
    };
    Ok(BenchModelContext {
        rendered,
        tokenizer,
    })
}

fn qwen35_output_metrics_report(
    metrics: Option<&Qwen35CudaDecodeOutputMetrics>,
) -> BenchQwen35OutputMetricsReport {
    let Some(metrics) = metrics else {
        return BenchQwen35OutputMetricsReport {
            output_modes: Vec::new(),
            readback_bytes: 0,
            raw_logits: false,
            graph_hits: 0,
            graph_misses: 0,
            graph_captures: 0,
            graph_shape_drifts: 0,
            graph_cache_identity: None,
            attention_layer_invocations: 0,
            attention_backends: Vec::new(),
        };
    };
    let output_modes = metrics
        .output_modes
        .iter()
        .map(|mode| match mode {
            psionic_serve::Qwen35CudaDecodeOutputMode::ArgmaxOnly => String::from("argmax_only"),
            psionic_serve::Qwen35CudaDecodeOutputMode::TopKCandidates { top_k } => {
                format!("top_k_candidates:{top_k}")
            }
            psionic_serve::Qwen35CudaDecodeOutputMode::SparseLogits { token_count } => {
                format!("sparse_logits:{token_count}")
            }
            psionic_serve::Qwen35CudaDecodeOutputMode::RawLogits => String::from("raw_logits"),
        })
        .collect::<Vec<_>>();
    BenchQwen35OutputMetricsReport {
        output_modes,
        readback_bytes: metrics.readback_bytes,
        raw_logits: metrics.raw_logits_materialized,
        graph_hits: metrics
            .graph_replay
            .as_ref()
            .map_or(0, |graph| graph.replay_hit_count),
        graph_misses: metrics
            .graph_replay
            .as_ref()
            .map_or(0, |graph| graph.replay_miss_count),
        graph_captures: metrics
            .graph_replay
            .as_ref()
            .map_or(0, |graph| graph.capture_count),
        graph_shape_drifts: metrics
            .graph_replay
            .as_ref()
            .map_or(0, |graph| graph.shape_drift_count),
        graph_cache_identity: metrics.graph_cache_identity.clone(),
        attention_layer_invocations: metrics
            .attention_backend
            .as_ref()
            .map_or(0, |backend| backend.layer_invocation_count),
        attention_backends: metrics
            .attention_backend
            .as_ref()
            .map_or_else(Vec::new, |backend| backend.executions.clone()),
    }
}

fn format_qwen35_output_metrics(report: &BenchQwen35OutputMetricsReport) -> String {
    let attention_backends = if report.attention_backends.is_empty() {
        String::from("none")
    } else {
        report
            .attention_backends
            .iter()
            .map(|backend| {
                let split = backend
                    .split_count
                    .map(|count| format!("split{count}"))
                    .unwrap_or_else(|| String::from("nosplit"));
                let fallback = backend
                    .fallback_reason
                    .as_deref()
                    .map(|reason| format!(":{reason}"))
                    .unwrap_or_default();
                format!("{}@{}{}", backend.executed_backend, split, fallback)
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        "qwen35_output_modes=[{}] qwen35_readback_bytes={} qwen35_raw_logits={} qwen35_graph_hits={} qwen35_graph_misses={} qwen35_graph_captures={} qwen35_graph_shape_drifts={} qwen35_attention_layer_invocations={} qwen35_attention_backends=[{}]",
        report.output_modes.join(","),
        report.readback_bytes,
        report.raw_logits,
        report.graph_hits,
        report.graph_misses,
        report.graph_captures,
        report.graph_shape_drifts,
        report.attention_layer_invocations,
        attention_backends,
    )
}

fn structured_output_runtime_report(
    provenance: Option<&psionic_serve::GenerationProvenance>,
    structured_value: Option<&StructuredOutputValue>,
) -> BenchStructuredOutputRuntimeReport {
    let Some(report) = provenance.and_then(|provenance| provenance.structured_output.as_ref())
    else {
        return BenchStructuredOutputRuntimeReport {
            mode: String::from("none"),
            parser: String::from("none"),
            kind: String::from("none"),
            value: None,
        };
    };
    BenchStructuredOutputRuntimeReport {
        mode: String::from(report.mode.label()),
        parser: String::from(report.parser.label()),
        kind: String::from(report.kind.label()),
        value: structured_value.and_then(|value| serde_json::to_value(value).ok()),
    }
}

fn format_structured_output_report(report: &BenchStructuredOutputRuntimeReport) -> String {
    let value = report
        .value
        .as_ref()
        .and_then(|value| serde_json::to_string(value).ok())
        .unwrap_or_else(|| String::from("none"));
    format!(
        "structured_output_mode={} structured_output_parser={} structured_output_kind={} structured_output_value={}",
        report.mode, report.parser, report.kind, value
    )
}

fn bench_decode_mode_label(mode: BenchDecodeMode) -> &'static str {
    match mode {
        BenchDecodeMode::Greedy => "greedy",
        BenchDecodeMode::Sample => "sample",
    }
}

fn min_warmup_tokens(max_output_tokens: usize) -> usize {
    max_output_tokens.min(16).max(1)
}

fn next_arg(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    let value_index = index.saturating_add(1);
    let value = args
        .get(value_index)
        .cloned()
        .ok_or_else(|| format!("missing value for `{flag}`"))?;
    *index = value_index;
    Ok(value)
}

fn parse_structured_output(
    json_object: bool,
    json_schema_file: Option<PathBuf>,
    json_schema_name: Option<String>,
) -> Result<Option<BenchStructuredOutput>, String> {
    let selected_modes = usize::from(json_object) + usize::from(json_schema_file.is_some());
    if selected_modes > 1 {
        return Err(String::from(
            "structured output accepts at most one of `--json-object` or `--json-schema-file`",
        ));
    }
    match json_schema_file {
        Some(path) => {
            let raw = fs::read_to_string(&path).map_err(|error| {
                format!(
                    "failed to read JSON schema file `{}`: {error}",
                    path.display()
                )
            })?;
            let schema = serde_json::from_str::<Value>(&raw).map_err(|error| {
                format!(
                    "failed to parse JSON schema file `{}`: {error}",
                    path.display()
                )
            })?;
            Ok(Some(BenchStructuredOutput::JsonSchema {
                name: json_schema_name,
                schema,
            }))
        }
        None if json_object => {
            if json_schema_name.is_some() {
                return Err(String::from(
                    "`--json-schema-name` requires `--json-schema-file`",
                ));
            }
            Ok(Some(BenchStructuredOutput::JsonObject))
        }
        None => {
            if json_schema_name.is_some() {
                return Err(String::from(
                    "`--json-schema-name` requires `--json-schema-file`",
                ));
            }
            Ok(None)
        }
    }
}

fn parse_arg<T>(value: &str, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse::<T>()
        .map_err(|error| format!("invalid {name} `{value}`: {error}"))
}

fn parse_token_ids(value: &str) -> Result<Vec<u32>, String> {
    if value.trim().is_empty() {
        return Err(String::from("--prompt-token-ids must not be empty"));
    }
    value
        .split(',')
        .map(|field| {
            let field = field.trim();
            if field.is_empty() {
                return Err(format!(
                    "invalid --prompt-token-ids `{value}`: empty token field"
                ));
            }
            parse_arg(field, "--prompt-token-ids")
        })
        .collect()
}

fn tokens_per_second(tokens: usize, duration_ns: u64) -> f64 {
    if tokens == 0 || duration_ns == 0 {
        return 0.0;
    }
    tokens as f64 / nanos_to_seconds(duration_ns)
}

fn nanos_to_seconds(duration_ns: u64) -> f64 {
    duration_ns as f64 / 1_000_000_000.0
}

fn current_unix_timestamp_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn build_bench_report(
    config: &BenchConfig,
    rendered: &RenderedPrompt,
    psionic_cuda_startup: Option<BenchPsionicCudaStartupReport>,
    load_s: Option<f64>,
    psionic_cuda_fast_path: Option<BenchPsionicCudaFastPathReport>,
    llama_cpp_server: Option<BenchLlamaCppServerReport>,
    run_status: String,
    refusal_reason: Option<String>,
    runs: Vec<BenchRunReport>,
) -> BenchReport {
    let repeats = runs.len().max(1) as f64;
    let mean_output_tokens = runs.iter().map(|run| run.output_tokens as f64).sum::<f64>() / repeats;
    let mean_prompt_s = runs.iter().map(|run| run.prompt_s).sum::<f64>() / repeats;
    let mean_decode_s = runs.iter().map(|run| run.decode_s).sum::<f64>() / repeats;
    let mean_total_s = runs.iter().map(|run| run.total_s).sum::<f64>() / repeats;
    let mean_ttft_s = mean_optional_seconds(runs.iter().filter_map(|run| run.ttft_s));
    let mean_itl_s = mean_optional_seconds(runs.iter().filter_map(|run| run.itl_s));
    let mean_decode_tok_s = runs.iter().map(|run| run.decode_tok_s).sum::<f64>() / repeats;
    BenchReport {
        schema_version: 7,
        report_kind: String::from("qwen35_cuda_bench"),
        run_status,
        refusal_reason,
        benchmark_class: String::from(bench_report_class(config.backend)),
        generated_at_unix_s: current_unix_timestamp_seconds(),
        backend: String::from(bench_backend_label(config.backend)),
        model_path: config.model_path.display().to_string(),
        ollama_model: config.ollama_model.clone(),
        ollama_base_url: matches!(config.backend, BenchBackend::Ollama)
            .then(|| config.ollama_base_url.clone()),
        prompt: config.prompt.clone(),
        prompt_mode: String::from(if config.prompt_token_ids.is_some() {
            "token_ids"
        } else if config.raw_prompt {
            "raw_text"
        } else {
            "chat_template"
        }),
        prompt_token_ids: config.prompt_token_ids.clone(),
        rendered_prompt: rendered.text.clone(),
        stop_sequences: rendered.stop_sequences.clone(),
        decode_mode: String::from(bench_decode_mode_label(config.decode_mode)),
        max_output_tokens: config.max_output_tokens,
        repeats: runs.len(),
        steady_state_concurrency: 1,
        load_s,
        temperature: config.effective_temperature_for_backend(config.backend),
        top_k: config.effective_top_k_for_backend(config.backend),
        top_p: config.effective_top_p_for_backend(config.backend),
        min_p: config.effective_min_p_for_backend(config.backend),
        typical_p: config.effective_typical_p(),
        mirostat: config.effective_mirostat(),
        mirostat_tau: config.effective_mirostat_tau(),
        mirostat_eta: config.effective_mirostat_eta(),
        repeat_penalty: config.effective_repeat_penalty_for_backend(config.backend),
        repeat_last_n: config.effective_repeat_last_n_for_backend(config.backend),
        presence_penalty: config.effective_presence_penalty_for_backend(config.backend),
        frequency_penalty: config.effective_frequency_penalty_for_backend(config.backend),
        seed: config.effective_seed_for_backend(config.backend),
        structured_output: structured_output_config_report(config.structured_output.as_ref()),
        llama_cpp_server,
        psionic_cuda_startup,
        psionic_cuda_fast_path,
        runs,
        mean_output_tokens,
        mean_prompt_s,
        mean_decode_s,
        mean_total_s,
        mean_ttft_s,
        mean_itl_s,
        mean_decode_tok_s,
    }
}

fn required_cuda_allocator_telemetry(
    service: &CudaGgufQwen35TextGenerationService,
) -> Result<CudaAllocatorPoolTelemetry, String> {
    service.cuda_allocator_pool_telemetry().ok_or_else(|| {
        String::from("cuda allocator telemetry is unavailable for the selected Psionic backend")
    })
}

fn cuda_gemm_tuning_startup_fields(
    report: Option<CudaGemmTuningReport>,
) -> BenchCudaGemmTuningStartupFields {
    let Some(report) = report else {
        return BenchCudaGemmTuningStartupFields {
            tuning_status: String::from("unavailable"),
            plan_cache_scope: String::from("unknown"),
            ..BenchCudaGemmTuningStartupFields::default()
        };
    };
    BenchCudaGemmTuningStartupFields {
        tuning_status: report.tuning_status,
        plan_cache_scope: report.plan_cache_scope,
        selected_plan_count: report.selected_plan_count,
        tuned_shape_count: report.tuned_shape_count,
        fallback_shape_count: report.fallback_shape_count,
        max_workspace_bytes: report.max_workspace_bytes,
        selected_plans: report
            .selected_plans
            .into_iter()
            .map(|plan| BenchCudaGemmTuningPlanReport {
                model_family: plan.model_family,
                op_kind: plan.op_kind,
                rows: plan.rows,
                inner: plan.inner,
                cols: plan.cols,
                input_dtype: plan.input_dtype,
                output_dtype: plan.output_dtype,
                backend_route: plan.backend_route,
                workspace_bytes: plan.workspace_bytes,
                mean_time_us: plan.mean_time_us,
                algorithm_fingerprint: plan.algorithm_fingerprint,
            })
            .collect(),
    }
}

fn psionic_cuda_fast_path_report(config: &BenchConfig) -> BenchPsionicCudaFastPathReport {
    BenchPsionicCudaFastPathReport {
        lane: String::from(if config.require_fallback_free_cuda {
            "fallback_free_fast_path"
        } else {
            "explicit_fallback_path"
        }),
        status: String::from(if config.require_fallback_free_cuda {
            "validated"
        } else {
            "compatibility_fallback_allowed"
        }),
        refusal_reason: None,
        required_output_modes: vec![String::from("argmax_only")],
        raw_logits_forbidden: true,
        host_fallback_forbidden: true,
        graph_capture_required: true,
        env_guards: qwen35_fast_path_env_guards(),
    }
}

fn qwen35_fast_path_env_guards() -> Vec<BenchEnvGuardReport> {
    [
        "PSIONIC_QWEN35_DISABLE_FAST_GREEDY",
        "PSIONIC_QWEN35_DISABLE_FUSED_QKV_RMS_NORM",
        "PSIONIC_QWEN35_DEBUG_ATTENTION",
        "PSIONIC_QWEN35_DEBUG_FUSED_LAYERS",
    ]
    .into_iter()
    .map(|name| BenchEnvGuardReport {
        name: String::from(name),
        required_state: String::from("unset"),
        actual_state: env::var(name).unwrap_or_else(|_| String::from("unset")),
    })
    .collect()
}

fn validate_psionic_cuda_fast_path_contract(
    config: &BenchConfig,
    report: &mut BenchPsionicCudaFastPathReport,
) -> Result<(), String> {
    if !config.require_fallback_free_cuda {
        return Ok(());
    }
    let refusal = if !matches!(config.backend, BenchBackend::Psionic) {
        Some(String::from(
            "fallback-free cuda benchmark lane is only admitted on `--backend psionic`",
        ))
    } else if !matches!(config.decode_mode, BenchDecodeMode::Greedy) {
        Some(String::from(
            "fallback-free cuda benchmark lane is currently admitted only for greedy decode",
        ))
    } else if config.structured_output.is_some() {
        Some(String::from(
            "fallback-free cuda benchmark lane refuses structured-output contracts",
        ))
    } else if config.temperature.is_some()
        || config.top_k.is_some()
        || config.top_p.is_some()
        || config.min_p.is_some()
        || config.typical_p.is_some()
        || config.mirostat.is_some()
        || config.repeat_penalty.is_some()
        || config.repeat_last_n.is_some()
        || config.presence_penalty.is_some()
        || config.frequency_penalty.is_some()
    {
        Some(String::from(
            "fallback-free cuda benchmark lane refuses decode knobs outside the admitted greedy fast path",
        ))
    } else {
        report
            .env_guards
            .iter()
            .find(|guard| guard.actual_state != "unset")
            .map(|guard| {
                format!(
                    "fallback-free cuda benchmark lane refused because env guard `{}` is `{}`",
                    guard.name, guard.actual_state
                )
            })
    };
    if let Some(reason) = refusal {
        mark_fast_path_refused(report, reason.clone());
        return Err(reason);
    }
    Ok(())
}

fn validate_psionic_cuda_fast_path_warmup(
    config: &BenchConfig,
    output_metrics: &BenchQwen35OutputMetricsReport,
    fallback_evidence: &BenchCudaHostFallbackEvidenceReport,
    report: &mut BenchPsionicCudaFastPathReport,
) -> Result<(), String> {
    if !config.require_fallback_free_cuda {
        return Ok(());
    }
    validate_qwen35_fast_path_metrics(output_metrics, fallback_evidence, false).map_err(|reason| {
        mark_fast_path_refused(report, reason.clone());
        reason
    })
}

fn validate_psionic_cuda_fast_path_run(
    config: &BenchConfig,
    output_metrics: &BenchQwen35OutputMetricsReport,
    fallback_evidence: &BenchCudaHostFallbackEvidenceReport,
    report: &mut BenchPsionicCudaFastPathReport,
) -> Result<(), String> {
    if !config.require_fallback_free_cuda {
        return Ok(());
    }
    validate_qwen35_fast_path_metrics(output_metrics, fallback_evidence, true).map_err(|reason| {
        mark_fast_path_refused(report, reason.clone());
        reason
    })
}

fn validate_qwen35_fast_path_metrics(
    output_metrics: &BenchQwen35OutputMetricsReport,
    fallback_evidence: &BenchCudaHostFallbackEvidenceReport,
    require_graph_hit: bool,
) -> Result<(), String> {
    if fallback_evidence.fallback_invocations > 0 {
        return Err(format!(
            "fallback-free cuda benchmark lane recorded host fallback evidence: labels={} invocations={}",
            fallback_evidence.op_labels.join(","),
            fallback_evidence.fallback_invocations
        ));
    }
    if output_metrics.raw_logits {
        return Err(String::from(
            "fallback-free cuda benchmark lane refused raw logits materialization",
        ));
    }
    if output_metrics.output_modes.as_slice() != [String::from("argmax_only")] {
        return Err(format!(
            "fallback-free cuda benchmark lane requires `argmax_only`, actual modes={}",
            output_metrics.output_modes.join(",")
        ));
    }
    if output_metrics.graph_shape_drifts > 0 {
        return Err(format!(
            "fallback-free cuda benchmark lane refused graph shape drift count={}",
            output_metrics.graph_shape_drifts
        ));
    }
    if output_metrics.graph_captures + output_metrics.graph_hits == 0 {
        return Err(String::from(
            "fallback-free cuda benchmark lane requires graph capture-ready output posture",
        ));
    }
    if require_graph_hit && output_metrics.graph_hits == 0 {
        return Err(String::from(
            "fallback-free cuda benchmark lane requires steady-state graph hits after warmup",
        ));
    }
    if require_graph_hit && output_metrics.graph_misses > output_metrics.graph_captures {
        return Err(format!(
            "fallback-free cuda benchmark lane refused unexpected graph misses outside initial capture count: misses={} captures={}",
            output_metrics.graph_misses, output_metrics.graph_captures
        ));
    }
    if require_graph_hit && output_metrics.graph_captures > 1 {
        return Err(format!(
            "fallback-free cuda benchmark lane refused repeated graph recapture count={}",
            output_metrics.graph_captures
        ));
    }
    if output_metrics.attention_layer_invocations == 0
        || output_metrics.attention_backends.is_empty()
    {
        return Err(String::from(
            "fallback-free cuda benchmark lane requires explicit attention-backend evidence",
        ));
    }
    if let Some(backend) = output_metrics.attention_backends.iter().find(|backend| {
        backend.executed_backend != psionic_serve::PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_NAME
            || backend.fallback_reason.is_some()
    }) {
        return Err(format!(
            "fallback-free cuda benchmark lane requires `{}` with no fallback, actual backend={} fallback_reason={}",
            psionic_serve::PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_NAME,
            backend.executed_backend,
            backend.fallback_reason.as_deref().unwrap_or("none"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_ready_output_metrics(
        graph_hits: usize,
        graph_misses: usize,
        graph_captures: usize,
    ) -> BenchQwen35OutputMetricsReport {
        BenchQwen35OutputMetricsReport {
            output_modes: vec![String::from("argmax_only")],
            readback_bytes: 4,
            raw_logits: false,
            graph_hits,
            graph_misses,
            graph_captures,
            graph_shape_drifts: 0,
            graph_cache_identity: Some(String::from("test-graph-cache-identity")),
            attention_layer_invocations: 27,
            attention_backends: vec![psionic_serve::Qwen35CudaAttentionBackendExecution {
                requested_backend: String::from(
                    psionic_serve::PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_NAME,
                ),
                executed_backend: String::from(
                    psionic_serve::PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_NAME,
                ),
                fallback_reason: None,
                graph_capture_compatible: true,
                split_count: Some(1),
                architecture: Some(String::from("ada")),
                compute_capability: Some(String::from("8.9")),
            }],
        }
    }

    #[test]
    fn fallback_free_cuda_gate_allows_single_initial_graph_capture() {
        let metrics = graph_ready_output_metrics(27, 1, 1);
        let fallback_evidence = BenchCudaHostFallbackEvidenceReport::default();
        assert!(validate_qwen35_fast_path_metrics(&metrics, &fallback_evidence, true).is_ok());
    }

    #[test]
    fn fallback_free_cuda_gate_rejects_repeated_graph_recapture() {
        let metrics = graph_ready_output_metrics(27, 2, 2);
        let fallback_evidence = BenchCudaHostFallbackEvidenceReport::default();
        let error = validate_qwen35_fast_path_metrics(&metrics, &fallback_evidence, true)
            .expect_err("repeated graph recapture should be refused");
        assert!(error.contains("repeated graph recapture"));
    }

    #[test]
    fn bench_config_parses_exact_prompt_token_ids() {
        let config = BenchConfig::parse(
            [
                "--backend",
                "psionic",
                "--model-path",
                "model.gguf",
                "--prompt",
                "Hello",
                "--prompt-token-ids",
                "9419, 11",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("exact prompt token ids should parse");
        assert_eq!(config.prompt_token_ids, Some(vec![9419, 11]));
        assert!(!config.raw_prompt);
    }

    #[test]
    fn bench_config_refuses_raw_text_with_exact_prompt_token_ids() {
        let error = BenchConfig::parse(
            [
                "--backend",
                "psionic",
                "--model-path",
                "model.gguf",
                "--raw-prompt",
                "--prompt-token-ids",
                "9419",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect_err("raw and exact-token prompt modes must be exclusive");
        assert!(error.contains("mutually exclusive"));
    }
}

fn mark_fast_path_refused(report: &mut BenchPsionicCudaFastPathReport, reason: String) {
    report.lane = String::from("unsupported_or_refused");
    report.status = String::from("refused");
    report.refusal_reason = Some(reason);
}

fn fallback_profile_capture_path() -> PathBuf {
    let mut path = env::temp_dir();
    path.push(format!(
        "psionic_qwen35_cuda_host_fallback_{}_{}.jsonl",
        std::process::id(),
        current_unix_timestamp_seconds()
    ));
    path
}

fn read_host_fallback_evidence_delta(
    path: &Path,
    previous_record_count: &mut usize,
) -> Result<BenchCudaHostFallbackEvidenceReport, String> {
    let records = read_host_fallback_records(path)?;
    let start = (*previous_record_count).min(records.len());
    let evidence = host_fallback_evidence_from_records(&records[start..]);
    *previous_record_count = records.len();
    Ok(evidence)
}

fn read_host_fallback_records(path: &Path) -> Result<Vec<CudaHostFallbackProfileReport>, String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<CudaHostFallbackProfileReport>(line).map_err(|error| {
                format!(
                    "failed to parse cuda host fallback profile `{}`: {error}",
                    path.display()
                )
            })
        })
        .collect()
}

fn host_fallback_evidence_from_records(
    records: &[CudaHostFallbackProfileReport],
) -> BenchCudaHostFallbackEvidenceReport {
    let mut labels = BTreeSet::new();
    let mut op_count = 0_usize;
    let mut case_count = 0_usize;
    let mut fallback_invocations = 0_u64;
    let mut total_host_fallback_ms = 0_u64;
    for record in records {
        total_host_fallback_ms =
            total_host_fallback_ms.saturating_add(record.total_host_fallback_ms);
        for op in &record.ops {
            labels.insert(op.label.clone());
            op_count = op_count.saturating_add(1);
            case_count = case_count.saturating_add(op.cases.len());
            fallback_invocations = fallback_invocations.saturating_add(op.count);
        }
    }
    BenchCudaHostFallbackEvidenceReport {
        report_count: records.len(),
        op_count,
        case_count,
        fallback_invocations,
        total_host_fallback_ms,
        op_labels: labels.into_iter().collect(),
    }
}

fn structured_output_config_report(
    structured_output: Option<&BenchStructuredOutput>,
) -> BenchStructuredOutputConfigReport {
    match structured_output {
        Some(BenchStructuredOutput::JsonObject) => BenchStructuredOutputConfigReport {
            mode: String::from("json_object"),
            schema_name: None,
            schema: None,
        },
        Some(BenchStructuredOutput::JsonSchema { name, schema }) => {
            BenchStructuredOutputConfigReport {
                mode: String::from("json_schema"),
                schema_name: name.clone(),
                schema: Some(schema.clone()),
            }
        }
        None => BenchStructuredOutputConfigReport {
            mode: String::from("none"),
            schema_name: None,
            schema: None,
        },
    }
}

fn bench_backend_label(backend: BenchBackend) -> &'static str {
    match backend {
        BenchBackend::Psionic => "psionic",
        BenchBackend::Ollama => "ollama",
        BenchBackend::LlamaCpp => "llama_cpp",
    }
}

fn bench_report_class(backend: BenchBackend) -> &'static str {
    match backend {
        BenchBackend::Psionic => "direct_engine",
        BenchBackend::Ollama => "http",
        BenchBackend::LlamaCpp => "http",
    }
}

fn mean_optional_seconds(values: impl Iterator<Item = f64>) -> Option<f64> {
    let values = values.collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn format_optional_seconds(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.6}"))
        .unwrap_or_else(|| String::from("none"))
}

fn write_json_output<T: Serialize>(value: &T, output: Option<&PathBuf>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize JSON report: {error}"))?;
    match output {
        Some(path) => fs::write(path, format!("{json}\n"))
            .map_err(|error| format!("failed to write JSON report `{}`: {error}", path.display())),
        None => Ok(()),
    }
}

fn usage() -> String {
    String::from(
        "usage:\n  cargo run -p psionic-serve --example qwen35_cuda_bench -- <model.gguf> [prompt] [max_output_tokens] [repeats]\n  cargo run -p psionic-serve --example qwen35_cuda_bench -- --backend psionic --model-path <model.gguf> [--require-fallback-free-cuda] [--decode greedy|sample] [--temperature 0.8] [--top-k 40] [--top-p 0.9] [--min-p 0.05] [--typical-p 0.5] [--mirostat 1|2] [--mirostat-tau 5.0] [--mirostat-eta 0.1] [--repeat-penalty 1.0] [--repeat-last-n 64] [--presence-penalty 0.0] [--frequency-penalty 0.0] [--seed 42] [--json-object | --json-schema-file schema.json [--json-schema-name summary]] [--json-out report.json] [--prompt <text>] [--raw-prompt | --prompt-token-ids 9419,11] [--max-output-tokens 128] [--repeats 3]\n  cargo run -p psionic-serve --example qwen35_cuda_bench -- --backend ollama --model-path <model.gguf> --ollama-model qwen3.5:0.8b [--decode greedy|sample] [--temperature 0.8] [--top-k 40] [--top-p 0.9] [--min-p 0.05] [--typical-p 0.5] [--mirostat 1|2] [--mirostat-tau 5.0] [--mirostat-eta 0.1] [--repeat-penalty 1.0] [--repeat-last-n 64] [--presence-penalty 0.0] [--frequency-penalty 0.0] [--seed 42] [--json-object | --json-schema-file schema.json [--json-schema-name summary]] [--json-out report.json] [--prompt <text>] [--raw-prompt] [--max-output-tokens 128] [--repeats 3]\n  cargo run -p psionic-serve --example qwen35_cuda_bench -- --backend llama_cpp --model-path <model.gguf> [--llama-server-bin <path>] [--llama-server-port <port>] [--llama-gpu-layers 99] [--llama-context-size 4096] [--decode greedy|sample] [--temperature 0.8] [--top-k 40] [--top-p 0.9] [--min-p 0.05] [--seed 42] [--json-out report.json] [--prompt <text>] [--raw-prompt | --prompt-token-ids 9419,11] [--max-output-tokens 128] [--repeats 3]",
    )
}

#[derive(Clone, Debug)]
struct RenderedPrompt {
    text: String,
    stop_sequences: Vec<String>,
}

#[derive(Clone, Debug)]
struct BenchModelContext {
    rendered: RenderedPrompt,
    tokenizer: GgufRuntimeTokenizer,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
    #[serde(default)]
    prompt_eval_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<usize>,
    #[serde(default)]
    eval_duration: Option<u64>,
}

fn token_ids(tokens: &[TokenId]) -> Vec<u32> {
    tokens.iter().copied().map(TokenId::as_u32).collect()
}

fn psionic_termination_report(
    response: &GenerationResponse,
    tokenizer: &GgufRuntimeTokenizer,
    stop_sequences: &[String],
) -> BenchTerminationReport {
    let observed = termination_reason_label(response.termination).to_string();
    if let Some(detail) = response.metrics.termination_detail.as_ref() {
        return BenchTerminationReport {
            observed,
            classification: termination_cause_label(detail.cause).to_string(),
            matched_stop_sequence: detail.matched_stop_sequence.clone(),
        };
    }
    let classification = match response.termination {
        TerminationReason::EndOfSequence => response
            .output
            .tokens
            .as_slice()
            .last()
            .copied()
            .filter(|token| tokenizer.is_end_of_sequence(*token))
            .map(|_| String::from("eos_token"))
            .unwrap_or_else(|| {
                if stop_sequences.is_empty() {
                    String::from("end_of_sequence")
                } else {
                    String::from("stop_sequence")
                }
            }),
        TerminationReason::MaxOutputTokens => String::from("max_output_tokens"),
        TerminationReason::ContextLimit => String::from("context_limit"),
        TerminationReason::Cancelled => String::from("cancelled"),
        TerminationReason::Disconnected => String::from("disconnected"),
        TerminationReason::Error => String::from("error"),
    };
    BenchTerminationReport {
        observed,
        classification,
        matched_stop_sequence: None,
    }
}

fn ollama_termination_report(
    response: &OllamaGenerateResponse,
    stop_sequences: &[String],
    max_output_tokens: usize,
) -> BenchTerminationReport {
    let observed = response
        .done_reason
        .clone()
        .or_else(|| response.error.as_ref().map(|_| String::from("error")))
        .unwrap_or_else(|| String::from("unknown"));
    let classification = if response.error.is_some() {
        String::from("error")
    } else {
        match response.done_reason.as_deref() {
            Some("length") => String::from("max_output_tokens"),
            Some("stop") if stop_sequences.is_empty() => String::from("eos_token"),
            Some("stop") => String::from("ambiguous_stop_or_eos"),
            Some("unload") => String::from("unknown"),
            Some(other) => other.replace('-', "_"),
            None if response.eval_count.unwrap_or(0) >= max_output_tokens => {
                String::from("max_output_tokens")
            }
            None => String::from("unknown"),
        }
    };
    BenchTerminationReport {
        observed,
        classification,
        matched_stop_sequence: None,
    }
}

fn termination_reason_label(reason: TerminationReason) -> &'static str {
    match reason {
        TerminationReason::EndOfSequence => "end_of_sequence",
        TerminationReason::MaxOutputTokens => "max_output_tokens",
        TerminationReason::ContextLimit => "context_limit",
        TerminationReason::Cancelled => "cancelled",
        TerminationReason::Disconnected => "disconnected",
        TerminationReason::Error => "error",
    }
}

fn termination_cause_label(cause: GenerationTerminationCause) -> &'static str {
    match cause {
        GenerationTerminationCause::EndOfSequenceToken => "eos_token",
        GenerationTerminationCause::StopSequence => "stop_sequence",
        GenerationTerminationCause::MaxOutputTokens => "max_output_tokens",
        GenerationTerminationCause::ContextLimit => "context_limit",
        GenerationTerminationCause::Cancelled => "cancelled",
        GenerationTerminationCause::Disconnected => "disconnected",
        GenerationTerminationCause::Error => "error",
    }
}

fn ollama_generate(
    client: &Client,
    base_url: &str,
    model: &str,
    rendered: &RenderedPrompt,
    config: &BenchConfig,
    max_output_tokens: usize,
) -> Result<OllamaGenerateResponse, String> {
    let mut options = serde_json::json!({
        "num_predict": max_output_tokens,
    });
    if let Some(temperature) = config.effective_temperature_for_backend(BenchBackend::Ollama) {
        options["temperature"] = serde_json::json!(temperature);
    }
    if let Some(top_k) = config.effective_top_k_for_backend(BenchBackend::Ollama) {
        options["top_k"] = serde_json::json!(top_k);
    }
    if let Some(top_p) = config.effective_top_p_for_backend(BenchBackend::Ollama) {
        options["top_p"] = serde_json::json!(top_p);
    }
    if let Some(min_p) = config.effective_min_p_for_backend(BenchBackend::Ollama) {
        options["min_p"] = serde_json::json!(min_p);
    }
    if let Some(typical_p) = config.effective_typical_p() {
        options["typical_p"] = serde_json::json!(typical_p);
    }
    if let Some(mirostat) = config.effective_mirostat() {
        options["mirostat"] = serde_json::json!(mirostat);
    }
    if let Some(mirostat_tau) = config.effective_mirostat_tau() {
        options["mirostat_tau"] = serde_json::json!(mirostat_tau);
    }
    if let Some(mirostat_eta) = config.effective_mirostat_eta() {
        options["mirostat_eta"] = serde_json::json!(mirostat_eta);
    }
    if let Some(repeat_penalty) = config.effective_repeat_penalty_for_backend(BenchBackend::Ollama)
    {
        options["repeat_penalty"] = serde_json::json!(repeat_penalty);
    }
    if let Some(repeat_last_n) = config.effective_repeat_last_n_for_backend(BenchBackend::Ollama) {
        options["repeat_last_n"] = serde_json::json!(repeat_last_n);
    }
    if let Some(presence_penalty) =
        config.effective_presence_penalty_for_backend(BenchBackend::Ollama)
    {
        options["presence_penalty"] = serde_json::json!(presence_penalty);
    }
    if let Some(frequency_penalty) =
        config.effective_frequency_penalty_for_backend(BenchBackend::Ollama)
    {
        options["frequency_penalty"] = serde_json::json!(frequency_penalty);
    }
    if let Some(seed) = config.effective_seed_for_backend(BenchBackend::Ollama) {
        options["seed"] = serde_json::json!(seed);
    }
    if !rendered.stop_sequences.is_empty() {
        options["stop"] = serde_json::json!(rendered.stop_sequences);
    }
    let mut payload = serde_json::json!({
        "model": model,
        "prompt": rendered.text,
        "raw": true,
        "stream": false,
        "think": false,
        "keep_alive": 0,
        "options": options,
    });
    if let Some(format) = config.ollama_format_payload() {
        payload["format"] = format;
    }
    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));
    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .map_err(|error| format!("failed to call Ollama generate endpoint: {error}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .unwrap_or_else(|_| String::from("<unreadable response body>"));
        return Err(format!(
            "Ollama generate request failed with {status}: {body}"
        ));
    }
    response
        .json::<OllamaGenerateResponse>()
        .map_err(|error| format!("failed to decode Ollama generate response: {error}"))
}
