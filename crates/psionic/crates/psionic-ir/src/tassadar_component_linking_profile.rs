use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_COMPONENT_LINKING_PROFILE_ID: &str =
    "tassadar.proposal_profile.component_linking_interface_types.v1";
pub const TASSADAR_COMPONENT_LINKING_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID: &str =
    "cpu_reference_current_host";

/// One admitted interface-lowering topology in the bounded component profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingTopologySpec {
    pub topology_id: String,
    pub component_refs: Vec<String>,
    pub admitted_interface_type_ids: Vec<String>,
    pub detail: String,
}

/// Public contract for the bounded component/linking proposal profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingProfileContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub admitted_topologies: Vec<TassadarComponentLinkingTopologySpec>,
    pub refused_reason_ids: Vec<String>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl TassadarComponentLinkingProfileContract {
    fn new() -> Self {
        let mut contract = Self {
            schema_version: 1,
            contract_id: String::from("tassadar.component_linking_profile.contract.v1"),
            profile_id: String::from(TASSADAR_COMPONENT_LINKING_PROFILE_ID),
            portability_envelope_id: String::from(
                TASSADAR_COMPONENT_LINKING_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
            ),
            admitted_topologies: vec![
                TassadarComponentLinkingTopologySpec {
                    topology_id: String::from("utf8_decode_writer_component_pair"),
                    component_refs: vec![
                        String::from("utf8_decode_component@1.0.0"),
                        String::from("heap_writer_component@1.0.0"),
                    ],
                    admitted_interface_type_ids: vec![
                        String::from("list_u8"),
                        String::from("result_i32"),
                    ],
                    detail: String::from(
                        "one bounded component pair lowers `list<u8>` input bytes into a decode component and then lowers the result into one heap-writer component without widening to arbitrary interface graphs",
                    ),
                },
                TassadarComponentLinkingTopologySpec {
                    topology_id: String::from("checkpoint_resume_component_pair"),
                    component_refs: vec![
                        String::from("checkpoint_codec_component@1.0.0"),
                        String::from("resume_runtime_component@1.0.0"),
                    ],
                    admitted_interface_type_ids: vec![
                        String::from("record_i32_i32"),
                        String::from("result_refusal_code"),
                    ],
                    detail: String::from(
                        "one bounded checkpoint/resume component pair lowers fixed-width record interfaces and result-coded refusal semantics into the existing continuation lane",
                    ),
                },
            ],
            refused_reason_ids: vec![
                String::from("incompatible_component_interface"),
                String::from("unsupported_interface_type_lowering"),
                String::from("component_cycle_out_of_scope"),
            ],
            claim_boundary: String::from(
                "this contract names one bounded component/linking profile with explicit interface-type lowering on the current-host cpu-reference lane. It does not claim arbitrary component-model closure, unrestricted interface-type lowering, arbitrary host-import composition, or broader served publication",
            ),
            contract_digest: String::new(),
        };
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_component_linking_profile_contract|",
            &contract,
        );
        contract
    }
}

/// Returns the canonical bounded component/linking profile contract.
#[must_use]
pub fn tassadar_component_linking_profile_contract() -> TassadarComponentLinkingProfileContract {
    TassadarComponentLinkingProfileContract::new()
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
        TASSADAR_COMPONENT_LINKING_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        TASSADAR_COMPONENT_LINKING_PROFILE_ID, tassadar_component_linking_profile_contract,
    };

    #[test]
    fn component_linking_profile_contract_is_machine_legible() {
        let contract = tassadar_component_linking_profile_contract();

        assert_eq!(contract.profile_id, TASSADAR_COMPONENT_LINKING_PROFILE_ID);
        assert_eq!(
            contract.portability_envelope_id,
            TASSADAR_COMPONENT_LINKING_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID
        );
        assert_eq!(contract.admitted_topologies.len(), 2);
        assert!(contract.admitted_topologies.iter().any(|topology| {
            topology.topology_id == "checkpoint_resume_component_pair"
                && topology
                    .admitted_interface_type_ids
                    .contains(&String::from("record_i32_i32"))
        }));
        assert!(contract
            .refused_reason_ids
            .contains(&String::from("incompatible_component_interface")));
    }
}
