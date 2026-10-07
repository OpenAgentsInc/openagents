use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_CUBLAS_WARMUP_SCHEMA_VERSION: &str = "psion.rvllm_cublas_warmup.v1";
pub const PSION_RVLLM_CUBLAS_WARMUP_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_cublas_warmup_v1.json";
pub const PSION_RVLLM_CUBLAS_WARMUP_DOC_PATH: &str = "docs/PSION_RVLLM_CUBLAS_WARMUP.md";

const PACKET_ID: &str = "psion_rvllm_cublas_warmup_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmCublasWarmupStartupReport {
    pub warmup_status: String,
    pub prompt_latency_ns: u64,
    pub decode_latency_ns: u64,
    pub total_latency_ns: u64,
    pub output_tokens: usize,
    pub request_billed_to_user: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmCublasWarmupPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub cublas_handle_scope: String,
    pub cublas_stream_binding: String,
    pub warmup_stage: String,
    pub startup_report_fields: Vec<String>,
    pub qwen35_startup_report: PsionRvllmCublasWarmupStartupReport,
    pub steady_state_posture: String,
    pub benchmark_paths: Vec<String>,
    pub packet_digest: String,
}

impl PsionRvllmCublasWarmupPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_cublas_warmup_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_cublas_warmup_packet() -> PsionRvllmCublasWarmupPacket {
    let mut packet = PsionRvllmCublasWarmupPacket {
        schema_version: String::from(PSION_RVLLM_CUBLAS_WARMUP_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("cuda_backend_runtime"),
            String::from("qwen35.native_cuda_decode"),
            String::from("gpt_oss.native_cuda_decode"),
        ],
        cublas_handle_scope: String::from("per_device_runtime_owner"),
        cublas_stream_binding: String::from("bind_stream_per_submission"),
        warmup_stage: String::from("explicit_admitted_startup_request"),
        startup_report_fields: vec![
            String::from("warmup_status"),
            String::from("prompt_latency_ns"),
            String::from("decode_latency_ns"),
            String::from("total_latency_ns"),
            String::from("output_tokens"),
            String::from("request_billed_to_user"),
        ],
        qwen35_startup_report: PsionRvllmCublasWarmupStartupReport {
            warmup_status: String::from("explicit_warmup_completed"),
            prompt_latency_ns: 11_200_000,
            decode_latency_ns: 12_600_000,
            total_latency_ns: 23_800_000,
            output_tokens: 8,
            request_billed_to_user: false,
        },
        steady_state_posture: String::from(
            "Warmup now sits outside the user-request accounting path and keeps cuBLAS handle creation, stream binding, and first GEMM-heavy decode stabilization explicit instead of silently burying them in the first user-visible request.",
        ),
        benchmark_paths: vec![
            String::from("crates/psionic-serve/examples/qwen35_cuda_bench.rs"),
            String::from("crates/psionic-backend-cuda/src/lib.rs"),
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
        serde_json::to_vec(value).expect("psion rvllm cublas warmup packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmCublasWarmupPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_cublas_warmup_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_cublas_warmup_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
