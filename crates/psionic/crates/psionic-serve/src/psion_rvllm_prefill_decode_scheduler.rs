use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_PREFILL_DECODE_SCHEDULER_SCHEMA_VERSION: &str =
    "psion.rvllm_prefill_decode_scheduler.v1";
pub const PSION_RVLLM_PREFILL_DECODE_SCHEDULER_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_prefill_decode_scheduler_v1.json";
pub const PSION_RVLLM_PREFILL_DECODE_SCHEDULER_DOC_PATH: &str =
    "docs/PSION_RVLLM_PREFILL_DECODE_SCHEDULER.md";

const PACKET_ID: &str = "psion_rvllm_prefill_decode_scheduler_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmPrefillDecodeSchedulerPolicy {
    pub max_active_requests: usize,
    pub max_queued_requests: usize,
    pub max_prefill_tokens_per_tick: usize,
    pub max_decode_tokens_per_tick: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmPrefillDecodeSchedulerPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub admitted_scheduler_policy: PsionRvllmPrefillDecodeSchedulerPolicy,
    pub realized_scheduling_classes: Vec<String>,
    pub admitted_execution_modes: Vec<String>,
    pub admitted_handoff_transports: Vec<String>,
    pub request_receipt_fields: Vec<String>,
    pub aggregate_metric_fields: Vec<String>,
    pub response_headers: Vec<String>,
    pub retained_tests: Vec<String>,
    pub benchmark_paths: Vec<String>,
    pub stability_posture: String,
    pub packet_digest: String,
}

impl PsionRvllmPrefillDecodeSchedulerPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_prefill_decode_scheduler_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_prefill_decode_scheduler_packet()
-> PsionRvllmPrefillDecodeSchedulerPacket {
    let mut packet = PsionRvllmPrefillDecodeSchedulerPacket {
        schema_version: String::from(PSION_RVLLM_PREFILL_DECODE_SCHEDULER_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("psionic_runtime.generation_scheduler_policy"),
            String::from("psionic_serve.continuous_batch_scheduler"),
            String::from("psionic_serve.openai_http_scheduler_headers"),
        ],
        admitted_scheduler_policy: PsionRvllmPrefillDecodeSchedulerPolicy {
            max_active_requests: 4,
            max_queued_requests: 32,
            max_prefill_tokens_per_tick: 4,
            max_decode_tokens_per_tick: 8,
        },
        realized_scheduling_classes: vec![
            String::from("prefill"),
            String::from("decode"),
            String::from("mixed_prefill_decode"),
            String::from("fallback_single_request"),
        ],
        admitted_execution_modes: vec![String::from("disaggregated_colocated")],
        admitted_handoff_transports: vec![String::from("in_process_kv_state")],
        request_receipt_fields: vec![
            String::from("queue_depth_at_admission"),
            String::from("max_batch_size_observed"),
            String::from("scheduling_class"),
            String::from("prefill_tokens"),
            String::from("decode_tokens"),
            String::from("prefill_decode_mode"),
            String::from("prefill_decode_handoff"),
            String::from("time_to_first_token_ns"),
            String::from("inter_token_latency_ns"),
            String::from("fallback_reason"),
        ],
        aggregate_metric_fields: vec![
            String::from("total_prefill_tokens"),
            String::from("total_decode_tokens"),
            String::from("total_time_to_first_token_ns"),
            String::from("total_inter_token_latency_ns"),
            String::from("peak_kv_pages_in_use"),
            String::from("peak_kv_bytes_in_use"),
            String::from("fallback_counts"),
        ],
        response_headers: vec![
            String::from("x-psionic-batch-posture"),
            String::from("x-psionic-scheduling-class"),
            String::from("x-psionic-prefill-decode-mode"),
            String::from("x-psionic-ttft-ns"),
            String::from("x-psionic-itl-ns"),
        ],
        retained_tests: vec![
            String::from("cpu_reference_continuous_batch_scheduler_mixes_prefill_and_decode"),
            String::from("generic_server_grammar_fallback_is_machine_checkable"),
            String::from("generic_server_json_schema_fallback_is_machine_checkable"),
        ],
        benchmark_paths: vec![
            String::from("crates/psionic-runtime/src/lib.rs"),
            String::from("crates/psionic-serve/src/lib.rs"),
            String::from("crates/psionic-serve/src/openai_http.rs"),
            String::from("crates/psionic-serve/src/gpt_oss.rs"),
        ],
        stability_posture: String::from(
            "Psionic already exposes a real prefill-vs-decode scheduler split: the admitted continuous-batch policy budgets prompt-prefill and decode tokens separately, request receipts record realized scheduling class plus TTFT and ITL, and the OpenAI-compatible surface exports those facts as stable headers. This packet closes the scheduler-reporting seam without claiming a second runtime or broader batching capability than the admitted lane actually has.",
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
            .expect("psion rvllm prefill decode scheduler packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmPrefillDecodeSchedulerPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_prefill_decode_scheduler_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_prefill_decode_scheduler_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
