use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_runtime::{
    TASSADAR_MEMORY64_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID, TASSADAR_MEMORY64_PROFILE_ID,
};

const CONTRACT_SCHEMA_VERSION: u16 = 1;

/// Expected lowering status for one bounded memory64 case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMemory64LoweringStatus {
    Exact,
    Refusal,
}

/// One compiler-owned case specification for the bounded memory64 profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemory64LoweringCaseSpec {
    pub case_id: String,
    pub workload_family: String,
    pub address_shape_id: String,
    pub max_virtual_address_touched: u64,
    pub expected_status: TassadarMemory64LoweringStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Public compiler-owned contract for the bounded memory64 profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemory64ProfileContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub max_virtual_address_bits: u8,
    pub case_specs: Vec<TassadarMemory64LoweringCaseSpec>,
    pub claim_boundary: String,
    pub summary: String,
    pub contract_digest: String,
}

impl TassadarMemory64ProfileContract {
    fn new(case_specs: Vec<TassadarMemory64LoweringCaseSpec>) -> Self {
        let exact_case_count = case_specs
            .iter()
            .filter(|case| case.expected_status == TassadarMemory64LoweringStatus::Exact)
            .count();
        let refusal_case_count = case_specs
            .iter()
            .filter(|case| case.expected_status == TassadarMemory64LoweringStatus::Refusal)
            .count();
        let mut contract = Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            contract_id: String::from("tassadar.memory64_profile.contract.v1"),
            profile_id: String::from(TASSADAR_MEMORY64_PROFILE_ID),
            portability_envelope_id: String::from(
                TASSADAR_MEMORY64_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
            ),
            max_virtual_address_bits: 64,
            case_specs,
            claim_boundary: String::from(
                "this contract freezes one bounded single-memory memory64 continuation profile over sparse large-address lowering, continuation checkpoints, and typed backend-limit refusal. It does not claim arbitrary Wasm memory64 closure, multi-memory support, or backend-invariant large-address portability",
            ),
            summary: String::new(),
            contract_digest: String::new(),
        };
        contract.summary = format!(
            "Memory64 profile contract freezes {} cases across {} exact and {} refusal expectations.",
            contract.case_specs.len(),
            exact_case_count,
            refusal_case_count,
        );
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_memory64_profile_contract|", &contract);
        contract
    }
}

/// Returns the canonical compiler-owned memory64 profile contract.
#[must_use]
pub fn compile_tassadar_memory64_profile_contract() -> TassadarMemory64ProfileContract {
    TassadarMemory64ProfileContract::new(vec![
        case_spec(
            "sparse_above_4g_resume",
            "memory64_sparse_scan",
            "single_memory_sparse_resume",
            0x1_0000_11ff,
            TassadarMemory64LoweringStatus::Exact,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
            ],
            "single-memory sparse continuation above the 4GiB boundary should remain exact inside the bounded memory64 profile",
        ),
        case_spec(
            "memory_grow_above_4g_resume",
            "memory64_growth_resume",
            "single_memory_growth_resume",
            0x1_0002_0fff,
            TassadarMemory64LoweringStatus::Exact,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
                "fixtures/tassadar/reports/tassadar_resumable_multi_slice_promotion_report.json",
            ],
            "single-memory growth plus continuation above the 4GiB boundary should remain exact only inside the bounded sparse-window profile",
        ),
        case_spec(
            "backend_virtual_address_limit_refusal",
            "memory64_backend_limit_boundary",
            "single_memory_backend_limit_boundary",
            0x2_0000_0000,
            TassadarMemory64LoweringStatus::Refusal,
            Some("backend_virtual_address_limit"),
            &[
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
                "fixtures/tassadar/reports/tassadar_broad_internal_compute_portability_report.json",
            ],
            "unsupported host or backend virtual-address envelopes must stay typed refusals instead of being inferred from smaller-memory success cases",
        ),
    ])
}

fn case_spec(
    case_id: &str,
    workload_family: &str,
    address_shape_id: &str,
    max_virtual_address_touched: u64,
    expected_status: TassadarMemory64LoweringStatus,
    expected_refusal_reason_id: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarMemory64LoweringCaseSpec {
    TassadarMemory64LoweringCaseSpec {
        case_id: String::from(case_id),
        workload_family: String::from(workload_family),
        address_shape_id: String::from(address_shape_id),
        max_virtual_address_touched,
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
        TassadarMemory64LoweringStatus, compile_tassadar_memory64_profile_contract,
    };

    #[test]
    fn memory64_profile_contract_is_machine_legible() {
        let contract = compile_tassadar_memory64_profile_contract();

        assert_eq!(contract.max_virtual_address_bits, 64);
        assert_eq!(contract.case_specs.len(), 3);
        assert!(contract.case_specs.iter().any(|case| {
            case.expected_status == TassadarMemory64LoweringStatus::Exact
                && case.max_virtual_address_touched > u64::from(u32::MAX)
        }));
        assert!(contract.case_specs.iter().any(|case| {
            case.expected_status == TassadarMemory64LoweringStatus::Refusal
                && case.expected_refusal_reason_id.as_deref()
                    == Some("backend_virtual_address_limit")
        }));
    }
}
