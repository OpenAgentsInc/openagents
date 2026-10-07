use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_GPU_LOGITS_SELECTION_SCHEMA_VERSION: &str =
    "psion.rvllm_gpu_logits_selection.v1";
pub const PSION_RVLLM_GPU_LOGITS_SELECTION_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_gpu_logits_selection_v1.json";
pub const PSION_RVLLM_GPU_LOGITS_SELECTION_DOC_PATH: &str =
    "docs/PSION_RVLLM_GPU_LOGITS_SELECTION.md";

const PACKET_ID: &str = "psion_rvllm_gpu_logits_selection_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmGpuSelectionLane {
    pub lane_id: String,
    pub output_mode: String,
    pub host_logits_copy_required: bool,
    pub readback_bytes: u64,
    pub raw_logits_materialized: bool,
    pub selected_token_materialized: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmGpuLogitsSelectionPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub qwen35_lanes: Vec<PsionRvllmGpuSelectionLane>,
    pub gpt_oss_lanes: Vec<PsionRvllmGpuSelectionLane>,
    pub parity_posture: String,
    pub fallback_posture: String,
    pub benchmark_paths: Vec<String>,
    pub packet_digest: String,
}

impl PsionRvllmGpuLogitsSelectionPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_gpu_logits_selection_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_gpu_logits_selection_packet() -> PsionRvllmGpuLogitsSelectionPacket {
    let mut packet = PsionRvllmGpuLogitsSelectionPacket {
        schema_version: String::from(PSION_RVLLM_GPU_LOGITS_SELECTION_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("qwen35.native_cuda_decode"),
            String::from("gpt_oss.native_cuda_decode"),
        ],
        qwen35_lanes: vec![
            PsionRvllmGpuSelectionLane {
                lane_id: String::from("qwen35.argmax_only"),
                output_mode: String::from("argmax_only"),
                host_logits_copy_required: false,
                readback_bytes: 8,
                raw_logits_materialized: false,
                selected_token_materialized: true,
            },
            PsionRvllmGpuSelectionLane {
                lane_id: String::from("qwen35.top_k_candidates_40"),
                output_mode: String::from("top_k_candidates:40"),
                host_logits_copy_required: false,
                readback_bytes: 320,
                raw_logits_materialized: false,
                selected_token_materialized: true,
            },
            PsionRvllmGpuSelectionLane {
                lane_id: String::from("qwen35.raw_logits_fallback"),
                output_mode: String::from("raw_logits"),
                host_logits_copy_required: true,
                readback_bytes: 604_160,
                raw_logits_materialized: true,
                selected_token_materialized: true,
            },
        ],
        gpt_oss_lanes: vec![
            PsionRvllmGpuSelectionLane {
                lane_id: String::from("gpt_oss.device_argmax"),
                output_mode: String::from("argmax_only"),
                host_logits_copy_required: false,
                readback_bytes: 8,
                raw_logits_materialized: false,
                selected_token_materialized: true,
            },
            PsionRvllmGpuSelectionLane {
                lane_id: String::from("gpt_oss.raw_logits_fallback"),
                output_mode: String::from("raw_logits"),
                host_logits_copy_required: true,
                readback_bytes: 645_120,
                raw_logits_materialized: true,
                selected_token_materialized: true,
            },
        ],
        parity_posture: String::from(
            "Admitted argmax-only and bounded candidate paths keep seeded token selection deterministic while avoiding dense host logits copies; fallback to raw logits stays explicit when the request leaves that envelope.",
        ),
        fallback_posture: String::from(
            "Requests outside the admitted GPU-resident selection lane still materialize raw logits explicitly instead of silently narrowing decode semantics.",
        ),
        benchmark_paths: vec![
            String::from("crates/psionic-serve/examples/qwen35_cuda_bench.rs"),
            String::from("crates/psionic-serve/src/qwen35.rs"),
            String::from("crates/psionic-serve/src/gpt_oss.rs"),
        ],
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
            .expect("psion rvllm gpu logits selection packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmGpuLogitsSelectionPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_gpu_logits_selection_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_gpu_logits_selection_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
