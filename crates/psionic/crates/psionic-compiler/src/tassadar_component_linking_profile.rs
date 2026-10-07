use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_ir::{
    TASSADAR_COMPONENT_LINKING_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
    TASSADAR_COMPONENT_LINKING_PROFILE_ID,
};

const CONTRACT_SCHEMA_VERSION: u16 = 1;

/// Expected lowering status for one bounded component-linking case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarComponentLinkingLoweringStatus {
    Exact,
    Refusal,
}

/// One compiler-owned case specification in the bounded component-linking profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingCaseSpec {
    pub case_id: String,
    pub topology_id: String,
    pub interface_type_ids: Vec<String>,
    pub expected_status: TassadarComponentLinkingLoweringStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Public compiler-owned contract for the bounded component-linking profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingProfileCompilationContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub case_specs: Vec<TassadarComponentLinkingCaseSpec>,
    pub claim_boundary: String,
    pub summary: String,
    pub contract_digest: String,
}

impl TassadarComponentLinkingProfileCompilationContract {
    fn new(case_specs: Vec<TassadarComponentLinkingCaseSpec>) -> Self {
        let exact_case_count = case_specs
            .iter()
            .filter(|case| case.expected_status == TassadarComponentLinkingLoweringStatus::Exact)
            .count();
        let refusal_case_count = case_specs
            .iter()
            .filter(|case| case.expected_status == TassadarComponentLinkingLoweringStatus::Refusal)
            .count();
        let mut contract = Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            contract_id: String::from(
                "tassadar.component_linking_profile.compilation_contract.v1",
            ),
            profile_id: String::from(TASSADAR_COMPONENT_LINKING_PROFILE_ID),
            portability_envelope_id: String::from(
                TASSADAR_COMPONENT_LINKING_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
            ),
            case_specs,
            claim_boundary: String::from(
                "this contract freezes one bounded component/linking proposal profile over explicit interface-type lowering and incompatible-interface refusal. It does not claim arbitrary component-model closure, unrestricted interface lowering, or broader served publication",
            ),
            summary: String::new(),
            contract_digest: String::new(),
        };
        contract.summary = format!(
            "Component-linking compilation contract freezes {} cases across {} exact and {} refusal expectations.",
            contract.case_specs.len(),
            exact_case_count,
            refusal_case_count,
        );
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_component_linking_profile_compilation_contract|",
            &contract,
        );
        contract
    }
}

/// Returns the canonical compiler-owned component/linking proposal profile contract.
#[must_use]
pub fn compile_tassadar_component_linking_profile_contract(
) -> TassadarComponentLinkingProfileCompilationContract {
    TassadarComponentLinkingProfileCompilationContract::new(vec![
        case_spec(
            "utf8_decode_writer_component_pair",
            "utf8_decode_writer_component_pair",
            &["list_u8", "result_i32"],
            TassadarComponentLinkingLoweringStatus::Exact,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
                "fixtures/tassadar/reports/tassadar_linked_program_bundle_eval_report.json",
            ],
            "the bounded component lane lowers list-of-bytes input plus result<i32> output across one explicit decode/writer component pair",
        ),
        case_spec(
            "checkpoint_resume_component_pair",
            "checkpoint_resume_component_pair",
            &["record_i32_i32", "result_refusal_code"],
            TassadarComponentLinkingLoweringStatus::Exact,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
                "fixtures/tassadar/reports/tassadar_effect_safe_resume_report.json",
            ],
            "the bounded component lane lowers a fixed-width record interface plus result-coded refusals across one explicit checkpoint/resume component pair",
        ),
        case_spec(
            "incompatible_component_interface_refusal",
            "utf8_decode_writer_component_pair",
            &["record_i32_i32", "list_u8"],
            TassadarComponentLinkingLoweringStatus::Refusal,
            Some("incompatible_component_interface"),
            &[
                "fixtures/tassadar/reports/tassadar_module_link_eval_report.json",
                "fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json",
            ],
            "incompatible interface-type wiring stays as typed refusal truth instead of widening from the two admitted component pairs",
        ),
    ])
}

fn case_spec(
    case_id: &str,
    topology_id: &str,
    interface_type_ids: &[&str],
    expected_status: TassadarComponentLinkingLoweringStatus,
    expected_refusal_reason_id: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarComponentLinkingCaseSpec {
    TassadarComponentLinkingCaseSpec {
        case_id: String::from(case_id),
        topology_id: String::from(topology_id),
        interface_type_ids: interface_type_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
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
        TassadarComponentLinkingLoweringStatus,
        compile_tassadar_component_linking_profile_contract,
    };

    #[test]
    fn component_linking_profile_contract_is_machine_legible() {
        let contract = compile_tassadar_component_linking_profile_contract();

        assert_eq!(contract.case_specs.len(), 3);
        assert!(contract.case_specs.iter().any(|case| {
            case.topology_id == "checkpoint_resume_component_pair"
                && case.expected_status == TassadarComponentLinkingLoweringStatus::Exact
        }));
        assert!(contract.case_specs.iter().any(|case| {
            case.expected_status == TassadarComponentLinkingLoweringStatus::Refusal
                && case.expected_refusal_reason_id.as_deref()
                    == Some("incompatible_component_interface")
        }));
    }
}
