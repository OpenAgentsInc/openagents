use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_SCHEMA_VERSION: &str =
    "psion.rvllm_fa3_decode_attention_backend.v1";
pub const PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_fa3_decode_attention_backend_v1.json";
pub const PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_DOC_PATH: &str =
    "docs/PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND.md";

pub const PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_NAME: &str = "fa3_split_kv_f16_kv_graph";
pub const PSION_RVLLM_DENSE_F16_KV_GRAPH_LEGACY_BACKEND_NAME: &str = "dense_f16_kv_graph_legacy";
pub const PSION_RVLLM_DENSE_F16_KV_LEGACY_BACKEND_NAME: &str = "dense_f16_kv_legacy";
pub const PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_SPLITS: usize = 8;
pub const PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_HEADS_PER_GROUP: usize = 8;
pub const PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_HEAD_DIM: usize = 256;

const PACKET_ID: &str = "psion_rvllm_fa3_decode_attention_backend_v1";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Qwen35CudaAttentionBackendExecution {
    pub requested_backend: String,
    pub executed_backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    pub graph_capture_compatible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_capability: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Qwen35CudaAttentionBackendMetrics {
    pub layer_invocation_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub executions: Vec<Qwen35CudaAttentionBackendExecution>,
}

impl Qwen35CudaAttentionBackendMetrics {
    pub fn push(&mut self, execution: Qwen35CudaAttentionBackendExecution) {
        self.layer_invocation_count = self.layer_invocation_count.saturating_add(1);
        self.executions.push(execution);
        self.executions.sort();
        self.executions.dedup();
    }

    pub fn accumulate(&mut self, other: &Self) {
        self.layer_invocation_count = self
            .layer_invocation_count
            .saturating_add(other.layer_invocation_count);
        self.executions.extend(other.executions.iter().cloned());
        self.executions.sort();
        self.executions.dedup();
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.layer_invocation_count == 0 && self.executions.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsionRvllmCudaDeviceSurface {
    pub architecture: Option<String>,
    pub compute_capability: Option<String>,
}

impl PsionRvllmCudaDeviceSurface {
    #[must_use]
    pub fn new(architecture: Option<String>, compute_capability: Option<String>) -> Self {
        Self {
            architecture,
            compute_capability,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PsionRvllmFa3DecodeAttentionShape {
    pub use_graph_attention: bool,
    pub head_count: usize,
    pub kv_head_count: usize,
    pub head_dim: usize,
    pub sliding_window: usize,
    pub past_tokens: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsionRvllmFa3DecodeAttentionBackendKind {
    DenseF16KvLegacy,
    DenseF16KvGraphLegacy,
    Fa3SplitKvF16KvGraph,
}

impl PsionRvllmFa3DecodeAttentionBackendKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DenseF16KvLegacy => PSION_RVLLM_DENSE_F16_KV_LEGACY_BACKEND_NAME,
            Self::DenseF16KvGraphLegacy => PSION_RVLLM_DENSE_F16_KV_GRAPH_LEGACY_BACKEND_NAME,
            Self::Fa3SplitKvF16KvGraph => PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_NAME,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsionRvllmFa3DecodeAttentionSelection {
    pub requested_backend: PsionRvllmFa3DecodeAttentionBackendKind,
    pub executed_backend: PsionRvllmFa3DecodeAttentionBackendKind,
    pub fallback_reason: Option<String>,
    pub graph_capture_compatible: bool,
    pub split_count: Option<usize>,
    pub architecture: Option<String>,
    pub compute_capability: Option<String>,
}

impl PsionRvllmFa3DecodeAttentionSelection {
    #[must_use]
    pub fn execution(&self) -> Qwen35CudaAttentionBackendExecution {
        Qwen35CudaAttentionBackendExecution {
            requested_backend: String::from(self.requested_backend.as_str()),
            executed_backend: String::from(self.executed_backend.as_str()),
            fallback_reason: self.fallback_reason.clone(),
            graph_capture_compatible: self.graph_capture_compatible,
            split_count: self.split_count,
            architecture: self.architecture.clone(),
            compute_capability: self.compute_capability.clone(),
        }
    }
}

#[must_use]
pub fn psion_rvllm_fa3_decode_attention_split_count(
    sliding_window: usize,
    past_tokens: usize,
) -> usize {
    let context_tokens = if sliding_window > 0 {
        past_tokens.min(sliding_window)
    } else {
        past_tokens
    }
    .saturating_add(1);
    if context_tokens <= 512 {
        1
    } else if context_tokens <= 2_048 {
        2
    } else if context_tokens <= 8_192 {
        4
    } else {
        PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_SPLITS
    }
}

#[must_use]
pub fn select_psion_rvllm_fa3_decode_attention_backend(
    device: &PsionRvllmCudaDeviceSurface,
    shape: PsionRvllmFa3DecodeAttentionShape,
) -> PsionRvllmFa3DecodeAttentionSelection {
    let architecture = device.architecture.clone();
    let compute_capability = device.compute_capability.clone();
    if !shape.use_graph_attention {
        return PsionRvllmFa3DecodeAttentionSelection {
            requested_backend: PsionRvllmFa3DecodeAttentionBackendKind::DenseF16KvLegacy,
            executed_backend: PsionRvllmFa3DecodeAttentionBackendKind::DenseF16KvLegacy,
            fallback_reason: None,
            graph_capture_compatible: false,
            split_count: None,
            architecture,
            compute_capability,
        };
    }

    let split_count =
        psion_rvllm_fa3_decode_attention_split_count(shape.sliding_window, shape.past_tokens);
    let fallback_reason = if shape.head_count == 0 || shape.kv_head_count == 0 {
        Some(String::from("attention_head_shape_missing"))
    } else if shape.head_count % shape.kv_head_count != 0 {
        Some(String::from("head_count_not_divisible_by_kv_head_count"))
    } else if shape.head_count / shape.kv_head_count
        > PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_HEADS_PER_GROUP
    {
        Some(format!(
            "heads_per_group_exceeds_{}",
            PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_HEADS_PER_GROUP
        ))
    } else if shape.head_dim == 0 || shape.head_dim > PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_HEAD_DIM
    {
        Some(format!(
            "head_dim_exceeds_{}",
            PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_HEAD_DIM
        ))
    } else if !cuda_device_supports_fa3_decode(device) {
        Some(String::from("sm80_plus_required"))
    } else {
        None
    };

    if let Some(reason) = fallback_reason {
        return PsionRvllmFa3DecodeAttentionSelection {
            requested_backend: PsionRvllmFa3DecodeAttentionBackendKind::Fa3SplitKvF16KvGraph,
            executed_backend: PsionRvllmFa3DecodeAttentionBackendKind::DenseF16KvGraphLegacy,
            fallback_reason: Some(reason),
            graph_capture_compatible: true,
            split_count: None,
            architecture,
            compute_capability,
        };
    }

    PsionRvllmFa3DecodeAttentionSelection {
        requested_backend: PsionRvllmFa3DecodeAttentionBackendKind::Fa3SplitKvF16KvGraph,
        executed_backend: PsionRvllmFa3DecodeAttentionBackendKind::Fa3SplitKvF16KvGraph,
        fallback_reason: None,
        graph_capture_compatible: true,
        split_count: Some(split_count),
        architecture,
        compute_capability,
    }
}

fn cuda_device_supports_fa3_decode(device: &PsionRvllmCudaDeviceSurface) -> bool {
    device
        .compute_capability
        .as_deref()
        .and_then(parse_compute_capability)
        .map_or_else(
            || {
                device
                    .architecture
                    .as_deref()
                    .map_or(false, |architecture| {
                        matches!(architecture, "Ada Lovelace" | "Hopper" | "Blackwell")
                    })
            },
            |(major, _minor)| major >= 8,
        )
}

fn parse_compute_capability(value: &str) -> Option<(u32, u32)> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let mut segments = value.split('.');
    let major = segments.next()?.parse::<u32>().ok()?;
    let minor = segments
        .next()
        .and_then(|segment| segment.parse::<u32>().ok())
        .unwrap_or(0);
    Some((major, minor))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmFa3DecodeAttentionBackendPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub admitted_lane: String,
    pub requested_backend: String,
    pub graph_fallback_backend: String,
    pub non_graph_backend: String,
    pub architecture_gate: Vec<String>,
    pub split_kv_heuristic: Vec<String>,
    pub required_evidence_fields: Vec<String>,
    pub validation_surface: Vec<String>,
    pub stability_posture: String,
    pub packet_digest: String,
}

impl PsionRvllmFa3DecodeAttentionBackendPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(
            b"psion_rvllm_fa3_decode_attention_backend_packet|",
            &canonical,
        )
    }
}

#[must_use]
pub fn builtin_psion_rvllm_fa3_decode_attention_backend_packet()
-> PsionRvllmFa3DecodeAttentionBackendPacket {
    let mut packet = PsionRvllmFa3DecodeAttentionBackendPacket {
        schema_version: String::from(PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("qwen35.native_cuda_decode"),
            String::from("qwen35.cuda_graph_replay"),
            String::from("psionic_backend_cuda.attention_decode_kernels"),
        ],
        admitted_lane: String::from("qwen35.greedy_cuda_graph_decode"),
        requested_backend: String::from(PSION_RVLLM_FA3_DECODE_ATTENTION_BACKEND_NAME),
        graph_fallback_backend: String::from(PSION_RVLLM_DENSE_F16_KV_GRAPH_LEGACY_BACKEND_NAME),
        non_graph_backend: String::from(PSION_RVLLM_DENSE_F16_KV_LEGACY_BACKEND_NAME),
        architecture_gate: vec![
            String::from("cuda_compute_capability>=8.0"),
            String::from("head_count%kv_head_count==0"),
            format!(
                "heads_per_group<={}",
                PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_HEADS_PER_GROUP
            ),
            format!(
                "head_dim<={}",
                PSION_RVLLM_FA3_DECODE_ATTENTION_MAX_HEAD_DIM
            ),
        ],
        split_kv_heuristic: vec![
            String::from("ctx<=512 => 1 split"),
            String::from("ctx<=2048 => 2 splits"),
            String::from("ctx<=8192 => 4 splits"),
            String::from("ctx>8192 => 8 splits"),
        ],
        required_evidence_fields: vec![
            String::from("metrics.qwen35_cuda_decode.attention_backend.layer_invocation_count"),
            String::from(
                "metrics.qwen35_cuda_decode.attention_backend.executions[].requested_backend",
            ),
            String::from(
                "metrics.qwen35_cuda_decode.attention_backend.executions[].executed_backend",
            ),
            String::from(
                "metrics.qwen35_cuda_decode.attention_backend.executions[].fallback_reason",
            ),
            String::from("metrics.qwen35_cuda_decode.attention_backend.executions[].split_count"),
            String::from(
                "metrics.qwen35_cuda_decode.attention_backend.executions[].compute_capability",
            ),
            String::from("bench.runs[].qwen35_attention_backends"),
        ],
        validation_surface: vec![
            String::from("builtin_packet_matches_committed_fixture"),
            String::from(
                "cuda_submission_fused_attention_graph_fa3_f16_kv_matches_legacy_reference_when_available",
            ),
            String::from("cargo build -p psionic-serve --example qwen35_cuda_bench"),
            String::from("cargo build -p psionic-serve --bin psionic-openai-server"),
        ],
        stability_posture: String::from(
            "Psionic now exposes one explicit FA3-class decode-attention lane for native qwen35 CUDA graph decode. Runtime receipts say whether the request asked for the FA3 split-KV graph backend, whether the kernel actually executed, which split heuristic fired, and why the runtime stayed on the legacy graph kernel or the non-graph dense path instead of silently downgrading.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = packet.stable_digest();
    packet
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("psion rvllm fa3 decode attention backend packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supported_device() -> PsionRvllmCudaDeviceSurface {
        PsionRvllmCudaDeviceSurface::new(
            Some(String::from("Ada Lovelace")),
            Some(String::from("8.9")),
        )
    }

    #[test]
    fn split_count_matches_thresholds() {
        assert_eq!(psion_rvllm_fa3_decode_attention_split_count(0, 0), 1);
        assert_eq!(psion_rvllm_fa3_decode_attention_split_count(0, 511), 1);
        assert_eq!(psion_rvllm_fa3_decode_attention_split_count(0, 512), 2);
        assert_eq!(psion_rvllm_fa3_decode_attention_split_count(0, 2_048), 4);
        assert_eq!(psion_rvllm_fa3_decode_attention_split_count(0, 8_192), 8);
    }

    #[test]
    fn selector_uses_fa3_backend_on_supported_graph_lane() {
        let selection = select_psion_rvllm_fa3_decode_attention_backend(
            &supported_device(),
            PsionRvllmFa3DecodeAttentionShape {
                use_graph_attention: true,
                head_count: 28,
                kv_head_count: 4,
                head_dim: 128,
                sliding_window: 0,
                past_tokens: 768,
            },
        );
        assert_eq!(
            selection.executed_backend,
            PsionRvllmFa3DecodeAttentionBackendKind::Fa3SplitKvF16KvGraph
        );
        assert_eq!(selection.split_count, Some(2));
        assert_eq!(selection.fallback_reason, None);
    }

    #[test]
    fn selector_refuses_fa3_backend_when_device_is_too_old() {
        let selection = select_psion_rvllm_fa3_decode_attention_backend(
            &PsionRvllmCudaDeviceSurface::new(
                Some(String::from("Turing")),
                Some(String::from("7.5")),
            ),
            PsionRvllmFa3DecodeAttentionShape {
                use_graph_attention: true,
                head_count: 28,
                kv_head_count: 4,
                head_dim: 128,
                sliding_window: 0,
                past_tokens: 1_024,
            },
        );
        assert_eq!(
            selection.executed_backend,
            PsionRvllmFa3DecodeAttentionBackendKind::DenseF16KvGraphLegacy
        );
        assert_eq!(
            selection.fallback_reason.as_deref(),
            Some("sm80_plus_required")
        );
        assert_eq!(selection.split_count, None);
    }

    #[test]
    fn selector_uses_non_graph_backend_when_graph_decode_is_not_requested() {
        let selection = select_psion_rvllm_fa3_decode_attention_backend(
            &supported_device(),
            PsionRvllmFa3DecodeAttentionShape {
                use_graph_attention: false,
                head_count: 28,
                kv_head_count: 4,
                head_dim: 128,
                sliding_window: 0,
                past_tokens: 1_024,
            },
        );
        assert_eq!(
            selection.executed_backend,
            PsionRvllmFa3DecodeAttentionBackendKind::DenseF16KvLegacy
        );
        assert_eq!(selection.fallback_reason, None);
        assert_eq!(selection.split_count, None);
    }

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmFa3DecodeAttentionBackendPacket =
            serde_json::from_str(include_str!(
                "../../../fixtures/psion/serve/psion_rvllm_fa3_decode_attention_backend_v1.json"
            ))?;
        let packet = builtin_psion_rvllm_fa3_decode_attention_backend_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
