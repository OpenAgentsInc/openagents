use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_ir::{
    TASSADAR_INTERNAL_COMPONENT_ABI_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
    TASSADAR_INTERNAL_COMPONENT_ABI_PROFILE_ID,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComponentAbiLoweringStatus {
    Exact,
    Refusal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentAbiCaseSpec {
    pub case_id: String,
    pub interface_id: String,
    pub component_graph_id: String,
    pub expected_status: TassadarInternalComponentAbiLoweringStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentAbiCompilationContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub case_specs: Vec<TassadarInternalComponentAbiCaseSpec>,
    pub claim_boundary: String,
    pub summary: String,
    pub contract_digest: String,
}

impl TassadarInternalComponentAbiCompilationContract {
    fn new(case_specs: Vec<TassadarInternalComponentAbiCaseSpec>) -> Self {
        let exact_case_count = case_specs
            .iter()
            .filter(|case| case.expected_status == TassadarInternalComponentAbiLoweringStatus::Exact)
            .count();
        let refusal_case_count = case_specs
            .iter()
            .filter(|case| {
                case.expected_status == TassadarInternalComponentAbiLoweringStatus::Refusal
            })
            .count();
        let mut contract = Self {
            schema_version: 1,
            contract_id: String::from("tassadar.internal_component_abi.compilation_contract.v1"),
            profile_id: String::from(TASSADAR_INTERNAL_COMPONENT_ABI_PROFILE_ID),
            portability_envelope_id: String::from(
                TASSADAR_INTERNAL_COMPONENT_ABI_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
            ),
            case_specs,
            claim_boundary: String::from(
                "this contract freezes one bounded internal-compute component-model ABI lane over explicit interface-type contracts and typed refusal on handle mismatches and unsupported union shapes. It does not claim arbitrary component-model closure, arbitrary host-import composition, or broader served publication",
            ),
            summary: String::new(),
            contract_digest: String::new(),
        };
        contract.summary = format!(
            "Internal component ABI compilation contract freezes {} cases across {} exact and {} refusal expectations.",
            contract.case_specs.len(),
            exact_case_count,
            refusal_case_count,
        );
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_internal_component_abi_compilation_contract|",
            &contract,
        );
        contract
    }
}

#[must_use]
pub fn compile_tassadar_internal_component_abi_contract(
) -> TassadarInternalComponentAbiCompilationContract {
    TassadarInternalComponentAbiCompilationContract::new(vec![
        case_spec(
            "session_checkpoint_counter_stack",
            "session_counter_checkpoint_v1",
            "session_checkpoint_counter_stack",
            TassadarInternalComponentAbiLoweringStatus::Exact,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_session_process_profile_report.json",
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
            ],
            "the bounded internal component ABI composes session-loop state updates with checkpoint handles and durable snapshot refs instead of widening to arbitrary session/plugin composition",
        ),
        case_spec(
            "artifact_retry_reader_stack",
            "artifact_reader_retry_job_v1",
            "artifact_retry_reader_stack",
            TassadarInternalComponentAbiLoweringStatus::Exact,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_virtual_fs_mount_profile_report.json",
                "fixtures/tassadar/reports/tassadar_async_lifecycle_profile_report.json",
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
            ],
            "the bounded internal component ABI composes artifact-bound reads, retry budgets, and job-dispatch surfaces into one typed software-facing graph",
        ),
        case_spec(
            "spill_resume_adapter_stack",
            "spill_resume_adapter_v1",
            "spill_resume_adapter_stack",
            TassadarInternalComponentAbiLoweringStatus::Exact,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
                "fixtures/tassadar/reports/tassadar_effect_safe_resume_report.json",
            ],
            "the bounded internal component ABI composes spill-backed continuation handles with the current resumable runtime surfaces into one typed memory-window adapter lane",
        ),
        case_spec(
            "cross_profile_handle_mismatch_refusal",
            "session_counter_checkpoint_v1",
            "session_checkpoint_counter_stack",
            TassadarInternalComponentAbiLoweringStatus::Refusal,
            Some("cross_profile_handle_mismatch"),
            &[
                "fixtures/tassadar/reports/tassadar_component_linking_profile_report.json",
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
            ],
            "cross-profile snapshot and continuation handles stay as typed refusal truth instead of silently widening component compatibility across unrelated lanes",
        ),
        case_spec(
            "unsupported_variant_union_refusal",
            "artifact_reader_retry_job_v1",
            "artifact_retry_reader_stack",
            TassadarInternalComponentAbiLoweringStatus::Refusal,
            Some("unsupported_variant_union_shape"),
            &[
                "fixtures/tassadar/reports/tassadar_component_linking_profile_report.json",
                "fixtures/tassadar/reports/tassadar_virtual_fs_mount_profile_report.json",
            ],
            "variant-union payloads remain explicit refusal truth so this lane does not imply general interface-union lowering or arbitrary plugin discovery",
        ),
    ])
}

fn case_spec(
    case_id: &str,
    interface_id: &str,
    component_graph_id: &str,
    expected_status: TassadarInternalComponentAbiLoweringStatus,
    expected_refusal_reason_id: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarInternalComponentAbiCaseSpec {
    TassadarInternalComponentAbiCaseSpec {
        case_id: String::from(case_id),
        interface_id: String::from(interface_id),
        component_graph_id: String::from(component_graph_id),
        expected_status,
        expected_refusal_reason_id: expected_refusal_reason_id.map(String::from),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
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
        TassadarInternalComponentAbiLoweringStatus, compile_tassadar_internal_component_abi_contract,
    };

    #[test]
    fn internal_component_abi_compilation_contract_is_machine_legible() {
        let contract = compile_tassadar_internal_component_abi_contract();

        assert_eq!(contract.case_specs.len(), 5);
        assert!(contract.case_specs.iter().any(|case| {
            case.interface_id == "spill_resume_adapter_v1"
                && case.expected_status == TassadarInternalComponentAbiLoweringStatus::Exact
        }));
        assert!(contract.case_specs.iter().any(|case| {
            case.expected_status == TassadarInternalComponentAbiLoweringStatus::Refusal
                && case.expected_refusal_reason_id.as_deref()
                    == Some("unsupported_variant_union_shape")
        }));
    }
}
