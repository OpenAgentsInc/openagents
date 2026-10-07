use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_ir::{
    TASSADAR_MULTI_MEMORY_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
    TASSADAR_MULTI_MEMORY_PROFILE_ID,
};

const CONTRACT_SCHEMA_VERSION: u16 = 1;

/// Expected lowering status for one bounded multi-memory routing case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMultiMemoryLoweringStatus {
    Exact,
    Refusal,
}

/// One compiler-owned case specification in the bounded multi-memory profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryLoweringCaseSpec {
    pub case_id: String,
    pub topology_id: String,
    pub memory_route_ids: Vec<String>,
    pub expected_status: TassadarMultiMemoryLoweringStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Public compiler-owned contract for the bounded multi-memory profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryProfileCompilationContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub case_specs: Vec<TassadarMultiMemoryLoweringCaseSpec>,
    pub claim_boundary: String,
    pub summary: String,
    pub contract_digest: String,
}

impl TassadarMultiMemoryProfileCompilationContract {
    fn new(case_specs: Vec<TassadarMultiMemoryLoweringCaseSpec>) -> Self {
        let exact_case_count = case_specs
            .iter()
            .filter(|case| case.expected_status == TassadarMultiMemoryLoweringStatus::Exact)
            .count();
        let refusal_case_count = case_specs
            .iter()
            .filter(|case| case.expected_status == TassadarMultiMemoryLoweringStatus::Refusal)
            .count();
        let mut contract = Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            contract_id: String::from("tassadar.multi_memory_profile.compilation_contract.v1"),
            profile_id: String::from(TASSADAR_MULTI_MEMORY_PROFILE_ID),
            portability_envelope_id: String::from(
                TASSADAR_MULTI_MEMORY_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
            ),
            case_specs,
            claim_boundary: String::from(
                "this contract freezes one bounded multi-memory routing profile over two declared topology families plus typed malformed-topology refusal. It does not claim arbitrary Wasm multi-memory closure, memory64 plus multi-memory mixing, or broader served publication",
            ),
            summary: String::new(),
            contract_digest: String::new(),
        };
        contract.summary = format!(
            "Multi-memory profile compilation contract freezes {} cases across {} exact and {} refusal expectations.",
            contract.case_specs.len(),
            exact_case_count,
            refusal_case_count,
        );
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_multi_memory_profile_compilation_contract|",
            &contract,
        );
        contract
    }
}

/// Returns the canonical compiler-owned multi-memory routing contract.
#[must_use]
pub fn compile_tassadar_multi_memory_profile_contract(
) -> TassadarMultiMemoryProfileCompilationContract {
    TassadarMultiMemoryProfileCompilationContract::new(vec![
        case_spec(
            "rodata_heap_output_route",
            "rodata_heap_output_split",
            &[
                "memory0_rodata_readonly",
                "memory1_heap_output_mutable",
                "cross_memory_copy_to_output",
            ],
            TassadarMultiMemoryLoweringStatus::Exact,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_memory_abi_v2_report.json",
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
            ],
            "the bounded multi-memory lane admits one split rodata/output topology with explicit route ownership instead of flattening both memories into a single buffer",
        ),
        case_spec(
            "scratch_heap_checkpoint_route",
            "scratch_heap_checkpoint_split",
            &[
                "memory0_scratch_mutable",
                "memory1_heap_mutable",
                "per_memory_checkpoint_order",
            ],
            TassadarMultiMemoryLoweringStatus::Exact,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
            ],
            "the bounded multi-memory lane admits one scratch-plus-heap topology with explicit per-memory checkpoint and replay ordering",
        ),
        case_spec(
            "malformed_memory_topology_refusal",
            "invalid_duplicate_memory_owner",
            &["memory_owner_collision", "unmapped_output_memory"],
            TassadarMultiMemoryLoweringStatus::Refusal,
            Some("malformed_memory_topology"),
            &[
                "fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json",
                "fixtures/tassadar/reports/tassadar_memory64_profile_report.json",
            ],
            "malformed memory-owner or route topology stays as typed refusal truth instead of being inferred from the green bounded cases",
        ),
    ])
}

fn case_spec(
    case_id: &str,
    topology_id: &str,
    memory_route_ids: &[&str],
    expected_status: TassadarMultiMemoryLoweringStatus,
    expected_refusal_reason_id: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarMultiMemoryLoweringCaseSpec {
    TassadarMultiMemoryLoweringCaseSpec {
        case_id: String::from(case_id),
        topology_id: String::from(topology_id),
        memory_route_ids: memory_route_ids
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
        TassadarMultiMemoryLoweringStatus, compile_tassadar_multi_memory_profile_contract,
    };

    #[test]
    fn multi_memory_profile_contract_is_machine_legible() {
        let contract = compile_tassadar_multi_memory_profile_contract();

        assert_eq!(contract.case_specs.len(), 3);
        assert!(contract.case_specs.iter().any(|case| {
            case.topology_id == "scratch_heap_checkpoint_split"
                && case.expected_status == TassadarMultiMemoryLoweringStatus::Exact
        }));
        assert!(contract.case_specs.iter().any(|case| {
            case.expected_status == TassadarMultiMemoryLoweringStatus::Refusal
                && case.expected_refusal_reason_id.as_deref()
                    == Some("malformed_memory_topology")
        }));
    }
}
