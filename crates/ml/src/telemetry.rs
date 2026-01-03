use serde::Serialize;
use web_time::SystemTime;

#[derive(Debug, Clone, Serialize)]
pub struct TokenCandidate {
    pub token_id: u32,
    pub token_text: String,
    pub probability: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InferenceTelemetry {
    TokenGenerated {
        token_id: u32,
        token_text: String,
        top_k: Vec<TokenCandidate>,
        entropy: f32,
        tokens_per_sec: f32,
    },
    AttentionWeights {
        layer: usize,
        head: usize,
        weights: Vec<Vec<f32>>,
    },
    LayerActivation {
        layer: usize,
        attention_norm: f32,
        mlp_norm: f32,
        output_norm: f32,
    },
    CacheStatus {
        layer: usize,
        seq_len: usize,
        max_len: usize,
        offset: usize,
        memory_bytes: usize,
    },
    MemoryUsage {
        gpu_allocated: usize,
        cache_total: usize,
        activations: usize,
    },
}

pub trait InferenceHook: Send + Sync {
    fn on_telemetry(&self, telemetry: InferenceTelemetry);
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Started,
    Progress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelLifecycleTelemetry {
    LoadStage {
        stage: String,
        status: StageStatus,
        detail: Option<String>,
        bytes: Option<u64>,
        total_bytes: Option<u64>,
        ts_ms: u64,
    },
    InferenceStage {
        stage: String,
        status: StageStatus,
        step: Option<usize>,
        total_steps: Option<usize>,
        detail: Option<String>,
        ts_ms: u64,
    },
    InferenceEvent {
        event: InferenceTelemetry,
        ts_ms: u64,
    },
}

pub trait ModelLifecycleHook: Send + Sync {
    fn on_lifecycle(&self, telemetry: ModelLifecycleTelemetry);
}

pub fn telemetry_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
