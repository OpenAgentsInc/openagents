use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_FALLBACK_FREE_CUDA_GATE_SCHEMA_VERSION: &str =
    "psion.rvllm_fallback_free_cuda_gate.v1";
pub const PSION_RVLLM_FALLBACK_FREE_CUDA_GATE_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_fallback_free_cuda_gate_v1.json";
pub const PSION_RVLLM_FALLBACK_FREE_CUDA_GATE_DOC_PATH: &str =
    "docs/PSION_RVLLM_FALLBACK_FREE_CUDA_GATE.md";

const PACKET_ID: &str = "psion_rvllm_fallback_free_cuda_gate_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmFallbackFreeCudaGatePacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub admitted_lane: String,
    pub compatibility_lane: String,
    pub refusal_lane: String,
    pub admitted_contract: Vec<String>,
    pub env_guards: Vec<String>,
    pub required_evidence_fields: Vec<String>,
    pub operator_paths: Vec<String>,
    pub validation_surface: Vec<String>,
    pub stability_posture: String,
    pub packet_digest: String,
}

impl PsionRvllmFallbackFreeCudaGatePacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_fallback_free_cuda_gate_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_fallback_free_cuda_gate_packet() -> PsionRvllmFallbackFreeCudaGatePacket
{
    let mut packet = PsionRvllmFallbackFreeCudaGatePacket {
        schema_version: String::from(PSION_RVLLM_FALLBACK_FREE_CUDA_GATE_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("qwen35.native_cuda_decode"),
            String::from("qwen35.native_cuda_benchmark_publication"),
            String::from("openai_compat.native_cuda_comparator_publication"),
        ],
        admitted_lane: String::from("fallback_free_fast_path"),
        compatibility_lane: String::from("explicit_fallback_path"),
        refusal_lane: String::from("unsupported_or_refused"),
        admitted_contract: vec![
            String::from("backend=psionic"),
            String::from("decode_mode=greedy"),
            String::from("structured_output=none"),
            String::from("sampling_knobs=absent"),
            String::from("cli_flag=--require-fallback-free-cuda"),
        ],
        env_guards: vec![
            String::from("PSIONIC_QWEN35_DISABLE_FAST_GREEDY=unset"),
            String::from("PSIONIC_QWEN35_DISABLE_FUSED_QKV_RMS_NORM=unset"),
            String::from("PSIONIC_QWEN35_DEBUG_ATTENTION=unset"),
            String::from("PSIONIC_QWEN35_DEBUG_FUSED_LAYERS=unset"),
        ],
        required_evidence_fields: vec![
            String::from("run_status"),
            String::from("refusal_reason"),
            String::from("psionic_cuda_fast_path.lane"),
            String::from("psionic_cuda_fast_path.status"),
            String::from("psionic_cuda_fast_path.env_guards[]"),
            String::from("psionic_cuda_startup.warmup_host_fallback_evidence"),
            String::from("runs[].qwen35_host_fallback_evidence"),
            String::from("runs[].qwen35_output_modes"),
            String::from("runs[].qwen35_raw_logits"),
            String::from("runs[].qwen35_graph_hits"),
            String::from("runs[].qwen35_graph_misses"),
            String::from("runs[].qwen35_graph_captures"),
            String::from("runs[].qwen35_graph_shape_drifts"),
            String::from("publication_gate.direct_engine_fallback_free_required"),
        ],
        operator_paths: vec![
            String::from("crates/psionic-serve/examples/qwen35_cuda_bench.rs"),
            String::from("scripts/release/qwen35_direct_vs_http_compare.py"),
            String::from("docs/PSION_RVLLM_FALLBACK_FREE_CUDA_GATE.md"),
        ],
        validation_surface: vec![
            String::from("builtin_packet_matches_committed_fixture"),
            String::from("cargo build -p psionic-serve --example qwen35_cuda_bench"),
            String::from("cargo build -p psionic-serve --bin psionic-openai-server"),
            String::from("python3 -m py_compile scripts/release/qwen35_direct_vs_http_compare.py"),
        ],
        stability_posture: String::from(
            "Psionic now publishes one explicit benchmark gate for the admitted qwen35 CUDA greedy lane: either the receipt proves fallback_free_fast_path, it stays visible as an explicit_fallback_path compatibility run, or it refuses publication as unsupported_or_refused. Host fallback evidence, raw-logits materialization, and graph instability are no longer silent degradations inside the published direct-engine comparator.",
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
            .expect("psion rvllm fallback free cuda gate packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmFallbackFreeCudaGatePacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_fallback_free_cuda_gate_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_fallback_free_cuda_gate_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
