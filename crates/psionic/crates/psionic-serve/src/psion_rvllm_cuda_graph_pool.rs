use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    CudaGraphReplayMetrics, CudaGraphReplayMode, Qwen35CudaDecodeOutputMetrics,
    Qwen35CudaDecodeOutputMode,
};

pub const PSION_RVLLM_CUDA_GRAPH_POOL_SCHEMA_VERSION: &str = "psion.rvllm_cuda_graph_pool.v1";
pub const PSION_RVLLM_CUDA_GRAPH_POOL_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_cuda_graph_pool_v1.json";
pub const PSION_RVLLM_CUDA_GRAPH_POOL_DOC_PATH: &str = "docs/PSION_RVLLM_CUDA_GRAPH_POOL.md";

const PACKET_ID: &str = "psion_rvllm_cuda_graph_pool_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmCudaGraphPoolPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub graph_key_fields: Vec<String>,
    pub admitted_decode_modes: Vec<CudaGraphReplayMode>,
    pub qwen35_decode_metrics: Qwen35CudaDecodeOutputMetrics,
    pub gpt_oss_cuda_graph_metrics: CudaGraphReplayMetrics,
    pub benchmark_paths: Vec<String>,
    pub fallback_posture: String,
    pub detail: String,
    pub packet_digest: String,
}

impl PsionRvllmCudaGraphPoolPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_cuda_graph_pool_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_cuda_graph_pool_packet() -> PsionRvllmCudaGraphPoolPacket {
    let mut packet = PsionRvllmCudaGraphPoolPacket {
        schema_version: String::from(PSION_RVLLM_CUDA_GRAPH_POOL_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("qwen35.native_cuda_decode"),
            String::from("gpt_oss.native_cuda_decode"),
        ],
        graph_key_fields: vec![
            String::from("served_artifact_id"),
            String::from("batch_size"),
            String::from("sequence_length"),
            String::from("decode_mode"),
            String::from("kv_cache_encoding"),
            String::from("cache_allocation_identity"),
        ],
        admitted_decode_modes: vec![
            CudaGraphReplayMode::ArgmaxOnly,
            CudaGraphReplayMode::TopKCandidates { top_k: 40 },
            CudaGraphReplayMode::RawLogits,
        ],
        qwen35_decode_metrics: Qwen35CudaDecodeOutputMetrics {
            step_count: 3,
            output_modes: vec![
                Qwen35CudaDecodeOutputMode::ArgmaxOnly,
                Qwen35CudaDecodeOutputMode::TopKCandidates { top_k: 40 },
                Qwen35CudaDecodeOutputMode::RawLogits,
            ],
            readback_bytes: 8_516,
            raw_logits_materialized: true,
            graph_cache_identity: None,
            graph_replay: Some(CudaGraphReplayMetrics {
                step_count: 3,
                replay_hit_count: 1,
                replay_miss_count: 2,
                capture_count: 2,
                shape_drift_count: 1,
                refusal_count: 0,
                capture_latency_ns: 126_000,
                output_modes: vec![
                    CudaGraphReplayMode::ArgmaxOnly,
                    CudaGraphReplayMode::TopKCandidates { top_k: 40 },
                    CudaGraphReplayMode::RawLogits,
                ],
            }),
            attention_backend: None,
        },
        gpt_oss_cuda_graph_metrics: CudaGraphReplayMetrics {
            step_count: 3,
            replay_hit_count: 1,
            replay_miss_count: 1,
            capture_count: 1,
            shape_drift_count: 0,
            refusal_count: 1,
            capture_latency_ns: 54_000,
            output_modes: vec![
                CudaGraphReplayMode::ArgmaxOnly,
                CudaGraphReplayMode::RawLogits,
            ],
        },
        benchmark_paths: vec![
            String::from("crates/psionic-serve/examples/qwen35_cuda_bench.rs"),
            String::from("scripts/benchmark-gpt-oss-vs-llama.sh"),
        ],
        fallback_posture: String::from(
            "Graph replay stays explicit and admitted-path only: capture refusal, shape drift, and uncaptured fallback remain visible instead of silently pretending every decode step is graph-stable.",
        ),
        detail: String::from(
            "This packet records the first shared RVLLM harvest contract in Psionic. The runtime already had lane-local captured-graph wins, but the retained truth is now one explicit graph-pool contract with stable key fields, admitted decode modes, and machine-readable qwen35 plus GPT-OSS replay evidence without swapping runtimes or widening claim surface.",
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
        serde_json::to_vec(value).expect("psion rvllm cuda graph pool packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmCudaGraphPoolPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_cuda_graph_pool_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_cuda_graph_pool_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
