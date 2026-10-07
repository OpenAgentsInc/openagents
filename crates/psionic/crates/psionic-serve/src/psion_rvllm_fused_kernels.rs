use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_FUSED_KERNELS_SCHEMA_VERSION: &str = "psion.rvllm_fused_kernels.v1";
pub const PSION_RVLLM_FUSED_KERNELS_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_fused_kernels_v1.json";
pub const PSION_RVLLM_FUSED_KERNELS_DOC_PATH: &str = "docs/PSION_RVLLM_FUSED_KERNELS.md";

const PACKET_ID: &str = "psion_rvllm_fused_kernels_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmFusedKernelGate {
    pub env_var: String,
    pub semantics: String,
    pub admitted_paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmFusedKernelFamily {
    pub family_id: String,
    pub kernel_labels: Vec<String>,
    pub admitted_paths: Vec<String>,
    pub disable_path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionRvllmFusedKernelBenchmark {
    pub family_id: String,
    pub before_op_latency_ms: f32,
    pub after_op_latency_ms: f32,
    pub before_end_to_end_latency_ms: f32,
    pub after_end_to_end_latency_ms: f32,
    pub before_tokens_per_second: f32,
    pub after_tokens_per_second: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionRvllmFusedKernelsPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub feature_gates: Vec<PsionRvllmFusedKernelGate>,
    pub admitted_families: Vec<PsionRvllmFusedKernelFamily>,
    pub benchmark_comparison: Vec<PsionRvllmFusedKernelBenchmark>,
    pub retained_tests: Vec<String>,
    pub claim_boundary: String,
    pub packet_digest: String,
}

impl PsionRvllmFusedKernelsPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_fused_kernels_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_fused_kernels_packet() -> PsionRvllmFusedKernelsPacket {
    let mut packet = PsionRvllmFusedKernelsPacket {
        schema_version: String::from(PSION_RVLLM_FUSED_KERNELS_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("psionic_backend_cuda.quantized_fused_submission_kernels"),
            String::from("psionic_serve.qwen35_native_cuda_decode"),
            String::from("psionic_serve.gpt_oss_native_cuda_decode"),
        ],
        feature_gates: vec![
            PsionRvllmFusedKernelGate {
                env_var: String::from("PSIONIC_QWEN35_DISABLE_FUSED_QKV_RMS_NORM"),
                semantics: String::from("disable_when_present"),
                admitted_paths: vec![String::from("qwen35.native_cuda_decode")],
            },
            PsionRvllmFusedKernelGate {
                env_var: String::from("PSIONIC_GPT_OSS_EXPERIMENTAL_FUSED_SELECTED4_MOE_DOWN"),
                semantics: String::from("enable_when_true"),
                admitted_paths: vec![String::from("gpt_oss.native_cuda_decode")],
            },
        ],
        admitted_families: vec![
            PsionRvllmFusedKernelFamily {
                family_id: String::from("qwen35_qkv_rms_norm"),
                kernel_labels: vec![
                    String::from("split_interleaved_query_gate_rms_norm_f32"),
                    String::from("pack_qwen35_key_value_rms_norm_f32"),
                    String::from("pack_qwen35_hybrid_qkv_rms_norm_f32"),
                ],
                admitted_paths: vec![
                    String::from("qwen35.native_cuda_decode"),
                    String::from("qwen35.native_cuda_hybrid_decode"),
                ],
                disable_path: String::from(
                    "falls back to split_interleaved_query_gate_f32 plus rms_norm[_region] and explicit copy_buffer_region sequencing",
                ),
            },
            PsionRvllmFusedKernelFamily {
                family_id: String::from("gpt_oss_selected4_moe"),
                kernel_labels: vec![
                    String::from("expert_gate_up_swiglu_q8_1_ids"),
                    String::from("moe_down_aggregate_q8_1_f32"),
                ],
                admitted_paths: vec![String::from("gpt_oss.native_cuda_decode")],
                disable_path: String::from(
                    "falls back to moe_gate_up_swiglu_q8_1 plus expert_matvec_q8_1_ids or moe_down_aggregate_q8_1 depending on selected-count posture",
                ),
            },
        ],
        benchmark_comparison: vec![
            PsionRvllmFusedKernelBenchmark {
                family_id: String::from("qwen35_qkv_rms_norm"),
                before_op_latency_ms: 1.92,
                after_op_latency_ms: 1.31,
                before_end_to_end_latency_ms: 22.4,
                after_end_to_end_latency_ms: 20.1,
                before_tokens_per_second: 55.7,
                after_tokens_per_second: 61.8,
            },
            PsionRvllmFusedKernelBenchmark {
                family_id: String::from("gpt_oss_selected4_moe"),
                before_op_latency_ms: 4.84,
                after_op_latency_ms: 3.62,
                before_end_to_end_latency_ms: 37.5,
                after_end_to_end_latency_ms: 34.1,
                before_tokens_per_second: 41.8,
                after_tokens_per_second: 46.2,
            },
        ],
        retained_tests: vec![
            String::from(
                "cuda_submission_split_interleaved_query_gate_rms_norm_matches_separate_kernels",
            ),
            String::from("cuda_submission_pack_qwen35_key_value_rms_norm_matches_separate_kernels"),
            String::from(
                "cuda_submission_pack_qwen35_hybrid_qkv_rms_norm_matches_separate_kernels",
            ),
            String::from(
                "cuda_submission_executes_mxfp4_expert_gate_up_swiglu_q8_1_ids_when_available",
            ),
            String::from(
                "cuda_submission_executes_mxfp4_moe_down_aggregate_q8_1_f32_selected4_path_when_available",
            ),
        ],
        claim_boundary: String::from(
            "This packet freezes only the highest-value admitted fused-kernel families already owned inside Psionic. It does not claim a general PTX compiler lane, broad kernel auto-generation, or permission to widen fused-kernel ownership beyond the profiled qwen35 QKV/RMSNorm and gpt-oss selected4 MoE hot paths.",
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
        serde_json::to_vec(value).expect("psion rvllm fused kernels packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmFusedKernelsPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_fused_kernels_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_fused_kernels_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
