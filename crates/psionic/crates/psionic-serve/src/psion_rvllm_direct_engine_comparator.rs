use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_DIRECT_ENGINE_COMPARATOR_SCHEMA_VERSION: &str =
    "psion.rvllm_direct_engine_comparator.v1";
pub const PSION_RVLLM_DIRECT_ENGINE_COMPARATOR_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_direct_engine_comparator_v1.json";
pub const PSION_RVLLM_DIRECT_ENGINE_COMPARATOR_DOC_PATH: &str =
    "docs/PSION_RVLLM_DIRECT_ENGINE_COMPARATOR.md";

const PACKET_ID: &str = "psion_rvllm_direct_engine_comparator_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmDirectEngineComparatorPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub admitted_benchmark_classes: Vec<String>,
    pub prompt_contracts: Vec<String>,
    pub direct_engine_receipt_fields: Vec<String>,
    pub http_receipt_fields: Vec<String>,
    pub concurrency_ladder: Vec<usize>,
    pub operator_paths: Vec<String>,
    pub optional_reference_helpers: Vec<String>,
    pub validation_surface: Vec<String>,
    pub stability_posture: String,
    pub packet_digest: String,
}

impl PsionRvllmDirectEngineComparatorPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_direct_engine_comparator_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_direct_engine_comparator_packet()
-> PsionRvllmDirectEngineComparatorPacket {
    let mut packet = PsionRvllmDirectEngineComparatorPacket {
        schema_version: String::from(PSION_RVLLM_DIRECT_ENGINE_COMPARATOR_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("qwen35.native_cuda_direct_receipt"),
            String::from("openai_compat.native_cuda_http_receipt"),
            String::from("optional_reference.vllm_direct_receipt"),
        ],
        admitted_benchmark_classes: vec![
            String::from("direct_engine"),
            String::from("http"),
            String::from("optional_reference_direct_engine"),
        ],
        prompt_contracts: vec![String::from("greedy_one_sentence")],
        direct_engine_receipt_fields: vec![
            String::from("load_s"),
            String::from("psionic_cuda_startup.warmup_prompt_s"),
            String::from("psionic_cuda_startup.warmup_decode_s"),
            String::from("psionic_cuda_startup.warmup_total_s"),
            String::from("runs[].ttft_s"),
            String::from("runs[].itl_s"),
            String::from("runs[].total_s"),
            String::from("mean_ttft_s"),
            String::from("mean_itl_s"),
            String::from("mean_total_s"),
            String::from("mean_decode_tok_s"),
            String::from("steady_state_concurrency"),
        ],
        http_receipt_fields: vec![
            String::from("startup_ready_s"),
            String::from("warmup.wallclock_s"),
            String::from("warmup.ttft_s"),
            String::from("warmup.itl_s"),
            String::from("warmup.completion_tok_s"),
            String::from("concurrency_results[].concurrency"),
            String::from("concurrency_results[].aggregate_tok_s"),
            String::from("concurrency_results[].mean_wallclock_s"),
            String::from("concurrency_results[].mean_ttft_s"),
            String::from("concurrency_results[].mean_itl_s"),
            String::from("concurrency_results[].scheduling_classes"),
        ],
        concurrency_ladder: vec![1, 2, 4],
        operator_paths: vec![
            String::from("crates/psionic-serve/examples/qwen35_cuda_bench.rs"),
            String::from("crates/psionic-serve/src/openai_http.rs"),
            String::from("scripts/release/qwen35_direct_vs_http_compare.py"),
        ],
        optional_reference_helpers: vec![
            String::from("python_import:vllm"),
            String::from("competition/repos/rvllm/deploy/vllm_direct_bench.py"),
        ],
        validation_surface: vec![
            String::from("builtin_packet_matches_committed_fixture"),
            String::from("cargo build -p psionic-serve --example qwen35_cuda_bench"),
            String::from("cargo build -p psionic-serve --bin psionic-openai-server"),
            String::from("python3 -m py_compile scripts/release/qwen35_direct_vs_http_compare.py"),
        ],
        stability_posture: String::from(
            "Psionic now owns one explicit comparator contract that keeps native direct-engine, native OpenAI-compatible HTTP, and optional reference direct-engine receipts separate while pinning them to one prompt contract and one admitted concurrency ladder. It makes runtime-versus-server overhead visible without widening the admitted model lane or publishing mixed benchmark classes as if they were the same number.",
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
            .expect("psion rvllm direct engine comparator packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmDirectEngineComparatorPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_direct_engine_comparator_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_direct_engine_comparator_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
