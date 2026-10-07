use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PSION_RVLLM_KV_EVICTION_REUSE_SCHEMA_VERSION: &str =
    "psion.rvllm_kv_eviction_reuse.v1";
pub const PSION_RVLLM_KV_EVICTION_REUSE_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_rvllm_kv_eviction_reuse_v1.json";
pub const PSION_RVLLM_KV_EVICTION_REUSE_DOC_PATH: &str =
    "docs/PSION_RVLLM_KV_EVICTION_REUSE.md";

const PACKET_ID: &str = "psion_rvllm_kv_eviction_reuse_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmKvEvictionStrategy {
    pub strategy_id: String,
    pub current_posture: String,
    pub explicit_surface: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmKvReuseStressRow {
    pub scenario_id: String,
    pub max_context_tokens: usize,
    pub tokens_per_page: usize,
    pub appended_tokens: usize,
    pub peak_live_pages_before: usize,
    pub peak_live_pages_after: usize,
    pub max_page_index_before: usize,
    pub max_page_index_after: usize,
    pub reuse_hits_before: usize,
    pub reuse_hits_after: usize,
    pub final_cached_tokens: usize,
    pub tail_correctness_retained: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionRvllmKvEvictionReusePacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_scope: Vec<String>,
    pub eviction_strategies: Vec<PsionRvllmKvEvictionStrategy>,
    pub reuse_strategies: Vec<PsionRvllmKvEvictionStrategy>,
    pub long_context_stress_rows: Vec<PsionRvllmKvReuseStressRow>,
    pub retained_tests: Vec<String>,
    pub claim_boundary: String,
    pub packet_digest: String,
}

impl PsionRvllmKvEvictionReusePacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.packet_digest.clear();
        stable_digest(b"psion_rvllm_kv_eviction_reuse_packet|", &canonical)
    }
}

#[must_use]
pub fn builtin_psion_rvllm_kv_eviction_reuse_packet() -> PsionRvllmKvEvictionReusePacket {
    let mut packet = PsionRvllmKvEvictionReusePacket {
        schema_version: String::from(PSION_RVLLM_KV_EVICTION_REUSE_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_scope: vec![
            String::from("psionic_runtime.kv_cache_policy"),
            String::from("psionic_runtime.kv_cache_ownership_accounting"),
            String::from("psionic_serve.inmemory_kv_cache"),
        ],
        eviction_strategies: vec![
            PsionRvllmKvEvictionStrategy {
                strategy_id: String::from("evict_oldest_pages"),
                current_posture: String::from(
                    "bounded long-context growth still uses explicit oldest-page eviction instead of hidden attention-aware heuristics",
                ),
                explicit_surface: vec![
                    String::from("KvCacheSpillPolicy::EvictOldestPages"),
                    String::from("KvCacheOwnershipAccounting.reclaimed_pages"),
                    String::from("InMemoryKvCache::evict_oldest_page"),
                ],
            },
            PsionRvllmKvEvictionStrategy {
                strategy_id: String::from("truncate_then_refill"),
                current_posture: String::from(
                    "explicit truncation continues to reclaim visible pages before later refill growth",
                ),
                explicit_surface: vec![
                    String::from("InMemoryKvCache::truncate"),
                    String::from("KvCacheOwnershipAccounting.reclaimed_pages"),
                ],
            },
        ],
        reuse_strategies: vec![
            PsionRvllmKvEvictionStrategy {
                strategy_id: String::from("reclaim_page_index_reuse"),
                current_posture: String::from(
                    "fully reclaimed logical page indices now re-enter a deterministic reuse pool instead of growing monotonically forever",
                ),
                explicit_surface: vec![
                    String::from("InMemoryKvCache::reusable_page_indices"),
                    String::from("KvCacheOwnershipAccounting.reused_pages"),
                ],
            },
            PsionRvllmKvEvictionStrategy {
                strategy_id: String::from("predictive_growth_reuse"),
                current_posture: String::from(
                    "device-resident growth prediction now reports when future growth can reuse reclaimed pages instead of assuming every page is cold",
                ),
                explicit_surface: vec![
                    String::from("InMemoryKvCache::ownership_since_with_device_tokens"),
                    String::from("KvCacheOwnershipAccounting.reused_pages"),
                ],
            },
        ],
        long_context_stress_rows: vec![
            PsionRvllmKvReuseStressRow {
                scenario_id: String::from("session_ring_18_tokens"),
                max_context_tokens: 6,
                tokens_per_page: 2,
                appended_tokens: 18,
                peak_live_pages_before: 3,
                peak_live_pages_after: 3,
                max_page_index_before: 8,
                max_page_index_after: 2,
                reuse_hits_before: 0,
                reuse_hits_after: 6,
                final_cached_tokens: 6,
                tail_correctness_retained: true,
            },
            PsionRvllmKvReuseStressRow {
                scenario_id: String::from("truncate_refill_window"),
                max_context_tokens: 6,
                tokens_per_page: 2,
                appended_tokens: 6,
                peak_live_pages_before: 2,
                peak_live_pages_after: 2,
                max_page_index_before: 2,
                max_page_index_after: 1,
                reuse_hits_before: 0,
                reuse_hits_after: 1,
                final_cached_tokens: 4,
                tail_correctness_retained: true,
            },
        ],
        retained_tests: vec![
            String::from("paged_kv_cache_tracks_owner_bound_page_eviction_and_reclaim"),
            String::from("paged_kv_cache_reuses_reclaimed_page_indices_under_long_context_stress"),
            String::from("paged_kv_cache_predicts_reused_page_growth_from_existing_reclaim"),
            String::from("host_device_kv_residency_reports_prefetch_writeback_and_refusal"),
        ],
        claim_boundary: String::from(
            "This packet claims only explicit bounded eviction and reclaimed-page reuse under the existing paged-KV manager. It does not claim hidden semantic eviction, unbounded long-context scaling, or a general swap daemon beyond the current visible spill and residency surfaces.",
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
            .expect("psion rvllm kv eviction and reuse packet should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_packet_matches_committed_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let expected: PsionRvllmKvEvictionReusePacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_rvllm_kv_eviction_reuse_v1.json"
        ))?;
        let packet = builtin_psion_rvllm_kv_eviction_reuse_packet();
        assert_eq!(packet, expected);
        assert_eq!(packet.packet_digest, packet.stable_digest());
        Ok(())
    }
}
