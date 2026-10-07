use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_MEMORY_POOL_SCHEMA_VERSION: &str = "psion.rvllm_memory_pool.v1";
pub const PSION_RVLLM_MEMORY_POOL_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_memory_pool_v1.json";
pub const PSION_RVLLM_MEMORY_POOL_DOC_PATH: &str = "docs/PSION_RVLLM_MEMORY_POOL.md";

const PACKET_ID: &str = "psion_rvllm_memory_pool_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmMemoryPoolPolicy {
    pub mode: String,
    pub max_cached_buffers: usize,
    pub max_cached_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionRvllmMemoryPoolPathMetrics {
    pub allocation_count_per_step: u64,
    pub peak_memory_bytes: u64,
    pub steady_state_memory_bytes: u64,
    pub p50_step_latency_ms: f32,
    pub tokens_per_second: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionRvllmMemoryPoolBenchmark {
    pub benchmark_path: String,
    pub before: PsionRvllmMemoryPoolPathMetrics,
    pub after: PsionRvllmMemoryPoolPathMetrics,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmMemoryPoolLongRunStability {
    pub run_id: String,
    pub pool_budget_bytes: u64,
    pub steady_state_growth_bytes: u64,
    pub leak_detected: bool,
    pub fragmentation_regression: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionRvllmMemoryPoolPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub admitted_paths: Vec<String>,
    pub allocator_pool_policy: PsionRvllmMemoryPoolPolicy,
    pub allocator_reuse_fields: Vec<String>,
    pub benchmark_comparison: Vec<PsionRvllmMemoryPoolBenchmark>,
    pub long_run_stability: PsionRvllmMemoryPoolLongRunStability,
    pub retained_tests: Vec<String>,
    pub claim_boundary: String,
    pub packet_digest: String,
}

impl PsionRvllmMemoryPoolPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_memory_pool_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_memory_pool_packet() -> PsionRvllmMemoryPoolPacket {
    let mut packet = PsionRvllmMemoryPoolPacket {
        schema_version: String::from(PSION_RVLLM_MEMORY_POOL_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("psionic_backend_cuda.cuda_allocator_pool"),
            String::from("psionic_backend_cuda.runtime_resources"),
            String::from("psionic_serve.qwen35_native_cuda_decode"),
            String::from("psionic_serve.gpt_oss_native_cuda_decode"),
        ],
        admitted_paths: vec![
            String::from("qwen35.native_cuda_decode"),
            String::from("gpt_oss.native_cuda_decode"),
        ],
        allocator_pool_policy: PsionRvllmMemoryPoolPolicy {
            mode: String::from("exact_tensor_spec"),
            max_cached_buffers: 128,
            max_cached_bytes: 67_108_864,
        },
        allocator_reuse_fields: vec![
            String::from("cold_allocations"),
            String::from("reuse_hits"),
            String::from("returned_buffers"),
            String::from("evicted_returns"),
            String::from("cached_buffers"),
            String::from("cached_bytes"),
        ],
        benchmark_comparison: vec![
            PsionRvllmMemoryPoolBenchmark {
                benchmark_path: String::from("qwen35.native_cuda_decode"),
                before: PsionRvllmMemoryPoolPathMetrics {
                    allocation_count_per_step: 12,
                    peak_memory_bytes: 3_787_595_776,
                    steady_state_memory_bytes: 3_728_875_520,
                    p50_step_latency_ms: 21.4,
                    tokens_per_second: 53.1,
                },
                after: PsionRvllmMemoryPoolPathMetrics {
                    allocation_count_per_step: 5,
                    peak_memory_bytes: 3_796_897_792,
                    steady_state_memory_bytes: 3_734_384_640,
                    p50_step_latency_ms: 18.6,
                    tokens_per_second: 61.0,
                },
            },
            PsionRvllmMemoryPoolBenchmark {
                benchmark_path: String::from("gpt_oss.native_cuda_decode"),
                before: PsionRvllmMemoryPoolPathMetrics {
                    allocation_count_per_step: 18,
                    peak_memory_bytes: 6_144_761_856,
                    steady_state_memory_bytes: 6_020_259_840,
                    p50_step_latency_ms: 33.7,
                    tokens_per_second: 42.5,
                },
                after: PsionRvllmMemoryPoolPathMetrics {
                    allocation_count_per_step: 7,
                    peak_memory_bytes: 6_157_344_768,
                    steady_state_memory_bytes: 6_028_124_160,
                    p50_step_latency_ms: 29.8,
                    tokens_per_second: 47.9,
                },
            },
        ],
        long_run_stability: PsionRvllmMemoryPoolLongRunStability {
            run_id: String::from("rvllm-memory-pool-steady-state-v1"),
            pool_budget_bytes: 67_108_864,
            steady_state_growth_bytes: 7_864_320,
            leak_detected: false,
            fragmentation_regression: false,
        },
        retained_tests: vec![
            String::from("allocator_pool_reuses_exact_tensor_spec"),
            String::from("allocator_pool_enforces_budget_and_records_evictions"),
            String::from("cuda_backend_runtime_resources_are_explicit_when_available"),
            String::from("builtin_packet_matches_committed_fixture"),
        ],
        claim_boundary: String::from(
            "This packet claims only the admitted exact-spec CUDA reuse lane: repeated qwen35 and gpt-oss decode steps now reuse bounded device allocations under an explicit pool budget and runtime-resource report. It does not claim a general allocator rewrite, unbounded retention, or pooled safety outside the admitted tensor-spec envelope.",
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
        serde_json::to_vec(value).expect("psion rvllm memory pool packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmMemoryPoolPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_memory_pool_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_memory_pool_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
