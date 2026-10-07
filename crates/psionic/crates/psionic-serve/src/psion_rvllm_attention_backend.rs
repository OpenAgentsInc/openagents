use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_ATTENTION_BACKEND_SCHEMA_VERSION: &str = "psion.rvllm_attention_backend.v1";
pub const PSION_RVLLM_ATTENTION_BACKEND_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_attention_backend_v1.json";
pub const PSION_RVLLM_ATTENTION_BACKEND_DOC_PATH: &str = "docs/PSION_RVLLM_ATTENTION_BACKEND.md";

const PACKET_ID: &str = "psion_rvllm_attention_backend_v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionRvllmAttentionBackend {
    DenseF16Kv,
    DenseF16KvQ81OutputFusion,
    TurboquantKv,
}

impl PsionRvllmAttentionBackend {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DenseF16Kv => "dense_f16_kv",
            Self::DenseF16KvQ81OutputFusion => "dense_f16_kv_q8_1_output_fusion",
            Self::TurboquantKv => "turboquant_kv",
        }
    }
}

#[must_use]
pub const fn select_psion_rvllm_attention_backend(
    use_turboquant_kv: bool,
    use_q8_1_attention_output_fusion: bool,
) -> PsionRvllmAttentionBackend {
    if use_turboquant_kv {
        PsionRvllmAttentionBackend::TurboquantKv
    } else if use_q8_1_attention_output_fusion {
        PsionRvllmAttentionBackend::DenseF16KvQ81OutputFusion
    } else {
        PsionRvllmAttentionBackend::DenseF16Kv
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmAttentionBackendPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub default_backend: String,
    pub alternate_backends: Vec<String>,
    pub selection_inputs: Vec<String>,
    pub backend_capability_gates: Vec<String>,
    pub retained_tests: Vec<String>,
    pub benchmark_paths: Vec<String>,
    pub stability_posture: String,
    pub packet_digest: String,
}

impl PsionRvllmAttentionBackendPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_attention_backend_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_attention_backend_packet() -> PsionRvllmAttentionBackendPacket {
    let mut packet = PsionRvllmAttentionBackendPacket {
        schema_version: String::from(PSION_RVLLM_ATTENTION_BACKEND_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("psionic_serve.gpt_oss_cuda_attention_decode"),
            String::from("psionic_serve.decoder_kv_cache_encoding_policy"),
            String::from("psionic_backend_cuda.attention_decode_kernels"),
        ],
        default_backend: String::from(PsionRvllmAttentionBackend::DenseF16Kv.as_str()),
        alternate_backends: vec![
            String::from(PsionRvllmAttentionBackend::DenseF16KvQ81OutputFusion.as_str()),
            String::from(PsionRvllmAttentionBackend::TurboquantKv.as_str()),
        ],
        selection_inputs: vec![
            String::from("use_turboquant_kv"),
            String::from("use_q8_1_attention_output_fusion"),
            String::from("use_graph_attention"),
        ],
        backend_capability_gates: vec![
            String::from("cuda_kv_cache_encoding_selection"),
            String::from("q8_1_attention_output_fusion_capability"),
            String::from("graph_replay_admission"),
        ],
        retained_tests: vec![
            String::from("selector_defaults_to_dense_f16_kv_backend"),
            String::from("selector_uses_q8_1_output_fusion_backend_when_enabled"),
            String::from("selector_prefers_turboquant_backend"),
            String::from("cuda_kv_cache_encoding_selection_activates_turboquant_when_supported"),
        ],
        benchmark_paths: vec![
            String::from("crates/psionic-serve/src/gpt_oss.rs"),
            String::from("crates/psionic-serve/src/lib.rs"),
            String::from("crates/psionic-backend-cuda/src/lib.rs"),
        ],
        stability_posture: String::from(
            "Psionic already shipped multiple real CUDA attention decode backends. This packet and selector make that seam explicit: dense f16 KV remains the default backend, q8_1 output fusion and turboquant KV remain capability-gated alternates, and route logic no longer has to hard-code nested kernel selection branches at each callsite.",
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
        serde_json::to_vec(value).expect("psion rvllm attention backend packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_defaults_to_dense_f16_kv_backend() {
        assert_eq!(
            select_psion_rvllm_attention_backend(false, false),
            PsionRvllmAttentionBackend::DenseF16Kv
        );
    }

    #[test]
    fn selector_uses_q8_1_output_fusion_backend_when_enabled() {
        assert_eq!(
            select_psion_rvllm_attention_backend(false, true),
            PsionRvllmAttentionBackend::DenseF16KvQ81OutputFusion
        );
    }

    #[test]
    fn selector_prefers_turboquant_backend() {
        assert_eq!(
            select_psion_rvllm_attention_backend(true, false),
            PsionRvllmAttentionBackend::TurboquantKv
        );
        assert_eq!(
            select_psion_rvllm_attention_backend(true, true),
            PsionRvllmAttentionBackend::TurboquantKv
        );
    }

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmAttentionBackendPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_attention_backend_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_attention_backend_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
