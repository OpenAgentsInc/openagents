use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::psion_rvllm_cublaslt_plan_cache::builtin_psion_rvllm_cublaslt_plan_cache_packet;

pub const PSION_RVLLM_PREFLIGHT_BUNDLE_SCHEMA_VERSION: &str = "psion.rvllm_preflight_bundle.v1";
pub const PSION_RVLLM_PREFLIGHT_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_preflight_bundle_v1.json";
pub const PSION_RVLLM_PREFLIGHT_BUNDLE_DOC_PATH: &str = "docs/PSION_RVLLM_PREFLIGHT_BUNDLE.md";

const PACKET_ID: &str = "psion_rvllm_preflight_bundle_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmPreflightReference {
    pub packet_id: String,
    pub packet_digest: String,
    pub purpose: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmPreflightStep {
    pub step_id: String,
    pub stage: String,
    pub success_signal: String,
    pub cold_start_cost_billed_to_user: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmAllocatorPoolPosture {
    pub policy: String,
    pub max_cached_buffers: usize,
    pub max_cached_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmKernelCachePosture {
    pub enabled: bool,
    pub state: String,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmPreflightBundlePacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub admitted_serving_paths: Vec<String>,
    pub referenced_packets: Vec<PsionRvllmPreflightReference>,
    pub preflight_steps: Vec<PsionRvllmPreflightStep>,
    pub startup_report_fields: Vec<String>,
    pub allocator_pool: PsionRvllmAllocatorPoolPosture,
    pub kernel_cache: PsionRvllmKernelCachePosture,
    pub cold_vs_warm_posture: String,
    pub benchmark_paths: Vec<String>,
    pub packet_digest: String,
}

impl PsionRvllmPreflightBundlePacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_preflight_bundle_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_preflight_bundle_packet() -> PsionRvllmPreflightBundlePacket {
    let cublaslt_plan_cache = builtin_psion_rvllm_cublaslt_plan_cache_packet();
    let mut packet = PsionRvllmPreflightBundlePacket {
        schema_version: String::from(PSION_RVLLM_PREFLIGHT_BUNDLE_SCHEMA_VERSION),
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
        referenced_packets: vec![
            PsionRvllmPreflightReference {
                packet_id: String::from("psion_rvllm_cuda_graph_pool_v1"),
                packet_digest: String::from(
                    "47864743915a631327933f5ec2ca3d1d7bed30b426592c7aad9b84f23276e509",
                ),
                purpose: String::from("graph capture, replay, and refusal posture"),
            },
            PsionRvllmPreflightReference {
                packet_id: cublaslt_plan_cache.packet_id.clone(),
                packet_digest: cublaslt_plan_cache.packet_digest.clone(),
                purpose: String::from(
                    "startup autotune, selected-plan receipts, and bounded cublasLt fallback posture",
                ),
            },
            PsionRvllmPreflightReference {
                packet_id: String::from("psion_rvllm_cublas_warmup_v1"),
                packet_digest: String::from(
                    "ab8ec999b8e80605744171d3adab152b6ffaecda22877b321ec4c65bb084ea36",
                ),
                purpose: String::from("startup request, cuBLAS handle reuse, and stream binding"),
            },
        ],
        preflight_steps: vec![
            PsionRvllmPreflightStep {
                step_id: String::from("cuda_runtime_owner_ready"),
                stage: String::from("device discovery and runtime resources"),
                success_signal: String::from("runtime_resources present"),
                cold_start_cost_billed_to_user: false,
            },
            PsionRvllmPreflightStep {
                step_id: String::from("allocator_pool_primed"),
                stage: String::from("allocation reuse posture"),
                success_signal: String::from("allocator_pool policy exported"),
                cold_start_cost_billed_to_user: false,
            },
            PsionRvllmPreflightStep {
                step_id: String::from("cublaslt_plan_cache_ready"),
                stage: String::from("bounded startup autotune and GEMM-plan selection"),
                success_signal: String::from("psionic_cuda_startup.cublas_lt_tuning_status"),
                cold_start_cost_billed_to_user: false,
            },
            PsionRvllmPreflightStep {
                step_id: String::from("cublas_warmup_request"),
                stage: String::from("explicit admitted startup request"),
                success_signal: String::from("psionic_cuda_startup.warmup_status"),
                cold_start_cost_billed_to_user: false,
            },
            PsionRvllmPreflightStep {
                step_id: String::from("cuda_graph_capture_or_refusal"),
                stage: String::from("decode graph bringup"),
                success_signal: String::from("cuda_graph_replay hit/miss/capture or refusal"),
                cold_start_cost_billed_to_user: false,
            },
            PsionRvllmPreflightStep {
                step_id: String::from("kernel_cache_posture"),
                stage: String::from("kernel or module load visibility"),
                success_signal: String::from("runtime_resources.kernel_cache"),
                cold_start_cost_billed_to_user: false,
            },
        ],
        startup_report_fields: vec![
            String::from("cublas_handle_scope"),
            String::from("cublas_stream_binding"),
            String::from("cublas_lt_tuning_status"),
            String::from("cublas_lt_plan_cache_scope"),
            String::from("cublas_lt_selected_plan_count"),
            String::from("cublas_lt_tuned_shape_count"),
            String::from("cublas_lt_fallback_shape_count"),
            String::from("cublas_lt_max_workspace_bytes"),
            String::from("cublas_lt_selected_plans"),
            String::from("warmup_status"),
            String::from("warmup_prompt_s"),
            String::from("warmup_decode_s"),
            String::from("warmup_total_s"),
            String::from("warmup_output_tokens"),
            String::from("request_billed_to_user"),
        ],
        allocator_pool: PsionRvllmAllocatorPoolPosture {
            policy: String::from("exact_tensor_spec"),
            max_cached_buffers: 128,
            max_cached_bytes: 64 * 1024 * 1024,
        },
        kernel_cache: PsionRvllmKernelCachePosture {
            enabled: false,
            state: String::from("disabled_but_machine_visible"),
            claim_boundary: String::from(
                "Kernel-cache posture is exported as runtime evidence even when the active cache remains disabled on the admitted lane.",
            ),
        },
        cold_vs_warm_posture: String::from(
            "The admitted CUDA serving lane now has one explicit pre-flight bundle: cublasLt plan selection and startup warmup are both logged outside the user-billed request path, cold-start cost stays separate from steady-state timings, graph capture or refusal remains explicit, and allocator-pool / kernel-cache posture is exported through runtime resources instead of being implied by folklore.",
        ),
        benchmark_paths: vec![
            String::from("crates/psionic-serve/examples/qwen35_cuda_bench.rs"),
            String::from("crates/psionic-serve/src/qwen35.rs"),
            String::from("crates/psionic-serve/src/gpt_oss.rs"),
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
        serde_json::to_vec(value).expect("psion rvllm preflight bundle packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmPreflightBundlePacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_preflight_bundle_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_preflight_bundle_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
