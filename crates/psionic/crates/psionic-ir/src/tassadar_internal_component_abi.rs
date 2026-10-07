use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_INTERNAL_COMPONENT_ABI_PROFILE_ID: &str =
    "tassadar.internal_compute.component_model_abi.v1";
pub const TASSADAR_INTERNAL_COMPONENT_ABI_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID: &str =
    "cpu_reference_current_host";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentInterfaceContract {
    pub interface_id: String,
    pub component_role_ids: Vec<String>,
    pub input_type_ids: Vec<String>,
    pub output_type_ids: Vec<String>,
    pub compatibility_class: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentAbiContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub admitted_interfaces: Vec<TassadarInternalComponentInterfaceContract>,
    pub refused_reason_ids: Vec<String>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl TassadarInternalComponentAbiContract {
    fn new() -> Self {
        let mut contract = Self {
            schema_version: 1,
            contract_id: String::from("tassadar.internal_component_abi.contract.v1"),
            profile_id: String::from(TASSADAR_INTERNAL_COMPONENT_ABI_PROFILE_ID),
            portability_envelope_id: String::from(
                TASSADAR_INTERNAL_COMPONENT_ABI_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
            ),
            admitted_interfaces: vec![
                TassadarInternalComponentInterfaceContract {
                    interface_id: String::from("session_counter_checkpoint_v1"),
                    component_role_ids: vec![
                        String::from("session_loop"),
                        String::from("checkpoint_codec"),
                        String::from("snapshot_store"),
                    ],
                    input_type_ids: vec![
                        String::from("record_session_delta_v1"),
                        String::from("option_process_snapshot_ref_v1"),
                    ],
                    output_type_ids: vec![
                        String::from("record_counter_value_v1"),
                        String::from("result_process_snapshot_ref_v1"),
                    ],
                    compatibility_class: String::from("stateful_process_interface"),
                    detail: String::from(
                        "one bounded internal component ABI admits session-loop state updates that can checkpoint through the durable process-object lane without widening to arbitrary interface unions",
                    ),
                },
                TassadarInternalComponentInterfaceContract {
                    interface_id: String::from("artifact_reader_retry_job_v1"),
                    component_role_ids: vec![
                        String::from("artifact_reader"),
                        String::from("retry_scheduler"),
                        String::from("job_dispatch"),
                    ],
                    input_type_ids: vec![
                        String::from("record_artifact_mount_ref_v1"),
                        String::from("record_retry_budget_v1"),
                    ],
                    output_type_ids: vec![
                        String::from("result_read_batch_v1"),
                        String::from("record_refusal_code_v1"),
                    ],
                    compatibility_class: String::from("artifact_bound_async_interface"),
                    detail: String::from(
                        "one bounded internal component ABI admits artifact-bound reads with bounded retry metadata and typed refusal codes instead of widening to arbitrary async callbacks or host imports",
                    ),
                },
                TassadarInternalComponentInterfaceContract {
                    interface_id: String::from("spill_resume_adapter_v1"),
                    component_role_ids: vec![
                        String::from("spill_loader"),
                        String::from("resume_runtime"),
                        String::from("memory_window_adapter"),
                    ],
                    input_type_ids: vec![
                        String::from("record_spill_segment_refs_v1"),
                        String::from("record_tape_cursor_v1"),
                    ],
                    output_type_ids: vec![
                        String::from("result_resume_token_v1"),
                        String::from("record_memory_window_v1"),
                    ],
                    compatibility_class: String::from("continuation_handle_interface"),
                    detail: String::from(
                        "one bounded internal component ABI admits spill-backed continuation handles and memory-window adapters without widening to arbitrary persistent heaps or generic external storage",
                    ),
                },
            ],
            refused_reason_ids: vec![
                String::from("cross_profile_handle_mismatch"),
                String::from("unsupported_variant_union_shape"),
                String::from("ambient_host_capability_handle"),
            ],
            claim_boundary: String::from(
                "this contract names one bounded internal-compute component-model ABI lane with explicit interface-type contracts on the current-host cpu-reference portability envelope. It does not claim arbitrary component-model closure, arbitrary host-import composition, or broader served publication",
            ),
            contract_digest: String::new(),
        };
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_internal_component_abi_contract|", &contract);
        contract
    }
}

#[must_use]
pub fn tassadar_internal_component_abi_contract() -> TassadarInternalComponentAbiContract {
    TassadarInternalComponentAbiContract::new()
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
        TASSADAR_INTERNAL_COMPONENT_ABI_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        TASSADAR_INTERNAL_COMPONENT_ABI_PROFILE_ID, tassadar_internal_component_abi_contract,
    };

    #[test]
    fn internal_component_abi_contract_is_machine_legible() {
        let contract = tassadar_internal_component_abi_contract();

        assert_eq!(contract.profile_id, TASSADAR_INTERNAL_COMPONENT_ABI_PROFILE_ID);
        assert_eq!(
            contract.portability_envelope_id,
            TASSADAR_INTERNAL_COMPONENT_ABI_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID
        );
        assert_eq!(contract.admitted_interfaces.len(), 3);
        assert!(contract
            .admitted_interfaces
            .iter()
            .any(|interface| interface.interface_id == "artifact_reader_retry_job_v1"));
        assert!(contract
            .refused_reason_ids
            .contains(&String::from("cross_profile_handle_mismatch")));
    }
}
