use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_SAMPLING_LOOP_SCHEMA_VERSION: &str = "psion.rvllm_sampling_loop.v1";
pub const PSION_RVLLM_SAMPLING_LOOP_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_sampling_loop_v1.json";
pub const PSION_RVLLM_SAMPLING_LOOP_DOC_PATH: &str = "docs/PSION_RVLLM_SAMPLING_LOOP.md";

const PACKET_ID: &str = "psion_rvllm_sampling_loop_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmSamplingLane {
    pub lane_id: String,
    pub selection_surface: String,
    pub candidate_materialization: String,
    pub host_device_sync_posture: String,
    pub seeded_replay_preserved: bool,
    pub structured_output_preserved: bool,
    pub penalty_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmSamplingLoopPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub qwen35_lanes: Vec<PsionRvllmSamplingLane>,
    pub gpt_oss_lanes: Vec<PsionRvllmSamplingLane>,
    pub qwen35_scratch_reuse_fields: Vec<String>,
    pub seeded_parity_tests: Vec<String>,
    pub benchmark_paths: Vec<String>,
    pub optimization_posture: String,
    pub packet_digest: String,
}

impl PsionRvllmSamplingLoopPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_sampling_loop_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_sampling_loop_packet() -> PsionRvllmSamplingLoopPacket {
    let mut packet = PsionRvllmSamplingLoopPacket {
        schema_version: String::from(PSION_RVLLM_SAMPLING_LOOP_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("generation_sampler.seeded_replay"),
            String::from("qwen35.native_cuda_decode"),
            String::from("gpt_oss.native_cuda_decode"),
        ],
        qwen35_lanes: vec![
            PsionRvllmSamplingLane {
                lane_id: String::from("qwen35.argmax_or_exact_candidate"),
                selection_surface: String::from(
                    "gpu argmax or exact bounded candidates with seeded host replay",
                ),
                candidate_materialization: String::from(
                    "selected token only, or exact top-k candidate set when admitted",
                ),
                host_device_sync_posture: String::from(
                    "avoids dense logits copies on admitted argmax and bounded-candidate lanes",
                ),
                seeded_replay_preserved: true,
                structured_output_preserved: true,
                penalty_path: String::from(
                    "sparse penalty history encoded once per step into reused device buffers",
                ),
            },
            PsionRvllmSamplingLane {
                lane_id: String::from("qwen35.raw_logits_fallback"),
                selection_surface: String::from("dense host logits fallback"),
                candidate_materialization: String::from("full logits row"),
                host_device_sync_posture: String::from(
                    "fallback remains explicit when the request leaves the bounded candidate envelope",
                ),
                seeded_replay_preserved: true,
                structured_output_preserved: true,
                penalty_path: String::from(
                    "host sampler applies the same penalty and structured-output contract",
                ),
            },
        ],
        gpt_oss_lanes: vec![
            PsionRvllmSamplingLane {
                lane_id: String::from("gpt_oss.device_argmax"),
                selection_surface: String::from("device argmax with seeded host replay"),
                candidate_materialization: String::from("selected token only"),
                host_device_sync_posture: String::from(
                    "keeps decode hot path on the device while preserving seeded replay semantics",
                ),
                seeded_replay_preserved: true,
                structured_output_preserved: true,
                penalty_path: String::from(
                    "host sampler timing stays explicit in stage_timings.sampling_ns",
                ),
            },
            PsionRvllmSamplingLane {
                lane_id: String::from("gpt_oss.raw_logits_fallback"),
                selection_surface: String::from("dense host logits fallback"),
                candidate_materialization: String::from("full logits row"),
                host_device_sync_posture: String::from(
                    "fallback stays explicit for non-admitted sampled requests",
                ),
                seeded_replay_preserved: true,
                structured_output_preserved: true,
                penalty_path: String::from(
                    "penalties remain distribution-preserving because they reuse the same GenerationSampler contract",
                ),
            },
        ],
        qwen35_scratch_reuse_fields: vec![
            String::from("penalty_token_ids_scratch"),
            String::from("penalty_token_counts_scratch"),
            String::from("sparse_logit_indices_scratch"),
            String::from("top_k_indices_buffer"),
            String::from("top_k_values_buffer"),
        ],
        seeded_parity_tests: vec![
            String::from("seeded_sampling_is_replayable"),
            String::from(
                "bounded_candidate_sampling_matches_dense_sampling_when_candidate_set_is_exact",
            ),
            String::from(
                "bounded_candidate_sampling_matches_dense_sampling_with_penalties_when_candidate_set_is_exact",
            ),
        ],
        benchmark_paths: vec![
            String::from("crates/psionic-serve/examples/qwen35_cuda_bench.rs"),
            String::from("crates/psionic-serve/src/qwen35.rs"),
            String::from("crates/psionic-serve/src/gpt_oss.rs"),
        ],
        optimization_posture: String::from(
            "The sampling loop is already narrowed to admitted device-resident selection lanes, reused scratch buffers, sparse penalty encoding, and seeded replay parity. This packet makes that hot-path truth explicit instead of treating it as lane-local implementation folklore.",
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
        serde_json::to_vec(value).expect("psion rvllm sampling loop packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmSamplingLoopPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_sampling_loop_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_sampling_loop_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
