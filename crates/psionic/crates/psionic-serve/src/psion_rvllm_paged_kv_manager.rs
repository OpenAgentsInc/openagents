use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_PAGED_KV_MANAGER_SCHEMA_VERSION: &str = "psion.rvllm_paged_kv_manager.v1";
pub const PSION_RVLLM_PAGED_KV_MANAGER_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_paged_kv_manager_v1.json";
pub const PSION_RVLLM_PAGED_KV_MANAGER_DOC_PATH: &str = "docs/PSION_RVLLM_PAGED_KV_MANAGER.md";

const PACKET_ID: &str = "psion_rvllm_paged_kv_manager_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmPagedKvFeature {
    pub feature_id: String,
    pub current_posture: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmPagedKvManagerPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub logical_page_tokens_default: usize,
    pub owner_classes: Vec<String>,
    pub spill_policies: Vec<String>,
    pub residency_tiers: Vec<String>,
    pub residency_movements: Vec<String>,
    pub block_manager_features: Vec<PsionRvllmPagedKvFeature>,
    pub retained_tests: Vec<String>,
    pub benchmark_paths: Vec<String>,
    pub stability_posture: String,
    pub packet_digest: String,
}

impl PsionRvllmPagedKvManagerPacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_paged_kv_manager_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_paged_kv_manager_packet() -> PsionRvllmPagedKvManagerPacket {
    let mut packet = PsionRvllmPagedKvManagerPacket {
        schema_version: String::from(PSION_RVLLM_PAGED_KV_MANAGER_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("psionic_runtime.kv_cache_policy"),
            String::from("psionic_serve.inmemory_kv_cache"),
            String::from("gpt_oss.cuda_kv_cache"),
        ],
        logical_page_tokens_default: 16,
        owner_classes: vec![
            String::from("request"),
            String::from("session"),
            String::from("shared_prefix"),
        ],
        spill_policies: vec![
            String::from("refuse_new_pages"),
            String::from("evict_oldest_pages"),
            String::from("spill_to_host"),
        ],
        residency_tiers: vec![
            String::from("device"),
            String::from("host"),
            String::from("distributed"),
        ],
        residency_movements: vec![
            String::from("prefetch"),
            String::from("write_back"),
            String::from("spill"),
            String::from("restore"),
        ],
        block_manager_features: vec![
            PsionRvllmPagedKvFeature {
                feature_id: String::from("logical_page_layout"),
                current_posture: String::from(
                    "KvCachePageLayout and KvCacheState remain the canonical visible geometry surface",
                ),
            },
            PsionRvllmPagedKvFeature {
                feature_id: String::from("request_owned_growth_delta"),
                current_posture: String::from(
                    "KvCacheOwnershipAccounting keeps before/after growth and scheduler binding explicit",
                ),
            },
            PsionRvllmPagedKvFeature {
                feature_id: String::from("owner_bound_eviction_and_reclaim"),
                current_posture: String::from(
                    "owner-bound page eviction and reclaim are explicit instead of hidden inside opaque cache heuristics",
                ),
            },
            PsionRvllmPagedKvFeature {
                feature_id: String::from("host_device_residency_accounting"),
                current_posture: String::from(
                    "host and device residency tiers plus refusal reasons are serialized directly in runtime evidence",
                ),
            },
        ],
        retained_tests: vec![
            String::from("paged_kv_cache_tracks_growth_refill_and_refusal"),
            String::from("paged_kv_cache_tracks_owner_bound_page_eviction_and_reclaim"),
            String::from("paged_kv_cache_predicts_device_resident_growth_from_empty_seed"),
            String::from("paged_kv_cache_predicts_device_resident_growth_from_existing_seed"),
            String::from("host_device_kv_residency_reports_prefetch_writeback_and_refusal"),
        ],
        benchmark_paths: vec![
            String::from("crates/psionic-runtime/src/lib.rs"),
            String::from("crates/psionic-serve/src/lib.rs"),
            String::from("crates/psionic-serve/src/gpt_oss.rs"),
        ],
        stability_posture: String::from(
            "Psionic already exposes a real paged-KV manager with visible page geometry, ownership accounting, spill policy, residency movement, and refusal posture. This packet closes the block-manager reporting seam without replacing that explicit evidence model with hidden heuristics.",
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
        serde_json::to_vec(value).expect("psion rvllm paged kv manager packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmPagedKvManagerPacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_paged_kv_manager_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_paged_kv_manager_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
