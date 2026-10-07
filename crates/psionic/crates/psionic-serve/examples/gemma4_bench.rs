use std::{
    env,
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use psionic_cluster::{
    AdmissionToken, ClusterArtifactReference, ClusterArtifactResidencyKey,
    ClusterArtifactResidencyRecord, ClusterArtifactResidencyStatus, ClusterBackendReadinessStatus,
    ClusterId, ClusterMembershipRecord, ClusterMembershipStatus, ClusterNamespace,
    ClusterNodeIdentity, ClusterNodeTelemetry, ClusterSnapshot, ClusterState,
    Gemma4MoeDistributedLaneRequest, NodeEpoch, NodeId, NodeRole, SparseExpertHostInventoryRecord,
    SparseExpertHostInventorySnapshot,
};
use psionic_models::{
    GgufDecoderAdapterLoader, GgufDecoderFamily, PromptMessage, PromptMessageRole,
};
use psionic_serve::{
    CpuGgufTextGenerationService, CudaGemma4TextGenerationService, DistributedGemma4PeerConfig,
    DistributedGemma4TextGenerationService, Gemma4SparseExecutionObservation, GenerationMetrics,
    GenerationOptions, GenerationRequest, GenerationResponse, MetalGemma4TextGenerationService,
    OpenAiCompatBackend, OpenAiCompatConfig, OpenAiCompatServer, TerminationReason,
    TextGenerationExecutor,
};
use serde::Serialize;
use serde_json::json;
use tokio::{net::TcpListener, sync::oneshot};

const LOCAL_METAL_BENCH_OVERRIDE_ENV: &str = "PSIONIC_ALLOW_PARALLEL_METAL_BENCH";
const LOCAL_METAL_BENCH_LOCK_NAME: &str = "gemma4-metal-bench.lock";

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(async_main()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn async_main() -> Result<(), String> {
    let config = BenchConfig::parse(env::args().skip(1))?;
    let _local_metal_guard = LocalMetalBenchGuard::acquire(&config)?;
    let report = match config.mode {
        BenchMode::Single => run_dense_benchmark(&config, config.backend)?,
        BenchMode::DistributedDense => run_dense_benchmark(&config, DenseBenchBackend::Distributed)
            .map(|mut report| {
                report.peer_base_url = config.peer_base_url.clone();
                report.split_layer = config.split_layer;
                report
            })?,
        BenchMode::DistributedSparse => run_sparse_benchmark(&config).await?,
    };
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
enum BenchMode {
    Single,
    DistributedDense,
    DistributedSparse,
}

impl BenchMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "single" => Ok(Self::Single),
            "distributed-dense" => Ok(Self::DistributedDense),
            "distributed-sparse" => Ok(Self::DistributedSparse),
            other => Err(format!(
                "unsupported Gemma benchmark mode `{other}`; expected one of: single, distributed-dense, distributed-sparse"
            )),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::DistributedDense => "distributed_dense",
            Self::DistributedSparse => "distributed_sparse",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PromptMode {
    RenderedTokens,
    Text,
}

impl PromptMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "rendered-tokens" => Ok(Self::RenderedTokens),
            "text" => Ok(Self::Text),
            other => Err(format!(
                "unsupported Gemma prompt mode `{other}`; expected one of: rendered-tokens, text"
            )),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::RenderedTokens => "rendered_tokens",
            Self::Text => "text",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DenseBenchBackend {
    Auto,
    Cpu,
    Metal,
    Cuda,
    Distributed,
}

impl DenseBenchBackend {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "cpu" => Ok(Self::Cpu),
            "metal" => Ok(Self::Metal),
            "cuda" => Ok(Self::Cuda),
            other => Err(format!(
                "unsupported Gemma backend `{other}`; expected one of: auto, cpu, metal, cuda"
            )),
        }
    }

    fn runtime_label(self) -> &'static str {
        match self {
            Self::Auto => {
                if cfg!(target_os = "macos") {
                    "metal"
                } else {
                    "cuda"
                }
            }
            Self::Metal => "metal",
            Self::Cuda => "cuda",
            Self::Cpu => "cpu",
            Self::Distributed => "metal+cuda",
        }
    }

    fn resolve_single(self) -> Self {
        match self {
            Self::Auto => {
                if cfg!(target_os = "macos") {
                    Self::Metal
                } else {
                    Self::Cuda
                }
            }
            other => other,
        }
    }

    fn resolve_sparse(self) -> Result<Self, String> {
        match self {
            Self::Auto => Ok(self.resolve_single()),
            Self::Metal | Self::Cuda => Ok(self),
            Self::Distributed => Err(String::from(
                "distributed-sparse benchmarks require a local backend; use auto, metal, or cuda",
            )),
            Self::Cpu => Err(String::from(
                "distributed-sparse benchmarks do not support the cpu backend; use auto, metal, or cuda",
            )),
        }
    }

    fn openai_backend(self) -> OpenAiCompatBackend {
        match self.resolve_single() {
            Self::Cuda => OpenAiCompatBackend::Cuda,
            Self::Metal => OpenAiCompatBackend::Metal,
            Self::Auto | Self::Cpu | Self::Distributed => {
                unreachable!("backend should be resolved before use")
            }
        }
    }
}

#[derive(Debug)]
struct LocalMetalBenchGuard {
    lock_path: PathBuf,
}

impl LocalMetalBenchGuard {
    fn acquire(config: &BenchConfig) -> Result<Option<Self>, String> {
        if !config.requires_interactive_metal_guard()
            || env::var_os(LOCAL_METAL_BENCH_OVERRIDE_ENV).is_some()
        {
            return Ok(None);
        }
        let lock_dir = env::temp_dir().join("psionic-bench-locks");
        fs::create_dir_all(&lock_dir).map_err(|error| {
            format!(
                "failed to create local Metal benchmark lock directory {}: {error}",
                lock_dir.display()
            )
        })?;
        let lock_path = lock_dir.join(LOCAL_METAL_BENCH_LOCK_NAME);
        Self::acquire_at_path(lock_path).map(Some)
    }

    fn acquire_at_path(lock_path: PathBuf) -> Result<Self, String> {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut lock_file) => {
                let _ = writeln!(lock_file, "pid={}", std::process::id());
                let _ = writeln!(
                    lock_file,
                    "cwd={}",
                    env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .display()
                );
                Ok(Self { lock_path })
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => Err(format!(
                "refusing to start a second local Metal Gemma benchmark on the interactive host because {} already exists. Wait for the current run to finish, remove the stale lock if an earlier run crashed, or set {}=1 to override.",
                lock_path.display(),
                LOCAL_METAL_BENCH_OVERRIDE_ENV,
            )),
            Err(error) => Err(format!(
                "failed to create local Metal benchmark lock {}: {error}",
                lock_path.display()
            )),
        }
    }
}

impl Drop for LocalMetalBenchGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

#[derive(Clone, Debug)]
struct BenchConfig {
    model_path: PathBuf,
    mode: BenchMode,
    backend: DenseBenchBackend,
    benchmark_prompt_id: String,
    prompt_mode: PromptMode,
    prompt: String,
    max_output_tokens: usize,
    repeats: usize,
    peer_base_url: Option<String>,
    split_layer: Option<usize>,
    json_stdout: bool,
    json_out: Option<PathBuf>,
}

impl BenchConfig {
    fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut model_path = None::<PathBuf>;
        let mut mode = BenchMode::Single;
        let mut backend = DenseBenchBackend::Auto;
        let mut benchmark_prompt_id = String::from("custom_inline_prompt");
        let mut prompt_mode = PromptMode::RenderedTokens;
        let mut prompt =
            String::from("Write one short sentence about decentralized Gemma inference.");
        let mut max_output_tokens = 96usize;
        let mut repeats = 3usize;
        let mut peer_base_url = None::<String>;
        let mut split_layer = None::<usize>;
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
                "--mode" => {
                    mode = BenchMode::parse(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --mode"))?
                            .as_str(),
                    )?;
                }
                "--backend" => {
                    backend = DenseBenchBackend::parse(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --backend"))?
                            .as_str(),
                    )?;
                }
                "--prompt-id" => {
                    benchmark_prompt_id = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --prompt-id"))?;
                }
                "--prompt-mode" => {
                    prompt_mode = PromptMode::parse(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --prompt-mode"))?
                            .as_str(),
                    )?;
                }
                "--prompt" => {
                    prompt = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --prompt"))?;
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
                "--peer-base-url" => {
                    peer_base_url = Some(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --peer-base-url"))?,
                    );
                }
                "--split-layer" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --split-layer"))?;
                    split_layer =
                        Some(value.parse::<usize>().map_err(|error| {
                            format!("invalid --split-layer `{value}`: {error}")
                        })?);
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
        if matches!(mode, BenchMode::DistributedDense) && peer_base_url.is_none() {
            return Err(String::from(
                "distributed-dense benchmarks require --peer-base-url",
            ));
        }
        Ok(Self {
            model_path,
            mode,
            backend,
            benchmark_prompt_id,
            prompt_mode,
            prompt,
            max_output_tokens,
            repeats,
            peer_base_url,
            split_layer,
            json_stdout,
            json_out,
        })
    }

    fn requires_interactive_metal_guard(&self) -> bool {
        match self.mode {
            BenchMode::Single => matches!(self.backend.resolve_single(), DenseBenchBackend::Metal),
            BenchMode::DistributedDense => cfg!(target_os = "macos"),
            BenchMode::DistributedSparse => self
                .backend
                .resolve_sparse()
                .is_ok_and(|backend| matches!(backend, DenseBenchBackend::Metal)),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct BenchReport {
    schema_version: u32,
    report_kind: String,
    mode: String,
    model_id: String,
    model_path: String,
    model_artifact: String,
    benchmark_prompt_id: String,
    runtime_backend: String,
    sparse_expert_topology: bool,
    sparse_layer_count: Option<usize>,
    host_fallback_layer_count: Option<usize>,
    sparse_ffn_backend: Option<String>,
    router_backend: Option<String>,
    expert_dispatch_backend: Option<String>,
    native_sparse_execution: Option<bool>,
    host_fallback_observed: Option<bool>,
    peer_base_url: Option<String>,
    split_layer: Option<usize>,
    prompt: String,
    prompt_mode: String,
    max_output_tokens: usize,
    repeats: usize,
    load_s: f64,
    prompt_eval: Option<f64>,
    decode: Option<f64>,
    total: f64,
    decode_tok_s: Option<f64>,
    ttft: Option<f64>,
    state_readback_bytes: Option<u64>,
    state_readback_bytes_per_token: Option<f64>,
    cluster_topology: Option<String>,
    runs: Vec<BenchRunReport>,
    mean_output_tokens: f64,
    mean_total_s: f64,
    mean_ttft_s: Option<f64>,
    mean_decode_tok_s: Option<f64>,
    mean_gemma4_metal_decode_readback_bytes_per_token: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
struct BenchRunReport {
    run_index: usize,
    output_tokens: usize,
    output_token_ids: Vec<u32>,
    prompt_eval: Option<f64>,
    decode: Option<f64>,
    total: f64,
    state_readback_bytes: Option<u64>,
    total_s: f64,
    prompt_s: Option<f64>,
    decode_s: Option<f64>,
    ttft_s: Option<f64>,
    decode_tok_s: Option<f64>,
    gemma4_metal_decode_output_modes: Option<Vec<String>>,
    gemma4_metal_decode_readback_bytes: Option<u64>,
    gemma4_metal_decode_readback_bytes_per_token: Option<f64>,
    gemma4_metal_host_kv_materialization_events: Option<usize>,
    gemma4_metal_host_kv_materialization_tokens: Option<usize>,
    termination: String,
    output_text: String,
}

enum DenseRuntime {
    Cpu(CpuGgufTextGenerationService),
    Metal(MetalGemma4TextGenerationService),
    Cuda(CudaGemma4TextGenerationService),
    Distributed(DistributedGemma4TextGenerationService),
}

impl DenseRuntime {
    fn model_id(&self) -> &str {
        match self {
            Self::Cpu(service) => service.model_descriptor().model.model_id.as_str(),
            Self::Metal(service) => service.model_descriptor().model.model_id.as_str(),
            Self::Cuda(service) => service.model_descriptor().model.model_id.as_str(),
            Self::Distributed(service) => service.model_descriptor().model.model_id.as_str(),
        }
    }

    fn model_descriptor(&self) -> psionic_models::DecoderModelDescriptor {
        match self {
            Self::Cpu(service) => service.model_descriptor().clone(),
            Self::Metal(service) => service.model_descriptor().clone(),
            Self::Cuda(service) => service.model_descriptor().clone(),
            Self::Distributed(service) => service.model_descriptor().clone(),
        }
    }

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, String> {
        match self {
            Self::Cpu(service) => service.generate(request).map_err(|error| error.to_string()),
            Self::Metal(service) => service.generate(request).map_err(|error| error.to_string()),
            Self::Cuda(service) => service.generate(request).map_err(|error| error.to_string()),
            Self::Distributed(service) => {
                service.generate(request).map_err(|error| error.to_string())
            }
        }
    }

    fn sparse_execution_observation(&self) -> Option<Gemma4SparseExecutionObservation> {
        match self {
            Self::Cpu(_) | Self::Distributed(_) => None,
            Self::Metal(service) => service.sparse_execution_observation(),
            Self::Cuda(service) => service.sparse_execution_observation(),
        }
    }
}

fn run_dense_benchmark(
    config: &BenchConfig,
    backend: DenseBenchBackend,
) -> Result<BenchReport, String> {
    let adapter = GgufDecoderAdapterLoader
        .load_path(config.model_path.as_path())
        .map_err(|error| {
            format!(
                "failed to inspect GGUF at {}: {error}",
                config.model_path.display()
            )
        })?;
    let sparse = adapter.family_metadata().expert_count.unwrap_or(0) > 0;
    let resolved_single_backend = validate_single_benchmark_backend(
        adapter.descriptor().model.model_id.as_str(),
        sparse,
        backend,
    )?;
    let rendered = adapter
        .render_prompt(
            None,
            &[PromptMessage::new(
                PromptMessageRole::User,
                config.prompt.clone(),
            )],
            true,
        )
        .map_err(|error| format!("failed to render Gemma prompt: {error}"))?;
    let prompt_tokens = adapter
        .prompt_renderer()
        .tokenize_rendered_prompt(rendered.text.as_str())
        .map_err(|error| format!("failed to tokenize rendered prompt: {error}"))?;
    let split_layer = config
        .split_layer
        .unwrap_or(adapter.descriptor().config.layer_count / 2);

    let load_started = Instant::now();
    let mut runtime = match backend {
        DenseBenchBackend::Auto
        | DenseBenchBackend::Cpu
        | DenseBenchBackend::Metal
        | DenseBenchBackend::Cuda => match resolved_single_backend {
            DenseBenchBackend::Cpu => DenseRuntime::Cpu(
                CpuGgufTextGenerationService::from_gguf_path(config.model_path.as_path()).map_err(
                    |error| {
                        format!(
                            "failed to load CPU Gemma runtime from {}: {error}",
                            config.model_path.display()
                        )
                    },
                )?,
            ),
            DenseBenchBackend::Metal => DenseRuntime::Metal(
                MetalGemma4TextGenerationService::from_gguf_path(config.model_path.as_path())
                    .map_err(|error| {
                        format!(
                            "failed to load Metal Gemma 4 runtime from {}: {error}",
                            config.model_path.display()
                        )
                    })?,
            ),
            DenseBenchBackend::Cuda => DenseRuntime::Cuda(
                CudaGemma4TextGenerationService::from_gguf_path(config.model_path.as_path())
                    .map_err(|error| {
                        format!(
                            "failed to load CUDA Gemma 4 runtime from {}: {error}",
                            config.model_path.display()
                        )
                    })?,
            ),
            DenseBenchBackend::Auto | DenseBenchBackend::Distributed => unreachable!(),
        },
        DenseBenchBackend::Distributed => DenseRuntime::Distributed(
            DistributedGemma4TextGenerationService::from_gguf_path(
                config.model_path.as_path(),
                DistributedGemma4PeerConfig {
                    peer_base_url: config
                        .peer_base_url
                        .clone()
                        .expect("distributed dense benchmark requires peer"),
                    split_layer,
                    shared_key: None,
                },
            )
            .map_err(|error| {
                format!(
                    "failed to load distributed Gemma 4 runtime from {}: {error}",
                    config.model_path.display()
                )
            })?,
        ),
    };
    let load_s = load_started.elapsed().as_secs_f64();
    let model_descriptor = runtime.model_descriptor();
    let sparse_execution_observation = runtime.sparse_execution_observation();
    let mut runs = Vec::with_capacity(config.repeats);
    for run_index in 0..config.repeats {
        let mut options = GenerationOptions::greedy(config.max_output_tokens);
        options.stop_sequences = rendered.stop_sequences.clone();
        let request = match config.prompt_mode {
            PromptMode::RenderedTokens => GenerationRequest::new_tokens(
                format!("gemma4-bench-{}", run_index + 1),
                model_descriptor.clone(),
                None,
                prompt_tokens.clone(),
                options,
            ),
            PromptMode::Text => GenerationRequest::new_text(
                format!("gemma4-bench-{}", run_index + 1),
                model_descriptor.clone(),
                None,
                config.prompt.clone(),
                options,
            ),
        };
        let started = Instant::now();
        let response = runtime.generate(&request)?;
        let total_s = started.elapsed().as_secs_f64();
        runs.push(run_report_from_generation(run_index, &response, total_s));
    }
    Ok(finish_report(
        config,
        runtime.model_id().to_string(),
        backend.runtime_label().to_string(),
        sparse,
        sparse_execution_observation,
        load_s,
        match backend {
            DenseBenchBackend::Distributed => Some(String::from("pipeline_sharded")),
            _ => None,
        },
        Some(split_layer),
        runs,
    ))
}

fn validate_single_benchmark_backend(
    model_id: &str,
    sparse: bool,
    backend: DenseBenchBackend,
) -> Result<DenseBenchBackend, String> {
    let resolved = backend.resolve_single();
    if sparse && !matches!(resolved, DenseBenchBackend::Metal | DenseBenchBackend::Cuda) {
        return Err(format!(
            "model `{model_id}` declares sparse expert topology and single-node benchmarking is currently admitted only on metal or cuda; use --backend metal or --backend cuda, or use --mode distributed-sparse for the admitted clustered sparse lane",
        ));
    }
    Ok(resolved)
}

async fn run_sparse_benchmark(config: &BenchConfig) -> Result<BenchReport, String> {
    let backend = config.backend.resolve_sparse()?;
    let adapter = GgufDecoderAdapterLoader
        .inspect_path(config.model_path.as_path())
        .map_err(|error| {
            format!(
                "failed to inspect sparse GGUF at {}: {error}",
                config.model_path.display()
            )
        })?;
    let topology = adapter
        .family_metadata()
        .expert_topology_requirements()
        .ok_or_else(|| {
            format!(
                "model `{}` does not declare sparse expert topology; use single or distributed-dense mode instead",
                adapter.descriptor().model.model_id
            )
        })?;
    let canonical_sparse_model = adapter.descriptor().model.model_id == "gemma4:26b"
        || adapter.family_metadata().family == GgufDecoderFamily::Gemma4;
    if !canonical_sparse_model {
        return Err(format!(
            "distributed-sparse mode currently expects model id `gemma4:26b`, got `{}`",
            adapter.descriptor().model.model_id
        ));
    }
    let served_artifact_digest = adapter
        .descriptor()
        .weights
        .primary_artifact_digest()
        .unwrap_or(adapter.descriptor().weights.digest.as_str())
        .to_string();
    let served_artifact_digest = psionic_serve::gguf_served_artifact_digest(
        config.model_path.as_path(),
        backend.openai_backend(),
    )
    .unwrap_or(served_artifact_digest);
    let load_started = Instant::now();
    let mut server_config = OpenAiCompatConfig::new(config.model_path.as_path());
    server_config.backend = backend.openai_backend();
    server_config
        .admit_gemma4_26b_sparse_distributed_lane(
            &sample_sparse_cluster_state(backend.runtime_label(), served_artifact_digest.as_str()),
            &Gemma4MoeDistributedLaneRequest::new(
                NodeId::new("scheduler"),
                sample_gemma4_26b_sparse_inventory(
                    backend.runtime_label(),
                    served_artifact_digest.as_str(),
                    topology.expert_count,
                ),
            )
            .with_minimum_free_memory_bytes_per_host(16 * 1024 * 1024 * 1024),
        )
        .map_err(|error| format!("failed to admit sparse Gemma 4 distributed lane: {error}"))?;
    let server = OpenAiCompatServer::from_config(&server_config)
        .map_err(|error| format!("failed to start sparse Gemma 4 server state: {error}"))?;
    let (base_url, shutdown_tx) = start_openai_compat_server(server)
        .await
        .map_err(|error| format!("failed to bind sparse Gemma 4 benchmark server: {error}"))?;
    let load_s = load_started.elapsed().as_secs_f64();

    let client = reqwest::Client::new();
    let models = client
        .get(format!("{base_url}/v1/models"))
        .send()
        .await
        .map_err(|error| format!("failed to query sparse Gemma 4 model list: {error}"))?
        .error_for_status()
        .map_err(|error| format!("sparse Gemma 4 model list failed: {error}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("failed to decode sparse Gemma 4 model list: {error}"))?;
    let model_id = models["data"]
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(|row| row["id"].as_str())
        .map(str::to_string)
        .ok_or_else(|| String::from("sparse Gemma 4 server did not publish any model ids"))?;

    let mut runs = Vec::with_capacity(config.repeats);
    for run_index in 0..config.repeats {
        let started = Instant::now();
        let response = client
            .post(format!("{base_url}/v1/chat/completions"))
            .json(&json!({
                "model": model_id,
                "messages": [{"role":"user","content": config.prompt}],
                "temperature": 0.0,
                "max_tokens": config.max_output_tokens,
                "stream": false
            }))
            .send()
            .await
            .map_err(|error| format!("sparse Gemma 4 request failed: {error}"))?;
        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|error| format!("<failed to read error body: {error}>"));
            return Err(format!(
                "sparse Gemma 4 request failed with {status}: {body}"
            ));
        }
        let total_s = started.elapsed().as_secs_f64();
        let payload = response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| format!("failed to decode sparse Gemma 4 response: {error}"))?;
        let metrics = serde_json::from_value::<GenerationMetrics>(
            payload
                .get("psionic_metrics")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )
        .ok();
        let output_text = payload["choices"]
            .get(0)
            .and_then(|choice| choice["message"]["content"].as_str())
            .unwrap_or_default()
            .to_string();
        let termination = payload["choices"]
            .get(0)
            .and_then(|choice| choice["finish_reason"].as_str())
            .unwrap_or("unknown")
            .to_string();
        let output_tokens = payload["usage"]["completion_tokens"]
            .as_u64()
            .unwrap_or_default() as usize;
        runs.push(BenchRunReport {
            run_index: run_index + 1,
            output_tokens,
            output_token_ids: Vec::new(),
            prompt_eval: metrics
                .as_ref()
                .and_then(|metrics| metrics.prompt_eval_duration_ns)
                .map(ns_to_s),
            decode: metrics
                .as_ref()
                .and_then(|metrics| metrics.eval_duration_ns)
                .map(ns_to_s),
            total: total_s,
            state_readback_bytes: metrics
                .as_ref()
                .and_then(|metrics| metrics.gemma4_metal_decode.as_ref())
                .map(|metrics| metrics.readback_bytes),
            total_s,
            prompt_s: metrics
                .as_ref()
                .and_then(|metrics| metrics.prompt_eval_duration_ns)
                .map(ns_to_s),
            decode_s: metrics
                .as_ref()
                .and_then(|metrics| metrics.eval_duration_ns)
                .map(ns_to_s),
            ttft_s: metrics
                .as_ref()
                .and_then(|metrics| metrics.time_to_first_token_ns)
                .map(ns_to_s),
            decode_tok_s: metrics
                .as_ref()
                .and_then(|metrics| metrics.eval_duration_ns)
                .and_then(|decode_ns| tokens_per_second(output_tokens, decode_ns)),
            gemma4_metal_decode_output_modes: metrics
                .as_ref()
                .and_then(|metrics| metrics.gemma4_metal_decode.as_ref())
                .map(|metrics| {
                    metrics
                        .output_modes
                        .iter()
                        .map(gemma4_metal_decode_mode_label)
                        .collect()
                }),
            gemma4_metal_decode_readback_bytes: metrics
                .as_ref()
                .and_then(|metrics| metrics.gemma4_metal_decode.as_ref())
                .map(|metrics| metrics.readback_bytes),
            gemma4_metal_decode_readback_bytes_per_token: metrics
                .as_ref()
                .and_then(|metrics| metrics.gemma4_metal_decode.as_ref())
                .and_then(|metrics| {
                    if metrics.step_count == 0 {
                        None
                    } else {
                        Some(metrics.readback_bytes as f64 / metrics.step_count as f64)
                    }
                }),
            gemma4_metal_host_kv_materialization_events: metrics
                .as_ref()
                .and_then(|metrics| metrics.gemma4_metal_decode.as_ref())
                .map(|metrics| metrics.host_kv_materialization_events),
            gemma4_metal_host_kv_materialization_tokens: metrics
                .as_ref()
                .and_then(|metrics| metrics.gemma4_metal_decode.as_ref())
                .map(|metrics| metrics.host_kv_materialization_tokens),
            termination,
            output_text,
        });
    }
    let _ = shutdown_tx.send(());
    Ok(finish_report(
        config,
        adapter.descriptor().model.model_id.clone(),
        format!(
            "{}/experts={}x{}",
            backend.runtime_label(),
            topology.expert_count,
            topology.active_expert_count.unwrap_or(0)
        ),
        true,
        None,
        load_s,
        Some(String::from("tensor_sharded")),
        None,
        runs,
    ))
}

fn finish_report(
    config: &BenchConfig,
    model_id: String,
    runtime_backend: String,
    sparse_expert_topology: bool,
    sparse_execution_observation: Option<Gemma4SparseExecutionObservation>,
    load_s: f64,
    cluster_topology: Option<String>,
    split_layer: Option<usize>,
    runs: Vec<BenchRunReport>,
) -> BenchReport {
    let mean_output_tokens = mean(runs.iter().map(|run| run.output_tokens as f64));
    let prompt_eval = mean_option(runs.iter().map(|run| run.prompt_eval));
    let decode = mean_option(runs.iter().map(|run| run.decode));
    let mean_total_s = mean(runs.iter().map(|run| run.total_s));
    let mean_ttft_s = mean_option(runs.iter().map(|run| run.ttft_s));
    let mean_decode_tok_s = mean_option(runs.iter().map(|run| run.decode_tok_s));
    let state_readback_bytes = mean_u64_option(runs.iter().map(|run| run.state_readback_bytes));
    let mean_gemma4_metal_decode_readback_bytes_per_token = mean_option(
        runs.iter()
            .map(|run| run.gemma4_metal_decode_readback_bytes_per_token),
    );
    BenchReport {
        schema_version: 1,
        report_kind: String::from("psionic_gemma4_bench"),
        mode: config.mode.label().to_string(),
        model_id,
        model_path: config.model_path.display().to_string(),
        model_artifact: model_artifact(config.model_path.as_path()),
        benchmark_prompt_id: config.benchmark_prompt_id.clone(),
        runtime_backend,
        sparse_expert_topology,
        sparse_layer_count: sparse_execution_observation
            .as_ref()
            .map(|value| value.sparse_layer_count),
        host_fallback_layer_count: sparse_execution_observation
            .as_ref()
            .map(|value| value.host_fallback_layer_count),
        sparse_ffn_backend: sparse_execution_observation
            .as_ref()
            .map(|value| value.sparse_ffn_backend.clone()),
        router_backend: sparse_execution_observation
            .as_ref()
            .map(|value| value.router_backend.clone()),
        expert_dispatch_backend: sparse_execution_observation
            .as_ref()
            .map(|value| value.expert_dispatch_backend.clone()),
        native_sparse_execution: sparse_execution_observation
            .as_ref()
            .map(|value| value.native_sparse_execution),
        host_fallback_observed: sparse_execution_observation
            .as_ref()
            .map(|value| value.host_fallback_observed),
        peer_base_url: config.peer_base_url.clone(),
        split_layer,
        prompt: config.prompt.clone(),
        prompt_mode: config.prompt_mode.label().to_string(),
        max_output_tokens: config.max_output_tokens,
        repeats: config.repeats,
        load_s,
        prompt_eval,
        decode,
        total: mean_total_s,
        decode_tok_s: mean_decode_tok_s,
        ttft: mean_ttft_s,
        state_readback_bytes,
        state_readback_bytes_per_token: mean_gemma4_metal_decode_readback_bytes_per_token,
        cluster_topology,
        runs,
        mean_output_tokens,
        mean_total_s,
        mean_ttft_s,
        mean_decode_tok_s,
        mean_gemma4_metal_decode_readback_bytes_per_token,
    }
}

fn run_report_from_generation(
    run_index: usize,
    response: &GenerationResponse,
    total_s: f64,
) -> BenchRunReport {
    let gemma4_metal_decode = response.metrics.gemma4_metal_decode.as_ref();
    BenchRunReport {
        run_index: run_index + 1,
        output_tokens: response.usage.output_tokens,
        output_token_ids: response
            .output
            .tokens
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        prompt_eval: response.metrics.prompt_eval_duration_ns.map(ns_to_s),
        decode: response.metrics.eval_duration_ns.map(ns_to_s),
        total: total_s,
        state_readback_bytes: gemma4_metal_decode.map(|metrics| metrics.readback_bytes),
        total_s,
        prompt_s: response.metrics.prompt_eval_duration_ns.map(ns_to_s),
        decode_s: response.metrics.eval_duration_ns.map(ns_to_s),
        ttft_s: response.metrics.time_to_first_token_ns.map(ns_to_s),
        decode_tok_s: response
            .metrics
            .eval_duration_ns
            .and_then(|decode_ns| tokens_per_second(response.usage.output_tokens, decode_ns)),
        gemma4_metal_decode_output_modes: gemma4_metal_decode.map(|metrics| {
            metrics
                .output_modes
                .iter()
                .map(gemma4_metal_decode_mode_label)
                .collect()
        }),
        gemma4_metal_decode_readback_bytes: gemma4_metal_decode
            .map(|metrics| metrics.readback_bytes),
        gemma4_metal_decode_readback_bytes_per_token: gemma4_metal_decode.and_then(|metrics| {
            if metrics.step_count == 0 {
                None
            } else {
                Some(metrics.readback_bytes as f64 / metrics.step_count as f64)
            }
        }),
        gemma4_metal_host_kv_materialization_events: gemma4_metal_decode
            .map(|metrics| metrics.host_kv_materialization_events),
        gemma4_metal_host_kv_materialization_tokens: gemma4_metal_decode
            .map(|metrics| metrics.host_kv_materialization_tokens),
        termination: termination_label(response.termination).to_string(),
        output_text: response.output.text.clone(),
    }
}

fn gemma4_metal_decode_mode_label(mode: &psionic_serve::Gemma4MetalDecodeOutputMode) -> String {
    match mode {
        psionic_serve::Gemma4MetalDecodeOutputMode::GreedyToken => String::from("greedy_token"),
        psionic_serve::Gemma4MetalDecodeOutputMode::TopKCandidates { top_k } => {
            format!("top_k_candidates:{top_k}")
        }
        psionic_serve::Gemma4MetalDecodeOutputMode::RawLogits => String::from("raw_logits"),
    }
}

fn mean<I>(values: I) -> f64
where
    I: IntoIterator<Item = f64>,
{
    let values: Vec<f64> = values.into_iter().collect();
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn mean_option<I>(values: I) -> Option<f64>
where
    I: IntoIterator<Item = Option<f64>>,
{
    let values: Vec<f64> = values.into_iter().flatten().collect();
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

fn mean_u64_option<I>(values: I) -> Option<u64>
where
    I: IntoIterator<Item = Option<u64>>,
{
    let values: Vec<u64> = values.into_iter().flatten().collect();
    if values.is_empty() {
        return None;
    }
    Some((values.iter().sum::<u64>() as f64 / values.len() as f64).round() as u64)
}

fn ns_to_s(value: u64) -> f64 {
    Duration::from_nanos(value).as_secs_f64()
}

fn tokens_per_second(tokens: usize, duration_ns: u64) -> Option<f64> {
    if tokens == 0 || duration_ns == 0 {
        return None;
    }
    Some(tokens as f64 / ns_to_s(duration_ns))
}

fn termination_label(reason: TerminationReason) -> &'static str {
    match reason {
        TerminationReason::EndOfSequence => "eos",
        TerminationReason::MaxOutputTokens => "max_output_tokens",
        TerminationReason::ContextLimit => "context_limit",
        TerminationReason::Cancelled => "cancelled",
        TerminationReason::Disconnected => "disconnected",
        TerminationReason::Error => "error",
    }
}

fn model_artifact(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn render_human_report(report: &BenchReport) -> String {
    let mut lines = vec![
        format!("model: {}", report.model_id),
        format!("mode: {}", report.mode),
        format!("backend: {}", report.runtime_backend),
        format!("path: {}", report.model_path),
        format!("artifact: {}", report.model_artifact),
        format!("prompt id: {}", report.benchmark_prompt_id),
        format!("load: {:.3}s", report.load_s),
        format!("repeats: {}", report.repeats),
    ];
    if let Some(sparse_ffn_backend) = report.sparse_ffn_backend.as_deref() {
        lines.push(format!("sparse ffn backend: {sparse_ffn_backend}"));
    }
    if let Some(router_backend) = report.router_backend.as_deref() {
        lines.push(format!("router backend: {router_backend}"));
    }
    if let Some(expert_dispatch_backend) = report.expert_dispatch_backend.as_deref() {
        lines.push(format!(
            "expert dispatch backend: {expert_dispatch_backend}"
        ));
    }
    if let Some(host_fallback_observed) = report.host_fallback_observed {
        lines.push(format!("host fallback observed: {host_fallback_observed}"));
    }
    if let Some(peer_base_url) = report.peer_base_url.as_deref() {
        lines.push(format!("peer: {peer_base_url}"));
    }
    if let Some(split_layer) = report.split_layer {
        lines.push(format!("split layer: {split_layer}"));
    }
    if let Some(cluster_topology) = report.cluster_topology.as_deref() {
        lines.push(format!("topology: {cluster_topology}"));
    }
    lines.push(String::new());
    for run in &report.runs {
        let ttft = run
            .ttft_s
            .map(|value| format!("{value:.3}s"))
            .unwrap_or_else(|| String::from("n/a"));
        let tok_s = run
            .decode_tok_s
            .map(|value| format!("{value:.2} tok/s"))
            .unwrap_or_else(|| String::from("n/a"));
        let readback = run
            .gemma4_metal_decode_readback_bytes_per_token
            .map(|value| format!("{value:.1} B/token"))
            .unwrap_or_else(|| String::from("n/a"));
        lines.push(format!(
            "run {}: total {:.3}s ttft {} tok/s {} readback {} output_tokens {} termination {}",
            run.run_index, run.total_s, ttft, tok_s, readback, run.output_tokens, run.termination
        ));
    }
    lines.push(String::new());
    if let Some(prompt_eval) = report.prompt_eval {
        lines.push(format!("prompt eval: {:.3}s", prompt_eval));
    }
    if let Some(decode) = report.decode {
        lines.push(format!("decode: {:.3}s", decode));
    }
    lines.push(format!("total: {:.3}s", report.total));
    if let Some(decode_tok_s) = report.decode_tok_s {
        lines.push(format!("decode tok/s: {:.2}", decode_tok_s));
    }
    if let Some(ttft) = report.ttft {
        lines.push(format!("ttft: {:.3}s", ttft));
    }
    if let Some(state_readback_bytes) = report.state_readback_bytes {
        lines.push(format!("state readback bytes: {state_readback_bytes}"));
    }
    if let Some(state_readback_bytes_per_token) = report.state_readback_bytes_per_token {
        lines.push(format!(
            "state readback bytes/token: {:.1}",
            state_readback_bytes_per_token
        ));
    }
    lines.push(format!("mean total: {:.3}s", report.mean_total_s));
    if let Some(mean_ttft_s) = report.mean_ttft_s {
        lines.push(format!("mean ttft: {:.3}s", mean_ttft_s));
    }
    if let Some(mean_decode_tok_s) = report.mean_decode_tok_s {
        lines.push(format!("mean tok/s: {:.2}", mean_decode_tok_s));
    }
    if let Some(mean_readback) = report.mean_gemma4_metal_decode_readback_bytes_per_token {
        lines.push(format!("mean readback: {:.1} B/token", mean_readback));
    }
    lines.join("\n")
}

async fn start_openai_compat_server(
    server: OpenAiCompatServer,
) -> Result<(String, oneshot::Sender<()>), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let base_url = format!("http://{address}");
    let router = server.router();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
    });
    let client = reqwest::Client::new();
    for _ in 0..50 {
        if client
            .get(format!("{base_url}/health"))
            .send()
            .await
            .is_ok()
        {
            return Ok((base_url, shutdown_tx));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(format!("OpenAI-compatible Gemma server did not become ready at {base_url}").into())
}

fn sample_sparse_cluster_id() -> ClusterId {
    ClusterId::new(
        &ClusterNamespace::new("cluster-lan"),
        &AdmissionToken::new("cluster-secret"),
    )
}

fn ready_sparse_membership(
    cluster_id: &ClusterId,
    node_id: &str,
    role: NodeRole,
) -> ClusterMembershipRecord {
    ClusterMembershipRecord::new(
        ClusterNodeIdentity {
            cluster_id: cluster_id.clone(),
            node_id: NodeId::new(node_id),
            node_epoch: NodeEpoch::initial(),
            role,
            auth_public_key: String::new(),
            attestation: None,
        },
        None,
        ClusterMembershipStatus::Ready,
    )
}

fn ready_sparse_telemetry(
    node_id: &str,
    runtime_backend: &str,
    free_memory_bytes: u64,
) -> ClusterNodeTelemetry {
    ClusterNodeTelemetry::new(NodeId::new(node_id))
        .with_memory(Some(64 * 1024 * 1024 * 1024), Some(free_memory_bytes))
        .with_cpu_logical_cores(16)
        .with_accelerator_count(1)
        .with_backend_readiness(runtime_backend, ClusterBackendReadinessStatus::Ready)
}

fn sample_sparse_cluster_state(
    runtime_backend: &str,
    served_artifact_digest: &str,
) -> ClusterState {
    let cluster_id = sample_sparse_cluster_id();
    let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
    snapshot.memberships.insert(
        NodeId::new("scheduler"),
        ready_sparse_membership(&cluster_id, "scheduler", NodeRole::Mixed),
    );
    for worker in ["worker-a", "worker-b"] {
        snapshot.memberships.insert(
            NodeId::new(worker),
            ready_sparse_membership(&cluster_id, worker, NodeRole::ExecutorOnly),
        );
        snapshot.telemetry.insert(
            NodeId::new(worker),
            ready_sparse_telemetry(worker, runtime_backend, 48 * 1024 * 1024 * 1024),
        );
        snapshot.artifact_residency.insert(
            ClusterArtifactResidencyKey::new(NodeId::new(worker), served_artifact_digest),
            ClusterArtifactResidencyRecord::new(
                NodeId::new(worker),
                ClusterArtifactReference::new("decoder", served_artifact_digest),
                ClusterArtifactResidencyStatus::Resident,
            ),
        );
    }
    ClusterState::from_snapshot(snapshot)
}

fn sample_gemma4_26b_sparse_inventory(
    runtime_backend: &str,
    served_artifact_digest: &str,
    expert_count: usize,
) -> SparseExpertHostInventorySnapshot {
    let split_point = (expert_count / 2).max(1);
    SparseExpertHostInventorySnapshot::new(
        psionic_serve::TEXT_GENERATION_PRODUCT_ID,
        "gemma4:26b",
        runtime_backend,
        served_artifact_digest,
    )
    .with_sharded_model_manifest_digest("gemma4-26b-manifest")
    .with_host(SparseExpertHostInventoryRecord::new(
        NodeId::new("worker-a"),
        0,
        split_point,
    ))
    .with_host(SparseExpertHostInventoryRecord::new(
        NodeId::new("worker-b"),
        split_point,
        expert_count,
    ))
}

fn usage() -> &'static str {
    "Gemma 4 benchmark harness.\n\
Usage: cargo run -p psionic-serve --example gemma4_bench -- \\\n\
  --model-path <gguf> [--mode single|distributed-dense|distributed-sparse] \\\n\
  [--backend auto|cpu|metal|cuda] [--peer-base-url <url>] [--split-layer <n>] \\\n\
  [--prompt-id <id>] [--prompt-mode rendered-tokens|text] [--prompt <text>] [--max-output-tokens <n>] \\\n\
  [--repeats <n>] [--json] [--json-out <path>]\n\
\n\
Notes:\n\
  - single runs one local Gemma 4 text lane on one backend; sparse `gemma4:26b` is admitted only on metal or cuda, while the CPU reference path stays dense-only.\n\
  - distributed-dense runs the Metal+CUDA split-execution lane and requires --peer-base-url.\n\
  - distributed-sparse runs the admitted Gemma 4 26B sparse lane through the generic server on the selected local backend.\n\
  - local Metal runs refuse parallel launch on an interactive Mac by default; set PSIONIC_ALLOW_PARALLEL_METAL_BENCH=1 only when you intentionally want to override that lock.\n"
}

#[cfg(test)]
mod tests {
    use super::{DenseBenchBackend, LocalMetalBenchGuard, validate_single_benchmark_backend};

    #[test]
    fn local_metal_bench_guard_releases_lock_on_drop() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let lock_path = tempdir.path().join("gemma4-metal-bench.lock");
        let guard = LocalMetalBenchGuard::acquire_at_path(lock_path.clone()).expect("first guard");
        drop(guard);
        LocalMetalBenchGuard::acquire_at_path(lock_path).expect("lock should be reusable");
    }

    #[test]
    fn local_metal_bench_guard_rejects_parallel_holder() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let lock_path = tempdir.path().join("gemma4-metal-bench.lock");
        let _guard = LocalMetalBenchGuard::acquire_at_path(lock_path.clone()).expect("first guard");
        let error =
            LocalMetalBenchGuard::acquire_at_path(lock_path).expect_err("second guard rejects");
        assert!(error.contains("refusing to start a second local Metal Gemma benchmark"));
    }

    #[test]
    fn sparse_single_benchmark_backend_requires_accelerator_lane() {
        let error = validate_single_benchmark_backend("gemma4:26b", true, DenseBenchBackend::Cpu)
            .expect_err("cpu sparse single-node lane should be refused");
        assert!(error.contains("metal or cuda"), "unexpected error: {error}");

        assert_eq!(
            validate_single_benchmark_backend("gemma4:26b", true, DenseBenchBackend::Metal)
                .expect("metal sparse single-node lane should be admitted"),
            DenseBenchBackend::Metal
        );
        assert_eq!(
            validate_single_benchmark_backend("gemma4:26b", true, DenseBenchBackend::Cuda)
                .expect("cuda sparse single-node lane should be admitted"),
            DenseBenchBackend::Cuda
        );
    }
}
