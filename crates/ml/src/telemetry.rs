use serde::Serialize;

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
