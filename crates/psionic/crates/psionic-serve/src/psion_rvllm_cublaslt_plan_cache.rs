use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_CUBLASLT_PLAN_CACHE_SCHEMA_VERSION: &str =
    "psion.rvllm_cublaslt_plan_cache.v1";
pub const PSION_RVLLM_CUBLASLT_PLAN_CACHE_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_cublaslt_plan_cache_v1.json";
pub const PSION_RVLLM_CUBLASLT_PLAN_CACHE_DOC_PATH: &str =
    "docs/PSION_RVLLM_CUBLASLT_PLAN_CACHE.md";

const PACKET_ID: &str = "psion_rvllm_cublaslt_plan_cache_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmCublasLtPlanCachePacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub admitted_serving_paths: Vec<String>,
    pub plan_cache_scope: String,
    pub admitted_backend_route: String,
    pub admitted_input_dtype: String,
    pub admitted_output_dtype: String,
    pub admitted_row_ladder: Vec<usize>,
    pub max_admitted_rows: usize,
    pub representative_scopes: Vec<String>,
    pub startup_report_fields: Vec<String>,
    pub required_evidence_fields: Vec<String>,
    pub validation_surface: Vec<String>,
    pub fallback_posture: String,
    pub packet_digest: String,
}

impl PsionRvllmCublasLtPlanCachePacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_cublaslt_plan_cache_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_cublaslt_plan_cache_packet() -> PsionRvllmCublasLtPlanCachePacket {
    let mut packet = PsionRvllmCublasLtPlanCachePacket {
        schema_version: String::from(PSION_RVLLM_CUBLASLT_PLAN_CACHE_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("cuda_backend_runtime"),
            String::from("qwen35.native_cuda_decode"),
            String::from("gpt_oss.native_cuda_decode"),
        ],
        admitted_serving_paths: vec![
            String::from("qwen35.native_cuda_decode"),
            String::from("gpt_oss.native_cuda_decode"),
        ],
        plan_cache_scope: String::from("per_weight_scope_shape_dtype_route"),
        admitted_backend_route: String::from("cublaslt_f16_to_f32"),
        admitted_input_dtype: String::from("f16"),
        admitted_output_dtype: String::from("f32"),
        admitted_row_ladder: vec![1, 8, 32],
        max_admitted_rows: 32,
        representative_scopes: vec![
            String::from("qwen35.native_cuda_decode/output_logits"),
            String::from("qwen35.native_cuda_decode/ffn_gate_up"),
            String::from("qwen35.native_cuda_decode/ffn_down"),
            String::from("qwen35.native_cuda_decode/hybrid_qkv_gate_alpha_beta"),
            String::from("qwen35.native_cuda_decode/hybrid_ssm_out"),
            String::from("qwen35.native_cuda_decode/attention_qkv"),
            String::from("qwen35.native_cuda_decode/attention_output"),
            String::from("gpt_oss.native_cuda_decode/output_logits"),
            String::from("gpt_oss.native_cuda_decode/attention_qkv"),
            String::from("gpt_oss.native_cuda_decode/attention_output"),
        ],
        startup_report_fields: vec![
            String::from("cublas_lt_tuning_status"),
            String::from("cublas_lt_plan_cache_scope"),
            String::from("cublas_lt_selected_plan_count"),
            String::from("cublas_lt_tuned_shape_count"),
            String::from("cublas_lt_fallback_shape_count"),
            String::from("cublas_lt_max_workspace_bytes"),
            String::from("cublas_lt_selected_plans"),
        ],
        required_evidence_fields: vec![
            String::from("psionic_cuda_startup.cublas_lt_tuning_status"),
            String::from("psionic_cuda_startup.cublas_lt_plan_cache_scope"),
            String::from("psionic_cuda_startup.cublas_lt_selected_plan_count"),
            String::from("psionic_cuda_startup.cublas_lt_tuned_shape_count"),
            String::from("psionic_cuda_startup.cublas_lt_fallback_shape_count"),
            String::from("psionic_cuda_startup.cublas_lt_max_workspace_bytes"),
            String::from("psionic_cuda_startup.cublas_lt_selected_plans[].model_family"),
            String::from("psionic_cuda_startup.cublas_lt_selected_plans[].op_kind"),
            String::from("psionic_cuda_startup.cublas_lt_selected_plans[].rows"),
            String::from("psionic_cuda_startup.cublas_lt_selected_plans[].inner"),
            String::from("psionic_cuda_startup.cublas_lt_selected_plans[].cols"),
            String::from("psionic_cuda_startup.cublas_lt_selected_plans[].backend_route"),
            String::from("psionic_cuda_startup.cublas_lt_selected_plans[].workspace_bytes"),
            String::from("psionic_cuda_startup.cublas_lt_selected_plans[].mean_time_us"),
            String::from("psionic_cuda_startup.cublas_lt_selected_plans[].algorithm_fingerprint"),
        ],
        validation_surface: vec![
            String::from("builtin_packet_matches_committed_fixture"),
            String::from("cargo build -p psionic-serve --example qwen35_cuda_bench"),
            String::from("cargo build -p psionic-serve --bin psionic-openai-server"),
        ],
        fallback_posture: String::from(
            "The admitted CUDA serving lane now caches one selected cublasLt plan per representative weight scope and admitted row shape. Startup receipts publish whether tuning completed, how many shapes stayed on the legacy GEMM fallback, which algorithm fingerprints were selected, and how much workspace the runtime reserved instead of silently blending tuned and untuned GEMMs into one opaque throughput number.",
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
        serde_json::to_vec(value).expect("psion rvllm cublasLt plan cache packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmCublasLtPlanCachePacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_cublaslt_plan_cache_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_cublaslt_plan_cache_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
