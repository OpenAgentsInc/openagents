const DEFAULT_METADATA_BYTES: u64 = 16 * 1024 * 1024;
const MAX_METADATA_ATTEMPTS: usize = 3;
const LOAD_CHUNK_BYTES: u64 = 8 * 1024 * 1024;
const PROGRESS_STEP_BYTES: u64 = 64 * 1024 * 1024;
const LOCAL_GGUF_ROUTE: &str = "/gpt-oss-20b-Q8_0.gguf";
const LOCAL_GGUF_URL: &str = "http://localhost:8080/gpt-oss-20b-Q8_0.gguf";
const LOCAL_GGUF_DEV_URL: &str = "http://localhost:3000/gpt-oss-20b-Q8_0.gguf";
const LOCAL_GGUF_PATH: &str = "crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf";
const LOCAL_GGUF_SERVE_CMD: &str =
    "cargo run -p ml --bin gguf_serve -- crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf";
const PYLON_API_URL: &str = "http://127.0.0.1:9899";
const PYLON_SOURCE_LABEL: &str = "pylon://127.0.0.1:9899";
const CURRENT_DATE: &str = "2026-01-04";
const DEFAULT_USER_PROMPT: &str = "Give me one sentence about what GPT-OSS can do.";
const DEFAULT_DEVELOPER_PROMPT: &str = "";
const DEFAULT_MAX_NEW_TOKENS: usize = 8;
const DEFAULT_MAX_KV_TOKENS: usize = 32;
const DEFAULT_KV_BUDGET_BYTES: u64 = 6 * 1024 * 1024 * 1024;
const DEFAULT_SAMPLE_TOP_K: usize = 40;
const DEFAULT_SAMPLE_TEMP: f32 = 1.0;
const DEFAULT_SAMPLE_TOP_P: f32 = 1.0;
const Q8_0_BLOCK_BYTES: usize = 34;
const Q8_0_BLOCK_VALUES: usize = 32;
const MXFP4_BLOCK_BYTES: usize = 17;
const MXFP4_BLOCK_VALUES: usize = 32;
// GGML MXFP4 uses doubled E2M1 values with a half-scaled exponent.
const MXFP4_VALUES: [f32; 16] = [
    0.0, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, -0.0, -1.0, -2.0, -3.0, -4.0, -6.0, -8.0,
    -12.0,
];
const SWIGLU_ALPHA: f32 = 1.702;
const SWIGLU_LIMIT: f32 = 7.0;
const PROBE_TOLERANCE: f32 = 1e-3;
const ROPE_NTK_ALPHA: f32 = 1.0;
const ROPE_NTK_BETA: f32 = 32.0;
const TENSOR_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;
const TENSOR_CACHE_MAX_ENTRY_BYTES: usize = 4 * 1024 * 1024;
const TOKEN_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;
const TOKEN_CACHE_MAX_ENTRY_BYTES: usize = 256 * 1024;
const Q8_0_CACHE_MAX_BYTES: usize = 96 * 1024 * 1024;
const Q8_0_CACHE_MAX_ENTRY_BYTES: usize = 32 * 1024 * 1024;
const EXPERT_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;
const EXPERT_CACHE_MAX_ENTRY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
struct GptOssConfig {
    block_count: u32,
    context_length: u32,
    embedding_length: u32,
    feed_forward_length: u32,
    head_count: u32,
    head_count_kv: u32,
    rope_dimension_count: u32,
    rope_theta: f32,
    rope_scaling_factor: f32,
    rope_scaling_original_context: u32,
    rms_epsilon: f32,
    sliding_window: u32,
    expert_count: u32,
    experts_per_token: u32,
}

#[derive(Clone, Copy, Debug)]
struct SamplingConfig {
    enabled: bool,
    temperature: f32,
    top_k: usize,
    top_p: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct SamplingOverrides {
    enabled: Option<bool>,
    temperature: Option<f32>,
    top_k: Option<usize>,
    top_p: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
struct KvLimit {
    max_tokens: usize,
    per_layer_max: usize,
    budget_max: Option<usize>,
    budget_bytes: u64,
}

