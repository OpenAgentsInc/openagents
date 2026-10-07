use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_MULTI_MEMORY_PROFILE_ID: &str =
    "tassadar.proposal_profile.multi_memory_routing.v1";
pub const TASSADAR_MULTI_MEMORY_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID: &str =
    "cpu_reference_current_host";

/// One admitted topology in the bounded multi-memory routing profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryTopologySpec {
    pub topology_id: String,
    pub memory_ids: Vec<String>,
    pub admitted_routing_semantic_ids: Vec<String>,
    pub checkpoint_semantic_ids: Vec<String>,
    pub detail: String,
}

/// Public contract for the bounded multi-memory routing profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryProfileContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub admitted_topologies: Vec<TassadarMultiMemoryTopologySpec>,
    pub refused_reason_ids: Vec<String>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl TassadarMultiMemoryProfileContract {
    fn new() -> Self {
        let mut contract = Self {
            schema_version: 1,
            contract_id: String::from("tassadar.multi_memory_profile.contract.v1"),
            profile_id: String::from(TASSADAR_MULTI_MEMORY_PROFILE_ID),
            portability_envelope_id: String::from(
                TASSADAR_MULTI_MEMORY_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
            ),
            admitted_topologies: vec![
                TassadarMultiMemoryTopologySpec {
                    topology_id: String::from("rodata_heap_output_split"),
                    memory_ids: vec![String::from("rodata"), String::from("heap_output")],
                    admitted_routing_semantic_ids: vec![
                        String::from("readonly_memory_0"),
                        String::from("mutable_memory_1"),
                        String::from("cross_memory_copy_to_output"),
                    ],
                    checkpoint_semantic_ids: Vec::new(),
                    detail: String::from(
                        "one bounded split-memory topology keeps immutable lookup bytes in memory 0 and mutable output state in memory 1 instead of collapsing routing into a single-memory abstraction",
                    ),
                },
                TassadarMultiMemoryTopologySpec {
                    topology_id: String::from("scratch_heap_checkpoint_split"),
                    memory_ids: vec![String::from("scratch"), String::from("heap")],
                    admitted_routing_semantic_ids: vec![
                        String::from("scratch_memory_0"),
                        String::from("heap_memory_1"),
                        String::from("stable_memory_owner_ids"),
                    ],
                    checkpoint_semantic_ids: vec![
                        String::from("per_memory_checkpoint_order"),
                        String::from("per_memory_digest_replay"),
                    ],
                    detail: String::from(
                        "one bounded scratch-plus-heap topology keeps memory ownership, replay order, and checkpoint lineage explicit across two memories",
                    ),
                },
            ],
            refused_reason_ids: vec![
                String::from("malformed_memory_topology"),
                String::from("memory_route_alias_violation"),
                String::from("memory64_multi_memory_mix_out_of_scope"),
            ],
            claim_boundary: String::from(
                "this contract names one bounded multi-memory routing profile over two explicit topology families on the current-host cpu-reference lane. It does not claim arbitrary Wasm multi-memory closure, memory64 plus multi-memory mixing, generic allocator portability, or broader served publication",
            ),
            contract_digest: String::new(),
        };
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_multi_memory_profile_contract|", &contract);
        contract
    }
}

/// Returns the canonical bounded multi-memory routing profile contract.
#[must_use]
pub fn tassadar_multi_memory_profile_contract() -> TassadarMultiMemoryProfileContract {
    TassadarMultiMemoryProfileContract::new()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_MULTI_MEMORY_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        TASSADAR_MULTI_MEMORY_PROFILE_ID, tassadar_multi_memory_profile_contract,
    };

    #[test]
    fn multi_memory_profile_contract_is_machine_legible() {
        let contract = tassadar_multi_memory_profile_contract();

        assert_eq!(contract.profile_id, TASSADAR_MULTI_MEMORY_PROFILE_ID);
        assert_eq!(
            contract.portability_envelope_id,
            TASSADAR_MULTI_MEMORY_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID
        );
        assert_eq!(contract.admitted_topologies.len(), 2);
        assert!(contract
            .admitted_topologies
            .iter()
            .any(|topology| topology.topology_id == "scratch_heap_checkpoint_split"
                && topology.checkpoint_semantic_ids
                    .contains(&String::from("per_memory_checkpoint_order"))));
        assert!(contract
            .refused_reason_ids
            .contains(&String::from("malformed_memory_topology")));
    }
}
